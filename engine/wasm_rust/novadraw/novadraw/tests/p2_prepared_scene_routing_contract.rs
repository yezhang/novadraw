//! External producer/router contract for Runtime-owned immutable preparation snapshots.
use novadraw::{
    connection::*,
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureDrawing, FigureMeasurement,
        FigurePreparation, FigurePresentation, PREPARATION, PreparationCapability,
        PreparationScene, PreparedScene, PreparedSceneSnapshot, RectangleFigure,
    },
    geometry::{Dimension, Point, Rectangle},
    graphics::{GraphicsError, PaintContext},
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparationError, Runtime},
    text::{MeasureContext, TextError},
    tree::FigureId,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Default)]
struct Inputs {
    revision: AtomicU64,
    reject: AtomicBool,
    required: Mutex<Vec<ConnectionId>>,
    geometry_read: Mutex<Option<FigureId>>,
    snapshot_read: Mutex<Option<FigureId>>,
    measurement_read: Mutex<Option<FigureId>>,
}
struct Page(Arc<Inputs>);
struct Empty;
impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}
impl Figure for Page {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 120.0, 80.0)
    }
    fn name(&self) -> &'static str {
        "ExternalProjection"
    }
    fn register_capabilities(
        &self,
        output: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        output.register(CONTAINER, ContainerCapability::of::<Self>())?;
        output.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigureContainer for Page {}
impl FigurePreparation for Page {
    fn revision(&self) -> u64 {
        self.0.revision.load(Ordering::Relaxed)
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        FigurePresentation::new(
            FigureMeasurement::new(120.0, 80.0, None),
            Dimension::ZERO,
            Rectangle::new(0.0, 0.0, 120.0, 80.0),
            Arc::new(Empty),
        )
        .map(|presentation| presentation.with_data(Arc::new(self.revision())))
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        if self.0.reject.load(Ordering::Relaxed) {
            return Err(TextError::InvalidPreparedScene("rejected candidate".into()));
        }
        if let Some(id) = *self.0.geometry_read.lock().unwrap() {
            scene.client_area(id)?;
        }
        if let Some(id) = *self.0.snapshot_read.lock().unwrap() {
            scene.prepared_scene(id)?;
        }
        if let Some(id) = *self.0.measurement_read.lock().unwrap() {
            scene.measure_figure(context, id, novadraw::figure::MeasureConstraints::UNBOUNDED)?;
        }
        let mut result =
            PreparedScene::new(self.prepare(context, bounds)?).with_result_version(self.revision());
        let required = self.0.required.lock().unwrap().clone();
        if !required.is_empty() {
            result.require_route_group(CoordinateSpace::ChildContent(scene.root()), required);
        }
        Ok(result)
    }
}
struct SnapshotRouter {
    owner: FigureId,
    seen: Arc<Mutex<Vec<PreparedSceneSnapshot>>>,
    fail: Arc<AtomicBool>,
}
impl ConnectionRouter for SnapshotRouter {
    fn prepares_group_endpoints(&self) -> bool {
        true
    }
    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
    fn solve_group(
        &self,
        mut input: RouteGroupInput<'_>,
    ) -> Result<Option<RouteGroupOutput>, RouteError> {
        let snapshot = input
            .scene()
            .prepared_scene(self.owner)
            .map_err(RouteError::Scene)?;
        self.seen.lock().unwrap().push(snapshot);
        if self.fail.load(Ordering::Relaxed) {
            return Err(RouteError::NoObstacleFreePath);
        }
        let routes = input
            .members()
            .iter()
            .map(|member| {
                let metadata = *member.metadata();
                RouteOutput::new(
                    novadraw::geometry::PointList::from_points(vec![
                        metadata.source.site.point,
                        metadata.target.site.point,
                    ]),
                    metadata,
                )
                .map(|route| (member.connection(), route))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(RouteGroupOutput::new(routes)))
    }
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        let snapshot = request
            .scene
            .prepared_scene(self.owner)
            .map_err(RouteError::Scene)?;
        self.seen.lock().unwrap().push(snapshot);
        if self.fail.load(Ordering::Relaxed) {
            return Err(RouteError::NoObstacleFreePath);
        }
        DirectRouter.route(request)
    }
}
struct Fixture {
    runtime: Runtime,
    owner: FigureId,
    connection: ConnectionId,
    source: novadraw::connection::AnchorId,
    inputs: Arc<Inputs>,
    seen: Arc<Mutex<Vec<PreparedSceneSnapshot>>>,
    route_fail: Arc<AtomicBool>,
}
fn fixture(required: bool) -> Fixture {
    let inputs = Arc::new(Inputs::default());
    let mut runtime = Runtime::empty();
    let owner = runtime
        .set_contents(Box::new(Page(inputs.clone())))
        .unwrap();
    let figure = runtime
        .figure(owner)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let source = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(10.0, 10.0),
            CoordinateSpace::ChildContent(owner),
        )));
    let target = runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(90.0, 60.0),
            CoordinateSpace::ChildContent(owner),
        )));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let route_fail = Arc::new(AtomicBool::new(false));
    let router = runtime
        .connections()
        .register_router(Box::new(SnapshotRouter {
            owner,
            seen: seen.clone(),
            fail: route_fail.clone(),
        }));
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
    if required {
        inputs.required.lock().unwrap().push(connection);
    }
    Fixture {
        runtime,
        owner,
        connection,
        source,
        inputs,
        seen,
        route_fail,
    }
}
fn frame(runtime: &mut Runtime) -> Result<(), FramePreparationError> {
    runtime.frames().request_full_redraw();
    let prepared = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 120.0,
            logical_height: 80.0,
            pixel_width: 120,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    );
    match prepared {
        novadraw::runtime::FramePreparation::Ready(frame) => {
            let _ = runtime
                .frames()
                .complete_submission(
                    frame.session_id,
                    frame.frame_id,
                    Ok(RenderDisposition::Presented),
                )
                .unwrap();
            Ok(())
        }
        novadraw::runtime::FramePreparation::Error(error) => Err(error),
        other => panic!("unexpected frame state: {other:?}"),
    }
}

#[test]
fn required_members_must_cover_the_full_solver_group() {
    let mut f = fixture(true);
    let binding = f.runtime.connections().state(f.connection).unwrap().router;
    let figure = f
        .runtime
        .figure(f.owner)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let target = f
        .runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(90.0, 30.0),
            CoordinateSpace::ChildContent(f.owner),
        )));
    let extra = f
        .runtime
        .connections()
        .register_state(figure, Some(f.source), Some(target), binding, None)
        .unwrap();
    let result = frame(&mut f.runtime);
    assert!(matches!(
        result,
        Err(FramePreparationError::Text(TextError::PreparationRoute(error)))
            if error.kind == novadraw::figure::PreparationRouteErrorKind::IncompleteGroup
    ));
    f.inputs.required.lock().unwrap().push(extra);
    f.inputs.revision.store(1, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
}

#[test]
fn duplicate_required_members_are_rejected_before_routing() {
    let mut f = fixture(true);
    f.inputs.required.lock().unwrap().push(f.connection);
    assert!(matches!(
        frame(&mut f.runtime),
        Err(FramePreparationError::Text(TextError::PreparationRoute(error)))
            if error.kind == novadraw::figure::PreparationRouteErrorKind::DuplicateMember
    ));
    assert!(f.seen.lock().unwrap().is_empty());
}

#[test]
fn descendant_layout_feedback_preserves_domain_identity_and_does_not_reroute() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let old = f.seen.lock().unwrap().last().unwrap().identity();
    let count = f.runtime.connections().routing_stats().route_calculations;
    let child = f
        .runtime
        .figure(f.connection.figure())
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    frame(&mut f.runtime).unwrap();
    // Adding a source node changes the frozen topology, so it replaces the result.
    let current = f.seen.lock().unwrap().last().unwrap().identity();
    assert_ne!(old, current);
    assert!(f.runtime.connections().routing_stats().route_calculations > count);
    f.runtime
        .connections()
        .set_locator(f.connection, child, Box::new(ConnectionLocator::Middle))
        .unwrap();
    frame(&mut f.runtime).unwrap();
    let count = f.runtime.connections().routing_stats().route_calculations;
    let current = f.seen.lock().unwrap().last().unwrap().identity();
    frame(&mut f.runtime).unwrap();
    assert_eq!(f.seen.lock().unwrap().last().unwrap().identity(), current);
    assert_eq!(
        f.runtime.connections().routing_stats().route_calculations,
        count
    );
}

#[test]
fn changed_payload_with_unchanged_bounds_replaces_identity_and_routes() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let old = f.seen.lock().unwrap().last().unwrap().clone();
    f.inputs.revision.store(1, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
    let seen = f.seen.lock().unwrap();
    let new = seen.last().unwrap();
    assert_ne!(old.identity(), new.identity());
    assert_eq!(*old.presentation().data::<u64>().unwrap(), 0);
    assert_eq!(*new.presentation().data::<u64>().unwrap(), 1);
}

#[test]
fn required_failed_route_rejects_frame_but_ordinary_route_keeps_existing_failure_channel() {
    for required in [true, false] {
        let mut f = fixture(required);
        f.route_fail.store(true, Ordering::Relaxed);
        let result = frame(&mut f.runtime);
        if required {
            assert!(matches!(
                result,
                Err(FramePreparationError::Text(TextError::PreparationRoute(_)))
            ));
            assert!(
                f.runtime
                    .tree()
                    .prepared_presentation(f.owner)
                    .unwrap()
                    .is_none()
            );
        } else {
            result.unwrap();
            assert!(matches!(
                f.runtime
                    .connections()
                    .state(f.connection)
                    .unwrap()
                    .resolution,
                ConnectionResolution::Unresolved(_)
            ));
        }
    }
}

#[test]
fn rejected_same_source_candidate_keeps_the_old_identity() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let old = f.seen.lock().unwrap().last().unwrap().identity();
    // Force a geometry feedback attempt without changing producer inputs.
    let _child = f
        .runtime
        .figure(f.connection.figure())
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0)))
        .unwrap();
    frame(&mut f.runtime).unwrap();
    let stable = f.seen.lock().unwrap().last().unwrap().identity();
    assert_ne!(old, stable);
    f.inputs.reject.store(true, Ordering::Relaxed);
    f.runtime.figure(f.owner).unwrap().revalidate().unwrap();
    assert!(frame(&mut f.runtime).is_err());
    f.inputs.reject.store(false, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
    assert_eq!(f.seen.lock().unwrap().last().unwrap().identity(), stable);
}

#[test]
fn same_source_route_failure_restores_successful_metadata_and_generation() {
    let mut f = fixture(true);
    let measured = f
        .runtime
        .figure(f.owner)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 20.0)))
        .unwrap();
    *f.inputs.measurement_read.lock().unwrap() = Some(measured);
    frame(&mut f.runtime).unwrap();
    let stable = f.runtime.connections().state(f.connection).unwrap();
    let identity = f.seen.lock().unwrap().last().unwrap().identity();
    let points = f
        .runtime
        .tree()
        .connection_route_points(f.connection.figure())
        .unwrap()
        .unwrap()
        .clone();
    f.route_fail.store(true, Ordering::Relaxed);
    f.runtime
        .connections()
        .invalidate_dependency(&DependencySubject::PreparedScene(f.owner))
        .unwrap();
    assert!(frame(&mut f.runtime).is_err());
    assert_eq!(f.runtime.connections().state(f.connection).unwrap(), stable);
    assert_eq!(
        f.runtime
            .tree()
            .connection_route_points(f.connection.figure())
            .unwrap()
            .unwrap(),
        &points
    );
    assert_eq!(f.seen.lock().unwrap().last().unwrap().identity(), identity);
    assert!(f.runtime.connections().last_error().is_some());
    f.route_fail.store(false, Ordering::Relaxed);
    let calculations = f.runtime.connections().routing_stats().route_calculations;
    frame(&mut f.runtime).unwrap();
    assert_eq!(f.seen.lock().unwrap().last().unwrap().identity(), identity);
    assert_eq!(f.runtime.connections().state(f.connection).unwrap(), stable);
    assert_eq!(
        f.runtime.connections().routing_stats().route_calculations,
        calculations
    );
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connection)
            .unwrap()
            .resolution,
        ConnectionResolution::Resolved { .. }
    ));
    let stable = f.runtime.connections().state(f.connection).unwrap();
    frame(&mut f.runtime).unwrap();
    assert_eq!(f.runtime.connections().state(f.connection).unwrap(), stable);
}

#[test]
fn retired_connection_is_not_restored_by_rejected_preparation() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    f.runtime.connections().remove_state(f.connection).unwrap();
    assert!(frame(&mut f.runtime).is_err());
    assert!(matches!(
        f.runtime.connections().state(f.connection),
        Err(ConnectionRuntimeError::UnknownConnection(id)) if id == f.connection
    ));
    f.inputs.required.lock().unwrap().clear();
    f.inputs.revision.store(1, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
    assert!(matches!(
        f.runtime.connections().state(f.connection),
        Err(ConnectionRuntimeError::UnknownConnection(id)) if id == f.connection
    ));
}

#[test]
fn changed_source_route_failure_cannot_restore_resolved_metadata() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let identity = f.seen.lock().unwrap().last().unwrap().identity();
    f.inputs.revision.store(1, Ordering::Relaxed);
    f.route_fail.store(true, Ordering::Relaxed);
    assert!(frame(&mut f.runtime).is_err());
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connection)
            .unwrap()
            .resolution,
        ConnectionResolution::Unresolved(_)
    ));
    assert!(
        f.runtime
            .connections()
            .resolve_route(f.connection, CoordinateSpace::ChildContent(f.owner))
            .is_err()
    );
    f.route_fail.store(false, Ordering::Relaxed);
    f.inputs.revision.store(2, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
    assert_ne!(f.seen.lock().unwrap().last().unwrap().identity(), identity);
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connection)
            .unwrap()
            .resolution,
        ConnectionResolution::Resolved { .. }
    ));
}

#[test]
fn changed_binding_route_failure_preserves_binding_and_rejects_old_resolution() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let source = f
        .runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(15.0, 20.0),
            CoordinateSpace::ChildContent(f.owner),
        )));
    f.runtime
        .connections()
        .set_source(f.connection, Some(source))
        .unwrap();
    f.route_fail.store(true, Ordering::Relaxed);
    assert!(frame(&mut f.runtime).is_err());
    let state = f.runtime.connections().state(f.connection).unwrap();
    assert_eq!(state.source, Some(source));
    assert!(matches!(
        state.resolution,
        ConnectionResolution::Unresolved(_)
    ));
    f.route_fail.store(false, Ordering::Relaxed);
    let attempts = f.seen.lock().unwrap().len();
    assert!(
        f.runtime
            .connections()
            .resolve_route(f.connection, CoordinateSpace::ChildContent(f.owner))
            .is_err()
    );
    // The failure must come from unavailable projection authority, before the router consumes it.
    assert_eq!(f.seen.lock().unwrap().len(), attempts);
    f.runtime
        .connections()
        .set_source(f.connection, Some(f.source))
        .unwrap();
    frame(&mut f.runtime).unwrap();
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connection)
            .unwrap()
            .resolution,
        ConnectionResolution::Resolved { .. }
    ));
}

#[test]
fn old_snapshot_cannot_be_routed_after_source_revision_drift() {
    let mut f = fixture(false);
    frame(&mut f.runtime).unwrap();
    f.inputs.revision.store(1, Ordering::Relaxed);
    // Explicit resolution bypasses preparation and must reject the stale payload.
    f.runtime
        .connections()
        .set_source(f.connection, Some(f.source))
        .unwrap();
    assert!(
        f.runtime
            .connections()
            .resolve_route(f.connection, CoordinateSpace::ChildContent(f.owner))
            .is_err()
    );
    frame(&mut f.runtime).unwrap();
    assert_eq!(
        *f.seen
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .presentation()
            .data::<u64>()
            .unwrap(),
        1
    );
}

#[test]
fn source_bounds_commit_is_observed_even_when_equal_to_current_derived_bounds() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let old = f.seen.lock().unwrap().last().unwrap().identity();
    let bounds = f
        .runtime
        .tree()
        .figure_bounds(f.connection.figure())
        .unwrap();
    f.runtime
        .figure(f.connection.figure())
        .unwrap()
        .set_bounds(bounds)
        .unwrap();
    frame(&mut f.runtime).unwrap();
    assert_ne!(f.seen.lock().unwrap().last().unwrap().identity(), old);
}

#[test]
fn required_route_output_read_is_rejected_before_solver_and_recovers_after_correction() {
    let mut f = fixture(true);
    *f.inputs.geometry_read.lock().unwrap() = Some(f.connection.figure());
    assert!(matches!(
        frame(&mut f.runtime),
        Err(FramePreparationError::Text(TextError::PreparationDependency(error)))
            if matches!(*error, novadraw::figure::PreparationDependencyError::ForbiddenRouteOutputDependency {
                owner, connection, ..
            } if owner == f.owner && connection == f.connection)
    ));
    assert!(f.seen.lock().unwrap().is_empty());
    *f.inputs.geometry_read.lock().unwrap() = None;
    f.inputs.revision.store(1, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
}

#[test]
fn route_cycle_expands_prepared_snapshot_computation_reads() {
    let mut f = fixture(false);
    *f.inputs.geometry_read.lock().unwrap() = Some(f.connection.figure());
    // Without a required barrier the ordinary route failure channel still applies.
    frame(&mut f.runtime).unwrap();
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connection)
            .unwrap()
            .resolution,
        ConnectionResolution::Unresolved(UnresolvedConnection::RouteFailed(
            RouteError::DependencyCycle
        ))
    ));
}

#[test]
fn preparation_self_snapshot_read_returns_ordered_structural_cycle() {
    let mut f = fixture(true);
    *f.inputs.snapshot_read.lock().unwrap() = Some(f.owner);
    assert!(matches!(
        frame(&mut f.runtime),
        Err(FramePreparationError::Text(TextError::PreparationDependency(error)))
            if matches!(&*error, novadraw::figure::PreparationDependencyError::DependencyCycle { path }
                if *path == vec![DependencySubject::PreparedScene(f.owner), DependencySubject::PreparedScene(f.owner)])
    ));
    assert!(f.seen.lock().unwrap().is_empty());
}
