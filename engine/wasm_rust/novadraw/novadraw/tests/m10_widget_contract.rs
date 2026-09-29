use std::sync::{Arc, Mutex};

use novadraw::{
    ActionEvent, ActionListener, ButtonFigure, ClickableFigure, ClickableKind, FigureId, Key,
    KeyModifiers, ListenerDirective, MouseButton, Rectangle, RectangleFigure, Runtime,
    ToggleFigure,
};

struct ActionRecorder(Arc<Mutex<Vec<ActionEvent>>>);

impl ActionListener for ActionRecorder {
    fn action_performed(&self, event: ActionEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}

fn runtime_with_button() -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)))
        .expect("valid Runtime mutation");
    let button = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ButtonFigure::new("Apply").with_bounds(Rectangle::new(20.0, 20.0, 120.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    (runtime, button)
}

fn runtime_with_toggle() -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)))
        .expect("valid Runtime mutation");
    let toggle = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ToggleFigure::new("Snap").with_bounds(Rectangle::new(20.0, 20.0, 120.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    (runtime, toggle)
}

#[test]
fn mouse_release_inside_fires_action_and_owns_focus() {
    let (mut runtime, button) = runtime_with_button();

    runtime.dispatch_mouse_moved(30.0, 30.0);
    assert!(runtime.clickable_snapshot(button).unwrap().visual.hovered);

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    let pressed = runtime.clickable_snapshot(button).unwrap();
    assert!(pressed.visual.pressed);
    assert!(pressed.visual.focused);
    assert_eq!(runtime.interaction().captured(), Some(button));
    assert_eq!(runtime.interaction().focus_owner(), Some(button));

    runtime.dispatch_mouse_released(30.0, 30.0, MouseButton::Left);

    let released = runtime.clickable_snapshot(button).unwrap();
    assert!(!released.visual.pressed);
    assert_eq!(released.action_revision, 1);
}

#[test]
fn drag_out_cancels_action_and_drag_back_rearms_it() {
    let (mut runtime, button) = runtime_with_button();

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_mouse_moved(250.0, 150.0);
    assert!(!runtime.clickable_snapshot(button).unwrap().visual.pressed);
    runtime.dispatch_mouse_released(250.0, 150.0, MouseButton::Left);
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        0
    );

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_mouse_moved(250.0, 150.0);
    runtime.dispatch_mouse_moved(40.0, 40.0);
    assert!(runtime.clickable_snapshot(button).unwrap().visual.pressed);
    runtime.dispatch_mouse_released(40.0, 40.0, MouseButton::Left);

    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        1
    );
}

#[test]
fn enter_and_space_activate_only_after_matching_key_release() {
    let (mut runtime, button) = runtime_with_button();
    runtime.request_focus(button).unwrap();

    runtime.dispatch_key_pressed(Key::Enter, KeyModifiers::default());
    assert!(runtime.clickable_snapshot(button).unwrap().visual.pressed);
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        0
    );
    runtime.dispatch_key_released(Key::Enter, KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        1
    );

    runtime.dispatch_key_pressed(Key::Character(' '), KeyModifiers::default());
    runtime.dispatch_key_released(Key::Character(' '), KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        2
    );

    runtime.dispatch_key_released(Key::Enter, KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        2
    );
}

#[test]
fn keyboard_press_cannot_be_completed_by_an_unrelated_mouse_release() {
    let (mut runtime, button) = runtime_with_button();
    runtime.request_focus(button).unwrap();
    runtime.dispatch_key_pressed(Key::Enter, KeyModifiers::default());

    runtime.dispatch_mouse_released(30.0, 30.0, MouseButton::Left);
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        0
    );

    runtime.dispatch_key_released(Key::Enter, KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        1
    );
}

#[test]
fn keyboard_activation_requires_the_matching_release_key() {
    let (mut runtime, button) = runtime_with_button();
    runtime.request_focus(button).unwrap();
    runtime.dispatch_key_pressed(Key::Enter, KeyModifiers::default());

    runtime.dispatch_key_released(Key::Character(' '), KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        0
    );
    assert!(runtime.clickable_snapshot(button).unwrap().visual.pressed);

    runtime.dispatch_key_released(Key::Enter, KeyModifiers::default());
    assert_eq!(
        runtime.clickable_snapshot(button).unwrap().action_revision,
        1
    );
}

#[test]
fn disabled_button_rejects_pointer_keyboard_and_programmatic_actions() {
    let (mut runtime, button) = runtime_with_button();
    assert!(
        runtime
            .figure(button)
            .unwrap()
            .set_enabled(false)
            .expect("valid Runtime mutation")
    );

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_mouse_released(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_key_pressed(Key::Enter, KeyModifiers::default());
    runtime.dispatch_key_released(Key::Enter, KeyModifiers::default());
    assert!(!runtime.do_click(button).unwrap());

    let snapshot = runtime.clickable_snapshot(button).unwrap();
    assert!(!snapshot.visual.enabled);
    assert_eq!(snapshot.action_revision, 0);
}

#[test]
fn toggle_changes_selection_before_emitting_action() {
    let (mut runtime, toggle) = runtime_with_toggle();

    runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);
    runtime.dispatch_mouse_released(30.0, 30.0, MouseButton::Left);

    let snapshot = runtime.clickable_snapshot(toggle).unwrap();
    assert_eq!(snapshot.kind, ClickableKind::Toggle);
    assert!(snapshot.selected);
    assert_eq!(snapshot.action_revision, 1);
}

#[test]
fn selected_state_is_model_owned_and_programmatic_click_uses_same_transaction() {
    let (mut runtime, toggle) = runtime_with_toggle();

    assert!(
        runtime
            .figure(toggle)
            .unwrap()
            .set_clickable_selected(true)
            .unwrap()
    );
    assert!(runtime.clickable_snapshot(toggle).unwrap().selected);
    assert!(!runtime.interaction().is_pressed(toggle));
    assert!(!runtime.interaction().is_hovered(toggle));

    assert!(runtime.do_click(toggle).unwrap());
    let snapshot = runtime.clickable_snapshot(toggle).unwrap();
    assert!(!snapshot.selected);
    assert_eq!(snapshot.action_revision, 1);
}

#[test]
fn button_exposes_composed_label_contract() {
    let (mut runtime, button) = runtime_with_button();

    assert!(
        runtime
            .figure(button)
            .unwrap()
            .set_label_text("Save changes")
            .unwrap()
    );
    assert_eq!(runtime.label_text(button).unwrap(), "Save changes");
}

#[test]
fn action_listener_flushes_at_the_update_transaction_boundary() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let clickable = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ClickableFigure::new(Rectangle::new(
            20.0, 20.0, 80.0, 40.0,
        ))))
        .expect("valid Runtime mutation");
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime.add_action_listener(Box::new(ActionRecorder(Arc::clone(&events))));

    assert!(runtime.do_click(clickable).unwrap());
    assert!(events.lock().unwrap().is_empty());
    assert!(runtime.prepare_frame().is_some());
    assert_eq!(
        *events.lock().unwrap(),
        vec![ActionEvent {
            figure_id: clickable,
            revision: 1,
        }]
    );
}
