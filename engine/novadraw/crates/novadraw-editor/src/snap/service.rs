use super::{
    ControlPoint, DofKey, RuleCall, SceneFeature, SnapError, SnapGeometry, SnapInput,
    SnapRegistration, SnapSession, SnapSettings, SnapSolution, SurfaceTranslationX,
    SurfaceTranslationY,
    geometry::{finite_map, finite_point},
    solver,
};
use crate::{EditPartFactory, EditPartId, GraphicalViewer, ModelAdapter, container::MoveCandidate};
use novadraw::geometry::{Point, Rectangle, Vec2};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use uuid::Uuid;

const MAX_SNAP_PARENT_DEPTH: usize = 10_000;

impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    pub(crate) fn snap_ready(&self) -> Result<(), SnapError> {
        if self.is_faulted() || self.runtime().is_faulted() {
            Err(SnapError::Faulted)
        } else {
            Ok(())
        }
    }
    /// Borrows the Viewer-owned open snapping registry after the fault gate.
    pub fn snapping(&mut self) -> Result<SnapRegistration<'_>, SnapError> {
        self.snap_ready()?;
        Ok(SnapRegistration {
            registry: &mut self.snapping,
        })
    }
    /// Starts a fresh gesture session. Cancellation cannot be reversed.
    pub fn begin_snap_session(&self) -> Result<SnapSession, SnapError> {
        self.snap_ready()?;
        Ok(SnapSession {
            owner: self.namespace(),
            id: Uuid::new_v4(),
            active: true,
            latest_candidate: None,
            latest_solution: None,
            baseline: None,
            metrics: Default::default(),
            clock: None,
            prepared: Vec::new(),
        })
    }

    /// Reads committed primary-Figure corners through checked Core queries.
    /// Invisible parts return empty success; foreign, retired and query failures remain errors.
    pub fn snap_scene_feature(&self, part: EditPartId) -> Result<Option<SceneFeature>, SnapError> {
        self.snap_ready()?;
        let node = self
            .parts()
            .get(part)
            .filter(|n| n.is_active())
            .ok_or(SnapError::Identity)?;
        let figure = node.primary_figure();
        if !self
            .runtime()
            .tree()
            .is_effectively_visible(figure)
            .map_err(crate::ViewerError::Identity)?
        {
            return Ok(None);
        }
        let bounds = self
            .runtime()
            .tree()
            .figure_bounds(figure)
            .map_err(crate::ViewerError::Identity)?;
        if bounds.width < 0.0 || bounds.height < 0.0 {
            return Err(SnapError::Invalid("negative committed Figure bounds"));
        }
        let mapping = finite_map(
            self.runtime()
                .tree()
                .local_to_surface_transform(figure)
                .map_err(crate::ViewerError::Coordinate)?,
        )?;
        let points = [
            Point::new(0.0, 0.0),
            Point::new(bounds.width, 0.0),
            Point::new(bounds.width, bounds.height),
            Point::new(0.0, bounds.height),
        ];
        let corners = points
            .into_iter()
            .map(|p| finite_point(mapping.transform_point(p)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(SceneFeature {
            part,
            corners,
            mapping,
        }))
    }

    /// Builds checked move input from an E03-issued current candidate and explicit layout rules.
    /// No model command or application plan is created by this operation.
    pub fn snap_move_input(
        &mut self,
        candidate: &MoveCandidate<A::ModelId>,
        settings: SnapSettings,
        layouts: Vec<RuleCall>,
    ) -> Result<SnapInput, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let batch = candidate.batch();
            if batch.namespace() != self.namespace()
                || self.model().revision() != batch.revision()
                || self.applied_revision() != batch.revision()
            {
                return Err(SnapError::Invalidated);
            }
            let target = candidate.target();
            let target_map = finite_map(self.child_content_to_surface(target)?)?;
            if target_map != candidate.target_to_surface() {
                return Err(SnapError::Invalidated);
            }
            let mut generations = batch.generations.to_vec();
            generations.push((target, candidate.generation()));
            for &(part, generation) in &generations {
                if self.container_policy_generation(part)? != generation {
                    return Err(SnapError::Invalidated);
                }
            }
            let mut source_maps = Vec::new();
            for (source, &expected) in batch.sources().iter().zip(candidate.source_maps.iter()) {
                let node = self
                    .parts()
                    .get(source.part())
                    .filter(|n| n.is_active())
                    .ok_or(SnapError::Identity)?;
                if node.model_id() != Some(source.model())
                    || self.parts().parent(source.part()) != Some(source.parent())
                    || self
                        .parts()
                        .children(source.parent())
                        .and_then(|s| s.get(source.index()))
                        != Some(&source.part())
                {
                    return Err(SnapError::Invalidated);
                }
                let map = finite_map(self.child_content_to_surface(source.parent())?)?;
                if map != expected {
                    return Err(SnapError::Invalidated);
                }
                source_maps.push((source.parent(), map));
            }
            let sources = batch.sources().iter().map(|s| s.part()).collect::<Vec<_>>();
            let (siblings, scene) = self.snap_scene(target, &sources, settings)?;
            let mut controls = Vec::new();
            let mut anchor_controls = 0..0;
            for (source, geometry) in batch.sources().iter().zip(candidate.geometry()) {
                let start = controls.len();
                for &point in geometry.corners() {
                    controls.push(ControlPoint {
                        origin: finite_point(target_map.transform_point(point))?,
                        influence: vec![Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
                    });
                }
                if source.part() == batch.anchor().part() {
                    anchor_controls = start..controls.len();
                }
            }
            for rule in &layouts {
                self.snapping.check_rule(rule)?;
            }
            let input = SnapInput {
                namespace: self.namespace(),
                candidate: candidate.id(),
                revision: batch.revision(),
                gesture: super::geometry::GestureIdentity::of::<SurfaceTranslationX>(u128::from(
                    batch.interaction_revision().value(),
                )),
                target,
                sources,
                anchor: batch.anchor().part(),
                generations,
                source_maps,
                target_map,
                pointer: finite_point(target_map.transform_point(candidate.pointer()))?,
                dofs: vec![
                    DofKey::of::<SurfaceTranslationX>(),
                    DofKey::of::<SurfaceTranslationY>(),
                ],
                controls,
                anchor_controls,
                scene,
                siblings,
                viewport: self.snap_viewport()?,
                settings,
                layouts,
            };
            self.validate_snap_input(&input)?;
            Ok(input)
        }));
        self.finish_snap_boundary(result, "snap.input")
    }

    fn snap_excluded(&self, part: EditPartId, sources: &[EditPartId]) -> Result<bool, SnapError> {
        let mut current = Some(part);
        for _ in 0..MAX_SNAP_PARENT_DEPTH {
            let Some(part) = current else {
                return Ok(false);
            };
            if sources.contains(&part) {
                return Ok(true);
            }
            current = self.parts().parent(part);
        }
        Err(SnapError::Invalid(
            "snapping source ancestry exceeds supported depth",
        ))
    }
    pub(crate) fn snap_scene(
        &self,
        target: EditPartId,
        sources: &[EditPartId],
        settings: SnapSettings,
    ) -> Result<(Vec<EditPartId>, Vec<SceneFeature>), SnapError> {
        let siblings = self
            .parts()
            .children(target)
            .ok_or(SnapError::Identity)?
            .to_vec();
        if siblings.len() > settings.budget().sources.get() {
            return Err(SnapError::Budget("sibling source budget exceeded"));
        }
        let mut scene = Vec::new();
        for &part in &siblings {
            if self.snap_excluded(part, sources)? {
                continue;
            }
            if let Some(feature) = self.snap_scene_feature(part)? {
                if scene.len() >= settings.budget().features.get() {
                    return Err(SnapError::Budget("scene feature budget exceeded"));
                }
                scene.push(feature);
            }
        }
        Ok((siblings, scene))
    }
    pub(crate) fn snap_viewport(&self) -> Result<(Point, f64, Rectangle), SnapError> {
        let origin = finite_point(self.viewport_origin()?)?;
        let scale = self.viewport_scale()?;
        let bounds = self.viewport_bounds_in_surface()?;
        if !scale.is_finite()
            || scale <= 0.0
            || ![bounds.x, bounds.y, bounds.width, bounds.height]
                .into_iter()
                .all(f64::is_finite)
        {
            return Err(SnapError::Invalid("invalid snapping viewport"));
        }
        Ok((origin, scale, bounds))
    }
    pub(crate) fn validate_snap_input(&self, input: &SnapInput) -> Result<(), SnapError> {
        self.snap_ready()?;
        if input.namespace != self.namespace()
            || self.model().revision() != input.revision
            || self.applied_revision() != input.revision
        {
            return Err(SnapError::Invalidated);
        }
        for &(part, generation) in &input.generations {
            if self.container_policy_generation(part)? != generation {
                return Err(SnapError::Invalidated);
            }
        }
        for &(part, map) in &input.source_maps {
            if self.child_content_to_surface(part)? != map {
                return Err(SnapError::Invalidated);
            }
        }
        if self.child_content_to_surface(input.target)? != input.target_map
            || self.snap_viewport()? != input.viewport
        {
            return Err(SnapError::Invalidated);
        }
        let (siblings, scene) = self.snap_scene(input.target, &input.sources, input.settings)?;
        if siblings != input.siblings || scene != input.scene {
            return Err(SnapError::Invalidated);
        }
        Ok(())
    }

    /// Runs one complete checked solve, preserving the failed-prefix metrics on the session.
    /// Panic marks the Viewer faulted and propagates through the existing Editor panic contract.
    pub fn solve_snapping(
        &mut self,
        session: &mut SnapSession,
        input: SnapInput,
    ) -> Result<SnapSolution, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            solver::solve(&self.snapping, session, input, |input| {
                self.validate_snap_input(input)
            })
        }));
        self.finish_snap_boundary(
            result,
            session.metrics.callback_boundary.unwrap_or("snap.solve"),
        )
    }
    /// Rechecks all retained dependencies and adopted facts against actual scalar plan corrections.
    /// This validates geometry only; E03 still owns the checked plan, release and command gate.
    pub fn validate_snapping_geometry(
        &mut self,
        session: &mut SnapSession,
        solution: &SnapSolution,
        values: Vec<f64>,
    ) -> Result<SnapGeometry, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            solver::validate_final(&self.snapping, session, solution, values, |input| {
                self.validate_snap_input(input)
            })
        }));
        self.finish_snap_boundary(
            result,
            session
                .metrics
                .callback_boundary
                .unwrap_or("snap.final-geometry"),
        )
    }
    pub(crate) fn validate_snapping_plan_geometry(
        &mut self,
        session: &mut SnapSession,
        solution: &SnapSolution,
        geometry: SnapGeometry,
    ) -> Result<SnapGeometry, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            solver::validate_final_geometry(&self.snapping, session, solution, geometry, |input| {
                self.validate_snap_input(input)
            })
        }));
        self.finish_snap_boundary(
            result,
            session
                .metrics
                .callback_boundary
                .unwrap_or("snap.final-plan-geometry"),
        )
    }
    pub(crate) fn finish_snap_boundary<T>(
        &mut self,
        result: std::thread::Result<Result<T, SnapError>>,
        location: &'static str,
    ) -> Result<T, SnapError> {
        match result {
            Ok(value) => value,
            Err(payload) => {
                self.record_extension_panic(payload.as_ref(), location);
                resume_unwind(payload)
            }
        }
    }
}
