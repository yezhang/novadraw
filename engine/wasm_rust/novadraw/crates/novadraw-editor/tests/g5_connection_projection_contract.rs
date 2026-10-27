mod support;

use support::prepare_next_submission;

use std::{
    cell::Cell,
    collections::HashMap,
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use novadraw::connection::{
    AnchorSemanticKey, ChopboxAnchor, ConnectionFigure, ConnectionId, ConnectionResolution,
    ConnectionRouter, ConnectionRuntimeError, DirectRouter, RouteError, RouteOutput, RouteRequest,
};
use novadraw::figure::RootFigure;
use novadraw::geometry::{Point, PointList, Rectangle};
use novadraw::{figure::Figure, figure::RectangleFigure};
use novadraw_editor::{
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionPartFactoryContext,
    ConnectionRouterKey, ConnectionRouterRegistration, ConnectionRoutingDescriptor,
    EditPartBehavior, EditPartError, EditPartFactory, GraphicalViewer, ModelAdapter,
    ModelConnection, ModelEvent, ModelRevision, PartFactoryContext, PartKind, ViewerError,
    VisualUpdateContext,
};

const ROOT: ModelId = ModelId(1);
const FIRST: ModelId = ModelId(2);
const SECOND: ModelId = ModelId(3);
const THIRD: ModelId = ModelId(4);
const EDGE_A: ModelId = ModelId(10);
const EDGE_B: ModelId = ModelId(11);
const LINEAR_REFRESH_CONNECTION_COUNT: u64 = 64;

struct CountingRouter {
    route_count: Arc<AtomicUsize>,
}

struct AdmissionProbeRouter;

impl ConnectionRouter for AdmissionProbeRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        let invalid = request
            .constraint
            .and_then(|value| value.as_any().downcast_ref::<Arc<AtomicBool>>())
            .is_some_and(|value| value.load(Ordering::SeqCst));
        let output = DirectRouter.route(RouteRequest {
            constraint: None,
            ..request
        })?;
        if invalid {
            RouteOutput::new(
                PointList::from_points(vec![
                    output.metadata().source.site.point,
                    Point::new(-f64::MAX, 0.0),
                    Point::new(f64::MAX, 0.0),
                    output.metadata().target.site.point,
                ]),
                *output.metadata(),
            )
        } else {
            Ok(output)
        }
    }
    fn constraint_type(&self) -> Option<std::any::TypeId> {
        Some(std::any::TypeId::of::<Arc<AtomicBool>>())
    }
}

impl ConnectionRouter for CountingRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        self.route_count.fetch_add(1, Ordering::Relaxed);
        DirectRouter.route(request)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ModelId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramEvent {
    Changed,
}

struct DiagramModel {
    revision: Cell<ModelRevision>,
    children: Vec<ModelId>,
    bounds: HashMap<ModelId, Rectangle>,
    connections: Vec<ModelConnection<ModelId>>,
    events: Vec<ModelEvent<ModelId, DiagramEvent>>,
    drift_during_connections: Cell<bool>,
    anchor_versions: HashMap<ModelId, u64>,
    failure: Option<ConnectionFailure>,
    cleanup_callbacks: Arc<AtomicUsize>,
    invalid_route: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum ConnectionFailure {
    TargetProvider,
    BindingConstraint,
    ActivationPanic,
    CleanupPanic,
}

impl DiagramModel {
    fn new(connections: Vec<ModelConnection<ModelId>>) -> Self {
        Self {
            revision: Cell::new(ModelRevision::initial()),
            children: vec![FIRST, SECOND, THIRD],
            bounds: HashMap::from([
                (ROOT, Rectangle::new(0.0, 0.0, 600.0, 400.0)),
                (FIRST, Rectangle::new(40.0, 60.0, 100.0, 80.0)),
                (SECOND, Rectangle::new(240.0, 60.0, 100.0, 80.0)),
                (THIRD, Rectangle::new(440.0, 220.0, 100.0, 80.0)),
            ]),
            connections,
            events: Vec::new(),
            drift_during_connections: Cell::new(false),
            anchor_versions: HashMap::new(),
            failure: None,
            cleanup_callbacks: Arc::new(AtomicUsize::new(0)),
            invalid_route: Arc::new(AtomicBool::new(false)),
        }
    }

    fn publish(&mut self) {
        let revision = self.revision.get().next().unwrap();
        self.revision.set(revision);
        self.events
            .push(ModelEvent::new(revision, ROOT, DiagramEvent::Changed));
    }
}

impl ModelAdapter for DiagramModel {
    type ModelId = ModelId;
    type Event = DiagramEvent;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        ROOT
    }

    fn revision(&self) -> ModelRevision {
        self.revision.get()
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == ROOT {
            self.children.clone()
        } else {
            Vec::new()
        })
    }

    fn connections(&self) -> Result<Vec<ModelConnection<Self::ModelId>>, Self::Error> {
        if self.drift_during_connections.replace(false) {
            self.revision.set(self.revision.get().next().unwrap());
        }
        Ok(self.connections.clone())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Lifecycle {
    Activate(ModelId),
    Deactivate(ModelId),
}

struct DiagramPart {
    lifecycle: Arc<Mutex<Vec<Lifecycle>>>,
}

impl EditPartBehavior<DiagramModel> for DiagramPart {
    fn create_figure(
        &mut self,
        model: &DiagramModel,
        model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let bounds = model.bounds[&model_id];
        Ok(if model_id == ROOT {
            Box::new(RootFigure::new(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            ))
        } else {
            Box::new(RectangleFigure::from_bounds(bounds))
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &DiagramModel,
        model_id: ModelId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.bounds[&model_id])?;
        Ok(())
    }

    fn source_connection_anchor(
        &mut self,
        model: &DiagramModel,
        model_id: ModelId,
        context: ConnectionAnchorContext<ModelId>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        let Some(version) = model.anchor_versions.get(&model_id).copied() else {
            return Ok(None);
        };
        let key = AnchorSemanticKey::new(
            Some(context.endpoint_figure()),
            "test-source-port",
            vec![version],
        )
        .unwrap();
        Ok(Some(ConnectionAnchorDescriptor::new(
            key,
            Box::new(ChopboxAnchor::new(context.endpoint_figure())),
        )))
    }

    fn activate(&mut self, _model: &DiagramModel, model_id: ModelId) -> Result<(), EditPartError> {
        self.lifecycle
            .lock()
            .unwrap()
            .push(Lifecycle::Activate(model_id));
        Ok(())
    }

    fn target_connection_anchor(
        &mut self,
        model: &DiagramModel,
        _model_id: ModelId,
        context: ConnectionAnchorContext<ModelId>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        if matches!(model.failure, Some(ConnectionFailure::TargetProvider))
            && context.connection_model() == Some(EDGE_B)
        {
            return Err(EditPartError::operation("target provider refused"));
        }
        Ok(None)
    }

    fn deactivate(&mut self, _model: &DiagramModel, model_id: ModelId) {
        self.lifecycle
            .lock()
            .unwrap()
            .push(Lifecycle::Deactivate(model_id));
    }
}

struct ConnectionPart {
    lifecycle: Arc<Mutex<Vec<Lifecycle>>>,
    reject_activation: bool,
}

struct ConnectionCleanupProbe(Arc<AtomicUsize>);

impl Figure for ConnectionCleanupProbe {
    fn name(&self) -> &'static str {
        "connection cleanup probe"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 1.0, 1.0)
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::LIFECYCLE,
            novadraw::figure::LifecycleCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigureLifecycle for ConnectionCleanupProbe {
    fn on_detached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("connection cleanup lifecycle panic");
    }
}

impl EditPartBehavior<DiagramModel> for ConnectionPart {
    fn configure_visual(
        &mut self,
        model: &DiagramModel,
        model_id: ModelId,
        context: &mut novadraw_editor::VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        if model_id == EDGE_B && matches!(model.failure, Some(ConnectionFailure::CleanupPanic)) {
            context.add_child(
                context.primary(),
                Box::new(ConnectionCleanupProbe(Arc::clone(&model.cleanup_callbacks))),
            )?;
        }
        Ok(())
    }
    fn create_figure(
        &mut self,
        _model: &DiagramModel,
        _model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(ConnectionFigure::new()))
    }

    fn activate(&mut self, model: &DiagramModel, model_id: ModelId) -> Result<(), EditPartError> {
        if model_id == EDGE_B && matches!(model.failure, Some(ConnectionFailure::ActivationPanic)) {
            panic!("connection activation panic");
        }
        if self.reject_activation
            || (model_id == EDGE_B
                && matches!(model.failure, Some(ConnectionFailure::CleanupPanic)))
        {
            return Err(EditPartError::operation(
                "connection activation rejected model",
            ));
        }
        self.lifecycle
            .lock()
            .unwrap()
            .push(Lifecycle::Activate(model_id));
        Ok(())
    }

    fn connection_routing(
        &mut self,
        model: &DiagramModel,
        model_id: ModelId,
    ) -> Result<ConnectionRoutingDescriptor, EditPartError> {
        if !model.invalid_route.load(Ordering::SeqCst)
            && !(model_id == EDGE_B
                && matches!(model.failure, Some(ConnectionFailure::BindingConstraint)))
        {
            return Ok(ConnectionRoutingDescriptor::inherited());
        }
        Ok(ConnectionRoutingDescriptor::registered(
            ConnectionRouterKey::new("admission-probe")?,
            Some(
                if model_id == EDGE_B
                    && matches!(model.failure, Some(ConnectionFailure::BindingConstraint))
                {
                    Box::new(())
                } else {
                    Box::new(Arc::clone(&model.invalid_route))
                },
            ),
        ))
    }

    fn deactivate(&mut self, _model: &DiagramModel, model_id: ModelId) {
        self.lifecycle
            .lock()
            .unwrap()
            .push(Lifecycle::Deactivate(model_id));
    }
}

struct NonConnectionPart;

impl EditPartBehavior<DiagramModel> for NonConnectionPart {
    fn create_figure(
        &mut self,
        _model: &DiagramModel,
        _model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
    }
}

struct DiagramFactory {
    lifecycle: Arc<Mutex<Vec<Lifecycle>>>,
    invalid_connection_figure: bool,
    rejected_connection: Option<ModelId>,
    rejected_activation: Option<ModelId>,
}

impl EditPartFactory<DiagramModel> for DiagramFactory {
    fn connection_routers(&mut self) -> Result<Vec<ConnectionRouterRegistration>, EditPartError> {
        Ok(vec![ConnectionRouterRegistration::new(
            ConnectionRouterKey::new("admission-probe")?,
            Box::new(AdmissionProbeRouter),
        )])
    }
    fn create(
        &mut self,
        _context: PartFactoryContext<ModelId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        Ok(Box::new(DiagramPart {
            lifecycle: Arc::clone(&self.lifecycle),
        }))
    }

    fn create_connection(
        &mut self,
        context: ConnectionPartFactoryContext<ModelId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        if self.rejected_connection == Some(context.model_id()) {
            return Err(EditPartError::operation(
                "connection factory rejected model",
            ));
        }
        if self.invalid_connection_figure {
            return Ok(Box::new(NonConnectionPart));
        }
        Ok(Box::new(ConnectionPart {
            lifecycle: Arc::clone(&self.lifecycle),
            reject_activation: self.rejected_activation == Some(context.model_id()),
        }))
    }
}

fn edge(id: ModelId, source: ModelId, target: ModelId) -> ModelConnection<ModelId> {
    ModelConnection::new(id, source, target)
}

fn viewer_with(
    connections: Vec<ModelConnection<ModelId>>,
) -> (
    GraphicalViewer<DiagramModel, DiagramFactory>,
    Arc<Mutex<Vec<Lifecycle>>>,
) {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let viewer = GraphicalViewer::new(
        DiagramModel::new(connections),
        DiagramFactory {
            lifecycle: Arc::clone(&lifecycle),
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap();
    (viewer, lifecycle)
}

#[test]
fn ordered_snapshot_projects_one_connection_part_outside_containment() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let part = connection.edit_part();
    let endpoints = viewer.parts().connection_endpoints(connection).unwrap();

    assert_eq!(
        viewer.parts().get(part).unwrap().kind(),
        PartKind::Connection
    );
    assert_eq!(viewer.parts().parent(part), None);
    assert_eq!(viewer.parts().children(part), None);
    assert_eq!(
        viewer.parts().children(viewer.root()).unwrap(),
        &[viewer.contents()]
    );
    assert_eq!(endpoints.source(), viewer.part_for_model(FIRST).unwrap());
    assert_eq!(endpoints.target(), viewer.part_for_model(SECOND).unwrap());
    assert_eq!(
        viewer
            .parts()
            .source_connections(endpoints.source())
            .unwrap(),
        &[connection]
    );
    assert_eq!(
        viewer
            .parts()
            .target_connections(endpoints.target())
            .unwrap(),
        &[connection]
    );

    let figure = viewer.parts().get(part).unwrap().primary_figure();
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Ok(vec![figure])
    );
    assert!(matches!(
        viewer
            .connection_state(ConnectionId::from_figure(figure))
            .unwrap()
            .resolution,
        ConnectionResolution::Resolved { .. }
    ));
}

#[test]
fn connection_part_id_is_a_checked_view_of_edit_part_identity() {
    let (viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let node = viewer.part_for_model(FIRST).unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();

    assert_eq!(
        connection.edit_part(),
        viewer.part_for_model(EDGE_A).unwrap()
    );
    assert!(viewer.parts().as_connection(node).is_err());
    assert_eq!(
        viewer.parts().as_connection(connection.edit_part()),
        Ok(connection)
    );
}

#[test]
fn self_loop_is_indexed_once_at_both_ends() {
    let (viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, FIRST)]);
    let node = viewer.part_for_model(FIRST).unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();

    assert_eq!(
        viewer.parts().source_connections(node).unwrap(),
        &[connection]
    );
    assert_eq!(
        viewer.parts().target_connections(node).unwrap(),
        &[connection]
    );
}

#[test]
fn model_order_drives_layer_and_relation_order_without_recreating_parts() {
    let (mut viewer, _) = viewer_with(vec![
        edge(EDGE_A, FIRST, SECOND),
        edge(EDGE_B, FIRST, THIRD),
    ]);
    let first = viewer.connection_part_for_model(EDGE_A).unwrap();
    let second = viewer.connection_part_for_model(EDGE_B).unwrap();
    let first_figure = viewer
        .parts()
        .get(first.edit_part())
        .unwrap()
        .primary_figure();
    let second_figure = viewer
        .parts()
        .get(second.edit_part())
        .unwrap()
        .primary_figure();

    viewer
        .synchronize_model(|model| {
            model.connections.reverse();
            model.publish();
        })
        .unwrap();

    assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(first));
    assert_eq!(viewer.connection_part_for_model(EDGE_B), Some(second));
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Ok(vec![second_figure, first_figure])
    );
    assert_eq!(
        viewer
            .parts()
            .source_connections(viewer.part_for_model(FIRST).unwrap())
            .unwrap(),
        &[second, first]
    );
}

#[test]
fn unchanged_connection_projection_does_not_reroute_each_connection() {
    let connections = (0..LINEAR_REFRESH_CONNECTION_COUNT)
        .map(|index| edge(ModelId(100 + index), FIRST, SECOND))
        .collect();
    let (mut viewer, _) = viewer_with(connections);
    let route_count = Arc::new(AtomicUsize::new(0));
    let router = viewer
        .register_connection_router(Box::new(CountingRouter {
            route_count: Arc::clone(&route_count),
        }))
        .unwrap();
    viewer.set_connection_layer_router(router).unwrap();
    prepare_next_submission(&mut viewer).unwrap();
    assert_eq!(
        route_count.load(Ordering::Relaxed),
        LINEAR_REFRESH_CONNECTION_COUNT as usize
    );
    route_count.store(0, Ordering::Relaxed);

    viewer.synchronize_model(DiagramModel::publish).unwrap();
    let _ = prepare_next_submission(&mut viewer);

    assert_eq!(
        route_count.load(Ordering::Relaxed),
        0,
        "an unchanged projection must not dirty or reroute connections"
    );
}

#[test]
fn reconnect_preserves_part_figure_order_and_unchanged_anchor() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    let before = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();

    viewer
        .synchronize_model(|model| {
            model.connections[0] = edge(EDGE_A, FIRST, THIRD);
            model.publish();
        })
        .unwrap();

    let after_part = viewer.connection_part_for_model(EDGE_A).unwrap();
    let after = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    let endpoints = viewer.parts().connection_endpoints(after_part).unwrap();
    assert_eq!(after_part, connection);
    assert_eq!(
        viewer
            .parts()
            .get(after_part.edit_part())
            .unwrap()
            .primary_figure(),
        figure
    );
    assert_eq!(after.source, before.source);
    assert_ne!(after.target, before.target);
    assert_eq!(endpoints.source(), viewer.part_for_model(FIRST).unwrap());
    assert_eq!(endpoints.target(), viewer.part_for_model(THIRD).unwrap());
}

#[test]
fn endpoint_anchor_descriptor_reuses_stable_keys_and_replaces_only_changed_end() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    viewer
        .synchronize_model(|model| {
            model.anchor_versions.insert(FIRST, 1);
            model.publish();
        })
        .unwrap();

    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    let first = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();

    viewer.synchronize_model(DiagramModel::publish).unwrap();
    let same_key = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    assert_eq!(same_key.source, first.source);
    assert_eq!(same_key.target, first.target);

    viewer
        .synchronize_model(|model| {
            model.anchor_versions.insert(FIRST, 2);
            model.publish();
        })
        .unwrap();
    let changed_key = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    assert_ne!(changed_key.source, first.source);
    assert_eq!(changed_key.target, first.target);
    assert_eq!(
        viewer.connection_part_for_model(EDGE_A),
        Some(connection),
        "anchor replacement must retain ConnectionPart identity"
    );
}

#[test]
fn duplicate_and_missing_endpoint_snapshots_are_rejected() {
    let duplicate = GraphicalViewer::new(
        DiagramModel::new(vec![
            edge(EDGE_A, FIRST, SECOND),
            edge(EDGE_A, SECOND, THIRD),
        ]),
        DiagramFactory {
            lifecycle: Arc::new(Mutex::new(Vec::new())),
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    );
    assert!(matches!(
        duplicate,
        Err(ViewerError::DuplicateConnectionModel { .. })
    ));

    let missing = GraphicalViewer::new(
        DiagramModel::new(vec![edge(EDGE_A, FIRST, ModelId(99))]),
        DiagramFactory {
            lifecycle: Arc::new(Mutex::new(Vec::new())),
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    );
    assert!(matches!(
        missing,
        Err(ViewerError::MissingConnectionEndpoint { .. })
    ));
}

#[test]
fn connection_and_containment_model_identity_collision_is_rejected() {
    let result = GraphicalViewer::new(
        DiagramModel::new(vec![edge(FIRST, FIRST, SECOND)]),
        DiagramFactory {
            lifecycle: Arc::new(Mutex::new(Vec::new())),
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    );

    assert!(matches!(
        result,
        Err(ViewerError::ConnectionModelCollision { .. })
    ));
}

#[test]
fn connection_add_and_remove_preserve_unrelated_part_identity() {
    let (mut viewer, _) = viewer_with(Vec::new());
    let first = viewer.part_for_model(FIRST).unwrap();

    viewer
        .synchronize_model(|model| {
            model.connections.push(edge(EDGE_A, FIRST, SECOND));
            model.publish();
        })
        .unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();

    viewer
        .synchronize_model(|model| {
            model.connections.clear();
            model.publish();
        })
        .unwrap();

    assert_eq!(viewer.part_for_model(FIRST), Some(first));
    assert!(viewer.parts().get(connection.edit_part()).is_none());
    assert_eq!(
        viewer.runtime().tree().is_attached(figure),
        Err(novadraw::tree::TreeQueryError::UnknownFigure(figure))
    );
}

#[test]
fn node_and_connection_can_be_removed_in_one_model_revision() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let node = viewer.part_for_model(SECOND).unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let connection_figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    viewer.replace_selection(connection.edit_part()).unwrap();

    viewer
        .synchronize_model(|model| {
            model.connections.clear();
            model.children.retain(|id| *id != SECOND);
            model.bounds.remove(&SECOND);
            model.publish();
        })
        .unwrap();

    assert!(viewer.parts().get(node).is_none());
    assert!(viewer.parts().get(connection.edit_part()).is_none());
    assert!(viewer.selection().is_empty());
    assert_eq!(viewer.part_for_model(EDGE_A), None);
    assert_eq!(
        viewer.connection_state(ConnectionId::from_figure(connection_figure)),
        Err(ConnectionRuntimeError::UnknownConnection(
            ConnectionId::from_figure(connection_figure)
        ))
    );
}

#[test]
fn unresolved_route_keeps_a_live_recoverable_connection_part() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let mut model = DiagramModel::new(vec![edge(EDGE_A, FIRST, SECOND)]);
    model
        .bounds
        .insert(SECOND, Rectangle::new(240.0, 60.0, 0.0, 0.0));
    let mut viewer = GraphicalViewer::new(
        model,
        DiagramFactory {
            lifecycle,
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();

    assert!(matches!(
        viewer
            .connection_state(ConnectionId::from_figure(figure))
            .unwrap()
            .resolution,
        ConnectionResolution::Unresolved(_)
    ));
    assert!(!viewer.is_faulted());
    let before = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    viewer
        .synchronize_model(|model| {
            model
                .bounds
                .insert(SECOND, Rectangle::new(240.0, 60.0, 100.0, 80.0));
            model.publish();
        })
        .unwrap();
    prepare_next_submission(&mut viewer).unwrap();
    let after = viewer
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    assert!(matches!(
        after.resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert_eq!(before.source, after.source);
    assert_eq!(before.target, after.target);
    assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(connection));
    assert!(!viewer.is_faulted());
}

#[test]
fn revision_drift_during_snapshot_is_rejected() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let model = DiagramModel {
        drift_during_connections: Cell::new(true),
        ..DiagramModel::new(vec![edge(EDGE_A, FIRST, SECOND)])
    };
    let result = GraphicalViewer::new(
        model,
        DiagramFactory {
            lifecycle,
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    );

    assert!(matches!(result, Err(ViewerError::SnapshotRevisionChanged)));
}

#[test]
fn dangling_endpoint_faults_before_existing_projection_changes() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let node = viewer.part_for_model(SECOND).unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();

    assert!(matches!(
        viewer.synchronize_model(|model| {
            model.children.retain(|id| *id != SECOND);
            model.bounds.remove(&SECOND);
            model.publish();
        }),
        Err(ViewerError::MissingConnectionEndpoint { .. })
    ));
    assert!(viewer.is_faulted());
    assert!(viewer.parts().get(node).is_some());
    assert!(viewer.runtime().tree().is_attached(figure).unwrap());
}

#[test]
fn connection_factory_must_create_a_connection_figure() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let result = GraphicalViewer::new(
        DiagramModel::new(vec![edge(EDGE_A, FIRST, SECOND)]),
        DiagramFactory {
            lifecycle,
            invalid_connection_figure: true,
            rejected_connection: None,
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    );

    assert!(matches!(
        result,
        Err(ViewerError::InvalidConnectionFigure { .. })
    ));
}

#[test]
fn viewer_drop_deactivates_connections_before_endpoint_parts() {
    let lifecycle = {
        let (viewer, lifecycle) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
        lifecycle.lock().unwrap().clear();
        drop(viewer);
        lifecycle
    };

    let events = lifecycle.lock().unwrap();
    assert_eq!(events.first(), Some(&Lifecycle::Deactivate(EDGE_A)));
    assert!(events.contains(&Lifecycle::Deactivate(FIRST)));
    assert!(events.contains(&Lifecycle::Deactivate(SECOND)));
}

#[test]
fn endpoint_geometry_change_is_rerouted_by_runtime() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    let before = viewer.runtime().tree().figure_bounds(figure).unwrap();

    viewer
        .synchronize_model(|model| {
            model
                .bounds
                .insert(FIRST, Rectangle::new(80.0, 180.0, 100.0, 80.0));
            model.publish();
        })
        .unwrap();
    prepare_next_submission(&mut viewer).unwrap();

    assert_ne!(viewer.runtime().tree().figure_bounds(figure), Ok(before));
    assert!(matches!(
        viewer
            .connection_state(ConnectionId::from_figure(figure))
            .unwrap()
            .resolution,
        ConnectionResolution::Resolved { .. }
    ));
}

#[test]
fn rejected_incremental_connection_creation_leaves_no_registration() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let mut viewer = GraphicalViewer::new(
        DiagramModel::new(vec![edge(EDGE_A, FIRST, SECOND)]),
        DiagramFactory {
            lifecycle,
            invalid_connection_figure: false,
            rejected_connection: Some(EDGE_B),
            rejected_activation: None,
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap();
    let layer_before = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().connection())
        .unwrap();

    let result = viewer.synchronize_model(|model| {
        model.connections.push(edge(EDGE_B, SECOND, THIRD));
        model.publish();
    });
    assert!(
        matches!(result, Err(ViewerError::EditPart(_))),
        "{result:?}"
    );
    assert!(viewer.is_faulted());
    assert_eq!(viewer.part_for_model(EDGE_B), None);
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Ok(layer_before)
    );
}

#[test]
fn failed_connection_activation_rolls_back_all_registrations() {
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    let mut viewer = GraphicalViewer::new(
        DiagramModel::new(vec![edge(EDGE_A, FIRST, SECOND)]),
        DiagramFactory {
            lifecycle,
            invalid_connection_figure: false,
            rejected_connection: None,
            rejected_activation: Some(EDGE_B),
        },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap();
    let layer_before = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().connection())
        .unwrap();

    assert!(matches!(
        viewer.synchronize_model(|model| {
            model.connections.push(edge(EDGE_B, SECOND, THIRD));
            model.publish();
        }),
        Err(ViewerError::EditPart(_))
    ));
    assert!(viewer.is_faulted());
    assert_eq!(viewer.part_for_model(EDGE_B), None);
    assert_eq!(viewer.connection_part_for_model(EDGE_B), None);
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Ok(layer_before)
    );
}

#[test]
fn late_connection_refusals_and_panic_preserve_existing_projection() {
    for failure in [
        ConnectionFailure::TargetProvider,
        ConnectionFailure::BindingConstraint,
        ConnectionFailure::ActivationPanic,
    ] {
        for target in [THIRD, SECOND] {
            let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
            let existing = viewer.connection_part_for_model(EDGE_A).unwrap();
            let existing_figure = viewer
                .parts()
                .get(existing.edit_part())
                .unwrap()
                .primary_figure();
            let existing_state = viewer
                .connection_state(ConnectionId::from_figure(existing_figure))
                .unwrap();
            let before = viewer
                .runtime()
                .tree()
                .child_order(viewer.root_layers().connection());
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                viewer.synchronize_model(|model| {
                    model.failure = Some(failure);
                    model.connections.push(edge(EDGE_B, SECOND, target));
                    model.publish();
                })
            }));
            assert!(!matches!(result, Ok(Ok(_))));
            assert!(viewer.is_faulted());
            assert!(!viewer.runtime().is_faulted());
            assert_eq!(viewer.connection_part_for_model(EDGE_B), None);
            assert_eq!(
                viewer
                    .runtime()
                    .tree()
                    .child_order(viewer.root_layers().connection()),
                before
            );
            assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(existing));
            let after = viewer
                .connection_state(ConnectionId::from_figure(existing_figure))
                .unwrap();
            assert_eq!(after.source, existing_state.source);
            assert_eq!(after.target, existing_state.target);
            let cleanup = viewer.last_projection_cleanup().unwrap();
            assert!(cleanup.failures.is_empty(), "{cleanup:?}");
            assert!(cleanup.live_figures.is_empty());
            assert!(cleanup.live_anchors.is_empty());
            assert!(cleanup.live_parts.is_empty());
            assert!(!cleanup.retired_figures.is_empty());
        }
    }
}

#[test]
fn connection_cleanup_lifecycle_panic_preserves_original_error_and_retired_ids() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let callbacks = Arc::clone(&viewer.model().cleanup_callbacks);
    let existing = viewer.connection_part_for_model(EDGE_A).unwrap();
    let error = viewer
        .synchronize_model(|model| {
            model.failure = Some(ConnectionFailure::CleanupPanic);
            model.connections.push(edge(EDGE_B, SECOND, SECOND));
            model.publish();
        })
        .unwrap_err();
    let ViewerError::ProjectionCleanup { original, report } = error else {
        panic!("missing cleanup report");
    };
    assert!(
        original
            .to_string()
            .contains("connection activation rejected")
    );
    assert!(report.runtime_faulted);
    assert!(report.live_figures.is_empty());
    assert!(report.live_parts.is_empty());
    assert!(report.live_anchors.is_empty());
    assert_eq!(report.retired_figures.len(), 2);
    assert_eq!(report.retired_anchors.len(), 2);
    assert!(
        report
            .retired_figures
            .iter()
            .all(|id| viewer.runtime().tree().is_attached(*id)
                == Err(novadraw::tree::TreeQueryError::UnknownFigure(*id)))
    );
    assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(existing));
    assert!(viewer.synchronize().is_err());
    drop(viewer);
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
}

#[test]
fn invalid_route_output_clears_geometry_and_recovers_without_recreating_resources() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let part = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(part.edit_part())
        .unwrap()
        .primary_figure();
    let connection = ConnectionId::from_figure(figure);
    let before = viewer.connection_state(connection).unwrap();
    viewer
        .synchronize_model(|model| {
            model.invalid_route.store(true, Ordering::SeqCst);
            model
                .bounds
                .insert(FIRST, Rectangle::new(60.0, 60.0, 100.0, 80.0));
            model.publish();
        })
        .unwrap();
    let unresolved = viewer.connection_state(connection).unwrap();
    assert!(
        matches!(unresolved.resolution, ConnectionResolution::Unresolved(
        novadraw::connection::UnresolvedConnection::InvalidGeometry(id)
    ) if id == figure)
    );
    assert_eq!(
        viewer.runtime().tree().figure_bounds(figure),
        Ok(Rectangle::ZERO)
    );
    assert_eq!(unresolved.source, before.source);
    assert_eq!(unresolved.target, before.target);
    assert!(unresolved.dependency_count > 0);
    assert!(!viewer.is_faulted());
    viewer
        .synchronize_model(|model| {
            model.invalid_route.store(false, Ordering::SeqCst);
            model
                .bounds
                .insert(FIRST, Rectangle::new(80.0, 60.0, 100.0, 80.0));
            model.publish();
        })
        .unwrap();
    let recovered = viewer.connection_state(connection).unwrap();
    assert!(matches!(
        recovered.resolution,
        ConnectionResolution::Resolved { .. }
    ));
    assert_eq!(recovered.source, before.source);
    assert_eq!(recovered.target, before.target);
    assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(part));
    assert!(!viewer.is_faulted());
}
