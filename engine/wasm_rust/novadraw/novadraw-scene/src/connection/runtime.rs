use std::{collections::HashMap, error::Error, fmt};

use slotmap::SlotMap;

use super::{
    AnchorGroupKey, AnchorId, ConnectionAnchor, ConnectionId, ConnectionRouter, CoordinateSpace,
    DependencyObservation, DependencySubject, DirectRouter, RouteError, RouteOutput, RouteRequest,
    RouterId, RoutingConstraint, RoutingGroupQuery, RoutingGroupScope, SceneRead,
    TrackedSceneQuery,
};
use crate::{FigureId, MAX_TREE_DEPTH};

/// Selects either a ConnectionLayer default Router or an explicit Router.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterBinding {
    /// Resolve the Router from the owning ConnectionLayer.
    Inherited { layer: FigureId },
    /// Use one explicitly registered Router.
    Explicit { router: RouterId },
}

/// Why a Connection currently has no valid route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnresolvedConnection {
    /// Source endpoint is not bound.
    MissingSource,
    /// Target endpoint is not bound.
    MissingTarget,
    /// Anchor or Router calculation failed.
    RouteFailed(RouteError),
}

/// Current route lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionResolution {
    /// Inputs changed and a new route is required.
    Dirty { revision: u64 },
    /// A route was calculated successfully.
    Resolved { generation: u64 },
    /// No valid route can currently be produced.
    Unresolved(UnresolvedConnection),
}

/// Read-only state exposed by the Connection Runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionStateSnapshot {
    /// Source Anchor identity.
    pub source: Option<AnchorId>,
    /// Target Anchor identity.
    pub target: Option<AnchorId>,
    /// Active Router binding.
    pub router: RouterBinding,
    /// Current route lifecycle state.
    pub resolution: ConnectionResolution,
    /// Number of tracked dependency subjects.
    pub dependency_count: usize,
}

/// Failure produced by Connection Runtime state operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionRuntimeError {
    /// Figure identity is not attached to the Runtime tree.
    UnknownFigure(FigureId),
    /// Figure does not provide Connection geometry behavior.
    NotConnectionFigure(FigureId),
    /// Named Anchor geometry is empty or non-finite.
    InvalidAnchorGeometry,
    /// Connection identity is already registered.
    DuplicateConnection(ConnectionId),
    /// Connection identity is not registered.
    UnknownConnection(ConnectionId),
    /// Anchor identity is not registered.
    UnknownAnchor(AnchorId),
    /// Router identity is not registered.
    UnknownRouter(RouterId),
    /// Anchor is still bound to at least one Connection.
    AnchorInUse(AnchorId),
    /// Router is still explicitly or indirectly referenced.
    RouterInUse(RouterId),
    /// Router does not accept the current constraint type.
    ConstraintTypeMismatch {
        /// Expected concrete type, or `None` when unsupported.
        expected: Option<&'static str>,
        /// Actual concrete type.
        actual: &'static str,
    },
    /// Connection cannot currently resolve both endpoints.
    Unresolved(UnresolvedConnection),
    /// A monotonic generation counter overflowed.
    GenerationExhausted,
}

impl fmt::Display for ConnectionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(figure) => write!(formatter, "unknown Figure {figure:?}"),
            Self::NotConnectionFigure(figure) => {
                write!(
                    formatter,
                    "Figure {figure:?} does not provide Connection behavior"
                )
            }
            Self::InvalidAnchorGeometry => write!(formatter, "invalid named Anchor geometry"),
            Self::DuplicateConnection(connection) => {
                write!(formatter, "Connection {connection:?} is already registered")
            }
            Self::UnknownConnection(connection) => {
                write!(formatter, "unknown Connection {connection:?}")
            }
            Self::UnknownAnchor(anchor) => write!(formatter, "unknown Anchor {anchor:?}"),
            Self::UnknownRouter(router) => write!(formatter, "unknown Router {router:?}"),
            Self::AnchorInUse(anchor) => write!(formatter, "Anchor {anchor:?} is still in use"),
            Self::RouterInUse(router) => write!(formatter, "Router {router:?} is still in use"),
            Self::ConstraintTypeMismatch { expected, actual } => match expected {
                Some(expected) => {
                    write!(
                        formatter,
                        "expected routing constraint {expected}, got {actual}"
                    )
                }
                None => write!(formatter, "router does not accept constraint {actual}"),
            },
            Self::Unresolved(reason) => write!(formatter, "Connection is unresolved: {reason:?}"),
            Self::GenerationExhausted => write!(formatter, "Connection generation exhausted"),
        }
    }
}

impl Error for ConnectionRuntimeError {}

struct ConnectionState {
    source: Option<AnchorId>,
    target: Option<AnchorId>,
    router: RouterBinding,
    constraint: Option<Box<dyn RoutingConstraint>>,
    dependencies: HashMap<DependencySubject, u64>,
    routing_space: Option<CoordinateSpace>,
    dirty_revision: u64,
    route_generation: u64,
    resolution: ConnectionResolution,
}

pub(crate) struct ConnectionRouteBatch {
    pub(crate) outputs: Vec<(ConnectionId, RouteOutput)>,
}

pub(crate) struct ConnectionRouteBatchError {
    pub(crate) affected: Vec<ConnectionId>,
    pub(crate) error: ConnectionRuntimeError,
}

struct RouteCalculation {
    connection: ConnectionId,
    output: RouteOutput,
    observations: Vec<DependencyObservation>,
    next_generation: u64,
}

type TopologyObservations = Vec<DependencyObservation>;
type ViewportChainResult =
    Result<(Vec<FigureId>, TopologyObservations), (RouteError, TopologyObservations)>;

/// Runtime-private ownership of Connection relationships and dependency state.
pub(crate) struct ConnectionRuntime {
    anchors: SlotMap<AnchorId, Box<dyn ConnectionAnchor>>,
    routers: SlotMap<RouterId, Box<dyn ConnectionRouter>>,
    direct_router: RouterId,
    states: HashMap<ConnectionId, ConnectionState>,
    order: Vec<ConnectionId>,
    by_dependency: HashMap<DependencySubject, Vec<ConnectionId>>,
    layer_defaults: HashMap<FigureId, RouterId>,
}

impl Default for ConnectionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRuntime {
    pub(crate) fn new() -> Self {
        let mut routers: SlotMap<RouterId, Box<dyn ConnectionRouter>> = SlotMap::with_key();
        let direct_router = routers.insert(Box::new(DirectRouter));
        Self {
            anchors: SlotMap::with_key(),
            routers,
            direct_router,
            states: HashMap::new(),
            order: Vec::new(),
            by_dependency: HashMap::new(),
            layer_defaults: HashMap::new(),
        }
    }

    pub(crate) fn direct_router(&self) -> RouterId {
        self.direct_router
    }

    pub(crate) fn register_anchor(&mut self, anchor: Box<dyn ConnectionAnchor>) -> AnchorId {
        self.anchors.insert(anchor)
    }

    pub(crate) fn remove_anchor(
        &mut self,
        anchor: AnchorId,
    ) -> Result<Box<dyn ConnectionAnchor>, ConnectionRuntimeError> {
        if !self.anchors.contains_key(anchor) {
            return Err(ConnectionRuntimeError::UnknownAnchor(anchor));
        }
        if self
            .states
            .values()
            .any(|state| state.source == Some(anchor) || state.target == Some(anchor))
        {
            return Err(ConnectionRuntimeError::AnchorInUse(anchor));
        }
        self.anchors
            .remove(anchor)
            .ok_or(ConnectionRuntimeError::UnknownAnchor(anchor))
    }

    pub(crate) fn register_router(&mut self, router: Box<dyn ConnectionRouter>) -> RouterId {
        self.routers.insert(router)
    }

    pub(crate) fn remove_router(
        &mut self,
        router: RouterId,
    ) -> Result<Box<dyn ConnectionRouter>, ConnectionRuntimeError> {
        if !self.routers.contains_key(router) {
            return Err(ConnectionRuntimeError::UnknownRouter(router));
        }
        if router == self.direct_router
            || self
                .layer_defaults
                .values()
                .any(|candidate| *candidate == router)
            || self.states.values().any(|state| {
                matches!(
                    state.router,
                    RouterBinding::Explicit { router: candidate } if candidate == router
                )
            })
        {
            return Err(ConnectionRuntimeError::RouterInUse(router));
        }
        self.routers
            .remove(router)
            .ok_or(ConnectionRuntimeError::UnknownRouter(router))
    }

    pub(crate) fn set_layer_router(
        &mut self,
        layer: FigureId,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.router(router)?;
        let affected: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|connection| {
                self.states
                    .get(connection)
                    .is_some_and(|state| state.router == RouterBinding::Inherited { layer })
            })
            .collect();
        for connection in &affected {
            self.validate_constraint_for_connection(*connection, router)?;
            self.next_dirty_revision(*connection)?;
        }
        self.layer_defaults.insert(layer, router);
        for connection in &affected {
            self.mark_dirty(*connection)?;
        }
        Ok(affected)
    }

    pub(crate) fn register_connection(
        &mut self,
        connection: ConnectionId,
        source: Option<AnchorId>,
        target: Option<AnchorId>,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<(), ConnectionRuntimeError> {
        if self.states.contains_key(&connection) {
            return Err(ConnectionRuntimeError::DuplicateConnection(connection));
        }
        self.validate_anchor(source)?;
        self.validate_anchor(target)?;
        let resolved_router = self.resolve_router_binding(router)?;
        self.validate_constraint(resolved_router, constraint.as_deref())?;
        let state = ConnectionState {
            source,
            target,
            router,
            constraint,
            dependencies: HashMap::new(),
            routing_space: None,
            dirty_revision: 1,
            route_generation: 0,
            resolution: ConnectionResolution::Dirty { revision: 1 },
        };
        self.states.insert(connection, state);
        self.order.push(connection);
        self.invalidate_all()?;
        Ok(())
    }

    pub(crate) fn remove_connection(
        &mut self,
        connection: ConnectionId,
    ) -> Result<(), ConnectionRuntimeError> {
        if !self.states.contains_key(&connection) {
            return Err(ConnectionRuntimeError::UnknownConnection(connection));
        }
        self.invalidate_all()?;
        let state = self
            .states
            .remove(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?;
        self.order.retain(|candidate| *candidate != connection);
        self.remove_reverse_dependencies(connection, state.dependencies.keys());
        Ok(())
    }

    pub(crate) fn state(
        &self,
        connection: ConnectionId,
    ) -> Result<ConnectionStateSnapshot, ConnectionRuntimeError> {
        let state = self
            .states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?;
        Ok(ConnectionStateSnapshot {
            source: state.source,
            target: state.target,
            router: state.router,
            resolution: state.resolution.clone(),
            dependency_count: state.dependencies.len(),
        })
    }

    pub(crate) fn set_source(
        &mut self,
        connection: ConnectionId,
        source: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.validate_anchor(source)?;
        let state = self.state_mut(connection)?;
        if state.source == source {
            return Ok(false);
        }
        self.invalidate_all()?;
        self.state_mut(connection)?.source = source;
        Ok(true)
    }

    pub(crate) fn set_target(
        &mut self,
        connection: ConnectionId,
        target: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.validate_anchor(target)?;
        let state = self.state_mut(connection)?;
        if state.target == target {
            return Ok(false);
        }
        self.invalidate_all()?;
        self.state_mut(connection)?.target = target;
        Ok(true)
    }

    pub(crate) fn set_router_binding(
        &mut self,
        connection: ConnectionId,
        binding: RouterBinding,
    ) -> Result<bool, ConnectionRuntimeError> {
        let router = self.resolve_router_binding(binding)?;
        self.validate_constraint_for_connection(connection, router)?;
        let state = self.state_mut(connection)?;
        if state.router == binding {
            return Ok(false);
        }
        self.invalidate_all()?;
        self.state_mut(connection)?.router = binding;
        Ok(true)
    }

    pub(crate) fn set_constraint(
        &mut self,
        connection: ConnectionId,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<(), ConnectionRuntimeError> {
        let binding = self
            .states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?
            .router;
        let router = self.resolve_router_binding(binding)?;
        self.validate_constraint(router, constraint.as_deref())?;
        self.invalidate_all()?;
        self.state_mut(connection)?.constraint = constraint;
        Ok(())
    }

    pub(crate) fn route(
        &mut self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
        source: &dyn SceneRead,
        routing_order: &[ConnectionId],
    ) -> Result<ConnectionRouteBatch, ConnectionRouteBatchError> {
        let router_id = match self.route_input(connection) {
            Ok((_, _, router)) => router,
            Err(error) => {
                return Err(self.fail_route_batch(vec![connection], error, Vec::new()));
            }
        };
        let members =
            match self.routing_group_members(connection, router_id, routing_space, routing_order) {
                Ok(members) => members,
                Err(error) => {
                    return Err(self.fail_route_batch(vec![connection], error, Vec::new()));
                }
            };
        let mut group = RuntimeRoutingGroup {
            connections: members.clone(),
            routes: HashMap::new(),
        };
        let mut calculations = Vec::with_capacity(members.len());

        for member in &members {
            let calculation = match self.calculate_route(*member, routing_space, source, &group) {
                Ok(calculation) => calculation,
                Err((error, observations)) => {
                    let mut dependency_updates = calculations
                        .iter()
                        .map(|calculation: &RouteCalculation| {
                            (calculation.connection, calculation.observations.clone())
                        })
                        .collect::<Vec<_>>();
                    dependency_updates.push((*member, observations));
                    return Err(self.fail_route_batch(members, error, dependency_updates));
                }
            };
            group.routes.insert(*member, calculation.output.clone());
            calculations.push(calculation);
        }

        for calculation in &calculations {
            if let Err(error) =
                self.replace_dependencies(calculation.connection, calculation.observations.clone())
            {
                return Err(ConnectionRouteBatchError {
                    affected: members,
                    error,
                });
            }
            let state = self
                .states
                .get_mut(&calculation.connection)
                .expect("calculated connection state must remain registered");
            state.routing_space = Some(routing_space);
            state.route_generation = calculation.next_generation;
            state.resolution = ConnectionResolution::Resolved {
                generation: state.route_generation,
            };
        }

        Ok(ConnectionRouteBatch {
            outputs: calculations
                .into_iter()
                .map(|calculation| (calculation.connection, calculation.output))
                .collect(),
        })
    }

    pub(crate) fn invalidate_dependency(
        &mut self,
        subject: &DependencySubject,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.invalidate_subjects(std::slice::from_ref(subject))
    }

    pub(crate) fn dirty_connections(&self) -> Vec<ConnectionId> {
        self.order
            .iter()
            .copied()
            .filter(|connection| {
                self.states.get(connection).is_some_and(|state| {
                    matches!(state.resolution, ConnectionResolution::Dirty { .. })
                })
            })
            .collect()
    }

    pub(crate) fn invalidate_figure_change(
        &mut self,
        figure: FigureId,
        topology_changed: bool,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        let affected_subjects: Vec<_> = self
            .by_dependency
            .keys()
            .filter(|subject| match subject {
                DependencySubject::FigureGeometry(candidate)
                | DependencySubject::NamedAnchorRegion(candidate, _) => *candidate == figure,
                DependencySubject::Topology(candidate) => topology_changed && *candidate == figure,
                // Ancestor movement can change a relative transform even when
                // neither endpoint names the ancestor directly.
                DependencySubject::RelativeTransform(_, _) => true,
            })
            .cloned()
            .collect();
        self.invalidate_subjects(&affected_subjects)
    }

    pub(crate) fn invalidate_all(&mut self) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        let affected = self.order.clone();
        for connection in &affected {
            self.next_dirty_revision(*connection)?;
        }
        for connection in &affected {
            self.mark_dirty(*connection)?;
        }
        Ok(affected)
    }

    pub(crate) fn retain_connections(&mut self, mut retain: impl FnMut(FigureId) -> bool) {
        let removed: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|connection| !retain(connection.figure()))
            .collect();
        for connection in removed {
            let _ = self.remove_connection(connection);
        }
    }

    fn validate_anchor(&self, anchor: Option<AnchorId>) -> Result<(), ConnectionRuntimeError> {
        if let Some(anchor) = anchor
            && !self.anchors.contains_key(anchor)
        {
            return Err(ConnectionRuntimeError::UnknownAnchor(anchor));
        }
        Ok(())
    }

    fn route_input(
        &self,
        connection: ConnectionId,
    ) -> Result<(AnchorId, AnchorId, RouterId), ConnectionRuntimeError> {
        let state = self
            .states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?;
        let Some(source) = state.source else {
            return Err(ConnectionRuntimeError::Unresolved(
                UnresolvedConnection::MissingSource,
            ));
        };
        let Some(target) = state.target else {
            return Err(ConnectionRuntimeError::Unresolved(
                UnresolvedConnection::MissingTarget,
            ));
        };
        Ok((source, target, self.resolve_router_binding(state.router)?))
    }

    fn routing_group_members(
        &self,
        connection: ConnectionId,
        router_id: RouterId,
        _routing_space: CoordinateSpace,
        routing_order: &[ConnectionId],
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        let scope = self.router(router_id)?.routing_group_scope();
        if scope == RoutingGroupScope::None {
            return Ok(vec![connection]);
        }
        let requested_pair = if scope == RoutingGroupScope::AnchorPair {
            let (source, target, _) = self.route_input(connection)?;
            Some((
                self.anchor_group_key(source)?,
                self.anchor_group_key(target)?,
            ))
        } else {
            None
        };
        let mut connections = Vec::new();
        for candidate in routing_order {
            let Some(candidate_state) = self.states.get(candidate) else {
                continue;
            };
            if self.resolve_router_binding(candidate_state.router)? != router_id {
                continue;
            }
            let (Some(candidate_source), Some(candidate_target)) =
                (candidate_state.source, candidate_state.target)
            else {
                continue;
            };
            if let Some(requested_pair) = &requested_pair {
                let candidate_pair = (
                    self.anchor_group_key(candidate_source)?,
                    self.anchor_group_key(candidate_target)?,
                );
                if !unordered_pair_eq(requested_pair, &candidate_pair) {
                    continue;
                }
            }
            connections.push(*candidate);
        }
        if !connections.contains(&connection) {
            connections.push(connection);
        }
        Ok(connections)
    }

    fn calculate_route(
        &self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
        source: &dyn SceneRead,
        group: &RuntimeRoutingGroup,
    ) -> Result<RouteCalculation, (ConnectionRuntimeError, Vec<DependencyObservation>)> {
        let (source_id, target_id, router_id) = self
            .route_input(connection)
            .map_err(|error| (error, Vec::new()))?;
        let source_anchor = self
            .anchors
            .get(source_id)
            .ok_or(ConnectionRuntimeError::UnknownAnchor(source_id))
            .map_err(|error| (error, Vec::new()))?;
        let target_anchor = self
            .anchors
            .get(target_id)
            .ok_or(ConnectionRuntimeError::UnknownAnchor(target_id))
            .map_err(|error| (error, Vec::new()))?;
        let mut observations = validate_viewport_topology(
            connection,
            source_anchor.owner(),
            target_anchor.owner(),
            routing_space,
            source,
        )
        .map_err(|(error, observations)| {
            (
                ConnectionRuntimeError::Unresolved(UnresolvedConnection::RouteFailed(error)),
                observations,
            )
        })?;
        let router = self
            .router(router_id)
            .map_err(|error| (error, observations.clone()))?;
        let constraint = self
            .states
            .get(&connection)
            .and_then(|state| state.constraint.as_deref());
        let mut query = TrackedSceneQuery::new(source);
        let output = router.route(RouteRequest {
            connection,
            routing_space,
            source: source_anchor.as_ref(),
            target: target_anchor.as_ref(),
            constraint,
            scene: &mut query,
            group: (router.routing_group_scope() != RoutingGroupScope::None)
                .then_some(group as &dyn RoutingGroupQuery),
        });
        observations.extend(query.into_observations());
        if dependency_cycle(connection, &observations, source) {
            return Err((
                ConnectionRuntimeError::Unresolved(UnresolvedConnection::RouteFailed(
                    RouteError::DependencyCycle,
                )),
                observations,
            ));
        }
        let output = output.map_err(|error| {
            (
                ConnectionRuntimeError::Unresolved(UnresolvedConnection::RouteFailed(error)),
                observations.clone(),
            )
        })?;
        let next_generation = self
            .states
            .get(&connection)
            .expect("route input validates the connection state")
            .route_generation
            .checked_add(1)
            .ok_or(ConnectionRuntimeError::GenerationExhausted)
            .map_err(|error| (error, observations.clone()))?;
        Ok(RouteCalculation {
            connection,
            output,
            observations,
            next_generation,
        })
    }

    fn fail_route_batch(
        &mut self,
        affected: Vec<ConnectionId>,
        error: ConnectionRuntimeError,
        dependency_updates: Vec<(ConnectionId, Vec<DependencyObservation>)>,
    ) -> ConnectionRouteBatchError {
        for (connection, observations) in dependency_updates {
            let _ = self.merge_dependencies(connection, observations);
        }
        if let ConnectionRuntimeError::Unresolved(reason) = &error {
            for connection in &affected {
                let _ = self.set_unresolved(*connection, reason.clone());
            }
        }
        ConnectionRouteBatchError { affected, error }
    }

    fn anchor_group_key(&self, anchor: AnchorId) -> Result<AnchorGroupKey, ConnectionRuntimeError> {
        let anchor_strategy = self
            .anchors
            .get(anchor)
            .ok_or(ConnectionRuntimeError::UnknownAnchor(anchor))?;
        Ok(anchor_strategy
            .semantic_group_key()
            .map(AnchorGroupKey::Semantic)
            .unwrap_or(AnchorGroupKey::Instance(anchor)))
    }

    fn router(&self, router: RouterId) -> Result<&dyn ConnectionRouter, ConnectionRuntimeError> {
        self.routers
            .get(router)
            .map(Box::as_ref)
            .ok_or(ConnectionRuntimeError::UnknownRouter(router))
    }

    fn resolve_router_binding(
        &self,
        binding: RouterBinding,
    ) -> Result<RouterId, ConnectionRuntimeError> {
        let router = match binding {
            RouterBinding::Explicit { router } => router,
            RouterBinding::Inherited { layer } => self
                .layer_defaults
                .get(&layer)
                .copied()
                .unwrap_or(self.direct_router),
        };
        self.router(router)?;
        Ok(router)
    }

    fn validate_constraint_for_connection(
        &self,
        connection: ConnectionId,
        router: RouterId,
    ) -> Result<(), ConnectionRuntimeError> {
        let constraint = self
            .states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?
            .constraint
            .as_deref();
        self.validate_constraint(router, constraint)
    }

    fn validate_constraint(
        &self,
        router: RouterId,
        constraint: Option<&dyn RoutingConstraint>,
    ) -> Result<(), ConnectionRuntimeError> {
        let router = self.router(router)?;
        match (router.constraint_type(), constraint) {
            (None, None) => Ok(()),
            (None, Some(constraint)) => Err(ConnectionRuntimeError::ConstraintTypeMismatch {
                expected: None,
                actual: constraint.type_name(),
            }),
            (Some(_), None) => Ok(()),
            (Some(expected), Some(constraint)) if expected == constraint.as_any().type_id() => {
                Ok(())
            }
            (Some(_), Some(constraint)) => Err(ConnectionRuntimeError::ConstraintTypeMismatch {
                expected: router.constraint_type_name(),
                actual: constraint.type_name(),
            }),
        }
    }

    fn state_mut(
        &mut self,
        connection: ConnectionId,
    ) -> Result<&mut ConnectionState, ConnectionRuntimeError> {
        self.states
            .get_mut(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))
    }

    fn mark_dirty(&mut self, connection: ConnectionId) -> Result<(), ConnectionRuntimeError> {
        let next_revision = self.next_dirty_revision(connection)?;
        let state = self.state_mut(connection)?;
        state.dirty_revision = next_revision;
        state.resolution = ConnectionResolution::Dirty {
            revision: state.dirty_revision,
        };
        Ok(())
    }

    fn next_dirty_revision(&self, connection: ConnectionId) -> Result<u64, ConnectionRuntimeError> {
        self.states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?
            .dirty_revision
            .checked_add(1)
            .ok_or(ConnectionRuntimeError::GenerationExhausted)
    }

    fn set_unresolved(
        &mut self,
        connection: ConnectionId,
        reason: UnresolvedConnection,
    ) -> Result<(), ConnectionRuntimeError> {
        self.state_mut(connection)?.resolution = ConnectionResolution::Unresolved(reason);
        Ok(())
    }

    fn replace_dependencies(
        &mut self,
        connection: ConnectionId,
        observations: Vec<DependencyObservation>,
    ) -> Result<(), ConnectionRuntimeError> {
        let old_subjects: Vec<_> = self
            .states
            .get(&connection)
            .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?
            .dependencies
            .keys()
            .cloned()
            .collect();
        self.remove_reverse_dependencies(connection, old_subjects.iter());
        let dependencies: HashMap<_, _> = observations
            .iter()
            .map(|observation| (observation.subject.clone(), observation.generation))
            .collect();
        self.state_mut(connection)?.dependencies = dependencies;
        for observation in observations {
            let dependents = self.by_dependency.entry(observation.subject).or_default();
            if !dependents.contains(&connection) {
                dependents.push(connection);
            }
        }
        Ok(())
    }

    fn merge_dependencies(
        &mut self,
        connection: ConnectionId,
        observations: Vec<DependencyObservation>,
    ) -> Result<(), ConnectionRuntimeError> {
        {
            let state = self.state_mut(connection)?;
            for observation in &observations {
                state
                    .dependencies
                    .insert(observation.subject.clone(), observation.generation);
            }
        }
        for observation in observations {
            let dependents = self.by_dependency.entry(observation.subject).or_default();
            if !dependents.contains(&connection) {
                dependents.push(connection);
            }
        }
        Ok(())
    }

    fn remove_reverse_dependencies<'a>(
        &mut self,
        connection: ConnectionId,
        subjects: impl IntoIterator<Item = &'a DependencySubject>,
    ) {
        let mut empty = Vec::new();
        for subject in subjects {
            if let Some(dependents) = self.by_dependency.get_mut(subject) {
                dependents.retain(|candidate| *candidate != connection);
                if dependents.is_empty() {
                    empty.push(subject.clone());
                }
            }
        }
        for subject in empty {
            self.by_dependency.remove(&subject);
        }
    }

    fn invalidate_subjects(
        &mut self,
        subjects: &[DependencySubject],
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        let directly_affected: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|connection| {
                subjects.iter().any(|subject| {
                    self.by_dependency
                        .get(subject)
                        .is_some_and(|dependents| dependents.contains(connection))
                })
            })
            .collect();
        let affected = self.expand_group_invalidation(&directly_affected)?;
        for connection in &affected {
            self.next_dirty_revision(*connection)?;
        }
        for connection in &affected {
            self.mark_dirty(*connection)?;
        }
        Ok(affected)
    }

    fn expand_group_invalidation(
        &self,
        directly_affected: &[ConnectionId],
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        let mut affected = Vec::new();
        for candidate in &self.order {
            let candidate_state = self
                .states
                .get(candidate)
                .ok_or(ConnectionRuntimeError::UnknownConnection(*candidate))?;
            let candidate_router = self.resolve_router_binding(candidate_state.router)?;
            let candidate_scope = self.router(candidate_router)?.routing_group_scope();
            let shares_group = directly_affected.iter().any(|direct| {
                if candidate == direct {
                    return true;
                }
                let Some(direct_state) = self.states.get(direct) else {
                    return false;
                };
                let Ok(direct_router) = self.resolve_router_binding(direct_state.router) else {
                    return false;
                };
                if candidate_router != direct_router
                    || candidate_state.routing_space != direct_state.routing_space
                {
                    return false;
                }
                match candidate_scope {
                    RoutingGroupScope::None => false,
                    RoutingGroupScope::RoutingDomain => true,
                    RoutingGroupScope::AnchorPair => self
                        .states_share_anchor_pair(candidate_state, direct_state)
                        .unwrap_or(false),
                }
            });
            if shares_group {
                affected.push(*candidate);
            }
        }
        Ok(affected)
    }

    fn states_share_anchor_pair(
        &self,
        left: &ConnectionState,
        right: &ConnectionState,
    ) -> Result<bool, ConnectionRuntimeError> {
        let (Some(left_source), Some(left_target), Some(right_source), Some(right_target)) =
            (left.source, left.target, right.source, right.target)
        else {
            return Ok(false);
        };
        let left_pair = (
            self.anchor_group_key(left_source)?,
            self.anchor_group_key(left_target)?,
        );
        let right_pair = (
            self.anchor_group_key(right_source)?,
            self.anchor_group_key(right_target)?,
        );
        Ok(unordered_pair_eq(&left_pair, &right_pair))
    }
}

struct RuntimeRoutingGroup {
    connections: Vec<ConnectionId>,
    routes: HashMap<ConnectionId, RouteOutput>,
}

impl RoutingGroupQuery for RuntimeRoutingGroup {
    fn ordered_connections(&self) -> &[ConnectionId] {
        &self.connections
    }

    fn route(&self, connection: ConnectionId) -> Option<&RouteOutput> {
        self.routes.get(&connection)
    }
}

fn validate_viewport_topology(
    connection: ConnectionId,
    source_owner: Option<FigureId>,
    target_owner: Option<FigureId>,
    _routing_space: CoordinateSpace,
    scene: &dyn SceneRead,
) -> Result<TopologyObservations, (RouteError, TopologyObservations)> {
    let connection_parent = scene.parent_id(connection.figure()).ok_or_else(|| {
        (
            RouteError::UnsupportedViewportTopology,
            Vec::<DependencyObservation>::new(),
        )
    })?;
    let (connection_chain, mut observations) = viewport_chain(connection_parent, scene)?;
    for owner in [source_owner, target_owner].into_iter().flatten() {
        let (owner_chain, owner_observations) = viewport_chain(owner, scene)?;
        observations.extend(owner_observations);
        if owner_chain != connection_chain {
            return Err((
                RouteError::UnsupportedViewportTopology,
                deduplicate_observations(observations),
            ));
        }
    }
    Ok(deduplicate_observations(observations))
}

fn viewport_chain(figure: FigureId, scene: &dyn SceneRead) -> ViewportChainResult {
    let mut chain = Vec::new();
    let mut observations = Vec::new();
    let mut current = Some(figure);
    for _ in 0..=MAX_TREE_DEPTH {
        let Some(candidate) = current else {
            return Ok((chain, observations));
        };
        let subject = DependencySubject::Topology(candidate);
        observations.push(DependencyObservation {
            generation: scene.dependency_generation(&subject),
            subject,
        });
        if !scene.is_attached(candidate) {
            return Err((RouteError::UnsupportedViewportTopology, observations));
        }
        if scene.is_viewport(candidate) {
            chain.push(candidate);
        }
        current = scene.parent_id(candidate);
    }
    Err((RouteError::UnsupportedViewportTopology, observations))
}

fn deduplicate_observations(observations: TopologyObservations) -> TopologyObservations {
    let mut generations = HashMap::new();
    let mut result = Vec::new();
    for observation in observations {
        if generations.contains_key(&observation.subject) {
            continue;
        }
        generations.insert(observation.subject.clone(), observation.generation);
        result.push(observation);
    }
    result
}

fn unordered_pair_eq(
    left: &(AnchorGroupKey, AnchorGroupKey),
    right: &(AnchorGroupKey, AnchorGroupKey),
) -> bool {
    (left.0 == right.0 && left.1 == right.1) || (left.0 == right.1 && left.1 == right.0)
}

fn dependency_cycle(
    connection: ConnectionId,
    observations: &[DependencyObservation],
    scene: &dyn SceneRead,
) -> bool {
    observations.iter().any(|observation| {
        dependency_figures(&observation.subject)
            .into_iter()
            .flatten()
            .any(|figure| is_self_or_descendant(connection.figure(), figure, scene))
    })
}

fn dependency_figures(subject: &DependencySubject) -> [Option<FigureId>; 2] {
    match subject {
        DependencySubject::FigureGeometry(figure)
        | DependencySubject::NamedAnchorRegion(figure, _)
        | DependencySubject::Topology(figure) => [Some(*figure), None],
        DependencySubject::RelativeTransform(from, to) => {
            [coordinate_figure(*from), coordinate_figure(*to)]
        }
    }
}

fn coordinate_figure(space: CoordinateSpace) -> Option<FigureId> {
    match space {
        CoordinateSpace::FigureLocal(figure) | CoordinateSpace::ChildContent(figure) => {
            Some(figure)
        }
        CoordinateSpace::LogicalSurface => None,
    }
}

fn is_self_or_descendant(
    connection: FigureId,
    mut candidate: FigureId,
    scene: &dyn SceneRead,
) -> bool {
    for _ in 0..=MAX_TREE_DEPTH {
        if candidate == connection {
            return true;
        }
        let Some(parent) = scene.parent_id(candidate) else {
            return false;
        };
        candidate = parent;
    }
    true
}
