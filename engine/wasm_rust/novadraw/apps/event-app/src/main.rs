use std::sync::{Arc, Mutex};

use novadraw::{
    BasicEventDispatcher, Bounded, Color, EventDispatcher, Figure, FigureGraph, FocusEvent,
    FocusEventKind, InteractionState, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind, NdCanvas, NovadrawContext, PendingMutations, Rectangle,
    RectangleFigure, SceneDispatchContext, SceneUpdateManager, Updatable, WheelEvent,
};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

type SceneEntry = (&'static str, Box<dyn FnMut() -> FigureGraph>);

#[derive(Clone, Debug, PartialEq)]
enum ProbeEvent {
    Mouse(MouseEventKind, f64, f64),
    Wheel(f64, f64, f64, f64),
    Key(KeyEventKind, Key, KeyModifiers),
    Focus(FocusEventKind),
}

#[derive(Default)]
struct ProbeState {
    events: Vec<ProbeEvent>,
    hovered: bool,
    pressed: bool,
    focused: bool,
}

struct EventProbeFigure {
    bounds: Rectangle,
    state: Arc<Mutex<ProbeState>>,
}

impl EventProbeFigure {
    fn new(bounds: Rectangle, state: Arc<Mutex<ProbeState>>) -> Self {
        Self { bounds, state }
    }

    fn record_mouse(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) {
        let mut state = self.state.lock().unwrap();
        match event.kind {
            MouseEventKind::Entered => state.hovered = true,
            MouseEventKind::Exited => state.hovered = false,
            MouseEventKind::Pressed => state.pressed = true,
            MouseEventKind::Released => state.pressed = false,
            MouseEventKind::Moved
            | MouseEventKind::Dragged
            | MouseEventKind::Hover
            | MouseEventKind::DoubleClicked => {}
        }
        state
            .events
            .push(ProbeEvent::Mouse(event.kind, event.x, event.y));
        drop(state);
        ctx.repaint(None);
    }
}

impl Bounded for EventProbeFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "EventProbeFigure"
    }
}

impl Updatable for EventProbeFigure {
    fn validate(&mut self) {}
}

impl Figure for EventProbeFigure {
    fn paint_figure(&self, canvas: &mut NdCanvas) {
        let state = self.state.lock().unwrap();
        let color = if state.pressed {
            Color::hex("#e74c3c")
        } else if state.focused {
            Color::hex("#9b59b6")
        } else if state.hovered {
            Color::hex("#2ecc71")
        } else {
            Color::hex("#3498db")
        };
        canvas.fill_rect(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
            color,
        );
    }

    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn wants_key_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_released(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_moved(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_hover(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_double_clicked(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_entered(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_exited(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.state.lock().unwrap().events.push(ProbeEvent::Wheel(
            event.x,
            event.y,
            event.delta_x,
            event.delta_y,
        ));
        ctx.repaint(None);
        true
    }

    fn on_key_pressed(&self, event: &KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.state.lock().unwrap().events.push(ProbeEvent::Key(
            event.kind,
            event.key,
            event.modifiers,
        ));
        ctx.repaint(None);
        true
    }

    fn on_key_released(&self, event: &KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.on_key_pressed(event, ctx)
    }

    fn on_focus_gained(&self, event: &FocusEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let mut state = self.state.lock().unwrap();
        state.focused = true;
        state.events.push(ProbeEvent::Focus(event.kind));
        drop(state);
        ctx.repaint(None);
        true
    }

    fn on_focus_lost(&self, event: &FocusEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let mut state = self.state.lock().unwrap();
        state.focused = false;
        state.events.push(ProbeEvent::Focus(event.kind));
        drop(state);
        ctx.repaint(None);
        true
    }
}

fn probe_scene(local_coordinates: bool) -> (FigureGraph, Arc<Mutex<ProbeState>>) {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        Color::hex("#eeeeee"),
    )));
    let parent = if local_coordinates {
        graph.add_child_to(
            root,
            Box::new(
                RectangleFigure::new_with_color(100.0, 80.0, 500.0, 360.0, Color::hex("#dfe6e9"))
                    .with_local_coordinates(true),
            ),
        )
    } else {
        root
    };
    let state = Arc::new(Mutex::new(ProbeState::default()));
    graph.add_child_to(
        parent,
        Box::new(EventProbeFigure::new(
            if local_coordinates {
                Rectangle::new(40.0, 50.0, 220.0, 140.0)
            } else {
                Rectangle::new(250.0, 180.0, 300.0, 200.0)
            },
            state.clone(),
        )),
    );
    (graph, state)
}

fn pointer_scene() -> FigureGraph {
    probe_scene(false).0
}

fn coordinate_scene() -> FigureGraph {
    probe_scene(true).0
}

fn scenes() -> Vec<SceneEntry> {
    vec![
        ("pointer_capture", Box::new(pointer_scene)),
        ("focus_keyboard", Box::new(pointer_scene)),
        ("wheel_hover_double", Box::new(pointer_scene)),
        ("coordinate_root", Box::new(coordinate_scene)),
    ]
}

fn with_context(
    graph: &mut FigureGraph,
    interaction: &mut InteractionState,
    manager: &mut SceneUpdateManager,
    pending: &mut PendingMutations,
    action: impl FnOnce(&mut BasicEventDispatcher, &mut SceneDispatchContext<'_>),
) {
    let mut dispatcher = BasicEventDispatcher;
    let mut context = SceneDispatchContext::new(graph, interaction, manager, pending);
    action(&mut dispatcher, &mut context);
}

fn verify_pointer_capture() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 300.0, 220.0, MouseButton::Left);
            dispatcher.dispatch_mouse_moved(ctx, 700.0, 500.0);
            dispatcher.dispatch_mouse_released(ctx, 700.0, 500.0, MouseButton::Left);
        },
    );
    let events = &state.lock().unwrap().events;
    assert_event_order(
        events,
        &[
            MouseEventKind::Entered,
            MouseEventKind::Pressed,
            MouseEventKind::Exited,
            MouseEventKind::Dragged,
            MouseEventKind::Released,
        ],
    )?;
    if interaction.captured().is_some() {
        return Err("capture was not released".to_string());
    }
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_focus_keyboard() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 300.0, 220.0, MouseButton::Left);
            dispatcher.dispatch_key_pressed(
                ctx,
                Key::Character('a'),
                KeyModifiers {
                    control: true,
                    ..KeyModifiers::default()
                },
            );
            dispatcher.dispatch_key_released(ctx, Key::Character('a'), KeyModifiers::default());
            dispatcher.release_focus(ctx);
        },
    );
    let events = &state.lock().unwrap().events;
    for expected in [
        ProbeEvent::Focus(FocusEventKind::Gained),
        ProbeEvent::Key(
            KeyEventKind::Pressed,
            Key::Character('a'),
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        ),
        ProbeEvent::Key(
            KeyEventKind::Released,
            Key::Character('a'),
            KeyModifiers::default(),
        ),
        ProbeEvent::Focus(FocusEventKind::Lost),
    ] {
        if !events.contains(&expected) {
            return Err(format!("missing event: {expected:?}"));
        }
    }
    Ok(metrics([(
        "focus_owner",
        format!("{:?}", interaction.focus_owner()),
    )]))
}

fn verify_wheel_hover_double() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_hover(ctx, 300.0, 220.0);
            dispatcher.dispatch_mouse_wheel(ctx, 300.0, 220.0, 1.0, -2.0);
            dispatcher.dispatch_mouse_double_clicked(ctx, 300.0, 220.0, MouseButton::Left);
        },
    );
    let events = &state.lock().unwrap().events;
    if !events
        .iter()
        .any(|event| matches!(event, ProbeEvent::Mouse(MouseEventKind::Hover, ..)))
        || !events
            .iter()
            .any(|event| matches!(event, ProbeEvent::Wheel(_, _, 1.0, -2.0)))
        || !events
            .iter()
            .any(|event| matches!(event, ProbeEvent::Mouse(MouseEventKind::DoubleClicked, ..)))
    {
        return Err("hover/wheel/double-click sequence incomplete".to_string());
    }
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_coordinate_reduction() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(true);
    let mut interaction = InteractionState::default();
    let mut manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 160.0, 150.0, MouseButton::Left);
        },
    );
    let events = &state.lock().unwrap().events;
    if !events.contains(&ProbeEvent::Mouse(MouseEventKind::Pressed, 60.0, 70.0)) {
        return Err(format!("target-domain point was not reduced: {events:?}"));
    }
    Ok(metrics([
        ("entry_x", "160".to_string()),
        ("target_x", "60".to_string()),
    ]))
}

fn assert_event_order(events: &[ProbeEvent], kinds: &[MouseEventKind]) -> Result<(), String> {
    let actual = events
        .iter()
        .filter_map(|event| match event {
            ProbeEvent::Mouse(kind, _, _) => Some(*kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut cursor = 0;
    for expected in kinds {
        let Some(offset) = actual[cursor..].iter().position(|kind| kind == expected) else {
            return Err(format!(
                "missing ordered mouse event {expected:?}: {actual:?}"
            ));
        };
        cursor += offset + 1;
    }
    Ok(())
}

fn metrics<const N: usize>(entries: [(&str, String); N]) -> VerificationMetrics {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn verification_cases() -> [VerificationCase; 4] {
    [
        VerificationCase {
            name: "pointer_capture",
            run: verify_pointer_capture,
        },
        VerificationCase {
            name: "focus_keyboard",
            run: verify_focus_keyboard,
        },
        VerificationCase {
            name: "wheel_hover_double",
            run: verify_wheel_hover_double,
        },
        VerificationCase {
            name: "coordinate_root",
            run: verify_coordinate_reduction,
        },
    ]
}

fn main() {
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if cli.verify {
        run_verification(
            "event-app",
            &verification_cases(),
            cli.scenario.as_deref(),
            cli.report.as_deref(),
        )
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(1);
        });
        return;
    }

    let scene_list = scenes();
    if cli.screenshot_all {
        run_demo_app_with_screenshot("Event Pipeline Verification", "event-app", scene_list, true)
            .expect("run event-app screenshots");
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scene_list.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
        run_demo_app_with_scene_screenshot(
            "Event Pipeline Verification",
            "event-app",
            scene_list,
            index,
        )
        .expect("run event-app screenshot");
    } else {
        run_demo_app("Event Pipeline Verification", "event-app", scene_list)
            .expect("run event-app");
    }
}
