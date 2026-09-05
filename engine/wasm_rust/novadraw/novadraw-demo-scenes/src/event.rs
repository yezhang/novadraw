use std::sync::{Arc, Mutex};

use novadraw::{
    Bounded, Color, EventContext, Figure, FigureEventHandler, FigureTree, FocusEvent,
    FocusEventKind, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
    NdCanvas, Rectangle, RectangleFigure, WheelEvent,
};

use crate::focus::{FocusProbeSpec, build_focus_traversal_scene};
use crate::{DemoSuite, SceneSpec, ValidationKind};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

#[derive(Clone, Debug, PartialEq)]
pub enum ProbeEvent {
    Mouse(MouseEventKind, f64, f64),
    Wheel(f64, f64, f64, f64),
    Key(KeyEventKind, Key, KeyModifiers),
    Focus(FocusEventKind),
}

#[derive(Default)]
pub struct ProbeState {
    pub events: Vec<ProbeEvent>,
    hovered: bool,
    pressed: bool,
    focused: bool,
}

struct EventProbeFigure {
    bounds: Rectangle,
    label: &'static str,
    disabled_visual: bool,
    state: Arc<Mutex<ProbeState>>,
}

impl EventProbeFigure {
    fn new(bounds: Rectangle, state: Arc<Mutex<ProbeState>>) -> Self {
        Self {
            bounds,
            label: "Probe",
            disabled_visual: false,
            state,
        }
    }

    fn from_focus_spec(spec: FocusProbeSpec, state: Arc<Mutex<ProbeState>>) -> Self {
        Self {
            bounds: spec.bounds,
            label: spec.label,
            disabled_visual: !spec.enabled,
            state,
        }
    }

    fn record_mouse(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) {
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

impl Figure for EventProbeFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure(&self, canvas: &mut NdCanvas) {
        let state = self.state.lock().unwrap();
        let color = if self.disabled_visual {
            Color::hex("#95a5a6")
        } else if state.pressed {
            Color::hex("#e74c3c")
        } else if state.focused {
            Color::hex("#9b59b6")
        } else if state.hovered {
            Color::hex("#2ecc71")
        } else {
            Color::hex("#3498db")
        };
        canvas.fill_rect(0.0, 0.0, self.bounds.width, self.bounds.height, color);
        canvas.fill_style(if self.disabled_visual {
            Color::hex("#2c3e50")
        } else {
            Color::WHITE
        });
        canvas.font("18px sans-serif");
        canvas.fill_text(self.label, 12.0, 30.0);
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for EventProbeFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_released(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_moved(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_hover(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_double_clicked(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_entered(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_exited(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        self.record_mouse(event, ctx);
        true
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, ctx: &mut EventContext<'_>) -> bool {
        self.state.lock().unwrap().events.push(ProbeEvent::Wheel(
            event.x,
            event.y,
            event.delta_x,
            event.delta_y,
        ));
        ctx.repaint(None);
        true
    }

    fn on_key_pressed(&self, event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
        self.state.lock().unwrap().events.push(ProbeEvent::Key(
            event.kind,
            event.key,
            event.modifiers,
        ));
        ctx.repaint(None);
        true
    }

    fn on_key_released(&self, event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
        FigureEventHandler::on_key_pressed(self, event, ctx)
    }

    fn on_focus_gained(&self, event: &FocusEvent, ctx: &mut EventContext<'_>) -> bool {
        let mut state = self.state.lock().unwrap();
        state.focused = true;
        state.events.push(ProbeEvent::Focus(event.kind));
        drop(state);
        ctx.repaint(None);
        true
    }

    fn on_focus_lost(&self, event: &FocusEvent, ctx: &mut EventContext<'_>) -> bool {
        let mut state = self.state.lock().unwrap();
        state.focused = false;
        state.events.push(ProbeEvent::Focus(event.kind));
        drop(state);
        ctx.repaint(None);
        true
    }
}

pub fn probe_scene(local_coordinates: bool) -> (FigureTree, Arc<Mutex<ProbeState>>) {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            Color::hex("#eeeeee"),
        )));
    let parent = if local_coordinates {
        graph.builder().add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                100.0,
                80.0,
                500.0,
                360.0,
                Color::hex("#dfe6e9"),
            )),
        )
    } else {
        root
    };
    let state = Arc::new(Mutex::new(ProbeState::default()));
    let probe = graph.builder().add_child_to(
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
    graph.set_focusable(probe, true);
    graph.set_focus_traversable(probe, true);
    (graph, state)
}

fn pointer_scene() -> FigureTree {
    probe_scene(false).0
}

fn coordinate_scene() -> FigureTree {
    probe_scene(true).0
}

fn focus_traversal_scene() -> FigureTree {
    build_focus_traversal_scene(|spec| {
        Box::new(EventProbeFigure::from_focus_spec(
            spec,
            Arc::new(Mutex::new(ProbeState::default())),
        ))
    })
    .0
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "event",
        "Events",
        vec![
            SceneSpec::new(
                "pointer-capture",
                "pointer_capture",
                size,
                ValidationKind::Interactive,
                pointer_scene,
            ),
            SceneSpec::new(
                "focus-keyboard",
                "focus_keyboard",
                size,
                ValidationKind::Interactive,
                focus_traversal_scene,
            ),
            SceneSpec::new(
                "wheel-hover-double",
                "wheel_hover_double",
                size,
                ValidationKind::Interactive,
                pointer_scene,
            ),
            SceneSpec::new(
                "coordinate-root",
                "coordinate_root",
                size,
                ValidationKind::Interactive,
                coordinate_scene,
            ),
        ],
    )
}
