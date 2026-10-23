mod support;

use support::record_full_submission;

// External contracts for P2-M01 Figure bounds capture and transition.

use std::time::Duration;

use novadraw::connection::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionFigure,
    ConnectionId, CoordinateSpace, RouterBinding,
};
use novadraw::event::MonotonicTime;
use novadraw::{
    animation::{
        AnimationError, AnimationMode, AnimationPlan, AnimationStart, AnimationState,
        AnimationSuppression, AnimationTransactionError, BoundsTransition, ConnectionPulse,
        ConnectionRouteTransition, Easing, InteractionGeometryPolicy, Motion, Tween,
        ViewportTransition,
    },
    figure::RectangleFigure,
    geometry::Affine2D,
    geometry::Point,
    geometry::Rectangle,
    graphics::Color,
    graphics::{DashPattern, StrokeStyle},
    layout::StackLayout,
    render::RenderCommandKind,
    runtime::Runtime,
    tree::FigureTree,
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

fn contains_transform(runtime: &mut Runtime, expected: Affine2D) -> bool {
    record_full_submission(runtime)
        .commands
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
    runtime.animation().advance_time(time(0)).unwrap();
    let capture = runtime
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();

    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(50.0, 60.0, 40.0, 20.0))
        .unwrap();
    let AnimationStart::Running(animation) = runtime
        .animation()
        .timeline()
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
        Ok(Rectangle::new(50.0, 60.0, 40.0, 20.0))
    );
    assert!(contains_transform(
        &mut runtime,
        Affine2D::from_translation(-40.0, -40.0) * Affine2D::from_scale(0.5, 0.5)
    ));

    runtime.animation().advance_time(time(50)).unwrap();
    assert!(contains_transform(
        &mut runtime,
        Affine2D::from_translation(-20.0, -20.0) * Affine2D::from_scale(0.75, 0.75)
    ));
    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(animation).unwrap(),
        AnimationState::Completed
    );
}

#[test]
fn unchanged_and_zero_duration_transitions_are_suppressed() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let unchanged = runtime
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .transition_bounds(unchanged, BoundsTransition::new(Duration::from_millis(100)))
            .unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta)
    );

    let zero_duration = runtime
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 10.0, 10.0))
        .unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .transition_bounds(zero_duration, BoundsTransition::new(Duration::ZERO))
            .unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::NoVisualDelta)
    );
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
}

#[test]
fn capture_rejects_empty_duplicate_foreign_and_disposed_targets() {
    let mut first = Runtime::empty();
    let figure = first
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    assert!(matches!(
        first.animation().timeline().capture_figures([]),
        Err(AnimationError::EmptyCapture)
    ));
    assert!(matches!(
        first
            .animation()
            .timeline()
            .capture_figures([figure, figure]),
        Err(AnimationError::DuplicateTarget)
    ));

    let foreign_capture = first
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    let mut second = Runtime::empty();
    assert!(matches!(
        second.animation().timeline().transition_bounds(
            foreign_capture,
            BoundsTransition::new(Duration::from_millis(100))
        ),
        Err(AnimationError::ForeignCapture)
    ));

    let disposed_capture = first
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    first.dispose_subtree(figure).unwrap();
    assert!(matches!(
        first.animation().timeline().transition_bounds(
            disposed_capture,
            BoundsTransition::new(Duration::from_millis(100))
        ),
        Err(AnimationError::Identity(novadraw::tree::TreeQueryError::UnknownFigure(id)))
            if id == figure
    ));
}

#[test]
fn stagger_uses_capture_order_without_mutating_source_bounds() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
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
    runtime.animation().advance_time(time(0)).unwrap();
    let first_transform = runtime
        .animation()
        .timeline()
        .bind_figure(first, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let second_transform = runtime
        .animation()
        .timeline()
        .bind_figure(second, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let capture = runtime
        .animation()
        .timeline()
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
        .animation()
        .timeline()
        .transition_bounds(
            capture,
            BoundsTransition::new(Duration::from_millis(100))
                .with_stagger(Duration::from_millis(50)),
        )
        .unwrap();

    runtime.animation().advance_time(time(25)).unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(first_transform)
            .unwrap(),
        Affine2D::from_translation(-30.0, 0.0)
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(second_transform)
            .unwrap(),
        Affine2D::from_translation(-40.0, 0.0)
    );
    assert_eq!(
        runtime.tree().figure_bounds(first),
        Ok(Rectangle::new(40.0, 0.0, 10.0, 10.0))
    );
    assert_eq!(
        runtime.tree().figure_bounds(second),
        Ok(Rectangle::new(60.0, 0.0, 10.0, 10.0))
    );
}

#[test]
fn nonzero_to_zero_bounds_transition_is_rejected_without_active_work() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let capture = runtime
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    runtime
        .figure(figure)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 0.0, 0.0, 10.0))
        .unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .transition_bounds(capture, BoundsTransition::new(Duration::from_millis(100))),
        Err(AnimationError::IncompatibleBoundsTransition)
    );
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Ok(Rectangle::new(20.0, 0.0, 0.0, 10.0))
    );
}

#[test]
fn static_animation_modes_allow_collapsed_layout_without_building_a_transform() {
    for (mode, suppression) in [
        (AnimationMode::Disabled, AnimationSuppression::Disabled),
        (
            AnimationMode::ReducedMotion,
            AnimationSuppression::ReducedMotionStaticFallback,
        ),
    ] {
        let mut runtime = Runtime::empty();
        let figure = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 50.0)))
            .unwrap();
        runtime.animation().timeline().set_mode(mode);
        let (_, start) = runtime
            .animation()
            .transition_bounds(
                [figure],
                BoundsTransition::new(Duration::from_millis(210)),
                |runtime| runtime.figure(figure)?.set_bounds(Rectangle::ZERO),
            )
            .expect("static fallback must accept the final collapsed layout");
        assert_eq!(start, AnimationStart::Suppressed(suppression));
        assert_eq!(
            runtime.tree().figure_bounds(figure).unwrap(),
            Rectangle::ZERO
        );
        assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
        assert_eq!(runtime.animation().timeline().temporary_visual_count(), 0);
        assert_eq!(runtime.animation().timeline().next_wake_deadline(), None);
        assert_eq!(runtime.animation().timeline().stats().tracks_sampled, 0);
    }
}

#[test]
fn static_bounds_fallback_still_rejects_a_disposed_capture_target() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 50.0)))
        .unwrap();
    let capture = runtime
        .animation()
        .timeline()
        .capture_figures([figure])
        .unwrap();
    runtime
        .animation()
        .timeline()
        .set_mode(AnimationMode::Disabled);
    runtime.dispose_subtree(figure).unwrap();
    assert!(matches!(
        runtime.animation().timeline().transition_bounds(capture, BoundsTransition::new(Duration::from_millis(210))),
        Err(AnimationError::Identity(novadraw::tree::TreeQueryError::UnknownFigure(id))) if id == figure
    ));
}

#[test]
fn runtime_transaction_stabilizes_layout_before_installing_presentation() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0)))
        .expect("valid FigureTree construction");
    let child = tree
        .builder()
        .add_child(parent, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    tree.builder()
        .set_layout_manager(parent, Box::new(StackLayout::new()))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.animation().advance_time(time(0)).unwrap();

    let (changed, start) = runtime
        .animation()
        .transition_bounds(
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
        Ok(Rectangle::new(0.0, 0.0, 200.0, 160.0))
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

    let empty = runtime.animation().transition_bounds(
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

    let mutation = runtime.animation().transition_bounds(
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
        Ok(Rectangle::new(0.0, 0.0, 10.0, 10.0))
    );
    assert_eq!(runtime.animation().timeline().active_animation_count(), 0);
}

#[test]
fn viewport_transaction_animates_pan_without_changing_committed_origin() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)))
        .expect("valid FigureTree construction");
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
    runtime.animation().advance_time(time(0)).unwrap();

    let (changed, start) = runtime
        .animation()
        .transition_viewport(
            &viewport,
            ViewportTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime
                    .viewports()
                    .set_view_location(viewport.figure_id(), 40.0, 20.0)
            },
        )
        .unwrap();
    assert!(changed);
    assert!(matches!(start, AnimationStart::Running(_)));
    assert_eq!(
        viewport.view_location(),
        novadraw::geometry::Point::new(40.0, 20.0)
    );

    let transform = runtime
        .animation()
        .timeline()
        .bind_figure(contents, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    assert_eq!(
        runtime.animation().timeline().value(transform).unwrap(),
        Affine2D::from_translation(40.0, 20.0)
    );
    runtime.animation().advance_time(time(50)).unwrap();
    assert_eq!(
        runtime.animation().timeline().value(transform).unwrap(),
        Affine2D::from_translation(20.0, 10.0)
    );
    assert_eq!(
        viewport.view_location(),
        novadraw::geometry::Point::new(40.0, 20.0)
    );
}

fn connection_fixture() -> (
    Runtime,
    novadraw::tree::FigureId,
    novadraw::tree::FigureId,
    novadraw::tree::FigureId,
    ConnectionId,
) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .unwrap();
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)))
        .unwrap();
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new().with_stroke_style(
            StrokeStyle::default().with_dash_pattern(DashPattern::Dash),
        )))
        .unwrap();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let direct_router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: direct_router,
            },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    (runtime, root, target, figure, connection)
}

#[test]
fn compatible_connection_route_interpolates_without_mutating_final_route() {
    let (mut runtime, _root, target, figure, _connection) = connection_fixture();
    runtime.animation().advance_time(time(0)).unwrap();
    let before = runtime
        .tree()
        .connection_route_points(figure)
        .unwrap()
        .unwrap()
        .clone();

    let (_, start) = runtime
        .animation()
        .transition_connection_routes(
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
        .unwrap()
        .clone();
    assert_ne!(before, committed);

    let route = runtime
        .animation()
        .timeline()
        .bind_connection_route(figure)
        .unwrap();
    assert_ne!(
        runtime.animation().timeline().value(route).unwrap(),
        committed
    );
    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animation().timeline().value(route).unwrap(),
        committed
    );
    assert_eq!(
        runtime.tree().connection_route_points(figure),
        Ok(Some(&committed))
    );
}

#[test]
fn incompatible_connection_route_crossfades_and_retires_old_visual() {
    let (mut runtime, _root, _target, figure, connection) = connection_fixture();
    runtime.animation().advance_time(time(0)).unwrap();
    let router = runtime
        .connections()
        .register_router(Box::new(BendpointConnectionRouter));

    let (_, start) = runtime
        .animation()
        .transition_connection_routes(
            [figure],
            ConnectionRouteTransition::new(Duration::from_millis(100)),
            |runtime| {
                runtime.connections().set_route_configuration(
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
            .unwrap()
            .len(),
        3
    );
    assert_eq!(runtime.animation().timeline().temporary_visual_count(), 1);

    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(runtime.animation().timeline().temporary_visual_count(), 0);
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn connection_dash_flow_uses_continuous_procedural_phase() {
    let (mut runtime, _root, _target, figure, _connection) = connection_fixture();
    runtime.animation().advance_time(time(0)).unwrap();
    let AnimationStart::Running(animation) = runtime
        .animation()
        .timeline()
        .start_connection_dash_flow(figure, 1.0)
        .unwrap()
    else {
        panic!("dashed connection must start continuous flow");
    };
    let phase = runtime
        .animation()
        .timeline()
        .bind_connection_dash_offset(figure)
        .unwrap();

    runtime.animation().advance_time(time(100)).unwrap();
    assert!((runtime.animation().timeline().value(phase).unwrap() + 0.1).abs() < 1e-9);
    assert_eq!(
        runtime.animation().timeline().state(animation).unwrap(),
        AnimationState::Running
    );
    assert!(runtime.animation().timeline().cancel(animation).unwrap());
    assert_eq!(runtime.animation().timeline().value(phase).unwrap(), 0.0);
}

#[test]
fn connection_route_and_dash_share_one_composable_presentation_family() {
    let (mut runtime, _root, _target, figure, _connection) = connection_fixture();
    runtime.animation().advance_time(time(0)).unwrap();
    let route = runtime
        .animation()
        .timeline()
        .bind_connection_route(figure)
        .unwrap();
    let committed = runtime.animation().timeline().value(route).unwrap();
    let shifted = committed
        .iter()
        .map(|point| Point::new(point.x() - 10.0, point.y()))
        .collect();
    let route_plan = AnimationPlan::track(
        route,
        Motion::Tween(
            Tween::between(shifted, committed.clone(), Duration::from_millis(200)).unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(route_animation) =
        runtime.animation().timeline().start(route_plan).unwrap()
    else {
        panic!("route transition must run");
    };
    let AnimationStart::Running(dash_animation) = runtime
        .animation()
        .timeline()
        .start_connection_dash_flow(figure, 1.0)
        .unwrap()
    else {
        panic!("dash flow must run beside route transition");
    };

    assert_eq!(runtime.animation().timeline().active_animation_count(), 2);
    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .state(route_animation)
            .unwrap(),
        AnimationState::Running
    );
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .state(dash_animation)
            .unwrap(),
        AnimationState::Running
    );
    let frame = record_full_submission(&mut runtime);
    let (painted_route, painted_stroke) = frame
        .commands
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::Polyline { points, stroke, .. } => Some((points, stroke)),
            _ => None,
        })
        .expect("combined Connection presentation must paint one polyline");
    assert!((painted_route.as_slice()[0].x() - (committed.as_slice()[0].x() - 5.0)).abs() < 1e-9);
    assert!((painted_stroke.dash_offset() + 0.1).abs() < 1e-9);
}

#[test]
fn connection_pulse_uses_arc_length_and_hands_off_to_decoration() {
    let (mut runtime, _root, _target, figure, _connection) = connection_fixture();
    let decoration = runtime
        .figure(figure)
        .unwrap()
        .add(Box::new(RectangleFigure::new(-3.0, -3.0, 6.0, 6.0)))
        .unwrap();
    runtime.animation().advance_time(time(0)).unwrap();
    let decoration_transform = runtime
        .animation()
        .timeline()
        .bind_figure(decoration, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();

    let AnimationStart::Running(animation) = runtime
        .animation()
        .timeline()
        .start_connection_pulse(
            figure,
            ConnectionPulse::new(Duration::from_millis(100), 4.0, Color::RED)
                .with_endpoint_handoff(decoration, Duration::from_millis(50)),
        )
        .unwrap()
    else {
        panic!("connection pulse must run");
    };
    assert_eq!(runtime.animation().timeline().temporary_visual_count(), 1);

    runtime.animation().advance_time(time(100)).unwrap();
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(decoration_transform)
            .unwrap(),
        Affine2D::from_uniform_scale(1.35)
    );
    assert_eq!(runtime.animation().timeline().temporary_visual_count(), 1);

    runtime.animation().advance_time(time(150)).unwrap();
    assert_eq!(
        runtime.animation().timeline().state(animation).unwrap(),
        AnimationState::Completed
    );
    assert_eq!(runtime.animation().timeline().temporary_visual_count(), 0);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .value(decoration_transform)
            .unwrap(),
        Affine2D::IDENTITY
    );
}
