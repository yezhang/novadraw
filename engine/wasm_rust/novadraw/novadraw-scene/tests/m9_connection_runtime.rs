use std::{any::TypeId, marker::PhantomData};

use novadraw_geometry::{Point, Vector};
use novadraw_render::{
    BackendCapabilities, RenderOutcome, SurfaceInfo, command::RenderCommandKind,
};
use novadraw_scene::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionFigure,
    ConnectionResolution, ConnectionRouter, ConnectionRuntimeError, CoordinateSpace, DirectRouter,
    FanRouter, FigureId, MANHATTAN_DEFAULT_LANE_SPACING, MANHATTAN_DEFAULT_MINIMUM_STUB,
    ManhattanConnectionRouter, RectangleFigure, RouteError, RouteOutput, RouteRequest,
    RouterBinding, Runtime, UnresolvedConnection, ViewportFigure, XYConstraint, XYLayout,
};

struct ConstraintA;
struct ConstraintB;

struct TypedRouter<T>(PhantomData<T>);

impl<T: 'static> ConnectionRouter for TypedRouter<T> {
    fn route(&self, _request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        Err(RouteError::InvalidConstraint)
    }

    fn constraint_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
    }

    fn constraint_type_name(&self) -> Option<&'static str> {
        Some(std::any::type_name::<T>())
    }
}

fn runtime_fixture() -> (Runtime, FigureId, FigureId, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)));
    let source = runtime.add_figure(root, Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)));
    let target = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)),
    );
    let connection = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    (runtime, root, source, target, connection)
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 500.0,
        logical_height: 300.0,
        pixel_width: 500,
        pixel_height: 300,
        scale_factor: 1.0,
    }
}

#[test]
fn resolved_route_replaces_dependencies_and_targeted_invalidation_marks_dirty() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    let output = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(output.points().len(), 2);
    let path_bounds = output.points().bounds().unwrap().inflate(1.0, 1.0);
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Some(path_bounds)
    );
    let local_points = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap();
    assert_eq!(
        local_points.get(0),
        output
            .points()
            .get(0)
            .map(|point| point - Point::new(path_bounds.x, path_bounds.y))
    );
    let source_point = output.points().get(0).unwrap();
    let target_point = output.points().get(1).unwrap();
    let midpoint = Point::new(
        (source_point.x() + target_point.x()) / 2.0,
        (source_point.y() + target_point.y()) / 2.0,
    );
    assert_eq!(
        runtime.tree().hit_test_simple((midpoint.x(), midpoint.y())),
        Some(connection_figure)
    );
    assert!(
        runtime
            .tree()
            .render()
            .commands()
            .iter()
            .any(|command| matches!(&command.kind, RenderCommandKind::Polyline { .. }))
    );
    assert!(matches!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    ));
    assert!(
        runtime
            .connection_state(connection)
            .unwrap()
            .dependency_count
            >= 4
    );

    assert!(runtime.set_bounds(
        source,
        novadraw_geometry::Rectangle::new(25.0, 35.0, 80.0, 40.0),
    ));
    assert_eq!(runtime.dirty_connections(), vec![connection]);
}

#[test]
fn removing_connection_figure_cleans_runtime_state() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    assert!(runtime.remove_figure(root, connection_figure));
    assert_eq!(
        runtime.connection_state(connection),
        Err(ConnectionRuntimeError::UnknownConnection(connection))
    );
}

#[test]
fn normal_frame_automatically_resolves_dirty_connection_routes() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    assert!(runtime.has_pending_update());
    let initial = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .expect("dirty route must schedule a frame");
    runtime.complete_submission(
        initial.session_id,
        initial.frame_id,
        RenderOutcome::Presented,
    );
    let initial_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();
    assert!(matches!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(!runtime.has_pending_update());

    assert!(runtime.set_bounds(
        source,
        novadraw_geometry::Rectangle::new(80.0, 60.0, 80.0, 40.0),
    ));
    assert!(runtime.has_pending_update());
    let moved = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .expect("owner geometry change must reroute in the same frame");
    runtime.complete_submission(moved.session_id, moved.frame_id, RenderOutcome::Presented);
    assert_ne!(
        runtime.tree().figure_bounds(connection_figure),
        Some(initial_bounds)
    );
    assert!(matches!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(!runtime.has_pending_update());
}

#[test]
fn layout_output_geometry_automatically_invalidates_and_reroutes() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    runtime
        .set_layout_manager(root, Box::new(XYLayout::new()))
        .unwrap();
    runtime
        .set_layout_constraint(source, XYConstraint::at_size(20.0, 30.0, 80.0, 40.0))
        .unwrap();
    runtime
        .set_layout_constraint(target, XYConstraint::at_size(300.0, 170.0, 100.0, 60.0))
        .unwrap();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    runtime.prepare_frame().expect("initial stable frame");
    let initial_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();

    runtime
        .set_layout_constraint(source, XYConstraint::at_size(90.0, 70.0, 80.0, 40.0))
        .unwrap();
    runtime
        .prepare_frame()
        .expect("layout commit must schedule route work");

    assert_ne!(
        runtime.tree().figure_bounds(connection_figure),
        Some(initial_bounds)
    );
    assert!(matches!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 2 }
    ));
}

#[test]
fn missing_endpoint_is_unresolved_and_binding_it_restores_dirty_state() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            None,
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime.resolve_connection_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::MissingTarget,
        ))
    );
    assert_eq!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::MissingTarget)
    );

    assert!(
        runtime
            .set_connection_target(connection, Some(target_anchor))
            .unwrap()
    );
    assert!(matches!(
        runtime.connection_state(connection).unwrap().resolution,
        ConnectionResolution::Dirty { .. }
    ));
}

#[test]
fn unresolved_transition_clears_previously_committed_geometry() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
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

    runtime.set_connection_target(connection, None).unwrap();
    assert_eq!(
        runtime.resolve_connection_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::MissingTarget,
        ))
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Some(novadraw_geometry::Rectangle::ZERO)
    );
    assert!(
        runtime
            .tree()
            .connection_route_points(connection_figure)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn shared_router_and_anchor_cannot_be_removed_while_referenced() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(DirectRouter));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    assert!(matches!(
        runtime.remove_connection_router(router),
        Err(ConnectionRuntimeError::RouterInUse(candidate)) if candidate == router
    ));
    assert!(matches!(
        runtime.remove_connection_anchor(source_anchor),
        Err(ConnectionRuntimeError::AnchorInUse(candidate)) if candidate == source_anchor
    ));

    runtime.remove_connection_state(connection).unwrap();
    runtime.remove_connection_router(router).unwrap();
    runtime.remove_connection_anchor(source_anchor).unwrap();
    runtime.remove_connection_anchor(target_anchor).unwrap();
}

#[test]
fn incompatible_router_change_is_rejected_atomically() {
    let (mut runtime, _root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router_a =
        runtime.register_connection_router(Box::new(TypedRouter::<ConstraintA>(PhantomData)));
    let router_b =
        runtime.register_connection_router(Box::new(TypedRouter::<ConstraintB>(PhantomData)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router: router_a },
            Some(Box::new(ConstraintA)),
        )
        .unwrap();

    assert!(matches!(
        runtime.set_connection_router_binding(
            connection,
            RouterBinding::Explicit { router: router_b },
        ),
        Err(ConnectionRuntimeError::ConstraintTypeMismatch { .. })
    ));
    assert_eq!(
        runtime.connection_state(connection).unwrap().router,
        RouterBinding::Explicit { router: router_a }
    );
}

#[test]
fn connection_cannot_anchor_to_its_own_subtree() {
    let (mut runtime, root, _source, target, connection_figure) = runtime_fixture();
    let self_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(connection_figure)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(self_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime.resolve_connection_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::DependencyCycle),
        ))
    );
}

#[test]
fn bendpoint_router_preserves_absolute_and_relative_constraints() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(BendpointConnectionRouter));
    let constraint = BendpointConstraint::new(vec![
        Bendpoint::Absolute(Point::new(160.0, 80.0)),
        Bendpoint::Relative {
            source_offset: Vector::new(10.0, 0.0),
            target_offset: Vector::new(-10.0, 0.0),
            weight: 0.5,
        },
    ]);
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            Some(Box::new(constraint)),
        )
        .unwrap();

    let output = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(output.points().len(), 4);
    assert_eq!(output.points().get(1), Some(Point::new(160.0, 80.0)));
    assert_eq!(output.points().get(2), Some(Point::new(205.0, 125.0)));
    assert_eq!(output.metadata().source.reference, Point::new(160.0, 80.0));
    assert_eq!(output.metadata().target.reference, Point::new(205.0, 125.0));
}

#[test]
fn manhattan_router_emits_only_orthogonal_non_duplicate_segments() {
    let (mut runtime, root, source, target, connection_figure) = runtime_fixture();
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let output = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert!(output.points().len() >= 3);
    for segment in output.points().as_slice().windows(2) {
        assert_ne!(segment[0], segment[1]);
        assert!(segment[0].x() == segment[1].x() || segment[0].y() == segment[1].y());
    }
    let segments = output.points().as_slice().windows(2).collect::<Vec<_>>();
    for segment in [segments.first().unwrap(), segments.last().unwrap()] {
        assert!((segment[1] - segment[0]).length() >= MANHATTAN_DEFAULT_MINIMUM_STUB);
    }
}

#[test]
fn fan_router_uses_stable_child_order_and_recenters_after_removal() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let second_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let third_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.register_connection_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), 16.0).unwrap(),
    ));
    let mut connections = Vec::new();
    for figure in [first_figure, second_figure, third_figure] {
        connections.push(
            runtime
                .register_connection_state(
                    figure,
                    Some(source_anchor),
                    Some(target_anchor),
                    RouterBinding::Explicit { router },
                    None,
                )
                .unwrap(),
        );
    }

    let outputs: Vec<_> = connections
        .iter()
        .map(|connection| {
            runtime
                .resolve_connection_route(*connection, CoordinateSpace::ChildContent(root))
                .unwrap()
        })
        .collect();

    assert_eq!(outputs[0].points().len(), 3);
    assert_eq!(outputs[1].points().len(), 2);
    assert_eq!(outputs[2].points().len(), 3);
    assert_ne!(outputs[0].points().get(1), outputs[2].points().get(1));

    assert!(runtime.remove_figure(root, first_figure));
    assert_eq!(runtime.dirty_connections(), connections[1..]);
    let recentered = runtime
        .resolve_connection_route(connections[1], CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(recentered.points().len(), 3);
}

#[test]
fn shared_manhattan_reserves_lanes_across_different_anchor_pairs() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    assert!(runtime.set_bounds(
        target,
        novadraw_geometry::Rectangle::new(300.0, 70.0, 100.0, 60.0),
    ));
    let second_source = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new(20.0, 130.0, 80.0, 40.0)),
    );
    let second_target = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)),
    );
    let second_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    let first_source_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let first_target_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let second_source_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(second_source)));
    let second_target_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(second_target)));
    let first = runtime
        .register_connection_state(
            first_figure,
            Some(first_source_anchor),
            Some(first_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    let second = runtime
        .register_connection_state(
            second_figure,
            Some(second_source_anchor),
            Some(second_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let first_output = runtime
        .resolve_connection_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert!(matches!(
        runtime.connection_state(second).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    ));
    let second_output = runtime
        .resolve_connection_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();
    let first_lane = first_output.points().get(1).unwrap().x();
    let second_lane = second_output.points().get(1).unwrap().x();
    assert_eq!(
        (first_lane - second_lane).abs(),
        MANHATTAN_DEFAULT_LANE_SPACING
    );

    assert!(runtime.set_bounds(
        second_source,
        novadraw_geometry::Rectangle::new(25.0, 130.0, 80.0, 40.0),
    ));
    assert_eq!(runtime.dirty_connections(), vec![first, second]);
    runtime
        .resolve_connection_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert!(runtime.remove_figure(root, first_figure));
    let recentered = runtime
        .resolve_connection_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();
    let expected_lane = (recentered.metadata().source.site.point.x()
        + recentered.metadata().target.site.point.x())
        / 2.0;
    assert_eq!(recentered.points().get(1).unwrap().x(), expected_lane);
}

#[test]
fn manhattan_reservations_are_isolated_by_router_id() {
    let (mut runtime, root, source, target, first_figure) = runtime_fixture();
    let second_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let first_router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    let second_router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    let first = runtime
        .register_connection_state(
            first_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: first_router,
            },
            None,
        )
        .unwrap();
    let second = runtime
        .register_connection_state(
            second_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: second_router,
            },
            None,
        )
        .unwrap();

    let first_output = runtime
        .resolve_connection_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    let second_output = runtime
        .resolve_connection_route(second, CoordinateSpace::ChildContent(root))
        .unwrap();

    assert_eq!(
        first_output.points().get(1).unwrap().x(),
        second_output.points().get(1).unwrap().x()
    );
}

#[test]
fn divergent_viewport_topology_is_rejected_and_reparent_recovers() {
    let mut tree = novadraw_scene::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)));
    let viewport = builder.add_child_to(
        root,
        Box::new(ViewportFigure::new(20.0, 20.0, 240.0, 180.0)),
    );
    let contents = builder.add_child_to(
        viewport,
        Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)),
    );
    let source = builder.add_child_to(
        contents,
        Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)),
    );
    let target = builder.add_child_to(
        root,
        Box::new(RectangleFigure::new(400.0, 200.0, 100.0, 60.0)),
    );
    let connection_figure = builder.add_child_to(root, Box::new(ConnectionFigure::new()));
    let mut runtime = Runtime::new(tree);
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime.resolve_connection_route(connection, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::UnsupportedViewportTopology),
        ))
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure),
        Some(novadraw_geometry::Rectangle::ZERO)
    );

    assert!(runtime.reparent(source, root));
    let recovered = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(recovered.points().len(), 2);
}

#[test]
fn matching_viewport_topology_routes_normally() {
    let mut tree = novadraw_scene::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)));
    let viewport = builder.add_child_to(
        root,
        Box::new(ViewportFigure::new(20.0, 20.0, 300.0, 220.0)),
    );
    let contents = builder.add_child_to(
        viewport,
        Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 350.0)),
    );
    let source = builder.add_child_to(
        contents,
        Box::new(RectangleFigure::new(20.0, 30.0, 80.0, 40.0)),
    );
    let target = builder.add_child_to(
        contents,
        Box::new(RectangleFigure::new(300.0, 170.0, 100.0, 60.0)),
    );
    let connection_figure = builder.add_child_to(contents, Box::new(ConnectionFigure::new()));
    let mut runtime = Runtime::new(tree);
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .unwrap();

    let output = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(contents))
        .unwrap();
    assert_eq!(output.points().len(), 2);
}

#[test]
fn manhattan_scope_failure_clears_the_complete_batch_and_recovers_atomically() {
    let mut tree = novadraw_scene::FigureTree::new();
    let mut builder = tree.builder();
    let root = builder.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 700.0, 420.0)));
    let valid_source =
        builder.add_child_to(root, Box::new(RectangleFigure::new(20.0, 40.0, 80.0, 40.0)));
    let valid_target = builder.add_child_to(
        root,
        Box::new(RectangleFigure::new(420.0, 90.0, 100.0, 60.0)),
    );
    let first_figure = builder.add_child_to(root, Box::new(ConnectionFigure::new()));
    let viewport = builder.add_child_to(
        root,
        Box::new(ViewportFigure::new(20.0, 200.0, 260.0, 180.0)),
    );
    let contents = builder.add_child_to(
        viewport,
        Box::new(RectangleFigure::new(0.0, 0.0, 420.0, 300.0)),
    );
    let invalid_source = builder.add_child_to(
        contents,
        Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 40.0)),
    );
    let invalid_target = builder.add_child_to(
        root,
        Box::new(RectangleFigure::new(440.0, 260.0, 100.0, 60.0)),
    );
    let second_figure = builder.add_child_to(root, Box::new(ConnectionFigure::new()));
    let mut runtime = Runtime::new(tree);
    let router = runtime.register_connection_router(Box::new(ManhattanConnectionRouter));
    let valid_source_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(valid_source)));
    let valid_target_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(valid_target)));
    let invalid_source_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(invalid_source)));
    let invalid_target_anchor =
        runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(invalid_target)));
    let first = runtime
        .register_connection_state(
            first_figure,
            Some(valid_source_anchor),
            Some(valid_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    let second = runtime
        .register_connection_state(
            second_figure,
            Some(invalid_source_anchor),
            Some(invalid_target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    assert_eq!(
        runtime.resolve_connection_route(first, CoordinateSpace::ChildContent(root)),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::UnsupportedViewportTopology),
        ))
    );
    for (connection, figure) in [(first, first_figure), (second, second_figure)] {
        assert_eq!(
            runtime.connection_state(connection).unwrap().resolution,
            ConnectionResolution::Unresolved(UnresolvedConnection::RouteFailed(
                RouteError::UnsupportedViewportTopology,
            ))
        );
        assert_eq!(
            runtime.tree().figure_bounds(figure),
            Some(novadraw_geometry::Rectangle::ZERO)
        );
    }

    assert!(runtime.reparent(invalid_source, root));
    runtime
        .resolve_connection_route(first, CoordinateSpace::ChildContent(root))
        .unwrap();
    for (connection, figure) in [(first, first_figure), (second, second_figure)] {
        assert!(matches!(
            runtime.connection_state(connection).unwrap().resolution,
            ConnectionResolution::Resolved { .. }
        ));
        assert_ne!(
            runtime.tree().figure_bounds(figure),
            Some(novadraw_geometry::Rectangle::ZERO)
        );
    }
}
