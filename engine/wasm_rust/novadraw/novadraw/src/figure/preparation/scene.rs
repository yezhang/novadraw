use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    rc::Rc,
};

use crate::figure::{CapabilityKey, ResolvedStyle};
use crate::graph::FigureTree;
use crate::layout::{LayoutError, LayoutOutput};
use crate::text::TextError;
use crate::{Figure, FigureId, FigureMeasurement, MeasureConstraints, Rectangle};

use super::{
    FigurePresentation, MeasurementSelection, PreparationDependencyError, PreparationMeasureError,
    PreparationMeasureErrorKind, PreparationMeasurement,
};

#[derive(Clone)]
struct MeasurementRecord {
    figure: FigureId,
    constraints: MeasureConstraints,
    selection: MeasurementSelection,
    text_revision: u64,
    measurement: PreparationMeasurement,
    observations: Vec<crate::connection::DependencyObservation>,
}

/// Frozen read access restricted to the preparing Figure's subtree.
pub struct PreparationScene<'a> {
    tree: &'a FigureTree,
    root: FigureId,
    styles: &'a HashMap<FigureId, ResolvedStyle>,
    members: HashSet<FigureId>,
    measurement_path: Rc<RefCell<Vec<FigureId>>>,
    observations: Rc<RefCell<Vec<crate::connection::DependencyObservation>>>,
    measurements: Rc<RefCell<Vec<MeasurementRecord>>>,
    measurement_constraints: Option<(MeasureConstraints, MeasureConstraints)>,
    budget: super::PreparationBudgetHandle,
}

impl<'a> PreparationScene<'a> {
    #[cfg(test)]
    pub(crate) fn new(
        tree: &'a FigureTree,
        root: FigureId,
        styles: &'a HashMap<FigureId, ResolvedStyle>,
    ) -> Result<Self, TextError> {
        Self::with_budget(
            tree,
            root,
            styles,
            super::PreparationBudgetHandle::default(),
        )
    }

    pub(crate) fn with_budget(
        tree: &'a FigureTree,
        root: FigureId,
        styles: &'a HashMap<FigureId, ResolvedStyle>,
        budget: super::PreparationBudgetHandle,
    ) -> Result<Self, TextError> {
        let mut members = HashSet::new();
        budget.allocate("preparation subtree", 1)?;
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            budget.allocate("preparation subtree", 1)?;
            members.insert(id);
            if let Ok(node) = tree.node(id) {
                budget.allocate("preparation subtree", node.children.len() as u64)?;
                pending.extend(node.children.iter().copied());
            }
        }
        Ok(Self {
            tree,
            root,
            styles,
            members,
            measurement_path: Rc::new(RefCell::new(Vec::new())),
            observations: Rc::new(RefCell::new(Vec::new())),
            measurements: Rc::new(RefCell::new(Vec::new())),
            measurement_constraints: None,
            budget,
        })
    }

    /// Shares this stabilization's accounting with an external producer or solver.
    pub fn budget(&self) -> super::PreparationBudgetHandle {
        self.budget.clone()
    }

    pub fn root(&self) -> FigureId {
        self.root
    }

    /// Outer and content constraints of the current measurement callback.
    pub fn measurement_constraints(&self) -> Option<(MeasureConstraints, MeasureConstraints)> {
        self.measurement_constraints
    }

    pub(crate) fn for_measurement(
        &self,
        root: FigureId,
        outer: MeasureConstraints,
        content: MeasureConstraints,
    ) -> Result<PreparationScene<'a>, TextError> {
        let mut scene = Self::with_budget(self.tree, root, self.styles, self.budget.clone())?;
        scene.measurement_path = self.measurement_path.clone();
        scene.observations = self.observations.clone();
        scene.measurements = self.measurements.clone();
        scene.measurement_constraints = Some((outer, content));
        Ok(scene)
    }

    /// Measures the selected Figure through the authoritative Runtime measurement pipeline.
    /// A registered preparation capability is explicitly selected when no override/layout wins.
    pub fn measure_figure(
        &self,
        context: &mut crate::text::MeasureContext<'_>,
        id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        self.measure_selected(context, id, constraints, MeasurementSelection::Both)
    }

    pub(crate) fn measure_selected(
        &self,
        context: &mut crate::text::MeasureContext<'_>,
        id: FigureId,
        constraints: MeasureConstraints,
        selection: MeasurementSelection,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let failure = |kind| PreparationMeasureError {
            figure: id,
            outer_constraints: constraints,
            content_constraints: None,
            kind,
        };
        let budget_failure = |error| {
            failure(PreparationMeasureErrorKind::Dependency(Box::new(
                PreparationDependencyError::Budget(error),
            )))
        };
        self.budget
            .consume("measurement query", 1)
            .map_err(budget_failure)?;
        self.tree.validate_identity(id).map_err(|error| {
            failure(PreparationMeasureErrorKind::Layout(Box::new(
                LayoutError::Identity(error),
            )))
        })?;
        if !self.members.contains(&id) {
            return Err(failure(PreparationMeasureErrorKind::Layout(Box::new(
                LayoutError::InvalidChild {
                    container: self.root,
                    child: id,
                },
            ))));
        }
        {
            let path = self.measurement_path.borrow();
            if path.contains(&id) {
                let mut path = path.clone();
                path.push(id);
                return Err(failure(PreparationMeasureErrorKind::Dependency(Box::new(
                    PreparationDependencyError::MeasurementCycle { path },
                ))));
            }
            if path.len() >= crate::graph::MAX_TREE_DEPTH {
                return Err(failure(PreparationMeasureErrorKind::Dependency(Box::new(
                    PreparationDependencyError::DepthExceeded {
                        limit: crate::graph::MAX_TREE_DEPTH,
                    },
                ))));
            }
        }
        let text_revision = context.revision();
        self.budget
            .consume("measurement cache", self.measurements.borrow().len() as u64)
            .map_err(budget_failure)?;
        let cached = self
            .measurements
            .borrow()
            .iter()
            .find(|record| {
                record.figure == id
                    && record.constraints == constraints
                    && record.selection == selection
                    && record.text_revision == text_revision
            })
            .cloned();
        if let Some(record) = cached {
            self.retain_observations(&record.observations);
            self.budget.check().map_err(budget_failure)?;
            return Ok(record.measurement);
        }
        struct MeasurementGuard(Rc<RefCell<Vec<FigureId>>>);
        impl Drop for MeasurementGuard {
            fn drop(&mut self) {
                self.0.borrow_mut().pop();
            }
        }
        self.budget
            .allocate("measurement path", 1)
            .map_err(budget_failure)?;
        self.measurement_path.borrow_mut().push(id);
        let _guard = MeasurementGuard(self.measurement_path.clone());
        let mut measured_scene = Self::with_budget(self.tree, id, self.styles, self.budget.clone())
            .map_err(|error| {
                failure(match error {
                    TextError::PreparationDependency(error) => {
                        PreparationMeasureErrorKind::Dependency(error)
                    }
                    error => PreparationMeasureErrorKind::Text(Box::new(error)),
                })
            })?;
        measured_scene.measurement_path = self.measurement_path.clone();
        measured_scene.measurements = self.measurements.clone();
        measured_scene.observe(crate::connection::DependencySubject::FigureMeasurementSource(id));
        self.budget.check().map_err(budget_failure)?;
        let result = self.tree.measure_preparation_figure(
            context,
            id,
            constraints,
            &measured_scene,
            selection,
        );
        let observations = measured_scene.observations();
        self.retain_observations(&observations);
        // A nested failure owns its subject and constraints, including budget errors.
        let measurement = result?;
        self.budget.check().map_err(budget_failure)?;
        self.budget
            .allocate("measurement cache", 1)
            .map_err(budget_failure)?;
        self.measurements.borrow_mut().push(MeasurementRecord {
            figure: id,
            constraints,
            selection,
            text_revision,
            measurement,
            observations,
        });
        Ok(measurement)
    }

    fn check(&self, id: FigureId) -> Result<(), TextError> {
        self.budget.consume("preparation query", 1)?;
        if self.members.contains(&id) {
            return Ok(());
        }
        Err(TextError::InvalidPreparedScene(
            "source is outside preparation subtree".into(),
        ))
    }

    pub fn children(&self, id: FigureId) -> Result<Vec<FigureId>, TextError> {
        self.check(id)?;
        self.budget.allocate(
            "children query",
            self.tree.node(id).map_err(scene_error)?.children.len() as u64,
        )?;
        self.tree.child_order(id).map_err(scene_error)
    }

    pub fn parent(&self, id: FigureId) -> Result<Option<FigureId>, TextError> {
        self.check(id)?;
        if id == self.root {
            return Ok(None);
        }
        self.tree.parent_id(id).map_err(scene_error)
    }

    /// Reads another checked immutable result and retains its computation dependencies.
    pub fn prepared_scene(&self, id: FigureId) -> Result<super::PreparedSceneSnapshot, TextError> {
        self.check(id)?;
        let subject = crate::connection::DependencySubject::PreparedScene(id);
        let mut path = vec![crate::connection::DependencySubject::PreparedScene(
            self.root,
        )];
        if self.snapshot_reaches_owner(&subject, &mut path)? {
            return Err(TextError::PreparationDependency(Box::new(
                PreparationDependencyError::DependencyCycle { path },
            )));
        }
        let snapshot = self
            .tree
            .checked_prepared_scene_with_budget(id, &self.budget)
            .map_err(|error| match error {
                crate::connection::RouteSceneQueryError::PreparationBudget(error) => {
                    TextError::from(error)
                }
                error => scene_error(error),
            })?;
        self.observe(subject);
        Ok(snapshot)
    }

    fn snapshot_reaches_owner(
        &self,
        subject: &crate::connection::DependencySubject,
        path: &mut Vec<crate::connection::DependencySubject>,
    ) -> Result<bool, TextError> {
        use crate::connection::DependencySubject;
        let target = DependencySubject::PreparedScene(self.root);
        self.budget.allocate("snapshot reachability", 1)?;
        let mut pending = vec![(subject.clone(), 1)];
        let mut parents: HashMap<DependencySubject, DependencySubject> = HashMap::new();
        let mut visited = HashSet::new();
        while let Some((current, depth)) = pending.pop() {
            self.budget.allocate("snapshot reachability", 1)?;
            if current == target {
                let mut suffix = vec![current.clone()];
                let mut cursor = current;
                while let Some(parent) = parents.get(&cursor) {
                    self.budget.allocate("snapshot cycle path", 1)?;
                    suffix.push(parent.clone());
                    cursor = parent.clone();
                }
                suffix.reverse();
                path.extend(suffix);
                return Ok(true);
            }
            if depth >= crate::graph::MAX_TREE_DEPTH {
                return Err(TextError::PreparationDependency(Box::new(
                    PreparationDependencyError::DepthExceeded {
                        limit: crate::graph::MAX_TREE_DEPTH,
                    },
                )));
            }
            if !visited.insert(current.clone()) {
                continue;
            }
            if let DependencySubject::PreparedScene(owner) = current
                && let Some(prepared) = self
                    .tree
                    .node(owner)
                    .map_err(scene_error)?
                    .prepared
                    .as_ref()
            {
                for observation in &prepared.observations {
                    self.budget.allocate("snapshot reachability", 2)?;
                    if !visited.contains(&observation.subject) {
                        parents
                            .entry(observation.subject.clone())
                            .or_insert(DependencySubject::PreparedScene(owner));
                        pending.push((observation.subject.clone(), depth + 1));
                    }
                }
            }
        }
        Ok(false)
    }

    pub fn figure(&self, id: FigureId) -> Result<&dyn Figure, TextError> {
        self.check(id)?;
        self.observe_borrowed_route_output(id)?;
        Ok(self.tree.node(id).map_err(scene_error)?.figure.as_ref())
    }

    pub fn capability<C: 'static>(
        &self,
        id: FigureId,
        key: CapabilityKey<C>,
    ) -> Result<Option<&C>, TextError> {
        self.check(id)?;
        self.observe_borrowed_route_output(id)?;
        self.tree.capability(id, key).map_err(scene_error)
    }

    // Raw capability/figure borrows can expose route or post-route derived data.
    // Treat access in that subtree as a calculation read; source membership has
    // separate children/parent queries and does not pass through this boundary.
    fn observe_borrowed_route_output(&self, mut id: FigureId) -> Result<(), TextError> {
        let subject = crate::connection::DependencySubject::FigureGeometry(id);
        loop {
            self.budget.consume("route ancestor query", 1)?;
            let node = self.tree.node(id).map_err(scene_error)?;
            if node.connection().is_some() {
                self.observe(subject);
                return Ok(());
            }
            let Some(parent) = node.parent else {
                return Ok(());
            };
            id = parent;
        }
    }

    pub fn style(&self, id: FigureId) -> Result<&ResolvedStyle, TextError> {
        self.check(id)?;
        self.styles
            .get(&id)
            .ok_or_else(|| TextError::InvalidPreparedScene("missing resolved style".into()))
    }

    pub fn client_area(&self, id: FigureId) -> Result<Rectangle, TextError> {
        self.check(id)?;
        self.observe(crate::connection::DependencySubject::FigureGeometry(id));
        Ok(self.tree.node(id).map_err(scene_error)?.client_area())
    }

    /// Resolved ordinary Border metrics, including owner-dependent snapshots.
    pub fn insets(&self, id: FigureId) -> Result<crate::Insets, TextError> {
        self.check(id)?;
        self.observe(crate::connection::DependencySubject::FigureMeasurement(id));
        Ok(self.tree.node(id).map_err(scene_error)?.insets())
    }

    pub fn preferred_measurement(
        &self,
        id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, TextError> {
        self.check(id)?;
        self.observe(crate::connection::DependencySubject::FigureMeasurement(id));
        let node = self.tree.node(id).map_err(scene_error)?;
        if node.preferred_size.is_none()
            && node.layout.manager.is_none()
            && let Some(prepared) = &node.prepared
        {
            self.observe(crate::connection::DependencySubject::PreparedScene(
                prepared.owner,
            ));
        }
        self.tree
            .preferred_measurement(id, constraints)
            .map_err(scene_error)
    }

    pub(crate) fn observe(&self, subject: crate::connection::DependencySubject) {
        use crate::connection::RouteSceneRead;
        let mut observations = self.observations.borrow_mut();
        if self
            .budget
            .consume("dependency observation", observations.len() as u64)
            .is_err()
        {
            return;
        }
        if !observations
            .iter()
            .any(|observation| observation.subject == subject)
        {
            if self.budget.allocate("dependency observation", 1).is_err() {
                return;
            }
            let geometries = HashMap::new();
            let read = crate::connection::FigureTreeRouteSceneRead::with_budget(
                self.tree,
                &geometries,
                self.budget.clone(),
            );
            let generation = read.dependency_generation(&subject);
            if self.budget.check().is_err() {
                return;
            }
            observations.push(crate::connection::DependencyObservation {
                subject,
                generation,
            });
        }
    }

    fn retain_observations(&self, retained: &[crate::connection::DependencyObservation]) {
        let mut observations = self.observations.borrow_mut();
        for observation in retained {
            if self
                .budget
                .consume("dependency observation", observations.len() as u64)
                .is_err()
            {
                return;
            }
            if !observations
                .iter()
                .any(|item| item.subject == observation.subject)
            {
                if self.budget.allocate("dependency observation", 1).is_err() {
                    return;
                }
                observations.push(observation.clone());
            }
        }
    }

    /// A pure producer cache must revalidate and retain every computation read, including
    /// reads made while obtaining its initial measurement demands.
    pub(crate) fn reuse_observations(
        &self,
        retained: &[crate::connection::DependencyObservation],
    ) -> Result<bool, TextError> {
        use crate::connection::RouteSceneRead;
        let geometries = HashMap::new();
        let read = crate::connection::FigureTreeRouteSceneRead::with_budget(
            self.tree,
            &geometries,
            self.budget.clone(),
        );
        for observation in retained {
            self.budget.consume("producer cache dependencies", 1)?;
            if read.dependency_generation(&observation.subject) != observation.generation {
                self.budget.check()?;
                return Ok(false);
            }
        }
        self.retain_observations(retained);
        self.budget.check()?;
        Ok(true)
    }

    pub(crate) fn check_layout_member(&self, id: FigureId) -> Result<(), LayoutError> {
        self.budget
            .consume("layout measurement query", 1)
            .map_err(|error| {
                LayoutError::Preparation(Box::new(PreparationMeasureError {
                    figure: id,
                    outer_constraints: MeasureConstraints::UNBOUNDED,
                    content_constraints: None,
                    kind: PreparationMeasureErrorKind::Dependency(Box::new(
                        PreparationDependencyError::Budget(error),
                    )),
                }))
            })?;
        self.tree.validate_identity(id)?;
        if self.members.contains(&id) {
            Ok(())
        } else {
            Err(LayoutError::InvalidChild {
                container: self.root,
                child: id,
            })
        }
    }

    pub(crate) fn observations(&self) -> Vec<crate::connection::DependencyObservation> {
        let observations = self.observations.borrow();
        if self
            .budget
            .allocate("observation snapshot", observations.len() as u64)
            .is_err()
        {
            return Vec::new();
        }
        observations.clone()
    }

    /// Frozen source prefix. Layout output bounds and cache generations are excluded.
    pub fn source_revision(&self) -> Result<u64, TextError> {
        self.input_revision(false)
    }

    pub(crate) fn source_prefix(&self) -> Result<super::PreparationSource, TextError> {
        let mut prefix = super::PreparationSource::default();
        let mut pending = vec![self.root];
        while let Some(id) = pending.pop() {
            let node = self.tree.node(id).map_err(scene_error)?;
            self.budget.allocate(
                "preparation source prefix",
                1 + node.resource_inputs.len() as u64 + node.children.len() as u64,
            )?;
            let bounds = node.source_bounds;
            let insets = node.insets();
            prefix.0.push(super::PreparationSourceNode {
                id,
                component: node.component_revision,
                resources: node.resource_inputs.clone(),
                provider: node.preparation().map(|provider| provider.revision()),
                parent: node.parent,
                children: node.children.clone(),
                layout: node.layout.source_generation,
                sizes: [node.preferred_size, node.minimum_size, node.maximum_size]
                    .map(|size| size.map(|size| [size.width.to_bits(), size.height.to_bits()])),
                geometry: [
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                    insets.top,
                    insets.right,
                    insets.bottom,
                    insets.left,
                ]
                .map(f64::to_bits),
                style: self.style(id)?.clone(),
            });
            pending.extend(node.children.iter().rev().copied());
        }
        Ok(prefix)
    }

    /// Input revision includes structure, component state, styles and measurement generation.
    pub fn revision(&self) -> Result<u64, TextError> {
        self.input_revision(true)
    }

    fn input_revision(&self, include_derived: bool) -> Result<u64, TextError> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            self.budget.consume("preparation revision", 1)?;
            id.hash(&mut hash);
            let node = self.tree.node(id).map_err(scene_error)?;
            node.component_revision.hash(&mut hash);
            node.resource_inputs.hash(&mut hash);
            if let Some(preparation) = node.preparation() {
                preparation.revision().hash(&mut hash);
            }
            node.parent.hash(&mut hash);
            node.children.hash(&mut hash);
            node.layout.source_generation.hash(&mut hash);
            for size in [node.preferred_size, node.minimum_size, node.maximum_size] {
                size.is_some().hash(&mut hash);
                if let Some(size) = size {
                    size.width.to_bits().hash(&mut hash);
                    size.height.to_bits().hash(&mut hash);
                }
            }
            if include_derived {
                node.layout.generation().hash(&mut hash);
                for value in [
                    node.source_bounds.x,
                    node.source_bounds.y,
                    node.source_bounds.width,
                    node.source_bounds.height,
                ] {
                    value.to_bits().hash(&mut hash);
                }
            }
            let bounds = if include_derived {
                node.bounds
            } else {
                node.source_bounds
            };
            for value in [
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                node.insets().top,
                node.insets().right,
                node.insets().bottom,
                node.insets().left,
            ] {
                value.to_bits().hash(&mut hash);
            }
            let style = self.style(id)?;
            style.font.hash(&mut hash);
            for value in style
                .foreground
                .to_rgba_array()
                .into_iter()
                .chain(style.background.to_rgba_array())
            {
                value.to_bits().hash(&mut hash);
            }
            style.alpha.to_bits().hash(&mut hash);
            std::mem::discriminant(&style.cursor).hash(&mut hash);
            style.tooltip.hash(&mut hash);
            for child in node.children.iter().rev() {
                self.budget.allocate("preparation revision", 1)?;
                stack.push(*child);
            }
        }
        Ok(hash.finish())
    }
}

pub(crate) fn scene_error(error: impl std::fmt::Display) -> TextError {
    TextError::InvalidPreparedScene(error.to_string())
}

/// Candidate drawing plus ordinary, parent-owned layout effects.
/// Runtime validates the complete batch before publishing any part.
pub struct PreparedScene {
    pub(crate) root: FigurePresentation,
    pub(crate) layouts: Vec<(FigureId, LayoutOutput)>,
    pub(crate) descendants: Vec<(FigureId, FigurePresentation)>,
    pub(crate) result_version: Option<u64>,
    pub(crate) required_routes: Vec<super::RequiredRouteGroup>,
}

impl PreparedScene {
    pub fn new(root: FigurePresentation) -> Self {
        Self {
            root,
            layouts: Vec::new(),
            descendants: Vec::new(),
            result_version: None,
            required_routes: Vec::new(),
        }
    }

    pub fn root_presentation(&self) -> &FigurePresentation {
        &self.root
    }

    /// Producer-owned stable version of the domain result and its complete input prefix.
    /// Equal versions must describe equivalent payload, placement, attachments and paths.
    pub fn with_result_version(mut self, version: u64) -> Self {
        self.result_version = Some(version);
        self
    }

    /// Declares required route publication without adding a computation dependency.
    pub fn require_route_group(
        &mut self,
        scope: crate::connection::CoordinateSpace,
        connections: impl IntoIterator<Item = crate::connection::ConnectionId>,
    ) {
        self.required_routes.push(super::RequiredRouteGroup {
            scope,
            connections: connections.into_iter().collect(),
        });
    }

    /// Composes root drawing while retaining the candidate's descendant and layout effects.
    pub fn with_root_presentation(mut self, root: FigurePresentation) -> Self {
        self.root = root;
        self
    }

    pub fn layout(&mut self, parent: FigureId, output: LayoutOutput) {
        self.layouts.push((parent, output));
    }

    pub fn present(&mut self, source: FigureId, presentation: FigurePresentation) {
        self.descendants.push((source, presentation));
    }

    pub(crate) fn validate(&self, scene: &PreparationScene<'_>) -> Result<(), TextError> {
        scene.budget.check()?;
        use super::{PreparationRouteError, PreparationRouteErrorKind};
        let mut required = HashSet::new();
        for group in &self.required_routes {
            let fail = |connection, kind| {
                TextError::PreparationRoute(Box::new(PreparationRouteError {
                    owner: scene.root,
                    connection,
                    kind,
                }))
            };
            if group.scope != crate::connection::CoordinateSpace::ChildContent(scene.root) {
                return Err(fail(
                    None,
                    PreparationRouteErrorKind::InvalidScope(group.scope),
                ));
            }
            for connection in &group.connections {
                scene.budget.allocate("required route validation", 1)?;
                if !required.insert(*connection) {
                    return Err(fail(
                        Some(*connection),
                        PreparationRouteErrorKind::DuplicateMember,
                    ));
                }
                let parent = scene.tree.parent_id(connection.figure()).map_err(|error| {
                    fail(
                        Some(*connection),
                        PreparationRouteErrorKind::Connection(Box::new(error.into())),
                    )
                })?;
                if parent != Some(scene.root) {
                    return Err(fail(
                        Some(*connection),
                        PreparationRouteErrorKind::OutsideScope,
                    ));
                }
            }
        }
        let mut pending = scene
            .observations()
            .into_iter()
            .map(|observation| observation.subject)
            .collect::<Vec<_>>();
        let mut visited = HashSet::new();
        while let Some(subject) = pending.pop() {
            scene
                .budget
                .allocate("candidate dependency validation", 1)?;
            if !visited.insert(subject.clone()) {
                continue;
            }
            use crate::connection::DependencySubject;
            let figures = match &subject {
                DependencySubject::FigureGeometry(id)
                | DependencySubject::FigureMeasurement(id)
                | DependencySubject::FigureMeasurementSource(id)
                | DependencySubject::NamedAnchorRegion(id, _) => vec![*id],
                DependencySubject::RelativeTransform(from, to) => [from, to]
                    .into_iter()
                    .filter_map(|space| match space {
                        crate::connection::CoordinateSpace::FigureLocal(id)
                        | crate::connection::CoordinateSpace::ChildContent(id) => Some(*id),
                        crate::connection::CoordinateSpace::LogicalSurface => None,
                    })
                    .collect(),
                // Source membership/visibility do not read route output.
                _ => Vec::new(),
            };
            for connection in &required {
                for figure in &figures {
                    let mut cursor = Some(*figure);
                    let mut reads_route = false;
                    while let Some(id) = cursor {
                        scene.budget.consume("candidate route ancestry", 1)?;
                        if id == connection.figure() {
                            reads_route = true;
                            break;
                        }
                        cursor = scene.tree.node(id).map_err(scene_error)?.parent;
                    }
                    if reads_route {
                        return Err(TextError::PreparationDependency(Box::new(
                            PreparationDependencyError::ForbiddenRouteOutputDependency {
                                owner: scene.root,
                                connection: *connection,
                                subject,
                            },
                        )));
                    }
                }
            }
            if let DependencySubject::PreparedScene(owner) = subject
                && let Some(prepared) = scene
                    .tree
                    .node(owner)
                    .map_err(scene_error)?
                    .prepared
                    .as_ref()
            {
                scene.budget.allocate(
                    "candidate dependency validation",
                    prepared.observations.len() as u64,
                )?;
                pending.extend(
                    prepared
                        .observations
                        .iter()
                        .map(|observation| observation.subject.clone()),
                );
            }
        }
        let mut sources = std::collections::HashSet::new();
        for (id, _) in &self.descendants {
            scene.budget.allocate("presentation validation", 1)?;
            scene.check(*id)?;
            if *id == scene.root || !sources.insert(*id) {
                return Err(TextError::InvalidPreparedScene(
                    "duplicate presentation source".into(),
                ));
            }
        }
        for (parent, output) in &self.layouts {
            scene.budget.consume("layout output validation", 1)?;
            scene.check(*parent)?;
            scene
                .tree
                .validate_layout_output(*parent, output)
                .map_err(|error: LayoutError| scene_error(error))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        figure::RectangleFigure,
        layout::StackLayout,
        text::{FontDescriptor, TextSystem},
    };

    #[test]
    fn cached_layout_measurement_retains_transitive_observations() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .unwrap();
        let child = tree
            .builder()
            .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 20.0)))
            .unwrap();
        tree.builder()
            .set_layout_manager(root, Box::new(StackLayout::new()))
            .unwrap();
        let styles = HashMap::new();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let mut text = TextSystem::new();
        let mut context = text.measure(FontDescriptor::default());
        let first = scene
            .measure_figure(&mut context, root, MeasureConstraints::UNBOUNDED)
            .unwrap();
        let observations = scene.observations();
        assert!(observations.iter().any(|observation| observation.subject
            == crate::connection::DependencySubject::FigureMeasurementSource(child)));
        let entries = scene.measurements.borrow().len();
        scene.observations.borrow_mut().clear();
        assert_eq!(
            scene
                .measure_figure(&mut context, root, MeasureConstraints::UNBOUNDED)
                .unwrap(),
            first
        );
        assert_eq!(scene.measurements.borrow().len(), entries);
        assert_eq!(scene.observations(), observations);
    }
}
