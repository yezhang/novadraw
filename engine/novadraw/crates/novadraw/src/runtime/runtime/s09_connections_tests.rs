use super::*;
use crate::connection::{ConnectionFigure, XYAnchor};
use crate::geometry::Point;

#[test]
fn s09_faulted_connection_registration_preserves_registry_and_committed_prefix() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            crate::figure::RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
        ))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let space = CoordinateSpace::ChildContent(root);
    let source = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(Point::new(0.0, 0.0), space)))
        .unwrap();
    let target = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(Point::new(50.0, 50.0), space)))
        .unwrap();
    let router = runtime
        .connections()
        .register_router(Box::new(DirectRouter))
        .unwrap();
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source),
            Some(target),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let before = runtime.connections.registration_facts_for_test();
    let state = runtime.connections().state(connection).unwrap();
    let bounds = runtime.tree().figure_bounds(figure).unwrap();
    // Admission itself has no user callback capable of faulting the Runtime.
    runtime.faulted = true;
    assert_eq!(
        runtime
            .connections()
            .register_anchor(Box::new(XYAnchor::new(Point::new(10.0, 10.0), space))),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(
        runtime
            .connections()
            .register_router(Box::new(DirectRouter)),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(before, runtime.connections.registration_facts_for_test());
    assert_eq!(state, runtime.connections().state(connection).unwrap());
    assert_eq!(bounds, runtime.tree().figure_bounds(figure).unwrap());
    assert_eq!(
        runtime.connections().direct_router(),
        runtime.connections.direct_router()
    );
    assert!(runtime.is_faulted());
}
