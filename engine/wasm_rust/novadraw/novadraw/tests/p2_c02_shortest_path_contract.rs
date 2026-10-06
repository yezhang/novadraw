use novadraw::connection::{ConnectionRoutingStats, UnresolvedConnection};
use novadraw::geometry::{Point, Rectangle};
use novadraw::{
    ChopboxAnchor, ConnectionFigure, ConnectionResolution, CoordinateSpace, RectangleFigure,
    RouteError, RouterBinding, Runtime, ShortestPathConnectionRouter,
};

const CLEARANCE: f64 = 10.0;

fn parent_route(runtime: &Runtime, connection_figure: novadraw::FigureId) -> Vec<Point> {
    let bounds = runtime.tree().figure_bounds(connection_figure).unwrap();
    runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .iter()
        .map(|point| Point::new(point.x() + bounds.x, point.y() + bounds.y))
        .collect()
}

fn segment_crosses_interior(start: Point, end: Point, obstacle: Rectangle) -> bool {
    if (start.y() - end.y()).abs() <= f64::EPSILON {
        let segment_min = start.x().min(end.x());
        let segment_max = start.x().max(end.x());
        start.y() > obstacle.y
            && start.y() < obstacle.y + obstacle.height
            && segment_max > obstacle.x
            && segment_min < obstacle.x + obstacle.width
    } else {
        let segment_min = start.y().min(end.y());
        let segment_max = start.y().max(end.y());
        start.x() > obstacle.x
            && start.x() < obstacle.x + obstacle.width
            && segment_max > obstacle.y
            && segment_min < obstacle.y + obstacle.height
    }
}

#[test]
fn shortest_path_routes_around_obstacles_and_reacts_to_geometry_and_visibility() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 640.0, 360.0)))
        .unwrap();
    let source = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 150.0, 60.0, 40.0)))
        .unwrap();
    let target = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(540.0, 150.0, 60.0, 40.0)))
        .unwrap();
    let obstacle = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(260.0, 110.0, 100.0, 120.0)))
        .unwrap();
    let connection_figure = runtime
        .container(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let router = runtime.register_connection_router(Box::new(
        ShortestPathConnectionRouter::new([obstacle])
            .with_clearance(CLEARANCE)
            .unwrap(),
    ));
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();

    let first = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert!(first.points().len() >= 4);
    let inflated = Rectangle::new(250.0, 100.0, 120.0, 140.0);
    assert!(
        first
            .points()
            .as_slice()
            .windows(2)
            .all(|segment| !segment_crosses_interior(segment[0], segment[1], inflated))
    );
    assert!(first.points().as_slice().windows(2).all(|segment| {
        (segment[0].x() - segment[1].x()).abs() <= f64::EPSILON
            || (segment[0].y() - segment[1].y()).abs() <= f64::EPSILON
    }));
    assert_eq!(
        runtime.connection_routing_stats(),
        ConnectionRoutingStats {
            route_calculations: 1,
            routing_order_entries: 4,
            obstacle_snapshot_builds: 1,
        }
    );
    let repeated = runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(repeated, first);

    runtime
        .figure(obstacle)
        .unwrap()
        .set_bounds(Rectangle::new(300.0, 80.0, 100.0, 180.0))
        .unwrap();
    assert_eq!(runtime.dirty_connections(), vec![connection]);
    runtime.prepare_frame().expect("obstacle movement reroutes");
    let moved = parent_route(&runtime, connection_figure);
    assert_ne!(moved, first.points().as_slice());

    runtime
        .figure(obstacle)
        .unwrap()
        .set_visible(false)
        .unwrap();
    runtime.prepare_frame().expect("visibility change reroutes");
    assert_eq!(parent_route(&runtime, connection_figure).len(), 2);
    runtime.figure(obstacle).unwrap().set_visible(true).unwrap();
    runtime.prepare_frame().expect("restored obstacle reroutes");
    assert!(parent_route(&runtime, connection_figure).len() >= 4);
    runtime
        .container(root)
        .unwrap()
        .remove(obstacle)
        .expect("obstacle removal is a valid topology mutation");
    runtime.prepare_frame().expect("obstacle removal reroutes");
    assert_eq!(parent_route(&runtime, connection_figure).len(), 2);
    assert_eq!(
        runtime.connection_routing_stats(),
        ConnectionRoutingStats {
            route_calculations: 6,
            routing_order_entries: 23,
            obstacle_snapshot_builds: 6,
        }
    );
}

#[test]
fn shortest_path_batch_failure_clears_every_member_and_recovers_atomically() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 500.0, 300.0)))
        .unwrap();
    let source = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 120.0, 60.0, 40.0)))
        .unwrap();
    let target = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(420.0, 120.0, 60.0, 40.0)))
        .unwrap();
    let blocker = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(70.0, 100.0, 40.0, 80.0)))
        .unwrap();
    let router = runtime.register_connection_router(Box::new(
        ShortestPathConnectionRouter::new([blocker])
            .with_clearance(CLEARANCE)
            .unwrap(),
    ));
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let mut connections = Vec::new();
    for _ in 0..2 {
        let figure = runtime
            .container(root)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let connection = runtime
            .register_connection_state(
                figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        connections.push((connection, figure));
    }

    assert!(matches!(
        runtime.resolve_connection_route(connections[0].0, CoordinateSpace::ChildContent(root)),
        Err(novadraw::ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::NoObstacleFreePath)
        ))
    ));
    for (connection, figure) in &connections {
        assert_eq!(
            runtime.connection_state(*connection).unwrap().resolution,
            ConnectionResolution::Unresolved(UnresolvedConnection::RouteFailed(
                RouteError::NoObstacleFreePath
            ))
        );
        assert!(
            runtime
                .tree()
                .connection_route_points(*figure)
                .unwrap()
                .is_empty()
        );
    }

    runtime.figure(blocker).unwrap().set_visible(false).unwrap();
    runtime
        .prepare_frame()
        .expect("hidden blocker recovers batch");
    assert!(connections.iter().all(|(connection, figure)| {
        matches!(
            runtime.connection_state(*connection).unwrap().resolution,
            ConnectionResolution::Resolved { .. }
        ) && !runtime
            .tree()
            .connection_route_points(*figure)
            .unwrap()
            .is_empty()
    }));
}

#[test]
fn one_obstacle_snapshot_is_reused_for_a_sixty_four_connection_batch() {
    const COUNT: usize = 64;

    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1200.0, 500.0)))
        .unwrap();
    let source = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 20.0, 40.0, 40.0)))
        .unwrap();
    let target = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(1100.0, 20.0, 40.0, 40.0)))
        .unwrap();
    let mut obstacles = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        obstacles.push(
            runtime
                .container(root)
                .unwrap()
                .add(Box::new(RectangleFigure::new(
                    100.0 + index as f64 * 12.0,
                    300.0,
                    8.0,
                    20.0,
                )))
                .unwrap(),
        );
    }
    let router = runtime.register_connection_router(Box::new(
        ShortestPathConnectionRouter::new(obstacles)
            .with_clearance(2.0)
            .unwrap(),
    ));
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let mut connections = Vec::with_capacity(COUNT);
    for _ in 0..COUNT {
        let figure = runtime
            .container(root)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let connection = runtime
            .register_connection_state(
                figure,
                Some(source_anchor),
                Some(target_anchor),
                RouterBinding::Explicit { router },
                None,
            )
            .unwrap();
        connections.push(connection);
    }

    runtime
        .resolve_connection_route(connections[0], CoordinateSpace::ChildContent(root))
        .unwrap();
    assert_eq!(
        runtime.connection_routing_stats(),
        ConnectionRoutingStats {
            route_calculations: COUNT as u64,
            routing_order_entries: 2 * COUNT as u64 + 2,
            obstacle_snapshot_builds: 1,
        }
    );
    assert!(connections.iter().all(|connection| matches!(
        runtime.connection_state(*connection).unwrap().resolution,
        ConnectionResolution::Resolved { generation: 1 }
    )));
}
