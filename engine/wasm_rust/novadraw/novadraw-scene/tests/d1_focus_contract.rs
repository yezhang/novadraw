use novadraw_scene::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, RectangleFigure,
    Runtime,
};

#[test]
fn public_runtime_focus_api_separates_direct_and_traversal_eligibility() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let traversal_only =
        runtime.add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    let direct_only =
        runtime.add_figure(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)));
    runtime.set_focus_traversable(traversal_only, true);
    runtime.set_focusable(direct_only, true);

    assert_eq!(
        runtime.request_focus(traversal_only),
        Err(FocusError::NotFocusable(traversal_only))
    );
    assert_eq!(
        runtime.traverse_focus(FocusTraversalDirection::Forward),
        FocusTraversalOutcome::Moved(traversal_only)
    );
    assert!(matches!(
        runtime.request_focus(direct_only),
        Ok(FocusChange::Changed {
            previous: Some(previous),
            current: Some(current),
        }) if previous == traversal_only && current == direct_only
    ));
}
