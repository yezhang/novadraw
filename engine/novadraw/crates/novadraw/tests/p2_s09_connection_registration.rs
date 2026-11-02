use novadraw::{
    connection::{
        AnchorError, AnchorId, AnchorSite, ConnectionAnchor, ConnectionFigure,
        ConnectionResolution, ConnectionRouter, ConnectionRuntimeError, CoordinateSpace,
        DirectRouter, RouteError, RouteOutput, RouteRequest, RouteSceneQuery, RouterBinding,
        RouterId, XYAnchor,
    },
    event::{ListenerDirective, NotificationRecord, ObservationListener},
    figure::RectangleFigure,
    geometry::Point,
    runtime::{Runtime, StableSceneQuery},
    tree::FigureId,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct ExternalAnchor(Rc<Cell<usize>>);
impl ConnectionAnchor for ExternalAnchor {
    fn owner(&self) -> Option<FigureId> {
        self.0.set(self.0.get() + 1);
        None
    }
    fn reference_point(
        &self,
        _: &mut dyn RouteSceneQuery,
        _: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        self.0.set(self.0.get() + 1);
        Ok(Point::new(0.0, 0.0))
    }
    fn location(
        &self,
        _: &mut dyn RouteSceneQuery,
        _: Point,
        _: CoordinateSpace,
        _: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        self.0.set(self.0.get() + 1);
        Ok(AnchorSite {
            point: Point::new(0.0, 0.0),
            outward_normal: None,
        })
    }
}

struct ExternalRouter(Rc<Cell<usize>>);
impl ConnectionRouter for ExternalRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        self.0.set(self.0.get() + 1);
        DirectRouter.route(request)
    }
}

// Explicit return types make this fixture a positive public-signature check.
fn anchor(
    runtime: &mut Runtime,
    value: Box<dyn ConnectionAnchor>,
) -> Result<AnchorId, ConnectionRuntimeError> {
    runtime.connections().register_anchor(value)
}
fn router(
    runtime: &mut Runtime,
    value: Box<dyn ConnectionRouter>,
) -> Result<RouterId, ConnectionRuntimeError> {
    runtime.connections().register_router(value)
}

#[test]
fn builtin_and_external_registration_share_admission_without_strategy_callbacks() {
    let mut runtime = Runtime::empty();
    let calls = Rc::new(Cell::new(0));
    let external = anchor(&mut runtime, Box::new(ExternalAnchor(calls.clone()))).unwrap();
    let custom = router(&mut runtime, Box::new(ExternalRouter(calls.clone()))).unwrap();
    assert_eq!(calls.get(), 0);
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    let builtin = anchor(
        &mut runtime,
        Box::new(XYAnchor::new(
            Point::new(0.0, 0.0),
            CoordinateSpace::ChildContent(root),
        )),
    )
    .unwrap();
    let direct = router(&mut runtime, Box::new(DirectRouter)).unwrap();
    assert_eq!(external.namespace(), builtin.namespace());
    assert_eq!(custom.namespace(), direct.namespace());
    assert_ne!(external, builtin);
    assert_ne!(custom, direct);
    let removed = runtime.connections().remove_anchor(external).unwrap();
    assert_eq!(removed.owner(), None);
    assert_eq!(calls.get(), 1);
    drop(runtime.connections().remove_router(custom).unwrap());
    assert!(
        matches!(runtime.connections().remove_anchor(external), Err(ConnectionRuntimeError::UnknownAnchor(id)) if id == external)
    );
    assert!(
        matches!(runtime.connections().remove_router(custom), Err(ConnectionRuntimeError::UnknownRouter(id)) if id == custom)
    );
    let replacement = anchor(&mut runtime, removed).unwrap();
    assert_ne!(replacement, external);
    assert_eq!(replacement.namespace(), external.namespace());
    assert_eq!(calls.get(), 1);
    assert!(!runtime.is_faulted());
}

#[test]
fn external_strategies_run_only_when_registered_connection_is_prepared() {
    let mut runtime = Runtime::empty();
    let calls = Rc::new(Cell::new(0));
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
        ))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let source = anchor(&mut runtime, Box::new(ExternalAnchor(calls.clone()))).unwrap();
    let target = anchor(
        &mut runtime,
        Box::new(XYAnchor::new(
            Point::new(50.0, 50.0),
            CoordinateSpace::ChildContent(root),
        )),
    )
    .unwrap();
    let custom = router(&mut runtime, Box::new(ExternalRouter(calls.clone()))).unwrap();
    assert_eq!(calls.get(), 0);
    let connection = runtime
        .connections()
        .register_state(
            figure,
            Some(source),
            Some(target),
            RouterBinding::Explicit { router: custom },
            None,
        )
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(calls.get() > 0);
    assert!(matches!(
        runtime.connections().state(connection).unwrap().resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert!(
        matches!(runtime.connections().remove_anchor(source), Err(ConnectionRuntimeError::AnchorInUse(id)) if id == source)
    );
    assert!(
        matches!(runtime.connections().remove_router(custom), Err(ConnectionRuntimeError::RouterInUse(id)) if id == custom)
    );
    runtime.connections().remove_state(connection).unwrap();
    drop(runtime.connections().remove_anchor(source).unwrap());
    drop(runtime.connections().remove_router(custom).unwrap());
    assert!(!runtime.is_faulted());
}

struct PanicObserver;
impl ObservationListener for PanicObserver {
    fn observed(&self, _: &NotificationRecord, _: StableSceneQuery<'_>) -> ListenerDirective {
        panic!("connection admission owner fault");
    }
}

#[test]
fn actual_callback_fault_rejects_new_objects_and_only_new_owner_recovers() {
    let mut runtime = Runtime::empty();
    let calls = Rc::new(Cell::new(0));
    let old_anchor = anchor(&mut runtime, Box::new(ExternalAnchor(calls.clone()))).unwrap();
    let old_router = router(&mut runtime, Box::new(ExternalRouter(calls.clone()))).unwrap();
    runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    let listener = runtime
        .observations()
        .add_observation_listener(Box::new(PanicObserver))
        .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| runtime.frames().stabilize_for_query())).is_err());
    assert!(runtime.is_faulted());
    assert_eq!(
        anchor(&mut runtime, Box::new(ExternalAnchor(calls.clone()))),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(
        router(&mut runtime, Box::new(ExternalRouter(calls.clone()))),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(calls.get(), 0);
    assert!(runtime.observations().remove_listener(listener).unwrap());
    assert!(matches!(
        runtime.connections().remove_anchor(old_anchor),
        Err(ConnectionRuntimeError::Faulted)
    ));
    assert!(matches!(
        runtime.connections().remove_router(old_router),
        Err(ConnectionRuntimeError::Faulted)
    ));
    let mut recreated = Runtime::empty();
    let new_anchor = anchor(&mut recreated, Box::new(ExternalAnchor(calls.clone()))).unwrap();
    let new_router = router(&mut recreated, Box::new(ExternalRouter(calls.clone()))).unwrap();
    assert_ne!(new_anchor.namespace(), old_anchor.namespace());
    assert_ne!(new_router.namespace(), old_router.namespace());
    assert!(
        matches!(recreated.connections().remove_anchor(old_anchor), Err(ConnectionRuntimeError::UnknownAnchor(id)) if id == old_anchor)
    );
    assert!(
        matches!(recreated.connections().remove_router(old_router), Err(ConnectionRuntimeError::UnknownRouter(id)) if id == old_router)
    );
    drop(recreated.connections().remove_anchor(new_anchor).unwrap());
    drop(recreated.connections().remove_router(new_router).unwrap());
    assert_eq!(calls.get(), 0);
    assert!(!recreated.is_faulted());
    assert!(runtime.is_faulted());
}
