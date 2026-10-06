//! External contracts for P2-M01 Figure bounds capture and transition.

use std::time::Duration;

use novadraw::{
    Affine2D, FigureTree, MonotonicTime, Rectangle, RectangleFigure, Runtime,
    animation::{
        AnimationError, AnimationStart, AnimationState, AnimationSuppression, BoundsTransition,
        Easing, InteractionGeometryPolicy,
    },
    render::RenderCommandKind,
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

fn contains_transform(runtime: &mut Runtime, expected: Affine2D) -> bool {
    runtime
        .record_full_frame()
        .commands()
        .iter()
        .any(|command| {
            matches!(
                command.kind,
                RenderCommandKind::ConcatTransform { matrix } if matrix == expected
            )
        })
}

#[test]
fn captured_bounds_transition_keeps_committed_geometry_final() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(10.0, 20.0, 20.0, 10.0)))
        .unwrap();
    runtime.advance_time(time(0)).unwrap();
    let capture = runtime.animations().capture_figures([figure]).unwrap();

    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(50.0, 60.0, 40.0, 20.0))
        .unwrap();
    let AnimationStart::Running(animation) = runtime
        .animations()
        .transition_bounds(
            capture,
            BoundsTransition::new(Duration::from_millis(100)).with_easing(Easing::Linear),
        )
        .unwrap()
    else {
        panic!("changed bounds must create a transition");
    };

    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Some(Rectangle::new(50.0, 60.0, 40.0, 20.0))
    );
    assert!(contains_transform(
        &mut runtime,
        Affine2D::from_translation(-40.0, -40.0) * Affine2D::from_scale(0.5, 0.5)
    ));

    runtime.advance_time(time(50)).unwrap();
    assert!(contains_transform(
        &mut runtime,
        Affine2D::from_translation(-20.0, -20.0) * Affine2D::from_scale(0.75, 0.75)
    ));
    runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Completed
    );
}

#[test]
fn unchanged_and_zero_duration_transitions_are_suppressed() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let unchanged = runtime.animations().capture_figures([figure]).unwrap();
    assert_eq!(
        runtime
            .animations()
            .transition_bounds(unchanged, BoundsTransition::new(Duration::from_millis(100)))
            .unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta)
    );

    let zero_duration = runtime.animations().capture_figures([figure]).unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    assert_eq!(
        runtime
            .animations()
            .transition_bounds(zero_duration, BoundsTransition::new(Duration::ZERO))
            .unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta)
    );
    assert_eq!(runtime.animations().active_animation_count(), 0);
}

#[test]
fn capture_rejects_empty_duplicate_foreign_and_disposed_targets() {
    let mut first = Runtime::empty();
    let figure = first
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    assert!(matches!(
        first.animations().capture_figures([]),
        Err(AnimationError::EmptyCapture)
    ));
    assert!(matches!(
        first.animations().capture_figures([figure, figure]),
        Err(AnimationError::DuplicateTarget)
    ));

    let foreign_capture = first.animations().capture_figures([figure]).unwrap();
    let mut second = Runtime::empty();
    assert!(matches!(
        second.animations().transition_bounds(
            foreign_capture,
            BoundsTransition::new(Duration::from_millis(100))
        ),
        Err(AnimationError::ForeignCapture)
    ));

    let disposed_capture = first.animations().capture_figures([figure]).unwrap();
    first.dispose_subtree(figure).unwrap();
    assert!(matches!(
        first.animations().transition_bounds(
            disposed_capture,
            BoundsTransition::new(Duration::from_millis(100))
        ),
        Err(AnimationError::DisposedTarget)
    ));
}

#[test]
fn stagger_uses_capture_order_without_mutating_source_bounds() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let first = tree
        .builder()
        .add_child(parent, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let second = tree
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::new(20.0, 0.0, 10.0, 10.0)),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.advance_time(time(0)).unwrap();
    let first_transform = runtime
        .animations()
        .bind_figure(first, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let second_transform = runtime
        .animations()
        .bind_figure(second, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let capture = runtime
        .animations()
        .capture_figures([first, second])
        .unwrap();
    runtime
        .figure(first)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime
        .figure(second)
        .unwrap()
        .set_bounds(Rectangle::new(60.0, 0.0, 10.0, 10.0))
        .unwrap();
    runtime
        .animations()
        .transition_bounds(
            capture,
            BoundsTransition::new(Duration::from_millis(100))
                .with_stagger(Duration::from_millis(50)),
        )
        .unwrap();

    runtime.advance_time(time(25)).unwrap();
    assert_eq!(
        runtime.animations().value(first_transform).unwrap(),
        Affine2D::from_translation(-30.0, 0.0)
    );
    assert_eq!(
        runtime.animations().value(second_transform).unwrap(),
        Affine2D::from_translation(-40.0, 0.0)
    );
    assert_eq!(
        runtime.tree().figure_bounds(first),
        Some(Rectangle::new(40.0, 0.0, 10.0, 10.0))
    );
    assert_eq!(
        runtime.tree().figure_bounds(second),
        Some(Rectangle::new(60.0, 0.0, 10.0, 10.0))
    );
}

#[test]
fn nonzero_to_zero_bounds_transition_is_rejected_without_active_work() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let capture = runtime.animations().capture_figures([figure]).unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 0.0, 10.0))
        .unwrap();
    assert_eq!(
        runtime
            .animations()
            .transition_bounds(capture, BoundsTransition::new(Duration::from_millis(100))),
        Err(AnimationError::IncompatibleBoundsTransition)
    );
    assert_eq!(runtime.animations().active_animation_count(), 0);
    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Some(Rectangle::new(20.0, 0.0, 0.0, 10.0))
    );
}
