use novadraw_render::{
    BackendCapabilities, DamageMode, FontData, FrameId, ImageData, NdCanvas, RenderOutcome,
    RenderSubmission, ResourceDelta, ResourceId, SurfaceInfo,
};

use crate::{
    CursorIcon, EventDispatcher, Figure, FigureId, FigureStyle, FigureTree, FontId, ImageId,
    InteractionState, Key, KeyModifiers, MouseButton, PendingMutations, ResourceError,
    ResourceRegistry, ResourceStatus, SceneDispatchContext, UpdateEvent, UpdateListener,
    UpdateManager, ValidationError, WheelEvent, ZoomEvent,
};

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
pub struct Runtime {
    tree: FigureTree,
    interaction: InteractionState,
    interaction_dispatcher: EventDispatcher,
    updates: UpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
    next_frame_id: FrameId,
    in_flight: Option<InFlightFrame>,
    last_surface: Option<SurfaceInfo>,
    resources: ResourceRegistry,
}

struct InFlightFrame {
    id: FrameId,
    resources: ResourceDelta,
}

impl Runtime {
    pub fn new(tree: FigureTree) -> Self {
        Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: EventDispatcher,
            updates: UpdateManager::new(),
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
            next_frame_id: FrameId::INITIAL,
            in_flight: None,
            last_surface: None,
            resources: ResourceRegistry::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new(FigureTree::new())
    }

    pub fn tree(&self) -> &FigureTree {
        &self.tree
    }

    pub fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.interaction
            .cursor_target()
            .and_then(|id| self.tree.resolved_style(id))
            .map_or(CursorIcon::Default, |style| style.cursor)
    }

    pub fn tooltip(&self) -> Option<String> {
        self.interaction
            .hover_source()
            .and_then(|id| self.tree.resolved_style(id))
            .and_then(|style| style.tooltip)
    }

    pub fn resources(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub fn set_contents(&mut self, figure: Box<dyn Figure>) -> FigureId {
        let id = self.tree.set_contents(figure);
        self.retain_interactive_figures();
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

    pub fn set_figure_style(&mut self, id: FigureId, style: FigureStyle) -> bool {
        self.tree
            .set_figure_style_with_update(&mut self.updates, id, style)
    }

    pub fn set_opaque(&mut self, id: FigureId, opaque: bool) -> bool {
        if !self.tree.set_opaque(id, opaque) {
            return false;
        }
        self.tree.repaint(&mut self.updates, id, None);
        true
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

    pub fn into_tree(self) -> FigureTree {
        self.tree
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) {
        self.updates.add_listener(listener);
    }

    pub fn has_pending_update(&self) -> bool {
        self.full_redraw_pending
            || self.updates.is_update_queued()
            || self.resources.has_pending_delta()
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

    pub fn register_image(&mut self) -> ImageId {
        self.resources.register_image()
    }

    pub fn register_font(&mut self) -> FontId {
        self.resources.register_font()
    }

    pub fn resource_status(
        &self,
        resource_id: ResourceId,
    ) -> Result<&ResourceStatus, ResourceError> {
        self.resources.status(resource_id)
    }

    pub fn add_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        if !self.tree.is_attached(figure) {
            return Err(ResourceError::UnknownFigure);
        }
        self.resources.add_dependency(resource_id, figure)
    }

    pub fn remove_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        self.resources.remove_dependency(resource_id, figure)
    }

    pub fn complete_image(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        let dependents = self.resources.complete_image(id, image)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn complete_font(&mut self, id: FontId, font: FontData) -> Result<(), ResourceError> {
        let dependents = self.resources.complete_font(id, font)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn fail_resource(
        &mut self,
        id: ResourceId,
        reason: impl Into<String>,
    ) -> Result<(), ResourceError> {
        let dependents = self.resources.fail(id, reason)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn remove_resource(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        let dependents = self.resources.remove(id)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
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
        action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>),
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
        self.resources
            .retain_dependencies(|id| self.tree.is_attached(id));
    }

    fn invalidate_resource_dependents(&mut self, dependents: Vec<FigureId>) {
        for figure in dependents {
            if self.tree.is_attached(figure) {
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
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

        let has_resource_delta = self.resources.has_pending_delta();
        let mut canvas = if self.updates.is_update_queued() {
            self.tree.perform_update(&mut self.updates)
        } else if self.full_redraw_pending {
            self.tree.render()
        } else if has_resource_delta {
            NdCanvas::new()
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
        let resources = self.resources.take_delta();
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
            self.resources.restore_delta(in_flight.resources);
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
        let mut tree = FigureTree::new();
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
    fn cursor_and_tooltip_resolve_from_the_current_pointer_targets() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        runtime.set_figure_style(
            root,
            FigureStyle {
                cursor: Some(CursorIcon::Crosshair),
                tooltip: Some(Some("root tip".to_string())),
                ..FigureStyle::default()
            },
        );

        runtime.dispatch_mouse_moved(50.0, 50.0);

        assert_eq!(runtime.cursor_icon(), CursorIcon::Crosshair);
        assert_eq!(runtime.tooltip().as_deref(), Some("root tip"));

        runtime.dispatch_mouse_moved(150.0, 150.0);
        assert_eq!(runtime.cursor_icon(), CursorIcon::Default);
        assert_eq!(runtime.tooltip(), None);
    }

    #[test]
    fn submission_contains_surface_resources_and_monotonic_frame_id() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
            .unwrap();

        let first = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(first.frame_id, FrameId::INITIAL);
        assert_eq!(first.surface, surface(100, 100));
        assert_eq!(first.resources.added.len(), 1);
        assert_eq!(first.resources.added[0].id, image.resource_id());
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
    fn resource_only_completion_produces_a_submission_without_scene_damage() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 0, 0], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
        assert!(resource_only.commands.is_empty());
        assert_eq!(resource_only.resources.added.len(), 1);
        assert_eq!(resource_only.resources.added[0].id, image.resource_id());
    }

    #[test]
    fn resource_completion_invalidates_and_repaints_dependent_figure() {
        let mut runtime = Runtime::empty();
        let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            runtime.add_figure(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), child)
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(2, 2, vec![255; 2 * 2 * 4], 1.0))
            .unwrap();

        assert_eq!(
            runtime.resource_status(image.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
        assert!(runtime.has_pending_update());
        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(submission.resources.added[0].id, image.resource_id());
        assert_ne!(submission.damage.mode(), DamageMode::None);
    }

    #[test]
    fn replacing_contents_removes_resource_dependencies_from_detached_figures() {
        let mut runtime = Runtime::empty();
        let old_contents =
            runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let image = runtime.register_image();
        runtime
            .add_resource_dependency(image.resource_id(), old_contents)
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(replacement.frame_id, RenderOutcome::Presented);
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
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
        let image = runtime.register_image();
        runtime
            .complete_image(
                image,
                ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
            )
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert!(runtime.complete_submission(initial.frame_id, RenderOutcome::Retry));

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert_eq!(retry.resources.added.len(), 1);
        assert_eq!(retry.resources.added[0].id, image.resource_id());
        runtime.complete_submission(retry.frame_id, RenderOutcome::Presented);

        let resized = runtime
            .prepare_submission(surface(120, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        assert_eq!(resized.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn retry_restores_in_flight_resource_updates_before_newer_updates() {
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime.complete_submission(initial.frame_id, RenderOutcome::Presented);

        let image = runtime.register_image();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let first_update = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        runtime.complete_submission(first_update.frame_id, RenderOutcome::Retry);

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .unwrap();
        let revisions: Vec<_> = retry
            .resources
            .added
            .iter()
            .map(|update| update.revision)
            .collect();

        assert_eq!(revisions, vec![1, 2]);
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
        let mut tree = FigureTree::new();
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
