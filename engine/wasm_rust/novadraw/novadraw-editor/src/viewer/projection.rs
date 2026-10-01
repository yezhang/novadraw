//! Model snapshot refresh and EditPart/Figure projection coordination.

use super::*;

impl<A, F> GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    /// Applies one validated notification batch to the EditPart and Figure projections.
    pub fn refresh(&mut self) -> Result<bool, ViewerError> {
        self.ensure_ready()?;
        match catch_unwind(AssertUnwindSafe(|| self.refresh_projection())) {
            Ok(result) => result,
            Err(payload) => {
                self.force_drop_direct_text_edit();
                self.faulted = true;
                resume_unwind(payload)
            }
        }
    }

    fn refresh_projection(&mut self) -> Result<bool, ViewerError> {
        let events = self.model.drain_events();
        if events.is_empty() {
            if self.model.revision() == self.applied_revision {
                return Ok(false);
            }
            return self.fail_revision(self.model.revision());
        }
        let final_revision = self.validate_revisions(&events)?;
        if self.model.revision() != final_revision {
            return self.fail_revision(self.model.revision());
        }

        let snapshot = match ModelSnapshot::capture(&self.model) {
            Ok(snapshot) if snapshot.root != self.model_root => {
                return self.fail(ViewerError::RootChanged);
            }
            Ok(snapshot) if snapshot.revision != final_revision => {
                return self.fail(ViewerError::SnapshotRevisionChanged);
            }
            Ok(snapshot) => snapshot,
            Err(error) => return self.fail(error),
        };
        let result = self.parts_to_retire(&snapshot).and_then(|retiring| {
            self.prepare_connections_for_containment_change(&snapshot, &retiring)
                .and_then(|()| self.remove_reparented_subtrees(&snapshot))
                .and_then(|()| self.synchronize_subtree(self.contents(), snapshot.root, &snapshot))
                .and_then(|()| self.synchronize_connections(&snapshot))
        });
        match result {
            Ok(()) => {
                self.applied_revision = final_revision;
                self.synchronize_direct_text_input_area()?;
                Ok(true)
            }
            Err(error) => self.fail(error),
        }
    }

    fn validate_revisions(
        &mut self,
        events: &[crate::ModelEvent<A::ModelId, A::Event>],
    ) -> Result<ModelRevision, ViewerError> {
        let mut current = self.applied_revision;
        let mut advanced = false;
        for event in events {
            let actual = event.revision();
            if advanced && actual == current {
                continue;
            }
            if actual <= current {
                return self.fail(ViewerError::StaleRevision {
                    applied: current,
                    actual,
                });
            }
            let expected = current.next().map_err(|_| ViewerError::RevisionGap {
                expected: current,
                actual,
            })?;
            if actual != expected {
                return self.fail(ViewerError::RevisionGap { expected, actual });
            }
            current = actual;
            advanced = true;
        }
        Ok(current)
    }

    fn fail_revision<T>(&mut self, actual: ModelRevision) -> Result<T, ViewerError> {
        let expected = self
            .applied_revision
            .next()
            .unwrap_or(self.applied_revision);
        self.fail(if actual <= self.applied_revision {
            ViewerError::StaleRevision {
                applied: self.applied_revision,
                actual,
            }
        } else {
            ViewerError::RevisionGap { expected, actual }
        })
    }

    fn fail<T>(&mut self, error: ViewerError) -> Result<T, ViewerError> {
        self.force_drop_direct_text_edit();
        self.faulted = true;
        Err(error)
    }

    pub(super) fn force_drop_direct_text_edit(&mut self) {
        self.direct_text_blink_deadline = None;
        let Some(active) = self.direct_text_edit.take() else {
            return;
        };
        self.text_input_effects.push(TextInputEffect::Release {
            session: active.state.session(),
        });
        for figure in active.feedback {
            if self.runtime.tree().is_attached(figure) {
                let _ = self.runtime.dispose_subtree(figure);
            }
            self.visual_registry.remove(&figure);
        }
    }

    pub(super) fn ensure_ready(&self) -> Result<(), ViewerError> {
        if self.faulted {
            return Err(ViewerError::Faulted);
        }
        Ok(())
    }

    fn activate_part(&mut self, part: EditPartId, model_id: A::ModelId) -> Result<(), ViewerError> {
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .activate(&self.model, model_id)?;
        let host = match self.policy_host(part) {
            Ok(host) => host,
            Err(error) => {
                self.behaviors
                    .get_mut(part)
                    .expect("behavior remains installed during activation")
                    .deactivate(&self.model, model_id);
                return Err(error);
            }
        };
        let policy_error = self.policies.roles_mut(part).and_then(|roles| {
            let mut activated = 0;
            let error = roles.values_mut().find_map(|policy| {
                policy
                    .activate(host, &self.model)
                    .map(|()| {
                        activated += 1;
                    })
                    .err()
            });
            if error.is_some() {
                for policy in roles.values_mut().take(activated).rev() {
                    policy.deactivate(host, &self.model);
                }
            }
            error
        });
        if let Some(error) = policy_error {
            self.behaviors
                .get_mut(part)
                .expect("behavior remains installed during activation rollback")
                .deactivate(&self.model, model_id);
            return Err(error.into());
        }
        if let Err(error) = self.parts.set_active(part, true) {
            if let Some(roles) = self.policies.roles_mut(part) {
                for policy in roles.values_mut().rev() {
                    policy.deactivate(host, &self.model);
                }
            }
            self.behaviors
                .get_mut(part)
                .expect("behavior remains installed during activation rollback")
                .deactivate(&self.model, model_id);
            return Err(error.into());
        }
        Ok(())
    }

    fn deactivate_part(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        if !node.is_active() {
            return Ok(());
        }
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let parent_model = self
            .parts
            .parent(part)
            .and_then(|parent| self.parts.get(parent))
            .and_then(|parent| parent.model_id());
        let host = PolicyHost::new(part, model_id, parent_model);
        self.parts.set_active(part, false)?;
        if let Some(roles) = self.policies.roles_mut(part) {
            for policy in roles.values_mut() {
                policy.deactivate(host, &self.model);
            }
        }
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .deactivate(&self.model, model_id);
        Ok(())
    }

    pub(super) fn create_subtree(
        &mut self,
        parent: EditPartId,
        model_id: A::ModelId,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<EditPartId, ViewerError> {
        if self.model_registry.contains_key(&model_id) {
            return Err(ViewerError::DuplicateModel);
        }
        let parent_node = self
            .parts
            .get(parent)
            .ok_or(ViewerError::InconsistentState)?;
        let parent_model = parent_node.model_id();
        let parent_figure = parent_node.content_pane();
        let context = PartFactoryContext::new(parent, parent_model, model_id);
        let mut behavior = self.factory.create(context, &self.model)?;
        let policies = behavior.create_policies(&self.model, model_id)?;
        let primary = self
            .runtime
            .container(parent_figure)?
            .add(behavior.create_figure(&self.model, model_id)?)?;
        let mut build = VisualBuildContext::new(&mut self.runtime, primary);
        if let Err(error) = behavior.configure_visual(&self.model, model_id, &mut build) {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(error.into());
        }
        let (content_pane, visuals) = build.finish();
        validate_runtime_namespace(self.runtime.tree().namespace(), primary)?;
        if visuals
            .iter()
            .any(|visual| self.visual_registry.contains_key(visual))
        {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(ViewerError::DuplicateVisual);
        }

        let part = self
            .parts
            .insert(parent, model_id, primary, content_pane, visuals.clone())?;
        self.model_registry.insert(model_id, part);
        for visual in visuals {
            self.visual_registry.insert(visual, VisualOwner::Part(part));
        }
        self.behaviors.insert(part, behavior)?;
        for (role, policy) in policies {
            self.policies.install(part, role, policy)?;
        }
        self.refresh_part_visuals(part)?;
        self.activate_part(part, model_id)?;

        for child in snapshot.children_of(model_id)?.iter().copied() {
            self.create_subtree(part, child, snapshot)?;
        }
        Ok(part)
    }

    fn build_connection_anchor(
        &mut self,
        endpoint: ConnectionEndpoint,
        endpoint_part: EditPartId,
        descriptor: ModelConnection<A::ModelId>,
        connection_figure: FigureId,
    ) -> Result<(AnchorSemanticKey, Box<dyn ConnectionAnchor>), ViewerError> {
        self.build_connection_anchor_for_models(
            endpoint,
            endpoint_part,
            Some(descriptor.id()),
            descriptor.source(),
            Some(descriptor.target()),
            Some(connection_figure),
        )
    }

    pub(super) fn build_connection_anchor_for_models(
        &mut self,
        endpoint: ConnectionEndpoint,
        endpoint_part: EditPartId,
        connection_model: Option<A::ModelId>,
        source_model: A::ModelId,
        target_model: Option<A::ModelId>,
        connection_figure: Option<FigureId>,
    ) -> Result<(AnchorSemanticKey, Box<dyn ConnectionAnchor>), ViewerError> {
        let endpoint_node = self
            .parts
            .get(endpoint_part)
            .ok_or(ViewerError::InconsistentState)?;
        let endpoint_model = endpoint_node
            .model_id()
            .ok_or(ViewerError::InconsistentState)?;
        let endpoint_figure = endpoint_node.primary_figure();
        let context = ConnectionAnchorContext::new(
            connection_model,
            source_model,
            target_model,
            connection_figure,
            endpoint_figure,
        );
        let behavior = self
            .behaviors
            .get_mut(endpoint_part)
            .ok_or(ViewerError::InconsistentState)?;
        let custom = match endpoint {
            ConnectionEndpoint::Source => {
                behavior.source_connection_anchor(&self.model, endpoint_model, context)?
            }
            ConnectionEndpoint::Target => {
                behavior.target_connection_anchor(&self.model, endpoint_model, context)?
            }
        };
        let descriptor = custom.unwrap_or_else(|| {
            let anchor = ChopboxAnchor::new(endpoint_figure);
            let key = anchor
                .semantic_group_key()
                .expect("ChopboxAnchor always provides semantic identity");
            ConnectionAnchorDescriptor::new(key, Box::new(anchor))
        });
        Ok(descriptor.into_parts())
    }

    fn create_connection_part(
        &mut self,
        descriptor: ModelConnection<A::ModelId>,
    ) -> Result<ConnectionPartId, ViewerError> {
        if self.model_registry.contains_key(&descriptor.id()) {
            return Err(ViewerError::DuplicateModel);
        }
        let source_part = self
            .model_registry
            .get(&descriptor.source())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.source()),
            })?;
        let target_part = self
            .model_registry
            .get(&descriptor.target())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.target()),
            })?;
        let context = ConnectionPartFactoryContext::new(
            self.root(),
            descriptor.id(),
            source_part,
            descriptor.source(),
            target_part,
            descriptor.target(),
        );
        let mut behavior = self.factory.create_connection(context, &self.model)?;
        let policies = behavior.create_policies(&self.model, descriptor.id())?;
        let bendpoints = behavior.connection_bendpoints(&self.model, descriptor.id())?;
        Self::validate_connection_bendpoints(descriptor.id(), &bendpoints)?;
        let routing = behavior.connection_routing(&self.model, descriptor.id())?;
        let (router, constraint) = self.resolve_connection_routing(routing)?;
        let primary = self
            .runtime
            .container(self.root_layers.connection())?
            .add(behavior.create_figure(&self.model, descriptor.id())?)?;
        if !self.runtime.tree().is_connection_figure(primary) {
            self.runtime.dispose_subtree(primary)?;
            return Err(ViewerError::InvalidConnectionFigure {
                connection: format!("{:?}", descriptor.id()),
            });
        }
        let mut build = VisualBuildContext::new(&mut self.runtime, primary);
        if let Err(error) = behavior.configure_visual(&self.model, descriptor.id(), &mut build) {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(error.into());
        }
        let (content_pane, visuals) = build.finish();
        if visuals
            .iter()
            .any(|visual| self.visual_registry.contains_key(visual))
        {
            self.runtime.dispose_subtree(primary)?;
            return Err(ViewerError::DuplicateVisual);
        }
        let (source_anchor_key, source_anchor_strategy) = match self.build_connection_anchor(
            ConnectionEndpoint::Source,
            source_part,
            descriptor,
            primary,
        ) {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error);
            }
        };
        let (target_anchor_key, target_anchor_strategy) = match self.build_connection_anchor(
            ConnectionEndpoint::Target,
            target_part,
            descriptor,
            primary,
        ) {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error);
            }
        };
        let source_anchor = self
            .runtime
            .try_register_connection_anchor(source_anchor_strategy)?;
        let target_anchor = match self
            .runtime
            .try_register_connection_anchor(target_anchor_strategy)
        {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.remove_connection_anchor(source_anchor);
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error.into());
            }
        };
        let connection = match self.runtime.register_connection_state(
            primary,
            Some(source_anchor),
            Some(target_anchor),
            router,
            constraint,
        ) {
            Ok(connection) => connection,
            Err(error) => {
                let _ = self.runtime.remove_connection_anchor(source_anchor);
                let _ = self.runtime.remove_connection_anchor(target_anchor);
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error.into());
            }
        };

        let part = self.parts.insert_connection(
            descriptor.id(),
            primary,
            content_pane,
            visuals.clone(),
            source_part,
            target_part,
        )?;
        self.model_registry
            .insert(descriptor.id(), part.edit_part());
        for visual in visuals {
            self.visual_registry
                .insert(visual, VisualOwner::Part(part.edit_part()));
        }
        self.behaviors.insert(part.edit_part(), behavior)?;
        self.connection_projections.insert(
            descriptor.id(),
            ConnectionProjection {
                part,
                connection,
                source_model: descriptor.source(),
                target_model: descriptor.target(),
                source_anchor: Some(source_anchor),
                target_anchor: Some(target_anchor),
                source_anchor_key: Some(source_anchor_key),
                target_anchor_key: Some(target_anchor_key),
                registered: true,
            },
        );
        let initialization = (|| {
            for (role, policy) in policies {
                self.policies.install(part.edit_part(), role, policy)?;
            }
            self.refresh_part_visuals(part.edit_part())?;
            self.activate_part(part.edit_part(), descriptor.id())?;
            self.resolve_connection(connection)
        })();
        if let Err(error) = initialization {
            self.remove_connection_part(part)?;
            return Err(error);
        }
        Ok(part)
    }

    fn resolve_connection(&mut self, connection: ConnectionId) -> Result<(), ViewerError> {
        match self.runtime.resolve_connection_route(
            connection,
            CoordinateSpace::ChildContent(self.root_layers.connection()),
        ) {
            Ok(_) | Err(ConnectionRuntimeError::Unresolved(_)) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn synchronize_connections(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let mut order = Vec::with_capacity(snapshot.connections.len());
        let mut figure_order = Vec::with_capacity(snapshot.connections.len());
        for descriptor in snapshot.connections.iter().copied() {
            let connection = match self
                .connection_projections
                .get(&descriptor.id())
                .map(|projection| projection.part)
            {
                Some(connection) => {
                    self.bind_connection_part(descriptor)?;
                    self.refresh_part_visuals(connection.edit_part())?;
                    connection
                }
                None => self.create_connection_part(descriptor)?,
            };
            let figure = self
                .parts
                .get(connection.edit_part())
                .ok_or(ViewerError::InconsistentState)?
                .primary_figure();
            order.push(connection);
            figure_order.push(figure);
        }
        self.runtime
            .container(self.root_layers.connection())?
            .set_child_order(&figure_order)?;
        self.parts.set_connection_order(order)?;
        Ok(())
    }

    fn bind_connection_part(
        &mut self,
        descriptor: ModelConnection<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let source_part = self
            .model_registry
            .get(&descriptor.source())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.source()),
            })?;
        let target_part = self
            .model_registry
            .get(&descriptor.target())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.target()),
            })?;
        let mut current = self
            .connection_projections
            .get(&descriptor.id())
            .cloned()
            .ok_or(ViewerError::InconsistentState)?;
        let behavior = self
            .behaviors
            .get_mut(current.part.edit_part())
            .ok_or(ViewerError::InconsistentState)?;
        let bendpoints = behavior.connection_bendpoints(&self.model, descriptor.id())?;
        Self::validate_connection_bendpoints(descriptor.id(), &bendpoints)?;
        let routing = behavior.connection_routing(&self.model, descriptor.id())?;
        let (router, constraint) = self.resolve_connection_routing(routing)?;
        let (source_anchor_key, source_anchor_strategy) = self.build_connection_anchor(
            ConnectionEndpoint::Source,
            source_part,
            descriptor,
            current.connection.figure(),
        )?;
        let (target_anchor_key, target_anchor_strategy) = self.build_connection_anchor(
            ConnectionEndpoint::Target,
            target_part,
            descriptor,
            current.connection.figure(),
        )?;
        let source_reusable = current.source_model == descriptor.source()
            && current.source_anchor.is_some()
            && current.source_anchor_key.as_ref() == Some(&source_anchor_key);
        let target_reusable = current.target_model == descriptor.target()
            && current.target_anchor.is_some()
            && current.target_anchor_key.as_ref() == Some(&target_anchor_key);
        if current.registered
            && source_reusable
            && target_reusable
            && self.parts.connection_endpoints(current.part)?
                == crate::ConnectionEndpoints::new(source_part, target_part)
        {
            if self.configure_connection_route(current.connection, router, constraint)? {
                self.resolve_connection(current.connection)?;
            }
            return Ok(());
        }
        if current.registered {
            self.detach_connection_binding(current.part, !source_reusable, !target_reusable)?;
            current = self
                .connection_projections
                .get(&descriptor.id())
                .cloned()
                .ok_or(ViewerError::InconsistentState)?;
        }

        let source_anchor = if source_reusable {
            current.source_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(source_anchor_strategy)
        })?;
        let target_anchor = if target_reusable {
            current.target_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(target_anchor_strategy)
        })?;
        self.parts
            .bind_connection(current.part, source_part, target_part)?;
        self.runtime.register_connection_state(
            current.connection.figure(),
            Some(source_anchor),
            Some(target_anchor),
            router,
            constraint,
        )?;
        let connection = current.connection;
        let projection = self
            .connection_projections
            .get_mut(&descriptor.id())
            .ok_or(ViewerError::InconsistentState)?;
        projection.source_model = descriptor.source();
        projection.target_model = descriptor.target();
        projection.source_anchor = Some(source_anchor);
        projection.target_anchor = Some(target_anchor);
        projection.source_anchor_key = Some(source_anchor_key);
        projection.target_anchor_key = Some(target_anchor_key);
        projection.registered = true;
        self.resolve_connection(connection)
    }

    fn resolve_connection_routing(
        &self,
        descriptor: ConnectionRoutingDescriptor,
    ) -> Result<(RouterBinding, Option<Box<dyn novadraw::RoutingConstraint>>), ViewerError> {
        let (selection, constraint) = descriptor.into_parts();
        let binding = match selection {
            ConnectionRouterSelection::Inherited => RouterBinding::Inherited {
                layer: self.root_layers.connection(),
            },
            ConnectionRouterSelection::Registered(key) => RouterBinding::Explicit {
                router: *self.connection_routers.get(&key).ok_or_else(|| {
                    EditPartError::operation(format!(
                        "unknown connection Router key {}",
                        key.as_str()
                    ))
                })?,
            },
        };
        Ok((binding, constraint))
    }

    fn validate_connection_bendpoints(
        model_id: A::ModelId,
        bendpoints: &[Point],
    ) -> Result<(), ViewerError> {
        if bendpoints
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            Err(ViewerError::InvalidConnectionBendpoint {
                connection: format!("{model_id:?}"),
            })
        } else {
            Ok(())
        }
    }

    fn configure_connection_route(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
        constraint: Option<Box<dyn novadraw::RoutingConstraint>>,
    ) -> Result<bool, ViewerError> {
        self.runtime
            .set_connection_route_configuration(connection, router, constraint)
            .map_err(Into::into)
    }

    fn parts_to_retire(
        &self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<HashSet<EditPartId>, ViewerError> {
        let mut retiring = HashSet::new();
        for part in self.parts.subtree_ids(self.contents())?.into_iter().skip(1) {
            if retiring.contains(&part) {
                continue;
            }
            let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let actual_parent = self
                .parts
                .parent(part)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            if !snapshot.contains_model(model_id)
                || snapshot.parents.get(&model_id).copied() != actual_parent
            {
                retiring.extend(self.parts.subtree_ids(part)?);
            }
        }
        Ok(retiring)
    }

    fn prepare_connections_for_containment_change(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
        retiring: &HashSet<EditPartId>,
    ) -> Result<(), ViewerError> {
        let current = self.parts.connection_parts().to_vec();
        for connection in current {
            let model_id = self
                .parts
                .get(connection.edit_part())
                .and_then(|node| node.model_id())
                .ok_or(ViewerError::InconsistentState)?;
            let Some(desired) = snapshot.connection(model_id) else {
                self.remove_connection_part(connection)?;
                continue;
            };
            let endpoints = self.parts.connection_endpoints(connection)?;
            let projection = self
                .connection_projections
                .get(&model_id)
                .ok_or(ViewerError::InconsistentState)?;
            let source_changes = projection.source_model != desired.source()
                || retiring.contains(&endpoints.source());
            let target_changes = projection.target_model != desired.target()
                || retiring.contains(&endpoints.target());
            if source_changes || target_changes {
                self.detach_connection_binding(connection, source_changes, target_changes)?;
            }
        }
        Ok(())
    }

    fn detach_connection_binding(
        &mut self,
        connection: ConnectionPartId,
        remove_source: bool,
        remove_target: bool,
    ) -> Result<(), ViewerError> {
        let model_id = self
            .parts
            .get(connection.edit_part())
            .and_then(|node| node.model_id())
            .ok_or(ViewerError::InconsistentState)?;
        let projection = self
            .connection_projections
            .get(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        if projection.registered {
            self.runtime
                .remove_connection_state(projection.connection)?;
        }
        let source_anchor = remove_source.then_some(projection.source_anchor).flatten();
        let target_anchor = remove_target.then_some(projection.target_anchor).flatten();
        if let Some(anchor) = source_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        if let Some(anchor) = target_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        self.parts.unbind_connection(connection)?;
        let projection = self
            .connection_projections
            .get_mut(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        projection.registered = false;
        if remove_source {
            projection.source_anchor = None;
            projection.source_anchor_key = None;
        }
        if remove_target {
            projection.target_anchor = None;
            projection.target_anchor_key = None;
        }
        Ok(())
    }

    fn remove_connection_part(&mut self, connection: ConnectionPartId) -> Result<(), ViewerError> {
        let part = connection.edit_part();
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let primary = node.primary_figure();
        let visuals = node.visuals().to_vec();
        self.selection.reconcile(|selected| selected != part);
        let overlays: Vec<_> = self
            .visual_registry
            .iter()
            .filter_map(|(figure, owner)| match owner {
                VisualOwner::Handle { owner, .. } if *owner == part => Some(*figure),
                VisualOwner::Feedback {
                    owner: Some(owner), ..
                } if *owner == part => Some(*figure),
                _ => None,
            })
            .collect();
        for overlay in overlays {
            if self.runtime.tree().is_attached(overlay) {
                self.runtime.dispose_subtree(overlay)?;
            }
            self.visual_registry.remove(&overlay);
        }
        self.deactivate_part(part)?;
        let projection = self
            .connection_projections
            .remove(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        if projection.registered {
            self.runtime
                .remove_connection_state(projection.connection)?;
        }
        if let Some(anchor) = projection.source_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        if let Some(anchor) = projection.target_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        self.parts.unbind_connection(connection)?;
        self.model_registry.remove(&model_id);
        for visual in visuals {
            self.visual_registry.remove(&visual);
        }
        self.policies.remove(part);
        self.behaviors.remove(part);
        self.runtime.dispose_subtree(primary)?;
        self.parts.retire_connection(connection)?;
        Ok(())
    }

    fn synchronize_subtree(
        &mut self,
        part: EditPartId,
        model_id: A::ModelId,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        self.refresh_part_visuals(part)?;
        let desired = snapshot.children_of(model_id)?.to_vec();
        let existing = self
            .parts
            .children(part)
            .ok_or(ViewerError::InconsistentState)?
            .to_vec();

        for child in existing.iter().copied() {
            let child_model = self
                .parts
                .get(child)
                .and_then(|node| node.model_id())
                .ok_or(ViewerError::InconsistentState)?;
            if !desired.contains(&child_model) {
                self.remove_subtree(child)?;
            }
        }

        for (index, child_model) in desired.iter().copied().enumerate() {
            let child = match self.model_registry.get(&child_model).copied() {
                Some(existing) if self.parts.parent(existing) == Some(part) => existing,
                Some(_) => return Err(ViewerError::DuplicateModel),
                None => self.create_subtree(part, child_model, snapshot)?,
            };
            let primary = self
                .parts
                .get(child)
                .ok_or(ViewerError::InconsistentState)?
                .primary_figure();
            let content_pane = self
                .parts
                .get(part)
                .ok_or(ViewerError::InconsistentState)?
                .content_pane();
            self.runtime
                .container(content_pane)?
                .move_child_to_index(primary, index)?;
            self.parts.reorder_child(part, child, index)?;
            self.synchronize_subtree(child, child_model, snapshot)?;
        }
        Ok(())
    }

    fn remove_reparented_subtrees(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let ids = self.parts.subtree_ids(self.contents())?;
        for part in ids.into_iter().skip(1) {
            let Some(node) = self.parts.get(part) else {
                continue;
            };
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let actual_parent = self
                .parts
                .parent(part)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            if snapshot.parents.get(&model_id).copied() != actual_parent {
                self.remove_subtree(part)?;
            }
        }
        Ok(())
    }

    fn refresh_part_visuals(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let mut context = VisualUpdateContext::new(
            &mut self.runtime,
            node.primary_figure(),
            node.content_pane(),
            node.visuals(),
        );
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .refresh_visuals(&self.model, model_id, &mut context)?;
        Ok(())
    }

    fn remove_subtree(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let ids = self.parts.subtree_ids(part)?;
        if self
            .direct_text_edit
            .as_ref()
            .is_some_and(|active| ids.contains(&active.state.source()))
        {
            self.cancel_direct_text_edit()?;
        }
        let primary = self
            .parts
            .get(part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        self.selection
            .reconcile(|selected| !ids.contains(&selected));
        let overlays: Vec<_> = self
            .visual_registry
            .iter()
            .filter_map(|(figure, owner)| match owner {
                VisualOwner::Handle { owner, .. } if ids.contains(owner) => Some(*figure),
                VisualOwner::Feedback {
                    owner: Some(owner), ..
                } if ids.contains(owner) => Some(*figure),
                _ => None,
            })
            .collect();
        for overlay in overlays {
            if self.runtime.tree().is_attached(overlay) {
                self.runtime.dispose_subtree(overlay)?;
            }
            self.visual_registry.remove(&overlay);
        }

        for id in ids.iter().copied() {
            let node = self.parts.get(id).ok_or(ViewerError::InconsistentState)?;
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let visuals = node.visuals().to_vec();
            self.deactivate_part(id)?;
            self.model_registry.remove(&model_id);
            for visual in visuals {
                self.visual_registry.remove(&visual);
            }
        }
        for id in ids.iter().rev().copied() {
            self.policies.remove(id);
            self.behaviors.remove(id);
        }
        self.runtime.dispose_subtree(primary)?;
        self.parts.retire_subtree(part)?;
        Ok(())
    }
}

impl<A, F> Drop for GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    fn drop(&mut self) {
        for connection in self.parts.connection_parts().to_vec() {
            let part = connection.edit_part();
            let Some(node) = self.parts.get(part) else {
                continue;
            };
            let Some(model_id) = (if node.is_active() {
                node.model_id()
            } else {
                None
            }) else {
                continue;
            };
            let host = PolicyHost::new(part, model_id, None);
            if let Some(roles) = self.policies.roles_mut(part) {
                for policy in roles.values_mut() {
                    policy.deactivate(host, &self.model);
                }
            }
            if let Some(behavior) = self.behaviors.get_mut(part) {
                behavior.deactivate(&self.model, model_id);
            }
        }
        let Ok(ids) = self.parts.subtree_ids(self.parts.root()) else {
            return;
        };
        for id in ids.into_iter().skip(1) {
            let Some(node) = self.parts.get(id) else {
                continue;
            };
            let Some(model_id) = (if node.is_active() {
                node.model_id()
            } else {
                None
            }) else {
                continue;
            };
            let parent_model = self
                .parts
                .parent(id)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            let host = PolicyHost::new(id, model_id, parent_model);
            if let Some(roles) = self.policies.roles_mut(id) {
                for policy in roles.values_mut() {
                    policy.deactivate(host, &self.model);
                }
            }
            if let Some(behavior) = self.behaviors.get_mut(id) {
                behavior.deactivate(&self.model, model_id);
            }
        }
    }
}
