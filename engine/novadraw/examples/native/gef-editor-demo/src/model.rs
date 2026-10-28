use novadraw::connection::{
    AnchorSemanticKey, ChopboxAnchor, ConnectionFigure, ConnectionLocator, EllipseAnchor,
    PolygonDecorationFigure,
};
use novadraw::container::FreeformLayerFigure;
use novadraw::figure::{EllipseFigure, Figure, RectangleFigure};
use novadraw::geometry::Dimension;
use novadraw::geometry::Rectangle;
use novadraw::graphics::Color;
use novadraw::graphics::{DashPattern, StrokeStyle};
use novadraw_editor::tool::CreationTemplate;
use novadraw_editor::{
    ChangeBounds, ChangeBoundsRequest, Command, CommandError, ComponentSlot, CreateChild,
    CreateRequest, DeleteParts, DeleteRequest, EditPartBehavior, EditPartError, EditPartFactory,
    EditPolicy, EditingContribution, FeedbackVisual, LayoutSlot, ModelAdapter, ModelEvent,
    ModelRevision, PartFactoryContext, PolicyError, PolicyHandlerBuilder, PolicyHost,
    PolicyInstallation, RequestMetadata, VisualUpdateContext,
};
use novadraw_editor::{
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionCreation,
    ConnectionCreationSlot, ConnectionEndpoint, ConnectionFeedbackRoute,
    ConnectionPartFactoryContext, ConnectionReconnectSlot, ConnectionReconnection,
    CreateConnection, CreateConnectionRequest, CreationType, EditPartId, InteractionRevision,
    ModelConnection, ReconnectConnection, ReconnectConnectionRequest, RequestModifiers,
    RequestProtocol, VisualBuildContext,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, convert::Infallible};

pub const ROOT: u64 = 0;
pub const FIRST: u64 = 1;
pub const INITIAL_BOUNDS: Rectangle = Rectangle {
    x: 60.0,
    y: 70.0,
    width: 120.0,
    height: 80.0,
};
pub const MIN_SIZE: f64 = 24.0;
const OUTLINE_WIDTH: f64 = 2.0;
pub const DEFAULT_WIDTH: f64 = 120.0;
pub const DEFAULT_HEIGHT: f64 = 80.0;
const ARROW_LENGTH: f64 = 10.0;
const ARROW_HALF_WIDTH: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl ShapeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub bounds: Rectangle,
    pub kind: ShapeKind,
}

pub struct Diagram {
    pub shapes: BTreeMap<u64, Shape>,
    pub order: Vec<u64>,
    pub edges: BTreeMap<u64, Edge>,
    revision: ModelRevision,
    events: Vec<ModelEvent<u64, ()>>,
    next_id: u64,
}

impl Default for Diagram {
    fn default() -> Self {
        Self {
            shapes: BTreeMap::from([
                (
                    FIRST,
                    Shape {
                        bounds: INITIAL_BOUNDS,
                        kind: ShapeKind::Rectangle,
                    },
                ),
                (
                    2,
                    Shape {
                        bounds: Rectangle::new(270.0, 160.0, 140.0, 100.0),
                        kind: ShapeKind::Ellipse,
                    },
                ),
            ]),
            order: vec![FIRST, 2],
            edges: BTreeMap::new(),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id: 3,
        }
    }
}

impl Diagram {
    fn publish(&mut self, subject: u64) {
        self.revision = self.revision.next().expect("demo model revision exhausted");
        self.events
            .push(ModelEvent::new(self.revision, subject, ()));
    }

    pub fn accepts_edge(&self, source: u64, target: u64, excluding: Option<u64>) -> bool {
        source != target
            && self.shapes.contains_key(&source)
            && self.shapes.contains_key(&target)
            && !self
                .edges
                .iter()
                .any(|(id, e)| Some(*id) != excluding && e.source == source && e.target == target)
    }
}

impl ModelAdapter for Diagram {
    type ModelId = u64;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u64 {
        ROOT
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: u64) -> Result<Vec<u64>, Infallible> {
        Ok(if id == ROOT {
            self.order.clone()
        } else {
            Vec::new()
        })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
    fn connections(&self) -> Result<Vec<ModelConnection<u64>>, Infallible> {
        Ok(self
            .edges
            .iter()
            .map(|(id, edge)| ModelConnection::new(*id, edge.source, edge.target))
            .collect())
    }
}

pub struct MoveShape {
    pub id: u64,
    pub before: Rectangle,
    pub after: Rectangle,
}
impl Command<Diagram> for MoveShape {
    fn label(&self) -> &str {
        "Move / resize shape"
    }
    fn can_execute(&self, model: &Diagram) -> bool {
        model.shapes.contains_key(&self.id) && valid_bounds(self.after)
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.get_mut(&self.id).unwrap().bounds = self.after;
        model.publish(self.id);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.get_mut(&self.id).unwrap().bounds = self.before;
        model.publish(self.id);
        Ok(())
    }
}

struct InsertShape {
    id: u64,
    shape: Shape,
}
impl Command<Diagram> for InsertShape {
    fn label(&self) -> &str {
        "Create shape"
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.insert(self.id, self.shape);
        model.order.push(self.id);
        model.next_id = model.next_id.max(self.id + 1);
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.remove(&self.id);
        model.order.retain(|id| *id != self.id);
        model.publish(ROOT);
        Ok(())
    }
}

struct RemoveShape {
    id: u64,
    shape: Shape,
    index: usize,
    edges: BTreeMap<u64, Edge>,
}
impl Command<Diagram> for RemoveShape {
    fn label(&self) -> &str {
        "Delete shape"
    }
    fn execute(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        self.index = model
            .order
            .iter()
            .position(|id| *id == self.id)
            .ok_or_else(|| CommandError::operation("shape no longer exists"))?;
        self.edges = model
            .edges
            .iter()
            .filter(|(_, e)| e.source == self.id || e.target == self.id)
            .map(|(id, e)| (*id, *e))
            .collect();
        model.edges.retain(|id, _| !self.edges.contains_key(id));
        model.shapes.remove(&self.id);
        model.order.remove(self.index);
        model.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, model: &mut Diagram) -> Result<(), CommandError> {
        model.shapes.insert(self.id, self.shape);
        model.order.insert(self.index, self.id);
        model.edges.extend(self.edges.clone());
        model.publish(ROOT);
        Ok(())
    }
}

fn shape_figure(shape: Shape) -> Box<dyn Figure> {
    let fill = Color::from_hex(match shape.kind {
        ShapeKind::Rectangle => "#4b91c8",
        ShapeKind::Ellipse => "#e5ad52",
    })
    .unwrap();
    let stroke = Color::from_hex("#253448").unwrap();
    let b = shape.bounds;
    match shape.kind {
        ShapeKind::Rectangle => Box::new(
            RectangleFigure::new_with_color(b.x, b.y, b.width, b.height, fill)
                .with_stroke(stroke, OUTLINE_WIDTH),
        ),
        ShapeKind::Ellipse => Box::new(
            EllipseFigure::new_with_color(b.x, b.y, b.width, b.height, fill)
                .with_stroke(stroke, OUTLINE_WIDTH),
        ),
    }
}

struct ShapePolicy;
impl EditPolicy<Diagram> for ShapePolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        builder.handle::<ChangeBounds>(Self::change);
        builder.handle::<DeleteParts>(Self::delete);
    }
}
impl ShapePolicy {
    fn change(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &ChangeBoundsRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        let shape = model.shapes[&host.model()];
        let after = req.transformed_bounds(shape.bounds);
        Ok(Some(EditingContribution::new(
            Some(Box::new(MoveShape {
                id: host.model(),
                before: shape.bounds,
                after,
            })),
            vec![FeedbackVisual::scaled(shape_figure(Shape {
                bounds: after,
                ..shape
            }))],
        )))
    }
    fn delete(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(RemoveShape {
            id: host.model(),
            shape: model.shapes[&host.model()],
            index: 0,
            edges: BTreeMap::new(),
        }))))
    }
}

struct CanvasPolicy;
impl EditPolicy<Diagram> for CanvasPolicy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        builder.handle::<CreateChild>(Self::create);
    }
}
impl CanvasPolicy {
    fn create(
        &mut self,
        _: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &CreateRequest,
        model: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        let kind = match req.creation_type().as_str() {
            "rectangle" => ShapeKind::Rectangle,
            "ellipse" => ShapeKind::Ellipse,
            _ => return Err(PolicyError::operation("unsupported shape")),
        };
        if !valid_bounds(req.bounds()) || model.next_id == u64::MAX {
            return Ok(None);
        }
        let shape = Shape {
            bounds: req.bounds(),
            kind,
        };
        Ok(Some(EditingContribution::new(
            Some(Box::new(InsertShape {
                id: model.next_id,
                shape,
            })),
            vec![FeedbackVisual::scaled(shape_figure(shape))],
        )))
    }
}

pub struct ShapePart;
impl EditPartBehavior<Diagram> for ShapePart {
    fn create_figure(
        &mut self,
        model: &Diagram,
        id: u64,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == ROOT {
            Box::new(FreeformLayerFigure::new(0.0, 0.0, 640.0, 480.0))
        } else {
            shape_figure(model.shapes[&id])
        })
    }
    fn create_policies(
        &mut self,
        _: &Diagram,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Diagram>>, EditPartError> {
        let policy = if id == ROOT {
            PolicyInstallation::new(LayoutSlot::key(), CanvasPolicy)
        } else {
            PolicyInstallation::new(ComponentSlot::key(), ShapePolicy)
        };
        let mut policies =
            vec![policy.map_err(|error| EditPartError::operation(error.to_string()))?];
        if id != ROOT {
            policies.push(
                PolicyInstallation::new(ConnectionCreationSlot::key(), NodeConnectionPolicy)
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            );
        }
        Ok(policies)
    }
    fn refresh_visuals(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        if id != ROOT {
            ctx.set_primary_bounds(model.shapes[&id].bounds)?;
        }
        Ok(())
    }
    fn source_connection_anchor(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: ConnectionAnchorContext<u64>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        anchor(model, id, ctx)
    }
    fn target_connection_anchor(
        &mut self,
        model: &Diagram,
        id: u64,
        ctx: ConnectionAnchorContext<u64>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        anchor(model, id, ctx)
    }
}

pub struct ShapeFactory;
impl EditPartFactory<Diagram> for ShapeFactory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Diagram,
    ) -> Result<Box<dyn EditPartBehavior<Diagram>>, EditPartError> {
        Ok(Box::new(ShapePart))
    }
    fn create_connection(
        &mut self,
        _: ConnectionPartFactoryContext<u64>,
        _: &Diagram,
    ) -> Result<Box<dyn EditPartBehavior<Diagram>>, EditPartError> {
        Ok(Box::new(EdgePart))
    }
}

fn valid_bounds(b: Rectangle) -> bool {
    [b.x, b.y, b.width, b.height]
        .into_iter()
        .all(f64::is_finite)
        && b.width >= MIN_SIZE
        && b.height >= MIN_SIZE
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeStyle {
    Solid,
    Dashed,
}
impl EdgeStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(Self::Solid),
            "dashed" => Some(Self::Dashed),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: u64,
    pub target: u64,
    pub style: EdgeStyle,
}

#[derive(Clone, Copy)]
pub struct ShapeTemplate(pub ShapeKind);
impl CreationTemplate<Diagram> for ShapeTemplate {
    type Request = CreateChild;
    fn default_size(&self) -> Dimension {
        Dimension::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
    fn request(
        &self,
        contents: EditPartId,
        bounds: Rectangle,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> (RequestMetadata, CreateRequest) {
        (
            RequestMetadata::new(revision, vec![contents]),
            CreateRequest::new(
                contents,
                CreationType::new(self.0.name()).unwrap(),
                bounds,
                modifiers,
                revision,
            ),
        )
    }
}

fn anchor(
    model: &Diagram,
    id: u64,
    ctx: ConnectionAnchorContext<u64>,
) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
    let Some(shape) = model.shapes.get(&id) else {
        return Ok(None);
    };
    let owner = ctx.endpoint_figure();
    let (name, anchor): (_, Box<dyn novadraw::connection::ConnectionAnchor>) = match shape.kind {
        ShapeKind::Rectangle => ("chopbox", Box::new(ChopboxAnchor::new(owner))),
        ShapeKind::Ellipse => ("ellipse", Box::new(EllipseAnchor::new(owner))),
    };
    let key = AnchorSemanticKey::new(Some(owner), name, vec![])
        .map_err(|e| EditPartError::operation(e.to_string()))?;
    Ok(Some(ConnectionAnchorDescriptor::new(key, anchor)))
}

struct InsertEdge {
    id: u64,
    edge: Edge,
}
impl Command<Diagram> for InsertEdge {
    fn label(&self) -> &str {
        "Create connection"
    }
    fn can_execute(&self, m: &Diagram) -> bool {
        !m.edges.contains_key(&self.id)
            && self.id != u64::MAX
            && m.accepts_edge(self.edge.source, self.edge.target, None)
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.edge);
        m.next_id = m.next_id.max(self.id + 1);
        m.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.remove(&self.id);
        m.publish(ROOT);
        Ok(())
    }
}

struct DeleteEdge {
    id: u64,
    removed: Option<Edge>,
}
impl Command<Diagram> for DeleteEdge {
    fn label(&self) -> &str {
        "Delete connection"
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        // An earlier node deletion in the same compound may have removed this edge.
        self.removed = m.edges.remove(&self.id);
        m.publish(ROOT);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        if let Some(edge) = self.removed {
            m.edges.insert(self.id, edge);
        }
        m.publish(ROOT);
        Ok(())
    }
}

struct ReconnectEdge {
    id: u64,
    before: Edge,
    after: Edge,
}
impl Command<Diagram> for ReconnectEdge {
    fn label(&self) -> &str {
        "Reconnect connection"
    }
    fn can_execute(&self, m: &Diagram) -> bool {
        m.edges.contains_key(&self.id)
            && m.accepts_edge(self.after.source, self.after.target, Some(self.id))
    }
    fn execute(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.after);
        m.publish(self.id);
        Ok(())
    }
    fn undo(&mut self, m: &mut Diagram) -> Result<(), CommandError> {
        m.edges.insert(self.id, self.before);
        m.publish(self.id);
        Ok(())
    }
}

struct EdgeCreation {
    source: u64,
    style: EdgeStyle,
}
impl ConnectionCreation<Diagram> for EdgeCreation {
    fn can_complete(
        &self,
        source: PolicyHost<u64>,
        target: PolicyHost<u64>,
        _: &CreateConnectionRequest,
        m: &Diagram,
    ) -> Result<bool, PolicyError> {
        Ok(source.model() == self.source
            && m.next_id != u64::MAX
            && m.accepts_edge(source.model(), target.model(), None))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<u64>,
        _: Option<PolicyHost<u64>>,
        _: &CreateConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Diagram,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        edge_preview(route)
    }
    fn command(
        &mut self,
        source: PolicyHost<u64>,
        target: PolicyHost<u64>,
        _: &CreateConnectionRequest,
        m: &Diagram,
    ) -> Result<Box<dyn Command<Diagram>>, PolicyError> {
        Ok(Box::new(InsertEdge {
            id: m.next_id,
            edge: Edge {
                source: source.model(),
                target: target.model(),
                style: self.style,
            },
        }))
    }
}
fn edge_preview(
    route: Option<ConnectionFeedbackRoute>,
) -> Result<Vec<FeedbackVisual>, PolicyError> {
    let Some(route) = route else {
        return Ok(Vec::new());
    };
    Ok(vec![FeedbackVisual::scaled(Box::new(
        novadraw::figure::PolylineFigure::new(
            route.source().x(),
            route.source().y(),
            route.target().x(),
            route.target().y(),
        ),
    ))])
}
struct NodeConnectionPolicy;
impl EditPolicy<Diagram> for NodeConnectionPolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        b.handle::<CreateConnection>(Self::start);
    }
}
impl NodeConnectionPolicy {
    fn start(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        req: &CreateConnectionRequest,
        _: &Diagram,
    ) -> Result<Option<<CreateConnection as RequestProtocol<Diagram>>::Contribution>, PolicyError>
    {
        Ok(
            EdgeStyle::from_name(req.connection_type().as_str()).map(|style| {
                Box::new(EdgeCreation {
                    source: host.model(),
                    style,
                }) as Box<dyn ConnectionCreation<Diagram>>
            }),
        )
    }
}

struct EdgeReconnection {
    id: u64,
    before: Edge,
}
impl EdgeReconnection {
    fn after(&self, fixed: u64, candidate: u64, endpoint: ConnectionEndpoint) -> Edge {
        match endpoint {
            ConnectionEndpoint::Source => Edge {
                source: candidate,
                target: fixed,
                ..self.before
            },
            ConnectionEndpoint::Target => Edge {
                source: fixed,
                target: candidate,
                ..self.before
            },
        }
    }
}
impl ConnectionReconnection<Diagram> for EdgeReconnection {
    fn can_complete(
        &self,
        _: PolicyHost<u64>,
        fixed: PolicyHost<u64>,
        candidate: PolicyHost<u64>,
        req: &ReconnectConnectionRequest,
        m: &Diagram,
    ) -> Result<bool, PolicyError> {
        let edge = self.after(fixed.model(), candidate.model(), req.endpoint());
        Ok(m.accepts_edge(edge.source, edge.target, Some(self.id)))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<u64>,
        _: PolicyHost<u64>,
        _: Option<PolicyHost<u64>>,
        _: &ReconnectConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Diagram,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        edge_preview(route)
    }
    fn command(
        &mut self,
        _: PolicyHost<u64>,
        fixed: PolicyHost<u64>,
        candidate: PolicyHost<u64>,
        req: &ReconnectConnectionRequest,
        _: &Diagram,
    ) -> Result<Box<dyn Command<Diagram>>, PolicyError> {
        Ok(Box::new(ReconnectEdge {
            id: self.id,
            before: self.before,
            after: self.after(fixed.model(), candidate.model(), req.endpoint()),
        }))
    }
}
struct EdgePolicy;
impl EditPolicy<Diagram> for EdgePolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Diagram, Self>) {
        b.handle::<DeleteParts>(Self::delete);
        b.handle::<ReconnectConnection>(Self::reconnect);
    }
}
impl EdgePolicy {
    fn delete(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        _: &Diagram,
    ) -> Result<Option<EditingContribution<Diagram>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(DeleteEdge {
            id: host.model(),
            removed: None,
        }))))
    }
    fn reconnect(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        _: &ReconnectConnectionRequest,
        m: &Diagram,
    ) -> Result<Option<<ReconnectConnection as RequestProtocol<Diagram>>::Contribution>, PolicyError>
    {
        Ok(m.edges.get(&host.model()).map(|edge| {
            Box::new(EdgeReconnection {
                id: host.model(),
                before: *edge,
            }) as Box<dyn ConnectionReconnection<Diagram>>
        }))
    }
}

struct EdgePart;
impl EditPartBehavior<Diagram> for EdgePart {
    fn create_figure(&mut self, m: &Diagram, id: u64) -> Result<Box<dyn Figure>, EditPartError> {
        let dash = match m.edges[&id].style {
            EdgeStyle::Solid => DashPattern::Solid,
            EdgeStyle::Dashed => DashPattern::Dash,
        };
        let stroke = StrokeStyle::default()
            .with_width(OUTLINE_WIDTH)
            .map_err(|e| EditPartError::operation(e.to_string()))?
            .with_dash_pattern(dash);
        Ok(Box::new(
            ConnectionFigure::new()
                .with_stroke(Color::BLACK, OUTLINE_WIDTH)
                .with_stroke_style(stroke),
        ))
    }
    fn configure_visual(
        &mut self,
        _: &Diagram,
        _: u64,
        ctx: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        let arrow = PolygonDecorationFigure::triangle()
            .with_scale(ARROW_LENGTH, ARROW_HALF_WIDTH)
            .map_err(|e| EditPartError::operation(e.to_string()))?
            .with_fill_color(Color::BLACK);
        ctx.add_connection_child(Box::new(arrow), Box::new(ConnectionLocator::Target))?;
        Ok(())
    }
    fn create_policies(
        &mut self,
        _: &Diagram,
        _: u64,
    ) -> Result<Vec<PolicyInstallation<Diagram>>, EditPartError> {
        // One open policy handles both families in one slot; duplicate registration would dispatch twice.
        Ok(vec![
            PolicyInstallation::new(ConnectionReconnectSlot::key(), EdgePolicy)
                .map_err(|e| EditPartError::operation(e.to_string()))?,
        ])
    }
}

/// Persisted document contains business identities only, never runtime handles.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    version: u32,
    #[serde(deserialize_with = "unique_ids")]
    shapes: BTreeMap<u64, Shape>,
    order: Vec<u64>,
    #[serde(deserialize_with = "unique_ids")]
    edges: BTreeMap<u64, Edge>,
}

fn unique_ids<'de, D, T>(deserializer: D) -> Result<BTreeMap<u64, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<u64, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object with unique numeric identities")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((id, value)) = map.next_entry()? {
                if values.insert(id, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate model identity"));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}
const DOCUMENT_VERSION: u32 = 1;
impl Diagram {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&Document {
            version: DOCUMENT_VERSION,
            shapes: self.shapes.clone(),
            order: self.order.clone(),
            edges: self.edges.clone(),
        })
        .map_err(|e| e.to_string())
    }
    pub fn from_json(json: &str) -> Result<Self, String> {
        let doc: Document = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if doc.version != DOCUMENT_VERSION {
            return Err("unsupported Shapes document version".into());
        }
        let ids = doc
            .order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if ids.len() != doc.order.len()
            || ids.len() != doc.shapes.len()
            || !doc.shapes.keys().all(|id| ids.contains(id) && *id != ROOT)
            || doc.shapes.values().any(|s| !valid_bounds(s.bounds))
            || doc
                .edges
                .keys()
                .any(|id| *id == ROOT || doc.shapes.contains_key(id))
        {
            return Err("invalid shape identity, order or geometry".into());
        }
        let next_id = doc
            .shapes
            .keys()
            .chain(doc.edges.keys())
            .max()
            .copied()
            .unwrap_or(ROOT)
            .checked_add(1)
            .ok_or("document identity exhausted")?;
        let model = Self {
            shapes: doc.shapes,
            order: doc.order,
            edges: doc.edges,
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id,
        };
        if model
            .edges
            .iter()
            .any(|(id, e)| !model.accepts_edge(e.source, e.target, Some(*id)))
        {
            return Err("invalid connection endpoints or duplicate edge".into());
        }
        Ok(model)
    }
}
