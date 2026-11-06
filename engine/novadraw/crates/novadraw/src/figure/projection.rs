//! Checked, pure child-content projection. Scene access never exposes a Figure.
use std::{cell::RefCell, collections::HashMap, error::Error, fmt, rc::Rc};

use crate::{Dimension, FigureId, Insets, Point, tree::FigureTree, tree::TreeQueryError};

use super::{
    CapabilityKey, ChildTransform, Figure, PreparationBudgetError, PreparationBudgetHandle,
    PreparationInputError, PreparationInputSnapshot, ResolvedStyle,
    preparation::input::{
        InputFamily, InputReadPath, TypedInputProof, TypedInputRegistry, merge_proofs,
    },
};

pub use super::{
    PreparationInputKey as ProjectionInputKey, PreparationInputPayload as ProjectionInputPayload,
};

/// The only extra child transform source when this capability is admitted.
/// Implementations are pure and bounded. Shared mutable values must be read
/// through `ProjectionScene::input`, including values used by nested readers.
pub trait FigureProjection {
    fn project(
        &self,
        scene: &ProjectionScene<'_>,
    ) -> Result<ProjectionContribution, ProjectionError>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionContribution {
    pub child_transform: ChildTransform,
}

impl ProjectionContribution {
    pub const fn new(child_transform: ChildTransform) -> Self {
        Self { child_transform }
    }
}

fn projection_of<T: Figure + FigureProjection + 'static>(
    figure: &dyn Figure,
) -> Option<&dyn FigureProjection> {
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as _)
}

#[derive(Clone, Copy)]
pub struct ProjectionCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureProjection>,
}

impl ProjectionCapability {
    pub fn of<T: Figure + FigureProjection + 'static>() -> Self {
        Self {
            resolve: projection_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureProjection> {
        (self.resolve)(figure)
    }
}

pub const PROJECTION: CapabilityKey<ProjectionCapability> =
    CapabilityKey::new("novadraw.projection");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    Identity(TreeQueryError),
    Capability(super::CapabilityQueryError),
    OutsideRoot {
        root: FigureId,
        figure: FigureId,
    },
    Input(Box<PreparationInputError>),
    Budget(PreparationBudgetError),
    Retention(crate::text::TextContentError),
    InvalidTransform {
        figure: FigureId,
    },
    CoreChanged {
        figure: FigureId,
        fact: &'static str,
    },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "projection failed: {self:?}")
    }
}
impl Error for ProjectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Capability(error) => Some(error),
            Self::Input(error) => Some(error),
            Self::Budget(error) => Some(error),
            Self::Retention(error) => Some(error),
            _ => None,
        }
    }
}
impl From<TreeQueryError> for ProjectionError {
    fn from(error: TreeQueryError) -> Self {
        Self::Identity(error)
    }
}
impl From<super::CapabilityQueryError> for ProjectionError {
    fn from(error: super::CapabilityQueryError) -> Self {
        Self::Capability(error)
    }
}
impl From<PreparationInputError> for ProjectionError {
    fn from(error: PreparationInputError) -> Self {
        Self::Input(Box::new(error))
    }
}
impl From<PreparationBudgetError> for ProjectionError {
    fn from(error: PreparationBudgetError) -> Self {
        Self::Budget(error)
    }
}
impl From<crate::text::TextContentError> for ProjectionError {
    fn from(error: crate::text::TextContentError) -> Self {
        Self::Retention(error)
    }
}

pub trait ProjectionInputReader<P: ProjectionInputPayload>: Send + Sync {
    fn read(&self, source: FigureId, scene: &ProjectionScene<'_>) -> Result<P, ProjectionError>;
}
impl<P, F> ProjectionInputReader<P> for F
where
    P: ProjectionInputPayload,
    F: Fn(FigureId, &ProjectionScene<'_>) -> Result<P, ProjectionError> + Send + Sync,
{
    fn read(&self, source: FigureId, scene: &ProjectionScene<'_>) -> Result<P, ProjectionError> {
        self(source, scene)
    }
}

/// Clones share registration tokens and retention accounting. Old receipts hold
/// readers weakly; dropping the final registry retires their read authority.
#[derive(Clone)]
pub struct ProjectionInputRegistry {
    registry: TypedInputRegistry<ProjectionInputFamily>,
    store: crate::text::TextContentStore,
}
impl ProjectionInputRegistry {
    /// Explicit logical byte limit for owned snapshots and proof records.
    pub fn new(retained_bytes: usize) -> Self {
        Self {
            registry: TypedInputRegistry::default(),
            store: crate::text::TextContentStore::new(retained_bytes),
        }
    }
    pub fn register<P: ProjectionInputPayload>(
        &mut self,
        key: ProjectionInputKey<P>,
        reader: impl ProjectionInputReader<P> + 'static,
    ) -> Result<(), PreparationInputError> {
        self.registry.register(
            key,
            move |source, scene| reader.read(source, scene),
            Some(self.store.clone()),
        )
    }
    pub fn retention_usage(&self) -> Result<crate::text::TextContentUsage, ProjectionError> {
        Ok(self.store.usage()?)
    }
}

pub(crate) struct ProjectionInputFamily;
impl InputFamily for ProjectionInputFamily {
    type Scene<'a> = ProjectionScene<'a>;
    type Error = ProjectionError;
    fn root(scene: &ProjectionScene<'_>) -> FigureId {
        scene.root
    }
    fn constraints(
        _: &ProjectionScene<'_>,
    ) -> Option<(super::MeasureConstraints, super::MeasureConstraints)> {
        None
    }
    fn budget(scene: &ProjectionScene<'_>) -> PreparationBudgetHandle {
        scene.budget()
    }
    fn read_path(scene: &ProjectionScene<'_>) -> InputReadPath {
        scene.path.clone()
    }
    fn check_reader(scene: &ProjectionScene<'_>) -> Result<(), ProjectionError> {
        scene.check()
    }
    fn scope<'a>(
        scene: &ProjectionScene<'a>,
        root: FigureId,
        source: FigureId,
        _: Option<(super::MeasureConstraints, super::MeasureConstraints)>,
    ) -> Result<ProjectionScene<'a>, ProjectionError>
    where
        Self: 'a,
    {
        scene.member(root)?;
        let scoped =
            ProjectionScene::with_path(scene.tree, root, scene.budget(), scene.path.clone())?;
        scoped.member(source)?;
        Ok(scoped)
    }
    fn complete_scope(
        scene: &ProjectionScene<'_>,
        scoped: &ProjectionScene<'_>,
    ) -> Result<(), ProjectionError> {
        scoped.check()?;
        let proofs = scoped.receipts()?;
        scene.retain(&proofs)?;
        scene.retain_facts(&scoped.facts()?)
    }
}

#[derive(Default)]
struct ProofLog {
    ordered: Vec<TypedInputProof<ProjectionInputFamily>>,
    facts: Vec<Rc<CoreRead>>,
    subjects: HashMap<(FigureId, std::mem::Discriminant<CoreFact>), usize>,
    failure: Option<ProjectionError>,
}

/// Internal Core facts are not extension behavior identities.
#[derive(Clone, PartialEq)]
enum CoreFact {
    Parent(Option<FigureId>),
    Children(Vec<FigureId>),
    Origin(Point),
    Size(Dimension),
    Insets(Insets),
    Style(crate::FigureStyle),
    Component(u64),
}
impl CoreFact {
    fn name(&self) -> &'static str {
        match self {
            Self::Parent(_) => "parent",
            Self::Children(_) => "children",
            Self::Origin(_) => "origin",
            Self::Size(_) => "size",
            Self::Insets(_) => "insets",
            Self::Style(_) => "style",
            Self::Component(_) => "component",
        }
    }
}
struct CoreRead {
    figure: FigureId,
    value: CoreFact,
}

/// Owned operation evidence survives committed Core effects between callbacks.
/// Core facts are checked within each immutable consumption scope. Shared inputs
/// keep their first value for the entire operation, including final verification.
type OperationConsumers = Vec<(FigureId, Vec<TypedInputProof<ProjectionInputFamily>>)>;

#[derive(Clone)]
pub(crate) struct ProjectionOperation {
    budget: PreparationBudgetHandle,
    first_inputs: Rc<RefCell<Vec<TypedInputProof<ProjectionInputFamily>>>>,
    failure: Rc<RefCell<Option<ProjectionError>>>,
    coordinate_failure: Rc<RefCell<Option<crate::tree::CoordinateQueryError>>>,
    consumers: Rc<RefCell<OperationConsumers>>,
    pub(crate) contributions:
        Rc<RefCell<std::collections::BTreeMap<FigureId, Rc<ProjectionReceipt>>>>,
    pub(crate) prefixes:
        Rc<RefCell<std::collections::BTreeMap<FigureId, Rc<crate::graph::WorldPrefix>>>>,
}
impl ProjectionOperation {
    pub(crate) fn budget(&self) -> PreparationBudgetHandle {
        self.budget.clone()
    }
    pub(crate) fn new(budget: PreparationBudgetHandle) -> Self {
        // Allocation rejection is sticky in the handle and is observed before
        // the first scene can call an extension.
        let _ = budget.allocate(
            "projection operation storage",
            (std::mem::size_of::<Self>()
                + std::mem::size_of::<Vec<TypedInputProof<ProjectionInputFamily>>>()
                + std::mem::size_of::<Option<ProjectionError>>()
                + std::mem::size_of::<Option<crate::tree::CoordinateQueryError>>()
                + std::mem::size_of::<Vec<(FigureId, Vec<TypedInputProof<ProjectionInputFamily>>)>>(
                )
                + std::mem::size_of::<std::collections::BTreeMap<FigureId, Rc<ProjectionReceipt>>>(
                )
                + std::mem::size_of::<
                    std::collections::BTreeMap<FigureId, Rc<crate::graph::WorldPrefix>>,
                >()) as u64,
        );
        Self {
            budget,
            first_inputs: Rc::new(RefCell::new(Vec::new())),
            failure: Rc::new(RefCell::new(None)),
            coordinate_failure: Rc::new(RefCell::new(None)),
            consumers: Rc::new(RefCell::new(Vec::new())),
            contributions: Rc::new(RefCell::new(std::collections::BTreeMap::new())),
            prefixes: Rc::new(RefCell::new(std::collections::BTreeMap::new())),
        }
    }
    pub(crate) fn scene<'a>(
        &self,
        tree: &'a FigureTree,
    ) -> Result<ProjectionScene<'a>, ProjectionError> {
        let mut scene = ProjectionScene::new(tree, tree.synthetic_root(), self.budget.clone())?;
        scene.first_inputs = self.first_inputs.clone();
        scene.failure = self.failure.clone();
        scene.check()?;
        Ok(scene)
    }
    pub(crate) fn retain(&self, scene: &ProjectionScene<'_>) -> Result<(), ProjectionError> {
        scene.check()?;
        let incoming = scene.receipts()?;
        let mut consumers = self.consumers.borrow_mut();
        if let Some((_, proofs)) = consumers.iter_mut().find(|(root, _)| *root == scene.root) {
            scene.sticky(merge_proofs(proofs, &incoming, &self.budget))?;
        } else {
            self.budget.allocate(
                "projection operation consumer",
                std::mem::size_of::<(FigureId, Vec<TypedInputProof<ProjectionInputFamily>>)>()
                    as u64,
            )?;
            consumers.push((scene.root, incoming));
        }
        Ok(())
    }
    pub(crate) fn validate(
        &self,
        tree: &FigureTree,
        checkpoint: &'static str,
    ) -> Result<(), ProjectionError> {
        let scene = self.scene(tree)?;
        let mut index = 0;
        loop {
            let next = {
                let consumers = self.consumers.borrow();
                if let Some((root, proofs)) = consumers.get(index) {
                    self.budget.allocate(
                        "projection operation verification proofs",
                        (proofs.len()
                            * std::mem::size_of::<TypedInputProof<ProjectionInputFamily>>())
                            as u64,
                    )?;
                    Some((*root, proofs.clone()))
                } else {
                    None
                }
            };
            let Some((root, proofs)) = next else { break };
            let scope = scene.owner_scope(root)?;
            scope.retain(&proofs)?;
            scope.validate(checkpoint)?;
            self.retain(&scope)?;
            index += 1;
        }
        scene.check()
    }
    pub(crate) fn remember<T>(&self, result: &Result<T, crate::tree::CoordinateQueryError>) {
        if let Err(error) = result {
            self.coordinate_failure
                .borrow_mut()
                .get_or_insert_with(|| error.clone());
        }
    }
    pub(crate) fn coordinate_error(&self) -> Option<crate::tree::CoordinateQueryError> {
        self.coordinate_failure.borrow().clone()
    }
    pub(crate) fn checked_validate(
        &self,
        tree: &FigureTree,
        checkpoint: &'static str,
    ) -> Result<(), crate::tree::CoordinateQueryError> {
        if let Some(error) = self.coordinate_failure.borrow().as_ref() {
            return Err(error.clone());
        }
        let result = self.validate(tree, checkpoint).map_err(Into::into);
        self.remember(&result);
        result
    }
    pub(crate) fn publish(
        &self,
        tree: &FigureTree,
        checkpoint: &'static str,
    ) -> Result<(), crate::tree::CoordinateQueryError> {
        self.checked_validate(tree, checkpoint)?;
        tree.publish_projection_candidates(
            std::mem::take(&mut *self.contributions.borrow_mut()),
            std::mem::take(&mut *self.prefixes.borrow_mut()),
        );
        Ok(())
    }
}

/// Immutable Core facts restricted to one owner's subtree. There are no world,
/// hit, route, measurement, preparation, tree, capability or mutation accessors.
/// Ordinary tree reads are safe for this borrow; cache reuse must separately
/// retain and verify the facts consumed by a provider.
pub struct ProjectionScene<'a> {
    tree: &'a FigureTree,
    root: FigureId,
    budget: PreparationBudgetHandle,
    path: InputReadPath,
    proofs: Rc<RefCell<ProofLog>>,
    first_inputs: Rc<RefCell<Vec<TypedInputProof<ProjectionInputFamily>>>>,
    failure: Rc<RefCell<Option<ProjectionError>>>,
}
impl<'a> ProjectionScene<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        root: FigureId,
        budget: PreparationBudgetHandle,
    ) -> Result<Self, ProjectionError> {
        Self::with_path(tree, root, budget, InputReadPath::default())
    }
    fn with_path(
        tree: &'a FigureTree,
        root: FigureId,
        budget: PreparationBudgetHandle,
        path: InputReadPath,
    ) -> Result<Self, ProjectionError> {
        tree.parent_id(root)?;
        budget.allocate("projection scene", std::mem::size_of::<ProofLog>() as u64)?;
        Ok(Self {
            tree,
            root,
            budget,
            path,
            proofs: Rc::new(RefCell::new(ProofLog::default())),
            first_inputs: Rc::new(RefCell::new(Vec::new())),
            failure: Rc::new(RefCell::new(None)),
        })
    }
    pub fn root(&self) -> FigureId {
        self.root
    }
    pub fn budget(&self) -> PreparationBudgetHandle {
        self.budget.clone()
    }
    /// Core authorizes each owner separately. Providers cannot obtain this
    /// constructor or escape their own subtree; all owners share the operation's
    /// first reads and final verification closure.
    pub(crate) fn owner_scope(&self, root: FigureId) -> Result<Self, ProjectionError> {
        self.check()?;
        self.tree.parent_id(root)?;
        self.budget.allocate(
            "projection owner scope",
            std::mem::size_of::<ProofLog>() as u64,
        )?;
        Ok(Self {
            tree: self.tree,
            root,
            budget: self.budget(),
            path: self.path.clone(),
            proofs: Rc::new(RefCell::new(ProofLog::default())),
            first_inputs: self.first_inputs.clone(),
            failure: self.failure.clone(),
        })
    }
    pub(crate) fn check(&self) -> Result<(), ProjectionError> {
        if let Some(error) = &*self.failure.borrow() {
            return Err(error.clone());
        }
        if let Some(error) = &self.proofs.borrow().failure {
            return Err(error.clone());
        }
        self.budget.check()?;
        Ok(())
    }
    fn sticky<T>(&self, result: Result<T, ProjectionError>) -> Result<T, ProjectionError> {
        if let Err(error) = &result {
            let mut failure = self.failure.borrow_mut();
            if failure.is_none() {
                *failure = Some(error.clone());
            }
            let mut log = self.proofs.borrow_mut();
            if log.failure.is_none() {
                log.failure = Some(error.clone());
            }
        }
        result
    }
    fn member(&self, figure: FigureId) -> Result<(), ProjectionError> {
        self.sticky((|| {
            self.check()?;
            // Validate the requested identity before interpreting absence/range.
            self.read_parent(figure)?;
            let mut current = Some(figure);
            while let Some(id) = current {
                self.budget.consume("projection membership", 1)?;
                if id == self.root {
                    return Ok(());
                }
                current = self.read_parent(id)?;
            }
            Err(ProjectionError::OutsideRoot {
                root: self.root,
                figure,
            })
        })())
    }
    pub fn parent_id(&self, figure: FigureId) -> Result<Option<FigureId>, ProjectionError> {
        self.member(figure)?;
        self.read_parent(figure)
    }
    fn read_parent(&self, figure: FigureId) -> Result<Option<FigureId>, ProjectionError> {
        let parent = self.tree.parent_id(figure)?;
        self.record(figure, CoreFact::Parent(parent))?;
        Ok(parent)
    }
    pub fn children(&self, figure: FigureId) -> Result<Vec<FigureId>, ProjectionError> {
        self.member(figure)?;
        self.sticky((|| {
            let count = self.tree.node(figure)?.children.len();
            self.budget.allocate(
                "projection children",
                (count * std::mem::size_of::<FigureId>()) as u64,
            )?;
            let children = self.tree.child_order(figure)?;
            self.record(figure, CoreFact::Children(children))?;
            // Return a distinct owned vector, charged before cloning.
            self.budget.allocate(
                "projection children result",
                (count * std::mem::size_of::<FigureId>()) as u64,
            )?;
            Ok(self.tree.child_order(figure)?)
        })())
    }
    pub fn origin(&self, figure: FigureId) -> Result<Point, ProjectionError> {
        self.member(figure)?;
        let bounds = self.tree.figure_bounds(figure)?;
        let origin = Point::new(bounds.x, bounds.y);
        self.record(figure, CoreFact::Origin(origin))?;
        Ok(origin)
    }
    pub fn size(&self, figure: FigureId) -> Result<Dimension, ProjectionError> {
        self.member(figure)?;
        let bounds = self.tree.figure_bounds(figure)?;
        let size = Dimension::new(bounds.width, bounds.height);
        self.record(figure, CoreFact::Size(size))?;
        Ok(size)
    }
    pub fn insets(&self, figure: FigureId) -> Result<Insets, ProjectionError> {
        self.member(figure)?;
        let insets = self.tree.insets(figure)?;
        self.record(figure, CoreFact::Insets(insets))?;
        Ok(insets)
    }
    pub fn resolved_style(&self, figure: FigureId) -> Result<ResolvedStyle, ProjectionError> {
        self.member(figure)?;
        self.sticky((|| {
            let mut current = Some(figure);
            let mut chain = Vec::new();
            while let Some(id) = current {
                self.budget.allocate(
                    "projection inherited style",
                    std::mem::size_of::<FigureId>() as u64,
                )?;
                let style = self.tree.figure_style(id)?;
                self.charge_style(style)?;
                self.record(id, CoreFact::Style(style.clone()))?;
                chain.push(id);
                current = self.read_parent(id)?;
            }
            // Every clone into the result is charged, including overridden values.
            let mut resolved = ResolvedStyle::default();
            self.budget.allocate(
                "projection default style",
                resolved.font.family_capacity() as u64,
            )?;
            for id in chain.into_iter().rev() {
                let style = self.tree.figure_style(id)?;
                self.charge_style(style)?;
                resolved.apply_override(style);
            }
            Ok(resolved)
        })())
    }
    fn charge_style(&self, style: &crate::FigureStyle) -> Result<(), ProjectionError> {
        let font = style.font.as_ref().map_or(0, |font| font.family_capacity());
        let tooltip = style
            .tooltip
            .as_ref()
            .and_then(Option::as_ref)
            .map_or(0, String::capacity);
        self.budget
            .allocate("projection style backing", font as u64)?;
        self.budget
            .allocate("projection style backing", tooltip as u64)?;
        Ok(())
    }
    fn record(&self, figure: FigureId, value: CoreFact) -> Result<(), ProjectionError> {
        self.sticky((|| {
            self.check()?;
            let mut log = self.proofs.borrow_mut();
            self.budget.consume("projection Core merge", 1)?;
            let subject = (figure, std::mem::discriminant(&value));
            if let Some(&index) = log.subjects.get(&subject) {
                let first = &log.facts[index];
                if first.value != value {
                    return Err(ProjectionError::CoreChanged {
                        figure,
                        fact: value.name(),
                    });
                }
                return Ok(());
            }
            self.budget.allocate(
                "projection Core record",
                (std::mem::size_of::<CoreRead>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<Rc<CoreRead>>()
                    + std::mem::size_of::<((FigureId, std::mem::Discriminant<CoreFact>), usize)>())
                    as u64,
            )?;
            let index = log.facts.len();
            log.subjects.insert(subject, index);
            log.facts.push(Rc::new(CoreRead { figure, value }));
            Ok(())
        })())
    }
    fn facts(&self) -> Result<Vec<Rc<CoreRead>>, ProjectionError> {
        self.check()?;
        let log = self.proofs.borrow();
        self.budget.allocate(
            "projection Core receipts",
            (log.facts.len() * std::mem::size_of::<Rc<CoreRead>>()) as u64,
        )?;
        Ok(log.facts.clone())
    }
    fn retain_facts(&self, incoming: &[Rc<CoreRead>]) -> Result<(), ProjectionError> {
        self.sticky((|| {
            self.check()?;
            let mut log = self.proofs.borrow_mut();
            // Validate the whole batch before publishing any new records.
            for fact in incoming {
                self.budget.consume("projection Core receipts merge", 1)?;
                let subject = (fact.figure, std::mem::discriminant(&fact.value));
                if let Some(&index) = log.subjects.get(&subject)
                    && log.facts[index].value != fact.value
                {
                    return Err(ProjectionError::CoreChanged {
                        figure: fact.figure,
                        fact: fact.value.name(),
                    });
                }
            }
            self.budget.allocate(
                "projection Core receipts merge",
                (incoming.len() * (std::mem::size_of::<Rc<CoreRead>>()
                    + std::mem::size_of::<((FigureId, std::mem::Discriminant<CoreFact>), usize)>()))
                    as u64,
            )?;
            for fact in incoming {
                let subject = (fact.figure, std::mem::discriminant(&fact.value));
                if !log.subjects.contains_key(&subject) {
                    let index = log.facts.len();
                    log.subjects.insert(subject, index);
                    log.facts.push(fact.clone());
                }
            }
            Ok(())
        })())
    }
    pub fn input<P: ProjectionInputPayload>(
        &self,
        registry: &ProjectionInputRegistry,
        key: ProjectionInputKey<P>,
        source: FigureId,
    ) -> Result<PreparationInputSnapshot, ProjectionError> {
        self.sticky((|| {
            self.member(source)?;
            let proof = registry.registry.capture(key, source, self)?;
            let value = proof.first.clone();
            self.retain(&[proof])?;
            Ok(value)
        })())
    }
    fn retain(
        &self,
        incoming: &[TypedInputProof<ProjectionInputFamily>],
    ) -> Result<(), ProjectionError> {
        self.check()?;
        let first = merge_proofs(&mut self.first_inputs.borrow_mut(), incoming, &self.budget);
        self.sticky(first)?;
        let result = merge_proofs(
            &mut self.proofs.borrow_mut().ordered,
            incoming,
            &self.budget,
        );
        self.sticky(result)
    }
    pub(crate) fn receipts(
        &self,
    ) -> Result<Vec<TypedInputProof<ProjectionInputFamily>>, ProjectionError> {
        self.check()?;
        let log = self.proofs.borrow();
        self.budget.allocate(
            "projection receipts",
            (log.ordered.len() * std::mem::size_of::<TypedInputProof<ProjectionInputFamily>>())
                as u64,
        )?;
        Ok(log.ordered.clone())
    }
    pub(crate) fn validate(&self, checkpoint: &'static str) -> Result<(), ProjectionError> {
        self.sticky((|| {
            self.check()?;
            let mut index = 0;
            loop {
                let proof = self.proofs.borrow().ordered.get(index).cloned();
                let Some(proof) = proof else { break };
                proof.validate(self, checkpoint)?;
                index += 1;
            }
            // Recheck Core facts after all reader callbacks, including facts
            // discovered by independent nested verification scopes.
            for fact in self.facts()? {
                let current = self.current_fact(&fact)?;
                self.record(fact.figure, current)?;
            }
            self.check()
        })())
    }
    pub(crate) fn evaluate(
        &self,
        provider: &dyn FigureProjection,
    ) -> Result<ProjectionContribution, ProjectionError> {
        self.check()?;
        self.record(
            self.root,
            CoreFact::Component(self.tree.component_revision(self.root)?),
        )?;
        let contribution = self.sticky(provider.project(self))?;
        self.validate("projection return")?;
        if !contribution
            .child_transform
            .affine()
            .coeffs()
            .into_iter()
            .all(f64::is_finite)
        {
            return self.sticky(Err(ProjectionError::InvalidTransform { figure: self.root }));
        }
        Ok(contribution)
    }
    fn current_fact(&self, fact: &CoreRead) -> Result<CoreFact, ProjectionError> {
        self.budget.consume("projection Core verification", 1)?;
        let figure = fact.figure;
        Ok(match &fact.value {
            CoreFact::Parent(_) => CoreFact::Parent(self.tree.parent_id(figure)?),
            CoreFact::Children(_) => {
                let count = self.tree.node(figure)?.children.len();
                self.budget.allocate(
                    "projection children verification",
                    (count * std::mem::size_of::<FigureId>()) as u64,
                )?;
                CoreFact::Children(self.tree.child_order(figure)?)
            }
            CoreFact::Origin(_) => {
                let bounds = self.tree.figure_bounds(figure)?;
                CoreFact::Origin(Point::new(bounds.x, bounds.y))
            }
            CoreFact::Size(_) => {
                let bounds = self.tree.figure_bounds(figure)?;
                CoreFact::Size(Dimension::new(bounds.width, bounds.height))
            }
            CoreFact::Insets(_) => CoreFact::Insets(self.tree.insets(figure)?),
            CoreFact::Style(_) => {
                let style = self.tree.figure_style(figure)?;
                self.charge_style(style)?;
                CoreFact::Style(style.clone())
            }
            CoreFact::Component(_) => CoreFact::Component(self.tree.component_revision(figure)?),
        })
    }
}

/// An output and exactly its consumed evidence. Qualification reads historical
/// subjects into the new first-value ledger, but a miss never transfers obsolete
/// consumers into the final verification closure.
pub(crate) struct ProjectionReceipt {
    pub(crate) contribution: ProjectionContribution,
    root: FigureId,
    inputs: Vec<TypedInputProof<ProjectionInputFamily>>,
    facts: Vec<Rc<CoreRead>>,
    _retention: crate::text::TextContentLease,
}

impl ProjectionScene<'_> {
    pub(crate) fn receipt(
        &self,
        contribution: ProjectionContribution,
        store: &crate::text::TextContentStore,
    ) -> Result<ProjectionReceipt, ProjectionError> {
        self.sticky((|| {
            let inputs = self.receipts()?;
            let facts = self.facts()?;
            let fixed = std::mem::size_of::<ProjectionReceipt>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<(FigureId, Rc<ProjectionReceipt>)>()
                + 4 * std::mem::size_of::<usize>();
            self.budget
                .allocate("projection contribution receipt", fixed as u64)?;
            let mut bytes = fixed
                + inputs.len() * std::mem::size_of::<TypedInputProof<ProjectionInputFamily>>()
                + facts.len() * std::mem::size_of::<Rc<CoreRead>>();
            for fact in &facts {
                let backing = match &fact.value {
                    CoreFact::Children(children) => {
                        children.capacity() * std::mem::size_of::<FigureId>()
                    }
                    CoreFact::Style(style) => {
                        style.font.as_ref().map_or(0, |font| font.family_capacity())
                            + style
                                .tooltip
                                .as_ref()
                                .and_then(Option::as_ref)
                                .map_or(0, String::capacity)
                    }
                    _ => 0,
                };
                bytes = bytes
                    .checked_add(std::mem::size_of::<CoreRead>() + 2 * std::mem::size_of::<usize>())
                    .and_then(|bytes| bytes.checked_add(backing))
                    .ok_or(crate::text::TextContentError::AccountingOverflow)?;
            }
            Ok(ProjectionReceipt {
                contribution,
                root: self.root,
                inputs,
                facts,
                _retention: store.reserve(bytes)?,
            })
        })())
    }
}

impl ProjectionReceipt {
    pub(crate) fn has_input_proofs(&self) -> bool {
        !self.inputs.is_empty()
    }

    pub(crate) fn matches_consumption(
        &self,
        scene: &ProjectionScene<'_>,
    ) -> Result<bool, ProjectionError> {
        scene.check()?;
        let log = scene.proofs.borrow();
        scene.budget.consume(
            "projection consumption comparison",
            (log.ordered.len() + log.facts.len()) as u64,
        )?;
        Ok(log.ordered.len() == self.inputs.len()
            && log.facts.len() == self.facts.len()
            && log
                .ordered
                .iter()
                .zip(&self.inputs)
                .all(|(input, old)| input.same_subject(old) && input.first == old.first)
            && self.facts.iter().all(|old| {
                let subject = (old.figure, std::mem::discriminant(&old.value));
                log.subjects
                    .get(&subject)
                    .is_some_and(|&index| log.facts[index].value == old.value)
            }))
    }

    pub(crate) fn qualify<'a>(
        &self,
        operation: &ProjectionScene<'a>,
    ) -> Result<Option<ProjectionScene<'a>>, ProjectionError> {
        let scene = operation.owner_scope(self.root)?;
        scene
            .sticky((|| {
                let mut matches = true;
                // Check every reader even if a Core fact already differs: a retired
                // reader is an error, never an ordinary cache miss.
                for input in &self.inputs {
                    let current = input.refreshed(&scene, "projection cache qualification")?;
                    matches &= current.first == input.first;
                    scene.retain(&[current])?;
                }
                for fact in &self.facts {
                    let current = scene.current_fact(fact)?;
                    matches &= current == fact.value;
                    scene.record(fact.figure, current)?;
                }
                scene.check()?;
                Ok(matches)
            })())
            .map(|matches| matches.then_some(scene))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{
        CONTAINER, ContainerCapability, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureContainer, PreparationBudget, PreparationInputErrorKind, RectangleFigure,
        capability::FigureCapabilitySet,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    #[derive(Eq, PartialEq)]
    struct Outer;
    impl ProjectionInputPayload for Outer {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    #[derive(Eq, PartialEq)]
    struct Inner(u64);
    impl ProjectionInputPayload for Inner {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    const OUTER: ProjectionInputKey<Outer> = ProjectionInputKey::new("projection.outer");
    const INNER: ProjectionInputKey<Inner> = ProjectionInputKey::new("projection.inner");
    const RETAINED_BYTES: usize = 100_000;

    fn scene(tree: &FigureTree) -> ProjectionScene<'_> {
        ProjectionScene::new(tree, tree.root_id(), PreparationBudgetHandle::default()).unwrap()
    }
    fn input_kind(error: &ProjectionError) -> &PreparationInputErrorKind {
        let ProjectionError::Input(error) = error else {
            panic!("{error:?}")
        };
        &error.kind
    }

    struct Container;
    impl Figure for Container {
        fn initial_bounds(&self) -> crate::Rectangle {
            crate::Rectangle::new(0.0, 0.0, 1.0, 1.0)
        }
        fn name(&self) -> &'static str {
            "ProjectionTestContainer"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(CONTAINER, ContainerCapability::of::<Self>())?;
            out.register(PROJECTION, ProjectionCapability::of::<Self>())
        }
    }
    impl FigureContainer for Container {}
    impl FigureProjection for Container {
        fn project(
            &self,
            _: &ProjectionScene<'_>,
        ) -> Result<ProjectionContribution, ProjectionError> {
            Ok(ProjectionContribution::new(ChildTransform::IDENTITY))
        }
    }

    struct InvalidContainer;
    impl Figure for InvalidContainer {
        fn initial_bounds(&self) -> crate::Rectangle {
            crate::Rectangle::new(0.0, 0.0, 1.0, 1.0)
        }
        fn name(&self) -> &'static str {
            "InvalidProjectionTestContainer"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(PROJECTION, ProjectionCapability::of::<Self>())
        }
    }
    impl FigureProjection for InvalidContainer {
        fn project(
            &self,
            _: &ProjectionScene<'_>,
        ) -> Result<ProjectionContribution, ProjectionError> {
            unreachable!("rejected at admission")
        }
    }
    #[test]
    fn projection_requires_matching_container_and_provider() {
        assert!(FigureCapabilitySet::build(&Container).is_ok());
        assert!(matches!(
            FigureCapabilitySet::build(&InvalidContainer),
            Err(FigureCapabilityRegistrationError::InvalidCapability { .. })
        ));
        let tree = FigureTree::new();
        assert_eq!(
            scene(&tree).evaluate(&Container).unwrap().child_transform,
            ChildTransform::IDENTITY
        );
    }

    #[test]
    fn nested_input_keeps_first_value_even_if_outer_value_is_unchanged() {
        let tree = FigureTree::new();
        let scene = scene(&tree);
        let version = Arc::new(AtomicU64::new(1));
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        let value = version.clone();
        registry
            .register(INNER, move |_, _: &ProjectionScene<'_>| {
                Ok(Inner(value.load(Ordering::SeqCst)))
            })
            .unwrap();
        let nested = registry.clone();
        registry
            .register(OUTER, move |source, scene: &ProjectionScene<'_>| {
                scene.input(&nested, INNER, source)?;
                Ok(Outer)
            })
            .unwrap();
        scene.input(&registry, OUTER, tree.root_id()).unwrap();
        version.store(2, Ordering::SeqCst);
        let error = scene.validate("final").unwrap_err();
        assert!(matches!(
            input_kind(&error),
            PreparationInputErrorKind::Changed { .. }
        ));
        assert_eq!(scene.check(), Err(error));
        // A fresh operation may read the new state; the failed scope cannot.
        let fresh =
            super::ProjectionScene::new(&tree, tree.root_id(), PreparationBudgetHandle::default())
                .unwrap();
        fresh.input(&registry, OUTER, tree.root_id()).unwrap();
        fresh.validate("retry").unwrap();
    }

    #[test]
    fn nested_dependency_discovered_during_verification_joins_the_closure() {
        let tree = FigureTree::new();
        let scene = scene(&tree);
        let calls = Arc::new(AtomicU64::new(0));
        let value = calls.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(INNER, move |_, _: &ProjectionScene<'_>| {
                Ok(Inner(value.fetch_add(1, Ordering::SeqCst)))
            })
            .unwrap();
        let read_nested = Arc::new(AtomicU64::new(0));
        let switch = read_nested.clone();
        let nested = registry.clone();
        registry
            .register(OUTER, move |source, scene: &ProjectionScene<'_>| {
                if switch.load(Ordering::SeqCst) != 0 {
                    scene.input(&nested, INNER, source)?;
                }
                Ok(Outer)
            })
            .unwrap();
        scene.input(&registry, OUTER, tree.root_id()).unwrap();
        read_nested.store(1, Ordering::SeqCst);
        let error = scene.validate("final").unwrap_err();
        assert!(matches!(
            input_kind(&error),
            PreparationInputErrorKind::Changed { .. }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn ignored_scope_and_budget_errors_reject_provider_return() {
        struct Ignoring(FigureId);
        impl FigureProjection for Ignoring {
            fn project(
                &self,
                scene: &ProjectionScene<'_>,
            ) -> Result<ProjectionContribution, ProjectionError> {
                let _ = scene.size(self.0);
                Ok(ProjectionContribution::new(ChildTransform::IDENTITY))
            }
        }
        let mut tree = FigureTree::new();
        let child = tree.add_child_to(
            tree.root_id(),
            Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
        );
        let restricted =
            ProjectionScene::new(&tree, child, PreparationBudgetHandle::default()).unwrap();
        assert!(matches!(
            restricted.evaluate(&Ignoring(tree.root_id())),
            Err(ProjectionError::OutsideRoot { .. })
        ));
        let budget = PreparationBudgetHandle::new(PreparationBudget::new(
            10_000,
            std::mem::size_of::<ProofLog>() as u64,
        ));
        let bounded = ProjectionScene::new(&tree, tree.root_id(), budget).unwrap();
        assert!(bounded.children(tree.root_id()).is_err());
        assert!(matches!(
            bounded.evaluate(&Container),
            Err(ProjectionError::Budget(_))
        ));
    }

    #[test]
    fn retired_registry_fails_without_releasing_live_receipt_leases() {
        let tree = FigureTree::new();
        let scene = scene(&tree);
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(INNER, |_, _: &ProjectionScene<'_>| Ok(Inner(1)))
            .unwrap();
        let store = registry.store.clone();
        let snapshot = scene.input(&registry, INNER, tree.root_id()).unwrap();
        let receipts = scene.receipts().unwrap();
        let charged = store.usage().unwrap().retained_units;
        assert!(charged > 0);
        drop(registry);
        assert!(matches!(
            input_kind(&scene.validate("retired").unwrap_err()),
            PreparationInputErrorKind::RetiredRegistration
        ));
        assert_eq!(snapshot.value(INNER).unwrap().0, 1);
        assert_eq!(store.usage().unwrap().retained_units, charged);
        drop(scene);
        drop(receipts);
        drop(snapshot);
        assert_eq!(store.usage().unwrap().retained_units, 0);
    }

    #[test]
    fn recursive_reader_cannot_hide_the_cycle_and_fresh_retry_can_succeed() {
        let tree = FigureTree::new();
        let scene = scene(&tree);
        let owner = Arc::new(std::sync::Mutex::new(None::<ProjectionInputRegistry>));
        let reader_owner = owner.clone();
        let recurse = Arc::new(AtomicU64::new(1));
        let reader_recurse = recurse.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(OUTER, move |source, scene: &ProjectionScene<'_>| {
                if reader_recurse.load(Ordering::SeqCst) != 0 {
                    let registry = reader_owner.lock().unwrap().as_ref().unwrap().clone();
                    let _ = scene.input(&registry, OUTER, source);
                }
                Ok(Outer)
            })
            .unwrap();
        *owner.lock().unwrap() = Some(registry.clone());
        let error = scene.input(&registry, OUTER, tree.root_id()).unwrap_err();
        assert_eq!(
            input_kind(&error),
            &PreparationInputErrorKind::DependencyCycle
        );
        assert_eq!(scene.check(), Err(error));
        recurse.store(0, Ordering::SeqCst);
        let fresh = ProjectionScene::new(&tree, tree.root_id(), PreparationBudgetHandle::default())
            .unwrap();
        fresh.input(&registry, OUTER, tree.root_id()).unwrap();
        fresh.validate("retry").unwrap();
        // Break the instance-owned reader/registry cycle explicitly.
        *owner.lock().unwrap() = None;
    }

    #[test]
    fn empty_children_and_origin_size_reads_are_distinct_facts() {
        let mut tree = FigureTree::new();
        let child = tree.add_child_to(
            tree.root_id(),
            Box::new(RectangleFigure::new(2.0, 3.0, 4.0, 5.0).unwrap()),
        );
        let scene = scene(&tree);
        assert!(scene.children(child).unwrap().is_empty());
        assert_eq!(scene.origin(child).unwrap(), Point::new(2.0, 3.0));
        scene.validate("empty").unwrap();
        let facts = scene.facts().unwrap();
        assert!(
            facts.iter().any(|fact| {
                fact.figure == child && fact.value == CoreFact::Children(Vec::new())
            })
        );
        assert!(facts.iter().any(|fact| {
            fact.figure == child && fact.value == CoreFact::Origin(Point::new(2.0, 3.0))
        }));
        assert!(
            !facts
                .iter()
                .any(|fact| { fact.figure == child && matches!(fact.value, CoreFact::Size(_)) })
        );
        drop(scene);
        tree.set_bounds(child, 2.0, 3.0, 40.0, 50.0);
        let fresh =
            super::ProjectionScene::new(&tree, tree.root_id(), PreparationBudgetHandle::default())
                .unwrap();
        // Width changes must not invalidate an origin-only proof.
        for fact in &facts {
            assert!(fresh.current_fact(fact).unwrap() == fact.value);
        }
        drop(fresh);
        tree.add_child_to(
            child,
            Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
        );
        let fresh =
            super::ProjectionScene::new(&tree, tree.root_id(), PreparationBudgetHandle::default())
                .unwrap();
        let empty = facts
            .iter()
            .find(|fact| matches!(fact.value, CoreFact::Children(_)))
            .unwrap();
        assert!(fresh.current_fact(empty).unwrap() != empty.value);
    }

    #[test]
    fn inherited_style_records_source_path_even_when_resolved_value_is_equal() {
        let mut tree = FigureTree::new();
        let root = tree.root_id();
        let child = tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
        );
        let scene = scene(&tree);
        let resolved = scene.resolved_style(child).unwrap();
        let facts = scene.facts().unwrap();
        drop(scene);
        tree.builder()
            .set_figure_style(
                root,
                crate::FigureStyle {
                    foreground: Some(resolved.foreground),
                    ..Default::default()
                },
            )
            .unwrap();
        let fresh =
            super::ProjectionScene::new(&tree, tree.root_id(), PreparationBudgetHandle::default())
                .unwrap();
        assert_eq!(fresh.resolved_style(child).unwrap(), resolved);
        let ancestor = facts
            .iter()
            .find(|fact| fact.figure == root && matches!(fact.value, CoreFact::Style(_)))
            .unwrap();
        assert!(fresh.current_fact(ancestor).unwrap() != ancestor.value);
        assert!(
            facts
                .iter()
                .any(|fact| fact.figure == root && fact.value == CoreFact::Parent(None))
        );
    }

    #[test]
    fn verification_scope_transfers_new_core_facts_and_ignored_conflict_is_sticky() {
        let tree = FigureTree::new();
        let scene = scene(&tree);
        let enabled = Arc::new(AtomicU64::new(0));
        let switch = enabled.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(OUTER, move |source, scene: &ProjectionScene<'_>| {
                if switch.load(Ordering::SeqCst) != 0 {
                    scene.children(source)?;
                    scene.size(source)?;
                    scene.resolved_style(source)?;
                }
                Ok(Outer)
            })
            .unwrap();
        scene.input(&registry, OUTER, tree.root_id()).unwrap();
        enabled.store(1, Ordering::SeqCst);
        scene.validate("new facts").unwrap();
        let facts = scene.facts().unwrap();
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact.value, CoreFact::Children(_)))
        );
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact.value, CoreFact::Size(_)))
        );
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact.value, CoreFact::Style(_)))
        );
        let error = scene
            .record(tree.root_id(), CoreFact::Size(Dimension::new(123.0, 456.0)))
            .unwrap_err();
        assert_eq!(
            error,
            ProjectionError::CoreChanged {
                figure: tree.root_id(),
                fact: "size"
            }
        );
        assert_eq!(scene.check(), Err(error));
    }

    #[test]
    fn historical_input_difference_is_a_miss_but_new_first_value_cannot_be_replaced() {
        let tree = FigureTree::new();
        let source = tree.root_id();
        let version = Arc::new(AtomicU64::new(1));
        let reader_version = version.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(INNER, move |_, _: &ProjectionScene<'_>| {
                Ok(Inner(reader_version.load(Ordering::SeqCst)))
            })
            .unwrap();
        let previous = scene(&tree);
        previous.input(&registry, INNER, source).unwrap();
        let history = previous.receipts().unwrap().remove(0);
        drop(previous);
        version.store(2, Ordering::SeqCst);
        let operation = scene(&tree);
        let current = history.refreshed(&operation, "qualification").unwrap();
        assert_ne!(history.first, current.first);
        operation.retain(&[current]).unwrap();
        operation.validate("new value").unwrap();
        version.store(3, Ordering::SeqCst);
        let conflicting = history.refreshed(&operation, "second consumer").unwrap();
        assert!(matches!(
            input_kind(&operation.retain(&[conflicting]).unwrap_err()),
            PreparationInputErrorKind::Changed { .. }
        ));
        assert_eq!(history.first.value(INNER).unwrap().0, 1);
    }

    #[test]
    fn inherited_tooltip_backing_is_charged_before_style_clone() {
        let mut tree = FigureTree::new();
        let root = tree.root_id();
        let tooltip = "x".repeat(16_384);
        tree.builder()
            .set_figure_style(
                root,
                crate::FigureStyle {
                    tooltip: Some(Some(tooltip)),
                    ..Default::default()
                },
            )
            .unwrap();
        let bounded = ProjectionScene::new(
            &tree,
            root,
            PreparationBudgetHandle::new(PreparationBudget::new(100_000, 4_096)),
        )
        .unwrap();
        let error = bounded.resolved_style(root).unwrap_err();
        assert!(matches!(
            error,
            ProjectionError::Budget(PreparationBudgetError {
                stage: "projection style backing",
                allocation: true,
                ..
            })
        ));
        assert_eq!(bounded.check(), Err(error));
    }

    #[test]
    fn cache_miss_retires_old_consumers_but_preserves_operation_first_values() {
        let tree = FigureTree::new();
        let root = tree.root_id();
        let version = Arc::new(AtomicU64::new(1));
        let value = version.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(INNER, move |_, _: &ProjectionScene<'_>| {
                Ok(Inner(value.load(Ordering::SeqCst)))
            })
            .unwrap();
        registry
            .register(OUTER, |_, _: &ProjectionScene<'_>| Ok(Outer))
            .unwrap();
        let previous = scene(&tree);
        previous.input(&registry, INNER, root).unwrap();
        previous.input(&registry, OUTER, root).unwrap();
        let receipt = previous
            .receipt(
                ProjectionContribution::new(ChildTransform::IDENTITY),
                &crate::text::TextContentStore::new(RETAINED_BYTES),
            )
            .unwrap();
        drop(previous);
        version.store(2, Ordering::SeqCst);
        let operation = scene(&tree);
        assert!(receipt.qualify(&operation).unwrap().is_none());
        let replacement = operation.owner_scope(root).unwrap();
        replacement.input(&registry, OUTER, root).unwrap();
        version.store(3, Ordering::SeqCst);
        // INNER's old output is no longer consumed. Its qualification read still
        // constrains a later read in the same operation.
        replacement.validate("replacement final").unwrap();
        let error = replacement.input(&registry, INNER, root).unwrap_err();
        assert!(matches!(
            input_kind(&error),
            PreparationInputErrorKind::Changed { .. }
        ));
        assert_eq!(operation.check(), Err(error));
    }

    #[test]
    fn retired_reader_cannot_be_hidden_by_changed_core_cache_facts() {
        let mut tree = FigureTree::new();
        let root = tree.root_id();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(OUTER, |_, _: &ProjectionScene<'_>| Ok(Outer))
            .unwrap();
        let previous = scene(&tree);
        previous.resolved_style(root).unwrap();
        previous.input(&registry, OUTER, root).unwrap();
        let receipt = previous
            .receipt(
                ProjectionContribution::new(ChildTransform::IDENTITY),
                &crate::text::TextContentStore::new(RETAINED_BYTES),
            )
            .unwrap();
        drop(previous);
        drop(registry);
        tree.builder()
            .set_figure_style(
                root,
                crate::FigureStyle {
                    foreground: Some(crate::graphics::Color::RED),
                    ..Default::default()
                },
            )
            .unwrap();
        let operation = scene(&tree);
        let error = receipt.qualify(&operation).err().unwrap();
        assert!(matches!(
            input_kind(&error),
            PreparationInputErrorKind::RetiredRegistration
        ));
        assert_eq!(operation.check(), Err(error));
    }

    #[test]
    fn completed_receipt_includes_dependencies_discovered_by_verification() {
        let tree = FigureTree::new();
        let root = tree.root_id();
        let enabled = Arc::new(AtomicU64::new(0));
        let switch = enabled.clone();
        let mut registry = ProjectionInputRegistry::new(RETAINED_BYTES);
        registry
            .register(INNER, |_, _: &ProjectionScene<'_>| Ok(Inner(1)))
            .unwrap();
        let nested = registry.clone();
        registry
            .register(OUTER, move |source, scene: &ProjectionScene<'_>| {
                if switch.load(Ordering::SeqCst) != 0 {
                    scene.input(&nested, INNER, source)?;
                    scene.children(source)?;
                }
                Ok(Outer)
            })
            .unwrap();
        let previous = scene(&tree);
        previous.input(&registry, OUTER, root).unwrap();
        let store = crate::text::TextContentStore::new(RETAINED_BYTES);
        let initial = previous
            .receipt(
                ProjectionContribution::new(ChildTransform::IDENTITY),
                &store,
            )
            .unwrap();
        enabled.store(1, Ordering::SeqCst);
        previous.validate("discovery").unwrap();
        assert!(!initial.matches_consumption(&previous).unwrap());
        let completed = previous.receipt(initial.contribution, &store).unwrap();
        assert!(completed.matches_consumption(&previous).unwrap());
        assert!(completed.inputs.len() > initial.inputs.len());
        assert!(completed.facts.len() > initial.facts.len());
    }
}
