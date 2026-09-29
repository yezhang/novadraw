use std::time::Duration;

use novadraw::{
    FigureId, FigureStyle, MonotonicTime, MouseButton, RectangleFigure, Runtime, TooltipTiming,
    TooltipUpdate,
};

fn tooltip_runtime() -> (Runtime, FigureId, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    runtime
        .set_tooltip_timing(
            TooltipTiming::new(Duration::from_micros(10), Duration::from_micros(20)).unwrap(),
        )
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let left = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 100.0, 100.0)))
        .expect("valid Runtime mutation");
    let right = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(150.0, 10.0, 100.0, 100.0)))
        .expect("valid Runtime mutation");
    assert!(
        runtime
            .figure(root)
            .unwrap()
            .set_style(FigureStyle {
                tooltip: Some(Some("container tip".to_string())),
                ..FigureStyle::default()
            },)
            .expect("valid Runtime mutation")
    );
    (runtime, root, left, right)
}

#[test]
fn inherited_tooltip_uses_the_owning_ancestor_as_stable_source() {
    let (mut runtime, root, _left, _right) = tooltip_runtime();

    runtime.dispatch_mouse_moved(20.0, 20.0);
    assert_eq!(
        runtime.next_wake_deadline(),
        Some(MonotonicTime::from_micros(10))
    );

    runtime.dispatch_mouse_moved(160.0, 20.0);
    assert_eq!(
        runtime.next_wake_deadline(),
        Some(MonotonicTime::from_micros(10))
    );
    assert!(
        runtime
            .advance_time(MonotonicTime::from_micros(10))
            .unwrap()
    );

    let updates = runtime.take_tooltip_updates();
    assert!(matches!(
        updates.as_slice(),
        [TooltipUpdate::Show(snapshot)]
            if snapshot.source == root
                && snapshot.text == "container tip"
                && snapshot.anchor.x() == 160.0
    ));
}

#[test]
fn explicit_disable_cancels_an_inherited_waiting_tooltip() {
    let (mut runtime, _root, left, _right) = tooltip_runtime();

    runtime.dispatch_mouse_moved(20.0, 20.0);
    assert!(runtime.next_wake_deadline().is_some());
    assert!(
        runtime
            .figure(left)
            .unwrap()
            .set_style(FigureStyle {
                tooltip: Some(None),
                ..FigureStyle::default()
            },)
            .expect("valid Runtime mutation")
    );

    assert_eq!(runtime.next_wake_deadline(), None);
    assert!(
        !runtime
            .advance_time(MonotonicTime::from_micros(10))
            .unwrap()
    );
    assert!(runtime.take_tooltip_updates().is_empty());
}

#[test]
fn visible_tooltip_replaces_content_and_press_hides_it() {
    let (mut runtime, root, _left, _right) = tooltip_runtime();
    runtime.dispatch_mouse_moved(20.0, 20.0);
    runtime
        .advance_time(MonotonicTime::from_micros(10))
        .unwrap();
    runtime.take_tooltip_updates();

    assert!(
        runtime
            .figure(root)
            .unwrap()
            .set_style(FigureStyle {
                tooltip: Some(Some("updated tip".to_string())),
                ..FigureStyle::default()
            },)
            .expect("valid Runtime mutation")
    );
    assert!(matches!(
        runtime.take_tooltip_updates().as_slice(),
        [TooltipUpdate::Replace(snapshot)] if snapshot.text == "updated tip"
    ));

    runtime.dispatch_mouse_pressed(20.0, 20.0, MouseButton::Left);
    assert!(matches!(
        runtime.take_tooltip_updates().as_slice(),
        [TooltipUpdate::Hide { .. }]
    ));
    assert!(runtime.visible_tooltip().is_none());
    assert_eq!(runtime.next_wake_deadline(), None);
}
