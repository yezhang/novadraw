use std::{collections::HashMap, convert::Infallible};

use novadraw_editor::{
    Command, CommandError, ConnectionCreation, ConnectionEndpoint, ConnectionPartFactoryContext,
    ConnectionReconnection, CreateConnectionRequest, CreationType, DeleteRequest, EditPartBehavior,
    EditPartError, EditPartFactory, EditPolicy, EditorDomain, EditorRequest, FeedbackVisual,
    GraphicalViewer, HandleRole, InteractionRevision, ModelAdapter, ModelConnection, ModelEvent,
    ModelRevision, PartFactoryContext, PolicyError, PolicyHost, PolicyInstallation, PolicyRole,
    ReconnectConnectionRequest, RequestModifiers, ViewerError,
};
use novadraw_geometry::{Point, Rectangle};
use novadraw_scene::{
    ClickableFigure, ConnectionFigure, Figure, KeyModifiers, MouseButton, PolylineFigure,
    RectangleFigure, RootFigure,
};

const ROOT: ModelId = ModelId(1);
const FIRST: ModelId = ModelId(2);
const SECOND: ModelId = ModelId(3);
const WIDGET: ModelId = ModelId(4);
const FIRST_EDGE: ModelId = ModelId(100);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ModelId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramEvent {
    Changed,
}

struct DiagramModel {
    revision: ModelRevision,
    children: Vec<ModelId>,
    bounds: HashMap<ModelId, Rectangle>,
    connections: Vec<ModelConnection<ModelId>>,
    events: Vec<ModelEvent<ModelId, DiagramEvent>>,
    next_connection: u64,
}

impl DiagramModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            children: vec![FIRST, SECOND, WIDGET],
            bounds: HashMap::from([
                (ROOT, Rectangle::new(0.0, 0.0, 600.0, 400.0)),
                (FIRST, Rectangle::new(40.0, 60.0, 100.0, 80.0)),
                (SECOND, Rectangle::new(240.0, 60.0, 100.0, 80.0)),
                (WIDGET, Rectangle::new(440.0, 60.0, 100.0, 80.0)),
            ]),
            connections: Vec::new(),
            events: Vec::new(),
            next_connection: FIRST_EDGE.0,
        }
    }

    fn publish(&mut self, subject: ModelId) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(
            self.revision,
            subject,
            DiagramEvent::Changed,
        ));
    }

    fn insert_connection(&mut self, connection: ModelConnection<ModelId>, index: usize) {
        self.connections.insert(index, connection);
        self.next_connection = self.next_connection.max(connection.id().0 + 1);
        self.publish(connection.id());
    }

    fn remove_connection(&mut self, id: ModelId) -> (ModelConnection<ModelId>, usize) {
        let index = self
            .connections
            .iter()
            .position(|connection| connection.id() == id)
            .unwrap();
        let connection = self.connections.remove(index);
        self.publish(id);
        (connection, index)
    }

    fn replace_connection(&mut self, connection: ModelConnection<ModelId>) {
        let slot = self
            .connections
            .iter_mut()
            .find(|candidate| candidate.id() == connection.id())
            .unwrap();
        *slot = connection;
        self.publish(connection.id());
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
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == ROOT {
            self.children.clone()
        } else {
            Vec::new()
        })
    }

    fn connections(&self) -> Result<Vec<ModelConnection<Self::ModelId>>, Self::Error> {
        Ok(self.connections.clone())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct CreateEdgeCommand {
    connection: ModelConnection<ModelId>,
    index: usize,
}

impl Command<DiagramModel> for CreateEdgeCommand {
    fn label(&self) -> &str {
        "Create connection"
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.insert_connection(self.connection, self.index);
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        let (connection, index) = model.remove_connection(self.connection.id());
        if connection != self.connection || index != self.index {
            return Err(CommandError::state_unknown(
                "connection identity or order changed during undo",
            ));
        }
        Ok(())
    }
}

struct EdgeCreation {
    source: ModelId,
}

impl ConnectionCreation<DiagramModel> for EdgeCreation {
    fn can_complete(
        &self,
        source: PolicyHost<ModelId>,
        target: PolicyHost<ModelId>,
        request: &CreateConnectionRequest,
        model: &DiagramModel,
    ) -> Result<bool, PolicyError> {
        Ok(source.model() == self.source
            && request.connection_type().as_str() == "edge"
            && target.model() != WIDGET
            && !model.connections.iter().any(|connection| {
                connection.source() == source.model() && connection.target() == target.model()
            }))
    }

    fn feedback(
        &mut self,
        source: PolicyHost<ModelId>,
        target: Option<PolicyHost<ModelId>>,
        request: &CreateConnectionRequest,
        model: &DiagramModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let source_bounds = model.bounds[&source.model()];
        let start = Point::new(
            source_bounds.x + source_bounds.width / 2.0,
            source_bounds.y + source_bounds.height / 2.0,
        );
        let end = target.map_or(request.location(), |target| {
            let bounds = model.bounds[&target.model()];
            Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            )
        });
        let mut feedback = vec![FeedbackVisual::scaled(Box::new(PolylineFigure::new(
            start.x(),
            start.y(),
            end.x(),
            end.y(),
        )))];
        if let Some(target) = target {
            feedback.push(FeedbackVisual::scaled(Box::new(
                RectangleFigure::from_bounds(model.bounds[&target.model()]),
            )));
        }
        Ok(feedback)
    }

    fn command(
        &mut self,
        source: PolicyHost<ModelId>,
        target: PolicyHost<ModelId>,
        _request: &CreateConnectionRequest,
        model: &DiagramModel,
    ) -> Result<Box<dyn Command<DiagramModel>>, PolicyError> {
        Ok(Box::new(CreateEdgeCommand {
            connection: ModelConnection::new(
                ModelId(model.next_connection),
                source.model(),
                target.model(),
            ),
            index: model.connections.len(),
        }))
    }
}

struct NodePolicy;

impl EditPolicy<DiagramModel> for NodePolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::CreateConnection(_))
    }

    fn command(
        &mut self,
        _host: PolicyHost<ModelId>,
        _request: &EditorRequest,
        _model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_connection(
        &mut self,
        host: PolicyHost<ModelId>,
        request: &CreateConnectionRequest,
        _model: &DiagramModel,
    ) -> Result<Option<Box<dyn ConnectionCreation<DiagramModel>>>, PolicyError> {
        Ok((request.connection_type().as_str() == "edge").then(|| {
            Box::new(EdgeCreation {
                source: host.model(),
            }) as Box<dyn ConnectionCreation<DiagramModel>>
        }))
    }
}

struct DiagramPart {
    duplicate_policy: bool,
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
        } else if model_id == WIDGET {
            Box::new(ClickableFigure::new(bounds))
        } else {
            Box::new(RectangleFigure::from_bounds(bounds))
        })
    }

    fn create_policies(
        &mut self,
        _model: &DiagramModel,
        model_id: ModelId,
    ) -> Result<Vec<PolicyInstallation<DiagramModel>>, EditPartError> {
        if model_id == ROOT || model_id == WIDGET {
            return Ok(Vec::new());
        }
        let mut policies: Vec<PolicyInstallation<DiagramModel>> =
            vec![(PolicyRole::ConnectionCreation, Box::new(NodePolicy))];
        if self.duplicate_policy {
            policies.push((
                PolicyRole::custom("duplicate").unwrap(),
                Box::new(NodePolicy),
            ));
        }
        Ok(policies)
    }
}

struct ConnectionPart;

struct DeleteEdgeCommand {
    id: ModelId,
    removed: Option<(ModelConnection<ModelId>, usize)>,
}

impl Command<DiagramModel> for DeleteEdgeCommand {
    fn label(&self) -> &str {
        "Delete connection"
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        self.removed = Some(model.remove_connection(self.id));
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        let (connection, index) = self
            .removed
            .take()
            .ok_or_else(|| CommandError::state_unknown("deleted edge snapshot is missing"))?;
        model.insert_connection(connection, index);
        Ok(())
    }
}

struct ConnectionDeletePolicy;

struct ReconnectEdgeCommand {
    before: ModelConnection<ModelId>,
    after: ModelConnection<ModelId>,
}

impl Command<DiagramModel> for ReconnectEdgeCommand {
    fn label(&self) -> &str {
        "Reconnect"
    }

    fn execute(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.replace_connection(self.after);
        Ok(())
    }

    fn undo(&mut self, model: &mut DiagramModel) -> Result<(), CommandError> {
        model.replace_connection(self.before);
        Ok(())
    }
}

struct EdgeReconnection {
    before: ModelConnection<ModelId>,
}

impl ConnectionReconnection<DiagramModel> for EdgeReconnection {
    fn can_complete(
        &self,
        _connection: PolicyHost<ModelId>,
        _fixed: PolicyHost<ModelId>,
        candidate: PolicyHost<ModelId>,
        _request: &ReconnectConnectionRequest,
        _model: &DiagramModel,
    ) -> Result<bool, PolicyError> {
        Ok(candidate.model() != WIDGET)
    }

    fn command(
        &mut self,
        _connection: PolicyHost<ModelId>,
        fixed: PolicyHost<ModelId>,
        candidate: PolicyHost<ModelId>,
        request: &ReconnectConnectionRequest,
        _model: &DiagramModel,
    ) -> Result<Box<dyn Command<DiagramModel>>, PolicyError> {
        let after = match request.endpoint() {
            ConnectionEndpoint::Source => {
                ModelConnection::new(self.before.id(), candidate.model(), fixed.model())
            }
            ConnectionEndpoint::Target => {
                ModelConnection::new(self.before.id(), fixed.model(), candidate.model())
            }
        };
        Ok(Box::new(ReconnectEdgeCommand {
            before: self.before,
            after,
        }))
    }
}

struct ConnectionReconnectPolicy;

impl EditPolicy<DiagramModel> for ConnectionReconnectPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::ReconnectConnection(_))
    }

    fn command(
        &mut self,
        _host: PolicyHost<ModelId>,
        _request: &EditorRequest,
        _model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_reconnection(
        &mut self,
        host: PolicyHost<ModelId>,
        _request: &ReconnectConnectionRequest,
        model: &DiagramModel,
    ) -> Result<Option<Box<dyn ConnectionReconnection<DiagramModel>>>, PolicyError> {
        Ok(model
            .connections
            .iter()
            .find(|connection| connection.id() == host.model())
            .copied()
            .map(|before| {
                Box::new(EdgeReconnection { before })
                    as Box<dyn ConnectionReconnection<DiagramModel>>
            }))
    }
}

impl EditPolicy<DiagramModel> for ConnectionDeletePolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Delete(_))
    }

    fn command(
        &mut self,
        host: PolicyHost<ModelId>,
        _request: &EditorRequest,
        _model: &DiagramModel,
    ) -> Result<Option<Box<dyn Command<DiagramModel>>>, PolicyError> {
        Ok(Some(Box::new(DeleteEdgeCommand {
            id: host.model(),
            removed: None,
        })))
    }
}

impl EditPartBehavior<DiagramModel> for ConnectionPart {
    fn create_figure(
        &mut self,
        _model: &DiagramModel,
        _model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(ConnectionFigure::new()))
    }

    fn create_policies(
        &mut self,
        _model: &DiagramModel,
        _model_id: ModelId,
    ) -> Result<Vec<PolicyInstallation<DiagramModel>>, EditPartError> {
        Ok(vec![
            (PolicyRole::Component, Box::new(ConnectionDeletePolicy)),
            (
                PolicyRole::ConnectionReconnect,
                Box::new(ConnectionReconnectPolicy),
            ),
        ])
    }
}

struct DiagramFactory {
    duplicate_policy: bool,
}

impl EditPartFactory<DiagramModel> for DiagramFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<ModelId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        Ok(Box::new(DiagramPart {
            duplicate_policy: self.duplicate_policy,
        }))
    }

    fn create_connection(
        &mut self,
        _context: ConnectionPartFactoryContext<ModelId>,
        _model: &DiagramModel,
    ) -> Result<Box<dyn EditPartBehavior<DiagramModel>>, EditPartError> {
        Ok(Box::new(ConnectionPart))
    }
}

fn viewer(duplicate_policy: bool) -> GraphicalViewer<DiagramModel, DiagramFactory> {
    GraphicalViewer::new(
        DiagramModel::new(),
        DiagramFactory { duplicate_policy },
        Rectangle::new(0.0, 0.0, 600.0, 400.0),
    )
    .unwrap()
}

fn arm(
    domain: &mut EditorDomain<DiagramModel>,
    viewer: &mut GraphicalViewer<DiagramModel, DiagramFactory>,
) {
    domain
        .activate_connection_creation(viewer, CreationType::new("edge").unwrap())
        .unwrap();
}

fn click(
    domain: &mut EditorDomain<DiagramModel>,
    viewer: &mut GraphicalViewer<DiagramModel, DiagramFactory>,
    point: Point,
) {
    domain
        .pointer_pressed(viewer, point, MouseButton::Left, KeyModifiers::default())
        .unwrap();
    domain
        .pointer_released(viewer, point, MouseButton::Left)
        .unwrap();
}

#[test]
fn typed_request_preserves_source_target_and_interaction_state() {
    let viewer = viewer(false);
    let source = viewer.part_for_model(FIRST).unwrap();
    let target = viewer.part_for_model(SECOND).unwrap();
    let request = CreateConnectionRequest::new(
        CreationType::new("edge").unwrap(),
        source,
        Point::new(12.0, 34.0),
        RequestModifiers {
            shift: true,
            ..RequestModifiers::default()
        },
        InteractionRevision::initial(),
    )
    .with_target_candidate(Some(target));

    assert_eq!(request.source(), source);
    assert_eq!(request.target_candidate(), Some(target));
    assert_eq!(request.location(), Point::new(12.0, 34.0));
    assert!(request.modifiers().shift);
}

#[test]
fn two_stage_tool_locks_source_preserves_selection_and_commits_after_feedback_cleanup() {
    let mut viewer = viewer(false);
    let selected = viewer.part_for_model(SECOND).unwrap();
    viewer.replace_selection(selected).unwrap();
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);

    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    assert!(domain.has_active_gesture());
    assert_eq!(viewer.selection().items(), &[selected]);
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .len(),
        1
    );

    domain
        .pointer_moved(&mut viewer, Point::new(280.0, 90.0))
        .unwrap();
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .len(),
        2
    );
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));

    assert!(!domain.is_connection_creation_active());
    assert!(!domain.has_active_gesture());
    assert_eq!(viewer.selection().items(), &[selected]);
    assert!(viewer.connection_part_for_model(FIRST_EDGE).is_some());
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
    assert_eq!(domain.command_stack().undo_len(), 1);
}

#[test]
fn invalid_target_keeps_gesture_and_escape_cancels_without_model_change() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));

    click(&mut domain, &mut viewer, Point::new(560.0, 360.0));
    assert!(domain.has_active_gesture());
    assert!(viewer.model().connections.is_empty());

    domain.cancel_tool(&mut viewer).unwrap();
    assert!(!domain.is_connection_creation_active());
    assert!(!domain.has_active_gesture());
    assert!(viewer.model().connections.is_empty());
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn widget_consumed_press_does_not_start_or_complete_connection() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);

    click(&mut domain, &mut viewer, Point::new(480.0, 90.0));
    assert!(!domain.has_active_gesture());
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(480.0, 90.0));

    assert!(domain.has_active_gesture());
    assert!(viewer.model().connections.is_empty());
}

#[test]
fn duplicate_source_plans_are_rejected_without_starting_a_gesture() {
    let mut viewer = viewer(true);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);

    let result = domain.pointer_pressed(
        &mut viewer,
        Point::new(80.0, 90.0),
        MouseButton::Left,
        KeyModifiers::default(),
    );
    assert!(matches!(
        result,
        Err(novadraw_editor::EditorDomainError::Tool(
            novadraw_editor::ToolError::Viewer(ViewerError::Policy(_))
        ))
    ));
    assert!(!domain.has_active_gesture());
}

#[test]
fn committed_connection_undo_redo_preserves_model_identity_and_projection() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    let original_part = viewer.connection_part_for_model(FIRST_EDGE).unwrap();

    domain.undo(&mut viewer).unwrap();
    assert!(viewer.connection_part_for_model(FIRST_EDGE).is_none());
    domain.redo(&mut viewer).unwrap();

    let rebuilt = viewer.connection_part_for_model(FIRST_EDGE).unwrap();
    assert_ne!(rebuilt, original_part);
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, SECOND)]
    );
}

#[test]
fn selected_connection_deletes_and_undo_restores_the_same_model_identity() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    let connection_part = viewer
        .connection_part_for_model(FIRST_EDGE)
        .unwrap()
        .edit_part();
    viewer.replace_selection(connection_part).unwrap();

    let request = EditorRequest::Delete(DeleteRequest::new(
        vec![connection_part],
        domain.next_interaction_revision().unwrap(),
    ));
    domain.execute_request(&mut viewer, &request).unwrap();

    assert!(viewer.model().connections.is_empty());
    assert!(viewer.connection_part_for_model(FIRST_EDGE).is_none());
    assert!(viewer.selection().is_empty());

    domain.undo(&mut viewer).unwrap();
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, SECOND)]
    );
    assert!(viewer.connection_part_for_model(FIRST_EDGE).is_some());
}

#[test]
fn target_endpoint_handle_reconnects_and_undo_preserves_connection_part_identity() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    let connection = viewer.connection_part_for_model(FIRST_EDGE).unwrap();
    let original_part = connection.edit_part();
    viewer
        .add_handle_visual_with_role(
            original_part,
            HandleRole::ConnectionEndpoint(ConnectionEndpoint::Target),
            Box::new(RectangleFigure::new(275.0, 85.0, 10.0, 10.0)),
        )
        .unwrap();

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(280.0, 90.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(80.0, 90.0))
        .unwrap();
    let release = domain
        .pointer_released(&mut viewer, Point::new(80.0, 90.0), MouseButton::Left)
        .unwrap();

    assert!(release.command_executed());
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, FIRST)]
    );
    assert_eq!(
        viewer
            .connection_part_for_model(FIRST_EDGE)
            .unwrap()
            .edit_part(),
        original_part
    );

    domain.undo(&mut viewer).unwrap();
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, SECOND)]
    );
    assert_eq!(
        viewer
            .connection_part_for_model(FIRST_EDGE)
            .unwrap()
            .edit_part(),
        original_part
    );
}

#[test]
fn source_endpoint_reconnects_while_invalid_drop_has_no_effect() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    let connection = viewer.connection_part_for_model(FIRST_EDGE).unwrap();
    viewer
        .add_handle_visual_with_role(
            connection.edit_part(),
            HandleRole::ConnectionEndpoint(ConnectionEndpoint::Source),
            Box::new(RectangleFigure::new(75.0, 85.0, 10.0, 10.0)),
        )
        .unwrap();

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 90.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(480.0, 90.0))
        .unwrap();
    let invalid = domain
        .pointer_released(&mut viewer, Point::new(480.0, 90.0), MouseButton::Left)
        .unwrap();
    assert!(!invalid.command_executed());
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, SECOND)]
    );

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(80.0, 90.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(280.0, 90.0))
        .unwrap();
    let valid = domain
        .pointer_released(&mut viewer, Point::new(280.0, 90.0), MouseButton::Left)
        .unwrap();
    assert!(valid.command_executed());
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, SECOND, SECOND)]
    );
}

#[test]
fn reconnect_cancel_clears_feedback_without_model_change() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    let connection = viewer.connection_part_for_model(FIRST_EDGE).unwrap();
    viewer
        .add_handle_visual_with_role(
            connection.edit_part(),
            HandleRole::ConnectionEndpoint(ConnectionEndpoint::Target),
            Box::new(RectangleFigure::new(275.0, 85.0, 10.0, 10.0)),
        )
        .unwrap();

    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(280.0, 90.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(80.0, 90.0))
        .unwrap();
    assert!(domain.has_active_gesture());
    domain.cancel_tool(&mut viewer).unwrap();

    assert!(!domain.has_active_gesture());
    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, SECOND)]
    );
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn policy_can_accept_a_self_loop() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));

    assert_eq!(
        viewer.model().connections,
        vec![ModelConnection::new(FIRST_EDGE, FIRST, FIRST)]
    );
    let connection = viewer.connection_part_for_model(FIRST_EDGE).unwrap();
    let (source, target) = viewer
        .connection_route_endpoints_in_surface(connection)
        .unwrap();
    assert_ne!(source, target);
    assert!(source.x() >= 140.0);
    assert!(target.x() >= 140.0);
}

#[test]
fn connection_visual_is_an_invalid_target_without_aborting_the_gesture() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));

    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(190.0, 100.0));

    assert!(domain.has_active_gesture());
    assert_eq!(viewer.model().connections.len(), 1);
}

#[test]
fn history_transition_cancels_an_incomplete_connection_before_model_refresh() {
    let mut viewer = viewer(false);
    let mut domain = EditorDomain::new();
    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(80.0, 90.0));
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));

    arm(&mut domain, &mut viewer);
    click(&mut domain, &mut viewer, Point::new(280.0, 90.0));
    assert!(domain.has_active_gesture());
    domain.undo(&mut viewer).unwrap();

    assert!(!domain.is_connection_creation_active());
    assert!(!domain.has_active_gesture());
    assert!(viewer.model().connections.is_empty());
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
}
