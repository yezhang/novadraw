use std::{
    cell::Cell,
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};

use novadraw_editor::{
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionPartFactoryContext,
    EditPartBehavior, EditPartError, EditPartFactory, GraphicalViewer, ModelAdapter,
    ModelConnection, ModelEvent, ModelRevision, PartFactoryContext, PartKind, ViewerError,
    VisualUpdateContext,
};
use novadraw_geometry::Rectangle;
use novadraw_scene::{
    AnchorSemanticKey, ChopboxAnchor, ConnectionFigure, ConnectionId, ConnectionResolution,
    ConnectionRuntimeError, Figure, RectangleFigure, RootFigure,
};

const ROOT: ModelId = ModelId(1);
const FIRST: ModelId = ModelId(2);
const SECOND: ModelId = ModelId(3);
const THIRD: ModelId = ModelId(4);
const EDGE_A: ModelId = ModelId(10);
const EDGE_B: ModelId = ModelId(11);

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

impl EditPartBehavior<DiagramModel> for ConnectionPart {
    fn create_figure(
        &mut self,
        _model: &DiagramModel,
        _model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(ConnectionFigure::new()))
    }

    fn activate(&mut self, _model: &DiagramModel, model_id: ModelId) -> Result<(), EditPartError> {
        if self.reject_activation {
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
    let (viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
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
        Some(vec![figure])
    );
    assert!(matches!(
        viewer
            .runtime()
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

    viewer.model_mut().unwrap().connections.reverse();
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();

    assert_eq!(viewer.connection_part_for_model(EDGE_A), Some(first));
    assert_eq!(viewer.connection_part_for_model(EDGE_B), Some(second));
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Some(vec![second_figure, first_figure])
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
fn reconnect_preserves_part_figure_order_and_unchanged_anchor() {
    let (mut viewer, _) = viewer_with(vec![edge(EDGE_A, FIRST, SECOND)]);
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    let before = viewer
        .runtime()
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();

    viewer.model_mut().unwrap().connections[0] = edge(EDGE_A, FIRST, THIRD);
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();

    let after_part = viewer.connection_part_for_model(EDGE_A).unwrap();
    let after = viewer
        .runtime()
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
    viewer.model_mut().unwrap().anchor_versions.insert(FIRST, 1);
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();

    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();
    let first = viewer
        .runtime()
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();

    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();
    let same_key = viewer
        .runtime()
        .connection_state(ConnectionId::from_figure(figure))
        .unwrap();
    assert_eq!(same_key.source, first.source);
    assert_eq!(same_key.target, first.target);

    viewer.model_mut().unwrap().anchor_versions.insert(FIRST, 2);
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();
    let changed_key = viewer
        .runtime()
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
        .model_mut()
        .unwrap()
        .connections
        .push(edge(EDGE_A, FIRST, SECOND));
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();
    let connection = viewer.connection_part_for_model(EDGE_A).unwrap();
    let figure = viewer
        .parts()
        .get(connection.edit_part())
        .unwrap()
        .primary_figure();

    viewer.model_mut().unwrap().connections.clear();
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();

    assert_eq!(viewer.part_for_model(FIRST), Some(first));
    assert!(viewer.parts().get(connection.edit_part()).is_none());
    assert!(!viewer.runtime().tree().is_attached(figure));
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

    viewer.model_mut().unwrap().connections.clear();
    viewer
        .model_mut()
        .unwrap()
        .children
        .retain(|id| *id != SECOND);
    viewer.model_mut().unwrap().bounds.remove(&SECOND);
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();

    assert!(viewer.parts().get(node).is_none());
    assert!(viewer.parts().get(connection.edit_part()).is_none());
    assert!(viewer.selection().is_empty());
    assert_eq!(viewer.part_for_model(EDGE_A), None);
    assert_eq!(
        viewer
            .runtime()
            .connection_state(ConnectionId::from_figure(connection_figure)),
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
    let viewer = GraphicalViewer::new(
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
            .runtime()
            .connection_state(ConnectionId::from_figure(figure))
            .unwrap()
            .resolution,
        ConnectionResolution::Unresolved(_)
    ));
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

    viewer
        .model_mut()
        .unwrap()
        .children
        .retain(|id| *id != SECOND);
    viewer.model_mut().unwrap().bounds.remove(&SECOND);
    viewer.model_mut().unwrap().publish();

    assert!(matches!(
        viewer.refresh(),
        Err(ViewerError::MissingConnectionEndpoint { .. })
    ));
    assert!(viewer.is_faulted());
    assert!(viewer.parts().get(node).is_some());
    assert!(viewer.runtime().tree().is_attached(figure));
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
        .model_mut()
        .unwrap()
        .bounds
        .insert(FIRST, Rectangle::new(80.0, 180.0, 100.0, 80.0));
    viewer.model_mut().unwrap().publish();
    viewer.refresh().unwrap();
    viewer.runtime_mut().prepare_frame().unwrap();

    assert_ne!(viewer.runtime().tree().figure_bounds(figure), Some(before));
    assert!(matches!(
        viewer
            .runtime()
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

    viewer
        .model_mut()
        .unwrap()
        .connections
        .push(edge(EDGE_B, SECOND, THIRD));
    viewer.model_mut().unwrap().publish();

    assert!(matches!(viewer.refresh(), Err(ViewerError::EditPart(_))));
    assert!(viewer.is_faulted());
    assert_eq!(viewer.part_for_model(EDGE_B), None);
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Some(layer_before)
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

    viewer
        .model_mut()
        .unwrap()
        .connections
        .push(edge(EDGE_B, SECOND, THIRD));
    viewer.model_mut().unwrap().publish();

    assert!(matches!(viewer.refresh(), Err(ViewerError::EditPart(_))));
    assert!(viewer.is_faulted());
    assert_eq!(viewer.part_for_model(EDGE_B), None);
    assert_eq!(viewer.connection_part_for_model(EDGE_B), None);
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().connection()),
        Some(layer_before)
    );
}
