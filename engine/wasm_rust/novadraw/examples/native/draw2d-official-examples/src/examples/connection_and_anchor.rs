use std::sync::Mutex;

use novadraw::{
    connection::{
        ChopboxAnchor, ConnectionEndpointLocator, ConnectionFigure, ConnectionId, CoordinateSpace,
        MidpointLocator, PolygonDecorationFigure, RouteEnd, RouterBinding,
    },
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        Border, CONTAINER, ChildClippingStrategy, ContainerCapability, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer, FigureStyle,
        INPUT, InputCapability, LabelFigure, RectangleFigure, border::LineBorder,
    },
    geometry::{Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    runtime::Runtime,
    text::{FontDescriptor, TextConstraints},
    tree::{FigureId, FigureSubtree, SubtreeBindingNodes, SubtreeConnection},
};

use super::runtime_with_fonts;

const WIDTH: f64 = 820.0;
const HEIGHT: f64 = 440.0;
const PANEL_WIDTH: f64 = 400.0;
const PANEL_HEIGHT: f64 = 400.0;
const SECOND_PANEL_X: f64 = 420.0;
const NODE_WIDTH: f64 = 50.0;
const NODE_HEIGHT: f64 = 30.0;

struct GroupBoxFigure {
    bounds: Rectangle,
}

impl GroupBoxFigure {
    fn new(bounds: Rectangle) -> Self {
        Self { bounds }
    }
}

impl Figure for GroupBoxFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "GroupBox"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(Color::WHITE),
            ..FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::WHITE);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            9.5,
            bounds.width - 1.0,
            bounds.height - 10.0,
        ))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for GroupBoxFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

#[derive(Clone, Copy)]
struct DragSession {
    entry: Point,
    bounds: Rectangle,
}

struct DraggableRectangle {
    bounds: Rectangle,
    fill: Color,
    drag: Mutex<Option<DragSession>>,
}

impl DraggableRectangle {
    fn new(bounds: Rectangle, fill: Color) -> Self {
        Self {
            bounds,
            fill,
            drag: Mutex::new(None),
        }
    }
}

impl Figure for DraggableRectangle {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RectangleFigure"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(self.fill),
            ..FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(self.fill);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for DraggableRectangle {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let entry = event.entry_point();
        let target = context.target_id();
        let Ok(bounds) = context.scene().figure_bounds(target) else {
            return false;
        };
        *self.drag.lock().expect("drag state") = Some(DragSession { entry, bounds });
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let Some(session) = *self.drag.lock().expect("drag state") else {
            return false;
        };
        let entry = event.entry_point();
        let target = context.target_id();
        context.effects().set_bounds_later(
            target,
            Rectangle::new(
                session.bounds.x + entry.x() - session.entry.x(),
                session.bounds.y + entry.y() - session.entry.y(),
                session.bounds.width,
                session.bounds.height,
            ),
        );
        true
    }

    fn on_mouse_released(&self, _event: &MouseEvent, _context: &mut EventContext<'_>) -> bool {
        self.drag.lock().expect("drag state").take().is_some()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    first_node: FigureId,
    endpoint_node: FigureId,
    first_connection: ConnectionId,
}

pub fn connection_and_anchor() -> Runtime {
    fixture().runtime
}

fn fixture() -> Fixture {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("ConnectionAndAnchorExample root");

    let (midpoint_panel, first_node, first_connection_figure, first_connection) =
        add_connected_group(
            &mut runtime,
            root,
            Rectangle::new(0.0, 0.0, PANEL_WIDTH, PANEL_HEIGHT),
            "Midpoint Locator",
            Rectangle::new(40.0, 40.0, NODE_WIDTH, NODE_HEIGHT),
            Color::rgba(1.0, 0.0, 0.0, 1.0),
            Rectangle::new(200.0, 40.0, NODE_WIDTH, NODE_HEIGHT),
            Color::rgba(0.0, 0.0, 1.0, 1.0),
        );
    let midpoint_label =
        add_connection_label(&mut runtime, first_connection_figure, "Midpoint", 72.0);
    runtime
        .connections()
        .set_locator(
            first_connection,
            midpoint_label,
            Box::new(MidpointLocator::new(0)),
        )
        .expect("midpoint locator");
    add_target_decoration(&mut runtime, first_connection, first_connection_figure);
    runtime
        .connections()
        .resolve_route(
            first_connection,
            CoordinateSpace::ChildContent(midpoint_panel),
        )
        .expect("midpoint route");

    let (endpoint_panel, endpoint_node, second_connection_figure, second_connection) =
        add_connected_group(
            &mut runtime,
            root,
            Rectangle::new(SECOND_PANEL_X, 0.0, PANEL_WIDTH, PANEL_HEIGHT),
            "Connection endpoint Locator",
            Rectangle::new(60.0, 40.0, NODE_WIDTH, NODE_HEIGHT),
            Color::rgba(0.0, 1.0, 0.0, 1.0),
            Rectangle::new(60.0, 230.0, NODE_WIDTH, NODE_HEIGHT),
            Color::BLACK,
        );
    let target_label =
        add_connection_label(&mut runtime, second_connection_figure, "Endpoint", 72.0);
    runtime
        .connections()
        .set_locator(
            second_connection,
            target_label,
            Box::new(ConnectionEndpointLocator::with_default_distances(
                RouteEnd::Target,
            )),
        )
        .expect("target endpoint locator");
    let source_label =
        add_connection_label(&mut runtime, second_connection_figure, "Endpoint", 72.0);
    runtime
        .connections()
        .set_locator(
            second_connection,
            source_label,
            Box::new(
                ConnectionEndpointLocator::new(RouteEnd::Source, 4.0, -3.0)
                    .expect("source locator"),
            ),
        )
        .expect("source endpoint locator");
    add_target_decoration(&mut runtime, second_connection, second_connection_figure);
    runtime
        .connections()
        .resolve_route(
            second_connection,
            CoordinateSpace::ChildContent(endpoint_panel),
        )
        .expect("endpoint route");

    let instructions_text = " Drag the shapes to see the Locators in action ";
    let instruction_layout = runtime
        .text()
        .layout(
            instructions_text,
            &FontDescriptor::parse("13px Inter Variable").expect("instruction font"),
            TextConstraints::UNBOUNDED,
        )
        .expect("instruction measurement");
    let instruction_border = LineBorder::default_border();
    let insets = instruction_border.get_insets();
    let instructions = runtime
        .figure(root)
        .expect("root")
        .add(Box::new(
            LabelFigure::new(instructions_text)
                .with_bounds(Rectangle::new(
                    10.0,
                    420.0,
                    f64::from(instruction_layout.width()) + insets.width(),
                    f64::from(instruction_layout.height()) + insets.height(),
                ))
                .with_border(instruction_border),
        ))
        .expect("instructions");
    runtime
        .figure(instructions)
        .expect("instructions")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            background: Some(Color::WHITE),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("instruction style");

    Fixture {
        runtime,
        first_node,
        endpoint_node,
        first_connection,
    }
}

#[allow(clippy::too_many_arguments)]
fn add_connected_group(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    title: &str,
    source_bounds: Rectangle,
    source_fill: Color,
    target_bounds: Rectangle,
    target_fill: Color,
) -> (FigureId, FigureId, FigureId, ConnectionId) {
    let mut subtree = FigureSubtree::new(Box::new(GroupBoxFigure::new(bounds)));
    let group = subtree.root();
    let title_layout = runtime
        .text()
        .layout(
            title,
            &FontDescriptor::parse("13px Inter Variable").expect("title font"),
            TextConstraints::UNBOUNDED,
        )
        .expect("GroupBox title measurement");
    let title_width = f64::from(title_layout.width());
    let label = subtree
        .add(
            group,
            Box::new(LabelFigure::new(title).with_bounds(Rectangle::new(
                8.0,
                0.0,
                title_width,
                f64::from(title_layout.height()),
            ))),
        )
        .expect("GroupBox title");
    subtree
        .set_style(
            label,
            FigureStyle {
                foreground: Some(Color::BLACK),
                background: Some(Color::WHITE),
                font: Some("13px Inter Variable".to_owned()),
                ..FigureStyle::default()
            },
        )
        .expect("GroupBox title style");
    let source = subtree
        .add(
            group,
            Box::new(DraggableRectangle::new(source_bounds, source_fill)),
        )
        .expect("source node");
    let target = subtree
        .add(
            group,
            Box::new(DraggableRectangle::new(target_bounds, target_fill)),
        )
        .expect("target node");
    let figure = subtree
        .add(
            group,
            Box::new(ConnectionFigure::new().with_stroke(Color::BLACK, 1.0)),
        )
        .expect("connection Figure");
    let router = runtime.connections().direct_router();
    subtree
        .set_connection_binding(figure, move |nodes: &SubtreeBindingNodes| {
            Ok(SubtreeConnection {
                source: Some(Box::new(ChopboxAnchor::new(
                    nodes.figure(source).expect("source mapping"),
                ))),
                target: Some(Box::new(ChopboxAnchor::new(
                    nodes.figure(target).expect("target mapping"),
                ))),
                router: RouterBinding::Explicit { router },
                constraint: None,
            })
        })
        .expect("construction binding");
    let receipt = runtime
        .figure(parent)
        .expect("group parent")
        .add_subtree(subtree)
        .expect("complete connected group");
    let group = receipt.root();
    let figure = receipt.figure(figure).expect("connection mapping");
    let connection = ConnectionId::from_figure(figure);
    runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(group))
        .expect("initial route");
    (
        group,
        receipt.figure(source).expect("source mapping"),
        figure,
        connection,
    )
}

fn add_connection_label(
    runtime: &mut Runtime,
    connection: FigureId,
    text: &str,
    width: f64,
) -> FigureId {
    let label = runtime
        .figure(connection)
        .expect("connection Figure")
        .add(Box::new(
            LabelFigure::new(text)
                .with_bounds(Rectangle::new(0.0, 0.0, width, 24.0))
                .with_border(LineBorder::default_border()),
        ))
        .expect("connection label");
    runtime
        .figure(label)
        .expect("connection label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            background: Some(Color::rgba(0.95, 0.95, 0.95, 1.0)),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("connection label style");
    label
}

fn add_target_decoration(
    runtime: &mut Runtime,
    connection: ConnectionId,
    connection_figure: FigureId,
) {
    let decoration = runtime
        .figure(connection_figure)
        .expect("connection Figure")
        .add(Box::new(
            PolygonDecorationFigure::triangle()
                .with_fill_color(Color::BLACK)
                .with_stroke(Color::BLACK, 1.0),
        ))
        .expect("target decoration");
    runtime
        .connections()
        .set_locator(
            connection,
            decoration,
            Box::new(novadraw::connection::ConnectionLocator::Target),
        )
        .expect("target decoration locator");
}

#[cfg(test)]
mod tests {
    use novadraw::render::{BackendCapabilities, SurfaceInfo};

    use super::*;

    #[test]
    fn figure_composition_matches_both_official_panels() {
        let fixture = fixture();
        let root = fixture.runtime.tree().contents().expect("root");
        let root_children = fixture
            .runtime
            .tree()
            .child_order(root)
            .expect("root children");
        assert_eq!(root_children.len(), 3);
        assert_eq!(
            fixture.runtime.tree().figure_bounds(root_children[0]),
            Ok(Rectangle::new(0.0, 0.0, PANEL_WIDTH, PANEL_HEIGHT))
        );
        assert_eq!(
            fixture.runtime.tree().figure_bounds(root_children[1]),
            Ok(Rectangle::new(
                SECOND_PANEL_X,
                0.0,
                PANEL_WIDTH,
                PANEL_HEIGHT
            ))
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .child_order(root_children[0])
                .expect("midpoint panel")
                .len(),
            4
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .child_order(root_children[1])
                .expect("endpoint panel")
                .len(),
            4
        );
    }

    #[test]
    fn dragging_an_endpoint_moves_the_figure_and_reroutes_the_connection() {
        let mut fixture = fixture();
        let before_bounds = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.first_node)
            .expect("node bounds");
        let before_route = fixture
            .runtime
            .tree()
            .connection_route_points(fixture.first_connection.figure())
            .expect("route query")
            .expect("route")
            .clone();
        let start = before_bounds.center();
        fixture
            .runtime
            .input()
            .dispatch_mouse_pressed(start.x(), start.y(), MouseButton::Left);
        fixture
            .runtime
            .input()
            .dispatch_mouse_moved(start.x() + 30.0, start.y() + 20.0);
        fixture.runtime.input().dispatch_mouse_released(
            start.x() + 30.0,
            start.y() + 20.0,
            MouseButton::Left,
        );
        assert_eq!(
            fixture.runtime.tree().figure_bounds(fixture.first_node),
            Ok(Rectangle::new(
                before_bounds.x + 30.0,
                before_bounds.y + 20.0,
                before_bounds.width,
                before_bounds.height
            ))
        );

        let submission = fixture
            .runtime
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: WIDTH,
                    logical_height: HEIGHT,
                    pixel_width: WIDTH as u32,
                    pixel_height: HEIGHT as u32,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL,
            )
            .into_ready()
            .expect("frame");
        let after_route = fixture
            .runtime
            .tree()
            .connection_route_points(fixture.first_connection.figure())
            .expect("route query")
            .expect("route");
        assert_ne!(after_route, &before_route);
        assert!(!submission.commands.is_empty());
    }

    #[test]
    fn dragging_in_offset_panel_preserves_parent_local_bounds() {
        let mut fixture = fixture();
        let before = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.endpoint_node)
            .expect("endpoint node bounds");
        let start = Point::new(SECOND_PANEL_X + before.center().x(), before.center().y());

        fixture
            .runtime
            .input()
            .dispatch_mouse_pressed(start.x(), start.y(), MouseButton::Left);
        fixture
            .runtime
            .input()
            .dispatch_mouse_moved(start.x() + 12.0, start.y() + 8.0);
        fixture.runtime.input().dispatch_mouse_released(
            start.x() + 12.0,
            start.y() + 8.0,
            MouseButton::Left,
        );

        assert_eq!(
            fixture.runtime.tree().figure_bounds(fixture.endpoint_node),
            Ok(Rectangle::new(
                before.x + 12.0,
                before.y + 8.0,
                before.width,
                before.height,
            ))
        );
    }
}
