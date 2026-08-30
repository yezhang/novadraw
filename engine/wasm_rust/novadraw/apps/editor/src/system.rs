use std::{sync::Arc, time::Duration};

use novadraw::{
    BasicEventDispatcher, BlockId, EventDispatcher, FigureEvent, Key, KeyModifiers, MouseButton,
    NdCanvas, NovadrawSystem, PendingMutations, RenderBackend, SceneDispatchContext, SceneHost,
    SceneUpdateManager, UpdateEvent, UpdateListener, WheelEvent, ZoomEvent,
    backend::vello::WinitWindowProxy,
};

use crate::scene_manager::{SceneManager, scene_host::WinitSceneHost};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPointerInput {
    pub physical_x: f64,
    pub physical_y: f64,
    pub scale_factor: f64,
}

impl RawPointerInput {
    pub fn new(physical_x: f64, physical_y: f64, scale_factor: f64) -> Self {
        Self {
            physical_x,
            physical_y,
            scale_factor,
        }
    }

    pub fn logical_position(self) -> LogicalPointerPosition {
        let scale_factor = if self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        // Winit 输入是窗口物理像素；分发到场景前统一转换到入口节点坐标域使用的逻辑坐标。
        LogicalPointerPosition {
            x: self.physical_x / scale_factor,
            y: self.physical_y / scale_factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPointerPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionTrace {
    pub phase: &'static str,
    pub raw: Option<RawPointerInput>,
    pub logical: LogicalPointerPosition,
    pub button: Option<MouseButton>,
    pub hit_target_before: Option<BlockId>,
    pub mouse_target_before: Option<BlockId>,
    pub mouse_target_after: Option<BlockId>,
    pub focus_owner_after: Option<BlockId>,
    pub captured_after: Option<BlockId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionStep {
    #[cfg(test)]
    Move(RawPointerInput),
    #[cfg(test)]
    Press {
        input: RawPointerInput,
        button: MouseButton,
    },
    #[cfg(test)]
    Release {
        input: RawPointerInput,
        button: MouseButton,
    },
    Hover {
        input: RawPointerInput,
        duration_ms: u64,
    },
    Click {
        input: RawPointerInput,
        button: MouseButton,
    },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InteractionReport {
    pub traces: Vec<InteractionTrace>,
}

pub struct EditorInteractionCore {
    scene_manager: SceneManager,
    update_manager: SceneUpdateManager,
    dispatcher: BasicEventDispatcher,
    pending_mutations: PendingMutations,
}

impl Default for EditorInteractionCore {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorInteractionCore {
    pub fn new() -> Self {
        let mut update_manager = SceneUpdateManager::new();

        update_manager.add_listener(Box::new(TraceUpdateListener));

        Self {
            scene_manager: SceneManager::new(),
            update_manager,
            dispatcher: BasicEventDispatcher,
            pending_mutations: PendingMutations::new(),
        }
    }

    pub fn scene_manager_mut(&mut self) -> &mut SceneManager {
        &mut self.scene_manager
    }

    fn build_trace(
        &self,
        phase: &'static str,
        raw: Option<RawPointerInput>,
        logical: LogicalPointerPosition,
        button: Option<MouseButton>,
    ) -> InteractionTrace {
        InteractionTrace {
            phase,
            raw,
            logical,
            button,
            hit_target_before: self
                .scene_manager
                .scene
                .find_mouse_event_target_at(logical.x, logical.y),
            mouse_target_before: self.scene_manager.scene.mouse_target(),
            mouse_target_after: None,
            focus_owner_after: None,
            captured_after: None,
        }
    }

    fn finish_trace(&self, trace: &mut InteractionTrace) {
        trace.mouse_target_after = self.scene_manager.scene.mouse_target();
        trace.focus_owner_after = self.scene_manager.scene.focus_owner();
        trace.captured_after = self.scene_manager.scene.captured();
    }

    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.dispatch_mouse_moved(&mut ctx, x, y);
        self.apply_pending_mutations();
    }

    pub fn dispatch_mouse_pressed(&mut self, x: f64, y: f64, button: MouseButton) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher
            .dispatch_mouse_pressed(&mut ctx, x, y, button);
        self.apply_pending_mutations();
    }

    pub fn dispatch_mouse_released(&mut self, x: f64, y: f64, button: MouseButton) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher
            .dispatch_mouse_released(&mut ctx, x, y, button);
        self.apply_pending_mutations();
    }

    pub fn dispatch_mouse_double_clicked(&mut self, x: f64, y: f64, button: MouseButton) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher
            .dispatch_mouse_double_clicked(&mut ctx, x, y, button);
        self.apply_pending_mutations();
    }

    pub fn dispatch_mouse_hover(&mut self, x: f64, y: f64) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.dispatch_mouse_hover(&mut ctx, x, y);
        self.apply_pending_mutations();
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.dispatch_scroll(&mut ctx, event);
        self.apply_pending_mutations();
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.dispatch_zoom(&mut ctx, event);
        self.apply_pending_mutations();
    }

    pub fn cancel_gestures(&mut self) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.cancel_gestures(&mut ctx);
    }

    pub fn dispatch_key_pressed(&mut self, key: Key, modifiers: KeyModifiers) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher
            .dispatch_key_pressed(&mut ctx, key, modifiers);
        self.apply_pending_mutations();
    }

    pub fn dispatch_key_released(&mut self, key: Key, modifiers: KeyModifiers) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher
            .dispatch_key_released(&mut ctx, key, modifiers);
        self.apply_pending_mutations();
    }

    pub fn release_focus(&mut self) {
        let mut ctx = SceneDispatchContext::new(
            &mut self.scene_manager.scene,
            &mut self.update_manager,
            &mut self.pending_mutations,
        );
        self.dispatcher.release_focus(&mut ctx);
        self.apply_pending_mutations();
    }

    pub fn dispatch_raw_mouse_moved(&mut self, input: RawPointerInput) -> InteractionTrace {
        let logical = input.logical_position();
        let mut trace = self.build_trace("move", Some(input), logical, None);
        self.dispatch_mouse_moved(logical.x, logical.y);
        self.finish_trace(&mut trace);
        trace
    }

    pub fn dispatch_raw_mouse_pressed(
        &mut self,
        input: RawPointerInput,
        button: MouseButton,
    ) -> InteractionTrace {
        let logical = input.logical_position();
        let mut trace = self.build_trace("press", Some(input), logical, Some(button));
        self.dispatch_mouse_pressed(logical.x, logical.y, button);
        self.finish_trace(&mut trace);
        trace
    }

    pub fn dispatch_raw_mouse_released(
        &mut self,
        input: RawPointerInput,
        button: MouseButton,
    ) -> InteractionTrace {
        let logical = input.logical_position();
        let mut trace = self.build_trace("release", Some(input), logical, Some(button));
        self.dispatch_mouse_released(logical.x, logical.y, button);
        self.finish_trace(&mut trace);
        trace
    }

    pub fn run_interaction_script(&mut self, steps: &[InteractionStep]) -> InteractionReport {
        let mut report = InteractionReport::default();
        for step in steps {
            match *step {
                #[cfg(test)]
                InteractionStep::Move(input) => {
                    report.traces.push(self.dispatch_raw_mouse_moved(input));
                }
                #[cfg(test)]
                InteractionStep::Press { input, button } => {
                    report
                        .traces
                        .push(self.dispatch_raw_mouse_pressed(input, button));
                }
                #[cfg(test)]
                InteractionStep::Release { input, button } => {
                    report
                        .traces
                        .push(self.dispatch_raw_mouse_released(input, button));
                }
                InteractionStep::Hover { input, duration_ms } => {
                    report.traces.push(self.dispatch_raw_mouse_moved(input));
                    std::thread::sleep(Duration::from_millis(duration_ms));
                    let logical = input.logical_position();
                    self.dispatch_mouse_hover(logical.x, logical.y);
                }
                InteractionStep::Click { input, button } => {
                    report
                        .traces
                        .push(self.dispatch_raw_mouse_pressed(input, button));
                    report
                        .traces
                        .push(self.dispatch_raw_mouse_released(input, button));
                }
            }
        }
        report
    }

    fn apply_pending_mutations(&mut self) -> bool {
        let mutations = self.pending_mutations.drain();
        self.scene_manager
            .scene
            .apply_pending_mutations(&mut self.update_manager, mutations)
    }
}

pub struct WinitNovadrawSystem {
    core: EditorInteractionCore,
    scene_host: WinitSceneHost,
}

impl WinitNovadrawSystem {
    pub fn new(window_proxy: Arc<WinitWindowProxy>) -> Self {
        Self {
            core: EditorInteractionCore::new(),
            scene_host: WinitSceneHost::new(window_proxy),
        }
    }

    pub fn is_scene(&self, scene_type: crate::scene_manager::SceneType) -> bool {
        self.core.scene_manager.current_scene == scene_type
    }

    pub fn switch_scene(&mut self, scene_type: crate::scene_manager::SceneType) {
        self.core.scene_manager_mut().switch_scene(scene_type);
        self.core.update_manager.clear();
        self.scene_host.request_update();
    }

    pub fn translate_contents_if_scene(
        &mut self,
        scene_type: crate::scene_manager::SceneType,
        dx: f64,
        dy: f64,
    ) -> bool {
        if !self.is_scene(scene_type) {
            return false;
        }
        self.translate_contents(dx, dy)
    }

    pub fn translate_contents(&mut self, dx: f64, dy: f64) -> bool {
        if let Some(root_id) = self.core.scene_manager.scene().get_contents() {
            self.core
                .scene_manager_mut()
                .scene_mut()
                .prim_translate(root_id, dx, dy);
            self.scene_host.request_update();
            true
        } else {
            false
        }
    }

    fn schedule_update_if_transitioned(&self, was_queued: bool) {
        if !was_queued && self.core.update_manager.is_update_queued() {
            self.scene_host.request_update();
        }
    }

    fn run_update_transaction<R>(&mut self, f: impl FnOnce(&mut EditorInteractionCore) -> R) -> R {
        let was_queued = self.core.update_manager.is_update_queued();
        let result = f(&mut self.core);
        self.schedule_update_if_transitioned(was_queued);
        result
    }

    pub fn dispatch_raw_mouse_moved(&mut self, input: RawPointerInput) -> InteractionTrace {
        self.run_update_transaction(|core| core.dispatch_raw_mouse_moved(input))
    }

    pub fn dispatch_raw_mouse_pressed(
        &mut self,
        input: RawPointerInput,
        button: MouseButton,
    ) -> InteractionTrace {
        self.run_update_transaction(|core| core.dispatch_raw_mouse_pressed(input, button))
    }

    pub fn dispatch_raw_mouse_released(
        &mut self,
        input: RawPointerInput,
        button: MouseButton,
    ) -> InteractionTrace {
        self.run_update_transaction(|core| core.dispatch_raw_mouse_released(input, button))
    }

    pub fn dispatch_raw_mouse_double_clicked(
        &mut self,
        input: RawPointerInput,
        button: MouseButton,
    ) {
        let logical = input.logical_position();
        self.run_update_transaction(|core| {
            core.dispatch_mouse_double_clicked(logical.x, logical.y, button)
        });
    }

    pub fn dispatch_scroll(&mut self, event: WheelEvent) {
        self.run_update_transaction(|core| core.dispatch_scroll(event));
    }

    pub fn dispatch_zoom(&mut self, event: ZoomEvent) {
        self.run_update_transaction(|core| core.dispatch_zoom(event));
    }

    pub fn cancel_gestures(&mut self) {
        self.run_update_transaction(EditorInteractionCore::cancel_gestures);
    }

    pub fn dispatch_key_pressed(&mut self, key: Key, modifiers: KeyModifiers) {
        self.run_update_transaction(|core| core.dispatch_key_pressed(key, modifiers));
    }

    pub fn dispatch_key_released(&mut self, key: Key, modifiers: KeyModifiers) {
        self.run_update_transaction(|core| core.dispatch_key_released(key, modifiers));
    }

    pub fn release_focus(&mut self) {
        self.run_update_transaction(EditorInteractionCore::release_focus);
    }

    pub fn run_interaction_script(&mut self, steps: &[InteractionStep]) -> InteractionReport {
        self.run_update_transaction(|core| core.run_interaction_script(steps))
    }

    pub fn request_update(&self) {
        self.scene_host.request_update();
    }
}

impl NovadrawSystem for WinitNovadrawSystem {
    fn render(&mut self, renderer: &mut impl RenderBackend) -> NdCanvas {
        self.scene_host.execute_update(
            &mut self.core.scene_manager.scene,
            &mut self.core.update_manager,
            renderer,
        )
    }

    fn viewport_size(&self) -> (f64, f64) {
        self.scene_host.viewport_size()
    }

    fn request_update(&self) {
        self.scene_host.request_update();
    }
}

struct TraceUpdateListener;

impl UpdateListener for TraceUpdateListener {
    fn on_update_event(&self, event: UpdateEvent) {
        tracing::info!("[Notification] update event: {:?}", event);
    }

    fn on_figure_event(&self, event: FigureEvent) {
        tracing::info!("[Notification] figure event: {:?}", event);
    }

    fn on_notify(&self, block_id: BlockId) {
        tracing::info!("[Notification] notify: {:?}", block_id);
    }
}

#[cfg(test)]
mod tests {
    use novadraw::{
        Bounded, Color, FigureGraph, MouseEvent, NdCanvas, NovadrawContext, Rectangle,
        RenderCommandKind, Shape, Updatable,
        command::{LineCap, LineJoin},
    };

    use super::*;
    use crate::scene_manager::SceneType;

    struct TestInteractiveFigure {
        bounds: Rectangle,
    }

    impl TestInteractiveFigure {
        fn new(bounds: Rectangle) -> Self {
            Self { bounds }
        }
    }

    impl Bounded for TestInteractiveFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "TestInteractiveFigure"
        }
    }

    impl Updatable for TestInteractiveFigure {
        fn validate(&mut self) {}

        fn invalidate(&mut self) {}
    }

    impl Shape for TestInteractiveFigure {
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

        fn fill_shape(&self, _gc: &mut NdCanvas) {}

        fn outline_shape(&self, _gc: &mut NdCanvas) {}

        fn wants_mouse_events(&self) -> bool {
            true
        }

        fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
            if event.button == MouseButton::Left {
                ctx.select_target();
            }
            true
        }

        fn on_mouse_released(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
            true
        }

        fn on_mouse_entered(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
            true
        }

        fn on_mouse_exited(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
            true
        }
    }

    fn build_test_core() -> (EditorInteractionCore, BlockId) {
        let mut scene = FigureGraph::new();
        let root_id = scene.set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            400.0,
            300.0,
            Color::rgba(0.0, 0.0, 0.0, 0.0),
        )));
        let target_id = scene.add_child_to(
            root_id,
            Box::new(TestInteractiveFigure::new(Rectangle::new(
                100.0, 100.0, 100.0, 100.0,
            ))),
        );
        let mut core = EditorInteractionCore::new();
        core.scene_manager = SceneManager {
            scene,
            current_scene: SceneType::DpiTest,
        };
        (core, target_id)
    }

    fn build_coordinate_root_test_core() -> (EditorInteractionCore, BlockId) {
        let mut scene = FigureGraph::new();
        let root_id = scene.set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            400.0,
            300.0,
            Color::rgba(0.0, 0.0, 0.0, 0.0),
        )));
        let coordinate_root_id = scene.add_child_to(
            root_id,
            Box::new(
                novadraw::RectangleFigure::new_with_color(
                    100.0,
                    50.0,
                    200.0,
                    150.0,
                    Color::rgba(0.2, 0.2, 0.2, 0.0),
                )
                .with_local_coordinates(true),
            ),
        );
        let target_id = scene.add_child_to(
            coordinate_root_id,
            Box::new(TestInteractiveFigure::new(Rectangle::new(
                20.0, 30.0, 40.0, 40.0,
            ))),
        );
        let mut core = EditorInteractionCore::new();
        core.scene_manager = SceneManager {
            scene,
            current_scene: SceneType::DpiTest,
        };
        (core, target_id)
    }

    const TEST_SELECTION_OUTLINE_COLOR: Color = Color {
        r: 0.98,
        g: 0.86,
        b: 0.22,
        a: 1.0,
    };
    const TEST_SELECTION_OUTLINE_STROKE_WIDTH: f64 = 4.0;

    fn has_selection_stroke(canvas: &NdCanvas) -> bool {
        canvas.commands().iter().any(|command| match &command.kind {
            RenderCommandKind::StrokeRect { color, width, .. } => {
                *color == TEST_SELECTION_OUTLINE_COLOR
                    && (*width - TEST_SELECTION_OUTLINE_STROKE_WIDTH).abs() < f64::EPSILON
            }
            _ => false,
        })
    }

    #[test]
    fn test_raw_pointer_conversion() {
        let logical = RawPointerInput::new(300.0, 200.0, 2.0).logical_position();
        assert_eq!(logical, LogicalPointerPosition { x: 150.0, y: 100.0 });
    }

    #[test]
    fn test_hover_script_hits_expected_target() {
        let (mut core, target_id) = build_test_core();
        let report = core.run_interaction_script(&[InteractionStep::Hover {
            input: RawPointerInput::new(300.0, 300.0, 2.0),
            duration_ms: 0,
        }]);

        assert_eq!(report.traces.len(), 1);
        assert_eq!(
            report.traces[0].logical,
            LogicalPointerPosition { x: 150.0, y: 150.0 }
        );
        assert_eq!(report.traces[0].hit_target_before, Some(target_id));
        assert_eq!(report.traces[0].mouse_target_after, Some(target_id));
        assert!(core.scene_manager.scene.is_hovered(target_id));
        assert!(!core.scene_manager.scene.is_pressed(target_id));
        assert!(!core.scene_manager.scene.is_selected(target_id));
    }

    #[test]
    fn test_click_script_updates_graph_interaction_state() {
        let (mut core, target_id) = build_test_core();
        let report = core.run_interaction_script(&[InteractionStep::Click {
            input: RawPointerInput::new(300.0, 300.0, 2.0),
            button: MouseButton::Left,
        }]);

        assert_eq!(report.traces.len(), 2);
        assert_eq!(report.traces[0].hit_target_before, Some(target_id));
        assert_eq!(report.traces[1].hit_target_before, Some(target_id));
        assert_eq!(report.traces[1].mouse_target_after, Some(target_id));
        assert!(core.scene_manager.scene.is_hovered(target_id));
        assert!(!core.scene_manager.scene.is_pressed(target_id));
        assert!(core.scene_manager.scene.is_selected(target_id));
    }

    #[test]
    fn test_release_after_dragging_outside_still_reaches_pressed_target() {
        let (mut core, target_id) = build_test_core();
        let report = core.run_interaction_script(&[
            InteractionStep::Move(RawPointerInput::new(300.0, 300.0, 2.0)),
            InteractionStep::Press {
                input: RawPointerInput::new(300.0, 300.0, 2.0),
                button: MouseButton::Left,
            },
            InteractionStep::Move(RawPointerInput::new(20.0, 20.0, 2.0)),
            InteractionStep::Release {
                input: RawPointerInput::new(20.0, 20.0, 2.0),
                button: MouseButton::Left,
            },
        ]);

        assert_eq!(report.traces.len(), 4);
        assert_eq!(report.traces[1].captured_after, Some(target_id));
        assert_eq!(report.traces[2].mouse_target_after, Some(target_id));
        assert_eq!(report.traces[3].hit_target_before, None);
        assert_eq!(report.traces[3].mouse_target_before, Some(target_id));
        assert_eq!(report.traces[3].captured_after, None);
        assert_eq!(report.traces[3].mouse_target_after, None);
        assert!(!core.scene_manager.scene.is_hovered(target_id));
        assert!(!core.scene_manager.scene.is_pressed(target_id));
        assert!(core.scene_manager.scene.is_selected(target_id));
    }

    #[test]
    fn test_raw_pointer_dispatch_uses_entry_domain_point_through_coordinate_root() {
        let (mut core, target_id) = build_coordinate_root_test_core();
        let report = core.run_interaction_script(&[InteractionStep::Click {
            input: RawPointerInput::new(260.0, 180.0, 2.0),
            button: MouseButton::Left,
        }]);

        assert_eq!(report.traces.len(), 2);
        assert_eq!(
            report.traces[0].logical,
            LogicalPointerPosition { x: 130.0, y: 90.0 }
        );
        assert_eq!(report.traces[0].hit_target_before, Some(target_id));
        assert_eq!(report.traces[1].hit_target_before, Some(target_id));
        assert_eq!(report.traces[1].mouse_target_after, Some(target_id));
        assert!(core.scene_manager.scene.is_hovered(target_id));
        assert!(!core.scene_manager.scene.is_pressed(target_id));
        assert!(core.scene_manager.scene.is_selected(target_id));
    }

    #[test]
    fn test_selected_target_renders_highlight_overlay() {
        let (mut core, target_id) = build_test_core();
        core.scene_manager.scene.set_selected(Some(target_id));

        let canvas = core.scene_manager.scene.render();

        assert!(has_selection_stroke(&canvas));
    }
}
