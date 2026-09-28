use novadraw_scene::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, RectangleFigure,
    Runtime,
};

#[test]
fn public_runtime_focus_api_separates_direct_and_traversal_eligibility() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid Runtime mutation");
    let first_traversal = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
        .expect("valid Runtime mutation");
    let last_traversal = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)))
        .expect("valid Runtime mutation");
    let direct_only = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(60.0, 0.0, 20.0, 20.0)))
        .expect("valid Runtime mutation");
    runtime
        .figure(first_traversal)
        .unwrap()
        .set_focus_traversable(true)
        .expect("valid Runtime mutation");
    runtime
        .figure(last_traversal)
        .unwrap()
        .set_focus_traversable(true)
        .expect("valid Runtime mutation");
    runtime
        .figure(direct_only)
        .unwrap()
        .set_focusable(true)
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime.request_focus(first_traversal),
        Err(FocusError::NotFocusable(first_traversal))
    );
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Forward),
        FocusTraversalOutcome::Moved(first_traversal)
    );
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Forward),
        FocusTraversalOutcome::Moved(last_traversal)
    );
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Forward),
        FocusTraversalOutcome::Boundary
    );
    runtime.clear_focus();
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Backward),
        FocusTraversalOutcome::Moved(last_traversal)
    );
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Backward),
        FocusTraversalOutcome::Moved(first_traversal)
    );
    assert!(matches!(
        runtime.request_focus(direct_only),
        Ok(FocusChange::Changed {
            previous: Some(previous),
            current: Some(current),
        }) if previous == first_traversal && current == direct_only
    ));
}
