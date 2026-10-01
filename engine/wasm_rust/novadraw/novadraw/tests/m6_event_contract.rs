use std::sync::{Arc, Mutex};

use novadraw::advanced::{EventDispatcher, InteractionState, PendingMutations, UpdateManager};
use novadraw::{
    Bounded, Figure, FigureEventHandler, FigureTree, FocusEvent, FocusEventKind, GesturePhase,
    GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind, Rectangle, RectangleFigure, Runtime, SceneDispatchContext, ScrollDeltaKind,
    WheelEvent,
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

impl Figure for InputProbeFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for InputProbeFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_released(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_moved(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_hover(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_entered(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_exited(&self, event: &MouseEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.record_mouse(event);
        true
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.events.lock().unwrap().push(RecordedInput::Wheel(
            event.x,
            event.y,
            event.delta_x,
            event.delta_y,
        ));
        true
    }

    fn on_key_pressed(&self, event: &KeyEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Key(event.kind, event.key));
        true
    }

    fn on_focus_gained(&self, event: &FocusEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(RecordedInput::Focus(event.kind));
        true
    }

    fn on_focus_lost(&self, event: &FocusEvent, _ctx: &mut novadraw::EventContext<'_>) -> bool {
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
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let coordinate_root = graph
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new(100.0, 50.0, 200.0, 150.0)),
        )
        .expect("valid FigureTree construction");
    let probe = graph
        .builder()
        .add_child(
            coordinate_root,
            Box::new(InputProbeFigure {
                bounds: Rectangle::new(10.0, 20.0, 50.0, 50.0),
                events: events.clone(),
            }),
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
    let mut update_manager = UpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = EventDispatcher;

    {
        let mut ctx = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
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

    assert_eq!(interaction.captured(), None);
    assert_eq!(interaction.focus_owner(), None);

    let events = events.lock().unwrap();
    assert!(events.contains(&RecordedInput::Mouse(MouseEventKind::Pressed, 10.0, 10.0,)));
    assert!(events.contains(&RecordedInput::Mouse(MouseEventKind::Dragged, 150.0, 120.0,)));
    assert!(events.contains(&RecordedInput::Wheel(10.0, 10.0, 1.0, -2.0)));
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
    assert_eq!(interaction.mouse_target(), None);
}

#[test]
fn continuous_scroll_keeps_its_target_and_does_not_follow_pointer_capture() {
    let captured_events = Arc::new(Mutex::new(Vec::new()));
    let gesture_events = Arc::new(Mutex::new(Vec::new()));
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    graph
        .builder()
        .add_child(
            root,
            Box::new(InputProbeFigure {
                bounds: Rectangle::new(10.0, 10.0, 80.0, 80.0),
                events: Arc::clone(&captured_events),
            }),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .add_child(
            root,
            Box::new(InputProbeFigure {
                bounds: Rectangle::new(200.0, 100.0, 80.0, 80.0),
                events: Arc::clone(&gesture_events),
            }),
        )
        .expect("valid FigureTree construction");
    let mut update_manager = UpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = EventDispatcher;
    let session = GestureSessionId::new(7);

    {
        let mut ctx = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
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

fn assert_retired_gesture_target_does_not_retarget(remove_target: bool) {
    let first_events = Arc::new(Mutex::new(Vec::new()));
    let second_events = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 160.0)))
        .expect("valid Runtime mutation");
    let first_viewport = runtime
        .add_viewport(root, Rectangle::new(0.0, 0.0, 120.0, 120.0))
        .unwrap();
    let second_viewport = runtime
        .add_viewport(root, Rectangle::new(200.0, 0.0, 120.0, 120.0))
        .unwrap();
    let first_target = runtime
        .container(first_viewport.figure_id())
        .unwrap()
        .add(Box::new(InputProbeFigure {
            bounds: Rectangle::new(0.0, 0.0, 120.0, 120.0),
            events: Arc::clone(&first_events),
        }))
        .expect("valid Runtime mutation");
    let second_target = runtime
        .container(second_viewport.figure_id())
        .unwrap()
        .add(Box::new(InputProbeFigure {
            bounds: Rectangle::new(0.0, 0.0, 120.0, 120.0),
            events: Arc::clone(&second_events),
        }))
        .expect("valid Runtime mutation");
    let session = GestureSessionId::new(17);

    let begin = runtime.dispatch_scroll(WheelEvent::with_details(
        20.0,
        20.0,
        0.0,
        -2.0,
        ScrollDeltaKind::LogicalPixels,
        GesturePhase::Begin,
        KeyModifiers::default(),
        session,
    ));
    assert_eq!(begin.target(), Some(first_target));

    if remove_target {
        assert!(
            runtime
                .container(root)
                .unwrap()
                .remove(first_viewport.figure_id())
                .expect("valid Runtime mutation")
        );
    } else {
        assert!(
            runtime
                .figure(first_viewport.figure_id())
                .unwrap()
                .set_visible(false)
                .expect("valid Runtime mutation")
        );
    }
    let update = runtime.dispatch_scroll(WheelEvent::with_details(
        220.0,
        20.0,
        0.0,
        -3.0,
        ScrollDeltaKind::LogicalPixels,
        GesturePhase::Update,
        KeyModifiers::default(),
        session,
    ));
    let end = runtime.dispatch_scroll(WheelEvent::with_details(
        220.0,
        20.0,
        0.0,
        -4.0,
        ScrollDeltaKind::LogicalPixels,
        GesturePhase::End,
        KeyModifiers::default(),
        session,
    ));

    assert_eq!(update.target(), None);
    assert_eq!(end.target(), None);
    assert_eq!(
        first_events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, RecordedInput::Wheel(..)))
            .count(),
        1
    );
    assert_eq!(
        second_events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, RecordedInput::Wheel(..)))
            .count(),
        0
    );

    let next_session = GestureSessionId::new(18);
    let next = runtime.dispatch_scroll(WheelEvent::with_details(
        220.0,
        20.0,
        0.0,
        -5.0,
        ScrollDeltaKind::LogicalPixels,
        GesturePhase::Begin,
        KeyModifiers::default(),
        next_session,
    ));
    assert_eq!(next.target(), Some(second_target));
}

#[test]
fn retired_gesture_target_does_not_retarget_the_same_session() {
    assert_retired_gesture_target_does_not_retarget(false);
    assert_retired_gesture_target_does_not_retarget(true);
}

#[test]
fn interactive_parent_remains_mouse_target_across_non_interactive_children() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)));
    let parent = graph
        .builder()
        .add_child(
            root,
            Box::new(InputProbeFigure {
                bounds: Rectangle::new(20.0, 20.0, 120.0, 100.0),
                events: Arc::clone(&events),
            }),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::new(10.0, 10.0, 30.0, 30.0)),
        )
        .expect("valid FigureTree construction");
    let mut update_manager = UpdateManager::new();
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = EventDispatcher;

    {
        let mut ctx = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_mouse_moved(&mut ctx, 35.0, 35.0);
        dispatcher.dispatch_mouse_hover(&mut ctx, 35.0, 35.0);
        dispatcher.dispatch_mouse_moved(&mut ctx, 100.0, 80.0);
    }

    assert_eq!(interaction.mouse_target(), Some(parent));
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            RecordedInput::Mouse(MouseEventKind::Entered, 15.0, 15.0),
            RecordedInput::Mouse(MouseEventKind::Moved, 15.0, 15.0),
            RecordedInput::Mouse(MouseEventKind::Hover, 15.0, 15.0),
            RecordedInput::Mouse(MouseEventKind::Moved, 80.0, 60.0),
        ]
    );
}
