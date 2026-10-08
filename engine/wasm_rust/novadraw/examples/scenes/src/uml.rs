//! A compound UML class diagram built only from Novadraw's public extension APIs.

use std::sync::Mutex;

use novadraw::connection::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionFigure,
    ConnectionLayerFigure, ConnectionLocator, CoordinateSpace, PathFractionLocator,
    PolygonDecorationFigure, PolylineDecorationFigure, RouterBinding, RoutingConstraint,
};
use novadraw::event::{EventContext, FigureEventHandler, MouseButton, MouseEvent};
use novadraw::figure::CursorIcon;
use novadraw::figure::{
    Alignment, CONTAINER, ChildClippingStrategy, ContainerCapability, FigureCapabilityBuilder,
    FigureCapabilityRegistrationError, FigureContainer, INPUT, InputCapability,
};
use novadraw::graphics::NdCanvas;
use novadraw::render::{BuiltinFont, DashPattern, LineJoin, StrokeStyle};
use novadraw::{
    Color, Figure, FigureId, FigureStyle, Insets, LabelFigure, Point, PointList, Rectangle,
    RectangleFigure, Runtime, ToolbarLayout,
};

use crate::{DemoSuite, SceneSpec};

const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;
const SCENE_SIZE: (u32, u32) = (WIDTH as u32, HEIGHT as u32);
const CLASS_WIDTH: f64 = 182.0;
const HEADER_HEIGHT: f64 = 38.0;
const MEMBER_HEIGHT: f64 = 20.0;
const COMPARTMENT_PADDING: f64 = 7.0;
const CLASS_LINE_WIDTH: f64 = 1.5;
const RELATION_LINE_WIDTH: f64 = 2.0;
const DECORATION_LENGTH: f64 = 14.0;
const DECORATION_HALF_WIDTH: f64 = 8.0;
const DECORATION_OVERLAP: f64 = 1.0;

const BACKGROUND: Color = Color::rgba(0.96, 0.97, 0.98, 1.0);
const CLASS_FILL: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const INTERFACE_FILL: Color = Color::rgba(0.92, 0.97, 1.0, 1.0);
const ENTITY_FILL: Color = Color::rgba(0.96, 0.98, 0.94, 1.0);
const VALUE_FILL: Color = Color::rgba(1.0, 0.97, 0.91, 1.0);
const INK: Color = Color::rgba(0.11, 0.14, 0.18, 1.0);
const MUTED: Color = Color::rgba(0.36, 0.40, 0.46, 1.0);
const ASSOCIATION: Color = Color::rgba(0.18, 0.27, 0.38, 1.0);
const DEPENDENCY: Color = Color::rgba(0.18, 0.42, 0.66, 1.0);
const COMPOSITION: Color = Color::rgba(0.46, 0.22, 0.12, 1.0);

struct UmlClassFigure {
    bounds: Rectangle,
    fill: Color,
    drag: Mutex<Option<UmlDragSession>>,
}

#[derive(Clone, Copy)]
struct UmlDragSession {
    start_entry: Point,
    start_bounds: Rectangle,
}

impl UmlClassFigure {
    fn new(bounds: Rectangle, fill: Color) -> Self {
        Self {
            bounds,
            fill,
            drag: Mutex::new(None),
        }
    }
}

impl Figure for UmlClassFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExampleUmlClassFigure"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            foreground: Some(INK),
            background: Some(self.fill),
            font: Some("13px Inter Variable".to_owned()),
            cursor: Some(CursorIcon::Move),
            ..FigureStyle::default()
        }
    }

    fn paint_figure_in_bounds(&self, canvas: &mut NdCanvas, bounds: Rectangle) {
        canvas.set_background_color(self.fill);
        canvas.fill_rectangle(0.0, 0.0, bounds.width, bounds.height);
        let stroke = if self.drag.lock().expect("drag state").is_some() {
            DEPENDENCY
        } else {
            INK
        };
        canvas.set_foreground_color(stroke);
        canvas
            .set_line_width(CLASS_LINE_WIDTH)
            .expect("class stroke is valid");
        canvas.draw_rectangle(0.0, 0.0, bounds.width, bounds.height);
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureContainer for UmlClassFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for UmlClassFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let local_bounds = context.target_bounds();
        let entry = event.entry_point();
        let start_bounds = Rectangle::new(
            entry.x() - event.x,
            entry.y() - event.y,
            local_bounds.width,
            local_bounds.height,
        );
        *self.drag.lock().expect("drag state") = Some(UmlDragSession {
            start_entry: entry,
            start_bounds,
        });
        context.repaint(None);
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let Some(session) = *self.drag.lock().expect("drag state") else {
            return false;
        };
        let entry = event.entry_point();
        let dx = entry.x() - session.start_entry.x();
        let dy = entry.y() - session.start_entry.y();
        let x = (session.start_bounds.x + dx).clamp(0.0, WIDTH - session.start_bounds.width);
        let y = (session.start_bounds.y + dy).clamp(0.0, HEIGHT - session.start_bounds.height);
        context.set_bounds_later(
            context.target_id(),
            Rectangle::new(
                x,
                y,
                session.start_bounds.width,
                session.start_bounds.height,
            ),
        );
        true
    }

    fn on_mouse_released(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let was_dragging = self.drag.lock().expect("drag state").take().is_some();
        if was_dragging {
            context.repaint(None);
        }
        was_dragging
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let was_dragging = self.drag.lock().expect("drag state").take().is_some();
        if was_dragging {
            context.repaint(None);
        }
        was_dragging
    }
}

#[derive(Clone)]
struct UmlCompartmentFigure {
    bounds: Rectangle,
}

impl UmlCompartmentFigure {
    fn new(width: f64, member_count: usize) -> Self {
        Self {
            bounds: Rectangle::new(
                0.0,
                0.0,
                width,
                member_count as f64 * MEMBER_HEIGHT + COMPARTMENT_PADDING * 2.0,
            ),
        }
    }
}

impl Figure for UmlCompartmentFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExampleUmlCompartmentFigure"
    }

    fn initial_insets(&self) -> Insets {
        Insets::uniform(COMPARTMENT_PADDING)
    }

    fn paint_figure_in_bounds(&self, canvas: &mut NdCanvas, bounds: Rectangle) {
        canvas.line_with_style(
            Point::new(0.0, CLASS_LINE_WIDTH / 2.0),
            Point::new(bounds.width, CLASS_LINE_WIDTH / 2.0),
            INK,
            StrokeStyle::default()
                .with_width(CLASS_LINE_WIDTH)
                .expect("compartment stroke is valid"),
        );
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for UmlCompartmentFigure {}

#[derive(Clone)]
struct UmlHeaderFigure {
    bounds: Rectangle,
}

impl UmlHeaderFigure {
    fn new(width: f64) -> Self {
        Self {
            bounds: Rectangle::new(0.0, 0.0, width, HEADER_HEIGHT),
        }
    }
}

impl Figure for UmlHeaderFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExampleUmlHeaderFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for UmlHeaderFigure {}

struct ClassSpec {
    stereotype: Option<&'static str>,
    name: &'static str,
    bounds: Rectangle,
    fill: Color,
    attributes: &'static [&'static str],
    methods: &'static [&'static str],
}

#[derive(Clone, Copy)]
enum RelationKind {
    Association,
    Dependency,
    Realization,
    Composition,
}

struct RelationSpec {
    source: usize,
    target: usize,
    kind: RelationKind,
    label: &'static str,
    label_fraction: f64,
    bendpoints: &'static [(f64, f64)],
}

pub struct UmlExample {
    pub runtime: Runtime,
    pub class_figures: Vec<FigureId>,
    pub connection_figures: Vec<FigureId>,
    pub relation_labels: Vec<FigureId>,
}

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "uml",
        "UML Extension",
        vec![SceneSpec::runtime_visual(
            "order-domain",
            "Order Domain UML",
            SCENE_SIZE,
            || build_example().runtime,
        )],
    )
}

pub fn build_example() -> UmlExample {
    let mut runtime = Runtime::empty();
    runtime
        .register_builtin_font(BuiltinFont::Inter)
        .expect("built-in font");
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0, 0.0, WIDTH, HEIGHT, BACKGROUND,
        )))
        .expect("valid UML root");
    runtime
        .figure(root)
        .expect("attached UML root")
        .set_style(FigureStyle {
            foreground: Some(INK),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("valid root style");

    add_label(
        &mut runtime,
        root,
        "Order Platform",
        Rectangle::new(28.0, 16.0, 330.0, 30.0),
        "22px Inter Variable",
        Alignment::Start,
        INK,
    );
    add_label(
        &mut runtime,
        root,
        "custom compound figures / nested layout / routed UML relations",
        Rectangle::new(28.0, 45.0, 520.0, 20.0),
        "12px Inter Variable",
        Alignment::Start,
        MUTED,
    );

    let connection_layer = runtime
        .container(root)
        .expect("root is a container")
        .add(Box::new(ConnectionLayerFigure::new(
            0.0, 0.0, WIDTH, HEIGHT,
        )))
        .expect("valid connection layer");

    let classes = [
        ClassSpec {
            stereotype: Some("<<interface>>"),
            name: "PaymentGateway",
            bounds: Rectangle::new(28.0, 86.0, CLASS_WIDTH, 142.0),
            fill: INTERFACE_FILL,
            attributes: &["provider: String"],
            methods: &["authorize(order): Auth", "capture(auth): Receipt"],
        },
        ClassSpec {
            stereotype: None,
            name: "OrderService",
            bounds: Rectangle::new(309.0, 76.0, CLASS_WIDTH, 162.0),
            fill: CLASS_FILL,
            attributes: &["gateway: PaymentGateway"],
            methods: &["placeOrder(cmd): Order", "cancel(id): Result"],
        },
        ClassSpec {
            stereotype: Some("<<interface>>"),
            name: "EventPublisher",
            bounds: Rectangle::new(590.0, 86.0, CLASS_WIDTH, 142.0),
            fill: INTERFACE_FILL,
            attributes: &["topic: String"],
            methods: &["publish(event): void", "flush(): Future"],
        },
        ClassSpec {
            stereotype: Some("<<entity>>"),
            name: "Customer",
            bounds: Rectangle::new(28.0, 362.0, CLASS_WIDTH, 152.0),
            fill: ENTITY_FILL,
            attributes: &["id: CustomerId", "email: Email"],
            methods: &["changeEmail(value): void"],
        },
        ClassSpec {
            stereotype: Some("<<aggregate>>"),
            name: "Order",
            bounds: Rectangle::new(309.0, 326.0, CLASS_WIDTH, 196.0),
            fill: VALUE_FILL,
            attributes: &["id: OrderId", "status: OrderStatus", "total: Money"],
            methods: &["add(item): void", "confirm(): Event", "cancel(): Event"],
        },
        ClassSpec {
            stereotype: Some("<<entity>>"),
            name: "LineItem",
            bounds: Rectangle::new(590.0, 362.0, CLASS_WIDTH, 162.0),
            fill: ENTITY_FILL,
            attributes: &["sku: Sku", "quantity: u32"],
            methods: &["subtotal(): Money"],
        },
    ];
    let class_figures = classes
        .iter()
        .map(|spec| add_class(&mut runtime, root, spec))
        .collect::<Vec<_>>();

    let relations = [
        RelationSpec {
            source: 1,
            target: 0,
            kind: RelationKind::Dependency,
            label: "uses",
            label_fraction: 0.5,
            bendpoints: &[],
        },
        RelationSpec {
            source: 1,
            target: 2,
            kind: RelationKind::Realization,
            label: "publishes",
            label_fraction: 0.4,
            bendpoints: &[],
        },
        RelationSpec {
            source: 1,
            target: 4,
            kind: RelationKind::Association,
            label: "creates",
            label_fraction: 0.5,
            bendpoints: &[],
        },
        RelationSpec {
            source: 3,
            target: 4,
            kind: RelationKind::Association,
            label: "places 0..*",
            label_fraction: 0.5,
            bendpoints: &[],
        },
        RelationSpec {
            source: 4,
            target: 5,
            kind: RelationKind::Composition,
            label: "items 1..*",
            label_fraction: 0.6,
            bendpoints: &[],
        },
        RelationSpec {
            source: 3,
            target: 1,
            kind: RelationKind::Dependency,
            label: "submits",
            label_fraction: 0.5,
            bendpoints: &[(250.0, 304.0), (260.0, 176.0)],
        },
    ];

    let mut connection_figures = Vec::with_capacity(relations.len());
    let mut relation_labels = Vec::with_capacity(relations.len());
    for relation in relations {
        let (connection, label) = add_relation(
            &mut runtime,
            root,
            connection_layer,
            class_figures[relation.source],
            class_figures[relation.target],
            &relation,
        );
        connection_figures.push(connection);
        relation_labels.push(label);
    }

    add_legend(&mut runtime, root);

    UmlExample {
        runtime,
        class_figures,
        connection_figures,
        relation_labels,
    }
}

fn add_class(runtime: &mut Runtime, root: FigureId, spec: &ClassSpec) -> FigureId {
    let class = runtime
        .container(root)
        .expect("root is a container")
        .add(Box::new(UmlClassFigure::new(spec.bounds, spec.fill)))
        .expect("valid UML class");
    runtime
        .container(class)
        .expect("UML class is a container")
        .set_layout_manager(Box::new(ToolbarLayout::new().with_stretch_minor_axis(true)))
        .expect("valid class layout");

    let header = runtime
        .container(class)
        .expect("class is a container")
        .add(Box::new(UmlHeaderFigure::new(spec.bounds.width)))
        .expect("valid UML header");
    runtime
        .container(header)
        .expect("header is a container")
        .set_layout_manager(Box::new(ToolbarLayout::new().with_stretch_minor_axis(true)))
        .expect("valid header layout");
    if let Some(stereotype) = spec.stereotype {
        add_label(
            runtime,
            header,
            stereotype,
            Rectangle::new(0.0, 0.0, spec.bounds.width, 16.0),
            "10px Inter Variable",
            Alignment::Center,
            MUTED,
        );
    }
    add_label(
        runtime,
        header,
        spec.name,
        Rectangle::new(0.0, 0.0, spec.bounds.width, 20.0),
        "14px Inter Variable",
        Alignment::Center,
        INK,
    );
    add_compartment(runtime, class, spec.bounds.width, spec.attributes);
    add_compartment(runtime, class, spec.bounds.width, spec.methods);
    class
}

fn add_compartment(
    runtime: &mut Runtime,
    class: FigureId,
    width: f64,
    members: &[&str],
) -> FigureId {
    let compartment = runtime
        .container(class)
        .expect("class is a container")
        .add(Box::new(UmlCompartmentFigure::new(width, members.len())))
        .expect("valid UML compartment");
    for (index, member) in members.iter().enumerate() {
        add_label(
            runtime,
            compartment,
            member,
            Rectangle::new(
                0.0,
                index as f64 * MEMBER_HEIGHT,
                width - COMPARTMENT_PADDING * 2.0,
                MEMBER_HEIGHT,
            ),
            "12px Inter Variable",
            Alignment::Start,
            INK,
        );
    }
    compartment
}

fn add_relation(
    runtime: &mut Runtime,
    routing_root: FigureId,
    connection_layer: FigureId,
    source: FigureId,
    target: FigureId,
    spec: &RelationSpec,
) -> (FigureId, FigureId) {
    let color = match spec.kind {
        RelationKind::Association => ASSOCIATION,
        RelationKind::Dependency | RelationKind::Realization => DEPENDENCY,
        RelationKind::Composition => COMPOSITION,
    };
    let dash = match spec.kind {
        RelationKind::Dependency | RelationKind::Realization => DashPattern::Dash,
        RelationKind::Association | RelationKind::Composition => DashPattern::Solid,
    };
    let stroke = StrokeStyle::default()
        .with_width(RELATION_LINE_WIDTH)
        .expect("relation width is valid")
        .with_dash_pattern(dash)
        .with_join(LineJoin::Miter);
    let has_target_decoration = matches!(
        spec.kind,
        RelationKind::Dependency | RelationKind::Realization
    );
    let has_source_decoration = matches!(spec.kind, RelationKind::Composition);
    let connection = runtime
        .container(connection_layer)
        .expect("connection layer is a container")
        .add(Box::new(
            ConnectionFigure::new()
                .with_stroke(color, RELATION_LINE_WIDTH)
                .with_stroke_style(stroke)
                .with_decoration_insets(
                    if has_source_decoration {
                        DECORATION_LENGTH - DECORATION_OVERLAP
                    } else {
                        0.0
                    },
                    if has_target_decoration {
                        DECORATION_LENGTH - DECORATION_OVERLAP
                    } else {
                        0.0
                    },
                ),
        ))
        .expect("valid UML relation");

    let mut source_decoration = None;
    let mut target_decoration = None;
    if has_source_decoration {
        source_decoration = Some(
            runtime
                .container(connection)
                .expect("connection is a container")
                .add(Box::new(
                    PolygonDecorationFigure::from_template(PointList::from_points(vec![
                        Point::ZERO,
                        Point::new(-DECORATION_LENGTH / 2.0, -DECORATION_HALF_WIDTH),
                        Point::new(-DECORATION_LENGTH, 0.0),
                        Point::new(-DECORATION_LENGTH / 2.0, DECORATION_HALF_WIDTH),
                    ]))
                    .expect("valid diamond")
                    .with_fill_color(INK)
                    .with_stroke(INK, 1.5)
                    .with_join(LineJoin::Miter),
                ))
                .expect("valid composition decoration"),
        );
    }
    if has_target_decoration {
        target_decoration = Some(match spec.kind {
            RelationKind::Realization => runtime
                .container(connection)
                .expect("connection is a container")
                .add(Box::new(
                    PolygonDecorationFigure::from_template(PointList::from_points(vec![
                        Point::ZERO,
                        Point::new(-DECORATION_LENGTH, -DECORATION_HALF_WIDTH),
                        Point::new(-DECORATION_LENGTH, DECORATION_HALF_WIDTH),
                    ]))
                    .expect("valid realization triangle")
                    .with_fill_color(BACKGROUND)
                    .with_stroke(color, 1.5)
                    .with_join(LineJoin::Miter),
                ))
                .expect("valid realization decoration"),
            RelationKind::Dependency => {
                let decoration = PolylineDecorationFigure::arrow()
                    .with_scale(1.2, 1.2)
                    .expect("valid dependency arrow scale")
                    .with_color(color)
                    .with_width(RELATION_LINE_WIDTH)
                    .with_join(LineJoin::Miter);
                runtime
                    .container(connection)
                    .expect("connection is a container")
                    .add(Box::new(decoration))
                    .expect("valid dependency decoration")
            }
            RelationKind::Association | RelationKind::Composition => unreachable!(),
        });
    }

    let label = add_relation_label(runtime, connection, spec.label);
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = if spec.bendpoints.is_empty() {
        runtime.direct_connection_router()
    } else {
        runtime.register_connection_router(Box::new(BendpointConnectionRouter))
    };
    let constraint = (!spec.bendpoints.is_empty()).then(|| {
        Box::new(BendpointConstraint::new(
            spec.bendpoints
                .iter()
                .map(|(x, y)| Bendpoint::Absolute(Point::new(*x, *y)))
                .collect::<Vec<_>>(),
        )) as Box<dyn RoutingConstraint>
    });
    let relation = runtime
        .register_connection_state(
            connection,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            constraint,
        )
        .expect("valid relation state");
    if let Some(decoration) = source_decoration {
        runtime
            .set_connection_locator(relation, decoration, Box::new(ConnectionLocator::Source))
            .expect("valid source decoration locator");
    }
    if let Some(decoration) = target_decoration {
        runtime
            .set_connection_locator(relation, decoration, Box::new(ConnectionLocator::Target))
            .expect("valid target decoration locator");
    }
    runtime
        .set_connection_locator(
            relation,
            label,
            Box::new(
                PathFractionLocator::new(spec.label_fraction)
                    .expect("relation label fraction is valid"),
            ),
        )
        .expect("valid relation label locator");
    runtime
        .resolve_connection_route(relation, CoordinateSpace::ChildContent(routing_root))
        .expect("valid UML relation route");
    (connection, label)
}

fn add_relation_label(runtime: &mut Runtime, connection: FigureId, text: &str) -> FigureId {
    let width = (text.chars().count() as f64 * 7.0 + 14.0).max(48.0);
    let badge = runtime
        .container(connection)
        .expect("connection is a container")
        .add(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, width, 22.0, BACKGROUND)
                .with_stroke(BACKGROUND, 3.0),
        ))
        .expect("valid relation badge");
    add_label(
        runtime,
        badge,
        text,
        Rectangle::new(0.0, 0.0, width, 22.0),
        "11px Inter Variable",
        Alignment::Center,
        MUTED,
    );
    badge
}

fn add_label(
    runtime: &mut Runtime,
    parent: FigureId,
    text: &str,
    bounds: Rectangle,
    font: &str,
    alignment: Alignment,
    color: Color,
) -> FigureId {
    let mut figure = LabelFigure::new(text).with_bounds(bounds);
    figure.set_label_alignment(alignment);
    figure.set_text_alignment(alignment);
    let label = runtime
        .container(parent)
        .expect("label parent is a container")
        .add(Box::new(figure))
        .expect("valid label");
    runtime
        .figure(label)
        .expect("attached label")
        .set_style(FigureStyle {
            foreground: Some(color),
            font: Some(font.to_owned()),
            ..FigureStyle::default()
        })
        .expect("valid label style");
    label
}

fn add_legend(runtime: &mut Runtime, root: FigureId) {
    add_label(
        runtime,
        root,
        "solid: association/composition    dashed: dependency/realization",
        Rectangle::new(28.0, 558.0, 600.0, 20.0),
        "11px Inter Variable",
        Alignment::Start,
        MUTED,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::render::{BackendCapabilities, RenderCommandKind, SurfaceInfo};

    #[test]
    fn complex_uml_scene_uses_external_figures_nested_layout_and_routed_relations() {
        let mut example = build_example();

        assert_eq!(example.class_figures.len(), 6);
        assert_eq!(example.connection_figures.len(), 6);
        assert_eq!(example.relation_labels.len(), 6);
        for class in &example.class_figures {
            let node = example
                .runtime
                .tree()
                .node(*class)
                .expect("class is attached");
            assert_eq!(node.figure_name(), "ExampleUmlClassFigure");
            assert_eq!(node.children_count(), 3);
        }
        for connection in &example.connection_figures {
            assert!(
                example
                    .runtime
                    .tree()
                    .connection_route_points(*connection)
                    .is_some_and(|points| points.len() >= 2)
            );
        }

        let dragged = example.class_figures[1];
        let before_bounds = example
            .runtime
            .tree()
            .figure_bounds(dragged)
            .expect("dragged class bounds");
        let before_route = example
            .runtime
            .tree()
            .connection_route_points(example.connection_figures[0])
            .expect("connected relation route")
            .clone();
        let start = Point::new(
            before_bounds.x + before_bounds.width / 2.0,
            before_bounds.y + 18.0,
        );
        let press = example
            .runtime
            .dispatch_mouse_pressed(start.x(), start.y(), MouseButton::Left);
        assert_eq!(press.target(), Some(dragged));
        assert_eq!(press.capture(), Some(dragged));

        let delta = Point::new(48.0, 36.0);
        example
            .runtime
            .dispatch_mouse_moved(start.x() + delta.x(), start.y() + delta.y());
        let after_bounds = example
            .runtime
            .tree()
            .figure_bounds(dragged)
            .expect("moved class bounds");
        assert_eq!(
            after_bounds,
            Rectangle::new(
                before_bounds.x + delta.x(),
                before_bounds.y + delta.y(),
                before_bounds.width,
                before_bounds.height,
            )
        );
        assert!(example.runtime.dirty_connections().len() >= 4);
        let release = example.runtime.dispatch_mouse_released(
            start.x() + delta.x(),
            start.y() + delta.y(),
            MouseButton::Left,
        );
        assert!(release.is_handled());
        assert_eq!(release.capture(), None);
        assert!(example.runtime.take_deferred_mutation_errors().is_empty());

        let submission = example
            .runtime
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
            .expect("UML scene prepares a frame");
        assert_ne!(
            example
                .runtime
                .tree()
                .connection_route_points(example.connection_figures[0])
                .expect("rerouted relation"),
            &before_route
        );

        let glyph_runs = submission
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count();
        let polylines = submission
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::Polyline { .. }))
            .count();
        assert_eq!(glyph_runs, 41, "expected every UML label to be recorded");
        assert!(
            polylines >= 6,
            "expected one painted polyline per routed relation, got {polylines}"
        );
    }
}
