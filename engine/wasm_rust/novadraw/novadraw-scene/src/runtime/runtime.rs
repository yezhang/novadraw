use novadraw_render::NdCanvas;

use crate::{
    BasicEventDispatcher, EventDispatcher, Figure, FigureGraph, FigureId, InteractionState, Key,
    KeyModifiers, MouseButton, PendingMutations, SceneDispatchContext, SceneUpdateManager,
    UpdateListener, WheelEvent, ZoomEvent,
};

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
///
/// `FigureGraph` remains accepted as the compatibility tree implementation while
/// callers migrate to the `FigureTree` name.
pub struct Runtime {
    tree: FigureGraph,
    interaction: InteractionState,
    interaction_dispatcher: BasicEventDispatcher,
    updates: SceneUpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
}

impl Runtime {
    pub fn new(tree: FigureGraph) -> Self {
        Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: BasicEventDispatcher,
            updates: SceneUpdateManager::new(),
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
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
        self.tree.set_bounds_with_update(
            &mut self.updates,
            id,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )
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

    pub fn into_tree(self) -> FigureGraph {
        self.tree
    }

    pub fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) {
        self.updates.add_listener(listener);
    }

    pub fn has_pending_update(&self) -> bool {
        self.full_redraw_pending || self.updates.is_update_queued()
    }

    pub fn request_full_redraw(&mut self) {
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
    use crate::RectangleFigure;
    use novadraw_core::Color;

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
}
