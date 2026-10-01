use super::{
    AnchorGeometry, AnchorGeometryKey, AnchorId, ConnectionAnchor, ConnectionId,
    ConnectionLocatorStrategy, ConnectionRouter, ConnectionRoutingStats, ConnectionRuntimeError,
    ConnectionStateSnapshot, CoordinateSpace, DependencySubject, DirectRouter, FigureId,
    FigureTreeSceneRead, Rectangle, RouteError, RouteMetadata, RouteOutput, RouteRequest,
    RouterBinding, RouterId, RoutingConstraint, RoutingGroupScope, Runtime, TrackedSceneQuery,
    UnresolvedConnection, route_metadata_in_local,
};

impl Runtime {
    pub fn direct_connection_router(&self) -> RouterId {
        self.connections.direct_router()
    }

    pub fn register_connection_anchor(&mut self, anchor: Box<dyn ConnectionAnchor>) -> AnchorId {
        self.try_register_connection_anchor(anchor)
            .expect("cannot register an Anchor in a faulted Runtime")
    }

    pub fn try_register_connection_anchor(
        &mut self,
        anchor: Box<dyn ConnectionAnchor>,
    ) -> Result<AnchorId, ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            Ok(runtime.connections.register_anchor(anchor))
        })
    }

    pub fn remove_connection_anchor(
        &mut self,
        anchor: AnchorId,
    ) -> Result<Box<dyn ConnectionAnchor>, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| runtime.connections.remove_anchor(anchor))
    }

    pub fn set_anchor_geometry(
        &mut self,
        figure: FigureId,
        key: AnchorGeometryKey,
        geometry: AnchorGeometry,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            runtime.set_anchor_geometry_inner(figure, key, geometry)
        })
    }

    fn set_anchor_geometry_inner(
        &mut self,
        figure: FigureId,
        key: AnchorGeometryKey,
        geometry: AnchorGeometry,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        if !self.tree.is_attached(figure) {
            return Err(ConnectionRuntimeError::UnknownFigure(figure));
        }
        let bounds = geometry.bounds();
        if key.is_border_box()
            || !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
        {
            return Err(ConnectionRuntimeError::InvalidAnchorGeometry);
        }
        self.anchor_geometries
            .insert((figure, key.clone()), geometry);
        self.connections
            .invalidate_dependency(&DependencySubject::NamedAnchorRegion(figure, key))
    }

    pub fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Option<&AnchorGeometry> {
        self.anchor_geometries.get(&(figure, key.clone()))
    }

    pub fn register_connection_router(&mut self, router: Box<dyn ConnectionRouter>) -> RouterId {
        self.try_register_connection_router(router)
            .expect("cannot register a Router in a faulted Runtime")
    }

    pub fn try_register_connection_router(
        &mut self,
        router: Box<dyn ConnectionRouter>,
    ) -> Result<RouterId, ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            Ok(runtime.connections.register_router(router))
        })
    }

    pub fn remove_connection_router(
        &mut self,
        router: RouterId,
    ) -> Result<Box<dyn ConnectionRouter>, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| runtime.connections.remove_router(router))
    }

    pub fn set_connection_layer_router(
        &mut self,
        layer: FigureId,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.set_connection_layer_router_inner(layer, router)
        })
    }

    fn set_connection_layer_router_inner(
        &mut self,
        layer: FigureId,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        if !self.tree.is_attached(layer) {
            return Err(ConnectionRuntimeError::UnknownFigure(layer));
        }
        self.connections.set_layer_router(layer, router)
    }

    pub fn register_connection_state(
        &mut self,
        figure: FigureId,
        source: Option<AnchorId>,
        target: Option<AnchorId>,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<ConnectionId, ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            runtime.register_connection_state_inner(figure, source, target, router, constraint)
        })
    }

    fn register_connection_state_inner(
        &mut self,
        figure: FigureId,
        source: Option<AnchorId>,
        target: Option<AnchorId>,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<ConnectionId, ConnectionRuntimeError> {
        if !self.tree.is_attached(figure) {
            return Err(ConnectionRuntimeError::UnknownFigure(figure));
        }
        if !self.tree.is_connection_figure(figure) {
            return Err(ConnectionRuntimeError::NotConnectionFigure(figure));
        }
        let connection = ConnectionId::from_figure(figure);
        self.connections
            .register_connection(connection, source, target, router, constraint)?;
        Ok(connection)
    }

    pub fn remove_connection_state(
        &mut self,
        connection: ConnectionId,
    ) -> Result<(), ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.connections.remove_connection(connection)
        })
    }

    pub fn connection_state(
        &self,
        connection: ConnectionId,
    ) -> Result<ConnectionStateSnapshot, ConnectionRuntimeError> {
        self.connections.state(connection)
    }

    /// Returns cumulative route and obstacle-snapshot work counters.
    pub fn connection_routing_stats(&self) -> ConnectionRoutingStats {
        self.connections.routing_stats()
    }

    pub fn set_connection_source(
        &mut self,
        connection: ConnectionId,
        source: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.connections.set_source(connection, source)
        })
    }

    pub fn set_connection_target(
        &mut self,
        connection: ConnectionId,
        target: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.connections.set_target(connection, target)
        })
    }

    pub fn set_connection_router_binding(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.connections.set_router_binding(connection, router)
        })
    }

    pub fn set_connection_constraint(
        &mut self,
        connection: ConnectionId,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<(), ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            runtime.connections.set_constraint(connection, constraint)
        })
    }

    /// Atomically replaces one Connection's Router binding and typed constraint.
    pub fn set_connection_route_configuration(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            runtime
                .connections
                .set_route_configuration(connection, router, constraint)
        })
    }

    /// Binds a direct Connection child to a route-derived Locator.
    pub fn set_connection_locator(
        &mut self,
        connection: ConnectionId,
        child: FigureId,
        strategy: Box<dyn ConnectionLocatorStrategy>,
    ) -> Result<(), ConnectionRuntimeError> {
        self.guarded_connection_mutation(move |runtime| {
            runtime.connections.state(connection)?;
            if !runtime.tree.is_attached(child) {
                return Err(ConnectionRuntimeError::UnknownFigure(child));
            }
            if runtime.tree.parent_id(child) != Some(connection.figure()) {
                return Err(ConnectionRuntimeError::InvalidLocatorChild { connection, child });
            }
            runtime.connections.set_locator(connection, child, strategy)
        })
    }

    /// Removes the Locator bound to a Connection child.
    pub fn remove_connection_locator(
        &mut self,
        child: FigureId,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            if !runtime.tree.is_attached(child) {
                return Err(ConnectionRuntimeError::UnknownFigure(child));
            }
            Ok(runtime.connections.remove_locator(child))
        })
    }

    /// Resolves a source/target Anchor pair without registering or mutating a Connection.
    pub fn preview_connection_endpoints(
        &self,
        source: &dyn ConnectionAnchor,
        target: &dyn ConnectionAnchor,
        routing_space: CoordinateSpace,
    ) -> Result<RouteMetadata, RouteError> {
        let scene = FigureTreeSceneRead::new(&self.tree, &self.anchor_geometries);
        let mut query = TrackedSceneQuery::new(&scene);
        DirectRouter
            .route(RouteRequest {
                connection: ConnectionId::from_figure(self.tree.synthetic_root()),
                routing_space,
                source,
                target,
                constraint: None,
                scene: &mut query,
                group: None,
            })
            .map(|output| *output.metadata())
    }

    pub fn resolve_connection_route(
        &mut self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
    ) -> Result<RouteOutput, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.resolve_connection_route_inner(connection, routing_space)
        })
    }

    fn resolve_connection_route_inner(
        &mut self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
    ) -> Result<RouteOutput, ConnectionRuntimeError> {
        let routing_order: Vec<_> = match (
            self.connections.routing_group_scope(connection)?,
            routing_space,
        ) {
            (RoutingGroupScope::None, _) => vec![connection],
            (_, CoordinateSpace::ChildContent(parent)) => self
                .tree
                .child_order(parent)
                .unwrap_or_default()
                .into_iter()
                .map(ConnectionId::from_figure)
                .collect(),
            _ => vec![connection],
        };
        let scene = FigureTreeSceneRead::new(&self.tree, &self.anchor_geometries);
        let result = self
            .connections
            .route(connection, routing_space, &scene, &routing_order);
        match result {
            Ok(batch) => {
                let requested = batch
                    .calculations()
                    .iter()
                    .find_map(|calculation| {
                        (calculation.connection() == connection)
                            .then(|| calculation.output().clone())
                    })
                    .ok_or(ConnectionRuntimeError::UnknownConnection(connection))?;
                let mut prepared = Vec::with_capacity(batch.calculations().len());
                for calculation in batch.calculations() {
                    let candidate = calculation.connection();
                    let Some(geometry) = self.tree.prepare_connection_route(
                        candidate.figure(),
                        calculation.output().points(),
                    ) else {
                        let reason = UnresolvedConnection::InvalidGeometry(candidate.figure());
                        self.connections.reject_route_batch(&batch, reason.clone());
                        for affected in batch.calculations() {
                            self.tree.clear_connection_route(
                                &mut self.updates,
                                affected.connection().figure(),
                            );
                        }
                        return Err(ConnectionRuntimeError::Unresolved(reason));
                    };
                    let local_metadata = route_metadata_in_local(
                        calculation.output().metadata(),
                        geometry.path_bounds(),
                    );
                    let placements = match self.connections.locator_placements(
                        candidate,
                        geometry.local_points(),
                        &local_metadata,
                    ) {
                        Ok(placements) => placements,
                        Err(reason) => {
                            self.connections.reject_route_batch(&batch, reason.clone());
                            for affected in batch.calculations() {
                                self.tree.clear_connection_route(
                                    &mut self.updates,
                                    affected.connection().figure(),
                                );
                            }
                            return Err(ConnectionRuntimeError::Unresolved(reason));
                        }
                    };
                    let mut child_updates = Vec::with_capacity(placements.len());
                    for (child, placement) in placements {
                        let finite_placement = [
                            placement.point.x(),
                            placement.point.y(),
                            placement.reference.x(),
                            placement.reference.y(),
                        ]
                        .into_iter()
                        .all(f64::is_finite);
                        let reason = if !finite_placement {
                            Some(UnresolvedConnection::LocatorFailed {
                                child,
                                error: crate::LocatorError::NonFinitePlacement,
                            })
                        } else if self.tree.parent_id(child) != Some(candidate.figure()) {
                            Some(UnresolvedConnection::InvalidLocatorChild(child))
                        } else {
                            None
                        };
                        if let Some(reason) = reason {
                            self.connections.reject_route_batch(&batch, reason.clone());
                            for affected in batch.calculations() {
                                self.tree.clear_connection_route(
                                    &mut self.updates,
                                    affected.connection().figure(),
                                );
                            }
                            return Err(ConnectionRuntimeError::Unresolved(reason));
                        }
                        match self.tree.prepare_connection_decoration(child, placement) {
                            Some(Ok(decoration)) => {
                                child_updates.push((child, decoration.bounds(), Some(decoration)));
                            }
                            Some(Err(error)) => {
                                let reason =
                                    UnresolvedConnection::DecorationFailed { child, error };
                                self.connections.reject_route_batch(&batch, reason.clone());
                                for affected in batch.calculations() {
                                    self.tree.clear_connection_route(
                                        &mut self.updates,
                                        affected.connection().figure(),
                                    );
                                }
                                return Err(ConnectionRuntimeError::Unresolved(reason));
                            }
                            None => {
                                let bounds = self
                                    .tree
                                    .figure_bounds(child)
                                    .expect("validated Locator child must remain attached");
                                child_updates.push((
                                    child,
                                    Rectangle::new(
                                        placement.point.x() - bounds.width / 2.0,
                                        placement.point.y() - bounds.height / 2.0,
                                        bounds.width,
                                        bounds.height,
                                    ),
                                    None,
                                ));
                            }
                        }
                    }
                    prepared.push((candidate, geometry, child_updates));
                }
                for (candidate, geometry, child_updates) in prepared {
                    self.tree.commit_prepared_connection_route(
                        &mut self.updates,
                        candidate.figure(),
                        geometry,
                    );
                    for (child, bounds, decoration) in child_updates {
                        if let Some(decoration) = decoration {
                            self.tree.commit_prepared_connection_decoration(
                                &mut self.updates,
                                child,
                                decoration,
                            );
                        } else {
                            self.tree.set_bounds_with_update(
                                &mut self.updates,
                                child,
                                bounds.x,
                                bounds.y,
                                bounds.width,
                                bounds.height,
                            );
                        }
                    }
                }
                self.connections.commit_route_batch(&batch, routing_space);
                Ok(requested)
            }
            Err(batch) => {
                for affected in batch.affected {
                    self.tree
                        .clear_connection_route(&mut self.updates, affected.figure());
                }
                Err(batch.error)
            }
        }
    }

    pub fn invalidate_connection_dependency(
        &mut self,
        subject: &DependencySubject,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.guarded_connection_mutation(|runtime| {
            runtime.connections.invalidate_dependency(subject)
        })
    }

    pub fn dirty_connections(&self) -> Vec<ConnectionId> {
        self.connections.dirty_connections()
    }

    pub fn last_connection_error(&self) -> Option<&ConnectionRuntimeError> {
        self.connection_error.as_ref()
    }

    pub fn take_connection_error(&mut self) -> Option<ConnectionRuntimeError> {
        self.connection_error.take()
    }
}
