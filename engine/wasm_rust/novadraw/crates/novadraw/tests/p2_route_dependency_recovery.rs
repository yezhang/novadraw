use novadraw::connection::{
    AnchorError, AnchorSite, ChopboxAnchor, ConnectionAnchor, ConnectionFigure,
    ConnectionResolution, CoordinateSpace, RouteError, RouteSceneQuery, RouteSceneQueryError,
    RouterBinding, UnresolvedConnection,
};
use novadraw::figure::RectangleFigure;
use novadraw::geometry::{Point, Rectangle};
use novadraw::runtime::Runtime;
use novadraw::tree::{FigureId, TreeQueryError};

mod support;

// This strategy has no primary lifecycle owner. Its geometry dependency is tracked
// through the same public query used by built-in anchors.
struct DependentAnchor(FigureId);

impl ConnectionAnchor for DependentAnchor {
    fn owner(&self) -> Option<FigureId> {
        None
    }
    fn reference_point(
        &self,
        query: &mut dyn RouteSceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        ChopboxAnchor::new(self.0).reference_point(query, output)
    }
    fn location(
        &self,
        query: &mut dyn RouteSceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        ChopboxAnchor::new(self.0).location(query, reference, reference_space, output)
    }
}

#[test]
fn missing_tracked_dependency_stabilizes_and_new_binding_recovers() {
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
        .add(Box::new(RectangleFigure::new(300.0, 180.0, 80.0, 40.0)))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(DependentAnchor(source)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().direct_router();
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    support::prepare_next_submission(&mut runtime).unwrap();
    let initial = runtime.connections().state(connection).unwrap();
    assert!(
        matches!(initial.resolution, ConnectionResolution::Resolved { .. }),
        "{initial:?}"
    );
    assert!(
        runtime
            .connections()
            .state(connection)
            .unwrap()
            .dependency_count
            > 0
    );
    let resolved_epoch = runtime.observations().stable_query().unwrap().epoch();

    runtime.dispose_subtree(source).unwrap();
    let stabilization = runtime.frames().stabilize_for_query();
    assert!(
        stabilization.is_ok(),
        "{stabilization:?}; {:?}",
        runtime.connections().state(connection)
    );
    support::prepare_next_submission(&mut runtime)
        .expect("missing dependency remains a recoverable domain failure");
    assert!(!runtime.is_faulted());
    assert_eq!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::RouteFailed(RouteError::Source(
            AnchorError::Scene(RouteSceneQueryError::UnknownFigure(source))
        )))
    );
    assert!(
        runtime
            .connections()
            .state(connection)
            .unwrap()
            .dependency_count
            > 0
    );
    assert!(runtime.observations().stable_query().unwrap().epoch() > resolved_epoch);
    assert!(support::prepare_next_submission(&mut runtime).is_none());

    let replacement = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(40.0, 50.0, 80.0, 40.0)))
        .unwrap();
    assert_ne!(replacement, source);
    let new_anchor = runtime
        .connections()
        .register_anchor(Box::new(DependentAnchor(replacement)));
    runtime
        .connections()
        .set_source(connection, Some(new_anchor))
        .unwrap();
    support::prepare_next_submission(&mut runtime).unwrap();
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(
        !runtime
            .tree()
            .connection_route_points(figure)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        runtime.tree().figure_bounds(source),
        Err(TreeQueryError::UnknownFigure(source))
    );
    assert_eq!(
        runtime.tree().figure_bounds(replacement),
        Ok(Rectangle::new(40.0, 50.0, 80.0, 40.0))
    );
}
