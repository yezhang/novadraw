use super::{
    BackendCapabilities, BackendSessionId, ConnectionRuntimeError, ConnectionStateSnapshot,
    CoordinateSpace, DERIVED_STATE_FEEDBACK_LIMIT, DERIVED_WORK_KIND_COUNT, DamageMode,
    DerivedWorkKind, DerivedWorkSet, FigureTreeSceneRead, FrameId, FramePreparation,
    FramePreparationError, InFlightFrame, NdCanvas, RenderOutcome, RenderSubmission, ResourceSync,
    Runtime, SurfaceInfo, UpdateEvent,
};

impl Runtime {
    /// Converges pending derived state without recording or consuming a render frame.
    ///
    /// Hosts use this boundary when overlay geometry must be rebuilt from the latest layout
    /// before the next renderer submission is frozen.
    pub fn stabilize_for_query(&mut self) -> Result<(), FramePreparationError> {
        if self.faulted {
            return Err(FramePreparationError::Faulted);
        }
        self.guarded(|runtime| {
            runtime.apply_pending_mutations_for_frame();
            runtime.try_stabilize()
        })
    }

    fn apply_pending_mutations_for_frame(&mut self) {
        let mutations = self.mutations.drain();
        if !mutations.is_empty() {
            self.apply_runtime_mutations(mutations);
            self.retain_interactive_figures();
        }
    }

    fn invalidate_stale_connection_dependencies(&mut self) -> Result<bool, ConnectionRuntimeError> {
        let scene = FigureTreeSceneRead::new(&self.tree, &self.anchor_geometries);
        self.connections
            .invalidate_stale_dependencies(&scene)
            .map(|affected| !affected.is_empty())
    }

    fn resolve_dirty_connection_routes(&mut self) -> bool {
        let dirty = self.connections.dirty_connections();
        if dirty.is_empty() {
            return false;
        }
        for connection in dirty {
            if !matches!(
                self.connections.state(connection),
                Ok(ConnectionStateSnapshot {
                    resolution: crate::ConnectionResolution::Dirty { .. },
                    ..
                })
            ) {
                continue;
            }
            let Some(parent) = self.tree.parent_id(connection.figure()) else {
                self.connection_error =
                    Some(ConnectionRuntimeError::UnknownFigure(connection.figure()));
                continue;
            };
            if let Err(error) =
                self.resolve_connection_route(connection, CoordinateSpace::ChildContent(parent))
            {
                self.connection_error = Some(error);
            }
        }
        true
    }

    fn stabilize(&mut self) -> Result<(), FramePreparationError> {
        self.derivation_epoch = self.derivation_epoch.wrapping_add(1);
        self.last_stabilization_error = None;

        let mut work = DerivedWorkSet::default();
        let mut feedback_counts = [0_usize; DERIVED_WORK_KIND_COUNT];
        work.insert(DerivedWorkKind::IntrinsicMetrics);
        work.insert(DerivedWorkKind::Layout);
        work.insert(DerivedWorkKind::DependencyInvalidation);
        work.insert(DerivedWorkKind::Routing);
        work.insert(DerivedWorkKind::Presentation);

        while let Some(kind) = work.pop_next() {
            let count = &mut feedback_counts[kind as usize];
            *count += 1;
            if *count > DERIVED_STATE_FEEDBACK_LIMIT {
                return Err(FramePreparationError::DidNotConverge);
            }

            match kind {
                DerivedWorkKind::IntrinsicMetrics => {
                    self.refresh_image_figures();
                    self.refresh_owner_scoped_borders()
                        .map_err(FramePreparationError::Text)?;
                    self.refresh_label_intrinsic_metrics()
                        .map_err(FramePreparationError::Text)?;
                    self.refresh_text_flow_layouts()
                        .map_err(FramePreparationError::Text)?;
                    self.tree
                        .refresh_prepared_figures(self.text.as_mut(), &mut self.updates)
                        .map_err(FramePreparationError::Text)?;
                    if self.updates.has_pending_layout() {
                        work.insert(DerivedWorkKind::Layout);
                    }
                }
                DerivedWorkKind::Layout => {
                    if self.updates.has_pending_layout() {
                        self.updates
                            .perform_validation_phase(&mut self.tree)
                            .map_err(FramePreparationError::Validation)?;
                        work.insert(DerivedWorkKind::DependencyInvalidation);
                        work.insert(DerivedWorkKind::Presentation);
                    }
                }
                DerivedWorkKind::DependencyInvalidation => {
                    if self
                        .invalidate_stale_connection_dependencies()
                        .map_err(|error| {
                            self.connection_error = Some(error);
                            FramePreparationError::DidNotConverge
                        })?
                        || !self.connections.dirty_connections().is_empty()
                    {
                        work.insert(DerivedWorkKind::Routing);
                    }
                }
                DerivedWorkKind::Routing => {
                    if self.resolve_dirty_connection_routes() {
                        work.insert(DerivedWorkKind::PostRouteGeometry);
                    }
                }
                DerivedWorkKind::PostRouteGeometry => {
                    if self.updates.has_pending_layout() {
                        work.insert(DerivedWorkKind::Layout);
                    }
                    if !self.connections.dirty_connections().is_empty() {
                        work.insert(DerivedWorkKind::Routing);
                    }
                    work.insert(DerivedWorkKind::Presentation);
                }
                DerivedWorkKind::Presentation => {
                    if self
                        .tree
                        .refresh_prepared_figures(self.text.as_mut(), &mut self.updates)
                        .map_err(FramePreparationError::Text)?
                    {
                        work.insert(DerivedWorkKind::Layout);
                    }
                    if self
                        .refresh_text_flow_layouts()
                        .map_err(FramePreparationError::Text)?
                    {
                        work.insert(DerivedWorkKind::Layout);
                    }
                    if self
                        .refresh_label_presentations()
                        .map_err(FramePreparationError::Text)?
                    {
                        work.insert(DerivedWorkKind::DependencyInvalidation);
                    }
                }
            }
        }

        if self.updates.has_pending_layout() || !self.connections.dirty_connections().is_empty() {
            return Err(FramePreparationError::DidNotConverge);
        }
        self.stable_epoch = self.derivation_epoch;
        Ok(())
    }

    fn try_stabilize(&mut self) -> Result<(), FramePreparationError> {
        match self.stabilize() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.last_stabilization_error = Some(error.clone());
                self.full_redraw_pending = true;
                Err(error)
            }
        }
    }

    /// Produces one complete renderer submission at a stable transaction boundary.
    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> Option<RenderSubmission> {
        match self.prepare_submission_state(surface, capabilities) {
            FramePreparation::Ready(submission) => Some(submission),
            FramePreparation::Idle
            | FramePreparation::Suspended
            | FramePreparation::AwaitingCompletion
            | FramePreparation::Error(_) => None,
        }
    }

    pub fn prepare_submission_state(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        if self.faulted {
            return FramePreparation::Error(FramePreparationError::Faulted);
        }
        self.guarded(|runtime| runtime.prepare_submission_state_inner(surface, capabilities))
    }

    fn prepare_submission_state_inner(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        if self.in_flight.is_some() {
            return FramePreparation::AwaitingCompletion;
        }

        self.apply_pending_mutations_for_frame();

        let surface_changed = self.last_surface != Some(surface);
        self.last_surface = Some(surface);
        if surface_changed {
            self.full_redraw_pending = true;
        }
        if !surface.is_renderable() {
            self.full_redraw_pending = true;
            return FramePreparation::Suspended;
        }
        if let Err(error) = self.try_stabilize() {
            return FramePreparation::Error(error);
        }
        if let Err(error) =
            self.accessibility
                .publish(&self.tree, &self.interaction, self.stable_epoch, surface)
        {
            return FramePreparation::Error(FramePreparationError::Accessibility(error));
        }
        self.updates.set_publication_epoch(self.stable_epoch);

        let has_resource_delta = self.resources.has_pending_delta();
        let force_full_frame = self.full_redraw_pending || self.session_sync_pending;
        let mut canvas = if self.updates.is_update_queued() {
            self.tree.perform_update(&mut self.updates)
        } else if force_full_frame {
            self.tree.render()
        } else if has_resource_delta {
            NdCanvas::new()
        } else {
            self.updates
                .flush_notifications_at(&mut self.tree, self.stable_epoch);
            return FramePreparation::Idle;
        };
        if let Some(error) = self.updates.last_validation_error().cloned() {
            self.full_redraw_pending = true;
            return FramePreparation::Error(FramePreparationError::Validation(error));
        }
        let promote_partial = canvas.damage().mode() == DamageMode::Partial
            && !capabilities.supports_partial_damage();
        if force_full_frame || promote_partial {
            if canvas.damage().mode() == DamageMode::None {
                canvas = self.tree.render();
            } else {
                canvas.damage_mut().set_full();
            }
        }
        if let Err(error) = canvas.validate_recording(&self.resources) {
            self.full_redraw_pending = true;
            return FramePreparation::Error(FramePreparationError::Graphics(error));
        }
        if let Err(error) = capabilities.validate_capabilities(canvas.commands()) {
            self.full_redraw_pending = true;
            return FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(
                error,
            ));
        }

        if let Err(error) =
            crate::render::validate_graphics_input(canvas.commands(), surface.scale_factor)
        {
            self.full_redraw_pending = true;
            return FramePreparation::Error(FramePreparationError::InvalidGraphicsInput(error));
        }

        let frame_id = self.next_frame_id;
        self.next_frame_id = self.next_frame_id.next();
        let resources = if self.session_sync_pending {
            ResourceSync::Snapshot(self.resources.take_ready_snapshot())
        } else {
            ResourceSync::Delta(self.resources.take_delta())
        };
        let submission = canvas.to_submission_for_session(
            surface,
            resources.clone(),
            self.backend_session_id,
            frame_id,
        );
        self.full_redraw_pending = false;
        self.in_flight = Some(InFlightFrame {
            session_id: self.backend_session_id,
            id: frame_id,
            resources,
        });
        self.updates.emit_update_event(UpdateEvent::Prepared {
            frame_id,
            damage: submission.damage.mode(),
        });
        self.updates
            .flush_notifications_at(&mut self.tree, self.stable_epoch);
        FramePreparation::Ready(submission)
    }

    /// Completes the in-flight frame and restores work when presentation failed.
    pub fn complete_submission(
        &mut self,
        session_id: BackendSessionId,
        frame_id: FrameId,
        outcome: RenderOutcome,
    ) -> bool {
        if self.faulted {
            return false;
        }
        self.guarded(|runtime| runtime.complete_submission_inner(session_id, frame_id, outcome))
    }

    fn complete_submission_inner(
        &mut self,
        session_id: BackendSessionId,
        frame_id: FrameId,
        outcome: RenderOutcome,
    ) -> bool {
        let Some(in_flight) = self.in_flight.take() else {
            return false;
        };
        if in_flight.session_id != session_id || in_flight.id != frame_id {
            self.in_flight = Some(in_flight);
            return false;
        }

        match (outcome, in_flight.resources) {
            (RenderOutcome::Presented, ResourceSync::Snapshot(_)) => {
                self.session_sync_pending = false;
            }
            (RenderOutcome::Presented, ResourceSync::Delta(_)) => {}
            (_, ResourceSync::Delta(delta)) => {
                self.full_redraw_pending = true;
                self.resources.restore_delta(delta);
            }
            (_, ResourceSync::Snapshot(_)) => {
                self.full_redraw_pending = true;
                self.session_sync_pending = true;
            }
        }
        self.updates
            .emit_update_event(UpdateEvent::Submitted { frame_id, outcome });
        self.updates
            .flush_notifications_at(&mut self.tree, self.stable_epoch);
        true
    }

    /// Prepares an incremental frame when the runtime has pending work.
    pub fn prepare_frame(&mut self) -> Option<NdCanvas> {
        if self.faulted {
            return None;
        }
        self.guarded(Self::prepare_frame_inner)
    }

    fn prepare_frame_inner(&mut self) -> Option<NdCanvas> {
        self.apply_pending_mutations_for_frame();
        if self.try_stabilize().is_err() {
            return None;
        }
        self.updates.set_publication_epoch(self.stable_epoch);
        let frame = if self.updates.is_update_queued() {
            let incremental = self.tree.perform_update(&mut self.updates);
            if std::mem::take(&mut self.full_redraw_pending) {
                Some(self.tree.render())
            } else {
                Some(incremental)
            }
        } else if std::mem::take(&mut self.full_redraw_pending) {
            Some(self.tree.render())
        } else {
            None
        };
        self.updates
            .flush_notifications_at(&mut self.tree, self.stable_epoch);
        frame
    }

    /// Records the complete visible tree, independent of pending update state.
    pub fn record_full_frame(&mut self) -> NdCanvas {
        assert!(!self.faulted, "cannot record a faulted Runtime");
        self.guarded(Self::record_full_frame_inner)
    }

    fn record_full_frame_inner(&mut self) -> NdCanvas {
        self.apply_pending_mutations_for_frame();
        self.stabilize()
            .expect("full-frame recording requires stable derived state");
        self.updates.set_publication_epoch(self.stable_epoch);
        let frame = self.tree.render();
        self.updates
            .flush_notifications_at(&mut self.tree, self.stable_epoch);
        frame
    }
}
