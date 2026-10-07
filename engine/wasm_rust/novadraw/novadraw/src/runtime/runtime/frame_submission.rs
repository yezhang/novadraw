use std::collections::HashSet;

use super::{
    BackendCapabilities, BackendSessionId, ConnectionRuntimeError, ConnectionStateSnapshot,
    CoordinateSpace, DERIVED_STATE_FEEDBACK_LIMIT, DERIVED_WORK_KIND_COUNT, DamageMode,
    DerivedWorkKind, DerivedWorkSet, FigureTreeSceneRead, FrameId, FramePreparation,
    FramePreparationError, InFlightFrame, NdCanvas, RenderOutcome, ResourceSync, Runtime,
    SurfaceInfo, UpdateEvent,
};
use crate::FigureId;
use crate::animation::{
    AnimationChannel, AnimationPlan, AnimationStart, AnimationSuppression,
    AnimationTransactionError, AnimationValue, BoundsTransition, ConnectionRouteTransition,
    FigureTransitionCapture, InteractionGeometryPolicy, Motion, Opacity, Tween, ViewportTransition,
};
use crate::{Affine2D, PointList, Rectangle, ViewportHandle, figure::FigurePresentation};

struct CapturedConnectionRoute {
    figure: FigureId,
    bounds: Rectangle,
    points: PointList,
    local_to_surface: Affine2D,
    presentation: FigurePresentation,
    channel: AnimationChannel<PointList>,
}
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
            runtime.try_stabilize()?;
            runtime.process_animation_behaviors();
            Ok(())
        })
    }

    /// Runs a source transaction between stable before/after bounds snapshots.
    ///
    /// The source mutation commits before presentation admission and is never rolled back.
    /// No render submission can observe the final committed layout before the transition
    /// override is installed.
    pub fn transition_bounds_transaction<I, F, T, E>(
        &mut self,
        figures: I,
        transition: BoundsTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        if self.faulted {
            return Err(AnimationTransactionError::BeforeStabilization(
                FramePreparationError::Faulted,
            ));
        }
        self.guarded(|runtime| {
            runtime.transition_bounds_transaction_inner(figures, transition, mutation)
        })
    }

    /// Runs a pan, zoom, or fit source transaction with a presentation-only transition.
    ///
    /// The Viewport and its contents commit their final geometry before the plan starts. The
    /// transition is applied to the contents Figure, so the committed Viewport clip remains
    /// authoritative throughout the animation.
    pub fn transition_viewport_transaction<F, T, E>(
        &mut self,
        viewport: &ViewportHandle,
        transition: ViewportTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        if self.faulted {
            return Err(AnimationTransactionError::BeforeStabilization(
                FramePreparationError::Faulted,
            ));
        }
        let viewport = viewport.clone();
        self.guarded(|runtime| {
            runtime.transition_viewport_transaction_inner(&viewport, transition, mutation)
        })
    }

    /// Runs a source transaction between stable Connection route snapshots.
    ///
    /// Routes with equal point topology interpolate in the final Figure-local domain. Other
    /// route changes crossfade an immutable old-route visual with the committed final Figure.
    pub fn transition_connection_routes_transaction<I, F, T, E>(
        &mut self,
        connections: I,
        transition: ConnectionRouteTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        if self.faulted {
            return Err(AnimationTransactionError::BeforeStabilization(
                FramePreparationError::Faulted,
            ));
        }
        self.guarded(|runtime| {
            runtime.transition_connection_routes_transaction_inner(
                connections,
                transition,
                mutation,
            )
        })
    }

    fn transition_bounds_transaction_inner<I, F, T, E>(
        &mut self,
        figures: I,
        transition: BoundsTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::BeforeStabilization)?;
        let capture: FigureTransitionCapture = self
            .animations()
            .capture_figures(figures)
            .map_err(AnimationTransactionError::Capture)?;
        let value = mutation(self).map_err(AnimationTransactionError::Mutation)?;
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::AfterStabilization)?;
        let start = self
            .animations()
            .transition_bounds(capture, transition)
            .map_err(AnimationTransactionError::Transition)?;
        Ok((value, start))
    }

    fn transition_viewport_transaction_inner<F, T, E>(
        &mut self,
        viewport: &ViewportHandle,
        transition: ViewportTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::BeforeStabilization)?;
        let (contents, before_prefix, before_suffix) =
            self.viewport_transition_factors(viewport)
                .map_err(AnimationTransactionError::Capture)?;
        let channels = self
            .animations()
            .bind_figure(contents, InteractionGeometryPolicy::Committed)
            .map_err(AnimationTransactionError::Capture)?;
        let before_presentation = self
            .animations()
            .value(channels.transform())
            .map_err(AnimationTransactionError::Capture)?;
        let before_mapping = before_prefix * before_presentation * before_suffix;

        let value = mutation(self).map_err(AnimationTransactionError::Mutation)?;
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::AfterStabilization)?;
        let (after_contents, after_prefix, after_suffix) = self
            .viewport_transition_factors(viewport)
            .map_err(AnimationTransactionError::Transition)?;
        if contents != after_contents {
            return Err(AnimationTransactionError::Transition(
                crate::animation::AnimationError::ViewportContentsChanged,
            ));
        }
        let start_transform = after_prefix
            .inverse()
            .and_then(|inverse_prefix| {
                after_suffix
                    .inverse()
                    .map(|inverse_suffix| inverse_prefix * before_mapping * inverse_suffix)
            })
            .filter(Affine2D::is_valid)
            .ok_or(AnimationTransactionError::Transition(
                crate::animation::AnimationError::IncompatibleViewportTransition,
            ))?;
        if transition.duration.is_zero() || start_transform == Affine2D::IDENTITY {
            return Ok((
                value,
                AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta),
            ));
        }
        let plan = AnimationPlan::track(
            channels.transform(),
            Motion::Tween(
                Tween::between(start_transform, Affine2D::IDENTITY, transition.duration)
                    .map_err(AnimationTransactionError::Transition)?
                    .with_easing(transition.easing),
            ),
        )
        .map_err(AnimationTransactionError::Transition)?
        .with_interruption(transition.interruption)
        .with_suspension(transition.suspension);
        let start = self
            .animations()
            .start(plan)
            .map_err(AnimationTransactionError::Transition)?;
        Ok((value, start))
    }

    fn transition_connection_routes_transaction_inner<I, F, T, E>(
        &mut self,
        connections: I,
        transition: ConnectionRouteTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::BeforeStabilization)?;
        let mut seen = HashSet::new();
        let mut captured = Vec::new();
        for figure in connections {
            if !seen.insert(figure) {
                return Err(AnimationTransactionError::Capture(
                    crate::animation::AnimationError::DuplicateTarget,
                ));
            }
            let bounds =
                self.tree
                    .figure_bounds(figure)
                    .ok_or(AnimationTransactionError::Capture(
                        crate::animation::AnimationError::DisposedTarget,
                    ))?;
            let local_to_surface = self.tree.local_to_surface_transform(figure).ok_or(
                AnimationTransactionError::Capture(
                    crate::animation::AnimationError::DisposedTarget,
                ),
            )?;
            let channel = self
                .animations()
                .bind_connection_route(figure)
                .map_err(AnimationTransactionError::Capture)?;
            let points = self
                .animations()
                .value(channel)
                .map_err(AnimationTransactionError::Capture)?;
            let presentation = self
                .tree
                .node(figure)
                .and_then(|node| node.figure.connection())
                .and_then(|connection| connection.capture_route_presentation(&points, bounds))
                .ok_or(AnimationTransactionError::Capture(
                    crate::animation::AnimationError::UnsupportedRoutePresentation,
                ))?;
            captured.push(CapturedConnectionRoute {
                figure,
                bounds,
                points,
                local_to_surface,
                presentation,
                channel,
            });
        }
        if captured.is_empty() {
            return Err(AnimationTransactionError::Capture(
                crate::animation::AnimationError::EmptyCapture,
            ));
        }

        let value = mutation(self).map_err(AnimationTransactionError::Mutation)?;
        self.stabilize_animation_transaction()
            .map_err(AnimationTransactionError::AfterStabilization)?;
        if transition.duration.is_zero() {
            for capture in &captured {
                let route = self
                    .tree
                    .connection_route_points(capture.figure)
                    .cloned()
                    .ok_or(AnimationTransactionError::Transition(
                        crate::animation::AnimationError::DisposedTarget,
                    ))?;
                self.animations()
                    .set_connection_route_committed(capture.channel, route)
                    .map_err(AnimationTransactionError::Transition)?;
            }
            return Ok((
                value,
                AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta),
            ));
        }

        let mut plans = Vec::new();
        let mut temporary_visuals = Vec::new();
        let build_result = (|| {
            for capture in captured {
                let after_points = self
                    .tree
                    .connection_route_points(capture.figure)
                    .cloned()
                    .ok_or(crate::animation::AnimationError::DisposedTarget)?;
                let after_to_surface = self
                    .tree
                    .local_to_surface_transform(capture.figure)
                    .ok_or(crate::animation::AnimationError::DisposedTarget)?;
                self.animations()
                    .set_connection_route_committed(capture.channel, after_points.clone())?;
                if capture.points == after_points
                    && capture.bounds
                        == self
                            .tree
                            .figure_bounds(capture.figure)
                            .unwrap_or(capture.bounds)
                {
                    continue;
                }
                if capture.points.len() == after_points.len() {
                    let surface_to_after = after_to_surface
                        .inverse()
                        .ok_or(crate::animation::AnimationError::IncompatibleRouteTransition)?;
                    let projected_before = capture
                        .points
                        .transformed(surface_to_after * capture.local_to_surface);
                    plans.push(AnimationPlan::track(
                        capture.channel,
                        Motion::Tween(
                            Tween::between(projected_before, after_points, transition.duration)?
                                .with_easing(transition.easing),
                        ),
                    )?);
                    continue;
                }

                let old_visual = self.animations().create_temporary_visual(
                    capture.presentation,
                    capture.local_to_surface,
                    0.0,
                )?;
                temporary_visuals.push(old_visual);
                let figure_opacity = self
                    .animations()
                    .bind_figure(capture.figure, InteractionGeometryPolicy::Committed)?
                    .opacity();
                let final_opacity = self.animations().value(figure_opacity)?;
                plans.push(AnimationPlan::parallel(vec![
                    AnimationPlan::track(
                        old_visual.opacity(),
                        Motion::Tween(
                            Tween::between(
                                Opacity::OPAQUE,
                                Opacity::TRANSPARENT,
                                transition.duration,
                            )?
                            .with_easing(transition.easing),
                        ),
                    )?,
                    AnimationPlan::track(
                        figure_opacity,
                        Motion::Tween(
                            Tween::between(
                                Opacity::TRANSPARENT,
                                final_opacity,
                                transition.duration,
                            )?
                            .with_easing(transition.easing),
                        ),
                    )?,
                ])?);
            }
            if plans.is_empty() {
                return Ok(AnimationStart::Suppressed(
                    AnimationSuppression::NoVisualDelta,
                ));
            }
            let plan = AnimationPlan::parallel(plans)?
                .with_interruption(transition.interruption)
                .with_suspension(transition.suspension);
            self.animations().start(plan)
        })();

        let start = match build_result {
            Ok(start @ AnimationStart::Running(_)) => start,
            Ok(start) => {
                for visual in temporary_visuals {
                    let _ = self.animations().remove_temporary_visual(visual.id());
                }
                start
            }
            Err(error) => {
                for visual in temporary_visuals {
                    let _ = self.animations().remove_temporary_visual(visual.id());
                }
                return Err(AnimationTransactionError::Transition(error));
            }
        };
        Ok((value, start))
    }

    fn viewport_transition_factors(
        &self,
        viewport: &ViewportHandle,
    ) -> Result<(FigureId, Affine2D, Affine2D), crate::animation::AnimationError> {
        let viewport_id = viewport.figure_id();
        if viewport_id.namespace() != self.tree.namespace() {
            return Err(crate::animation::AnimationError::ForeignTarget);
        }
        let tree_viewport = self
            .tree
            .viewport_handle(viewport_id)
            .ok_or(crate::animation::AnimationError::DisposedTarget)?;
        let contents = tree_viewport
            .contents(&self.tree)
            .ok_or(crate::animation::AnimationError::MissingViewportContents)?;
        let viewport_node = self
            .tree
            .node(viewport_id)
            .ok_or(crate::animation::AnimationError::DisposedTarget)?;
        let contents_node = self
            .tree
            .node(contents)
            .ok_or(crate::animation::AnimationError::DisposedTarget)?;
        let bounds = contents_node.figure_bounds();
        let prefix = viewport_node.child_transform().affine()
            * Affine2D::from_translation(bounds.x, bounds.y);
        let suffix = contents_node.child_transform().affine();
        Ok((contents, prefix, suffix))
    }

    fn stabilize_animation_transaction(&mut self) -> Result<(), FramePreparationError> {
        self.apply_pending_mutations_for_frame();
        self.try_stabilize()?;
        self.process_animation_behaviors();
        Ok(())
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

    fn process_animation_behaviors(&mut self) {
        let effects = self.updates.take_animation_effects(&mut self.tree);
        self.animations
            .access_mut(
                &self.tree,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .process_behaviors(&effects, self.stable_epoch);
    }

    /// Produces one complete renderer submission at a stable transaction boundary.
    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        if self.faulted {
            return FramePreparation::Error(FramePreparationError::Faulted);
        }
        self.guarded(|runtime| runtime.prepare_submission_inner(surface, capabilities))
    }

    fn prepare_submission_inner(
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
            self.animations
                .access_mut(
                    &self.tree,
                    &mut self.updates,
                    &mut self.full_redraw_pending,
                    &mut self.faulted,
                )
                .set_surface_suspended(true);
            self.full_redraw_pending = true;
            return FramePreparation::Suspended;
        }
        self.animations
            .access_mut(
                &self.tree,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .set_surface_suspended(false);
        if let Err(error) = self.try_stabilize() {
            return FramePreparation::Error(error);
        }
        self.process_animation_behaviors();
        self.animations
            .access_mut(
                &self.tree,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .reconcile_visibility();
        let presentation = self.animations.snapshot();
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
            self.tree
                .perform_update_with_presentation(&mut self.updates, presentation.as_ref())
        } else if force_full_frame {
            self.tree.render_with_presentation(presentation.as_ref())
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
                canvas = self.tree.render_with_presentation(presentation.as_ref());
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
        self.process_animation_behaviors();
        self.animations
            .access_mut(
                &self.tree,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .reconcile_visibility();
        let presentation = self.animations.snapshot();
        self.updates.set_publication_epoch(self.stable_epoch);
        let frame = if self.updates.is_update_queued() {
            let incremental = self
                .tree
                .perform_update_with_presentation(&mut self.updates, presentation.as_ref());
            if std::mem::take(&mut self.full_redraw_pending) {
                Some(self.tree.render_with_presentation(presentation.as_ref()))
            } else {
                Some(incremental)
            }
        } else if std::mem::take(&mut self.full_redraw_pending) {
            Some(self.tree.render_with_presentation(presentation.as_ref()))
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
        self.process_animation_behaviors();
        self.animations
            .access_mut(
                &self.tree,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .reconcile_visibility();
        let presentation = self.animations.snapshot();
        self.updates.set_publication_epoch(self.stable_epoch);
        let frame = self.tree.render_with_presentation(presentation.as_ref());
        self.updates
            .flush_notifications_at(&mut self.tree, self.stable_epoch);
        frame
    }
}
