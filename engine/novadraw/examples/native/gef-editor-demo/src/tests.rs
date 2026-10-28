use crate::{
    AppState, Session, UiCommand,
    model::{Diagram, FIRST, INITIAL_BOUNDS, ShapeFactory},
    session, sync,
};
use masonry_testing::TestHarnessParams;
#[path = "gpu_harness.rs"]
mod gpu_harness;
use gpu_harness::TestHarness;
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};
use novadraw_platform_masonry::{
    session::CanvasAction,
    view::{CanvasView, editor_canvas},
    widget::EditorCanvas,
};
use std::{fmt::Debug, sync::Arc};
use xilem::core::{
    DynMessage, Environment, MessageCtx, MessageResult, ProxyError, RawProxy, SendMessage, View,
    ViewId,
};
use xilem::masonry::core::{DefaultProperties, NewWidget, PointerButton, TextEvent, WindowEvent};
use xilem::masonry::kurbo::Size;
use xilem::{ViewCtx, core::ViewMarker};

type Canvas = EditorCanvas<Diagram, ShapeFactory>;
type DemoView = CanvasView<
    AppState,
    Diagram,
    ShapeFactory,
    fn(&mut AppState) -> Session,
    fn(&mut AppState, &mut Session) -> Result<bool, String>,
    fn(&mut AppState, CanvasAction),
>;

#[derive(Debug)]
struct NoProxy;
impl RawProxy for NoProxy {
    fn send_message(&self, _: Arc<[ViewId]>, _: SendMessage) -> Result<(), ProxyError> {
        panic!("canvas does not use asynchronous messages")
    }
    fn dyn_debug(&self) -> &dyn Debug {
        self
    }
}
fn view(disabled: bool) -> DemoView {
    fn build(_: &mut AppState) -> Session {
        session()
    }
    fn action(app: &mut AppState, action: CanvasAction) {
        app.canvas = action;
    }
    editor_canvas::<AppState, Diagram, ShapeFactory, _, _, _>(
        build as fn(&mut AppState) -> Session,
        sync as fn(&mut AppState, &mut Session) -> Result<bool, String>,
        action as fn(&mut AppState, CanvasAction),
    )
    .disabled(disabled)
}
fn context() -> ViewCtx {
    ViewCtx::new(
        Arc::new(NoProxy),
        Arc::new(tokio::runtime::Runtime::new().unwrap()),
    )
}
fn harness(scale: f64) -> TestHarness<Canvas> {
    let mut params = TestHarnessParams::DEFAULT;
    params.window_size =
        xilem::dpi::PhysicalSize::new((640.0 * scale) as u32, (480.0 * scale) as u32);
    params.scale_factor = scale;
    let mut harness = TestHarness::create_with(
        DefaultProperties::default(),
        NewWidget::new(Canvas::new(session())),
        params,
    );
    harness.animate_ms(1);
    harness
}
fn bounds(harness: &TestHarness<Canvas>) -> Rectangle {
    harness.root_widget().session().viewer.model().shapes[&FIRST].bounds
}
fn drag(harness: &mut TestHarness<Canvas>) {
    // Upstream mouse_move accepts logical coordinates and applies DPI itself.
    harness.mouse_move((90.0, 100.0));
    harness.mouse_button_press(PointerButton::Primary);
    assert_eq!(harness.pointer_capture_target_id(), Some(harness.root_id()));
    harness.mouse_move((140.0, 130.0));
    harness.mouse_button_release(PointerButton::Primary);
    assert_eq!(harness.pointer_capture_target_id(), None);
    harness.animate_ms(1);
}

#[test]
fn widget_drag_matches_headless_policy_and_one_history_entry() {
    let mut native = harness(1.0);
    drag(&mut native);
    assert_eq!(bounds(&native), Rectangle::new(110.0, 100.0, 120.0, 80.0));
    let mut headless = session();
    headless.render(Size::new(640.0, 480.0), 1.0).unwrap();
    headless
        .domain
        .pointer_pressed(
            &mut headless.viewer,
            Point::new(90.0, 100.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    headless
        .domain
        .pointer_moved(&mut headless.viewer, Point::new(140.0, 130.0))
        .unwrap();
    assert!(
        headless
            .domain
            .pointer_released(
                &mut headless.viewer,
                Point::new(140.0, 130.0),
                MouseButton::Left
            )
            .unwrap()
            .command_executed()
    );
    assert_eq!(
        bounds(&native),
        headless.viewer.model().shapes[&FIRST].bounds
    );
    let mut app = AppState::default();
    app.pending.push_back(UiCommand::Undo);
    let mut ctx = context();
    let descriptor = view(false);
    native.edit_root_widget(|widget| {
        descriptor.rebuild(&descriptor, &mut (), &mut ctx, widget, &mut app)
    });
    assert_eq!(bounds(&native), INITIAL_BOUNDS);
    assert!(!native.root_widget().session().domain.has_undo());
    assert!(native.root_widget().session().domain.has_redo());
}

#[test]
fn xilem_rebuild_preserves_namespaces_and_history_and_removes_action_source() {
    fn send_sync<T: Send + Sync + ViewMarker>() {}
    send_sync::<DemoView>();
    let mut ctx = context();
    let mut app = AppState::default();
    let descriptor = view(false);
    let (pod, mut state) = descriptor.build(&mut ctx, &mut app);
    let mut harness = TestHarness::create_with_size(
        DefaultProperties::default(),
        pod.new_widget,
        xilem::dpi::PhysicalSize::new(640, 480),
    );
    harness.animate_ms(1);
    let id = harness.root_id();
    let namespace = harness.root_widget().session().viewer.namespace();
    let runtime_namespace = harness
        .root_widget()
        .session()
        .viewer
        .runtime()
        .tree()
        .namespace();
    drag(&mut harness);
    let moved = bounds(&harness);
    app.pending.push_back(UiCommand::Undo);
    let next = view(false);
    harness.edit_root_widget(|widget| {
        next.rebuild(&descriptor, &mut state, &mut ctx, widget, &mut app)
    });
    assert_eq!(bounds(&harness), INITIAL_BOUNDS);
    app.pending.push_back(UiCommand::Redo);
    harness.edit_root_widget(|widget| next.rebuild(&next, &mut state, &mut ctx, widget, &mut app));
    assert_eq!(bounds(&harness), moved);
    // Rebuild again with no queued action: no duplicate command.
    harness.edit_root_widget(|widget| next.rebuild(&next, &mut state, &mut ctx, widget, &mut app));
    assert_eq!(bounds(&harness), moved);
    assert_eq!(harness.root_id(), id);
    assert_eq!(
        harness.root_widget().session().viewer.namespace(),
        namespace
    );
    assert_eq!(
        harness
            .root_widget()
            .session()
            .viewer
            .runtime()
            .tree()
            .namespace(),
        runtime_namespace
    );
    assert!(ctx.get_id_path(id).is_some());
    let mut message = MessageCtx::new(
        Environment::new(),
        vec![],
        DynMessage::new(harness.root_widget().session().snapshot(None)),
    );
    harness.edit_root_widget(|widget| {
        assert!(matches!(
            next.message(&mut state, &mut message, widget, &mut app),
            MessageResult::Action(())
        ));
    });
    assert!(app.canvas.can_undo);
    let mut wrong = MessageCtx::new(Environment::new(), vec![], DynMessage::new("wrong action"));
    harness.edit_root_widget(|widget| {
        assert!(matches!(
            next.message(&mut state, &mut wrong, widget, &mut app),
            MessageResult::Stale
        ));
    });
    harness.edit_root_widget(|widget| next.teardown(&mut state, &mut ctx, widget));
    assert!(ctx.get_id_path(id).is_none());
    let (fresh, _) = next.build(&mut ctx, &mut app);
    assert_ne!(fresh.new_widget.id(), id);
}

#[test]
fn disable_and_focus_loss_cancel_without_committing_model() {
    for disable in [true, false] {
        let mut harness = harness(1.0);
        harness.mouse_move((90.0, 100.0));
        harness.mouse_button_press(PointerButton::Primary);
        harness.mouse_move((140.0, 130.0));
        if disable {
            harness.edit_root_widget(|mut widget| widget.ctx.set_disabled(true));
        } else {
            harness.process_text_event(TextEvent::WindowFocusChange(false));
        }
        assert!(!harness.root_widget().session().domain.has_active_gesture());
        assert!(!harness.root_widget().session().domain.has_undo());
        assert_eq!(bounds(&harness), INITIAL_BOUNDS);
        harness.mouse_button_release(PointerButton::Primary);
        assert_eq!(bounds(&harness), INITIAL_BOUNDS);
        assert_eq!(harness.pointer_capture_target_id(), None);
    }
}

#[test]
fn create_delete_and_undo_use_same_policy_command_chain() {
    let mut controller = session();
    let mut app = AppState::default();
    app.pending.push_back(UiCommand::Ellipse);
    sync(&mut app, &mut controller).unwrap();
    assert_eq!(controller.viewer.model().shapes.len(), 3);
    let created = controller.viewer.part_for_model(3).unwrap();
    controller.viewer.replace_selection(created).unwrap();
    app.pending.push_back(UiCommand::Delete);
    sync(&mut app, &mut controller).unwrap();
    assert_eq!(controller.viewer.model().shapes.len(), 2);
    app.pending.push_back(UiCommand::Undo);
    sync(&mut app, &mut controller).unwrap();
    assert_eq!(controller.viewer.model().shapes.len(), 3);
    app.pending.push_back(UiCommand::Undo);
    sync(&mut app, &mut controller).unwrap();
    assert_eq!(controller.viewer.model().shapes.len(), 2);
}

#[test]
fn resize_and_dpi_changes_preserve_source_history_and_gpu_coordinates() {
    use xilem::winit::dpi::PhysicalSize;
    assert!(std::env::var("SKIP_RENDER_TESTS").is_err());
    let mut harness = harness(1.0);
    let namespace = harness.root_widget().session().viewer.namespace();
    let initial = harness.render();
    let fill = initial.get_pixel(70, 80).0;
    assert_ne!(fill, initial.get_pixel(20, 20).0);
    for (width, height, scale) in [
        (420.0, 320.0, 2.0),
        (280.0, 210.0, 1.5),
        (640.0, 480.0, 1.0),
    ] {
        let physical_size = PhysicalSize::new((width * scale) as u32, (height * scale) as u32);
        harness.process_window_event(WindowEvent::Rescale(scale));
        harness.process_window_event(WindowEvent::Resize(physical_size));
        harness.animate_ms(1);
        let image = harness.render();
        assert_eq!(
            image.dimensions(),
            (physical_size.width, physical_size.height)
        );
        assert_eq!(
            image
                .get_pixel((70.0 * scale) as u32, (80.0 * scale) as u32)
                .0,
            fill
        );
        assert_eq!(bounds(&harness), INITIAL_BOUNDS);
        assert!(!harness.root_widget().session().domain.has_undo());
        assert_eq!(
            harness.root_widget().session().viewer.namespace(),
            namespace
        );
        assert!(harness.root_widget().error().is_none());
    }
    // A resize while feedback is active must not commit the drag.
    harness.mouse_move((90.0, 100.0));
    harness.mouse_button_press(PointerButton::Primary);
    harness.mouse_move((140.0, 130.0));
    harness.process_window_event(WindowEvent::Resize(PhysicalSize::new(420, 320)));
    harness.animate_ms(1);
    assert_eq!(bounds(&harness), INITIAL_BOUNDS);
    assert!(!harness.root_widget().session().domain.has_undo());
    harness.mouse_button_release(PointerButton::Primary);
    harness.animate_ms(1);
    assert_eq!(bounds(&harness), Rectangle::new(110.0, 100.0, 120.0, 80.0));
    let descriptor = view(false);
    let mut app = AppState::default();
    let mut ctx = context();
    app.pending.push_back(UiCommand::Undo);
    harness.edit_root_widget(|widget| {
        descriptor.rebuild(&descriptor, &mut (), &mut ctx, widget, &mut app)
    });
    harness.animate_ms(1);
    assert_eq!(bounds(&harness), INITIAL_BOUNDS);
    assert!(!harness.root_widget().session().domain.has_undo());
    assert!(harness.root_widget().session().domain.has_redo());
    assert_eq!(harness.render().get_pixel(70, 80).0, fill);
}

#[test]
fn gpu_pixels_follow_drag_and_undo_at_two_dpi_scales() {
    assert!(
        std::env::var("SKIP_RENDER_TESTS").is_err(),
        "GPU evidence cannot be skipped"
    );
    for scale in [1.0, 2.0] {
        let mut harness = harness(scale);
        let initial = harness.render();
        if let Ok(directory) = std::env::var("NOVADRAW_EVIDENCE_DIR") {
            initial
                .save(std::path::Path::new(&directory).join(format!("initial-dpi-{scale}.png")))
                .unwrap();
        }
        assert!(
            harness.root_widget().error().is_none(),
            "{:?}",
            harness.root_widget().error()
        );
        assert_eq!(
            initial.dimensions(),
            ((640.0 * scale) as u32, (480.0 * scale) as u32)
        );
        let sample = |image: &image::RgbaImage, x: f64, y: f64| {
            image.get_pixel((x * scale) as u32, (y * scale) as u32).0
        };
        assert_ne!(sample(&initial, 70.0, 80.0), sample(&initial, 20.0, 20.0));
        drag(&mut harness);
        let moved = harness.render();
        assert_eq!(sample(&moved, 70.0, 80.0), sample(&initial, 20.0, 20.0));
        assert_eq!(sample(&moved, 130.0, 120.0), sample(&initial, 70.0, 80.0));
        let mut app = AppState::default();
        app.pending.push_back(UiCommand::Undo);
        let descriptor = view(false);
        let mut ctx = context();
        harness.edit_root_widget(|widget| {
            descriptor.rebuild(&descriptor, &mut (), &mut ctx, widget, &mut app)
        });
        harness.animate_ms(1);
        let restored = harness.render();
        // Selection feedback remains visible; compare interior and cleared old pixels.
        assert_eq!(sample(&restored, 70.0, 80.0), sample(&initial, 70.0, 80.0));
        assert_eq!(
            sample(&restored, 200.0, 130.0),
            sample(&initial, 200.0, 130.0)
        );
        harness.process_window_event(WindowEvent::Rescale(1.5));
        harness.animate_ms(1);
        assert!(harness.root_widget().error().is_none());
    }
}

#[test]
fn actual_toolbar_message_rebuild_and_gpu_composition() {
    use xilem::WidgetView;
    let mut app = AppState::default();
    let mut ctx = context();
    let mut descriptor = crate::app_view(&mut app).boxed();
    let (pod, mut state) = descriptor.build(&mut ctx, &mut app);
    let mut harness = TestHarness::create_with_size(
        xilem::masonry::theme::default_property_set(),
        pod.new_widget,
        xilem::dpi::PhysicalSize::new(800, 600),
    );
    harness.animate_ms(1);
    let mut canvas_id = None;
    let mut undo_label = None;
    harness.inspect_widgets(|widget| {
        if widget.downcast::<Canvas>().is_some() {
            canvas_id = Some(widget.id());
        }
        if widget.get_debug_text().as_deref() == Some("Undo") {
            undo_label = Some(widget.id());
        }
    });
    let canvas_id = canvas_id.unwrap();
    let canvas = harness
        .get_widget_with_id(canvas_id)
        .downcast::<Canvas>()
        .unwrap();
    let origin = canvas.ctx().to_window(xilem::kurbo::Point::ZERO);
    let namespace = canvas.session().viewer.namespace();
    harness.mouse_move((origin.x + 90.0, origin.y + 100.0));
    harness.mouse_button_press(PointerButton::Primary);
    harness.mouse_move((origin.x + 140.0, origin.y + 130.0));
    harness.mouse_button_release(PointerButton::Primary);
    harness.animate_ms(1);
    for phase in ["moved", "undo"] {
        // Route actions and rebuild exactly as the real Xilem driver does.
        let mut count = 0;
        while let Some((action, id)) = harness.pop_action_erased() {
            count += 1;
            assert!(count < 32, "action/rebuild feedback loop");
            let path = ctx.get_id_path(id).unwrap().clone();
            let mut message = MessageCtx::new(Environment::new(), path, DynMessage(action));
            harness.edit_root_widget(|widget| {
                assert!(matches!(
                    descriptor.message(&mut state, &mut message, widget, &mut app),
                    MessageResult::Action(()) | MessageResult::Nop
                ));
            });
            let next = crate::app_view(&mut app).boxed();
            harness.edit_root_widget(|widget| {
                next.rebuild(&descriptor, &mut state, &mut ctx, widget, &mut app)
            });
            descriptor = next;
        }
        harness.animate_ms(1);
        let image = harness.render();
        assert_eq!(image.dimensions(), (800, 600));
        let canvas = harness
            .get_widget_with_id(canvas_id)
            .downcast::<Canvas>()
            .unwrap();
        assert_eq!(canvas.session().viewer.namespace(), namespace);
        if phase == "moved" {
            assert_eq!(
                canvas.session().viewer.model().shapes[&FIRST].bounds.x,
                110.0
            );
            assert!(app.canvas.can_undo);
        } else {
            assert_eq!(
                canvas.session().viewer.model().shapes[&FIRST].bounds,
                INITIAL_BOUNDS
            );
            assert!(!app.canvas.can_undo);
            assert!(app.canvas.can_redo);
        }
        if let Ok(directory) = std::env::var("NOVADRAW_EVIDENCE_DIR") {
            image
                .save(std::path::Path::new(&directory).join(format!("toolbar-{phase}.png")))
                .unwrap();
        }
        if phase == "moved" {
            // Label is not interactive; click its center through normal hit testing.
            harness.mouse_move_to_unchecked(undo_label.unwrap());
            harness.mouse_button_press(PointerButton::Primary);
            harness.mouse_button_release(PointerButton::Primary);
        }
    }
}

#[test]
#[ignore = "release CPU/Scene cost sampling; no GPU presentation timing"]
fn input_and_scene_cost_sample() {
    use std::time::Instant;
    let mut harness = harness(1.0);
    let descriptor = view(false);
    let mut ctx = context();
    let mut app = AppState::default();
    let mut input = Vec::new();
    let mut scene = Vec::new();
    for index in 0..1020 {
        let start = Instant::now();
        harness.mouse_move((90.0, 100.0));
        harness.mouse_button_press(PointerButton::Primary);
        harness.mouse_move((140.0, 130.0));
        harness.mouse_button_release(PointerButton::Primary);
        let input_elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        harness.animate_ms(1);
        let scene_elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if index >= 20 {
            input.push(input_elapsed);
            scene.push(scene_elapsed);
        }
        app.pending.push_back(UiCommand::Undo);
        harness.edit_root_widget(|widget| {
            descriptor.rebuild(&descriptor, &mut (), &mut ctx, widget, &mut app)
        });
        harness.animate_ms(1);
        while harness.pop_action_erased().is_some() {}
        assert_eq!(bounds(&harness), INITIAL_BOUNDS);
        assert!(harness.root_widget().error().is_none());
    }
    for (name, mut samples) in [("four pointer events", input), ("Scene preparation", scene)] {
        samples.sort_by(f64::total_cmp);
        println!(
            "{name}: n={} p50={:.3}ms p95={:.3}ms max={:.3}ms",
            samples.len(),
            samples[499],
            samples[949],
            samples[999]
        );
    }
}
