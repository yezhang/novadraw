//! External contracts for P2-M01 Figure bounds capture and transition.

use std::time::Duration;

use novadraw::{
    Affine2D, Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, Color,
    ConnectionFigure, CoordinateSpace, FigureTree, MonotonicTime, Point, Rectangle,
    RectangleFigure, RouterBinding, Runtime, StackLayout,
    animation::{
        AnimationError, AnimationStart, AnimationState, AnimationSuppression,
        AnimationTransactionError, BoundsTransition, ConnectionPulse, ConnectionRouteTransition,
        Easing, InteractionGeometryPolicy, ViewportTransition,
    },
    graphics::{DashPattern, StrokeStyle},
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

#[test]
fn runtime_transaction_stabilizes_layout_before_installing_presentation() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0)));
    let child = tree
        .builder()
        .add_child(parent, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    tree.builder()
        .set_layout_manager(parent, Box::new(StackLayout::new()))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.advance_time(time(0)).unwrap();

    let (changed, start) = runtime
        .transition_bounds_transaction(
            [child],
            BoundsTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime
                    .figure(parent)?
                    .set_bounds(Rectangle::new(0.0, 0.0, 200.0, 160.0))
            },
        )
        .unwrap();

    assert!(changed);
    assert!(matches!(start, AnimationStart::Running(_)));
    assert_eq!(
        runtime.tree().figure_bounds(child),
        Some(Rectangle::new(0.0, 0.0, 200.0, 160.0))
    );
    assert!(contains_transform(
        &mut runtime,
        Affine2D::from_scale(0.5, 0.5)
    ));
}

#[test]
fn runtime_transaction_reports_capture_and_mutation_phases() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();

    let empty = runtime.transition_bounds_transaction(
        [],
        BoundsTransition::new(Duration::from_millis(100)),
        |_runtime| Ok::<_, &'static str>(()),
    );
    assert_eq!(
        empty,
        Err(AnimationTransactionError::Capture(
            AnimationError::EmptyCapture
        ))
    );

    let mutation = runtime.transition_bounds_transaction(
        [figure],
        BoundsTransition::new(Duration::from_millis(100)),
        |_runtime| Err::<(), _>("rejected source transaction"),
    );
    assert_eq!(
        mutation,
        Err(AnimationTransactionError::Mutation(
            "rejected source transaction"
        ))
    );
    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Some(Rectangle::new(0.0, 0.0, 10.0, 10.0))
    );
    assert_eq!(runtime.animations().active_animation_count(), 0);
}

#[test]
fn viewport_transaction_animates_pan_without_changing_committed_origin() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));
    let viewport = tree
        .builder()
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 300.0, 200.0))
        .unwrap();
    let contents = tree
        .builder()
        .add_child(
            viewport.figure_id(),
            Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.advance_time(time(0)).unwrap();

    let (changed, start) = runtime
        .transition_viewport_transaction(
            &viewport,
            ViewportTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime
                    .viewport(viewport.figure_id())?
                    .set_view_location(40.0, 20.0)
            },
        )
        .unwrap();
    assert!(changed);
    assert!(matches!(start, AnimationStart::Running(_)));
    assert_eq!(viewport.view_location(), novadraw::Point::new(40.0, 20.0));

    let transform = runtime
        .animations()
        .bind_figure(contents, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    assert_eq!(
        runtime.animations().value(transform).unwrap(),
        Affine2D::from_translation(40.0, 20.0)
    );
    runtime.advance_time(time(50)).unwrap();
    assert_eq!(
        runtime.animations().value(transform).unwrap(),
        Affine2D::from_translation(20.0, 10.0)
    );
    assert_eq!(viewport.view_location(), novadraw::Point::new(40.0, 20.0));
}

fn connection_fixture() -> (
    Runtime,
    novadraw::FigureId,
    novadraw::FigureId,
    novadraw::FigureId,
    novadraw::ConnectionId,
) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .unwrap();
    let source = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .unwrap();
    let target = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)))
        .unwrap();
    let figure = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new().with_stroke_style(
            StrokeStyle::default().with_dash_pattern(DashPattern::Dash),
        )))
        .unwrap();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();
    runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    (runtime, root, target, figure, connection)
}

#[test]
fn compatible_connection_route_interpolates_without_mutating_final_route() {
    let (mut runtime, _root, target, figure, _connection) = connection_fixture();
    runtime.advance_time(time(0)).unwrap();
    let before = runtime
        .tree()
        .connection_route_points(figure)
        .unwrap()
        .clone();

    let (_, start) = runtime
        .transition_connection_routes_transaction(
            [figure],
            ConnectionRouteTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime
                    .figure(target)?
                    .set_bounds(Rectangle::new(360.0, 210.0, 100.0, 60.0))
            },
        )
        .unwrap();
    assert!(matches!(start, AnimationStart::Running(_)));
    let committed = runtime
        .tree()
        .connection_route_points(figure)
        .unwrap()
        .clone();
    assert_ne!(before, committed);

    let route = runtime.animations().bind_connection_route(figure).unwrap();
    assert_ne!(runtime.animations().value(route).unwrap(), committed);
    runtime.advance_time(time(100)).unwrap();
    assert_eq!(runtime.animations().value(route).unwrap(), committed);
    assert_eq!(
        runtime.tree().connection_route_points(figure),
        Some(&committed)
    );
}

#[test]
fn incompatible_connection_route_crossfades_and_retires_old_visual() {
    let (mut runtime, _root, _target, figure, connection) = connection_fixture();
    runtime.advance_time(time(0)).unwrap();
    let router = runtime.register_connection_router(Box::new(BendpointConnectionRouter));

    let (_, start) = runtime
        .transition_connection_routes_transaction(
            [figure],
            ConnectionRouteTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime.set_connection_route_configuration(
                    connection,
                    RouterBinding::Explicit { router },
                    Some(Box::new(BendpointConstraint::new(vec![
                        Bendpoint::Absolute(Point::new(220.0, 80.0)),
                    ]))),
                )
            },
        )
        .unwrap();
    assert!(matches!(start, AnimationStart::Running(_)));
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(runtime.animations().temporary_visual_count(), 1);

    runtime.advance_time(time(100)).unwrap();
    assert_eq!(runtime.animations().temporary_visual_count(), 0);
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn connection_dash_flow_uses_continuous_procedural_phase() {
    let (mut runtime, _root, _target, figure, _connection) = connection_fixture();
    runtime.advance_time(time(0)).unwrap();
    let AnimationStart::Running(animation) = runtime
        .animations()
        .start_connection_dash_flow(figure, 1.0)
        .unwrap()
    else {
        panic!("dashed connection must start continuous flow");
    };
    let phase = runtime
        .animations()
        .bind_connection_dash_offset(figure)
        .unwrap();

    runtime.advance_time(time(100)).unwrap();
    assert!((runtime.animations().value(phase).unwrap() - 0.1).abs() < 1e-9);
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Running
    );
    assert!(runtime.animations().cancel(animation).unwrap());
    assert_eq!(runtime.animations().value(phase).unwrap(), 0.0);
}

#[test]
fn connection_pulse_uses_arc_length_and_hands_off_to_decoration() {
    let (mut runtime, _root, _target, figure, _connection) = connection_fixture();
    let decoration = runtime
        .container(figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(-3.0, -3.0, 6.0, 6.0)))
        .unwrap();
    runtime.advance_time(time(0)).unwrap();
    let decoration_transform = runtime
        .animations()
        .bind_figure(decoration, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();

    let AnimationStart::Running(animation) = runtime
        .animations()
        .start_connection_pulse(
            figure,
            ConnectionPulse::new(Duration::from_millis(100), 4.0, Color::RED)
                .with_endpoint_handoff(decoration, Duration::from_millis(50)),
        )
        .unwrap()
    else {
        panic!("connection pulse must run");
    };
    assert_eq!(runtime.animations().temporary_visual_count(), 1);

    runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animations().value(decoration_transform).unwrap(),
        Affine2D::from_uniform_scale(1.35)
    );
    assert_eq!(runtime.animations().temporary_visual_count(), 1);

    runtime.advance_time(time(150)).unwrap();
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Completed
    );
    assert_eq!(runtime.animations().temporary_visual_count(), 0);
    assert_eq!(
        runtime.animations().value(decoration_transform).unwrap(),
        Affine2D::IDENTITY
    );
}
