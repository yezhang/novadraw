use std::{
    collections::HashMap,
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};

use novadraw::backend::vello::VelloRenderer;
use novadraw::connection::{
    AnchorSemanticKey, Bendpoint, BendpointConnectionRouter, BendpointConstraint, ConnectionFigure,
    CoordinateSpace, XYAnchor, rectangle_boundary_site,
};
use novadraw::container::FreeformLayerFigure;
use novadraw::editor::{
    BendpointOperation, BendpointRequest, Command, CommandError, ConnectionAnchorContext,
    ConnectionAnchorDescriptor, ConnectionCreation, ConnectionEndpoint, ConnectionFeedbackRoute,
    ConnectionPartFactoryContext, ConnectionReconnection, ConnectionRouterKey,
    ConnectionRouterRegistration, ConnectionRoutingDescriptor, CreateConnectionRequest,
    EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorRequest, FeedbackVisual,
    GraphicalViewer, ModelAdapter, ModelConnection, ModelEvent, ModelRevision, PartFactoryContext,
    PolicyError, PolicyHost, PolicyInstallation, PolicyRole, ReconnectConnectionRequest,
    VisualUpdateContext,
};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::render::{RenderOutcome, SurfaceInfo};
use novadraw::{
    Color, Figure, PlatformHost, Point, PolylineFigure, Rectangle, RectangleFigure, RenderBackend,
};
use novadraw_apps::WinitPlatformHost;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

mod harness;
mod replay;

use harness::EditorHarness;

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
const SELF_LOOP_SOURCE_PORT_FRACTION: f64 = 0.25;
const SELF_LOOP_TARGET_PORT_FRACTION: f64 = 0.75;
const BENDPOINT_ROUTER_KEY: &str = "demo-bendpoint";
const AUTOEXPOSE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const WHEEL_SCROLL_STEP: f64 = 32.0;
const WHEEL_ZOOM_FACTOR: f64 = 1.1;
const VIEWPORT_BACKGROUND_COLOR: &str = "#F8FAFC";
const VIEWPORT_BORDER_COLOR: &str = "#64748B";
const VIEWPORT_BORDER_WIDTH: f64 = 2.0;

fn bendpoint_router_key() -> ConnectionRouterKey {
    ConnectionRouterKey::new(BENDPOINT_ROUTER_KEY).expect("static Router key is valid")
}

fn default_self_loop_bendpoints(bounds: Rectangle) -> [Point; 2] {
    let outer_x = bounds.x + bounds.width + SELF_LOOP_EXTENT;
    [
        Point::new(
            outer_x,
            bounds.y + bounds.height * SELF_LOOP_SOURCE_PORT_FRACTION,
        ),
        Point::new(
            outer_x,
            bounds.y + bounds.height * SELF_LOOP_TARGET_PORT_FRACTION,
        ),
    ]
}

fn self_loop_anchor_descriptor(
    model: &DemoModel,
    model_id: NodeId,
    context: ConnectionAnchorContext<NodeId>,
    fraction: f64,
    source_endpoint: bool,
    kind: &'static str,
) -> Option<ConnectionAnchorDescriptor> {
    if context.target_model() != Some(context.source_model()) || model_id != context.source_model()
    {
        return None;
    }
    let bounds = model.nodes.get(&model_id)?.bounds;
    let bendpoint_y = context
        .connection_model()
        .and_then(|connection| model.bendpoints.get(&connection))
        .and_then(|bendpoints| {
            if source_endpoint {
                bendpoints.first()
            } else {
                bendpoints.last()
            }
        })
        .map(|point| point.y());
    let point = Point::new(
        bounds.width,
        bendpoint_y.unwrap_or(bounds.y + bounds.height * fraction) - bounds.y,
    );
    let key = AnchorSemanticKey::new(
        Some(context.endpoint_figure()),
        kind,
        vec![point.x().to_bits(), point.y().to_bits()],
    )
    .expect("static self-loop Anchor kind is valid");
    Some(ConnectionAnchorDescriptor::new(
        key,
        Box::new(XYAnchor::new(
            point,
            CoordinateSpace::FigureLocal(context.endpoint_figure()),
        )),
    ))
}

const PRIMARY_HANDLE_COLOR: Color = Color::rgba(0.98, 0.78, 0.12, 1.0);
const BENDPOINT_HANDLE_COLOR: Color = Color::rgba(0.94, 0.42, 0.12, 1.0);
const BENDPOINT_CREATE_HANDLE_COLOR: Color = Color::rgba(0.1, 0.72, 0.72, 1.0);
const SECONDARY_HANDLE_COLOR: Color = Color::rgba(0.12, 0.78, 0.82, 1.0);
const FEEDBACK_COLOR: Color = Color::rgba(0.85, 0.36, 0.08, 1.0);

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
                        kind: NodeKind::Shape(
                            Color::from_hex("#2878D0").expect("valid color literal"),
                        ),
                    },
                ),
                (
                    NodeId(3),
                    Node {
                        bounds: Rectangle::new(330.0, 190.0, 200.0, 130.0),
                        kind: NodeKind::Shape(
                            Color::from_hex("#38A169").expect("valid color literal"),
                        ),
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
        let node = self
            .nodes
            .get_mut(&id)
            .expect("bounds command references a live node");
        let dx = bounds.x - node.bounds.x;
        let dy = bounds.y - node.bounds.y;
        node.bounds = bounds;
        if dx != 0.0 || dy != 0.0 {
            for connection in self
                .connections
                .iter()
                .filter(|connection| connection.source() == id && connection.target() == id)
            {
                if let Some(bendpoints) = self.bendpoints.get_mut(&connection.id()) {
                    for point in bendpoints {
                        *point = Point::new(point.x() + dx, point.y() + dy);
                    }
                }
            }
        }
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
        let bendpoints = self.bendpoints.remove(&connection.id()).unwrap_or_default();
        let bendpoints = self.normalized_connection_bendpoints(connection, bendpoints);
        self.connections.insert(index, connection);
        self.bendpoints.insert(connection.id(), bendpoints);
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

    fn replace_connection(&mut self, connection: ModelConnection<NodeId>, bendpoints: Vec<Point>) {
        let bendpoints = self.normalized_connection_bendpoints(connection, bendpoints);
        let slot = self
            .connections
            .iter_mut()
            .find(|candidate| candidate.id() == connection.id())
            .expect("reconnect command references a live connection");
        *slot = connection;
        self.bendpoints.insert(connection.id(), bendpoints);
        self.publish(connection.id());
    }

    fn set_bendpoints(&mut self, connection: NodeId, bendpoints: Vec<Point>) {
        self.bendpoints.insert(connection, bendpoints);
        self.publish(connection);
    }

    fn normalized_connection_bendpoints(
        &self,
        connection: ModelConnection<NodeId>,
        bendpoints: Vec<Point>,
    ) -> Vec<Point> {
        if connection.source() != connection.target() || bendpoints.len() >= 2 {
            return bendpoints;
        }
        let defaults = default_self_loop_bendpoints(self.nodes[&connection.source()].bounds);
        match bendpoints.as_slice() {
            [] => defaults.to_vec(),
            [point] if point.y() <= self.nodes[&connection.source()].bounds.center().y() => {
                vec![*point, defaults[1]]
            }
            [point] => vec![defaults[0], *point],
            _ => unreachable!("length checked above"),
        }
    }

    fn reconnected_connection_bendpoints(
        &self,
        before: ModelConnection<NodeId>,
        after: ModelConnection<NodeId>,
        bendpoints: Vec<Point>,
    ) -> Vec<Point> {
        let enters_self_loop = after.source() == after.target()
            && (before.source() != before.target() || before.source() != after.source());
        if enters_self_loop {
            default_self_loop_bendpoints(self.nodes[&after.source()].bounds).to_vec()
        } else {
            self.normalized_connection_bendpoints(after, bendpoints)
        }
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
    before_bendpoints: Vec<Point>,
    after_bendpoints: Vec<Point>,
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
        model.replace_connection(self.after, self.after_bendpoints.clone());
        Ok(())
    }

    fn undo(&mut self, model: &mut DemoModel) -> Result<(), CommandError> {
        model.replace_connection(self.before, self.before_bendpoints.clone());
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
                kind: NodeKind::Shape(Color::from_hex("#805AD5").expect("valid color literal")),
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
            NodeKind::Canvas => Box::new(FreeformLayerFigure::new(
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
                .with_stroke(
                    Color::from_hex("#17202A").expect("valid color literal"),
                    2.0,
                ),
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

    fn source_connection_anchor(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
        context: ConnectionAnchorContext<NodeId>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        Ok(self_loop_anchor_descriptor(
            model,
            model_id,
            context,
            SELF_LOOP_SOURCE_PORT_FRACTION,
            true,
            "demo-self-loop-source",
        ))
    }

    fn target_connection_anchor(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
        context: ConnectionAnchorContext<NodeId>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        Ok(self_loop_anchor_descriptor(
            model,
            model_id,
            context,
            SELF_LOOP_TARGET_PORT_FRACTION,
            false,
            "demo-self-loop-target",
        ))
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
        let stored_bendpoints = model
            .bendpoints
            .get(&connection.model())
            .cloned()
            .unwrap_or_default();
        let preview_connection = candidate.map(|candidate| match request.endpoint() {
            ConnectionEndpoint::Source => {
                ModelConnection::new(connection.model(), candidate.model(), fixed.model())
            }
            ConnectionEndpoint::Target => {
                ModelConnection::new(connection.model(), fixed.model(), candidate.model())
            }
        });
        let bendpoints = preview_connection.map_or_else(
            || stored_bendpoints.clone(),
            |after| {
                model.reconnected_connection_bendpoints(
                    self.before,
                    after,
                    stored_bendpoints.clone(),
                )
            },
        );
        let (source, target) = preview_connection
            .filter(|connection| connection.source() == connection.target())
            .and_then(|connection| {
                let bounds = model.nodes.get(&connection.source())?.bounds;
                Some((
                    Point::new(bounds.x + bounds.width, bendpoints.first()?.y()),
                    Point::new(bounds.x + bounds.width, bendpoints.last()?.y()),
                ))
            })
            .unwrap_or((route.source(), route.target()));
        let mut points = Vec::new();
        points.push(source);
        points.extend(bendpoints);
        points.push(target);
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
        model: &DemoModel,
    ) -> Result<Box<dyn Command<DemoModel>>, PolicyError> {
        let after = match request.endpoint() {
            ConnectionEndpoint::Source => {
                ModelConnection::new(self.before.id(), candidate.model(), fixed.model())
            }
            ConnectionEndpoint::Target => {
                ModelConnection::new(self.before.id(), fixed.model(), candidate.model())
            }
        };
        let before_bendpoints = model
            .bendpoints
            .get(&self.before.id())
            .cloned()
            .unwrap_or_default();
        let after_bendpoints =
            model.reconnected_connection_bendpoints(self.before, after, before_bendpoints.clone());
        Ok(Box::new(ReconnectCommand {
            before: self.before,
            after,
            before_bendpoints,
            after_bendpoints,
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
    apply_bendpoint_operation(
        model,
        connection_id,
        request.operation(),
        request.location(),
        before,
    )
}

fn apply_bendpoint_operation(
    model: &DemoModel,
    connection_id: NodeId,
    operation: BendpointOperation,
    location: Point,
    before: &[Point],
) -> Result<Vec<Point>, PolicyError> {
    let connection = model
        .connections
        .iter()
        .find(|connection| connection.id() == connection_id)
        .ok_or_else(|| PolicyError::operation("bendpoint connection no longer exists"))?;
    let is_self_loop = connection.source() == connection.target();
    let mut after = before.to_vec();
    match operation {
        BendpointOperation::Create { index } if index <= after.len() => {
            after.insert(index, location);
        }
        BendpointOperation::Move { index } if index < after.len() => {
            if is_self_loop && before.len() == 2 {
                let bounds = model.nodes[&connection.source()].bounds;
                let outer_x = location.x().max(bounds.x + bounds.width + SELF_LOOP_EXTENT);
                after[index] = Point::new(outer_x, location.y());
                after[1 - index] = Point::new(outer_x, before[1 - index].y());
                return Ok(after);
            }
            let previous = index
                .checked_sub(1)
                .and_then(|previous| before.get(previous).copied())
                .unwrap_or_else(|| model.nodes[&connection.source()].bounds.center());
            let next = before
                .get(index + 1)
                .copied()
                .unwrap_or_else(|| model.nodes[&connection.target()].bounds.center());
            if (!is_self_loop || before.len() > 2)
                && point_segment_distance(location, previous, next) <= BENDPOINT_DELETE_TOLERANCE
            {
                after.remove(index);
            } else {
                after[index] = location;
            }
        }
        BendpointOperation::Delete { index }
            if index < after.len() && (!is_self_loop || after.len() > 2) =>
        {
            after.remove(index);
        }
        BendpointOperation::Delete { index } if index < after.len() => {
            return Err(PolicyError::operation(
                "self-loop requires at least two bendpoints",
            ));
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
        Ok(Box::new(ConnectionFigure::new().with_stroke(
            Color::from_hex("#34495E").expect("valid color literal"),
            3.0,
        )))
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
        let connection = model
            .connections
            .iter()
            .find(|connection| connection.id() == model_id)
            .ok_or_else(|| EditPartError::operation("connection model no longer exists"))?;
        if connection.source() == connection.target() && bendpoints.len() < 2 {
            return Err(EditPartError::operation(
                "self-loop requires two explicit bendpoints",
            ));
        }
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
            Box::new(BendpointConnectionRouter),
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
    editor: Option<EditorHarness>,
    cursor: Option<(f64, f64)>,
    modifiers: KeyModifiers,
    last_autoexpose_step: Option<Instant>,
}

impl DemoApp {
    fn new() -> Self {
        Self {
            window: None,
            host: None,
            renderer: None,
            editor: None,
            cursor: None,
            modifiers: KeyModifiers::default(),
            last_autoexpose_step: None,
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
        if let Some(editor) = &mut self.editor {
            editor
                .resize_logical_viewport(surface.logical_width, surface.logical_height)
                .expect("window size must be valid");
        }
        self.request_redraw();
    }

    fn render(&mut self) {
        let (Some(editor), Some(renderer), Some(host)) =
            (&mut self.editor, &mut self.renderer, &self.host)
        else {
            return;
        };
        let Some(submission) = editor
            .runtime_mut()
            .prepare_submission(host.surface_info(), renderer.capabilities())
        else {
            return;
        };
        let outcome = renderer.submit(&submission);
        editor.runtime_mut().complete_submission(
            submission.session_id,
            submission.frame_id,
            outcome,
        );
        if outcome == RenderOutcome::Retry || editor.runtime().has_pending_update() {
            host.request_redraw();
        }
    }

    fn update_title(&self) {
        let (Some(window), Some(editor)) = (&self.window, &self.editor) else {
            return;
        };
        window.set_title(&format!("Novadraw Node Editor - {}", editor.title_status()));
    }

    fn activate_connection_creation(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor
            .activate_connection_creation()
            .expect("connection creation Tool must activate");
        self.update_title();
        self.request_redraw();
    }

    fn create_node(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.create_node().expect("create request must execute");
        self.update_title();
        self.request_redraw();
    }

    fn delete_selection(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if let Err(error) = editor.delete_selection() {
            eprintln!("delete request rejected: {error}");
            return;
        }
        self.update_title();
        self.request_redraw();
    }

    fn undo(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.undo().expect("undo must execute");
        self.update_title();
        self.request_redraw();
    }

    fn redo(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.redo().expect("redo must execute");
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
            SurfaceInfo {
                logical_width: f64::from(size.width) / scale,
                logical_height: f64::from(size.height) / scale,
                pixel_width: size.width,
                pixel_height: size.height,
                scale_factor: scale,
            },
        ));
        self.host = Some(WinitPlatformHost::new(Arc::clone(&window)));
        self.window = Some(window);

        if let Some(editor) = &mut self.editor {
            editor
                .runtime_mut()
                .reset_backend_session()
                .expect("backend session reset failed");
        } else {
            self.editor = Some(EditorHarness::new().expect("demo editor construction failed"));
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
                if let Some(editor) = &mut self.editor {
                    editor
                        .pointer_moved(Point::new(position.x / scale, position.y / scale))
                        .expect("pointer move must update the active Tool");
                    self.last_autoexpose_step = editor.autoexpose_requested().then(Instant::now);
                }
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                self.last_autoexpose_step = None;
                if let Some(editor) = &mut self.editor
                    && let Err(error) = editor.pointer_exited()
                {
                    eprintln!("pointer exit cancellation rejected: {error}");
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
                if let Some(editor) = &mut self.editor {
                    match state {
                        ElementState::Pressed => {
                            let changed = match editor.pointer_pressed(
                                Point::new(x / scale, y / scale),
                                button,
                                self.modifiers,
                            ) {
                                Ok(changed) => changed,
                                Err(error) => {
                                    eprintln!("editor gesture rejected: {error}");
                                    false
                                }
                            };
                            if changed {
                                self.update_title();
                            }
                        }
                        ElementState::Released => {
                            self.last_autoexpose_step = None;
                            if let Err(error) =
                                editor.pointer_released(Point::new(x / scale, y / scale), button)
                            {
                                eprintln!("editor gesture rejected: {error}");
                            }
                            self.update_title();
                        }
                    }
                }
                self.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        f64::from(x) * WHEEL_SCROLL_STEP,
                        f64::from(y) * WHEEL_SCROLL_STEP,
                    ),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x / scale, position.y / scale)
                    }
                };
                if let Some(editor) = &mut self.editor {
                    let result = if self.modifiers.control || self.modifiers.meta {
                        if dy == 0.0 {
                            Ok(false)
                        } else {
                            let factor = if dy > 0.0 {
                                WHEEL_ZOOM_FACTOR
                            } else {
                                WHEEL_ZOOM_FACTOR.recip()
                            };
                            let anchor = self
                                .cursor
                                .map(|(x, y)| Point::new(x / scale, y / scale))
                                .unwrap_or_else(|| Point::new(WIDTH / 2.0, HEIGHT / 2.0));
                            editor.zoom_by(factor, anchor)
                        }
                    } else {
                        editor.scroll_by(-dx, -dy)
                    };
                    if let Err(error) = result {
                        eprintln!("viewport update rejected: {error}");
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
                        self.last_autoexpose_step = None;
                        if let Some(editor) = &mut self.editor {
                            editor
                                .cancel_tool()
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
                self.last_autoexpose_step = None;
                if let Some(editor) = &mut self.editor {
                    editor
                        .focus_lost()
                        .expect("focus loss must cancel Tool state");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(editor) = &mut self.editor else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        if !editor.autoexpose_requested() {
            self.last_autoexpose_step = None;
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let now = Instant::now();
        let elapsed = self
            .last_autoexpose_step
            .replace(now)
            .map_or(AUTOEXPOSE_FRAME_INTERVAL, |previous| {
                now.saturating_duration_since(previous)
            });
        match editor.autoexpose_tick(elapsed) {
            Ok(outcome) => {
                if outcome.scrolled() {
                    self.request_redraw();
                }
                if outcome.continue_requested() {
                    event_loop
                        .set_control_flow(ControlFlow::WaitUntil(now + AUTOEXPOSE_FRAME_INTERVAL));
                } else {
                    self.last_autoexpose_step = None;
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }
            Err(error) => {
                eprintln!("auto-expose rejected: {error}");
                self.last_autoexpose_step = None;
                editor
                    .cancel_tool()
                    .expect("failed auto-expose must cancel Tool state");
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(editor) = &mut self.editor {
            editor
                .cancel_tool()
                .expect("suspension must cancel Tool feedback");
            editor
                .pointer_exited()
                .expect("suspension pointer exit must clear Tool feedback");
        }
        self.renderer = None;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(options) = replay::ReplayOptions::parse()? {
        replay::run(options)?;
        return Ok(());
    }

    println!("G5.5 manual validation:");
    println!("  drag a selected shape to move it");
    println!("  drag a yellow corner handle to resize the primary shape");
    println!("  press N to create a shape; Delete/Backspace removes selection");
    println!("  press C, then click a source shape and a target shape to connect");
    println!("  select a connection; drag cyan segment handles to create bendpoints");
    println!("  drag orange bendpoint handles to move or collapse them");
    println!("  wheel scrolls; Command/Control-wheel zooms around the pointer");
    println!("  keep dragging near an edge to auto-scroll without losing feedback");
    println!("  Escape cancels the active connection gesture");
    println!("  Command/Control-Z undoes; add Shift to redo");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut DemoApp::new())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::editor::HandleRole;
    use novadraw::graphics::LineStyle;
    use novadraw::render::command::RenderCommandKind;

    #[test]
    fn viewport_guide_uses_light_background_and_dashed_outline() {
        let mut harness = EditorHarness::new().unwrap();
        let frame = harness.runtime_mut().prepare_frame().unwrap();

        assert!(frame.commands().iter().any(|command| matches!(
            command.kind,
            RenderCommandKind::FillRect { color, .. }
                if color == Color::from_hex(VIEWPORT_BACKGROUND_COLOR).expect("valid color literal")
        )));
        let initial_outline = frame
            .commands()
            .iter()
            .find_map(|command| match command.kind {
                RenderCommandKind::StrokeRect {
                    rect,
                    color,
                    width,
                    line_style: LineStyle::Dash,
                    ..
                } if color
                    == Color::from_hex(VIEWPORT_BORDER_COLOR).expect("valid color literal")
                    && width == VIEWPORT_BORDER_WIDTH =>
                {
                    Some(rect)
                }
                _ => None,
            })
            .expect("viewport outline must be rendered");

        assert!(harness.resize_logical_viewport(1_200.0, 800.0).unwrap());
        let resized = harness.runtime_mut().prepare_frame().unwrap();
        let resized_outline = resized
            .commands()
            .iter()
            .find_map(|command| match command.kind {
                RenderCommandKind::StrokeRect {
                    rect,
                    color,
                    width,
                    line_style: LineStyle::Dash,
                    ..
                } if color
                    == Color::from_hex(VIEWPORT_BORDER_COLOR).expect("valid color literal")
                    && width == VIEWPORT_BORDER_WIDTH =>
                {
                    Some(rect)
                }
                _ => None,
            })
            .expect("resized viewport outline must be rendered");
        assert!(resized_outline.width > initial_outline.width);
        assert!(resized_outline.height > initial_outline.height);
    }

    #[test]
    fn resized_viewport_does_not_retain_the_initial_canvas_clip() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame().unwrap();
        assert!(harness.resize_logical_viewport(1_200.0, 800.0).unwrap());

        let start = harness.node_bounds_in_surface(3).unwrap().center();
        let end = Point::new(1_020.0, 690.0);
        assert!(harness.drag(start, end, KeyModifiers::default()).unwrap());
        let moved = harness.node_bounds_in_surface(3).unwrap();
        assert!(moved.x > WIDTH);
        assert!(moved.y > HEIGHT);
        assert!(moved.x + moved.width < 1_200.0);
        assert!(moved.y + moved.height < 800.0);

        let frame = harness.runtime_mut().prepare_frame().unwrap();
        assert!(
            frame.commands().iter().all(|command| {
                !matches!(
                    command.kind,
                    RenderCommandKind::Clip { rect }
                        if rect.width == WIDTH && rect.height == HEIGHT
                )
            }),
            "the initial canvas bounds must not clip content inside the resized viewport"
        );
    }

    #[test]
    fn self_loop_owns_two_bendpoints_and_moving_one_preserves_the_other() {
        let mut model = DemoModel::new();
        let connection = ModelConnection::new(NodeId(1000), NodeId(2), NodeId(2));
        model.insert_connection(connection, 0);
        let before = model.bendpoints[&connection.id()].clone();
        assert_eq!(before.len(), 2);

        let after = apply_bendpoint_operation(
            &model,
            connection.id(),
            BendpointOperation::Move { index: 0 },
            Point::new(before[0].x() + 20.0, before[0].y() - 10.0),
            &before,
        )
        .unwrap();
        assert_eq!(after.len(), 2);
        assert_ne!(after[0], before[0]);
        assert_eq!(after[1].x(), after[0].x());
        assert_eq!(after[1].y(), before[1].y());

        model.set_bendpoints(connection.id(), after);
        let before_move = model.bendpoints[&connection.id()].clone();
        let old_bounds = model.nodes[&connection.source()].bounds;
        model.set_bounds(
            connection.source(),
            Rectangle::new(
                old_bounds.x + 15.0,
                old_bounds.y + 25.0,
                old_bounds.width,
                old_bounds.height,
            ),
        );
        assert_eq!(
            model.bendpoints[&connection.id()],
            before_move
                .iter()
                .map(|point| Point::new(point.x() + 15.0, point.y() + 25.0))
                .collect::<Vec<_>>()
        );

        let mut viewer =
            GraphicalViewer::new(model, DemoFactory, Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))
                .unwrap();
        let part = viewer.connection_part_for_model(connection.id()).unwrap();
        let route = viewer.connection_route_points_in_surface(part).unwrap();
        assert_eq!(route.len(), 4);
        assert!(route.windows(2).all(|segment| {
            (segment[0].x() - segment[1].x()).abs() <= f64::EPSILON
                || (segment[0].y() - segment[1].y()).abs() <= f64::EPSILON
        }));
        let move_roles = viewer
            .connection_bendpoint_handle_sites(part)
            .unwrap()
            .into_iter()
            .filter_map(|site| match site.role() {
                HandleRole::BendpointMove(index) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(move_roles, vec![0, 1]);
    }

    #[test]
    fn reconnecting_both_self_loop_endpoints_rebases_bendpoints_to_the_new_owner() {
        let mut model = DemoModel::new();
        let blue_loop = ModelConnection::new(NodeId(1000), NodeId(2), NodeId(2));
        model.insert_connection(blue_loop, 0);
        let blue_bendpoints = model.bendpoints[&blue_loop.id()].clone();

        let blue_to_green = ModelConnection::new(blue_loop.id(), NodeId(2), NodeId(3));
        let crossing_bendpoints = model.reconnected_connection_bendpoints(
            blue_loop,
            blue_to_green,
            blue_bendpoints.clone(),
        );
        assert_eq!(crossing_bendpoints, blue_bendpoints);

        let green_loop = ModelConnection::new(blue_loop.id(), NodeId(3), NodeId(3));
        let green_bendpoints =
            model.reconnected_connection_bendpoints(blue_to_green, green_loop, crossing_bendpoints);
        assert_eq!(
            green_bendpoints,
            default_self_loop_bendpoints(model.nodes[&NodeId(3)].bounds)
        );

        model.replace_connection(green_loop, green_bendpoints);
        let viewer =
            GraphicalViewer::new(model, DemoFactory, Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))
                .unwrap();
        let connection = viewer.connection_part_for_model(green_loop.id()).unwrap();
        let route = viewer
            .connection_route_points_in_surface(connection)
            .unwrap();
        let green_bounds = viewer.model().nodes[&NodeId(3)].bounds;
        assert_eq!(route.len(), 4);
        assert!(route.iter().all(|point| point.x() >= green_bounds.x));
        assert!(route.windows(2).all(|segment| {
            (segment[0].x() - segment[1].x()).abs() <= f64::EPSILON
                || (segment[0].y() - segment[1].y()).abs() <= f64::EPSILON
        }));
    }

    fn assert_selection_handles_align_with_node(harness: &EditorHarness, node: u64) {
        let bounds = harness.node_bounds_in_surface(node).unwrap();
        let expected = [
            Point::new(bounds.x, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y),
            Point::new(bounds.x, bounds.y + bounds.height),
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
        ];
        let centers = harness.handle_centers_in_surface();
        assert_eq!(centers.len(), expected.len());
        assert!(expected.iter().all(|point| {
            centers
                .iter()
                .any(|center| (*center - *point).length() <= f64::EPSILON)
        }));
    }

    fn assert_connection_endpoint_handles_align(harness: &EditorHarness, connection: u64) {
        let route = harness.connection_route(connection).unwrap();
        let endpoints = [*route.first().unwrap(), *route.last().unwrap()];
        let centers = harness.handle_centers_in_surface();
        assert!(
            endpoints.iter().all(|point| {
                centers
                    .iter()
                    .any(|center| (*center - *point).length() <= f64::EPSILON)
            }),
            "connection endpoints {endpoints:?} must match handle centers {centers:?}"
        );
    }

    #[test]
    fn zoom_keeps_unscaled_selection_handles_aligned_after_frame_stabilization() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let center = harness.node_bounds(3).unwrap().center();
        harness.click(center, KeyModifiers::default()).unwrap();

        for _ in 0..6 {
            assert!(harness.zoom_by(1.1, center).unwrap());
            harness.runtime_mut().prepare_frame();
            assert_selection_handles_align_with_node(&harness, 3);
        }
    }

    #[test]
    fn autoexpose_and_pointer_return_keep_selection_handles_aligned() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let center = harness.node_bounds(3).unwrap().center();
        harness.click(center, KeyModifiers::default()).unwrap();
        let start = harness.node_bounds_in_surface(3).unwrap().center();
        harness
            .pointer_pressed(start, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(Point::new(815.0, 399.0)).unwrap();
        let mut scrolled = false;
        for _ in 0..32 {
            let tick = harness.autoexpose_tick(Duration::from_millis(30)).unwrap();
            scrolled |= tick.scrolled();
            harness.runtime_mut().prepare_frame();
            assert_selection_handles_align_with_node(&harness, 3);
            if !tick.continue_requested() {
                break;
            }
        }
        assert!(scrolled);
        let edge_origin = harness.viewport_origin().unwrap();
        assert!(edge_origin.x() > 50.0);

        harness
            .pointer_moved(Point::new(732.640625, 399.12890625))
            .unwrap();
        harness.runtime_mut().prepare_frame();

        assert_selection_handles_align_with_node(&harness, 3);
        let returned_origin = harness.viewport_origin().unwrap();
        assert!(returned_origin.x() <= edge_origin.x());
        assert!(returned_origin.y() <= edge_origin.y());
    }

    #[test]
    fn viewport_resize_reprojects_unscaled_selection_handles_after_origin_clamp() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let model_bounds = harness.node_bounds(3).unwrap();
        harness
            .click(model_bounds.center(), KeyModifiers::default())
            .unwrap();
        harness.zoom_by(2.0, Point::new(0.0, 0.0)).unwrap();
        harness.scroll_by(10_000.0, 10_000.0).unwrap();
        harness.runtime_mut().prepare_frame();

        assert!(harness.resize_logical_viewport(1_200.0, 800.0).unwrap());

        assert_selection_handles_align_with_node(&harness, 3);
    }

    #[test]
    fn pointer_leave_cancels_autoexpose_and_clears_the_active_gesture() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let center = harness.node_bounds(2).unwrap().center();
        harness.click(center, KeyModifiers::default()).unwrap();
        let start = harness.node_bounds_in_surface(2).unwrap().center();
        harness
            .pointer_pressed(start, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(Point::new(815.0, 555.0)).unwrap();
        assert!(harness.has_active_gesture());
        assert!(harness.autoexpose_requested());
        let mut scrolled = false;
        for _ in 0..12 {
            let tick = harness.autoexpose_tick(Duration::from_millis(30)).unwrap();
            scrolled |= tick.scrolled();
            if !tick.continue_requested() {
                break;
            }
        }
        assert!(scrolled);
        let edge_origin = harness.viewport_origin().unwrap();

        harness.pointer_exited().unwrap();
        harness.runtime_mut().prepare_frame().unwrap();

        assert!(!harness.has_active_gesture());
        assert!(!harness.autoexpose_requested());
        assert!(
            harness.viewport_origin().unwrap().x() < edge_origin.x(),
            "removing transient feedback must clamp the temporary auto-expose range"
        );
        assert_selection_handles_align_with_node(&harness, 2);
    }

    #[test]
    fn autoexpose_release_keeps_viewport_on_the_committed_target() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let center = harness.node_bounds(2).unwrap().center();
        harness.click(center, KeyModifiers::default()).unwrap();
        let start = harness.node_bounds_in_surface(2).unwrap().center();
        let edge = Point::new(815.0, 555.0);
        harness
            .pointer_pressed(start, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(edge).unwrap();
        for _ in 0..12 {
            let tick = harness.autoexpose_tick(Duration::from_millis(30)).unwrap();
            if !tick.continue_requested() {
                break;
            }
        }
        let edge_origin = harness.viewport_origin().unwrap();
        assert!(edge_origin.x() > 0.0);
        assert!(edge_origin.y() > 0.0);

        assert!(harness.pointer_released(edge, MouseButton::Left).unwrap());
        harness.runtime_mut().prepare_frame().unwrap();

        let committed_origin = harness.viewport_origin().unwrap();
        assert!(committed_origin.x() > 0.0);
        assert!(committed_origin.y() > 0.0);
        assert_selection_handles_align_with_node(&harness, 2);
    }

    #[test]
    fn dragging_committed_target_back_preserves_the_feedback_surface_position() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame();
        let center = harness.node_bounds(2).unwrap().center();
        harness.click(center, KeyModifiers::default()).unwrap();
        let start = harness.node_bounds_in_surface(2).unwrap().center();
        let edge = Point::new(815.0, 555.0);
        harness
            .pointer_pressed(start, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(edge).unwrap();
        for _ in 0..12 {
            let tick = harness.autoexpose_tick(Duration::from_millis(30)).unwrap();
            if !tick.continue_requested() {
                break;
            }
        }
        assert!(harness.pointer_released(edge, MouseButton::Left).unwrap());
        harness.runtime_mut().prepare_frame().unwrap();
        assert!(harness.viewport_origin().unwrap().x() > 0.0);

        let start = harness.node_bounds_in_surface(2).unwrap().center();
        let target = Point::new(300.0, 140.0);
        harness
            .pointer_pressed(start, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(target).unwrap();
        assert!(harness.pointer_released(target, MouseButton::Left).unwrap());
        harness.runtime_mut().prepare_frame().unwrap();

        let committed = harness.node_bounds_in_surface(2).unwrap().center();
        assert!(
            (committed - target).length() <= f64::EPSILON,
            "committed node {committed:?} must remain at feedback target {target:?}; origin={:?}",
            harness.viewport_origin().unwrap()
        );
        assert_selection_handles_align_with_node(&harness, 2);
    }

    #[test]
    fn reconnect_release_reprojects_handles_after_temporary_range_clamps() {
        let mut harness = EditorHarness::new().unwrap();
        harness.runtime_mut().prepare_frame().unwrap();
        harness.activate_connection_creation().unwrap();
        let source = harness.node_bounds_in_surface(2).unwrap().center();
        let target = harness.node_bounds_in_surface(3).unwrap().center();
        harness.click(source, KeyModifiers::default()).unwrap();
        harness.click(target, KeyModifiers::default()).unwrap();
        let connection = harness.connection_ids()[0];
        let route = harness.connection_route(connection).unwrap();
        let first = *route.first().unwrap();
        let last = *route.last().unwrap();
        harness
            .click(
                Point::new((first.x() + last.x()) / 2.0, (first.y() + last.y()) / 2.0),
                KeyModifiers::default(),
            )
            .unwrap();
        assert_eq!(harness.selected_model_ids(), vec![connection]);
        assert_connection_endpoint_handles_align(&harness, connection);

        let endpoint = *harness
            .connection_route(connection)
            .unwrap()
            .last()
            .unwrap();
        let edge = Point::new(815.0, 555.0);
        harness
            .pointer_pressed(endpoint, MouseButton::Left, KeyModifiers::default())
            .unwrap();
        harness.pointer_moved(edge).unwrap();
        for _ in 0..6 {
            assert!(
                harness
                    .autoexpose_tick(Duration::from_millis(30))
                    .unwrap()
                    .scrolled()
            );
        }
        let scrolled_origin = harness.viewport_origin().unwrap();
        assert!(scrolled_origin.x() > 0.0);
        assert!(scrolled_origin.y() > 0.0);

        assert!(!harness.pointer_released(edge, MouseButton::Left).unwrap());
        harness.runtime_mut().prepare_frame().unwrap();

        assert!(harness.viewport_origin().unwrap().x() < scrolled_origin.x());
        assert!(harness.viewport_origin().unwrap().y() < scrolled_origin.y());
        assert_connection_endpoint_handles_align(&harness, connection);
    }
}
