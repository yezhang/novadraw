use std::sync::Arc;

use novadraw_geometry::Point;

use crate::{
    BlockId, DispatchContext, Event, Figure, FigureEvent, FigureGraph, GestureSessionId,
    InteractionState, MouseEventKind, MouseLocationZoomScrollPolicy, NotificationEffect,
    PendingMutations, PropertyChangeEvent, PropertyValue, Rectangle, ScalableLayeredPaneFigure,
    ScrollPaneFigure, UpdateManager, ViewportFigure, WheelEvent, ZoomEvent, ZoomManager,
    mutation::{MutationContext, PendingMutation},
};

pub trait NovadrawContext {
    fn target_id(&self) -> BlockId;
    fn repaint(&mut self, rect: Option<Rectangle>);
    fn repaint_figure(&mut self, block_id: BlockId, rect: Rectangle) {
        if block_id == self.target_id() {
            self.repaint(Some(rect));
        }
    }
    fn emit_property_change(
        &mut self,
        _block_id: BlockId,
        _property: &'static str,
        _old_value: PropertyValue,
        _new_value: PropertyValue,
    ) {
    }
    fn coordinate_system_changed(&mut self, _block_id: BlockId, _bounds: Rectangle) {}
    fn invalidate(&mut self);

    /// Requests selection changes through the dispatch context.
    ///
    /// Figure callbacks never mutate `FigureGraph` directly; the engine applies
    /// the request after the target callback returns.
    fn set_selected(&mut self, block_id: Option<BlockId>);

    fn select_target(&mut self) {
        self.set_selected(Some(self.target_id()));
    }

    /// Enqueues a structural mutation for application after top-level dispatch.
    fn add_child_later(&mut self, parent: BlockId, figure: Box<dyn Figure>);

    /// Enqueues a child removal for application after top-level dispatch.
    fn remove_child_later(&mut self, parent: BlockId, child: BlockId);

    /// Enqueues a reparent operation for application after top-level dispatch.
    fn reparent_later(&mut self, child: BlockId, new_parent: BlockId);
}

enum RuntimeEffect {
    Repaint { block_id: BlockId, rect: Rectangle },
    Notification(NotificationEffect),
    Invalidate(BlockId),
    Select(Option<BlockId>),
    Mutation(PendingMutation),
}

/// 引擎层通用的 Figure 回调上下文。
///
/// 只记录 callback effects；Runtime 在 Figure 借用释放后按顺序提交。
pub struct SceneNovadrawContext<'a> {
    target_id: BlockId,
    bounds: Rectangle,
    effects: &'a mut Vec<RuntimeEffect>,
}

impl<'a> SceneNovadrawContext<'a> {
    fn new(target_id: BlockId, bounds: Rectangle, effects: &'a mut Vec<RuntimeEffect>) -> Self {
        Self {
            target_id,
            bounds,
            effects,
        }
    }
}

impl NovadrawContext for SceneNovadrawContext<'_> {
    fn target_id(&self) -> BlockId {
        self.target_id
    }

    fn repaint(&mut self, rect: Option<Rectangle>) {
        self.effects.push(RuntimeEffect::Repaint {
            block_id: self.target_id,
            rect: rect.unwrap_or(self.bounds),
        });
    }

    fn repaint_figure(&mut self, block_id: BlockId, rect: Rectangle) {
        self.effects.push(RuntimeEffect::Repaint { block_id, rect });
    }

    fn emit_property_change(
        &mut self,
        block_id: BlockId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
    ) {
        self.effects.push(RuntimeEffect::Notification(
            NotificationEffect::EmitProperty(PropertyChangeEvent {
                block_id,
                property,
                old_value,
                new_value,
            }),
        ));
    }

    fn coordinate_system_changed(&mut self, block_id: BlockId, bounds: Rectangle) {
        self.effects
            .push(RuntimeEffect::Notification(NotificationEffect::EmitFigure(
                FigureEvent::CoordinateSystemChanged {
                    block_id,
                    old_bounds: bounds,
                    new_bounds: bounds,
                },
            )));
    }

    fn invalidate(&mut self) {
        self.effects.push(RuntimeEffect::Invalidate(self.target_id));
    }

    fn set_selected(&mut self, block_id: Option<BlockId>) {
        self.effects.push(RuntimeEffect::Select(block_id));
    }

    fn add_child_later(&mut self, parent: BlockId, figure: Box<dyn Figure>) {
        MutationContext::add_child_later(self, parent, figure);
    }

    fn remove_child_later(&mut self, parent: BlockId, child: BlockId) {
        MutationContext::remove_child_later(self, parent, child);
    }

    fn reparent_later(&mut self, child: BlockId, new_parent: BlockId) {
        MutationContext::reparent_later(self, child, new_parent);
    }
}

impl MutationContext for SceneNovadrawContext<'_> {
    fn enqueue_mutation(&mut self, mutation: PendingMutation) {
        self.effects.push(RuntimeEffect::Mutation(mutation));
    }
}

/// 引擎层通用的事件分发上下文。
///
/// Apps 只负责把平台输入转换为入口节点坐标域中的点；真正的 target 解析、
/// 坐标域切换与 Figure 回调调用都在引擎层统一处理。
pub struct SceneDispatchContext<'a> {
    scene: &'a mut FigureGraph,
    interaction: &'a mut InteractionState,
    update_manager: &'a mut dyn UpdateManager,
    pending_mutations: &'a mut PendingMutations,
}

impl<'a> SceneDispatchContext<'a> {
    pub fn new(
        scene: &'a mut FigureGraph,
        interaction: &'a mut InteractionState,
        update_manager: &'a mut dyn UpdateManager,
        pending_mutations: &'a mut PendingMutations,
    ) -> Self {
        Self {
            scene,
            interaction,
            update_manager,
            pending_mutations,
        }
    }

    fn nearest_scalable(&self, mut target_id: BlockId) -> Option<BlockId> {
        loop {
            let block = self.scene.block(target_id)?;
            if block.figure.as_any().is::<ScalableLayeredPaneFigure>() {
                return Some(target_id);
            }
            target_id = self.scene.parent_id(target_id)?;
        }
    }

    fn nearest_viewport_parent(&self, mut block_id: BlockId) -> Option<BlockId> {
        while let Some(parent_id) = self.scene.parent_id(block_id) {
            let parent = self.scene.block(parent_id)?;
            if parent.figure.as_any().is::<ViewportFigure>() {
                return Some(parent_id);
            }
            block_id = parent_id;
        }
        None
    }

    fn nearest_scroll_pane_parent(&self, mut block_id: BlockId) -> Option<BlockId> {
        while let Some(parent_id) = self.scene.parent_id(block_id) {
            let parent = self.scene.block(parent_id)?;
            if parent.figure.as_any().is::<ScrollPaneFigure>() {
                return Some(parent_id);
            }
            block_id = parent_id;
        }
        None
    }

    fn apply_scroll_controller(&mut self, target_id: BlockId, event: &WheelEvent) -> bool {
        let controller = if event.phase == crate::GesturePhase::Impulse
            || event.session_id == GestureSessionId::IMPULSE
        {
            self.nearest_scroll_pane_parent(target_id)
        } else if let Some(controller) = self.interaction.scroll_controller(event.session_id) {
            controller
        } else {
            let controller = self.nearest_scroll_pane_parent(target_id);
            self.interaction
                .pin_scroll_controller(event.session_id, controller)
        };
        let Some(scroll_pane_id) = controller else {
            return false;
        };
        DispatchContext::dispatch_to_target(self, Some(scroll_pane_id), &Event::Wheel(*event))
    }

    fn apply_zoom_manager(&mut self, target_id: BlockId, event: &ZoomEvent) -> bool {
        let scalable = if event.phase == crate::GesturePhase::Impulse
            || event.session_id == GestureSessionId::IMPULSE
        {
            self.nearest_scalable(target_id)
        } else if let Some(controller) = self.interaction.zoom_controller(event.session_id) {
            controller
        } else {
            let controller = self.nearest_scalable(target_id);
            self.interaction
                .pin_zoom_controller(event.session_id, controller)
        };
        let Some(scalable_id) = scalable else {
            return false;
        };
        let Some(viewport_id) = self.nearest_viewport_parent(scalable_id) else {
            return false;
        };
        let Some(scalable) = self.scene.scale_handle(scalable_id) else {
            return false;
        };
        let Some(viewport) = self.scene.viewport_handle(viewport_id) else {
            return false;
        };
        let anchor = {
            let Some(block) = self.scene.block(viewport_id) else {
                return false;
            };
            let (top, left, _, _) = block.figure.insets();
            let mut point = event.entry_point();
            if !self.scene.translate_to_relative(viewport_id, &mut point) {
                return false;
            }
            Point::new(point.x() - left, point.y() - top)
        };
        let mut zoom_manager = ZoomManager::new(scalable, viewport);
        zoom_manager.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));
        zoom_manager
            .zoom_by_at(
                self.scene,
                self.update_manager,
                event.scale_factor,
                Some(anchor),
            )
            .unwrap_or(false)
    }
}

impl DispatchContext for SceneDispatchContext<'_> {
    fn find_mouse_event_target_at(&self, x: f64, y: f64) -> Option<BlockId> {
        self.scene.find_mouse_event_target_at(x, y)
    }

    fn find_gesture_target_at(&self, x: f64, y: f64) -> Option<BlockId> {
        self.scene.hit_test_simple((x, y))
    }

    fn mouse_target(&self) -> Option<BlockId> {
        self.interaction.mouse_target()
    }

    fn set_mouse_target(&mut self, id: Option<BlockId>) {
        self.interaction.set_mouse_target(id);
    }

    fn cursor_target(&self) -> Option<BlockId> {
        self.interaction.cursor_target()
    }

    fn set_cursor_target(&mut self, id: Option<BlockId>) {
        self.interaction.set_cursor_target(id);
    }

    fn hover_source(&self) -> Option<BlockId> {
        self.interaction.hover_source()
    }

    fn set_hover_source(&mut self, id: Option<BlockId>) {
        self.interaction.set_hover_source(id);
    }

    fn set_hovered(&mut self, id: BlockId, hovered: bool) {
        if self.scene.get_block(id).is_some() {
            self.interaction.set_hovered(id, hovered);
        }
    }

    fn set_pressed(&mut self, id: BlockId, pressed: bool) {
        if self.scene.get_block(id).is_some() {
            self.interaction.set_pressed(id, pressed);
        }
    }

    fn focus_owner(&self) -> Option<BlockId> {
        self.interaction.focus_owner()
    }

    fn set_focus_owner(&mut self, id: Option<BlockId>) {
        self.interaction.set_focus_owner(id);
    }

    fn captured(&self) -> Option<BlockId> {
        self.interaction.captured()
    }

    fn set_captured(&mut self, id: Option<BlockId>) {
        self.interaction.set_captured(id);
    }

    fn gesture_target(&self, session_id: GestureSessionId) -> Option<BlockId> {
        self.interaction
            .gesture_target(session_id)
            .filter(|id| self.scene.get_block(*id).is_some())
    }

    fn has_gesture_session(&self, session_id: GestureSessionId) -> bool {
        self.interaction.has_gesture_session(session_id)
    }

    fn set_gesture_target(&mut self, session_id: GestureSessionId, target_id: Option<BlockId>) {
        let target_id = target_id.filter(|id| self.scene.get_block(*id).is_some());
        self.interaction.set_gesture_target(session_id, target_id);
    }

    fn clear_gesture_target(&mut self, session_id: GestureSessionId) {
        self.interaction.clear_gesture_target(session_id);
    }

    fn clear_gesture_targets(&mut self) {
        self.interaction.clear_gestures();
    }

    fn apply_scroll_fallback(&mut self, target_id: BlockId, event: &WheelEvent) -> bool {
        self.apply_scroll_controller(target_id, event)
    }

    fn apply_zoom_fallback(&mut self, target_id: BlockId, event: &ZoomEvent) -> bool {
        self.apply_zoom_manager(target_id, event)
    }

    fn wants_key_events(&self, target_id: BlockId) -> bool {
        self.scene.is_effectively_visible(target_id)
            && self.scene.is_effectively_enabled(target_id)
            && self
                .scene
                .block(target_id)
                .is_some_and(|block| block.figure.wants_key_events())
    }

    fn dispatch_to_target(&mut self, target_id: Option<BlockId>, event: &Event) -> bool {
        let Some(target_id) = target_id else {
            return false;
        };
        let Some(block) = self.scene.block(target_id) else {
            return false;
        };
        let mut effects = Vec::new();
        let handled = {
            let bounds = block.figure_bounds();
            let local_bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
            let mut ctx = SceneNovadrawContext::new(target_id, local_bounds, &mut effects);

            match event {
                Event::Mouse(mouse_event) => {
                    let mut point = Point::new(mouse_event.x, mouse_event.y);
                    if !self.scene.translate_to_relative(target_id, &mut point) {
                        return false;
                    }
                    let local_event = mouse_event.with_target_point(point.x(), point.y());
                    match local_event.kind {
                        MouseEventKind::Pressed => {
                            block.figure.on_mouse_pressed(&local_event, &mut ctx)
                        }
                        MouseEventKind::Released => {
                            block.figure.on_mouse_released(&local_event, &mut ctx)
                        }
                        MouseEventKind::Moved => {
                            block.figure.on_mouse_moved(&local_event, &mut ctx)
                        }
                        MouseEventKind::Dragged => {
                            block.figure.on_mouse_dragged(&local_event, &mut ctx)
                        }
                        MouseEventKind::Hover => {
                            block.figure.on_mouse_hover(&local_event, &mut ctx)
                        }
                        MouseEventKind::DoubleClicked => {
                            block.figure.on_mouse_double_clicked(&local_event, &mut ctx)
                        }
                        MouseEventKind::Entered => {
                            block.figure.on_mouse_entered(&local_event, &mut ctx)
                        }
                        MouseEventKind::Exited => {
                            block.figure.on_mouse_exited(&local_event, &mut ctx)
                        }
                    }
                }
                Event::Wheel(wheel_event) => {
                    let mut point = Point::new(wheel_event.x, wheel_event.y);
                    if !self.scene.translate_to_relative(target_id, &mut point) {
                        return false;
                    }
                    let local_event = wheel_event.with_target_point(point.x(), point.y());
                    block.figure.on_mouse_wheel(&local_event, &mut ctx)
                }
                Event::Zoom(zoom_event) => {
                    let mut point = Point::new(zoom_event.x, zoom_event.y);
                    if !self.scene.translate_to_relative(target_id, &mut point) {
                        return false;
                    }
                    let local_event = zoom_event.with_target_point(point.x(), point.y());
                    block.figure.on_zoom(&local_event, &mut ctx)
                }
                Event::Key(key_event) => match key_event.kind {
                    crate::event::KeyEventKind::Pressed => {
                        block.figure.on_key_pressed(key_event, &mut ctx)
                    }
                    crate::event::KeyEventKind::Released => {
                        block.figure.on_key_released(key_event, &mut ctx)
                    }
                },
                Event::Focus(focus_event) => match focus_event.kind {
                    crate::event::FocusEventKind::Gained => {
                        block.figure.on_focus_gained(focus_event, &mut ctx)
                    }
                    crate::event::FocusEventKind::Lost => {
                        block.figure.on_focus_lost(focus_event, &mut ctx)
                    }
                },
            }
        };

        for effect in effects {
            match effect {
                RuntimeEffect::Repaint { block_id, rect } => {
                    self.update_manager.add_dirty_region(block_id, rect);
                }
                RuntimeEffect::Notification(effect) => {
                    self.update_manager.enqueue_notification_effect(effect);
                }
                RuntimeEffect::Invalidate(block_id) => {
                    self.scene.mark_invalid(self.update_manager, block_id);
                }
                RuntimeEffect::Select(selected) => {
                    let previous = self.scene.selected_block();
                    if previous != selected {
                        let previous_bounds = previous.and_then(|id| self.scene.figure_bounds(id));
                        let selected_bounds = selected.and_then(|id| self.scene.figure_bounds(id));
                        self.scene.set_selected(selected);
                        if let (Some(id), Some(bounds)) = (previous, previous_bounds) {
                            self.update_manager.add_dirty_region(
                                id,
                                Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
                            );
                        }
                        if let (Some(id), Some(bounds)) = (selected, selected_bounds) {
                            self.update_manager.add_dirty_region(
                                id,
                                Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
                            );
                        }
                    }
                }
                RuntimeEffect::Mutation(mutation) => self.pending_mutations.enqueue(mutation),
            }
        }

        handled
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use novadraw_core::Color;
    use novadraw_render::command::{LineCap, LineJoin};

    use super::*;
    use crate::{
        BasicEventDispatcher, Bounded, EventDispatcher, MouseButton, MouseEvent, RectangleFigure,
        Shape, Updatable,
    };

    struct EnqueueChildFigure {
        bounds: Rectangle,
    }

    impl Bounded for EnqueueChildFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "EnqueueChildFigure"
        }
    }

    impl Updatable for EnqueueChildFigure {
        fn validate(&mut self) {}

        fn invalidate(&mut self) {}
    }

    impl Shape for EnqueueChildFigure {
        fn stroke_color(&self) -> Option<Color> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<Color> {
            None
        }

        fn line_cap(&self) -> LineCap {
            LineCap::default()
        }

        fn line_join(&self) -> LineJoin {
            LineJoin::default()
        }

        fn fill_shape(&self, _gc: &mut novadraw_render::NdCanvas) {}

        fn outline_shape(&self, _gc: &mut novadraw_render::NdCanvas) {}

        fn wants_mouse_events(&self) -> bool {
            true
        }

        fn on_mouse_pressed(&self, _event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
            ctx.invalidate();
            ctx.add_child_later(
                ctx.target_id(),
                Box::new(RectangleFigure::new(5.0, 5.0, 10.0, 10.0)),
            );
            true
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq)]
    struct RecordedMousePoint {
        x: f64,
        y: f64,
        entry_x: f64,
        entry_y: f64,
    }

    struct RecordingFigure {
        bounds: Rectangle,
        last_mouse_point: Arc<Mutex<Option<RecordedMousePoint>>>,
    }

    impl RecordingFigure {
        fn new(
            bounds: Rectangle,
            _legacy_local_coordinates: bool,
            last_mouse_point: Arc<Mutex<Option<RecordedMousePoint>>>,
        ) -> Self {
            Self {
                bounds,
                last_mouse_point,
            }
        }
    }

    impl Bounded for RecordingFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "RecordingFigure"
        }
    }

    impl Updatable for RecordingFigure {
        fn validate(&mut self) {}

        fn invalidate(&mut self) {}
    }

    impl Shape for RecordingFigure {
        fn stroke_color(&self) -> Option<Color> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<Color> {
            None
        }

        fn line_cap(&self) -> LineCap {
            LineCap::default()
        }

        fn line_join(&self) -> LineJoin {
            LineJoin::default()
        }

        fn fill_shape(&self, _gc: &mut novadraw_render::NdCanvas) {}

        fn outline_shape(&self, _gc: &mut novadraw_render::NdCanvas) {}

        fn wants_mouse_events(&self) -> bool {
            true
        }

        fn on_mouse_pressed(&self, event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
            let entry_point = event.entry_point();
            *self.last_mouse_point.lock().unwrap() = Some(RecordedMousePoint {
                x: event.x,
                y: event.y,
                entry_x: entry_point.x(),
                entry_y: entry_point.y(),
            });
            true
        }
    }

    #[test]
    fn test_scene_dispatch_context_translates_mouse_point_to_target_coordinate_domain() {
        let recorded = Arc::new(Mutex::new(None));
        let mut scene = FigureGraph::new();
        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
        let coordinate_root_id = scene.add_child_to(
            contents_id,
            Box::new(RectangleFigure::new_with_color(
                100.0,
                50.0,
                200.0,
                150.0,
                Color::WHITE,
            )),
        );
        scene.add_child_to(
            coordinate_root_id,
            Box::new(RecordingFigure::new(
                Rectangle::new(20.0, 30.0, 40.0, 40.0),
                false,
                Arc::clone(&recorded),
            )),
        );

        let mut update_manager = crate::SceneUpdateManager::new();
        let mut interaction = InteractionState::default();
        let mut pending_mutations = PendingMutations::new();
        let mut dispatcher = BasicEventDispatcher;
        let mut ctx = SceneDispatchContext::new(
            &mut scene,
            &mut interaction,
            &mut update_manager,
            &mut pending_mutations,
        );

        dispatcher.dispatch_mouse_pressed(&mut ctx, 130.0, 90.0, MouseButton::Left);

        assert_eq!(
            *recorded.lock().unwrap(),
            Some(RecordedMousePoint {
                x: 10.0,
                y: 10.0,
                entry_x: 130.0,
                entry_y: 90.0,
            })
        );
    }

    #[test]
    fn test_scene_dispatch_context_uses_target_local_coordinate_domain() {
        let recorded = Arc::new(Mutex::new(None));
        let mut scene = FigureGraph::new();
        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
        let coordinate_root_id = scene.add_child_to(
            contents_id,
            Box::new(RectangleFigure::new_with_color(
                100.0,
                50.0,
                200.0,
                150.0,
                Color::WHITE,
            )),
        );
        scene.add_child_to(
            coordinate_root_id,
            Box::new(RecordingFigure::new(
                Rectangle::new(20.0, 30.0, 40.0, 40.0),
                true,
                Arc::clone(&recorded),
            )),
        );

        let mut update_manager = crate::SceneUpdateManager::new();
        let mut interaction = InteractionState::default();
        let mut pending_mutations = PendingMutations::new();
        let mut dispatcher = BasicEventDispatcher;
        let mut ctx = SceneDispatchContext::new(
            &mut scene,
            &mut interaction,
            &mut update_manager,
            &mut pending_mutations,
        );

        dispatcher.dispatch_mouse_pressed(&mut ctx, 130.0, 90.0, MouseButton::Left);

        assert_eq!(
            *recorded.lock().unwrap(),
            Some(RecordedMousePoint {
                x: 10.0,
                y: 10.0,
                entry_x: 130.0,
                entry_y: 90.0,
            })
        );
    }

    #[test]
    fn test_scene_dispatch_context_defers_structure_mutation_until_after_callback() {
        let mut scene = FigureGraph::new();
        let parent_id = scene.set_contents(Box::new(EnqueueChildFigure {
            bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
        }));
        scene.revalidate(parent_id);
        assert!(scene.is_valid(parent_id));
        let mut update_manager = crate::SceneUpdateManager::new();
        let mut interaction = InteractionState::default();
        let mut pending_mutations = PendingMutations::new();
        let mut dispatcher = BasicEventDispatcher;
        let mut ctx = SceneDispatchContext::new(
            &mut scene,
            &mut interaction,
            &mut update_manager,
            &mut pending_mutations,
        );

        dispatcher.dispatch_mouse_pressed(&mut ctx, 10.0, 10.0, MouseButton::Left);

        assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 0);
        assert!(!scene.is_valid(parent_id));
        assert!(update_manager.has_pending_layout());
        assert!(scene.apply_pending_mutations(&mut update_manager, pending_mutations.drain()));
        assert_eq!(scene.get_block(parent_id).unwrap().children_count(), 1);
    }
}
