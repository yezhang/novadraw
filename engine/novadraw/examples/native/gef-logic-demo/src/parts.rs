use crate::model::*;
use novadraw::connection::{
    AnchorError, AnchorSemanticKey, AnchorSite, Bendpoint, BendpointConstraint, ConnectionAnchor,
    ConnectionFigure, ConnectionFigureBehavior, CoordinateSpace, RouteSceneQuery,
};
use novadraw::container::FreeformLayerFigure;
use novadraw::figure::{Figure, FigureStyle, LabelFigure, RectangleFigure, border::LineBorder};
use novadraw::geometry::{Dimension, Point, Rectangle, Vec2};
use novadraw::graphics::Color;
use novadraw::runtime::{
    FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate, SetLabelText,
};
use novadraw::tree::FigureId;
use novadraw_editor::tool::CreationTemplate;
use novadraw_editor::*;

pub const PIN_SIZE: f64 = 18.0;
const PIN_STEP: f64 = 22.0;
const PIN_TOP: f64 = 18.0;
const LINE_WIDTH: f64 = 2.0;
pub fn high_color() -> Color {
    Color::rgba8(32, 143, 107, 255)
}
pub fn low_color() -> Color {
    Color::rgba8(139, 151, 164, 255)
}
pub fn selection_color() -> Color {
    Color::rgba8(58, 110, 184, 255)
}

pub fn port_bounds(circuit: &Circuit, port: Port) -> Rectangle {
    let kind = circuit.document.nodes[&port.node].kind;
    let x = match port.direction {
        Direction::Input => 0.0,
        Direction::Output => NODE_WIDTH - PIN_SIZE,
    };
    let count = match port.direction {
        Direction::Input => kind.inputs(),
        Direction::Output => kind.outputs(),
    };
    let y = if count == 1 {
        (NODE_HEIGHT - PIN_SIZE) / 2.0
    } else {
        PIN_TOP + f64::from(port.index) * PIN_STEP
    };
    Rectangle::new(x, y, PIN_SIZE, PIN_SIZE)
}

pub fn port_point(circuit: &Circuit, port: Port) -> Point {
    let b = port_bounds(circuit, port);
    let node = &circuit.document.nodes[&port.node];
    Point::new(node.x + b.x + b.width / 2.0, node.y + b.y + b.height / 2.0)
}
pub fn resolved_bendpoints(m: &Circuit, id: u64) -> Vec<Point> {
    let wire = m.document.wires[&id];
    let source = port_point(m, wire.source);
    let target = port_point(m, wire.target);
    m.document
        .bendpoints
        .get(&id)
        .into_iter()
        .flatten()
        .map(|p| {
            Point::new(
                (source.x() + p.source[0] + target.x() + p.target[0]) / 2.0,
                (source.y() + p.source[1] + target.y() + p.target[1]) / 2.0,
            )
        })
        .collect()
}

struct PinAnchor(FigureId, Direction);
impl ConnectionAnchor for PinAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.0)
    }
    fn reference_point(
        &self,
        scene: &mut dyn RouteSceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        let bounds = scene.border_box(self.0)?;
        Ok(scene.map_point(
            Point::new(bounds.width / 2.0, bounds.height / 2.0),
            CoordinateSpace::FigureLocal(self.0),
            output,
        )?)
    }
    fn location(
        &self,
        scene: &mut dyn RouteSceneQuery,
        _: Point,
        _: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        Ok(AnchorSite {
            point: self.reference_point(scene, output)?,
            outward_normal: Some(Vec2::new(
                if self.1 == Direction::Output {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            )),
        })
    }
}

fn pin_anchor(
    id: Id,
    ctx: ConnectionAnchorContext<Id>,
) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
    let Id::Port(port) = id else {
        return Ok(None);
    };
    let owner = ctx.endpoint_figure();
    let key = AnchorSemanticKey::new(Some(owner), "logic-terminal-center", Vec::new())
        .map_err(|e| EditPartError::operation(e.to_string()))?;
    Ok(Some(ConnectionAnchorDescriptor::new(
        key,
        Box::new(PinAnchor(owner, port.direction)),
    )))
}

pub struct Factory;
pub fn manual_router_key() -> ConnectionRouterKey {
    ConnectionRouterKey::new("logic-manual").expect("valid router key")
}
impl EditPartFactory<Circuit> for Factory {
    fn connection_routers(&mut self) -> Result<Vec<ConnectionRouterRegistration>, EditPartError> {
        const FAN_SEPARATION: f64 = 10.0;
        Ok(vec![ConnectionRouterRegistration::new(
            manual_router_key(),
            Box::new(
                novadraw::connection::FanRouter::new(
                    Box::new(novadraw::connection::BendpointConnectionRouter),
                    FAN_SEPARATION,
                )
                .map_err(|e| EditPartError::operation(e.to_string()))?,
            ),
        )])
    }
    fn create(
        &mut self,
        _: PartFactoryContext<Id>,
        _: &Circuit,
    ) -> Result<Box<dyn EditPartBehavior<Circuit>>, EditPartError> {
        Ok(Box::new(Part { value_label: None }))
    }
    fn create_connection(
        &mut self,
        _: ConnectionPartFactoryContext<Id>,
        _: &Circuit,
    ) -> Result<Box<dyn EditPartBehavior<Circuit>>, EditPartError> {
        Ok(Box::new(WirePart))
    }
}
struct Part {
    value_label: Option<FigureId>,
}
impl EditPartBehavior<Circuit> for Part {
    fn create_figure(&mut self, m: &Circuit, id: Id) -> Result<Box<dyn Figure>, EditPartError> {
        match id {
            Id::Root => Ok(Box::new(
                FreeformLayerFigure::new(0.0, 0.0, 880.0, 500.0)
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            )),
            Id::Node(node) => {
                let b = m.document.nodes[&node].bounds();
                Ok(Box::new(
                    RectangleFigure::new_with_color(b.x, b.y, b.width, b.height, Color::WHITE)
                        .map_err(|e| EditPartError::operation(e.to_string()))?
                        .with_stroke(Color::rgba8(204, 213, 223, 255), 1.0)
                        .map_err(|e| EditPartError::operation(e.to_string()))?,
                ))
            }
            Id::Port(port) => {
                let node = &m.document.nodes[&port.node];
                let label = if port.direction == Direction::Output && node.kind != Kind::Led {
                    "Q"
                } else {
                    port.label()
                };
                Ok(Box::new(
                    LabelFigure::new(label)
                        .with_bounds(port_bounds(m, port))
                        .map_err(|e| EditPartError::operation(e.to_string()))?
                        .with_border(
                            LineBorder::new(low_color(), 1.0)
                                .map_err(|e| EditPartError::operation(e.to_string()))?,
                        )
                        .map_err(|e| EditPartError::operation(e.to_string()))?,
                ))
            }
            _ => Err(EditPartError::operation("Invalid containment item")),
        }
    }
    fn configure_visual(
        &mut self,
        m: &Circuit,
        id: Id,
        ctx: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        if let Id::Node(node) = id {
            let primary = ctx.primary();
            let name = LabelFigure::new(&m.document.nodes[&node].name)
                .with_bounds(Rectangle::new(
                    PIN_SIZE,
                    4.0,
                    NODE_WIDTH - 2.0 * PIN_SIZE,
                    24.0,
                ))
                .map_err(|e| EditPartError::operation(e.to_string()))?;
            ctx.add_child(primary, Box::new(name))?;
            let value = LabelFigure::new(node_caption(m, node))
                .with_bounds(Rectangle::new(
                    PIN_SIZE,
                    38.0,
                    NODE_WIDTH - 2.0 * PIN_SIZE,
                    40.0,
                ))
                .map_err(|e| EditPartError::operation(e.to_string()))?;
            self.value_label = Some(ctx.add_child(primary, Box::new(value))?);
        }
        Ok(())
    }
    fn create_policies(
        &mut self,
        _: &Circuit,
        id: Id,
    ) -> Result<Vec<PolicyInstallation<Circuit>>, EditPartError> {
        let policy = match id {
            Id::Root => Some(PolicyInstallation::new(LayoutSlot::key(), CanvasPolicy)),
            Id::Node(_) => Some(PolicyInstallation::new(ComponentSlot::key(), NodePolicy)),
            Id::Port(port) if port.direction == Direction::Output => Some(PolicyInstallation::new(
                ConnectionCreationSlot::key(),
                PinPolicy,
            )),
            _ => None,
        };
        policy
            .into_iter()
            .map(|p| p.map_err(|e| EditPartError::operation(e.to_string())))
            .collect()
    }
    fn refresh_visuals(
        &mut self,
        m: &Circuit,
        id: Id,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        match id {
            Id::Node(node) => {
                ctx.set_primary_bounds(m.document.nodes[&node].bounds())?;
                ctx.set_primary_style(FigureStyle {
                    foreground: Some(Color::rgba8(55, 65, 81, 255)),
                    background: Some(Color::WHITE),
                    ..Default::default()
                })?;
                if let Some(label) = self.value_label {
                    ctx.update_visual_component(label, SetLabelText::new(node_caption(m, node)))?;
                }
            }
            Id::Port(port) => {
                ctx.set_primary_opaque(true)?;
                ctx.set_primary_style(FigureStyle {
                    foreground: Some(if m.signal(port) {
                        high_color()
                    } else {
                        low_color()
                    }),
                    background: Some(Color::WHITE),
                    ..Default::default()
                })?;
            }
            _ => {}
        }
        Ok(())
    }
    fn source_connection_anchor(
        &mut self,
        _: &Circuit,
        id: Id,
        ctx: ConnectionAnchorContext<Id>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        pin_anchor(id, ctx)
    }
    fn target_connection_anchor(
        &mut self,
        _: &Circuit,
        id: Id,
        ctx: ConnectionAnchorContext<Id>,
    ) -> Result<Option<ConnectionAnchorDescriptor>, EditPartError> {
        pin_anchor(id, ctx)
    }
}
fn node_caption(m: &Circuit, id: u64) -> String {
    let node = &m.document.nodes[&id];
    let value = m.signals.get(&id).copied().unwrap_or(0);
    if node.kind == Kind::Led {
        format!("{value:04b} · {value}")
    } else {
        format!("{} · {value}", node.kind.name())
    }
}

struct NodePolicy;
impl EditPolicy<Circuit> for NodePolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Circuit, Self>) {
        b.handle::<ChangeBounds>(Self::change);
        b.handle::<DeleteParts>(Self::delete);
    }
}
impl NodePolicy {
    fn change(
        &mut self,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        req: &ChangeBoundsRequest,
        m: &Circuit,
    ) -> Result<Option<EditingContribution<Circuit>>, PolicyError> {
        let Id::Node(id) = host.model() else {
            return Ok(None);
        };
        if req.kind() != ChangeBoundsKind::Move {
            return Ok(None);
        }
        let before = m.document.nodes[&id].clone();
        let b = req.transformed_bounds(before.bounds());
        let after = Node {
            x: b.x,
            y: b.y,
            ..before.clone()
        };
        let outline = RectangleFigure::new_with_color(
            b.x,
            b.y,
            NODE_WIDTH,
            NODE_HEIGHT,
            Color::rgba8(58, 110, 184, 20),
        )
        .map_err(|e| PolicyError::operation(e.to_string()))?
        .with_stroke(selection_color(), LINE_WIDTH)
        .map_err(|e| PolicyError::operation(e.to_string()))?;
        Ok(Some(EditingContribution::new(
            Some(Box::new(ChangeNode {
                id,
                before: Some(before),
                after,
            })),
            vec![FeedbackVisual::scaled(Box::new(outline))],
        )))
    }
    fn delete(
        &mut self,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        _: &Circuit,
    ) -> Result<Option<EditingContribution<Circuit>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(
            DeleteItem::new(host.model()),
        ))))
    }
}
pub struct NodeTemplate(pub Kind);
impl CreationTemplate<Circuit> for NodeTemplate {
    type Request = CreateChild;
    fn default_size(&self) -> Dimension {
        Dimension::new(NODE_WIDTH, NODE_HEIGHT)
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
                CreationType::new(self.0.name()).expect("nonempty kind"),
                bounds,
                modifiers,
                revision,
            ),
        )
    }
}
struct CanvasPolicy;
impl EditPolicy<Circuit> for CanvasPolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Circuit, Self>) {
        b.handle::<CreateChild>(Self::create);
    }
}
impl CanvasPolicy {
    fn create(
        &mut self,
        _: PolicyHost<Id>,
        _: &RequestMetadata,
        req: &CreateRequest,
        m: &Circuit,
    ) -> Result<Option<EditingContribution<Circuit>>, PolicyError> {
        let Some(kind) = Kind::ALL
            .into_iter()
            .find(|kind| kind.name() == req.creation_type().as_str())
        else {
            return Ok(None);
        };
        let b = req.bounds();
        Ok(Some(EditingContribution::command(Box::new(ChangeNode {
            id: m.next_id,
            before: None,
            after: Node {
                kind,
                name: format!("{} #{}", kind.name(), m.next_id),
                x: b.x,
                y: b.y,
                value: 0,
            },
        }))))
    }
}
struct PinPolicy;
impl EditPolicy<Circuit> for PinPolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Circuit, Self>) {
        b.handle::<CreateConnection>(Self::start);
    }
}
impl PinPolicy {
    fn start(
        &mut self,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        _: &CreateConnectionRequest,
        _: &Circuit,
    ) -> Result<Option<<CreateConnection as RequestProtocol<Circuit>>::Contribution>, PolicyError>
    {
        let Id::Port(port) = host.model() else {
            return Ok(None);
        };
        Ok(Some(Box::new(WireCreation(port))))
    }
}
struct WireCreation(Port);
fn pair(source: PolicyHost<Id>, target: PolicyHost<Id>) -> Option<Wire> {
    match (source.model(), target.model()) {
        (Id::Port(source), Id::Port(target)) => Some(Wire { source, target }),
        _ => None,
    }
}
impl ConnectionCreation<Circuit> for WireCreation {
    fn can_complete(
        &self,
        source: PolicyHost<Id>,
        target: PolicyHost<Id>,
        _: &CreateConnectionRequest,
        m: &Circuit,
    ) -> Result<bool, PolicyError> {
        Ok(pair(source, target).is_some_and(|wire| wire.source == self.0 && m.accepts(wire, None)))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<Id>,
        _: Option<PolicyHost<Id>>,
        _: &CreateConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Circuit,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        preview(route)
    }
    fn command(
        &mut self,
        source: PolicyHost<Id>,
        target: PolicyHost<Id>,
        _: &CreateConnectionRequest,
        m: &Circuit,
    ) -> Result<Box<dyn Command<Circuit>>, PolicyError> {
        Ok(Box::new(ChangeWire {
            id: m.next_id,
            before: None,
            after: pair(source, target)
                .ok_or_else(|| PolicyError::operation("Select input terminal"))?,
        }))
    }
}
fn preview(route: Option<ConnectionFeedbackRoute>) -> Result<Vec<FeedbackVisual>, PolicyError> {
    let Some(route) = route else {
        return Ok(Vec::new());
    };
    Ok(vec![FeedbackVisual::scaled(Box::new(
        novadraw::figure::PolylineFigure::new(
            route.source().x(),
            route.source().y(),
            route.target().x(),
            route.target().y(),
        )
        .map_err(|e| PolicyError::operation(e.to_string()))?
        .with_color(selection_color())
        .with_width(LINE_WIDTH)
        .map_err(|e| PolicyError::operation(e.to_string()))?,
    ))])
}
struct WirePart;
struct SetWireSignal(Color);
impl FigureComponentUpdate for SetWireSignal {
    type Figure = ConnectionFigure;
    type Prepared = ConnectionFigure;
    type Error = novadraw::graphics::GraphicsInputError;

    fn prepare(
        self,
        current: &Self::Figure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if current.connection_stroke_color() == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let updated = current
            .clone()
            .with_stroke(self.0, current.connection_stroke_width())?;
        Ok(PreparedFigureUpdate::paint(updated))
    }
    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}
impl EditPartBehavior<Circuit> for WirePart {
    fn create_figure(&mut self, _: &Circuit, _: Id) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(
            ConnectionFigure::new()
                .with_stroke(low_color(), LINE_WIDTH)
                .map_err(|e| EditPartError::operation(e.to_string()))?,
        ))
    }
    fn create_policies(
        &mut self,
        _: &Circuit,
        _: Id,
    ) -> Result<Vec<PolicyInstallation<Circuit>>, EditPartError> {
        Ok(vec![
            PolicyInstallation::new(ConnectionReconnectSlot::key(), WirePolicy)
                .map_err(|e| EditPartError::operation(e.to_string()))?,
            PolicyInstallation::new(
                PolicyKey::<WireBendpointSlot>::new("logic-bendpoint"),
                WireBendpointPolicy,
            )
            .map_err(|e| EditPartError::operation(e.to_string()))?,
        ])
    }
    fn connection_routing(
        &mut self,
        m: &Circuit,
        id: Id,
    ) -> Result<ConnectionRoutingDescriptor, EditPartError> {
        let Id::Wire(id) = id else {
            return Ok(ConnectionRoutingDescriptor::inherited());
        };
        if m.document.routing != Routing::Manual {
            return Ok(ConnectionRoutingDescriptor::inherited());
        }
        let points = m
            .document
            .bendpoints
            .get(&id)
            .into_iter()
            .flatten()
            .map(|p| Bendpoint::Relative {
                source_offset: Vec2::new(p.source[0], p.source[1]),
                target_offset: Vec2::new(p.target[0], p.target[1]),
                weight: 0.5,
            })
            .collect::<Vec<_>>();
        Ok(ConnectionRoutingDescriptor::registered(
            manual_router_key(),
            Some(Box::new(BendpointConstraint::new(points))),
        ))
    }
    fn connection_bendpoints(&mut self, m: &Circuit, id: Id) -> Result<Vec<Point>, EditPartError> {
        Ok(match id {
            Id::Wire(id) if m.document.routing == Routing::Manual => resolved_bendpoints(m, id),
            _ => Vec::new(),
        })
    }
    fn refresh_visuals(
        &mut self,
        m: &Circuit,
        id: Id,
        ctx: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        let Id::Wire(id) = id else {
            return Ok(());
        };
        let wire = m.document.wires[&id];
        ctx.update_visual_component(
            ctx.primary(),
            SetWireSignal(if m.signal(wire.source) {
                high_color()
            } else {
                low_color()
            }),
        )?;
        Ok(())
    }
}
struct WireBendpointSlot;
impl PolicySlot for WireBendpointSlot {
    const NAME: &'static str = "logic-bendpoint";
}
struct WireBendpointPolicy;
impl EditPolicy<Circuit> for WireBendpointPolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Circuit, Self>) {
        b.handle::<EditBendpoint>(Self::edit);
    }
}
impl WireBendpointPolicy {
    fn edit(
        &mut self,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        req: &BendpointRequest,
        m: &Circuit,
    ) -> Result<Option<EditingContribution<Circuit>>, PolicyError> {
        let Id::Wire(id) = host.model() else {
            return Ok(None);
        };
        if m.document.routing != Routing::Manual {
            return Ok(None);
        }
        let before = m.document.bendpoints.get(&id).cloned().unwrap_or_default();
        let mut after = before.clone();
        let wire = m.document.wires[&id];
        let source = port_point(m, wire.source);
        let target = port_point(m, wire.target);
        let p = req.location();
        let relative = RelativePoint {
            source: [p.x() - source.x(), p.y() - source.y()],
            target: [p.x() - target.x(), p.y() - target.y()],
        };
        match req.operation() {
            BendpointOperation::Create { index } if index <= after.len() => {
                after.insert(index, relative)
            }
            BendpointOperation::Move { index } if index < after.len() => after[index] = relative,
            BendpointOperation::Delete { index } if index < after.len() => {
                after.remove(index);
            }
            _ => return Err(PolicyError::operation("Invalid bendpoint index")),
        }
        let feedback = FeedbackVisual::scaled(Box::new(
            novadraw::figure::PolylineFigure::new(source.x(), source.y(), p.x(), p.y())
                .map_err(|e| PolicyError::operation(e.to_string()))?
                .with_color(selection_color())
                .with_width(LINE_WIDTH)
                .map_err(|e| PolicyError::operation(e.to_string()))?,
        ));
        Ok(Some(EditingContribution::new(
            Some(Box::new(ChangeBendpoints { id, before, after })),
            vec![feedback],
        )))
    }
}
struct WirePolicy;
impl EditPolicy<Circuit> for WirePolicy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Circuit, Self>) {
        b.handle::<DeleteParts>(NodePolicy::delete_wire);
        b.handle::<ReconnectConnection>(Self::reconnect);
    }
}
impl NodePolicy {
    fn delete_wire(
        _: &mut WirePolicy,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        _: &DeleteRequest,
        _: &Circuit,
    ) -> Result<Option<EditingContribution<Circuit>>, PolicyError> {
        Ok(Some(EditingContribution::command(Box::new(
            DeleteItem::new(host.model()),
        ))))
    }
}
impl WirePolicy {
    fn reconnect(
        &mut self,
        host: PolicyHost<Id>,
        _: &RequestMetadata,
        _: &ReconnectConnectionRequest,
        m: &Circuit,
    ) -> Result<Option<<ReconnectConnection as RequestProtocol<Circuit>>::Contribution>, PolicyError>
    {
        let Id::Wire(id) = host.model() else {
            return Ok(None);
        };
        Ok(Some(Box::new(WireReconnect {
            id,
            before: m.document.wires[&id],
        })))
    }
}
struct WireReconnect {
    id: u64,
    before: Wire,
}
impl WireReconnect {
    fn after(
        &self,
        fixed: PolicyHost<Id>,
        candidate: PolicyHost<Id>,
        endpoint: ConnectionEndpoint,
    ) -> Option<Wire> {
        if endpoint == ConnectionEndpoint::Source {
            pair(candidate, fixed)
        } else {
            pair(fixed, candidate)
        }
    }
}
impl ConnectionReconnection<Circuit> for WireReconnect {
    fn can_complete(
        &self,
        _: PolicyHost<Id>,
        fixed: PolicyHost<Id>,
        candidate: PolicyHost<Id>,
        req: &ReconnectConnectionRequest,
        m: &Circuit,
    ) -> Result<bool, PolicyError> {
        Ok(self
            .after(fixed, candidate, req.endpoint())
            .is_some_and(|wire| m.accepts(wire, Some(self.id))))
    }
    fn feedback_with_route(
        &mut self,
        _: PolicyHost<Id>,
        _: PolicyHost<Id>,
        _: Option<PolicyHost<Id>>,
        _: &ReconnectConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        _: &Circuit,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        preview(route)
    }
    fn command(
        &mut self,
        _: PolicyHost<Id>,
        fixed: PolicyHost<Id>,
        candidate: PolicyHost<Id>,
        req: &ReconnectConnectionRequest,
        _: &Circuit,
    ) -> Result<Box<dyn Command<Circuit>>, PolicyError> {
        Ok(Box::new(ChangeWire {
            id: self.id,
            before: Some(self.before),
            after: self
                .after(fixed, candidate, req.endpoint())
                .ok_or_else(|| PolicyError::operation("Select a terminal"))?,
        }))
    }
}
