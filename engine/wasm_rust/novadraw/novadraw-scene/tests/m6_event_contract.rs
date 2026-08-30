use std::sync::{Arc, Mutex};

use novadraw_scene::{
    BasicEventDispatcher, Bounded, EventDispatcher, Figure, FigureGraph, FocusEvent,
    FocusEventKind, GesturePhase, GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind, PendingMutations, Rectangle, RectangleFigure,
    SceneDispatchContext, SceneUpdateManager, ScrollDeltaKind, Updatable, WheelEvent,
};

#[derive(Clone, Debug, PartialEq)]
enum RecordedInput {
    Mouse(MouseEventKind, f64, f64),
    Wheel(f64, f64, f64, f64),
    Key(KeyEventKind, Key),
    Focus(FocusEventKind),
}

struct InputProbeFigure {
    bounds: Rectangle,
    events: Arc<Mutex<Vec<RecordedInput>>>,
}

impl Bounded for InputProbeFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "InputProbeFigure"
    }
}

impl Updatable for InputProbeFigure {
    fn validate(&mut self) {}
}

impl Figure for InputProbeFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn wants_key_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(
        &self,
        event: &MouseEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_released(
        &self,
        event: &MouseEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_dragged(
        &self,
        event: &MouseEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_entered(
        &self,
        event: &MouseEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_exited(
        &self,
        event: &MouseEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_wheel(
        &self,
        event: &WheelEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.events.lock().unwrap().push(RecordedInput::Wheel(
            event.x,
            event.y,
            event.delta_x,
            event.delta_y,
        ));
        true
    }

    fn on_key_pressed(
        &self,
        event: &KeyEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Key(event.kind, event.key));
        true
    }

    fn on_focus_gained(
        &self,
        event: &FocusEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Focus(event.kind));
        true
    }

    fn on_focus_lost(
        &self,
        event: &FocusEvent,
        _ctx: &mut dyn novadraw_scene::NovadrawContext,
    ) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Focus(event.kind));
        true
    }
}

impl InputProbeFigure {
    fn record_mouse(&self, event: &MouseEvent) {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Mouse(event.kind, event.x, event.y));
    }
}

#[test]
fn capture_hover_focus_key_and_wheel_share_the_engine_dispatch_contract() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let coordinate_root = graph.add_child_to(
        root,
        Box::new(RectangleFigure::new(100.0, 50.0, 200.0, 150.0).with_local_coordinates(true)),
    );
    graph.add_child_to(
        coordinate_root,
        Box::new(InputProbeFigure {
            bounds: Rectangle::new(10.0, 20.0, 50.0, 50.0),
            events: events.clone(),
        }),
    );
    let mut update_manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;

    {
        let mut ctx = SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
        dispatcher.dispatch_mouse_pressed(&mut ctx, 120.0, 80.0, MouseButton::Left);
        dispatcher.dispatch_mouse_moved(&mut ctx, 260.0, 190.0);
        dispatcher.dispatch_mouse_wheel(&mut ctx, 120.0, 80.0, 1.0, -2.0);
        dispatcher.dispatch_key_pressed(
            &mut ctx,
            Key::Character('x'),
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        );
        dispatcher.dispatch_mouse_released(&mut ctx, 260.0, 190.0, MouseButton::Left);
        dispatcher.release_focus(&mut ctx);
    }

    assert_eq!(graph.captured(), None);
    assert_eq!(graph.focus_owner(), None);
    assert_eq!(graph.hover_source(), None);
    assert_eq!(graph.cursor_target(), None);

    let events = events.lock().unwrap();
    assert!(events.contains(&RecordedInput::Mouse(MouseEventKind::Pressed, 20.0, 30.0,)));
    assert!(events.contains(&RecordedInput::Mouse(MouseEventKind::Dragged, 160.0, 140.0,)));
    assert!(events.contains(&RecordedInput::Wheel(20.0, 30.0, 1.0, -2.0)));
    assert!(events.contains(&RecordedInput::Key(
        KeyEventKind::Pressed,
        Key::Character('x'),
    )));
    assert!(events.contains(&RecordedInput::Focus(FocusEventKind::Gained)));
    assert!(events.contains(&RecordedInput::Focus(FocusEventKind::Lost)));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RecordedInput::Mouse(MouseEventKind::Exited, ..)))
    );
    assert!(events.iter().all(|event| match event {
        RecordedInput::Mouse(_, x, y) | RecordedInput::Wheel(x, y, _, _) => {
            x.is_finite() && y.is_finite()
        }
        RecordedInput::Key(..) | RecordedInput::Focus(..) => true,
    }));
    assert_eq!(graph.mouse_target(), None);
}

#[test]
fn continuous_scroll_keeps_its_target_and_does_not_follow_pointer_capture() {
    let captured_events = Arc::new(Mutex::new(Vec::new()));
    let gesture_events = Arc::new(Mutex::new(Vec::new()));
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    graph.add_child_to(
        root,
        Box::new(InputProbeFigure {
            bounds: Rectangle::new(10.0, 10.0, 80.0, 80.0),
            events: Arc::clone(&captured_events),
        }),
    );
    graph.add_child_to(
        root,
        Box::new(InputProbeFigure {
            bounds: Rectangle::new(200.0, 100.0, 80.0, 80.0),
            events: Arc::clone(&gesture_events),
        }),
    );
    let mut update_manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    let session = GestureSessionId::new(7);

    {
        let mut ctx = SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
        dispatcher.dispatch_mouse_pressed(&mut ctx, 20.0, 20.0, MouseButton::Left);
        dispatcher.dispatch_scroll(
            &mut ctx,
            WheelEvent::with_details(
                220.0,
                120.0,
                0.0,
                -4.0,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::Begin,
                KeyModifiers::default(),
                session,
            ),
        );
        dispatcher.dispatch_scroll(
            &mut ctx,
            WheelEvent::with_details(
                350.0,
                250.0,
                0.0,
                -6.0,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::End,
                KeyModifiers::default(),
                session,
            ),
        );
    }

    assert_eq!(
        captured_events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, RecordedInput::Wheel(..)))
            .count(),
        0
    );
    assert_eq!(
        gesture_events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, RecordedInput::Wheel(..)))
            .count(),
        2
    );
}
