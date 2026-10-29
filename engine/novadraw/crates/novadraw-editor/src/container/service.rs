use novadraw::geometry::{Affine2D, Point};
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

use super::{
    geometry::{finite_transform, inverse, map_point},
    *,
};
use crate::{
    ContributionLimit, EditPartId, InteractionRevision, ModelAdapter, PartKind, RequestMetadata,
    ToolContext, ToolError,
};

type PermissionGroup<I> = (EditPartId, PermissionKind, Vec<MoveSource<I>>);

impl<A: ModelAdapter + 'static> ToolContext<'_, A> {
    /// Captures an ordered, ancestor-deduplicated move with one explicit or first-source anchor.
    /// `pressed` may be a covered descendant; its selected ancestor becomes the unique anchor.
    pub fn begin_container_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        input: &[EditPartId],
        pressed: Option<EditPartId>,
        start: Point,
        revision: InteractionRevision,
    ) -> Result<MoveBatch<A::ModelId>, ToolError> {
        self.synchronize_model()?;
        super::geometry::finite_point(start)?;
        let selected: HashSet<_> = input.iter().copied().collect();
        if selected.len() != input.len() {
            return Err(ContainerEditError::Batch("duplicate container input").into());
        }
        for &part in input {
            self.checked_model_part(part)?;
        }
        if let Some(part) = pressed {
            self.checked_model_part(part)?;
        }
        let mut sources = Vec::new();
        for &part in input {
            let node = self
                .part_tree()
                .get(part)
                .ok_or(ContainerEditError::Identity(part))?;
            if node.kind() == PartKind::Connection {
                continue;
            }
            if self.ancestor_selected(part, &selected)? {
                continue;
            }
            let parent = self
                .part_tree()
                .parent(part)
                .ok_or(ContainerEditError::Identity(part))?;
            self.checked_model_part(parent)?;
            let parent_node = self
                .part_tree()
                .get(parent)
                .ok_or(ContainerEditError::Identity(parent))?;
            let index = self
                .part_tree()
                .children(parent)
                .and_then(|children| children.iter().position(|child| *child == part))
                .ok_or(ContainerEditError::Batch(
                    "source is absent from its original parent",
                ))?;
            sources.push(MoveSource {
                part,
                model: node.model_id().ok_or(ContainerEditError::Identity(part))?,
                parent,
                parent_model: parent_node
                    .model_id()
                    .ok_or(ContainerEditError::Identity(parent))?,
                index,
            });
        }
        if sources.is_empty() {
            return Err(ContainerEditError::Batch("empty containment move").into());
        }
        let anchor = if let Some(pressed) = pressed {
            sources
                .iter()
                .position(|source| source.part == pressed)
                .or_else(|| {
                    let mut current = self.part_tree().parent(pressed);
                    for _ in 0..MAX_CONTAINMENT_DEPTH {
                        let part = current?;
                        if let Some(index) = sources.iter().position(|source| source.part == part) {
                            return Some(index);
                        }
                        current = self.part_tree().parent(part);
                    }
                    None
                })
                .ok_or(ContainerEditError::Batch(
                    "pressed anchor is not covered by this operation batch",
                ))?
        } else {
            0
        };
        let geometry = sources
            .iter()
            .map(|source| P::source_geometry(self.model(), source))
            .collect::<Result<Vec<_>, _>>()?;
        let anchor_to_surface = self.child_content_to_surface(sources[anchor].parent)?;
        let surface_to_parent = inverse(anchor_to_surface)?.ok_or(ContainerEditError::Singular)?;
        let parent_grab = map_point(surface_to_parent, start)?;
        let local_to_parent = geometry[anchor].local_to_parent();
        let (grab, grab_to_parent) = match inverse(local_to_parent)? {
            Some(parent_to_local) => (map_point(parent_to_local, parent_grab)?, local_to_parent),
            None => (parent_grab, Affine2D::IDENTITY),
        };
        let mut generations = Vec::new();
        let mut generation_hosts = HashSet::new();
        for source in &sources {
            for host in [source.part, source.parent] {
                if generation_hosts.insert(host) {
                    generations.push((host, self.policy_generation(host)));
                }
            }
        }
        Ok(MoveBatch {
            namespace: self.part_tree().namespace(),
            revision: self.model().revision(),
            interaction: revision,
            input: Arc::from(input),
            sources: Arc::from(sources),
            anchor,
            grab,
            grab_to_parent,
            geometry: Arc::from(geometry),
            generations: Arc::from(generations),
        })
    }

    fn checked_model_part(&self, part: EditPartId) -> Result<(), ContainerEditError> {
        if self.part_tree().get(part).is_none_or(|node| {
            !node.is_active() || node.model_id().is_none() || node.kind() == PartKind::Root
        }) {
            return Err(ContainerEditError::Identity(part));
        }
        Ok(())
    }

    fn ancestor_selected(
        &self,
        part: EditPartId,
        selected: &HashSet<EditPartId>,
    ) -> Result<bool, ContainerEditError> {
        let mut current = self.part_tree().parent(part);
        for _ in 0..MAX_CONTAINMENT_DEPTH {
            let Some(parent) = current else {
                return Ok(false);
            };
            if selected.contains(&parent) {
                return Ok(true);
            }
            current = self.part_tree().parent(parent);
        }
        Err(ContainerEditError::Batch("containment depth exceeds 10000"))
    }

    /// Rejects any external model, source-parent, or relevant policy change.
    pub fn validate_container_move(
        &mut self,
        batch: &MoveBatch<A::ModelId>,
    ) -> Result<(), ToolError> {
        self.synchronize_model()?;
        if self.part_tree().namespace() != batch.namespace
            || self.model().revision() != batch.revision
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        for source in batch.sources.iter() {
            self.checked_model_part(source.part)?;
            if self.part_tree().parent(source.part) != Some(source.parent)
                || self
                    .part_tree()
                    .get(source.part)
                    .and_then(|node| node.model_id())
                    != Some(source.model)
                || self
                    .part_tree()
                    .get(source.parent)
                    .and_then(|node| node.model_id())
                    != Some(source.parent_model)
                || self
                    .part_tree()
                    .children(source.parent)
                    .and_then(|children| children.get(source.index))
                    != Some(&source.part)
            {
                return Err(ContainerEditError::Invalidated.into());
            }
        }
        if batch
            .generations
            .iter()
            .any(|(part, generation)| self.policy_generation(*part) != *generation)
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        Ok(())
    }

    /// Maps one candidate from current transforms and the batch's unique anchor displacement.
    pub fn container_move_candidate(
        &mut self,
        batch: &MoveBatch<A::ModelId>,
        target: EditPartId,
        pointer: Point,
        revision: InteractionRevision,
    ) -> Result<MoveCandidate<A::ModelId>, ToolError> {
        self.validate_container_move(batch)?;
        self.checked_model_part(target)?;
        if self
            .part_tree()
            .get(target)
            .is_none_or(|node| node.kind() != PartKind::Containment)
        {
            return Err(ContainerEditError::Identity(target).into());
        }
        let source_set = batch
            .sources
            .iter()
            .map(|source| source.part)
            .collect::<HashSet<_>>();
        if source_set.contains(&target) || self.ancestor_selected(target, &source_set)? {
            return Err(ContainerEditError::Batch(
                "container move would create a containment cycle",
            )
            .into());
        }
        let target_to_surface = self.child_content_to_surface(target)?;
        let target_inverse = inverse(target_to_surface)?.ok_or(ContainerEditError::Singular)?;
        let anchor_map = finite_transform(
            self.child_content_to_surface(batch.anchor().parent)? * batch.grab_to_parent,
        )?;
        let anchor_surface = map_point(anchor_map, batch.grab)?;
        let delta = super::geometry::finite_point(Point::new(
            pointer.x() - anchor_surface.x(),
            pointer.y() - anchor_surface.y(),
        ))?;
        let translation = Affine2D::from_translation(delta.x(), delta.y());
        let mut geometry = Vec::with_capacity(batch.sources.len());
        let mut source_maps = Vec::with_capacity(batch.sources.len());
        for (source, geometry_source) in batch.sources.iter().zip(batch.geometry.iter()) {
            let source_map = self.child_content_to_surface(source.parent)?;
            let composite =
                finite_transform(finite_transform(target_inverse * translation)? * source_map)?;
            geometry.push(geometry_source.mapped(composite)?);
            source_maps.push(source_map);
        }
        Ok(MoveCandidate {
            id: CandidateId(Uuid::new_v4()),
            batch: batch.clone(),
            target,
            target_model: self
                .part_tree()
                .get(target)
                .and_then(|node| node.model_id())
                .ok_or(ContainerEditError::Identity(target))?,
            generation: self.policy_generation(target),
            interaction: revision,
            geometry: geometry.into(),
            pointer: map_point(target_inverse, pointer)?,
            target_to_surface,
            translation,
            source_maps: source_maps.into(),
        })
    }

    fn validate_move_candidate(
        &mut self,
        candidate: &MoveCandidate<A::ModelId>,
    ) -> Result<(), ToolError> {
        self.validate_container_move(&candidate.batch)?;
        if self.policy_generation(candidate.target) != candidate.generation
            || self.child_content_to_surface(candidate.target)? != candidate.target_to_surface
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        for (source, map) in candidate
            .batch
            .sources
            .iter()
            .zip(candidate.source_maps.iter())
        {
            if self.child_content_to_surface(source.parent)? != *map {
                return Err(ContainerEditError::Invalidated.into());
            }
        }
        Ok(())
    }

    fn permission_queries<P: ContainerMoveProtocol<A>>(
        &self,
        candidate: &MoveCandidate<A::ModelId>,
    ) -> Vec<PermissionQuery<A::ModelId>> {
        let mut groups: Vec<PermissionGroup<A::ModelId>> = Vec::new();
        let local = candidate.local_sources().cloned().collect::<Vec<_>>();
        if !local.is_empty() {
            groups.push((candidate.target, PermissionKind::LocalModification, local));
        }
        let mut removal_groups = HashMap::<EditPartId, usize>::new();
        for source in candidate.incoming_sources() {
            if let Some(&index) = removal_groups.get(&source.parent) {
                groups[index].2.push(source.clone());
            } else {
                removal_groups.insert(source.parent, groups.len());
                groups.push((source.parent, PermissionKind::Removal, vec![source.clone()]));
            }
        }
        let full_candidate = Arc::new(candidate.clone());
        groups
            .into_iter()
            .map(|(host, kind, sources)| PermissionQuery {
                candidate: candidate.id,
                full_candidate: full_candidate.clone(),
                protocol: TypeId::of::<P::Request>(),
                host,
                kind,
                sources: sources.into(),
                revision: candidate.batch.revision,
                generation: self.policy_generation(host),
                interaction: candidate.interaction,
            })
            .collect()
    }

    /// Aggregates a raw target plan, checks its owned header, and obtains all real permissions.
    /// Absence of a target plan permits ancestor probing; rejection never does.
    pub fn resolve_container_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        candidate: &MoveCandidate<A::ModelId>,
    ) -> Result<Option<CheckedMovePlan<A, P>>, ToolError> {
        self.validate_move_candidate(candidate)?;
        let metadata = RequestMetadata::new(
            candidate.interaction,
            candidate
                .batch
                .sources
                .iter()
                .map(|source| source.part)
                .collect::<Vec<_>>(),
        );
        let mut plans = self
            .aggregate_at::<P::Request>(
                candidate.target,
                &metadata,
                &P::payload(candidate),
                ContributionLimit::AtMostOne,
            )?
            .into_contributions();
        let Some(plan) = plans.pop() else {
            return Ok(None);
        };
        let header = plan.header().clone();
        if header != MovePlanHeader::from_candidate(candidate) {
            return Err(ContainerEditError::Mismatch(
                "container plan header differs from the current candidate",
            )
            .into());
        }
        let queries = self.permission_queries::<P>(candidate);
        let mut receipts = Vec::with_capacity(queries.len());
        for query in queries {
            let metadata = RequestMetadata::new(
                query.interaction,
                query
                    .sources
                    .iter()
                    .map(|source| source.part)
                    .collect::<Vec<_>>(),
            );
            let mut decisions = self
                .aggregate_at::<CheckContainerPermission>(
                    query.host,
                    &metadata,
                    &query,
                    ContributionLimit::AtMostOne,
                )?
                .into_contributions();
            match decisions.pop() {
                Some(PermissionDecision::Allow) => {}
                Some(PermissionDecision::Reject(reason)) => {
                    return Err(ContainerEditError::Rejected(reason).into());
                }
                None => {
                    return Err(ContainerEditError::Rejected(
                        "required parent permission is unsupported".into(),
                    )
                    .into());
                }
            }
            if self.model().revision() != query.revision
                || self.policy_generation(query.host) != query.generation
            {
                return Err(ContainerEditError::Invalidated.into());
            }
            receipts.push(PermissionReceipt { query });
        }
        self.validate_move_candidate(candidate)?;
        Ok(Some(CheckedMovePlan {
            plan,
            header,
            receipts,
            candidate: candidate.clone(),
        }))
    }

    /// Validates a held plan and publishes feedback through the common overlay service.
    pub fn show_container_move_feedback<P: ContainerMoveProtocol<A>>(
        &mut self,
        checked: &CheckedMovePlan<A, P>,
    ) -> Result<Vec<novadraw::tree::FigureId>, ToolError> {
        self.validate_move_candidate(&checked.candidate)?;
        let visuals = checked.plan.feedback()?;
        self.attach_feedback(checked.header.target(), visuals)
    }

    /// Revalidates after caller cleanup, then builds and stages exactly one guarded command.
    pub fn submit_container_move<P: ContainerMoveProtocol<A>>(
        &mut self,
        checked: CheckedMovePlan<A, P>,
    ) -> Result<(), ToolError> {
        self.validate_move_candidate(&checked.candidate)?;
        if let Some(command) = checked.plan.build_command(self.model())? {
            self.submit_command(super::guard_command(command, checked.header.revision()))?;
        }
        Ok(())
    }
}
