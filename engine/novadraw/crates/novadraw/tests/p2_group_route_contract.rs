use novadraw::{
    connection::*,
    figure::RectangleFigure,
    geometry::{Point, PointList},
    runtime::Runtime,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct ExternalGroupSolver {
    mode: Arc<AtomicUsize>,
}

struct RejectingLocator(Arc<AtomicUsize>);
impl ConnectionLocatorStrategy for RejectingLocator {
    fn locate(&self, points: &PointList) -> Result<LocatorPlacement, LocatorError> {
        if self.0.load(Ordering::Relaxed) != 0 {
            return Err(LocatorError::DegenerateRoute);
        }
        ConnectionLocator::Middle.locate(points)
    }
}
impl ConnectionRouter for ExternalGroupSolver {
    fn route(&self, _: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        panic!("full group solver must not fall through to sequential routing")
    }
    fn prepares_group_endpoints(&self) -> bool {
        true
    }
    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
    fn solve_group(
        &self,
        input: RouteGroupInput<'_>,
    ) -> Result<Option<RouteGroupOutput>, RouteError> {
        let mut routes = input
            .members()
            .iter()
            .map(|member| {
                let metadata = *member.metadata();
                let points = PointList::from_points(vec![
                    metadata.source.site.point,
                    metadata.target.site.point,
                ]);
                (
                    member.connection(),
                    RouteOutput::new(points, metadata).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        match self.mode.load(Ordering::Relaxed) {
            1 => {
                routes.pop();
            }
            2 => {
                routes[1] = routes[0].clone();
            }
            3 => {
                routes.reverse();
            }
            4 => {
                return Err(RouteError::NoObstacleFreePath);
            }
            5 => {
                let mut foreign = Runtime::empty();
                let figure = foreign
                    .set_contents(Box::new(ConnectionFigure::new()))
                    .unwrap();
                routes[0].0 = ConnectionId::from_figure(figure);
            }
            6 => {
                let mut metadata = *routes[0].1.metadata();
                metadata.source.reference = Point::new(-100.0, 0.0);
                routes[0].1 = RouteOutput::new(routes[0].1.points().clone(), metadata).unwrap();
            }
            7 => {
                return RouteOutput::new(
                    PointList::from_points(vec![Point::new(f64::NAN, 0.0), Point::new(0.0, 0.0)]),
                    *routes[0].1.metadata(),
                )
                .map(|_| None);
            }
            _ => {}
        }
        Ok(Some(RouteGroupOutput::new(routes)))
    }
}

#[test]
fn external_group_outputs_are_atomic_and_recover_after_invalid_identity_sets() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 500.0, 400.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let mode = Arc::new(AtomicUsize::new(0));
    let router = runtime
        .connections()
        .register_router(Box::new(ExternalGroupSolver { mode: mode.clone() }));
    let mut connections = Vec::new();
    for y in [30.0, 100.0] {
        let figure = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let a = runtime
            .connections()
            .register_anchor(Box::new(XYAnchor::new(
                Point::new(20.0, y),
                CoordinateSpace::ChildContent(root),
            )));
        let b = runtime
            .connections()
            .register_anchor(Box::new(XYAnchor::new(
                Point::new(300.0, y),
                CoordinateSpace::ChildContent(root),
            )));
        connections.push(
            runtime
                .connections()
                .register_state(
                    figure,
                    Some(a),
                    Some(b),
                    RouterBinding::Explicit { router },
                    None,
                )
                .unwrap(),
        );
    }
    runtime
        .connections()
        .resolve_route(connections[0], CoordinateSpace::ChildContent(root))
        .unwrap();
    for bad in [1, 2, 3, 4, 5, 6, 7] {
        mode.store(bad, Ordering::Relaxed);
        assert!(
            runtime
                .connections()
                .resolve_route(connections[0], CoordinateSpace::ChildContent(root))
                .is_err()
        );
        for id in &connections {
            assert!(matches!(
                runtime.connections().state(*id).unwrap().resolution,
                ConnectionResolution::Unresolved(_)
            ));
            assert!(
                runtime
                    .tree()
                    .connection_route_points(id.figure())
                    .unwrap()
                    .is_none_or(|points| points.is_empty())
            );
        }
        mode.store(0, Ordering::Relaxed);
        runtime
            .connections()
            .resolve_route(connections[0], CoordinateSpace::ChildContent(root))
            .unwrap();
        for id in &connections {
            assert!(matches!(
                runtime.connections().state(*id).unwrap().resolution,
                ConnectionResolution::Resolved { .. }
            ));
        }
    }
    let child = runtime
        .figure(connections[1].figure())
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let reject = Arc::new(AtomicUsize::new(1));
    runtime
        .connections()
        .set_locator(
            connections[1],
            child,
            Box::new(RejectingLocator(reject.clone())),
        )
        .unwrap();
    assert!(
        runtime
            .connections()
            .resolve_route(connections[0], CoordinateSpace::ChildContent(root))
            .is_err()
    );
    for id in &connections {
        assert!(matches!(
            runtime.connections().state(*id).unwrap().resolution,
            ConnectionResolution::Unresolved(_)
        ));
        assert!(
            runtime
                .tree()
                .connection_route_points(id.figure())
                .unwrap()
                .is_none_or(|points| points.is_empty())
        );
    }
    reject.store(0, Ordering::Relaxed);
    runtime
        .connections()
        .resolve_route(connections[0], CoordinateSpace::ChildContent(root))
        .unwrap();
    for id in &connections {
        assert!(matches!(
            runtime.connections().state(*id).unwrap().resolution,
            ConnectionResolution::Resolved { .. }
        ));
    }
}

#[test]
fn sibling_admission_is_atomic_and_contents_remains_single_root() {
    use novadraw::tree::FigureSubtree;
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 500.0, 400.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let old = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let mut description = FigureSubtree::new(Box::new(
        RectangleFigure::new(10.0, 10.0, 20.0, 20.0).expect("valid figure bounds"),
    ));
    let first = description.root();
    let second = description
        .add_sibling(
            first,
            Box::new(RectangleFigure::new(40.0, 10.0, 20.0, 20.0).expect("valid figure bounds")),
        )
        .unwrap();
    let receipt = runtime
        .figure(root)
        .unwrap()
        .replace_child_subtree(old, description)
        .unwrap();
    assert!(runtime.tree().figure_bounds(old).is_err());
    assert_eq!(
        runtime.tree().parent_id(receipt.root()).unwrap(),
        Some(root)
    );
    assert_eq!(
        runtime
            .tree()
            .parent_id(receipt.figure(second).unwrap())
            .unwrap(),
        Some(root)
    );
    assert_eq!(runtime.tree().child_order(root).unwrap().len(), 2);
    let mut contents = FigureSubtree::new(Box::new(
        RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
    ));
    contents
        .add_sibling(
            contents.root(),
            Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds")),
        )
        .unwrap();
    assert!(runtime.set_contents_subtree(contents).is_err());
    assert!(runtime.tree().figure_bounds(root).is_ok());
    assert_eq!(runtime.tree().child_order(root).unwrap().len(), 2);
}
