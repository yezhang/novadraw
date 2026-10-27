use super::{
    AnchorGeometry, AnchorGeometryKey, AnchorId, BackendSessionError, ConnectionAnchor,
    ConnectionId, ConnectionLocatorStrategy, ConnectionRouter, ConnectionRoutingStats,
    ConnectionRuntimeError, ConnectionStateSnapshot, CoordinateSpace, DependencySubject, FigureId,
    FontId, FramePreparation, FramePreparationError, LogicalViewportResizeError, Rectangle,
    ResourceError, ResourceRegistry, RouteError, RouteMetadata, RouteOutput, RouterBinding,
    RouterId, RoutingConstraint, Runtime, SubmissionAcknowledgement, SubmissionCompletionError,
};
use crate::animation::AnimationMut;
use crate::animation::{
    AnimationStart, AnimationTransactionError, BoundsTransition, ConnectionRouteTransition,
    ViewportTransition,
};
use crate::render::{
    BackendCapabilities, BackendSessionId, BuiltinFont, CaretGeometry, FontData, FontDescriptor,
    FrameId, ImageData, RenderResult, ResourceId, SelectionQuad, SurfaceInfo, TextConstraints,
    TextError, TextLayout, TextMovement,
};
use crate::{
    AccessibilityAction, AccessibilityError, AccessibilityNodeId, AccessibilitySnapshot,
    AccessibilityUpdate, ActionListener, AncestorListener, ClickableSnapshot, CoordinateListener,
    CursorIcon, DispatchOutcome, FigureListener, FlowTextPosition, FlowTextRange, FocusChange,
    FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    ImageCompletionError, ImageId, InteractionState, Key, KeyModifiers, LayoutListener, ListenerId,
    ListenerScope, MonotonicTime, MouseButton, ObservationListener, PropertyChangeListener,
    ResourceKind, ResourceRequestToken, ResourceStatus, RuntimeMutationError, ShapeMutationError,
    StableQueryError, StableSceneQuery, TextFlowQueryError, TextLayoutStats, TimeError,
    TooltipSnapshot, TooltipTiming, TooltipUpdate, UpdateListener, ValidationError, WheelEvent,
    WidgetError, ZoomEvent,
};

pub struct RuntimeFrames<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeFrames<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn stabilize_for_query(&mut self) -> Result<(), FramePreparationError> {
        self.runtime.stabilize_for_query()
    }

    /// Enables optional CPU phase timing for subsequent work, clearing prior measurements.
    /// Default is disabled; no clocks are read or diagnostic records allocated in that mode.
    pub fn set_timing_enabled(&mut self, enabled: bool) {
        self.runtime.timing = if enabled {
            crate::runtime::timing::TimingRecorder::enabled()
        } else {
            crate::runtime::timing::TimingRecorder::default()
        };
    }

    /// Drains instance-owned timings, including failed attempts. Nested stages are inclusive;
    /// exclusive values subtract timed children. Neither is an input-to-screen latency.
    pub fn take_timings(&mut self) -> Vec<crate::runtime::RuntimePhaseTiming> {
        self.runtime.timing.take()
    }

    /// Limits all work and allocation entries in each complete stabilization attempt.
    pub fn preparation_budget(&self) -> crate::figure::PreparationBudget {
        self.runtime.preparation_budget
    }

    /// Changes host policy for subsequent attempts; callbacks cannot replenish this budget.
    pub fn set_preparation_budget(&mut self, budget: crate::figure::PreparationBudget) {
        self.runtime.preparation_budget = budget;
        self.runtime.request_full_redraw();
    }

    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        self.runtime.prepare_submission(surface, capabilities)
    }

    pub fn complete_submission(
        &mut self,
        session_id: BackendSessionId,
        frame_id: FrameId,
        result: RenderResult,
    ) -> Result<SubmissionAcknowledgement, SubmissionCompletionError> {
        self.runtime
            .complete_submission(session_id, frame_id, result)
    }

    pub fn logical_viewport(&self) -> Option<Rectangle> {
        self.runtime.logical_viewport()
    }

    pub fn resize_logical_viewport(
        &mut self,
        width: f64,
        height: f64,
    ) -> Result<bool, LogicalViewportResizeError> {
        self.runtime.resize_logical_viewport(width, height)
    }

    pub fn backend_session_id(&self) -> BackendSessionId {
        self.runtime.backend_session_id()
    }

    pub fn reset_backend_session(&mut self) -> Result<BackendSessionId, BackendSessionError> {
        self.runtime.reset_backend_session()
    }

    pub fn request_full_redraw(&mut self) {
        self.runtime.request_full_redraw();
    }

    pub fn has_pending_update(&self) -> bool {
        self.runtime.has_pending_update()
    }

    /// Registers a host wakeup for background work independently of Figure changes.
    pub fn set_background_waker(&mut self, waker: std::task::Waker) {
        self.runtime.background.waker = Some(waker.clone());
        if self.runtime.has_pending_update() {
            waker.wake();
        }
    }

    pub fn set_background_input(
        &mut self,
        input: crate::host::BackgroundFrameInput,
    ) -> Result<(), crate::host::HostBackgroundError> {
        self.runtime.set_background_input(input)
    }

    pub fn background_input(&self) -> Option<crate::host::BackgroundFrameInput> {
        self.runtime.background.input
    }

    /// Starts/replaces a host request. Initial Pending uses the explicit base clear.
    pub fn begin_background(
        &mut self,
        source: crate::host::HostBackgroundId,
        input: crate::host::BackgroundFrameInput,
        base: crate::graphics::Color,
    ) -> Result<crate::host::HostBackgroundRequest, crate::host::HostBackgroundError> {
        self.runtime.begin_background(source, input, base)
    }

    pub fn complete_background(
        &mut self,
        request: crate::host::HostBackgroundRequest,
        result: Result<crate::host::HostBackgroundSnapshot, crate::host::HostBackgroundError>,
    ) -> Result<(), crate::host::HostBackgroundError> {
        self.runtime.complete_background(request, result)
    }

    pub fn remove_background(
        &mut self,
        base: crate::graphics::Color,
    ) -> Result<(), crate::host::HostBackgroundError> {
        self.runtime.remove_background(base)
    }

    pub fn current_background(&self) -> Option<&crate::host::HostBackgroundSnapshot> {
        self.runtime
            .background
            .current
            .as_ref()
            .map(|current| &current.snapshot)
    }

    pub fn background_error(&self) -> Option<&crate::host::HostBackgroundError> {
        self.runtime.background.error.as_ref()
    }
}

pub struct RuntimeResources<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeResources<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn registry(&self) -> &ResourceRegistry {
        self.runtime.resource_registry()
    }

    pub fn builtin_font(&self, font: BuiltinFont) -> Option<FontId> {
        self.runtime.builtin_font(font)
    }

    pub fn register_image(&mut self) -> Result<ImageId, ResourceError> {
        self.runtime.register_image()
    }

    pub fn register_font(&mut self) -> Result<FontId, ResourceError> {
        self.runtime.register_font()
    }

    pub fn register_builtin_font(&mut self, builtin: BuiltinFont) -> Result<FontId, ResourceError> {
        self.runtime.register_builtin_font(builtin)
    }

    pub fn status(&self, resource_id: ResourceId) -> Result<&ResourceStatus, ResourceError> {
        self.runtime.resource_status(resource_id)
    }

    pub fn add_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        self.runtime.add_resource_dependency(resource_id, figure)
    }

    pub fn remove_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        self.runtime.remove_resource_dependency(resource_id, figure)
    }

    pub fn complete_image(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        self.runtime.complete_image(id, image)
    }

    pub fn begin_request(&mut self, id: ResourceId) -> Result<ResourceRequestToken, ResourceError> {
        self.runtime.begin_resource_request(id)
    }

    pub fn complete_image_request(
        &mut self,
        token: ResourceRequestToken,
        image: ImageData,
    ) -> Result<(), ResourceError> {
        self.runtime.complete_image_request(token, image)
    }

    pub fn complete_image_request_bytes(
        &mut self,
        token: ResourceRequestToken,
        bytes: &[u8],
        scale: f64,
    ) -> Result<(), ImageCompletionError> {
        self.runtime
            .complete_image_request_bytes(token, bytes, scale)
    }

    pub fn complete_font(&mut self, id: FontId, font: FontData) -> Result<(), ResourceError> {
        self.runtime.complete_font(id, font)
    }

    pub fn complete_font_request(
        &mut self,
        token: ResourceRequestToken,
        font: FontData,
    ) -> Result<(), ResourceError> {
        self.runtime.complete_font_request(token, font)
    }

    pub fn fail(&mut self, id: ResourceId, reason: impl Into<String>) -> Result<(), ResourceError> {
        self.runtime.fail_resource(id, reason)
    }

    pub fn fail_request(
        &mut self,
        token: ResourceRequestToken,
        reason: impl Into<String>,
    ) -> Result<(), ResourceError> {
        self.runtime.fail_resource_request(token, reason)
    }

    pub fn remove(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        self.runtime.remove_resource(id)
    }

    pub fn kind(&self, id: ResourceId) -> Result<ResourceKind, ResourceError> {
        self.runtime.resource_registry().kind(id)
    }
}

pub struct RuntimeConnections<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeConnections<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn direct_router(&self) -> RouterId {
        self.runtime.direct_connection_router()
    }

    pub fn register_anchor(&mut self, anchor: Box<dyn ConnectionAnchor>) -> AnchorId {
        self.runtime.register_connection_anchor(anchor)
    }

    pub fn try_register_anchor(
        &mut self,
        anchor: Box<dyn ConnectionAnchor>,
    ) -> Result<AnchorId, ConnectionRuntimeError> {
        self.runtime.try_register_connection_anchor(anchor)
    }

    pub fn remove_anchor(
        &mut self,
        anchor: AnchorId,
    ) -> Result<Box<dyn ConnectionAnchor>, ConnectionRuntimeError> {
        self.runtime.remove_connection_anchor(anchor)
    }

    pub fn set_anchor_geometry(
        &mut self,
        figure: FigureId,
        key: AnchorGeometryKey,
        geometry: AnchorGeometry,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.runtime.set_anchor_geometry(figure, key, geometry)
    }

    pub fn anchor_geometry(
        &self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Option<&AnchorGeometry> {
        self.runtime.anchor_geometry(figure, key)
    }

    pub fn register_router(&mut self, router: Box<dyn ConnectionRouter>) -> RouterId {
        self.runtime.register_connection_router(router)
    }

    pub fn try_register_router(
        &mut self,
        router: Box<dyn ConnectionRouter>,
    ) -> Result<RouterId, ConnectionRuntimeError> {
        self.runtime.try_register_connection_router(router)
    }

    pub fn remove_router(
        &mut self,
        router: RouterId,
    ) -> Result<Box<dyn ConnectionRouter>, ConnectionRuntimeError> {
        self.runtime.remove_connection_router(router)
    }

    pub fn set_layer_router(
        &mut self,
        layer: FigureId,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.runtime.set_connection_layer_router(layer, router)
    }

    pub fn register_state(
        &mut self,
        figure: FigureId,
        source: Option<AnchorId>,
        target: Option<AnchorId>,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<ConnectionId, ConnectionRuntimeError> {
        self.runtime
            .register_connection_state(figure, source, target, router, constraint)
    }

    pub fn remove_state(&mut self, connection: ConnectionId) -> Result<(), ConnectionRuntimeError> {
        self.runtime.remove_connection_state(connection)
    }

    pub fn state(
        &self,
        connection: ConnectionId,
    ) -> Result<ConnectionStateSnapshot, ConnectionRuntimeError> {
        self.runtime.connection_state(connection)
    }

    pub fn routing_stats(&self) -> ConnectionRoutingStats {
        self.runtime.connection_routing_stats()
    }

    pub fn set_source(
        &mut self,
        connection: ConnectionId,
        source: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.runtime.set_connection_source(connection, source)
    }

    pub fn set_target(
        &mut self,
        connection: ConnectionId,
        target: Option<AnchorId>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.runtime.set_connection_target(connection, target)
    }

    pub fn set_router_binding(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.runtime
            .set_connection_router_binding(connection, router)
    }

    pub fn set_constraint(
        &mut self,
        connection: ConnectionId,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<(), ConnectionRuntimeError> {
        self.runtime
            .set_connection_constraint(connection, constraint)
    }

    pub fn set_route_configuration(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
        constraint: Option<Box<dyn RoutingConstraint>>,
    ) -> Result<bool, ConnectionRuntimeError> {
        self.runtime
            .set_connection_route_configuration(connection, router, constraint)
    }

    pub fn set_locator(
        &mut self,
        connection: ConnectionId,
        child: FigureId,
        strategy: Box<dyn ConnectionLocatorStrategy>,
    ) -> Result<(), ConnectionRuntimeError> {
        self.runtime
            .set_connection_locator(connection, child, strategy)
    }

    pub fn remove_locator(&mut self, child: FigureId) -> Result<bool, ConnectionRuntimeError> {
        self.runtime.remove_connection_locator(child)
    }

    pub fn preview_endpoints(
        &self,
        source: &dyn ConnectionAnchor,
        target: &dyn ConnectionAnchor,
        routing_space: CoordinateSpace,
    ) -> Result<RouteMetadata, RouteError> {
        self.runtime
            .preview_connection_endpoints(source, target, routing_space)
    }

    pub fn resolve_route(
        &mut self,
        connection: ConnectionId,
        routing_space: CoordinateSpace,
    ) -> Result<RouteOutput, ConnectionRuntimeError> {
        self.runtime
            .resolve_connection_route(connection, routing_space)
    }

    pub fn invalidate_dependency(
        &mut self,
        subject: &DependencySubject,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        self.runtime.invalidate_connection_dependency(subject)
    }

    pub fn dirty(&self) -> Vec<ConnectionId> {
        self.runtime.dirty_connections()
    }

    pub fn last_error(&self) -> Option<&ConnectionRuntimeError> {
        self.runtime.last_connection_error()
    }

    pub fn take_error(&mut self) -> Option<ConnectionRuntimeError> {
        self.runtime.take_connection_error()
    }
}

pub struct RuntimeInput<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeInput<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn state(&self) -> &InteractionState {
        self.runtime.interaction()
    }

    pub fn clickable_snapshot(&self, id: FigureId) -> Result<ClickableSnapshot, WidgetError> {
        self.runtime.clickable_snapshot(id)
    }

    pub fn do_click(&mut self, id: FigureId) -> Result<bool, WidgetError> {
        self.runtime.do_click(id)
    }

    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) -> DispatchOutcome {
        self.runtime.dispatch_mouse_moved(x, y)
    }

    pub fn dispatch_mouse_pressed(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.runtime.dispatch_mouse_pressed(x, y, button)
    }

    pub fn dispatch_mouse_released(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.runtime.dispatch_mouse_released(x, y, button)
    }

    pub fn dispatch_mouse_double_clicked(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.runtime.dispatch_mouse_double_clicked(x, y, button)
    }

    pub fn dispatch_mouse_hover(&mut self, x: f64, y: f64) -> DispatchOutcome {
        self.runtime.dispatch_mouse_hover(x, y)
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) -> DispatchOutcome {
        self.runtime.dispatch_scroll(event)
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) -> DispatchOutcome {
        self.runtime.dispatch_zoom(event)
    }

    pub fn dispatch_key_pressed(&mut self, key: Key, modifiers: KeyModifiers) -> DispatchOutcome {
        self.runtime.dispatch_key_pressed(key, modifiers)
    }

    pub fn dispatch_key_released(&mut self, key: Key, modifiers: KeyModifiers) -> DispatchOutcome {
        self.runtime.dispatch_key_released(key, modifiers)
    }

    pub fn cancel_gestures(&mut self) {
        self.runtime.cancel_gestures();
    }

    pub fn pointer_exited(&mut self) {
        self.runtime.pointer_exited();
    }
}

pub struct RuntimeFocus<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeFocus<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.runtime.cursor_icon()
    }

    pub fn request(&mut self, target: FigureId) -> Result<FocusChange, FocusError> {
        self.runtime.request_focus(target)
    }

    pub fn clear(&mut self) -> FocusChange {
        self.runtime.clear_focus()
    }

    pub fn release(&mut self) -> FocusChange {
        self.runtime.release_focus()
    }

    pub fn traverse(&mut self, direction: FocusTraversalDirection) -> FocusTraversalOutcome {
        self.runtime.traverse_focus(direction)
    }

    pub fn set_traversal_policy(&mut self, policy: Box<dyn FocusTraversalPolicy>) {
        self.runtime.set_focus_traversal_policy(policy);
    }
}

pub struct RuntimeTooltip<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeTooltip<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn text(&self) -> Option<String> {
        self.runtime.tooltip_text()
    }

    pub fn set_timing(&mut self, timing: TooltipTiming) -> Result<bool, TimeError> {
        self.runtime.set_tooltip_timing(timing)
    }

    pub fn visible(&self) -> Option<&TooltipSnapshot> {
        self.runtime.visible_tooltip()
    }

    pub fn take_updates(&mut self) -> Vec<TooltipUpdate> {
        self.runtime.take_tooltip_updates()
    }
}

pub struct RuntimeAccessibility<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeAccessibility<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.runtime.accessibility_snapshot()
    }

    pub fn take_updates(&mut self) -> Vec<AccessibilityUpdate> {
        self.runtime.take_accessibility_updates()
    }

    pub fn request_snapshot(&mut self) {
        self.runtime.request_accessibility_snapshot();
    }

    pub fn perform_action(
        &mut self,
        node: AccessibilityNodeId,
        action: AccessibilityAction,
    ) -> Result<bool, AccessibilityError> {
        self.runtime.perform_accessibility_action(node, action)
    }
}

pub struct RuntimeAnimation<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeAnimation<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn timeline(&mut self) -> AnimationMut<'_> {
        self.runtime.animations()
    }

    pub fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        self.runtime.advance_time(now)
    }

    pub fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        self.runtime.next_wake_deadline()
    }

    /// Materializes the last accepted samples after source stabilization, without recording.
    pub fn capture_geometry(
        &mut self,
        dependencies: impl IntoIterator<Item = FigureId>,
    ) -> Result<crate::animation::GeometryCapture, crate::animation::GeometryCaptureError> {
        use crate::animation::{AnimationError, GeometryCaptureError};
        if self.runtime.faulted {
            return Err(GeometryCaptureError::Animation(
                AnimationError::RuntimeFaulted,
            ));
        }
        let max_nodes = self.runtime.animations().limits().staging_work.nodes;
        let dependencies = dependencies
            .into_iter()
            .try_fold(Vec::new(), |mut nodes, id| {
                if nodes.len() >= max_nodes {
                    return Err(GeometryCaptureError::Animation(
                        AnimationError::BudgetExceeded(
                            crate::animation::AnimationBudgetKind::GeometryNodes,
                        ),
                    ));
                }
                nodes.push(id);
                Ok(nodes)
            })?;
        self.runtime.guarded(|runtime| {
            runtime.apply_pending_mutations_for_frame();
            runtime
                .try_stabilize()
                .map_err(GeometryCaptureError::Source)?;
            runtime
                .animations
                .capture_geometry(
                    &runtime.tree,
                    &runtime.resources,
                    runtime.stable_epoch,
                    dependencies,
                )
                .map_err(GeometryCaptureError::Animation)
        })
    }

    /// Cache-only query. A new sample or source revision requires explicit materialization.
    pub fn geometry(
        &self,
    ) -> Result<&crate::animation::PresentationGeometry, crate::animation::AnimationError> {
        if self.runtime.faulted {
            return Err(crate::animation::AnimationError::RuntimeFaulted);
        }
        if !self.runtime.mutations.is_empty() {
            return Err(crate::animation::AnimationError::GeometryNotPrepared);
        }
        self.runtime
            .animations
            .geometry(self.runtime.derivation_epoch)
    }

    /// Commits the final viewport origin for a prepared geometry plan, then admits the plan.
    /// This is an animation source boundary, rather than direct input takeover. Admission
    /// failure leaves valid old owners running and does not roll back committed source.
    pub fn start_viewport_plan(
        &mut self,
        viewport: FigureId,
        final_origin: crate::geometry::Point,
        plan: crate::animation::AnimationPlan,
    ) -> Result<AnimationStart, crate::animation::AnimationError> {
        use crate::animation::AnimationError;
        let handle = self
            .runtime
            .viewports()
            .handle(viewport)
            .map_err(|_| AnimationError::InvalidValue)?;
        self.runtime.guarded(|runtime| {
            handle
                .set_view_location(
                    &mut runtime.tree,
                    &mut runtime.updates,
                    final_origin.x(),
                    final_origin.y(),
                )
                .map_err(|_| AnimationError::InvalidValue)?;
            runtime.animations().start(plan)
        })
    }

    /// Captures complete current samples, commits source once, then admits real bounds tracks.
    pub fn transition_layout_bounds<I, F, T, E>(
        &mut self,
        dependencies: I,
        transition: BoundsTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Runtime) -> Result<T, E>,
    {
        use crate::animation::GeometryCaptureError;
        let capture = self
            .capture_geometry(dependencies)
            .map_err(|error| match error {
                GeometryCaptureError::Source(error) => {
                    AnimationTransactionError::BeforeStabilization(error)
                }
                GeometryCaptureError::Animation(error) => AnimationTransactionError::Capture(error),
            })?;
        self.runtime.guarded(|runtime| {
            let result = mutation(runtime).map_err(AnimationTransactionError::Mutation)?;
            runtime.apply_pending_mutations_for_frame();
            runtime
                .try_stabilize()
                .map_err(AnimationTransactionError::AfterStabilization)?;
            let start = runtime
                .animations
                .access_mut(
                    &runtime.tree,
                    &runtime.resources,
                    &mut runtime.updates,
                    &mut runtime.full_redraw_pending,
                    &mut runtime.faulted,
                )
                .transition_layout_bounds(capture, transition)
                .map_err(AnimationTransactionError::Transition)?;
            Ok((result, start))
        })
    }

    pub fn transition_bounds<I, F, T, E>(
        &mut self,
        figures: I,
        transition: BoundsTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Runtime) -> Result<T, E>,
    {
        self.runtime
            .transition_bounds_transaction(figures, transition, mutation)
    }

    pub fn transition_viewport<F, T, E>(
        &mut self,
        viewport: &crate::container::ViewportHandle,
        transition: ViewportTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        F: FnOnce(&mut Runtime) -> Result<T, E>,
    {
        self.runtime
            .transition_viewport_transaction(viewport, transition, mutation)
    }

    pub fn transition_connection_routes<I, F, T, E>(
        &mut self,
        connections: I,
        transition: ConnectionRouteTransition,
        mutation: F,
    ) -> Result<(T, AnimationStart), AnimationTransactionError<E>>
    where
        I: IntoIterator<Item = FigureId>,
        F: FnOnce(&mut Runtime) -> Result<T, E>,
    {
        self.runtime
            .transition_connection_routes_transaction(connections, transition, mutation)
    }
}

pub struct RuntimeObservations<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeObservations<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) -> ListenerId {
        self.runtime.add_update_listener(listener)
    }

    pub fn add_update_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn UpdateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_update_listener_scoped(scope, listener)
    }

    pub fn add_figure_listener(&mut self, listener: Box<dyn FigureListener>) -> ListenerId {
        self.runtime.add_figure_listener(listener)
    }

    pub fn add_figure_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn FigureListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_figure_listener_scoped(scope, listener)
    }

    pub fn add_coordinate_listener(&mut self, listener: Box<dyn CoordinateListener>) -> ListenerId {
        self.runtime.add_coordinate_listener(listener)
    }

    pub fn add_coordinate_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn CoordinateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_coordinate_listener_scoped(scope, listener)
    }

    pub fn add_ancestor_listener(&mut self, listener: Box<dyn AncestorListener>) -> ListenerId {
        self.runtime.add_ancestor_listener(listener)
    }

    pub fn add_ancestor_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn AncestorListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_ancestor_listener_scoped(scope, listener)
    }

    pub fn add_property_listener(
        &mut self,
        listener: Box<dyn PropertyChangeListener>,
    ) -> ListenerId {
        self.runtime.add_property_listener(listener)
    }

    pub fn add_property_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn PropertyChangeListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_property_listener_scoped(scope, listener)
    }

    pub fn add_action_listener(&mut self, listener: Box<dyn ActionListener>) -> ListenerId {
        self.runtime.add_action_listener(listener)
    }

    pub fn add_action_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ActionListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_action_listener_scoped(scope, listener)
    }

    pub fn add_layout_listener(&mut self, listener: Box<dyn LayoutListener>) -> ListenerId {
        self.runtime.add_layout_listener(listener)
    }

    pub fn add_layout_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn LayoutListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime.add_layout_listener_scoped(scope, listener)
    }

    pub fn add_observation_listener(
        &mut self,
        listener: Box<dyn ObservationListener>,
    ) -> ListenerId {
        self.runtime.add_observation_listener(listener)
    }

    pub fn add_observation_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ObservationListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.runtime
            .add_observation_listener_scoped(scope, listener)
    }

    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        self.runtime.remove_listener(id)
    }

    pub fn stable_query(&self) -> Result<StableSceneQuery<'_>, StableQueryError> {
        self.runtime.stable_query()
    }

    pub fn last_validation_error(&self) -> Option<&ValidationError> {
        self.runtime.last_validation_error()
    }
}

pub struct RuntimeText<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeText<'a> {
    pub fn layout_styled(
        &mut self,
        request: &crate::text::StyledTextRequest,
    ) -> Result<crate::text::StyledTextLayout, TextError> {
        self.runtime.layout_styled_text(request)
    }

    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn revision(&self) -> u64 {
        self.runtime.text_revision()
    }

    pub fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.runtime.layout_text(text, font, constraints)
    }

    pub fn refresh_label_layouts(&mut self) -> Result<(), TextError> {
        self.runtime.refresh_label_layouts()
    }

    pub fn layout_stats(&self) -> TextLayoutStats {
        self.runtime.text_layout_stats()
    }

    pub fn label_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        self.runtime.label_text_layout(id)
    }

    pub fn flow_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        self.runtime.text_flow_layout(id)
    }

    pub fn flow_hit_test(
        &self,
        id: FigureId,
        surface_point: crate::geometry::Point,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        self.runtime.text_flow_hit_test(id, surface_point)
    }

    pub fn flow_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        self.runtime.text_flow_caret_geometry(id, position)
    }

    pub fn flow_local_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        self.runtime.text_flow_local_caret_geometry(id, position)
    }

    pub fn flow_selection_geometry(
        &self,
        id: FigureId,
        range: FlowTextRange,
    ) -> Result<Vec<SelectionQuad>, TextFlowQueryError> {
        self.runtime.text_flow_selection_geometry(id, range)
    }

    pub fn flow_move_position(
        &self,
        id: FigureId,
        position: FlowTextPosition,
        movement: TextMovement,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        self.runtime.text_flow_move_position(id, position, movement)
    }

    pub fn label_text(&self, id: FigureId) -> Result<&str, ShapeMutationError> {
        self.runtime.label_text(id)
    }

    pub fn title_bar_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        self.runtime.title_bar_text_layout(id)
    }
}
