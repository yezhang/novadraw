use std::sync::{Arc, Mutex};

use novadraw::event::{
    EventContext, FigureEventHandler, FocusEvent, FocusEventKind, Key, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEvent, MouseEventKind, WheelEvent,
};
use novadraw::figure::{
    Bounded, FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
};
use novadraw::graphics::NdCanvas;
use novadraw::{Color, Figure, FigureTree, Rectangle, RectangleFigure};

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
    disabled_visual: bool,
    state: Arc<Mutex<ProbeState>>,
}

impl EventProbeFigure {
    fn new(bounds: Rectangle, state: Arc<Mutex<ProbeState>>) -> Self {
        Self {
            bounds,
            disabled_visual: false,
            state,
        }
    }

    fn from_focus_spec(spec: FocusProbeSpec, state: Arc<Mutex<ProbeState>>) -> Self {
        Self {
            bounds: spec.bounds,
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
            Color::from_hex("#95a5a6").expect("valid color literal")
        } else if state.pressed {
            Color::from_hex("#e74c3c").expect("valid color literal")
        } else if state.focused {
            Color::from_hex("#9b59b6").expect("valid color literal")
        } else if state.hovered {
            Color::from_hex("#2ecc71").expect("valid color literal")
        } else {
            Color::from_hex("#3498db").expect("valid color literal")
        };
        canvas.fill_rect_with_color(0.0, 0.0, self.bounds.width, self.bounds.height, color);
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
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
            Color::from_hex("#eeeeee").expect("valid color literal"),
        )))
        .expect("valid FigureTree construction");
    let parent = if local_coordinates {
        graph
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new_with_color(
                    100.0,
                    80.0,
                    500.0,
                    360.0,
                    Color::from_hex("#dfe6e9").expect("valid color literal"),
                )),
            )
            .expect("valid FigureTree construction")
    } else {
        root
    };
    let state = Arc::new(Mutex::new(ProbeState::default()));
    let probe = graph
        .builder()
        .add_child(
            parent,
            Box::new(EventProbeFigure::new(
                if local_coordinates {
                    Rectangle::new(40.0, 50.0, 220.0, 140.0)
                } else {
                    Rectangle::new(250.0, 180.0, 300.0, 200.0)
                },
                state.clone(),
            )),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_focusable(probe, true)
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_focus_traversable(probe, true)
        .expect("valid FigureTree construction");
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
