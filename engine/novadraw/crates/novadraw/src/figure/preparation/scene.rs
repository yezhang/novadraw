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
    input_proofs: Vec<super::InputProof>,
}

#[derive(Default)]
struct InputProofLog {
    ordered: Vec<super::InputProof>,
    failure: Option<TextError>,
}

/// Keep the first read and its order while indexing typed subjects for deduplication.
#[derive(Default)]
struct ObservationLog {
    ordered: Vec<crate::connection::DependencyObservation>,
    subjects: HashSet<crate::connection::DependencySubject>,
}

struct RevisionBase {
    value: u64,
    providers: Vec<FigureId>,
}

struct RootInputs {
    members: Rc<HashSet<FigureId>>,
    route_members: HashSet<FigureId>,
    revision_bases: Rc<RefCell<[Option<RevisionBase>; 2]>>,
    source: RefCell<Option<SourceInputs>>,
}

struct SourceInputs {
    prefix: super::PreparationSource,
    providers: Vec<usize>,
}

/// Cache authority is the immutable tree/style borrow, never the budget's lifetime.
pub(crate) struct PreparationInputs<'a> {
    tree: &'a FigureTree,
    styles: &'a HashMap<FigureId, ResolvedStyle>,
    budget: super::PreparationBudgetHandle,
    roots: RefCell<HashMap<FigureId, Rc<RootInputs>>>,
    fonts: Rc<RefCell<HashSet<crate::text::FontDescriptor>>>,
}

impl<'a> PreparationInputs<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        styles: &'a HashMap<FigureId, ResolvedStyle>,
        budget: super::PreparationBudgetHandle,
    ) -> Result<Rc<Self>, TextError> {
        budget.allocate("preparation input cache", 2)?;
        Ok(Rc::new(Self {
            tree,
            styles,
            budget,
            roots: RefCell::new(HashMap::new()),
            fonts: Rc::new(RefCell::new(HashSet::new())),
        }))
    }

    pub(crate) fn scene(
        self: &Rc<Self>,
        root: FigureId,
    ) -> Result<PreparationScene<'a>, TextError> {
        self.budget.consume("preparation input query", 1)?;
        let mut roots = self.roots.borrow_mut();
        let inputs = if let Some(inputs) = roots.get(&root) {
            inputs.clone()
        } else {
            // Root entry, membership, revision slots and source cache precede allocation.
            self.budget.allocate("preparation root inputs", 5)?;
            let mut members = HashSet::new();
            let mut route_members = HashSet::new();
            let mut ancestor = Some(root);
            let mut inherited_route = false;
            while let Some(id) = ancestor {
                self.budget.consume("preparation root route ancestry", 1)?;
                let node = self.tree.node(id).map_err(scene_error)?;
                if node.connection().is_some() {
                    inherited_route = true;
                    break;
                }
                ancestor = node.parent;
            }
            self.budget.allocate("preparation subtree", 2)?;
            let mut pending = vec![(root, inherited_route)];
            while let Some((id, inherited_route)) = pending.pop() {
                self.budget.allocate("preparation subtree", 1)?;
                members.insert(id);
                if let Ok(node) = self.tree.node(id) {
                    let reads_route = inherited_route || node.connection().is_some();
                    if reads_route {
                        self.budget.allocate("preparation route member", 1)?;
                        route_members.insert(id);
                    }
                    self.budget
                        .allocate("preparation subtree", 2 * node.children.len() as u64)?;
                    pending.extend(node.children.iter().map(|&child| (child, reads_route)));
                }
            }
            let inputs = Rc::new(RootInputs {
                members: Rc::new(members),
                route_members,
                revision_bases: Rc::new(RefCell::new([None, None])),
                source: RefCell::new(None),
            });
            roots.insert(root, inputs.clone());
            inputs
        };
        Ok(PreparationScene {
            tree: self.tree,
            root,
            styles: self.styles,
            members: inputs.members.clone(),
            measurement_path: Rc::new(RefCell::new(Vec::new())),
            observations: Rc::new(RefCell::new(ObservationLog::default())),
            input_proofs: Rc::new(RefCell::new(InputProofLog::default())),
            measurements: Rc::new(RefCell::new(Vec::new())),
            measurement_constraints: None,
            revision_bases: inputs.revision_bases.clone(),
            parsed_fonts: self.fonts.clone(),
            inputs: self.clone(),
            root_inputs: inputs,
            budget: self.budget.clone(),
        })
    }
}

impl ObservationLog {
    fn push(&mut self, observation: crate::connection::DependencyObservation) {
        self.subjects.insert(observation.subject.clone());
        self.ordered.push(observation);
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.ordered.clear();
        self.subjects.clear();
    }
}

/// Frozen read access restricted to the preparing Figure's subtree.
pub struct PreparationScene<'a> {
    tree: &'a FigureTree,
    root: FigureId,
    styles: &'a HashMap<FigureId, ResolvedStyle>,
    members: Rc<HashSet<FigureId>>,
    measurement_path: Rc<RefCell<Vec<FigureId>>>,
    observations: Rc<RefCell<ObservationLog>>,
    input_proofs: Rc<RefCell<InputProofLog>>,
    measurements: Rc<RefCell<Vec<MeasurementRecord>>>,
    measurement_constraints: Option<(MeasureConstraints, MeasureConstraints)>,
    revision_bases: Rc<RefCell<[Option<RevisionBase>; 2]>>,
    parsed_fonts: Rc<RefCell<HashSet<crate::text::FontDescriptor>>>,
    inputs: Rc<PreparationInputs<'a>>,
    root_inputs: Rc<RootInputs>,
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
        PreparationInputs::new(tree, styles, budget)?.scene(root)
    }

    /// Shares this stabilization's accounting with an external producer or solver.
    pub fn budget(&self) -> super::PreparationBudgetHandle {
        self.budget.clone()
    }

    pub fn root(&self) -> FigureId {
        self.root
    }

    // Diagnostics read already frozen nodes; no provider callbacks, dependency writes or charges.
    pub(crate) fn diagnostic_component_revision(&self, id: FigureId) -> u64 {
        self.tree
            .node(id)
            .expect("captured flow source")
            .component_revision
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
        self.check(root)?;
        let mut scene = self.inputs.scene(root)?;
        scene.measurement_path = self.measurement_path.clone();
        scene.observations = self.observations.clone();
        scene.input_proofs = self.input_proofs.clone();
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
            self.retain_input_proofs(&record.input_proofs)
                .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
            self.validate_input_proofs("measurement cache return")
                .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
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
        // LayoutManagers retain their caller's read scope. A preparation callback is
        // narrowed to its own subtree by for_measurement before it receives this view.
        let measured_scene = Self {
            tree: self.tree,
            root: self.root,
            styles: self.styles,
            members: self.members.clone(),
            measurement_path: self.measurement_path.clone(),
            observations: Rc::new(RefCell::new(ObservationLog::default())),
            input_proofs: Rc::new(RefCell::new(InputProofLog::default())),
            measurements: self.measurements.clone(),
            measurement_constraints: self.measurement_constraints,
            revision_bases: self.revision_bases.clone(),
            parsed_fonts: self.parsed_fonts.clone(),
            inputs: self.inputs.clone(),
            root_inputs: self.root_inputs.clone(),
            budget: self.budget.clone(),
        };
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
        measured_scene
            .validate_input_proofs("measurement return")
            .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
        let input_proofs = measured_scene
            .input_proofs()
            .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
        self.retain_input_proofs(&input_proofs)
            .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
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
            input_proofs,
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
                crate::connection::RouteSceneQueryError::PreparationInput(error) => *error,
                crate::connection::RouteSceneQueryError::PreparationBudget(error) => {
                    TextError::from(error)
                }
                error => scene_error(error),
            })?;
        self.observe(subject);
        self.retain_input_proofs(snapshot.input_proofs())?;
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
                for observation in prepared.observations.iter() {
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
    fn observe_borrowed_route_output(&self, id: FigureId) -> Result<(), TextError> {
        self.budget.consume("route membership query", 1)?;
        if self.root_inputs.route_members.contains(&id) {
            self.observe(crate::connection::DependencySubject::FigureGeometry(id));
        }
        self.budget.check()?;
        Ok(())
    }

    pub fn style(&self, id: FigureId) -> Result<&ResolvedStyle, TextError> {
        self.check(id)?;
        self.styles
            .get(&id)
            .ok_or_else(|| TextError::InvalidPreparedScene("missing resolved style".into()))
    }

    pub(crate) fn parsed_font(
        &self,
        id: FigureId,
    ) -> Result<crate::text::FontDescriptor, TextError> {
        let style = self.style(id)?;
        let mut fonts = self.parsed_fonts.borrow_mut();
        if let Some(font) = fonts.get(&style.font) {
            return Ok(font.clone());
        }
        let font = &style.font;
        self.budget
            .allocate("preparation font descriptor", font.family.capacity() as u64)?;
        fonts.insert(font.clone());
        Ok(font.clone())
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
        if self.budget.consume("dependency observation", 1).is_err() {
            return;
        }
        if !observations.subjects.contains(&subject) {
            // Reserve both the ordered record and the subject index before inserting.
            if self.budget.allocate("dependency observation", 2).is_err() {
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
            if self.budget.consume("dependency observation", 1).is_err() {
                return;
            }
            if !observations.subjects.contains(&observation.subject) {
                if self.budget.allocate("dependency observation", 2).is_err() {
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
            .allocate("observation snapshot", observations.ordered.len() as u64)
            .is_err()
        {
            return Vec::new();
        }
        observations.ordered.clone()
    }

    /// Captures a registered external input and retains its first value for all consumers.
    /// Repeated reads of the same subject must agree; ignoring a conflict cannot clear it.
    pub fn input<P: super::PreparationInputPayload>(
        &self,
        registry: &super::PreparationInputRegistry,
        key: super::PreparationInputKey<P>,
        source: FigureId,
    ) -> Result<super::PreparationInputSnapshot, TextError> {
        if let Some(error) = &self.input_proofs.borrow().failure {
            return Err(error.clone());
        }
        let proof = match self
            .check(source)
            .and_then(|()| registry.capture(key, source, self))
        {
            Ok(proof) => proof,
            Err(error) => {
                self.input_proofs.borrow_mut().failure = Some(error.clone());
                return Err(error);
            }
        };
        let value = proof.first.clone();
        self.retain_input_proofs(&[proof])?;
        Ok(value)
    }

    pub(crate) fn input_scope(
        &self,
        root: FigureId,
        source: FigureId,
        constraints: Option<(MeasureConstraints, MeasureConstraints)>,
    ) -> Result<PreparationScene<'a>, TextError> {
        self.check(root)?;
        let mut scene = self.inputs.scene(root)?;
        scene.check(source)?;
        scene.measurement_constraints = constraints;
        Ok(scene)
    }

    pub(crate) fn input_proofs(&self) -> Result<Vec<super::InputProof>, TextError> {
        let proofs = self.input_proofs.borrow();
        if let Some(error) = &proofs.failure {
            return Err(error.clone());
        }
        if proofs.ordered.is_empty() {
            return Ok(Vec::new());
        }
        self.budget.allocate(
            "preparation input receipts",
            proofs.ordered.len() as u64 * std::mem::size_of::<super::InputProof>() as u64,
        )?;
        Ok(proofs.ordered.clone())
    }

    pub(crate) fn retain_input_proofs(
        &self,
        incoming: &[super::InputProof],
    ) -> Result<(), TextError> {
        let mut proofs = self.input_proofs.borrow_mut();
        if let Some(error) = &proofs.failure {
            return Err(error.clone());
        }
        if incoming.is_empty() {
            return Ok(());
        }
        if let Err(error) = super::merge_proofs(&mut proofs.ordered, incoming, &self.budget) {
            proofs.failure = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn validate_input_proofs(&self, checkpoint: &'static str) -> Result<(), TextError> {
        let _timing = self.budget.time("preparation-input-checkpoint");
        let proofs = self.input_proofs.borrow();
        if let Some(error) = &proofs.failure {
            return Err(error.clone());
        }
        if proofs.ordered.is_empty() {
            return Ok(());
        }
        // Validation readers receive an independent input_scope collector.
        // They cannot append to this log, so the ordered first values can be
        // borrowed for the entire check without allocating a receipt copy.
        let result = proofs
            .ordered
            .iter()
            .try_for_each(|proof| proof.validate(self, checkpoint));
        drop(proofs);
        if let Err(error) = result {
            self.input_proofs.borrow_mut().failure = Some(error.clone());
            return Err(error);
        }
        self.budget.check()?;
        Ok(())
    }

    /// Frozen source prefix. Layout output bounds and cache generations are excluded.
    pub fn source_revision(&self) -> Result<u64, TextError> {
        self.input_revision(false)
    }

    pub(crate) fn source_prefix(&self) -> Result<super::PreparationSource, TextError> {
        self.budget.consume("preparation source query", 1)?;
        let mut cached = self.root_inputs.source.borrow_mut();
        if cached.is_none() {
            *cached = Some(self.build_source_inputs()?);
            return Ok(cached.as_ref().expect("initialized source").prefix.clone());
        }
        let inputs = cached.as_mut().expect("initialized source");
        let mut changed = None;
        // Only provider revisions are mutable in this borrow. Keep exact comparison and
        // reread every provider, even when an earlier revision has already changed.
        for &index in &inputs.providers {
            self.budget
                .consume("preparation source provider revision", 1)?;
            let source = &inputs.prefix.0[index];
            let provider = self
                .tree
                .node(source.id)
                .map_err(scene_error)?
                .preparation()
                .expect("immutable capability set")
                .revision();
            if source.provider != Some(provider) {
                if changed.is_none() {
                    for node in inputs.prefix.0.iter() {
                        self.budget.allocate(
                            "preparation source replacement",
                            1 + node.resources.len() as u64 + node.children.len() as u64,
                        )?;
                    }
                    changed = Some(inputs.prefix.0.to_vec());
                }
                changed.as_mut().expect("replacement prefix")[index].provider = Some(provider);
            }
        }
        self.budget.check()?;
        if let Some(prefix) = changed {
            inputs.prefix = super::PreparationSource(prefix.into());
        }
        Ok(inputs.prefix.clone())
    }

    fn build_source_inputs(&self) -> Result<SourceInputs, TextError> {
        let mut prefix = Vec::new();
        let mut providers = Vec::new();
        let mut pending = vec![self.root];
        while let Some(id) = pending.pop() {
            let node = self.tree.node(id).map_err(scene_error)?;
            self.budget.allocate(
                "preparation source prefix",
                1 + node.resource_inputs.len() as u64 + node.children.len() as u64,
            )?;
            let bounds = node.source_bounds;
            let insets = node.insets();
            if node.preparation().is_some() {
                self.budget.allocate("preparation source providers", 1)?;
                providers.push(prefix.len());
            }
            prefix.push(super::PreparationSourceNode {
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
        Ok(SourceInputs {
            prefix: super::PreparationSource(prefix.into()),
            providers,
        })
    }

    /// Input revision includes structure, component state, styles and measurement generation.
    pub fn revision(&self) -> Result<u64, TextError> {
        self.input_revision(true)
    }

    fn input_revision(&self, include_derived: bool) -> Result<u64, TextError> {
        self.budget.check()?;
        let index = usize::from(include_derived);
        let mut bases = self.revision_bases.borrow_mut();
        if bases[index].is_none() {
            bases[index] = Some(self.build_revision_base(include_derived)?);
        }
        let base = bases[index].as_ref().expect("initialized revision base");
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        base.value.hash(&mut hash);
        // The tree and resolved styles are immutably borrowed, but external providers can
        // change through interior state. Never memoize those revisions or their call results.
        for id in &base.providers {
            self.budget.consume("preparation provider revision", 1)?;
            id.hash(&mut hash);
            self.tree
                .node(*id)
                .map_err(scene_error)?
                .preparation()
                .expect("immutable capability set")
                .revision()
                .hash(&mut hash);
        }
        self.budget.check()?;
        Ok(hash.finish())
    }

    fn build_revision_base(&self, include_derived: bool) -> Result<RevisionBase, TextError> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        let mut providers = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            self.budget.consume("preparation revision", 1)?;
            id.hash(&mut hash);
            let node = self.tree.node(id).map_err(scene_error)?;
            node.component_revision.hash(&mut hash);
            node.resource_inputs.hash(&mut hash);
            node.preparation().is_some().hash(&mut hash);
            if node.preparation().is_some() {
                self.budget.allocate("preparation revision providers", 1)?;
                providers.push(id);
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
            style.layout_direction.hash(&mut hash);
            for value in style
                .foreground
                .to_rgba_array()
                .into_iter()
                .chain(style.background.to_rgba_array())
            {
                value.to_bits().hash(&mut hash);
            }
            style.alpha.get().to_bits().hash(&mut hash);
            std::mem::discriminant(&style.cursor).hash(&mut hash);
            style.tooltip.hash(&mut hash);
            for child in node.children.iter().rev() {
                self.budget.allocate("preparation revision", 1)?;
                stack.push(*child);
            }
        }
        self.budget.check()?;
        Ok(RevisionBase {
            value: hash.finish(),
            providers,
        })
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
    pub(crate) input_proofs: Vec<super::InputProof>,
}

impl PreparedScene {
    pub fn new(root: FigurePresentation) -> Self {
        Self {
            root,
            layouts: Vec::new(),
            descendants: Vec::new(),
            result_version: None,
            required_routes: Vec::new(),
            input_proofs: Vec::new(),
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
        scene.validate_input_proofs("prepared candidate")?;
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

    struct RevisionProvider(std::sync::Arc<std::sync::atomic::AtomicU64>);

    impl Figure for RevisionProvider {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::ZERO
        }
        fn name(&self) -> &'static str {
            "RevisionProvider"
        }
        fn register_capabilities(
            &self,
            out: &mut crate::figure::FigureCapabilityBuilder,
        ) -> Result<(), crate::figure::FigureCapabilityRegistrationError> {
            out.register(
                crate::figure::PREPARATION,
                crate::figure::PreparationCapability::of::<Self>(),
            )
        }
    }
    impl crate::figure::FigurePreparation for RevisionProvider {
        fn revision(&self) -> u64 {
            self.0.load(std::sync::atomic::Ordering::Relaxed)
        }
        fn prepare(
            &self,
            _: &mut crate::text::MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Err(TextError::InvalidPreparedScene("revision-only test".into()))
        }
    }

    #[test]
    fn shared_inputs_reuse_roots_without_widening_measurement_scope() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 20.0).unwrap()),
            )
            .unwrap();
        let sibling = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 30.0, 40.0).unwrap()),
            )
            .unwrap();
        let styles = [root, child, sibling]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let parent = PreparationScene::new(&tree, root, &styles).unwrap();
        let outer = MeasureConstraints::UNBOUNDED;
        let narrow = parent.for_measurement(child, outer, outer).unwrap();
        let first = narrow.revision().unwrap();
        narrow.parsed_font(child).unwrap();
        let before = parent.budget.used();
        let repeated = parent.for_measurement(child, outer, outer).unwrap();
        assert_eq!(repeated.revision().unwrap(), first);
        repeated.parsed_font(child).unwrap();
        assert_eq!(parent.budget.used().allocation, before.allocation);
        assert!(Rc::ptr_eq(&narrow.members, &repeated.members));
        assert!(Rc::ptr_eq(&parent.parsed_fonts, &repeated.parsed_fonts));
        assert!(narrow.check(root).is_err());
        assert!(narrow.check(sibling).is_err());
        assert!(narrow.for_measurement(sibling, outer, outer).is_err());
        assert_eq!(narrow.parent(child).unwrap(), None);
        assert_eq!(narrow.measurement_constraints(), Some((outer, outer)));
        // Independent owner scenes share inputs, never measurement results or observations.
        let owner = parent.inputs.scene(child).unwrap();
        assert!(Rc::ptr_eq(&owner.members, &narrow.members));
        assert!(!Rc::ptr_eq(&owner.measurements, &narrow.measurements));
        assert!(!Rc::ptr_eq(&owner.observations, &narrow.observations));
        assert_ne!(parent.revision().unwrap(), first);
    }

    #[test]
    fn shared_route_members_preserve_leaf_observations_and_refresh_after_reparent() {
        use crate::connection::DependencySubject;
        struct ExternalRoute(crate::PointList);
        impl crate::ConnectionFigureBehavior for ExternalRoute {
            fn route_points(&self) -> &crate::PointList {
                &self.0
            }
            fn commit_route_points(&mut self, points: crate::PointList) {
                self.0 = points;
            }
            fn connection_stroke_width(&self) -> f64 {
                1.0
            }
            fn connection_stroke_color(&self) -> crate::Color {
                crate::Color::BLACK
            }
        }
        impl crate::FigureContainer for ExternalRoute {}
        impl Figure for ExternalRoute {
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::ZERO
            }
            fn name(&self) -> &'static str {
                "ExternalRoute"
            }
            fn register_capabilities(
                &self,
                out: &mut crate::FigureCapabilityBuilder,
            ) -> Result<(), crate::FigureCapabilityRegistrationError> {
                out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
                out.register(crate::CONNECTION, crate::ConnectionCapability::of::<Self>())
            }
        }
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let route = tree
            .builder()
            .add_child(root, Box::new(ExternalRoute(crate::PointList::new())))
            .unwrap();
        let leaf = tree
            .builder()
            .add_child(
                route,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        let sibling = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        let styles = [root, route, leaf, sibling]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        {
            let scene = PreparationScene::new(&tree, root, &styles).unwrap();
            scene.figure(sibling).unwrap();
            assert!(scene.observations().is_empty());
            scene.figure(leaf).unwrap();
            let first = scene.observations();
            assert_eq!(first.len(), 1);
            assert_eq!(first[0].subject, DependencySubject::FigureGeometry(leaf));
            // The connection ancestor is outside the restricted root.
            let scoped = scene.input_scope(leaf, leaf, None).unwrap();
            assert!(scoped.figure(route).is_err());
            scoped.figure(leaf).unwrap();
            scoped.capability(leaf, crate::CONNECTION).unwrap();
            assert_eq!(scoped.observations(), first);
            assert_eq!(scene.observations(), first);
            let repeated = scene.input_scope(leaf, leaf, None).unwrap();
            assert!(Rc::ptr_eq(&scoped.root_inputs, &repeated.root_inputs));
            assert!(repeated.observations().is_empty());
            repeated
                .budget
                .consume("force exhaustion", u64::MAX)
                .unwrap_err();
            assert!(repeated.figure(leaf).is_err());
        }
        let mut updates = crate::UpdateManager::new();
        tree.reparent(&mut updates, leaf, root).unwrap();
        let fresh = PreparationScene::new(&tree, root, &styles).unwrap();
        fresh.figure(leaf).unwrap();
        fresh.figure(sibling).unwrap();
        assert!(fresh.observations().is_empty());
        fresh.figure(route).unwrap();
        assert_eq!(
            fresh.observations()[0].subject,
            DependencySubject::FigureGeometry(route)
        );
    }

    #[test]
    fn shared_source_prefix_rechecks_providers_and_preserves_old_values() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let revision = Arc::new(AtomicU64::new(1));
        let child = tree
            .builder()
            .add_child(root, Box::new(RevisionProvider(revision.clone())))
            .unwrap();
        let styles = [root, child]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let first = scene.source_prefix().unwrap();
        let first_revision = scene.revision().unwrap();
        let before = scene.budget.used();
        let validation = scene.inputs.scene(root).unwrap();
        let again = validation.source_prefix().unwrap();
        assert!(Arc::ptr_eq(&first.0, &again.0));
        assert_eq!(first_revision, validation.revision().unwrap());
        let after = scene.budget.used();
        assert_eq!(after.allocation, before.allocation);
        // One root lookup, one prefix lookup and two external provider revision reads.
        assert_eq!(after.work - before.work, 4);
        revision.store(2, Ordering::Relaxed);
        let changed = validation.source_prefix().unwrap();
        assert!(first != changed);
        assert_eq!(first.0[1].provider, Some(1));
        assert_eq!(changed.0[1].provider, Some(2));
        assert_ne!(first_revision, validation.revision().unwrap());
        assert!(scene.budget.consume("force exhaustion", u64::MAX).is_err());
        assert!(validation.source_prefix().is_err());
        assert!(scene.inputs.scene(root).is_err());
        // Retry gets a new scope and does not inherit a poisoned or stale input cache.
        let fresh = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(fresh.source_prefix().unwrap() == changed);
    }

    #[test]
    fn failed_input_build_never_installs_a_partial_root() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let styles = HashMap::new();
        let budget = super::super::PreparationBudgetHandle::new(
            super::super::PreparationBudget::new(100, 7),
        );
        let inputs = PreparationInputs::new(&tree, &styles, budget.clone()).unwrap();
        assert!(inputs.scene(root).is_err());
        assert!(inputs.roots.borrow().is_empty());
        assert!(budget.check().is_err());
        assert!(inputs.scene(root).is_err());
    }

    #[test]
    fn immutable_revision_base_reuses_tree_work_but_rechecks_external_providers_and_budget() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let provider = Arc::new(AtomicU64::new(1));
        tree.builder()
            .add_child(root, Box::new(RevisionProvider(provider.clone())))
            .unwrap();
        for _ in 0..128 {
            tree.builder()
                .add_child(
                    root,
                    Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
                )
                .unwrap();
        }
        let styles = std::iter::once(root)
            .chain(tree.child_order(root).unwrap())
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let first = scene.revision().unwrap();
        let after_first = scene.budget.used();
        assert_eq!(first, scene.revision().unwrap());
        assert_eq!(scene.budget.used().work - after_first.work, 1);
        provider.store(2, Ordering::Relaxed);
        assert_ne!(first, scene.revision().unwrap());
        let source = scene.source_revision().unwrap();
        provider.store(3, Ordering::Relaxed);
        assert_ne!(source, scene.source_revision().unwrap());
        assert!(scene.budget.consume("force exhaustion", u64::MAX).is_err());
        assert!(scene.revision().is_err());
        // A second scene owns a fresh base; no lifetime crosses a tree mutation boundary.
        let fresh = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(fresh.revision().is_ok());
    }

    #[test]
    fn parsed_fonts_share_only_equal_descriptors_and_do_not_bypass_failed_budget() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        let mut styles: HashMap<_, _> = [root, child]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        styles.get_mut(&root).unwrap().font =
            crate::text::FontDescriptor::parse("13px Inter").unwrap();
        styles.get_mut(&child).unwrap().font =
            crate::text::FontDescriptor::parse("bold 24px Inter").unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let font = scene.parsed_font(root).unwrap();
        let after = scene.budget.used();
        assert_eq!(font, scene.parsed_font(root).unwrap());
        assert_eq!(after.allocation, scene.budget.used().allocation);
        assert_ne!(font, scene.parsed_font(child).unwrap());
        assert_eq!(scene.parsed_fonts.borrow().len(), 2);
        assert!(scene.budget.consume("force exhaustion", u64::MAX).is_err());
        assert!(scene.parsed_font(root).is_err());
    }

    #[test]
    fn observations_preserve_first_generation_and_order_with_linear_work() {
        use crate::connection::{DependencyObservation, DependencySubject};
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let mut retained = Vec::new();
        for generation in 0..512 {
            let child = tree
                .builder()
                .add_child(
                    root,
                    Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
                )
                .unwrap();
            retained.push(DependencyObservation {
                subject: DependencySubject::FigureGeometry(child),
                generation,
            });
        }
        let styles = HashMap::new();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let before = scene.budget.used();
        scene.retain_observations(&retained);
        let mut repeated = retained.clone();
        repeated.reverse();
        for observation in &mut repeated {
            observation.generation += 1;
        }
        scene.retain_observations(&repeated);
        for observation in &repeated {
            scene.observe(observation.subject.clone());
        }
        assert_eq!(scene.observations(), retained);
        let after = scene.budget.used();
        // Three indexed lookups, two stored entries and one snapshot entry per subject.
        assert_eq!(after.work - before.work, 512 * 6);
        assert_eq!(after.allocation - before.allocation, 512 * 3);
    }

    #[test]
    fn observation_index_reserves_both_entries_and_keeps_budget_failure_sticky() {
        use crate::connection::DependencySubject;
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()))
            .unwrap();
        let styles = HashMap::new();
        let mut scene = PreparationScene::new(&tree, root, &styles).unwrap();
        scene.budget = super::super::PreparationBudgetHandle::new(
            super::super::PreparationBudget::new(100, 1),
        );
        scene.observe(DependencySubject::FigureGeometry(root));
        assert!(scene.budget.check().is_err());
        assert!(scene.observations.borrow().subjects.is_empty());
        assert!(scene.observations.borrow().ordered.is_empty());
        scene.observe(DependencySubject::FigureMeasurement(root));
        assert!(scene.observations.borrow().ordered.is_empty());
    }

    #[test]
    fn cached_layout_measurement_retains_transitive_observations() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 20.0).unwrap()),
            )
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
