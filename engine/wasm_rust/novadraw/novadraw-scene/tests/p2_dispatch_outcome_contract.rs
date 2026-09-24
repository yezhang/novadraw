use novadraw_scene::{
    ClickableFigure, FigureTree, MouseButton, Rectangle, RectangleFigure, Runtime,
};

fn runtime_with_clickable() -> (Runtime, novadraw_scene::FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)));
    let clickable = tree
        .builder()
        .add_child(
            root,
            Box::new(ClickableFigure::new(Rectangle::new(20.0, 20.0, 80.0, 40.0))),
        )
        .expect("valid FigureTree construction");
    (Runtime::new(tree), clickable)
}

#[test]
fn handled_press_reports_target_and_capture_to_the_caller() {
    let (mut runtime, clickable) = runtime_with_clickable();

    let outcome = runtime.dispatch_mouse_pressed(30.0, 30.0, MouseButton::Left);

    assert!(outcome.is_handled());
    assert_eq!(outcome.target(), Some(clickable));
    assert_eq!(outcome.capture(), Some(clickable));
}

#[test]
fn unhandled_press_is_observable_for_editor_tool_fallback() {
    let (mut runtime, _) = runtime_with_clickable();

    let outcome = runtime.dispatch_mouse_pressed(250.0, 150.0, MouseButton::Left);

    assert!(!outcome.is_handled());
    assert_eq!(outcome.capture(), None);
}
