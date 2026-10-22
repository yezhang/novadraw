use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fmt,
    marker::PhantomData,
    sync::Arc,
};
use uuid::Uuid;

use crate::geometry::{Point, Rectangle};

use super::{
    AttachmentOffset, AttachmentSide, GraphAttachment, GraphDirection, GraphEdgeId, GraphEndpoint,
    GraphLayoutRequest, GraphRequestIdentity,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphStage {
    Request,
    Containment,
    Connectivity,
    Algorithm,
    Ranking,
    Ordering,
    Placement,
    Routing,
    Measurement,
    Result,
    Capability,
}

/// Work and auxiliary allocation units are independent limits. Units count collection entries;
/// payload implementations must charge their own internal allocations before performing them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphBudget {
    pub work: u64,
    pub allocation: u64,
}
impl GraphBudget {
    pub const fn new(work: u64, allocation: u64) -> Self {
        Self { work, allocation }
    }
}

pub struct GraphSolveContext {
    budget: GraphBudget,
    used: GraphBudget,
}
impl GraphSolveContext {
    pub fn new(budget: GraphBudget) -> Self {
        Self {
            budget,
            used: GraphBudget::new(0, 0),
        }
    }
    pub fn used(&self) -> GraphBudget {
        self.used
    }
    pub fn consume(&mut self, stage: GraphStage, units: u64) -> Result<(), GraphLayoutError> {
        self.used.work = checked_charge(stage, self.used.work, units, self.budget.work, false)?;
        Ok(())
    }
    pub fn allocate(&mut self, stage: GraphStage, units: u64) -> Result<(), GraphLayoutError> {
        let allocation = checked_charge(
            stage,
            self.used.allocation,
            units,
            self.budget.allocation,
            true,
        )?;
        self.consume(stage, units)?;
        self.used.allocation = allocation;
        Ok(())
    }
}
fn checked_charge(
    stage: GraphStage,
    used: u64,
    units: u64,
    limit: u64,
    allocation: bool,
) -> Result<u64, GraphLayoutError> {
    let next = used
        .checked_add(units)
        .ok_or(GraphLayoutError::Overflow(stage))?;
    if next > limit {
        return Err(GraphLayoutError::BudgetExceeded {
            stage,
            used: next,
            limit,
            allocation,
        });
    }
    Ok(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphViolation {
    Geometry,
    Size,
    Containment,
    Overlap,
    Padding,
    Attachment,
    Clearance,
    Extent,
    IdentityCoverage,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphLayoutError {
    UnknownEntity(GraphEndpoint),
    UnknownEdge(GraphEdgeId),
    DuplicateEntity(GraphEndpoint),
    DuplicateEdge(GraphEdgeId),
    InvalidInput {
        entity: Option<GraphEndpoint>,
        edge: Option<GraphEdgeId>,
        violation: GraphViolation,
    },
    RejectedCandidate {
        entity: Option<GraphEndpoint>,
        edge: Option<GraphEdgeId>,
        violation: GraphViolation,
    },
    RequestMismatch,
    RegistryMismatch,
    ContainmentCycle(super::GraphGroupId),
    DepthExceeded,
    Disconnected,
    UnsupportedGroups,
    UnsupportedSelfLoop(GraphEdgeId),
    UnknownAlgorithm(&'static str),
    DuplicateRegistration(&'static str),
    UnsupportedConstraint(&'static str),
    UnsupportedCapability(&'static str),
    MissingCapability(&'static str),
    InvalidPayload(&'static str),
    CapabilityRejected(&'static str),
    BudgetExceeded {
        stage: GraphStage,
        used: u64,
        limit: u64,
        allocation: bool,
    },
    Overflow(GraphStage),
    UnsatisfiedRankConstraint(GraphEdgeId),
    UnsatisfiedSizeConstraint(GraphEndpoint),
    NonConvergentMeasurement,
}
impl fmt::Display for GraphLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "graph layout: {self:?}")
    }
}
impl std::error::Error for GraphLayoutError {}

/// Owned frozen configuration. Implementations must not change its semantics through interior mutation.
pub trait GraphAlgorithmPayload: Any + Send + Sync {}
/// Owned frozen input constraint. Its validator runs before the algorithm.
pub trait GraphConstraintPayload: Any + Send + Sync {}
/// Owned frozen result evidence, checked by the capability's registered validator.
pub trait GraphCapabilityPayload: Any + Send + Sync {}

/// The entity or edge which owns an open input constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphConstraintOwner {
    /// Node or group.
    Entity(GraphEndpoint),
    /// Directed edge.
    Edge(GraphEdgeId),
}

/// Input validator shared by standard and consumer-defined constraints.
pub trait GraphConstraintValidator<P: GraphConstraintPayload>: Send + Sync {
    /// Reject malformed input before the solver can use it.
    fn validate(
        &self,
        payload: &P,
        owner: GraphConstraintOwner,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError>;
}
impl<P, F> GraphConstraintValidator<P> for F
where
    P: GraphConstraintPayload,
    F: Fn(
            &P,
            GraphConstraintOwner,
            &GraphLayoutRequest,
            &mut GraphSolveContext,
        ) -> Result<(), GraphLayoutError>
        + Send
        + Sync,
{
    fn validate(
        &self,
        payload: &P,
        owner: GraphConstraintOwner,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        self(payload, owner, request, context)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CapabilityIdentity {
    pub id: TypeId,
    pub name: &'static str,
}
macro_rules! typed_key {
    ($name:ident, $bound:ident) => {
        /// Payload type owns identity. The name is for diagnostics only.
        pub struct $name<P> {
            name: &'static str,
            marker: PhantomData<fn() -> P>,
        }
        impl<P> Copy for $name<P> {}
        impl<P> Clone for $name<P> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<P> $name<P> {
            pub const fn new(name: &'static str) -> Self {
                Self {
                    name,
                    marker: PhantomData,
                }
            }
            pub const fn name(self) -> &'static str {
                self.name
            }
        }
        impl<P: $bound> $name<P> {
            pub(crate) fn identity(self) -> CapabilityIdentity {
                CapabilityIdentity {
                    id: TypeId::of::<P>(),
                    name: self.name,
                }
            }
        }
    };
}
typed_key!(GraphAlgorithmKey, GraphAlgorithmPayload);
typed_key!(GraphConstraintKey, GraphConstraintPayload);
typed_key!(GraphCapabilityKey, GraphCapabilityPayload);

#[derive(Clone)]
struct Payload {
    identity: CapabilityIdentity,
    value: Arc<dyn Any + Send + Sync>,
}
macro_rules! payload_store {
    ($name:ident, $key:ident, $bound:ident) => {
        #[derive(Clone, Default)]
        pub struct $name {
            entries: Vec<Payload>,
        }
        impl $name {
            pub fn new() -> Self {
                Self::default()
            }
            /// Number of frozen typed payloads.
            pub fn len(&self) -> usize {
                self.entries.len()
            }
            /// Whether there are no typed payloads.
            pub fn is_empty(&self) -> bool {
                self.entries.is_empty()
            }
            pub fn insert<P: $bound>(
                &mut self,
                key: $key<P>,
                value: P,
                context: &mut GraphSolveContext,
            ) -> Result<(), GraphLayoutError> {
                context.consume(GraphStage::Capability, self.entries.len() as u64)?;
                if self
                    .entries
                    .iter()
                    .any(|entry| entry.identity.id == TypeId::of::<P>())
                {
                    return Err(GraphLayoutError::DuplicateRegistration(key.name));
                }
                context.allocate(GraphStage::Capability, 1)?;
                self.entries.push(Payload {
                    identity: key.identity(),
                    value: Arc::new(value),
                });
                Ok(())
            }
            pub fn get<P: $bound>(&self, key: $key<P>) -> Result<Option<&P>, GraphLayoutError> {
                self.entries
                    .iter()
                    .find(|entry| entry.identity.id == TypeId::of::<P>())
                    .map(|entry| {
                        entry
                            .value
                            .downcast_ref::<P>()
                            .ok_or(GraphLayoutError::InvalidPayload(key.name))
                    })
                    .transpose()
            }
        }
    };
}
payload_store!(GraphConstraints, GraphConstraintKey, GraphConstraintPayload);
payload_store!(
    GraphResultCapabilities,
    GraphCapabilityKey,
    GraphCapabilityPayload
);
impl GraphResultCapabilities {
    fn identities(&self) -> impl Iterator<Item = CapabilityIdentity> + '_ {
        self.entries.iter().map(|entry| entry.identity)
    }
}

#[derive(Clone)]
pub(crate) struct AlgorithmSelection {
    identity: CapabilityIdentity,
    value: Arc<dyn Any + Send + Sync>,
}
impl AlgorithmSelection {
    pub fn new<P: GraphAlgorithmPayload>(key: GraphAlgorithmKey<P>, value: P) -> Self {
        Self {
            identity: key.identity(),
            value: Arc::new(value),
        }
    }
}

/// Capabilities explicitly declared by each registration. No concrete algorithm dispatch exists.
#[derive(Clone, Default)]
pub struct GraphAlgorithmCapabilities {
    pub disconnected: bool,
    pub self_loops: bool,
    pub groups: bool,
    constraints: HashSet<TypeId>,
    results: HashSet<TypeId>,
}
impl GraphAlgorithmCapabilities {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn constraint<P: GraphConstraintPayload>(mut self, key: GraphConstraintKey<P>) -> Self {
        self.constraints.insert(key.identity().id);
        self
    }
    pub fn result<P: GraphCapabilityPayload>(mut self, key: GraphCapabilityKey<P>) -> Self {
        self.results.insert(key.identity().id);
        self
    }
}

pub trait GraphLayoutAlgorithm<P: GraphAlgorithmPayload>: Send + Sync {
    fn solve(
        &self,
        payload: &P,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutCandidate, GraphLayoutError>;
}
impl<P, F> GraphLayoutAlgorithm<P> for F
where
    P: GraphAlgorithmPayload,
    F: Fn(
            &P,
            &GraphLayoutRequest,
            &mut GraphSolveContext,
        ) -> Result<GraphLayoutCandidate, GraphLayoutError>
        + Send
        + Sync,
{
    fn solve(
        &self,
        payload: &P,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutCandidate, GraphLayoutError> {
        self(payload, request, context)
    }
}

pub trait GraphCapabilityValidator<P: GraphCapabilityPayload>: Send + Sync {
    fn validate(
        &self,
        payload: &P,
        request: &GraphLayoutRequest,
        candidate: &GraphLayoutCandidate,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError>;
}
impl<P, F> GraphCapabilityValidator<P> for F
where
    P: GraphCapabilityPayload,
    F: Fn(
            &P,
            &GraphLayoutRequest,
            &GraphLayoutCandidate,
            &mut GraphSolveContext,
        ) -> Result<(), GraphLayoutError>
        + Send
        + Sync,
{
    fn validate(
        &self,
        payload: &P,
        request: &GraphLayoutRequest,
        candidate: &GraphLayoutCandidate,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        self(payload, request, candidate, context)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphPlacement {
    pub entity: GraphEndpoint,
    pub bounds: Rectangle,
}

/// A selected side, offset and group border inset, tied to one original endpoint.
/// Capability validators check algorithm-specific head/tail provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedGraphAttachment {
    pub endpoint: GraphEndpoint,
    pub side: AttachmentSide,
    pub offset: AttachmentOffset,
    pub inset: f64,
    pub position: Point,
}
impl PreparedGraphAttachment {
    /// Compute a boundary point in the consumer coordinate domain. Validation still checks
    /// the request's side/offset and the algorithm's group-boundary provenance.
    pub fn on_bounds(
        endpoint: GraphEndpoint,
        bounds: Rectangle,
        side: AttachmentSide,
        attachment: GraphAttachment,
        inset: f64,
    ) -> Self {
        let cross = match side {
            AttachmentSide::Top | AttachmentSide::Bottom => bounds.width,
            AttachmentSide::Left | AttachmentSide::Right => bounds.height,
        };
        let offset = match attachment.offset {
            AttachmentOffset::Center => cross / 2.0,
            AttachmentOffset::Distance(value) => value,
        };
        let position = match side {
            AttachmentSide::Top => Point::new(bounds.x + offset, bounds.y + inset),
            AttachmentSide::Bottom => {
                Point::new(bounds.x + offset, bounds.y + bounds.height - inset)
            }
            AttachmentSide::Left => Point::new(bounds.x + inset, bounds.y + offset),
            AttachmentSide::Right => Point::new(bounds.x + bounds.width - inset, bounds.y + offset),
        };
        Self {
            endpoint,
            side,
            offset: attachment.offset,
            inset,
            position,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphRoute {
    pub edge: GraphEdgeId,
    pub source: PreparedGraphAttachment,
    pub target: PreparedGraphAttachment,
    /// Complete source-to-target path, including its prepared endpoints.
    pub points: Vec<Point>,
}

/// Mutable proposal. Only a validated result may enter a consumer's success cache.
#[derive(Clone)]
pub struct GraphLayoutCandidate {
    pub request: GraphRequestIdentity,
    pub direction: GraphDirection,
    pub placements: Vec<GraphPlacement>,
    pub routes: Vec<GraphRoute>,
    pub extent: Rectangle,
    pub capabilities: GraphResultCapabilities,
}
impl GraphLayoutCandidate {
    pub fn new(request: &GraphLayoutRequest) -> Self {
        Self {
            request: request.identity(),
            direction: request.direction(),
            placements: Vec::new(),
            routes: Vec::new(),
            extent: Rectangle::ZERO,
            capabilities: GraphResultCapabilities::new(),
        }
    }
}

/// Immutable checked proposal bound to the exact request and validator registry generation.
#[derive(Clone)]
pub struct GraphLayoutResult {
    candidate: GraphLayoutCandidate,
    registry: Uuid,
}
impl GraphLayoutResult {
    pub fn validate(
        request: &GraphLayoutRequest,
        candidate: GraphLayoutCandidate,
        service: &GraphLayoutService,
        context: &mut GraphSolveContext,
    ) -> Result<Self, GraphLayoutError> {
        super::validation::validate_request(request, context)?;
        let registration = service.admit(request, context)?;
        super::validation::validate_candidate(request, &candidate, context)?;
        service.validate_capabilities(request, &candidate, registration, context)?;
        Ok(Self {
            candidate,
            registry: service.generation,
        })
    }
    pub fn identity(&self) -> GraphRequestIdentity {
        self.candidate.request
    }
    pub fn direction(&self) -> GraphDirection {
        self.candidate.direction
    }
    pub fn extent(&self) -> Rectangle {
        self.candidate.extent
    }
    pub fn placements(&self) -> &[GraphPlacement] {
        &self.candidate.placements
    }
    pub fn routes(&self) -> &[GraphRoute] {
        &self.candidate.routes
    }
    /// Checked lookup in this result's exact graph domain.
    pub fn bounds(&self, entity: impl Into<GraphEndpoint>) -> Result<Rectangle, GraphLayoutError> {
        let entity = entity.into();
        self.placements()
            .iter()
            .find(|placement| placement.entity == entity)
            .map(|placement| placement.bounds)
            .ok_or(GraphLayoutError::UnknownEntity(entity))
    }
    /// Checked lookup in original edge direction.
    pub fn route(&self, edge: GraphEdgeId) -> Result<&GraphRoute, GraphLayoutError> {
        self.routes()
            .iter()
            .find(|route| route.edge == edge)
            .ok_or(GraphLayoutError::UnknownEdge(edge))
    }
    pub fn capability<P: GraphCapabilityPayload>(
        &self,
        key: GraphCapabilityKey<P>,
    ) -> Result<Option<&P>, GraphLayoutError> {
        self.candidate.capabilities.get(key)
    }
    pub fn check_prefix(
        &self,
        request: &GraphLayoutRequest,
        service: &GraphLayoutService,
    ) -> Result<(), GraphLayoutError> {
        if self.identity() != request.identity() {
            return Err(GraphLayoutError::RequestMismatch);
        }
        if self.registry != service.generation {
            return Err(GraphLayoutError::RegistryMismatch);
        }
        Ok(())
    }
}

type SolveHandler = dyn Fn(
        &(dyn Any + Send + Sync),
        &GraphLayoutRequest,
        &mut GraphSolveContext,
    ) -> Result<GraphLayoutCandidate, GraphLayoutError>
    + Send
    + Sync;
type ValidateHandler = dyn Fn(
        &(dyn Any + Send + Sync),
        &GraphLayoutRequest,
        &GraphLayoutCandidate,
        &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError>
    + Send
    + Sync;
type ConstraintHandler = dyn Fn(
        &(dyn Any + Send + Sync),
        GraphConstraintOwner,
        &GraphLayoutRequest,
        &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError>
    + Send
    + Sync;
struct AlgorithmRegistration {
    capabilities: GraphAlgorithmCapabilities,
    solve: Arc<SolveHandler>,
}

/// Registry and validation authority owned by one consumer. Every standard solver registers here.
pub struct GraphLayoutService {
    algorithms: HashMap<TypeId, AlgorithmRegistration>,
    validators: HashMap<TypeId, Arc<ValidateHandler>>,
    constraints: HashMap<TypeId, Arc<ConstraintHandler>>,
    generation: Uuid,
}
impl Default for GraphLayoutService {
    fn default() -> Self {
        Self {
            algorithms: HashMap::new(),
            validators: HashMap::new(),
            constraints: HashMap::new(),
            generation: Uuid::new_v4(),
        }
    }
}
impl GraphLayoutService {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<P: GraphAlgorithmPayload>(
        &mut self,
        key: GraphAlgorithmKey<P>,
        capabilities: GraphAlgorithmCapabilities,
        algorithm: impl GraphLayoutAlgorithm<P> + 'static,
    ) -> Result<(), GraphLayoutError> {
        if self.algorithms.contains_key(&TypeId::of::<P>()) {
            return Err(GraphLayoutError::DuplicateRegistration(key.name));
        }
        self.algorithms.insert(
            TypeId::of::<P>(),
            AlgorithmRegistration {
                capabilities,
                solve: Arc::new(move |payload, request, context| {
                    algorithm.solve(
                        payload
                            .downcast_ref::<P>()
                            .ok_or(GraphLayoutError::InvalidPayload(key.name))?,
                        request,
                        context,
                    )
                }),
            },
        );
        self.generation = Uuid::new_v4();
        Ok(())
    }
    pub fn register_capability<P: GraphCapabilityPayload>(
        &mut self,
        key: GraphCapabilityKey<P>,
        validator: impl GraphCapabilityValidator<P> + 'static,
    ) -> Result<(), GraphLayoutError> {
        if self.validators.contains_key(&TypeId::of::<P>()) {
            return Err(GraphLayoutError::DuplicateRegistration(key.name));
        }
        self.validators.insert(
            TypeId::of::<P>(),
            Arc::new(move |payload, request, candidate, context| {
                validator.validate(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(GraphLayoutError::InvalidPayload(key.name))?,
                    request,
                    candidate,
                    context,
                )
            }),
        );
        self.generation = Uuid::new_v4();
        Ok(())
    }
    /// Register typed input admission; names do not participate in dispatch.
    pub fn register_constraint<P: GraphConstraintPayload>(
        &mut self,
        key: GraphConstraintKey<P>,
        validator: impl GraphConstraintValidator<P> + 'static,
    ) -> Result<(), GraphLayoutError> {
        if self.constraints.contains_key(&TypeId::of::<P>()) {
            return Err(GraphLayoutError::DuplicateRegistration(key.name));
        }
        self.constraints.insert(
            TypeId::of::<P>(),
            Arc::new(move |payload, owner, request, context| {
                validator.validate(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(GraphLayoutError::InvalidPayload(key.name))?,
                    owner,
                    request,
                    context,
                )
            }),
        );
        self.generation = Uuid::new_v4();
        Ok(())
    }
    pub fn solve(
        &self,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutResult, GraphLayoutError> {
        super::validation::validate_request(request, context)?;
        let registration = self.admit(request, context)?;
        context.consume(GraphStage::Algorithm, 1)?;
        let candidate = (registration.solve)(request.algorithm.value.as_ref(), request, context)?;
        super::validation::validate_candidate(request, &candidate, context)?;
        self.validate_capabilities(request, &candidate, registration, context)?;
        Ok(GraphLayoutResult {
            candidate,
            registry: self.generation,
        })
    }
    fn admit(
        &self,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<&AlgorithmRegistration, GraphLayoutError> {
        context.consume(GraphStage::Algorithm, 1)?;
        let registration = self.algorithms.get(&request.algorithm.identity.id).ok_or(
            GraphLayoutError::UnknownAlgorithm(request.algorithm.identity.name),
        )?;
        let supports = &registration.capabilities;
        if !supports.groups && !request.groups().is_empty() {
            return Err(GraphLayoutError::UnsupportedGroups);
        }
        for (owner, constraints) in request
            .nodes()
            .iter()
            .map(|node| {
                (
                    GraphConstraintOwner::Entity(node.id().into()),
                    &node.constraints,
                )
            })
            .chain(request.groups().iter().map(|group| {
                (
                    GraphConstraintOwner::Entity(group.id().into()),
                    &group.constraints,
                )
            }))
            .chain(
                request
                    .edges()
                    .iter()
                    .map(|edge| (GraphConstraintOwner::Edge(edge.id()), &edge.constraints)),
            )
        {
            context.consume(GraphStage::Capability, 1)?;
            for entry in &constraints.entries {
                context.consume(GraphStage::Capability, 1)?;
                if !supports.constraints.contains(&entry.identity.id) {
                    return Err(GraphLayoutError::UnsupportedConstraint(entry.identity.name));
                }
                let validator = self
                    .constraints
                    .get(&entry.identity.id)
                    .ok_or(GraphLayoutError::UnsupportedConstraint(entry.identity.name))?;
                validator(entry.value.as_ref(), owner, request, context)?;
            }
        }
        for required in &request.required {
            context.consume(GraphStage::Capability, 1)?;
            if !supports.results.contains(&required.id)
                || !self.validators.contains_key(&required.id)
            {
                return Err(GraphLayoutError::UnsupportedCapability(required.name));
            }
        }
        for edge in request.edges() {
            context.consume(GraphStage::Connectivity, 1)?;
            if !supports.self_loops && edge.source == edge.target {
                return Err(GraphLayoutError::UnsupportedSelfLoop(edge.id()));
            }
        }
        if !supports.disconnected {
            super::validation::validate_connected(request, context)?;
        }
        Ok(registration)
    }
    fn validate_capabilities(
        &self,
        request: &GraphLayoutRequest,
        candidate: &GraphLayoutCandidate,
        registration: &AlgorithmRegistration,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        for required in &request.required {
            context.consume(
                GraphStage::Capability,
                candidate.capabilities.entries.len() as u64,
            )?;
            if !candidate
                .capabilities
                .identities()
                .any(|actual| actual.id == required.id)
            {
                return Err(GraphLayoutError::MissingCapability(required.name));
            }
        }
        for entry in &candidate.capabilities.entries {
            context.consume(GraphStage::Capability, 1)?;
            if !registration
                .capabilities
                .results
                .contains(&entry.identity.id)
            {
                return Err(GraphLayoutError::UnsupportedCapability(entry.identity.name));
            }
            let validator = self
                .validators
                .get(&entry.identity.id)
                .ok_or(GraphLayoutError::UnsupportedCapability(entry.identity.name))?;
            validator(entry.value.as_ref(), request, candidate, context)?;
        }
        Ok(())
    }
}
