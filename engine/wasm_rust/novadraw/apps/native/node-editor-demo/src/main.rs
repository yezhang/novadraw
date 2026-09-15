use std::{collections::HashMap, convert::Infallible, sync::Arc};

use novadraw::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, BuiltinFont, Color,
    ConnectionFigure, DirectRouter, Figure, KeyModifiers, MouseButton, PlatformHost, Point,
    PolylineFigure, Rectangle, RectangleFigure, RenderBackend, RenderOutcome, RootFigure,
    backend::vello::VelloRenderer, rectangle_boundary_site,
};
use novadraw_apps::WinitPlatformHost;
use novadraw_editor::{
    BendpointOperation, BendpointRequest, Command, CommandError, ConnectionCreation,
    ConnectionEndpoint, ConnectionFeedbackRoute, ConnectionPartFactoryContext,
    ConnectionReconnection, ConnectionRouterKey, ConnectionRouterRegistration,
    ConnectionRoutingDescriptor, CreateConnectionRequest, CreateRequest, CreationType,
    DeleteRequest, EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorDomain,
    EditorRequest, FeedbackVisual, GraphicalViewer, HandleRole, ModelAdapter, ModelConnection,
    ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHost, PolicyInstallation,
    PolicyRole, ReconnectConnectionRequest, RequestModifiers, ResizeDirection, VisualUpdateContext,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

mod self_loop_router;

use self_loop_router::VisibleSelfLoopRouter;

const WIDTH: f64 = 820.0;
const HEIGHT: f64 = 560.0;
const HANDLE_SIZE: f64 = 10.0;
const BENDPOINT_CREATE_HANDLE_SIZE: f64 = 7.0;
const BENDPOINT_DELETE_TOLERANCE: f64 = 7.0;
const MIN_NODE_SIZE: f64 = 32.0;
const CREATED_NODE_WIDTH: f64 = 140.0;
const CREATED_NODE_HEIGHT: f64 = 90.0;
const CREATED_NODE_OFFSET: f64 = 18.0;
const FIRST_CONNECTION_ID: u64 = 1_000;
const SELF_LOOP_EXTENT: f64 = 32.0;
const BENDPOINT_ROUTER_KEY: &str = "demo-bendpoint";

fn bendpoint_router_key() -> ConnectionRouterKey {
    ConnectionRouterKey::new(BENDPOINT_ROUTER_KEY).expect("static Router key is valid")
}
const PRIMARY_HANDLE_COLOR: Color = Color {
    r: 0.98,
    g: 0.78,
    b: 0.12,
    a: 1.0,
};
const BENDPOINT_HANDLE_COLOR: Color = Color {
    r: 0.94,
    g: 0.42,
    b: 0.12,
    a: 1.0,
};
const BENDPOINT_CREATE_HANDLE_COLOR: Color = Color {
    r: 0.1,
    g: 0.72,
    b: 0.72,
    a: 1.0,
};
const SECONDARY_HANDLE_COLOR: Color = Color {
    r: 0.12,
    g: 0.78,
    b: 0.82,
    a: 1.0,
};
const FEEDBACK_COLOR: Color = Color {
    r: 0.85,
    g: 0.36,
    b: 0.08,
    a: 1.0,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Clone, Copy)]
enum NodeKind {
    Canvas,
    Shape(Color),
    Widget,
}

#[derive(Clone, Copy)]
struct Node {
    bounds: Rectangle,
    kind: NodeKind,
}

struct DemoModel {
    revision: ModelRevision,
    nodes: HashMap<NodeId, Node>,
    children: HashMap<NodeId, Vec<NodeId>>,
    connections: Vec<ModelConnection<NodeId>>,
    bendpoints: HashMap<NodeId, Vec<Point>>,
    events: Vec<ModelEvent<NodeId, DemoEvent>>,
    next_id: u64,
    next_connection_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoEvent {
    Changed,
}

impl DemoModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            nodes: HashMap::from([
                (
                    NodeId(1),
                    Node {
                        bounds: Rectangle::new(0.0, 0.0, WIDTH, HEIGHT),
                        kind: NodeKind::Canvas,
                    },
                ),
                (
                    NodeId(2),
                    Node {
                        bounds: Rectangle::new(80.0, 100.0, 180.0, 120.0),
                        kind: NodeKind::Shape(Color::hex("#2878D0")),
                    },
                ),
                (
                    NodeId(3),
                    Node {
                        bounds: Rectangle::new(330.0, 190.0, 200.0, 130.0),
                        kind: NodeKind::Shape(Color::hex("#38A169")),
                    },
                ),
                (
                    NodeId(4),
                    Node {
                        bounds: Rectangle::new(610.0, 80.0, 120.0, 60.0),
                        kind: NodeKind::Widget,
                    },
                ),
            ]),
            children: HashMap::from([(NodeId(1), vec![NodeId(2), NodeId(3), NodeId(4)])]),
            connections: Vec::new(),
            bendpoints: HashMap::new(),
            events: Vec::new(),
            next_id: 5,
            next_connection_id: FIRST_CONNECTION_ID,
        }
    }

    fn publish(&mut self, subject: NodeId) {
        self.revision = self
            .revision
            .next()
            .expect("manual demo must not exhaust model revisions");
        self.events
            .push(ModelEvent::new(self.revision, subject, DemoEvent::Changed));
    }

    fn set_bounds(&mut self, id: NodeId, bounds: Rectangle) {
        self.nodes
            .get_mut(&id)
            .expect("bounds command references a live node")
            .bounds = bounds;
        self.publish(id);
    }

    fn insert(&mut self, id: NodeId, node: Node, index: usize) {
        assert!(self.nodes.insert(id, node).is_none());
        self.children
            .entry(NodeId(1))
            .or_default()
            .insert(index, id);
        self.next_id = self.next_id.max(id.0 + 1);
        self.publish(NodeId(1));
    }

    fn remove(&mut self, id: NodeId) -> RemovedNode {
        let children = self
            .children
            .get_mut(&NodeId(1))
            .expect("canvas child list exists");
        let index = children
            .iter()
            .position(|candidate| *candidate == id)
            .expect("delete command references a canvas child");
        children.remove(index);
        let node = self.nodes.remove(&id).expect("deleted node exists");
        let mut connections = Vec::new();
        for (connection_index, connection) in self.connections.iter().copied().enumerate() {
            if connection.source() == id || connection.target() == id {
                connections.push((
                    connection_index,
                    connection,
                    self.bendpoints.remove(&connection.id()).unwrap_or_default(),
                ));
            }
        }
        self.connections
            .retain(|connection| connection.source() != id && connection.target() != id);
        self.publish(NodeId(1));
        RemovedNode {
            node,
            index,
            connections,
        }
    }

    fn restore(&mut self, id: NodeId, removed: RemovedNode) {
        self.insert(id, removed.node, removed.index);
        for (index, connection, bendpoints) in removed.connections {
            self.connections.insert(index, connection);
            self.bendpoints.insert(connection.id(), bendpoints);
        }
    }

    fn insert_connection(&mut self, connection: ModelConnection<NodeId>, index: usize) {
        self.connections.insert(index, connection);
        self.bendpoints.entry(connection.id()).or_default();
        self.next_connection_id = self.next_connection_id.max(connection.id().0 + 1);
        self.publish(connection.id());
    }

    fn remove_connection(&mut self, id: NodeId) -> RemovedConnection {
        let index = self
            .connections
            .iter()
            .position(|connection| connection.id() == id)
            .expect("connection command references a live connection");
        let connection = self.connections.remove(index);
        let bendpoints = self.bendpoints.remove(&id).unwrap_or_default();
        self.publish(id);
        RemovedConnection {
            connection,
            index,
            bendpoints,
        }
    }

    fn replace_connection(&mut self, connection: ModelConnection<NodeId>) {
        let slot = self
            .connections
            .iter_mut()
            .find(|candidate| candidate.id() == connection.id())
            .expect("reconnect command references a live connection");
        *slot = connection;
        self.publish(connection.id());
    }

    fn set_bendpoints(&mut self, connection: NodeId, bendpoints: Vec<Point>) {
        self.bendpoints.insert(connection, bendpoints);
        self.publish(connection);
    }
}

struct RemovedNode {
    node: Node,
    index: usize,
    connections: Vec<(usize, ModelConnection<NodeId>, Vec<Point>)>,
}

struct RemovedConnection {
    connection: ModelConnection<NodeId>,
    index: usize,
    bendpoints: Vec<Point>,
}

impl ModelAdapter for DemoModel {
    type ModelId = NodeId;
    type Event = DemoEvent;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        NodeId(1)
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(self.children.get(&model).cloned().unwrap_or_default())
    }

    fn connections(&self) -> Result<Vec<ModelConnection<Self::ModelId>>, Self::Error> {
        Ok(self.connections.clone())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct SetBoundsCommand {
    id: NodeId,
    before: Rectangle,
    after: Rectangle,
}

impl Command<DemoModel> for SetBoundsCommand {
    fn label(&self) -> &str {
        "Set bounds"
    }

    fn can_execute(&self, _model: &DemoModel) -> bool {
        self.after.width >= MIN_NODE_SIZE && self.after.height >= MIN_NODE_SIZE
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.set_bounds(self.id, self.after);
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.set_bounds(self.id, self.before);
        Ok(())
    }
}

struct CreateNodeCommand {
    id: NodeId,
    node: Node,
    index: usize,
}

impl Command<DemoModel> for CreateNodeCommand {
    fn label(&self) -> &str {
        "Create node"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.insert(self.id, self.node, self.index);
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        let _ = model.remove(self.id);
        Ok(())
    }
}

struct DeleteNodeCommand {
    id: NodeId,
    removed: Option<RemovedNode>,
}

impl Command<DemoModel> for DeleteNodeCommand {
    fn label(&self) -> &str {
        "Delete node"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        self.removed = Some(model.remove(self.id));
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        let removed = self
            .removed
            .take()
            .ok_or_else(|| CommandError::state_unknown("deleted node snapshot is missing"))?;
        model.restore(self.id, removed);
        Ok(())
    }

    fn redo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        self.execute(model)
    }
}

struct CreateConnectionCommand {
    connection: ModelConnection<NodeId>,
    index: usize,
}

impl Command<DemoModel> for CreateConnectionCommand {
    fn label(&self) -> &str {
        "Create connection"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.insert_connection(self.connection, self.index);
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        let removed = model.remove_connection(self.connection.id());
        if removed.connection != self.connection || removed.index != self.index {
            return Err(CommandError::state_unknown(
                "connection identity or order changed during undo",
            ));
        }
        Ok(())
    }
}

struct DeleteConnectionCommand {
    id: NodeId,
    removed: Option<RemovedConnection>,
}

struct ReconnectCommand {
    before: ModelConnection<NodeId>,
    after: ModelConnection<NodeId>,
}

struct SetBendpointsCommand {
    connection: NodeId,
    before: Vec<Point>,
    after: Vec<Point>,
}

impl Command<DemoModel> for SetBendpointsCommand {
    fn label(&self) -> &str {
        "Edit bendpoint"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.set_bendpoints(self.connection, self.after.clone());
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.set_bendpoints(self.connection, self.before.clone());
        Ok(())
    }
}

impl Command<DemoModel> for ReconnectCommand {
    fn label(&self) -> &str {
        "Reconnect"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.replace_connection(self.after);
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.replace_connection(self.before);
        Ok(())
    }
}

impl Command<DemoModel> for DeleteConnectionCommand {
    fn label(&self) -> &str {
        "Delete connection"
    }

    fn execute(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        self.removed = Some(model.remove_connection(self.id));
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        let removed = self
            .removed
            .take()
            .ok_or_else(|| CommandError::state_unknown("deleted connection snapshot is missing"))?;
        let connection = removed.connection;
        model.insert_connection(connection, removed.index);
        model.bendpoints.insert(connection.id(), removed.bendpoints);
        Ok(())
    }
}

struct DemoConnectionCreation {
    source: NodeId,
}

impl ConnectionCreation<DemoModel> for DemoConnectionCreation {
    fn can_complete(
        &self,
        source: PolicyHost<NodeId>,
        target: PolicyHost<NodeId>,
        request: &CreateConnectionRequest,
        model: &DemoModel,
    ) -> Result<bool, PolicyError> {
        let source_is_shape = model
            .nodes
            .get(&source.model())
            .is_some_and(|node| matches!(node.kind, NodeKind::Shape(_)));
        let target_is_shape = model
            .nodes
            .get(&target.model())
            .is_some_and(|node| matches!(node.kind, NodeKind::Shape(_)));
        Ok(source.model() == self.source
            && request.connection_type().as_str() == "connection"
            && source_is_shape
            && target_is_shape
            && !model.connections.iter().any(|connection| {
                connection.source() == source.model() && connection.target() == target.model()
            }))
    }

    fn feedback(
        &mut self,
        source: PolicyHost<NodeId>,
        target: Option<PolicyHost<NodeId>>,
        request: &CreateConnectionRequest,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let source_bounds = model.nodes[&source.model()].bounds;
        let source_center = Point::new(
            source_bounds.x + source_bounds.width / 2.0,
            source_bounds.y + source_bounds.height / 2.0,
        );
        let reference = target.map_or(request.location(), |target| {
            let bounds = model.nodes[&target.model()].bounds;
            Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            )
        });
        let source_point = rectangle_boundary_site(source_bounds, reference).point;
        let target_point = target.map_or(request.location(), |target| {
            rectangle_boundary_site(model.nodes[&target.model()].bounds, source_center).point
        });
        let line = PolylineFigure::new_with_color(
            source_point.x(),
            source_point.y(),
            target_point.x(),
            target_point.y(),
            FEEDBACK_COLOR,
        )
        .with_width(3.0);
        let mut feedback = vec![FeedbackVisual::scaled(Box::new(line))];
        if let Some(target) = target {
            let bounds = model.nodes[&target.model()].bounds;
            let highlight = RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                Color::TRANSPARENT,
            )
            .with_stroke(FEEDBACK_COLOR, 3.0);
            feedback.push(FeedbackVisual::scaled(Box::new(highlight)));
        }
        Ok(feedback)
    }

    fn feedback_with_route(
        &mut self,
        source: PolicyHost<NodeId>,
        target: Option<PolicyHost<NodeId>>,
        request: &CreateConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let Some(route) = route else {
            return self.feedback(source, target, request, model);
        };
        let line = PolylineFigure::new_with_color(
            route.source().x(),
            route.source().y(),
            route.target().x(),
            route.target().y(),
            FEEDBACK_COLOR,
        )
        .with_width(3.0);
        let mut feedback = vec![FeedbackVisual::scaled(Box::new(line))];
        if let Some(target) = target {
            let bounds = model.nodes[&target.model()].bounds;
            feedback.push(FeedbackVisual::scaled(Box::new(
                RectangleFigure::new_with_color(
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                    Color::TRANSPARENT,
                )
                .with_stroke(FEEDBACK_COLOR, 3.0),
            )));
        }
        Ok(feedback)
    }

    fn command(
        &mut self,
        source: PolicyHost<NodeId>,
        target: PolicyHost<NodeId>,
        _request: &CreateConnectionRequest,
        model: &DemoModel,
    ) -> Result<Box<dyn Command<DemoModel>>, PolicyError> {
        Ok(Box::new(CreateConnectionCommand {
            connection: ModelConnection::new(
                NodeId(model.next_connection_id),
                source.model(),
                target.model(),
            ),
            index: model.connections.len(),
        }))
    }
}

struct NodePolicy;

impl EditPolicy<DemoModel> for NodePolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(
            request,
            EditorRequest::ChangeBounds(_) | EditorRequest::Delete(_)
        )
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        match request {
            EditorRequest::ChangeBounds(request) => {
                let node = model
                    .nodes
                    .get(&host.model())
                    .ok_or_else(|| PolicyError::operation("bounds target no longer exists"))?;
                if matches!(node.kind, NodeKind::Canvas | NodeKind::Widget) {
                    return Err(PolicyError::operation(
                        "node does not support bounds editing",
                    ));
                }
                Ok(Some(Box::new(SetBoundsCommand {
                    id: host.model(),
                    before: node.bounds,
                    after: request.transformed_bounds(node.bounds),
                })))
            }
            EditorRequest::Delete(_) => {
                let node = model
                    .nodes
                    .get(&host.model())
                    .ok_or_else(|| PolicyError::operation("delete target no longer exists"))?;
                if matches!(node.kind, NodeKind::Canvas | NodeKind::Widget) {
                    return Err(PolicyError::operation("node is not deletable"));
                }
                Ok(Some(Box::new(DeleteNodeCommand {
                    id: host.model(),
                    removed: None,
                })))
            }
            EditorRequest::Create(_)
            | EditorRequest::CreateConnection(_)
            | EditorRequest::ReconnectConnection(_)
            | EditorRequest::Bendpoint(_) => Ok(None),
        }
    }

    fn feedback(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let EditorRequest::ChangeBounds(request) = request else {
            return Ok(Vec::new());
        };
        let bounds = request.transformed_bounds(model.nodes[&host.model()].bounds);
        let feedback = RectangleFigure::new_with_color(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            Color::TRANSPARENT,
        )
        .with_stroke(FEEDBACK_COLOR, 2.0);
        Ok(vec![FeedbackVisual::scaled(Box::new(feedback))])
    }
}

struct ConnectionPolicy;

impl EditPolicy<DemoModel> for ConnectionPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::CreateConnection(_))
    }

    fn command(
        &mut self,
        _host: PolicyHost<NodeId>,
        _request: &EditorRequest,
        _model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_connection(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &CreateConnectionRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn ConnectionCreation<DemoModel>>>, PolicyError> {
        let is_shape = model
            .nodes
            .get(&host.model())
            .is_some_and(|node| matches!(node.kind, NodeKind::Shape(_)));
        Ok(
            (is_shape && request.connection_type().as_str() == "connection").then(|| {
                Box::new(DemoConnectionCreation {
                    source: host.model(),
                }) as Box<dyn ConnectionCreation<DemoModel>>
            }),
        )
    }
}

struct CanvasPolicy;

impl EditPolicy<DemoModel> for CanvasPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Create(_))
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        let EditorRequest::Create(request) = request else {
            return Ok(None);
        };
        if host.model() != NodeId(1) || request.creation_type().as_str() != "shape" {
            return Err(PolicyError::operation(
                "unsupported creation target or type",
            ));
        }
        Ok(Some(Box::new(CreateNodeCommand {
            id: NodeId(model.next_id),
            node: Node {
                bounds: request.bounds(),
                kind: NodeKind::Shape(Color::hex("#805AD5")),
            },
            index: model.children.get(&NodeId(1)).map_or(0, Vec::len),
        })))
    }
}

struct DemoPart;

impl EditPartBehavior<DemoModel> for DemoPart {
    fn create_figure(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let node = model.nodes[&model_id];
        Ok(match node.kind {
            NodeKind::Canvas => Box::new(RootFigure::new(
                node.bounds.x,
                node.bounds.y,
                node.bounds.width,
                node.bounds.height,
            )),
            NodeKind::Shape(color) => Box::new(
                RectangleFigure::new_with_color(
                    node.bounds.x,
                    node.bounds.y,
                    node.bounds.width,
                    node.bounds.height,
                    color,
                )
                .with_stroke(Color::hex("#17202A"), 2.0),
            ),
            NodeKind::Widget => {
                Box::new(novadraw::ButtonFigure::new("Widget").with_bounds(node.bounds))
            }
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.nodes[&model_id].bounds)?;
        Ok(())
    }

    fn create_policies(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
    ) -> Result<Vec<PolicyInstallation<DemoModel>>, EditPartError> {
        Ok(match model.nodes[&model_id].kind {
            NodeKind::Canvas => vec![(PolicyRole::Layout, Box::new(CanvasPolicy))],
            NodeKind::Shape(_) => vec![
                (PolicyRole::Component, Box::new(NodePolicy)),
                (PolicyRole::ConnectionCreation, Box::new(ConnectionPolicy)),
            ],
            NodeKind::Widget => vec![(PolicyRole::Component, Box::new(NodePolicy))],
        })
    }
}

struct DemoConnectionPart;

struct ConnectionDeletePolicy;

struct DemoReconnection {
    before: ModelConnection<NodeId>,
}

impl ConnectionReconnection<DemoModel> for DemoReconnection {
    fn can_complete(
        &self,
        _connection: PolicyHost<NodeId>,
        fixed: PolicyHost<NodeId>,
        candidate: PolicyHost<NodeId>,
        request: &ReconnectConnectionRequest,
        model: &DemoModel,
    ) -> Result<bool, PolicyError> {
        let candidate_is_shape = model
            .nodes
            .get(&candidate.model())
            .is_some_and(|node| matches!(node.kind, NodeKind::Shape(_)));
        let (source, target) = match request.endpoint() {
            ConnectionEndpoint::Source => (candidate.model(), fixed.model()),
            ConnectionEndpoint::Target => (fixed.model(), candidate.model()),
        };
        Ok(candidate_is_shape
            && !model.connections.iter().any(|connection| {
                connection.id() != self.before.id()
                    && connection.source() == source
                    && connection.target() == target
            }))
    }

    fn feedback(
        &mut self,
        _connection: PolicyHost<NodeId>,
        fixed: PolicyHost<NodeId>,
        candidate: Option<PolicyHost<NodeId>>,
        request: &ReconnectConnectionRequest,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let fixed_bounds = model.nodes[&fixed.model()].bounds;
        let fixed_center = fixed_bounds.center();
        let moving_reference = candidate.map_or(request.location(), |candidate| {
            model.nodes[&candidate.model()].bounds.center()
        });
        let fixed_point = rectangle_boundary_site(fixed_bounds, moving_reference).point;
        let moving_point = candidate.map_or(request.location(), |candidate| {
            rectangle_boundary_site(model.nodes[&candidate.model()].bounds, fixed_center).point
        });
        let (start, end) = match request.endpoint() {
            ConnectionEndpoint::Source => (moving_point, fixed_point),
            ConnectionEndpoint::Target => (fixed_point, moving_point),
        };
        Ok(vec![FeedbackVisual::scaled(Box::new(
            PolylineFigure::new_with_color(start.x(), start.y(), end.x(), end.y(), FEEDBACK_COLOR)
                .with_width(3.0),
        ))])
    }

    fn feedback_with_route(
        &mut self,
        connection: PolicyHost<NodeId>,
        fixed: PolicyHost<NodeId>,
        candidate: Option<PolicyHost<NodeId>>,
        request: &ReconnectConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let Some(route) = route else {
            return self.feedback(connection, fixed, candidate, request, model);
        };
        let mut points = Vec::new();
        points.push(route.source());
        points.extend(
            model
                .bendpoints
                .get(&connection.model())
                .into_iter()
                .flatten()
                .copied(),
        );
        points.push(route.target());
        Ok(vec![FeedbackVisual::scaled(Box::new(
            PolylineFigure::from_points(points)
                .with_color(FEEDBACK_COLOR)
                .with_width(3.0),
        ))])
    }

    fn command(
        &mut self,
        _connection: PolicyHost<NodeId>,
        fixed: PolicyHost<NodeId>,
        candidate: PolicyHost<NodeId>,
        request: &ReconnectConnectionRequest,
        _model: &DemoModel,
    ) -> Result<Box<dyn Command<DemoModel>>, PolicyError> {
        let after = match request.endpoint() {
            ConnectionEndpoint::Source => {
                ModelConnection::new(self.before.id(), candidate.model(), fixed.model())
            }
            ConnectionEndpoint::Target => {
                ModelConnection::new(self.before.id(), fixed.model(), candidate.model())
            }
        };
        Ok(Box::new(ReconnectCommand {
            before: self.before,
            after,
        }))
    }
}

impl EditPolicy<DemoModel> for ConnectionDeletePolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Delete(_))
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        if !matches!(request, EditorRequest::Delete(_)) {
            return Ok(None);
        }
        if !model
            .connections
            .iter()
            .any(|connection| connection.id() == host.model())
        {
            return Err(PolicyError::operation(
                "connection delete target no longer exists",
            ));
        }
        Ok(Some(Box::new(DeleteConnectionCommand {
            id: host.model(),
            removed: None,
        })))
    }
}

struct ConnectionReconnectPolicy;

impl EditPolicy<DemoModel> for ConnectionReconnectPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::ReconnectConnection(_))
    }

    fn command(
        &mut self,
        _host: PolicyHost<NodeId>,
        _request: &EditorRequest,
        _model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_reconnection(
        &mut self,
        host: PolicyHost<NodeId>,
        _request: &ReconnectConnectionRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn ConnectionReconnection<DemoModel>>>, PolicyError> {
        Ok(model
            .connections
            .iter()
            .find(|connection| connection.id() == host.model())
            .copied()
            .map(|before| {
                Box::new(DemoReconnection { before }) as Box<dyn ConnectionReconnection<DemoModel>>
            }))
    }
}

struct ConnectionBendpointPolicy;

impl EditPolicy<DemoModel> for ConnectionBendpointPolicy {
    fn understands(&self, request: &EditorRequest) -> bool {
        matches!(request, EditorRequest::Bendpoint(_))
    }

    fn command(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Option<Box<dyn Command<DemoModel>>>, PolicyError> {
        let EditorRequest::Bendpoint(request) = request else {
            return Ok(None);
        };
        let before = model
            .bendpoints
            .get(&host.model())
            .cloned()
            .unwrap_or_default();
        let after = apply_bendpoint_request(model, host.model(), request, &before)?;
        Ok((after != before).then(|| {
            Box::new(SetBendpointsCommand {
                connection: host.model(),
                before,
                after,
            }) as Box<dyn Command<DemoModel>>
        }))
    }

    fn feedback(
        &mut self,
        host: PolicyHost<NodeId>,
        request: &EditorRequest,
        model: &DemoModel,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let EditorRequest::Bendpoint(request) = request else {
            return Ok(Vec::new());
        };
        let before = model
            .bendpoints
            .get(&host.model())
            .cloned()
            .unwrap_or_default();
        let bendpoints = apply_bendpoint_request(model, host.model(), request, &before)?;
        let connection = model
            .connections
            .iter()
            .find(|connection| connection.id() == host.model())
            .ok_or_else(|| PolicyError::operation("bendpoint connection no longer exists"))?;
        let source_bounds = model.nodes[&connection.source()].bounds;
        let target_bounds = model.nodes[&connection.target()].bounds;
        let source_reference = bendpoints
            .first()
            .copied()
            .unwrap_or_else(|| target_bounds.center());
        let target_reference = bendpoints
            .last()
            .copied()
            .unwrap_or_else(|| source_bounds.center());
        let mut points = Vec::with_capacity(bendpoints.len() + 2);
        points.push(rectangle_boundary_site(source_bounds, source_reference).point);
        points.extend(bendpoints);
        points.push(rectangle_boundary_site(target_bounds, target_reference).point);
        Ok(vec![FeedbackVisual::scaled(Box::new(
            PolylineFigure::from_points(points)
                .with_color(FEEDBACK_COLOR)
                .with_width(3.0),
        ))])
    }
}

fn apply_bendpoint_request(
    model: &DemoModel,
    connection_id: NodeId,
    request: &BendpointRequest,
    before: &[Point],
) -> Result<Vec<Point>, PolicyError> {
    let connection = model
        .connections
        .iter()
        .find(|connection| connection.id() == connection_id)
        .ok_or_else(|| PolicyError::operation("bendpoint connection no longer exists"))?;
    let mut after = before.to_vec();
    match request.operation() {
        BendpointOperation::Create { index } if index <= after.len() => {
            after.insert(index, request.location());
        }
        BendpointOperation::Move { index } if index < after.len() => {
            let previous = index
                .checked_sub(1)
                .and_then(|previous| before.get(previous).copied())
                .unwrap_or_else(|| model.nodes[&connection.source()].bounds.center());
            let next = before
                .get(index + 1)
                .copied()
                .unwrap_or_else(|| model.nodes[&connection.target()].bounds.center());
            if point_segment_distance(request.location(), previous, next)
                <= BENDPOINT_DELETE_TOLERANCE
            {
                after.remove(index);
            } else {
                after[index] = request.location();
            }
        }
        BendpointOperation::Delete { index } if index < after.len() => {
            after.remove(index);
        }
        BendpointOperation::Create { .. }
        | BendpointOperation::Move { .. }
        | BendpointOperation::Delete { .. } => {
            return Err(PolicyError::operation("bendpoint index is out of range"));
        }
    }
    Ok(after)
}

fn point_segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f64::EPSILON {
        return (point - start).length();
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    (point - (start + segment * projection)).length()
}

impl EditPartBehavior<DemoModel> for DemoConnectionPart {
    fn create_figure(
        &mut self,
        _model: &DemoModel,
        _model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(
            ConnectionFigure::new().with_stroke(Color::hex("#34495E"), 3.0),
        ))
    }

    fn create_policies(
        &mut self,
        _model: &DemoModel,
        _model_id: NodeId,
    ) -> Result<Vec<PolicyInstallation<DemoModel>>, EditPartError> {
        Ok(vec![
            (PolicyRole::Component, Box::new(ConnectionDeletePolicy)),
            (
                PolicyRole::ConnectionReconnect,
                Box::new(ConnectionReconnectPolicy),
            ),
            (
                PolicyRole::custom("connection-bendpoint")
                    .expect("static bendpoint policy role is valid"),
                Box::new(ConnectionBendpointPolicy),
            ),
        ])
    }

    fn connection_routing(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
    ) -> Result<ConnectionRoutingDescriptor, EditPartError> {
        let bendpoints = model.bendpoints.get(&model_id).cloned().unwrap_or_default();
        Ok(if bendpoints.is_empty() {
            ConnectionRoutingDescriptor::inherited()
        } else {
            ConnectionRoutingDescriptor::registered(
                bendpoint_router_key(),
                Some(Box::new(BendpointConstraint::new(
                    bendpoints
                        .into_iter()
                        .map(Bendpoint::Absolute)
                        .collect::<Vec<_>>(),
                ))),
            )
        })
    }

    fn connection_bendpoints(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
    ) -> Result<Vec<Point>, EditPartError> {
        Ok(model.bendpoints.get(&model_id).cloned().unwrap_or_default())
    }
}

struct DemoFactory;

impl EditPartFactory<DemoModel> for DemoFactory {
    fn connection_routers(&mut self) -> Result<Vec<ConnectionRouterRegistration>, EditPartError> {
        Ok(vec![ConnectionRouterRegistration::new(
            bendpoint_router_key(),
            Box::new(
                VisibleSelfLoopRouter::new(Box::new(BendpointConnectionRouter), SELF_LOOP_EXTENT)
                    .expect("demo self-loop extent is valid"),
            ),
        )])
    }

    fn create(
        &mut self,
        _context: PartFactoryContext<NodeId>,
        _model: &DemoModel,
    ) -> Result<Box<dyn EditPartBehavior<DemoModel>>, EditPartError> {
        Ok(Box::new(DemoPart))
    }

    fn create_connection(
        &mut self,
        _context: ConnectionPartFactoryContext<NodeId>,
        _model: &DemoModel,
    ) -> Result<Box<dyn EditPartBehavior<DemoModel>>, EditPartError> {
        Ok(Box::new(DemoConnectionPart))
    }
}

type DemoViewer = GraphicalViewer<DemoModel, DemoFactory>;

struct DemoApp {
    window: Option<Arc<Window>>,
    host: Option<WinitPlatformHost>,
    renderer: Option<VelloRenderer>,
    viewer: Option<DemoViewer>,
    domain: EditorDomain<DemoModel>,
    cursor: Option<(f64, f64)>,
    modifiers: KeyModifiers,
    handles: Vec<novadraw::FigureId>,
}

impl DemoApp {
    fn new() -> Self {
        Self {
            window: None,
            host: None,
            renderer: None,
            viewer: None,
            domain: EditorDomain::new(),
            cursor: None,
            modifiers: KeyModifiers::default(),
            handles: Vec::new(),
        }
    }

    fn request_redraw(&self) {
        if let Some(host) = &self.host {
            host.request_redraw();
        }
    }

    fn sync_surface(&mut self) {
        let Some(host) = &self.host else {
            return;
        };
        let surface = host.surface_info();
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(
                surface.pixel_width,
                surface.pixel_height,
                surface.scale_factor,
            );
        }
        if let Some(viewer) = &mut self.viewer {
            viewer
                .runtime_mut()
                .resize_logical_viewport(surface.logical_width, surface.logical_height)
                .expect("window size must be valid");
        }
        self.request_redraw();
    }

    fn sync_selection_handles(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        for figure in self.handles.drain(..) {
            let _ = viewer.remove_overlay_visual(figure);
        }
        let selected = viewer.selection().items().to_vec();
        let primary = viewer.selection().primary();
        for part in selected {
            if let Some(connection) = viewer.as_connection_part(part) {
                let Some(route) = viewer.connection_route_points_in_surface(connection) else {
                    continue;
                };
                let Some((&source, &target)) = route.first().zip(route.last()) else {
                    continue;
                };
                for (point, endpoint) in [
                    (source, ConnectionEndpoint::Source),
                    (target, ConnectionEndpoint::Target),
                ] {
                    let handle = RectangleFigure::new_with_color(
                        point.x() - HANDLE_SIZE / 2.0,
                        point.y() - HANDLE_SIZE / 2.0,
                        HANDLE_SIZE,
                        HANDLE_SIZE,
                        PRIMARY_HANDLE_COLOR,
                    )
                    .with_stroke(Color::BLACK, 1.0);
                    if let Ok((_, figure)) = viewer.add_handle_visual_with_role(
                        part,
                        HandleRole::ConnectionEndpoint(endpoint),
                        Box::new(handle),
                    ) {
                        self.handles.push(figure);
                    }
                }
                let bendpoint_sites = viewer
                    .connection_bendpoint_handle_sites(connection)
                    .unwrap_or_default();
                for site in bendpoint_sites {
                    let (size, color) = match site.role() {
                        HandleRole::BendpointCreate(_) => {
                            (BENDPOINT_CREATE_HANDLE_SIZE, BENDPOINT_CREATE_HANDLE_COLOR)
                        }
                        HandleRole::BendpointMove(_) => (HANDLE_SIZE, BENDPOINT_HANDLE_COLOR),
                        _ => continue,
                    };
                    Self::add_bendpoint_handle(
                        &mut self.handles,
                        viewer,
                        part,
                        site.location(),
                        site.role(),
                        size,
                        color,
                    );
                }
                continue;
            }
            let Some(bounds) = viewer.part_bounds_in_surface(part) else {
                continue;
            };
            let color = if Some(part) == primary {
                PRIMARY_HANDLE_COLOR
            } else {
                SECONDARY_HANDLE_COLOR
            };
            for (x, y, direction) in [
                (bounds.x, bounds.y, ResizeDirection::NorthWest),
                (
                    bounds.x + bounds.width,
                    bounds.y,
                    ResizeDirection::NorthEast,
                ),
                (
                    bounds.x,
                    bounds.y + bounds.height,
                    ResizeDirection::SouthWest,
                ),
                (
                    bounds.x + bounds.width,
                    bounds.y + bounds.height,
                    ResizeDirection::SouthEast,
                ),
            ] {
                let handle = RectangleFigure::new_with_color(
                    x - HANDLE_SIZE / 2.0,
                    y - HANDLE_SIZE / 2.0,
                    HANDLE_SIZE,
                    HANDLE_SIZE,
                    color,
                )
                .with_stroke(Color::BLACK, 1.0);
                let result = if Some(part) == primary {
                    viewer.add_handle_visual_with_role(
                        part,
                        HandleRole::Resize(direction),
                        Box::new(handle),
                    )
                } else {
                    viewer.add_handle_visual(part, Box::new(handle))
                };
                if let Ok((_, figure)) = result {
                    self.handles.push(figure);
                }
            }
        }
    }

    fn add_bendpoint_handle(
        handles: &mut Vec<novadraw::FigureId>,
        viewer: &mut DemoViewer,
        owner: novadraw_editor::EditPartId,
        point: Point,
        role: HandleRole,
        size: f64,
        color: Color,
    ) {
        let handle = RectangleFigure::new_with_color(
            point.x() - size / 2.0,
            point.y() - size / 2.0,
            size,
            size,
            color,
        )
        .with_stroke(Color::BLACK, 1.0);
        if let Ok((_, figure)) = viewer.add_handle_visual_with_role(owner, role, Box::new(handle)) {
            handles.push(figure);
        }
    }

    fn render(&mut self) {
        let (Some(viewer), Some(renderer), Some(host)) =
            (&mut self.viewer, &mut self.renderer, &self.host)
        else {
            return;
        };
        let Some(submission) = viewer
            .runtime_mut()
            .prepare_submission(host.surface_info(), renderer.capabilities())
        else {
            return;
        };
        let outcome = renderer.submit(&submission);
        viewer.runtime_mut().complete_submission(
            submission.session_id,
            submission.frame_id,
            outcome,
        );
        if outcome == RenderOutcome::Retry || viewer.runtime().has_pending_update() {
            host.request_redraw();
        }
    }

    fn update_title(&self) {
        let (Some(window), Some(viewer)) = (&self.window, &self.viewer) else {
            return;
        };
        window.set_title(&format!(
            "Novadraw Node Editor - {} selected | undo {} redo {}{}",
            viewer.selection().items().len(),
            self.domain.command_stack().undo_len(),
            self.domain.command_stack().redo_len(),
            if self.domain.is_connection_creation_active() {
                " | CONNECTION"
            } else {
                ""
            },
        ));
    }

    fn activate_connection_creation(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        self.domain
            .activate_connection_creation(
                viewer,
                CreationType::new("connection").expect("static connection type is valid"),
            )
            .expect("connection creation Tool must activate");
        self.update_title();
        self.request_redraw();
    }

    fn create_node(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        let sequence = viewer.model().next_id.saturating_sub(5) as f64;
        let offset = sequence * CREATED_NODE_OFFSET;
        let request = EditorRequest::Create(CreateRequest::new(
            viewer.contents(),
            CreationType::new("shape").expect("static creation type is valid"),
            Rectangle::new(
                420.0 + offset,
                330.0 + offset,
                CREATED_NODE_WIDTH,
                CREATED_NODE_HEIGHT,
            ),
            RequestModifiers::default(),
            self.domain
                .next_interaction_revision()
                .expect("interaction revision must remain available"),
        ));
        self.domain
            .execute_request(viewer, &request)
            .expect("create request must execute");
        self.sync_selection_handles();
        self.update_title();
        self.request_redraw();
    }

    fn delete_selection(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        let parts = viewer.selection().items().to_vec();
        if parts.is_empty() {
            return;
        }
        let request = EditorRequest::Delete(DeleteRequest::new(
            parts,
            self.domain
                .next_interaction_revision()
                .expect("interaction revision must remain available"),
        ));
        let result = self.domain.execute_request(viewer, &request);
        if let Err(error) = result {
            eprintln!("delete request rejected: {error}");
            return;
        }
        self.update_title();
        self.request_redraw();
    }

    fn undo(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        if self.domain.command_stack().undo_len() == 0 {
            return;
        }
        self.domain.undo(viewer).expect("undo must execute");
        self.sync_selection_handles();
        self.update_title();
        self.request_redraw();
    }

    fn redo(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        if self.domain.command_stack().redo_len() == 0 {
            return;
        }
        self.domain.redo(viewer).expect("redo must execute");
        self.sync_selection_handles();
        self.update_title();
        self.request_redraw();
    }
}

impl ApplicationHandler<()> for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let window = self.window.clone().unwrap_or_else(|| {
            Arc::new(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("Novadraw Node Editor")
                            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
                            .with_resizable(true),
                    )
                    .expect("window creation failed"),
            )
        });
        let size = window.inner_size();
        let scale = window.scale_factor();
        self.renderer = Some(VelloRenderer::new(
            Arc::clone(&window),
            f64::from(size.width) / scale,
            f64::from(size.height) / scale,
        ));
        self.host = Some(WinitPlatformHost::new(Arc::clone(&window)));
        self.window = Some(window);

        if let Some(viewer) = &mut self.viewer {
            viewer
                .runtime_mut()
                .reset_backend_session()
                .expect("backend session reset failed");
        } else {
            let mut viewer = GraphicalViewer::new(
                DemoModel::new(),
                DemoFactory,
                Rectangle::new(0.0, 0.0, WIDTH, HEIGHT),
            )
            .expect("demo Viewer construction failed");
            let connection_layer = viewer.root_layers().connection();
            let self_loop_router = viewer.runtime_mut().register_connection_router(Box::new(
                VisibleSelfLoopRouter::new(Box::new(DirectRouter), SELF_LOOP_EXTENT)
                    .expect("default self-loop extent is valid"),
            ));
            viewer
                .runtime_mut()
                .set_connection_layer_router(connection_layer, self_loop_router)
                .expect("demo self-loop Router installation failed");
            viewer
                .runtime_mut()
                .register_builtin_font(BuiltinFont::Inter)
                .expect("built-in font registration failed");
            self.viewer = Some(viewer);
        }
        self.update_title();
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.sync_surface();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y));
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                if let Some(viewer) = &mut self.viewer {
                    self.domain
                        .pointer_moved(viewer, Point::new(position.x / scale, position.y / scale))
                        .expect("pointer move must update the active Tool");
                }
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                if let Some(viewer) = &mut self.viewer {
                    viewer.pointer_exited();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some((x, y)) = self.cursor else {
                    return;
                };
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    _ => MouseButton::None,
                };
                if let Some(viewer) = &mut self.viewer {
                    match state {
                        ElementState::Pressed => {
                            let changed = match self.domain.pointer_pressed(
                                viewer,
                                Point::new(x / scale, y / scale),
                                button,
                                self.modifiers,
                            ) {
                                Ok(outcome) => outcome.selection().is_some(),
                                Err(error) => {
                                    eprintln!("editor gesture rejected: {error}");
                                    false
                                }
                            };
                            if changed {
                                self.sync_selection_handles();
                                self.update_title();
                            }
                        }
                        ElementState::Released => {
                            if let Err(error) = self.domain.pointer_released(
                                viewer,
                                Point::new(x / scale, y / scale),
                                button,
                            ) {
                                eprintln!("editor gesture rejected: {error}");
                            }
                            self.sync_selection_handles();
                            self.update_title();
                        }
                    }
                }
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = KeyModifiers {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        if let Some(viewer) = &mut self.viewer {
                            self.domain
                                .cancel_tool(viewer)
                                .expect("gesture cancellation must succeed");
                        }
                        self.request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::Delete | KeyCode::Backspace) => {
                        self.delete_selection();
                    }
                    PhysicalKey::Code(KeyCode::KeyN)
                        if !self.modifiers.control && !self.modifiers.meta =>
                    {
                        self.create_node();
                    }
                    PhysicalKey::Code(KeyCode::KeyC)
                        if !self.modifiers.control && !self.modifiers.meta =>
                    {
                        self.activate_connection_creation();
                    }
                    PhysicalKey::Code(KeyCode::KeyZ)
                        if self.modifiers.control || self.modifiers.meta =>
                    {
                        if self.modifiers.shift {
                            self.redo();
                        } else {
                            self.undo();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::Focused(false) => {
                if let Some(viewer) = &mut self.viewer {
                    self.domain
                        .cancel_tool(viewer)
                        .expect("focus loss must cancel Tool feedback");
                    viewer.pointer_exited();
                    viewer.runtime_mut().cancel_gestures();
                    viewer.runtime_mut().release_focus();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(viewer) = &mut self.viewer {
            self.domain
                .cancel_tool(viewer)
                .expect("suspension must cancel Tool feedback");
            viewer.pointer_exited();
        }
        self.renderer = None;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("G5.4 manual validation:");
    println!("  drag a selected shape to move it");
    println!("  drag a yellow corner handle to resize the primary shape");
    println!("  press N to create a shape; Delete/Backspace removes selection");
    println!("  press C, then click a source shape and a target shape to connect");
    println!("  select a connection; drag cyan segment handles to create bendpoints");
    println!("  drag orange bendpoint handles to move or collapse them");
    println!("  Escape cancels the active connection gesture");
    println!("  Command/Control-Z undoes; add Shift to redo");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut DemoApp::new())?;
    Ok(())
}
