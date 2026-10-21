//! Port of zoom/ZoomExample.java, UMLClassFigure.java and CompartmentFigure.java.
use std::sync::Arc;

use novadraw::{
    container::{
        DefaultRangeModel, RangeModel, ScalableLayeredPaneFigure, ScrollBarConfig, ScrollBarFigure,
        ScrollBarPart, ScrollBarValueConsumer, ScrollOrientation,
    },
    event::EventContext,
    figure::{
        Alignment, Border, CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureStyle, LabelFigure,
        RectangleFigure,
        border::{EtchedBorder, LineBorder},
    },
    geometry::{Dimension, Insets, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::{MinorAlignment, StackLayout, ToolbarLayout, XYConstraint, XYLayout},
    runtime::{ImageId, Runtime},
    text::{FontDescriptor, TextConstraints, TextLayout},
    tree::FigureId,
};

use super::runtime_with_fonts;

const WINDOW_SIZE: f64 = 800.0;
const BAR_HEIGHT: f64 = 24.0;
const ARROW_WIDTH: f64 = 16.0;
const MINIMUM_THUMB: f64 = 70.0;
const MAXIMUM: f64 = 200.0;
const EXTENT: f64 = 25.0;
const CLASS_COLOR: Color = Color::rgba(1.0, 1.0, 206.0 / 255.0, 1.0);

struct GroupBoxBorder {
    title: TextLayout,
}

impl Border for GroupBoxBorder {
    fn get_insets(&self) -> Insets {
        Insets::uniform(f64::from(self.title.height()))
    }
    fn preferred_size(&self) -> Dimension {
        Dimension::new(
            f64::from(self.title.width() + 2.0 * self.title.height()),
            f64::from(self.title.height()),
        )
    }
    fn paint(
        &self,
        bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        let height = f64::from(self.title.height());
        EtchedBorder::new(Color::WHITE, Color::rgba(0.5, 0.5, 0.5, 1.0))
            .paint(bounds.inflate(-height / 2.0, -height / 2.0), context)?;
        context.set_fill_paint(Color::WHITE);
        context.fill_rect(Rectangle::new(
            height,
            0.0,
            f64::from(self.title.width()),
            height,
        ))?;
        context.set_fill_paint(Color::BLACK);
        context.fill_text(&self.title, Point::new(height, 0.0))
    }
}

struct SharedGroupBorder(Arc<GroupBoxBorder>);

impl Border for SharedGroupBorder {
    fn get_insets(&self) -> Insets {
        self.0.get_insets()
    }
    fn paint(
        &self,
        bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.0.paint(bounds, context)
    }
    fn preferred_size(&self) -> Dimension {
        self.0.preferred_size()
    }
}

struct ZoomValue(FigureId);
impl ScrollBarValueConsumer for ZoomValue {
    fn value_changed(&self, _: f64, value: f64, context: &mut EventContext<'_>) {
        context.effects().set_scale_later(self.0, zoom_for(value));
    }
}

struct CompartmentFigure;

impl Figure for CompartmentFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }

    fn name(&self) -> &'static str {
        "CompartmentFigure"
    }

    fn initial_insets(&self) -> Insets {
        Insets::new(1.0, 0.0, 0.0, 0.0)
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

impl FigureContainer for CompartmentFigure {}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    panel: FigureId,
    bar: FigureId,
    classes: [FigureId; 2],
    model: Arc<DefaultRangeModel>,
}

pub fn zoom_example() -> Runtime {
    fixture().runtime
}

fn fixture() -> Fixture {
    let mut runtime = runtime_with_fonts();
    let images = [
        register_image(
            &mut runtime,
            include_bytes!("../../assets/zoom/class_obj.png"),
        ),
        register_image(
            &mut runtime,
            include_bytes!("../../assets/zoom/field_private_obj.png"),
        ),
        register_image(
            &mut runtime,
            include_bytes!("../../assets/zoom/methpub_obj.png"),
        ),
    ];
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_SIZE,
            WINDOW_SIZE,
            Color::WHITE,
        )))
        .expect("ZoomExample root");
    let title = runtime
        .text()
        .layout(
            "Zooming figure",
            &FontDescriptor::parse("12px Inter Variable").expect("group font"),
            TextConstraints::UNBOUNDED,
        )
        .expect("group title");
    let border = Arc::new(GroupBoxBorder { title });
    let panel = runtime
        .figure(root)
        .expect("root")
        .add(Box::new(
            ScalableLayeredPaneFigure::new(0.0, BAR_HEIGHT, 500.0, 500.0)
                .with_border(SharedGroupBorder(border)),
        ))
        .expect("ZoomContainer");
    runtime
        .figure(panel)
        .expect("panel")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("StackLayout");
    runtime
        .figure(panel)
        .expect("panel")
        .set_preferred_size(Dimension::new(500.0, 500.0))
        .expect("panel preferred");
    runtime
        .figure(panel)
        .expect("panel")
        .set_minimum_size(Dimension::new(5.0, 5.0))
        .expect("panel minimum");

    let contents = runtime
        .figure(panel)
        .expect("panel")
        .add(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            500.0,
            500.0,
            Color::WHITE,
        )))
        .expect("contents");
    runtime
        .figure(contents)
        .expect("contents")
        .set_layout_manager(Box::new(XYLayout::new()))
        .expect("XYLayout");
    let table = add_class(
        &mut runtime,
        contents,
        "Table",
        (10.0, 10.0),
        ["columns: Column[]", "rows: Row[]"],
        ["getColumns(): Column[]", "getRows(): Row[]"],
        images,
    );
    let column = add_class(
        &mut runtime,
        contents,
        "Column",
        (150.0, 150.0),
        ["columnID: int", "items: List"],
        ["getColumnID(): int", "getItems(): List"],
        images,
    );
    let model = Arc::new(DefaultRangeModel::new(0.0, EXTENT, MAXIMUM).expect("zoom range"));
    model.set_value(40.0).expect("initial value");
    let scrollbar = ScrollBarFigure::new(model.clone())
        .with_bounds(Rectangle::new(0.0, 0.0, WINDOW_SIZE, BAR_HEIGHT))
        .with_config(ScrollBarConfig {
            orientation: ScrollOrientation::Horizontal,
            thickness: BAR_HEIGHT,
            button_length: ARROW_WIDTH,
            minimum_thumb_length: MINIMUM_THUMB,
            ..ScrollBarConfig::default()
        })
        .expect("Zoom ScrollBar config")
        .with_consumer(Arc::new(ZoomValue(panel)))
        .with_part(
            ScrollBarPart::Thumb,
            Box::new(LabelFigure::new("< Zoom >").with_border(LineBorder::default_border())),
        )
        .into_subtree();
    let receipt = runtime
        .figure(root)
        .expect("root")
        .add_subtree(scrollbar.subtree)
        .expect("ScrollBar");
    let bar = receipt.figure(scrollbar.root).expect("ScrollBar root");
    runtime
        .figure(root)
        .expect("root")
        .move_child_to_index(bar, 0)
        .expect("official bar order");
    runtime
        .figure(root)
        .expect("root")
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .expect("ToolbarLayout");
    Fixture {
        runtime,
        panel,
        bar,
        classes: [table, column],
        model,
    }
}

fn add_class(
    runtime: &mut Runtime,
    parent: FigureId,
    name: &str,
    position: (f64, f64),
    attributes: [&str; 2],
    methods: [&str; 2],
    images: [ImageId; 3],
) -> FigureId {
    let class = runtime
        .figure(parent)
        .expect("contents")
        .add(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, CLASS_COLOR)
                .with_border(LineBorder::default_border()),
        ))
        .expect("UMLClassFigure");
    runtime
        .figure(class)
        .expect("class")
        .set_layout_constraint(XYConstraint::at(position.0, position.1))
        .expect("class constraint");
    runtime
        .figure(class)
        .expect("class")
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .expect("class ToolbarLayout");
    let header = runtime
        .figure(class)
        .expect("class")
        .add(Box::new(LabelFigure::new(name).with_icon(images[0])))
        .expect("header");
    runtime
        .figure(header)
        .expect("header")
        .set_style(FigureStyle {
            font: Some("bold 16px Arial".to_owned()),
            ..FigureStyle::default()
        })
        .expect("header font");
    for (rows, image) in [(attributes, images[1]), (methods, images[2])] {
        let compartment = runtime
            .figure(class)
            .expect("class")
            .add(Box::new(CompartmentFigure))
            .expect("compartment");
        runtime
            .figure(compartment)
            .expect("compartment")
            .set_layout_manager(Box::new(
                ToolbarLayout::new()
                    .with_minor_alignment(MinorAlignment::Start)
                    .with_stretch_minor_axis(false)
                    .with_spacing(2.0),
            ))
            .expect("compartment ToolbarLayout");
        for row in rows {
            let mut label = LabelFigure::new(row).with_icon(image);
            label.set_label_alignment(Alignment::Start);
            runtime
                .figure(compartment)
                .expect("compartment")
                .add(Box::new(label))
                .expect("member Label");
        }
    }
    class
}

fn register_image(runtime: &mut Runtime, bytes: &[u8]) -> ImageId {
    let image = runtime
        .resources()
        .register_image()
        .expect("official image");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("image request");
    runtime
        .resources()
        .complete_image_request_bytes(request, bytes, 1.0)
        .expect("official PNG");
    image
}

fn zoom_for(value: f64) -> f64 {
    // Java computes in float; preserve its rounding before passing the neutral API an f64.
    f64::from((value as f32 + 10.0) * 0.02_f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::event::MouseButton;

    #[test]
    fn official_two_class_fixture_and_scroll_range_are_preserved() {
        let fixture = fixture();
        for class in fixture.classes {
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .child_order(class)
                    .expect("class children")
                    .len(),
                3
            );
            assert!(
                fixture
                    .runtime
                    .tree()
                    .layout_constraint::<XYConstraint>(class)
                    .expect("constraint")
                    .is_some()
            );
        }
        assert_eq!(fixture.model.minimum(), 0.0);
        assert_eq!(fixture.model.maximum(), 200.0);
        assert_eq!(fixture.model.extent(), 25.0);
        assert_eq!(fixture.model.value(), 40.0);
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.bar)
                .expect("bar")
                .figure_name(),
            "ScrollBar"
        );
    }

    #[test]
    fn repeated_zoom_keeps_the_bar_height_and_unscaled_content_visible() {
        let mut fixture = fixture();
        fixture
            .runtime
            .frames()
            .stabilize_for_query()
            .expect("initial layout");
        let contents = fixture.runtime.tree().child_order(fixture.panel).unwrap()[0];
        let class_bounds = fixture
            .classes
            .map(|id| fixture.runtime.tree().figure_bounds(id).unwrap());
        for value in [30.0, 20.0, 10.0, 0.0, 40.0, 80.0, 120.0, 175.0, 0.0] {
            let scale = zoom_for(value);
            fixture
                .runtime
                .viewports()
                .set_scale(fixture.panel, scale)
                .unwrap();
            fixture
                .runtime
                .frames()
                .stabilize_for_query()
                .expect("zoom layout");
            let bar = fixture.runtime.tree().figure_bounds(fixture.bar).unwrap();
            assert_eq!(bar.height, BAR_HEIGHT, "scale={scale}");
            let panel = fixture.runtime.tree().figure_bounds(fixture.panel).unwrap();
            let insets = fixture
                .runtime
                .tree()
                .node(fixture.panel)
                .unwrap()
                .state()
                .insets();
            let bounds = fixture.runtime.tree().figure_bounds(contents).unwrap();
            assert!(
                (bounds.width * scale - (panel.width - insets.width())).abs() < 1e-8,
                "scale={scale}, content={bounds:?}, panel={panel:?}"
            );
            assert!((bounds.height * scale - (panel.height - insets.height())).abs() < 1e-8);
            for (id, expected) in fixture.classes.into_iter().zip(class_bounds) {
                assert_eq!(fixture.runtime.tree().figure_bounds(id).unwrap(), expected);
                if scale <= 1.0 {
                    assert!(
                        expected.x >= bounds.x
                            && expected.y >= bounds.y
                            && expected.x + expected.width <= bounds.x + bounds.width
                            && expected.y + expected.height <= bounds.y + bounds.height,
                        "scale={scale}: class {expected:?} clipped by {bounds:?}"
                    );
                }
            }
        }
        for (increase, expected) in [(false, 0.0), (true, MAXIMUM - EXTENT), (false, 0.0)] {
            for _ in 0..20 {
                let bounds = fixture.runtime.tree().figure_bounds(fixture.bar).unwrap();
                let x = if increase {
                    bounds.width - ARROW_WIDTH / 2.0
                } else {
                    ARROW_WIDTH / 2.0
                };
                click(&mut fixture.runtime, x, BAR_HEIGHT / 2.0);
                fixture.runtime.frames().stabilize_for_query().unwrap();
                assert_eq!(
                    fixture
                        .runtime
                        .tree()
                        .figure_bounds(fixture.bar)
                        .unwrap()
                        .height,
                    BAR_HEIGHT
                );
            }
            assert_eq!(fixture.model.value(), expected);
        }
    }

    #[test]
    fn arrow_page_and_drag_apply_the_official_zoom_formula() {
        let mut fixture = fixture();
        let width = WINDOW_SIZE;
        click(&mut fixture.runtime, width - 4.0, BAR_HEIGHT / 2.0);
        assert_eq!(fixture.model.value(), 50.0);
        let scale = fixture
            .runtime
            .viewports()
            .scale_handle(fixture.panel)
            .expect("scale");
        assert_eq!(scale.scale(), zoom_for(50.0));
        click(
            &mut fixture.runtime,
            width - 2.0 * ARROW_WIDTH,
            BAR_HEIGHT / 2.0,
        );
        assert_eq!(fixture.model.value(), 100.0);
        let track = width - 2.0 * ARROW_WIDTH;
        let length = (track * EXTENT / MAXIMUM).max(MINIMUM_THUMB);
        let x = ARROW_WIDTH + 100.0 / (MAXIMUM - EXTENT) * (track - length) + length / 2.0;
        fixture
            .runtime
            .input()
            .dispatch_mouse_pressed(x, BAR_HEIGHT / 2.0, MouseButton::Left);
        fixture
            .runtime
            .input()
            .dispatch_mouse_moved(x + 60.0, BAR_HEIGHT / 2.0);
        fixture.runtime.input().dispatch_mouse_released(
            x + 60.0,
            BAR_HEIGHT / 2.0,
            MouseButton::Left,
        );
        assert!(fixture.model.value() > 100.0);
        assert_eq!(scale.scale(), zoom_for(fixture.model.value()));
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    fn click(runtime: &mut Runtime, x: f64, y: f64) {
        runtime
            .input()
            .dispatch_mouse_pressed(x, y, MouseButton::Left);
        runtime
            .input()
            .dispatch_mouse_released(x, y, MouseButton::Left);
    }
}
