use novadraw::{
    connection::{
        ChopboxAnchor, ConnectionFigure, ConnectionLocator, CoordinateSpace, EndpointLocator,
        PolygonDecorationFigure, RouterBinding,
    },
    figure::{
        CONTAINER, ChildClippingStrategy, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureStyle, FlowPage,
        InlineTextFragment, LabelFigure, TextFlowFigure,
        border::{LineBorder, MarginBorder},
    },
    geometry::{Dimension, Insets, Point, PointList, Rectangle},
    graphics::{Color, GraphicsError, PaintContext, Path},
    layout::{StackLayout, ToolbarLayout, XYConstraint, XYLayout},
    runtime::Runtime,
    tree::FigureId,
};

use super::runtime_with_fonts;

const WIDTH: f64 = 500.0;
const HEIGHT: f64 = 300.0;
const CLASS_WIDTH: f64 = 142.0;
const CLASS_HEIGHT: f64 = 112.0;
const HEADER_HEIGHT: f64 = 30.0;
const COMPARTMENT_HEIGHT: f64 = 40.0;
const STICKY_WIDTH: f64 = 90.0;
const STICKY_HEIGHT: f64 = 90.0;
const CLASS_FILL: Color = Color::rgba(242.0 / 255.0, 240.0 / 255.0, 1.0, 1.0);
const STICKY_FILL: Color = Color::rgba(1.0, 1.0, 225.0 / 255.0, 1.0);

struct UmlClassFigure {
    bounds: Rectangle,
}

impl UmlClassFigure {
    fn new(bounds: Rectangle) -> Self {
        Self { bounds }
    }
}

impl Figure for UmlClassFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "UMLClassFigure"
    }

    fn initial_insets(&self) -> Insets {
        Insets::uniform(1.0)
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(CLASS_FILL),
            ..FigureStyle::default()
        }
    }

    fn intrinsic_size(&self) -> Dimension {
        Dimension::new(CLASS_WIDTH, CLASS_HEIGHT)
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(CLASS_FILL);
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
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for UmlClassFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

struct CompartmentFigure {
    bounds: Rectangle,
}

impl CompartmentFigure {
    fn new() -> Self {
        Self {
            bounds: Rectangle::new(0.0, 0.0, CLASS_WIDTH - 2.0, COMPARTMENT_HEIGHT),
        }
    }
}

impl Figure for CompartmentFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "CompartmentFigure"
    }

    fn initial_insets(&self) -> Insets {
        Insets::new(3.0, 5.0, 3.0, 5.0)
    }

    fn intrinsic_size(&self) -> Dimension {
        Dimension::new(self.bounds.width, self.bounds.height)
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_line(Point::new(0.0, 0.5), Point::new(bounds.width, 0.5))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for CompartmentFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

struct StickyNote {
    bounds: Rectangle,
}

impl StickyNote {
    fn new(bounds: Rectangle) -> Self {
        Self { bounds }
    }
}

impl Figure for StickyNote {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "StickyNote"
    }

    fn initial_insets(&self) -> Insets {
        Insets::uniform(3.0)
    }

    fn intrinsic_size(&self) -> Dimension {
        Dimension::new(STICKY_WIDTH, STICKY_HEIGHT)
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let fold = 6.0;
        let points = [
            Point::new(0.0, 0.0),
            Point::new(bounds.width - fold, 0.0),
            Point::new(bounds.width, fold),
            Point::new(bounds.width, bounds.height),
            Point::new(0.0, bounds.height),
        ];
        let mut path = Path::new();
        path.move_to(points[0].x(), points[0].y());
        for point in &points[1..] {
            path.line_to(point.x(), point.y());
        }
        path.close();
        context.set_fill_paint(STICKY_FILL);
        context.fill_path(&path)?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_path(&path)?;
        context.stroke_line(
            Point::new(bounds.width - fold, 0.0),
            Point::new(bounds.width - fold, fold),
        )?;
        context.stroke_line(
            Point::new(bounds.width - fold, fold),
            Point::new(bounds.width, fold),
        )
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for StickyNote {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    first_class: FigureId,
    second_class: FigureId,
    connection: FigureId,
    sticky_note: FigureId,
}

pub fn uml_class_diagram() -> Runtime {
    fixture().runtime
}

fn fixture() -> Fixture {
    let mut runtime = runtime_with_fonts();
    let class_image = register_class_image(&mut runtime);
    let root = runtime
        .set_contents(Box::new(novadraw::figure::RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("UMLClassDiagram root");
    runtime
        .figure(root)
        .expect("UMLClassDiagram root")
        .set_layout_manager(Box::new(XYLayout::new()))
        .expect("UMLClassDiagram XYLayout");

    let first_class = add_class(
        &mut runtime,
        root,
        Rectangle::new(20.0, 20.0, CLASS_WIDTH, CLASS_HEIGHT),
        class_image,
    );
    let second_class = add_class(
        &mut runtime,
        root,
        Rectangle::new(230.0, 102.0, CLASS_WIDTH, CLASS_HEIGHT),
        class_image,
    );

    let connection = runtime
        .figure(root)
        .expect("diagram")
        .add(Box::new(
            ConnectionFigure::new().with_stroke(Color::BLACK, 1.0),
        ))
        .expect("association");
    let source_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(second_class)));
    let target_anchor = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(first_class)));
    let router = runtime.connections().direct_router();
    let association = runtime
        .connections()
        .register_state(
            connection,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .expect("association state");

    let diamond = runtime
        .figure(connection)
        .expect("association")
        .add(Box::new(
            PolygonDecorationFigure::from_template(PointList::from_points(vec![
                Point::new(-2.0, 0.0),
                Point::new(-1.0, 1.0),
                Point::new(0.0, 0.0),
                Point::new(-1.0, -1.0),
            ]))
            .expect("official diamond template")
            .with_scale(7.0, 3.0)
            .expect("diamond scale")
            .with_fill_color(Color::BLACK)
            .with_stroke(Color::BLACK, 1.0),
        ))
        .expect("containment decoration");
    runtime
        .connections()
        .set_locator(association, diamond, Box::new(ConnectionLocator::Target))
        .expect("containment locator");

    let endpoint = add_label(
        &mut runtime,
        connection,
        "end1",
        Rectangle::new(0.0, 0.0, 42.0, 20.0),
        false,
    );
    runtime
        .connections()
        .set_locator(
            association,
            endpoint,
            Box::new(EndpointLocator::source(8.0, 0.0).expect("endpoint locator")),
        )
        .expect("end1 locator");
    let middle = add_label(
        &mut runtime,
        connection,
        "connection",
        Rectangle::new(0.0, 0.0, 78.0, 22.0),
        true,
    );
    runtime
        .connections()
        .set_locator(association, middle, Box::new(ConnectionLocator::Middle))
        .expect("middle locator");
    runtime
        .connections()
        .resolve_route(association, CoordinateSpace::ChildContent(root))
        .expect("association route");

    let sticky_note = runtime
        .figure(root)
        .expect("diagram")
        .add(Box::new(StickyNote::new(Rectangle::new(
            180.0,
            10.0,
            STICKY_WIDTH,
            STICKY_HEIGHT,
        ))))
        .expect("StickyNote");
    runtime
        .figure(sticky_note)
        .expect("StickyNote")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("StickyNote text layout");
    runtime
        .figure(sticky_note)
        .expect("StickyNote")
        .set_layout_constraint(XYConstraint::at_size(180.0, 10.0, STICKY_WIDTH, -1.0))
        .expect("StickyNote constraint");
    let text = TextFlowFigure::new(
        Rectangle::new(3.0, 3.0, STICKY_WIDTH - 6.0, STICKY_HEIGHT - 6.0),
        FlowPage::new(vec![novadraw::figure::FlowParagraph::new(vec![
            InlineTextFragment::new("This is a sticky note.  It wraps text based on its width."),
        ])]),
    );
    runtime
        .figure(sticky_note)
        .expect("StickyNote")
        .add(Box::new(text))
        .expect("StickyNote text");

    Fixture {
        runtime,
        first_class,
        second_class,
        connection,
        sticky_note,
    }
}

fn register_class_image(runtime: &mut Runtime) -> novadraw::runtime::ImageId {
    let image = runtime.resources().register_image().expect("class image");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("class image request");
    runtime
        .resources()
        .complete_image_request_bytes(request, include_bytes!("../../assets/class_obj.png"), 1.0)
        .expect("official class_obj image");
    image
}

fn add_class(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    image: novadraw::runtime::ImageId,
) -> FigureId {
    let class = runtime
        .figure(parent)
        .expect("diagram")
        .add(Box::new(UmlClassFigure::new(bounds)))
        .expect("UMLClassFigure");
    runtime
        .figure(class)
        .expect("UMLClassFigure")
        .set_layout_constraint(XYConstraint::at(bounds.x, bounds.y))
        .expect("UMLClassFigure constraint");
    runtime
        .figure(class)
        .expect("UMLClassFigure")
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .expect("UMLClassFigure ToolbarLayout");

    let header = runtime
        .figure(class)
        .expect("UMLClassFigure")
        .add(Box::new(
            LabelFigure::new("ClassName")
                .with_icon(image)
                .with_border(MarginBorder::default_border().with_margins(3.0, 5.0, 3.0, 5.0))
                .with_bounds(Rectangle::new(0.0, 0.0, CLASS_WIDTH - 2.0, HEADER_HEIGHT)),
        ))
        .expect("class header");
    runtime
        .figure(header)
        .expect("class header")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some("bold 10px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("class header style");

    add_compartment(runtime, class, &["name : String", "ID : String"]);
    add_compartment(runtime, class, &["foo() : int", "bar() : char"]);
    class
}

fn add_compartment(runtime: &mut Runtime, parent: FigureId, rows: &[&str]) {
    let compartment = runtime
        .figure(parent)
        .expect("UMLClassFigure")
        .add(Box::new(CompartmentFigure::new()))
        .expect("CompartmentFigure");
    runtime
        .figure(compartment)
        .expect("CompartmentFigure")
        .set_layout_manager(Box::new(
            ToolbarLayout::new().with_stretch_minor_axis(false),
        ))
        .expect("CompartmentFigure ToolbarLayout");
    for row in rows {
        let label = runtime
            .figure(compartment)
            .expect("CompartmentFigure")
            .add(Box::new(LabelFigure::new(*row).with_bounds(
                Rectangle::new(0.0, 0.0, CLASS_WIDTH - 12.0, 17.0),
            )))
            .expect("member label");
        runtime
            .figure(label)
            .expect("member label")
            .set_style(FigureStyle {
                foreground: Some(Color::BLACK),
                font: Some("10px Inter Variable".to_owned()),
                ..FigureStyle::default()
            })
            .expect("member label style");
    }
}

fn add_label(
    runtime: &mut Runtime,
    parent: FigureId,
    text: &str,
    bounds: Rectangle,
    bordered: bool,
) -> FigureId {
    let figure = if bordered {
        LabelFigure::new(text)
            .with_bounds(bounds)
            .with_border(LineBorder::default_border())
    } else {
        LabelFigure::new(text).with_bounds(bounds)
    };
    let label = runtime
        .figure(parent)
        .expect("label parent")
        .add(Box::new(figure))
        .expect("connection label");
    if bordered {
        runtime
            .figure(label)
            .expect("connection label")
            .set_opaque(true)
            .expect("label opaque");
    }
    runtime
        .figure(label)
        .expect("connection label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            background: bordered.then_some(Color::rgba(1.0, 1.0, 225.0 / 255.0, 1.0)),
            font: Some("12px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("connection label style");
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_note_height_is_measured_from_its_width() {
        let mut fixture = fixture();
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("stable layout");
        let before = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.sticky_note)
            .expect("sticky bounds");
        fixture
            .runtime
            .figure(fixture.sticky_note)
            .expect("sticky")
            .set_layout_constraint(XYConstraint::at_size(180.0, 10.0, 160.0, -1.0))
            .expect("wider sticky");
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("stable wider layout");
        let after = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.sticky_note)
            .expect("sticky bounds");
        assert_eq!(before.width, 90.0);
        assert_eq!(after.width, 160.0);
        assert!(after.height < before.height, "{before:?} -> {after:?}");
    }

    #[test]
    fn official_figure_composition_is_preserved() {
        let fixture = fixture();
        let root = fixture.runtime.tree().contents().expect("diagram");
        let children = fixture
            .runtime
            .tree()
            .child_order(root)
            .expect("diagram children");
        assert_eq!(children.len(), 4);
        assert_eq!(children[0], fixture.first_class);
        assert_eq!(children[1], fixture.second_class);
        assert_eq!(children[2], fixture.connection);
        assert_eq!(children[3], fixture.sticky_note);

        for class in [fixture.first_class, fixture.second_class] {
            let class_children = fixture
                .runtime
                .tree()
                .child_order(class)
                .expect("class children");
            assert_eq!(class_children.len(), 3);
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .node(class)
                    .expect("class")
                    .figure_name(),
                "UMLClassFigure"
            );
        }
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.sticky_note)
                .expect("StickyNote")
                .figure_name(),
            "StickyNote"
        );
    }

    #[test]
    fn association_has_decoration_and_both_locator_labels() {
        let fixture = fixture();
        let connection_children = fixture
            .runtime
            .tree()
            .child_order(fixture.connection)
            .expect("connection children");
        assert_eq!(connection_children.len(), 3);
        assert_eq!(
            fixture
                .runtime
                .tree()
                .connection_route_points(fixture.connection)
                .expect("route query")
                .expect("route")
                .len(),
            2
        );
    }

    #[test]
    fn connection_label_text_remeasures_without_moving_classes() {
        let mut fixture = fixture();
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("initial UML layout");
        let children = fixture
            .runtime
            .tree()
            .child_order(fixture.connection)
            .unwrap();
        let label = children[2];
        let before = fixture.runtime.tree().figure_bounds(label).unwrap();
        let route = fixture
            .runtime
            .tree()
            .connection_route_points(fixture.connection)
            .unwrap()
            .unwrap()
            .clone();
        let class_bounds = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.first_class)
            .unwrap();
        fixture
            .runtime
            .figure(label)
            .unwrap()
            .update_component(novadraw::runtime::SetLabelText::new(
                "a substantially longer connection label",
            ))
            .expect("label text mutation");
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("updated UML label");
        let after = fixture.runtime.tree().figure_bounds(label).unwrap();
        assert!(after.width > before.width);
        assert_eq!(after.center(), before.center());
        assert_eq!(
            fixture
                .runtime
                .tree()
                .figure_bounds(fixture.first_class)
                .unwrap(),
            class_bounds
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .connection_route_points(fixture.connection)
                .unwrap()
                .unwrap(),
            &route
        );
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("settled UML label");
        assert_eq!(fixture.runtime.tree().figure_bounds(label).unwrap(), after);
    }
}
