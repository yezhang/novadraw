use novadraw_render::{
    BackendCapabilities, DamageMode, FrameId, NdCanvas, RenderOutcome, RenderSubmission,
    ResourceDelta, SurfaceInfo,
};

use crate::{
    BasicEventDispatcher, EventDispatcher, Figure, FigureGraph, FigureId, InteractionState, Key,
    KeyModifiers, MouseButton, PendingMutations, SceneDispatchContext, UpdateEvent, UpdateListener,
    UpdateManager, ValidationError, WheelEvent, ZoomEvent,
};

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
///
/// `FigureGraph` remains accepted as the compatibility tree implementation while
/// callers migrate to the `FigureTree` name.
pub struct Runtime {
    tree: FigureGraph,
    interaction: InteractionState,
    interaction_dispatcher: BasicEventDispatcher,
    updates: UpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
    next_frame_id: FrameId,
    in_flight: Option<InFlightFrame>,
    last_surface: Option<SurfaceInfo>,
    pending_resources: ResourceDelta,
}

struct InFlightFrame {
    id: FrameId,
    resources: ResourceDelta,
}

impl Runtime {
    pub fn new(tree: FigureGraph) -> Self {
        Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: BasicEventDispatcher,
            updates: UpdateManager::new(),
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
            next_frame_id: FrameId::INITIAL,
            in_flight: None,
            last_surface: None,
            pending_resources: ResourceDelta::default(),
        }
    }

    pub fn empty() -> Self {
        Self::new(FigureGraph::new())
    }

    pub fn tree(&self) -> &FigureGraph {
        &self.tree
    }

    pub fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub fn set_contents(&mut self, figure: Box<dyn Figure>) -> FigureId {
        let id = self.tree.set_contents(figure);
        self.full_redraw_pending = true;
        self.tree.mark_invalid(&mut self.updates, id);
        self.tree.repaint(&mut self.updates, id, None);
        id
    }

    pub fn add_figure(&mut self, parent: FigureId, figure: Box<dyn Figure>) -> FigureId {
        self.tree.add_child(&mut self.updates, parent, figure)
    }

    pub fn remove_figure(&mut self, parent: FigureId, child: FigureId) -> bool {
        let changed = self.tree.remove_child(&mut self.updates, parent, child);
        self.retain_interactive_figures();
        changed
    }

    pub fn reparent(&mut self, child: FigureId, new_parent: FigureId) -> bool {
        self.tree.reparent(&mut self.updates, child, new_parent)
    }

    pub fn set_bounds(&mut self, id: FigureId, bounds: novadraw_geometry::Rectangle) -> bool {
        let is_contents = self.tree.get_contents() == Some(id);
        let changed = self.tree.set_bounds_with_update(
            &mut self.updates,
            id,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
        if changed && is_contents {
            // The synthetic tree root has no drawable background to repair
            // pixels exposed by a moved or resized contents node.
            self.full_redraw_pending = true;
        }
        changed
    }

    pub fn set_visible(&mut self, id: FigureId, visible: bool) -> bool {
        let changed = self
            .tree
            .set_visible_with_update(&mut self.updates, id, visible);
        self.retain_interactive_figures();
        changed
    }

    pub fn set_enabled(&mut self, id: FigureId, enabled: bool) -> bool {
        let changed = self
            .tree
            .set_enabled_with_update(&mut self.updates, id, enabled);
        self.retain_interactive_figures();
        changed
    }

    pub fn translate(&mut self, id: FigureId, dx: f64, dy: f64) -> bool {
        let Some(bounds) = self.tree.figure_bounds(id) else {
            return false;
        };
        self.set_bounds(
            id,
            novadraw_geometry::Rectangle::new(
                bounds.x + dx,
                bounds.y + dy,
                bounds.width,
                bounds.height,
            ),
        )
    }

    pub fn set_selected(&mut self, id: Option<FigureId>) {
        self.tree.set_selected(id);
        self.full_redraw_pending = true;
    }

    pub fn into_tree(self) -> FigureGraph {
        self.tree
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) {
        self.updates.add_listener(listener);
    }

    pub fn has_pending_update(&self) -> bool {
        self.full_redraw_pending || self.updates.is_update_queued()
    }

    pub fn last_validation_error(&self) -> Option<&ValidationError> {
        self.updates.last_validation_error()
    }

    pub fn take_validation_error(&mut self) -> Option<ValidationError> {
        self.updates.take_validation_error()
    }

    pub fn request_full_redraw(&mut self) {
        self.full_redraw_pending = true;
    }

    pub fn add_resource(&mut self, resource_id: u64) {
        self.pending_resources.added.push(resource_id);
        self.full_redraw_pending = true;
    }

    pub fn remove_resource(&mut self, resource_id: u64) {
        self.pending_resources.removed.push(resource_id);
        self.full_redraw_pending = true;
    }

    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_moved(ctx, x, y));
    }

    pub fn dispatch_mouse_pressed(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_pressed(ctx, x, y, button));
    }

    pub fn dispatch_mouse_released(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_released(ctx, x, y, button));
    }

    pub fn dispatch_mouse_double_clicked(&mut self, x: f64, y: f64, button: MouseButton) {
        self.dispatch(|dispatcher, ctx| {
            dispatcher.dispatch_mouse_double_clicked(ctx, x, y, button)
        });
    }

    pub fn dispatch_mouse_hover(&mut self, x: f64, y: f64) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_hover(ctx, x, y));
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_scroll(ctx, event));
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_zoom(ctx, event));
    }

    pub fn dispatch_key_pressed(&mut self, key: Key, modifiers: KeyModifiers) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_pressed(ctx, key, modifiers));
    }

    pub fn dispatch_key_released(&mut self, key: Key, modifiers: KeyModifiers) {
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_released(ctx, key, modifiers));
    }

    pub fn release_focus(&mut self) {
        self.dispatch(|dispatcher, ctx| dispatcher.release_focus(ctx));
    }

    pub fn cancel_gestures(&mut self) {
        self.dispatch(|dispatcher, ctx| dispatcher.cancel_gestures(ctx));
    }

    /// Applies all callback effects and structural mutations before returning.
    fn dispatch(
        &mut self,
        action: impl FnOnce(&mut BasicEventDispatcher, &mut SceneDispatchContext<'_>),
    ) {
        {
            let mut context = SceneDispatchContext::new(
                &mut self.tree,
                &mut self.interaction,
                &mut self.updates,
                &mut self.mutations,
            );
            action(&mut self.interaction_dispatcher, &mut context);
        }
        let mutations = self.mutations.drain();
        self.tree
            .apply_pending_mutations(&mut self.updates, mutations);
        self.retain_interactive_figures();
    }

    fn retain_interactive_figures(&mut self) {
        self.interaction.reconcile(&self.tree);
    }

    /// Produces one complete renderer submission at a stable transaction boundary.
    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> Option<RenderSubmission> {
        if self.in_flight.is_some() {
            return None;
        }

        let mutations = self.mutations.drain();
        if !mutations.is_empty() {
            self.tree
                .apply_pending_mutations(&mut self.updates, mutations);
            self.retain_interactive_figures();
        }

        let surface_changed = self.last_surface != Some(surface);
        self.last_surface = Some(surface);
        if surface_changed {
            self.full_redraw_pending = true;
        }
        if !surface.is_renderable() {
            self.full_redraw_pending = true;
            return None;
        }

        let mut canvas = if self.updates.is_update_queued() {
            self.tree.perform_update(&mut self.updates)
        } else if self.full_redraw_pending {
            self.tree.render()
        } else {
            return None;
        };
        if self.updates.last_validation_error().is_some() {
            self.full_redraw_pending = true;
            return None;
        }

        if self.full_redraw_pending
            || (canvas.damage().mode() == DamageMode::Partial
                && !capabilities.supports_partial_damage())
        {
            if canvas.commands().is_empty() {
                canvas = self.tree.render();
            } else {
                canvas.damage_mut().set_full();
            }
        }

        let frame_id = self.next_frame_id;
        self.next_frame_id = self.next_frame_id.next();
        let resources = std::mem::take(&mut self.pending_resources);
        let submission = canvas.to_submission_for_frame(surface, resources.clone(), frame_id);
        self.full_redraw_pending = false;
        self.in_flight = Some(InFlightFrame {
            id: frame_id,
            resources,
        });
        self.updates.emit_update_event(UpdateEvent::Prepared {
            frame_id,
            damage: submission.damage.mode(),
        });
        self.updates.flush_notifications(&mut self.tree);
        Some(submission)
    }

    /// Completes the in-flight frame and restores work when presentation failed.
    pub fn complete_submission(&mut self, frame_id: FrameId, outcome: RenderOutcome) -> bool {
        let Some(in_flight) = self.in_flight.take() else {
            return false;
        };
        if in_flight.id != frame_id {
            self.in_flight = Some(in_flight);
            return false;
        }

        if outcome != RenderOutcome::Presented {
            self.full_redraw_pending = true;
            let newer_resources = std::mem::take(&mut self.pending_resources);
            self.pending_resources = in_flight.resources;
            self.pending_resources.extend(newer_resources);
        }
        self.updates
            .emit_update_event(UpdateEvent::Submitted { frame_id, outcome });
        self.updates.flush_notifications(&mut self.tree);
        true
    }

    /// Prepares an incremental frame when the runtime has pending work.
    pub fn prepare_frame(&mut self) -> Option<NdCanvas> {
        if self.updates.is_update_queued() {
            self.full_redraw_pending = false;
            return Some(self.tree.perform_update(&mut self.updates));
        }
        if std::mem::take(&mut self.full_redraw_pending) {
            return Some(self.tree.render());
        }
        None
    }

    /// Records the complete visible tree, independent of pending update state.
    pub fn record_full_frame(&self) -> NdCanvas {
        self.tree.render()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FigureEvent, RectangleFigure};
    use novadraw_core::Color;
    use std::sync::{Arc, Mutex};

    fn surface(width: u32, height: u32) -> SurfaceInfo {
        SurfaceInfo {
            logical_width: f64::from(width),
            logical_height: f64::from(height),
            pixel_width: width,
            pixel_height: height,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn dispatch_flushes_structural_mutations_before_returning() {
        let mut tree = FigureGraph::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            100.0,
            100.0,
            Color::WHITE,
        )));
        let mut runtime = Runtime::new(tree);

        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),
            ));
        runtime.dispatch(|_, _| {});

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.has_pending_update());
    }

    #[test]
    fn runtime_owns_interaction_state_separately_from_tree() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        runtime.interaction.set_mouse_target(Some(root));

        assert_eq!(runtime.interaction().mouse_target(), Some(root));
        assert_eq!(runtime.tree().get_contents(), Some(root));
    }

    #[test]
    fn submission_contains_surface_resources_and_monotonic_frame_id() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        runtime.add_resource(7);

        let first = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(first.frame_id, FrameId::INITIAL);
        assert_eq!(first.surface, surface(100, 100));
        assert_eq!(first.resources.added, vec![7]);
        assert_eq!(first.damage.mode(), DamageMode::Full);
        assert!(runtime.complete_submission(first.frame_id, RenderOutcome::Presented));

        runtime.request_full_redraw();
        let second = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(second.frame_id, first.frame_id.next());
        assert!(runtime.complete_submission(second.frame_id, RenderOutcome::Presented));
        assert!(
            runtime
                .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
                .is_none()
        );
    }

    #[test]
    fn backend_capability_promotes_partial_damage_to_full() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            runtime.add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        runtime.set_bounds(
            child,
            novadraw_geometry::Rectangle::new(15.0, 15.0, 20.0, 20.0),
        );
        let partial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(partial.damage.mode(), DamageMode::Partial);
        runtime.complete_submission(partial.frame_id, RenderOutcome::Presented);

        runtime.set_bounds(
            child,
            novadraw_geometry::Rectangle::new(20.0, 20.0, 20.0, 20.0),
        );
        let promoted = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY)
            .unwrap();
        assert_eq!(promoted.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn moving_contents_forces_full_damage_to_clear_exposed_pixels() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(20.0, 20.0, 60.0, 60.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        assert!(runtime.translate(root, 10.0, 10.0));
        let moved = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(moved.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn surface_change_and_retry_force_full_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        runtime.add_resource(11);
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(runtime.complete_submission(initial.frame_id, RenderOutcome::Retry));

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert_eq!(retry.resources.added, vec![11]);
        runtime.complete_submission(retry.frame_id, RenderOutcome::Presented);

        let resized = runtime
            .prepare_submission(surface(120, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resized.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn dpi_change_updates_surface_metadata_and_forces_full_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        let hidpi_surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 200,
            pixel_height: 200,
            scale_factor: 2.0,
        };
        let submission = runtime
            .prepare_submission(hidpi_surface, BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(submission.surface, hidpi_surface);
        assert_eq!(submission.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn suspended_surface_preserves_full_redraw_request() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        assert!(
            runtime
                .prepare_submission(surface(0, 100), BackendCapabilities::RETAINED_PARTIAL)
                .is_none()
        );
        assert!(runtime.has_pending_update());

        let resumed = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resumed.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn submission_notifications_are_flushed_in_causal_order() {
        struct CaptureUpdates(Arc<Mutex<Vec<UpdateEvent>>>);

        impl UpdateListener for CaptureUpdates {
            fn on_update_event(&self, event: UpdateEvent) {
                self.0.lock().unwrap().push(event);
            }

            fn on_figure_event(&self, _event: FigureEvent) {}

            fn on_notify(&self, _block_id: FigureId) {}
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::empty();
        runtime.add_update_listener(Box::new(CaptureUpdates(events.clone())));
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(submission.frame_id, RenderOutcome::Presented);

        let events = events.lock().unwrap();
        let validated = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Validated))
            .unwrap();
        let painted = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Painted { .. }))
            .unwrap();
        let prepared = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Prepared { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        let submitted = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Submitted { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        assert!(validated < painted);
        assert!(painted < prepared);
        assert!(prepared < submitted);
    }

    #[test]
    fn pending_mutations_are_applied_before_validation_and_recording() {
        let mut tree = FigureGraph::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let mut runtime = Runtime::new(tree);
        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
            ));

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.tree.is_valid(root));
        assert!(!submission.commands.is_empty());
    }
}
