use novadraw::figure::{CursorIcon, FigureStyle, border::RectangleBorder};
use novadraw::graphics::{GraphicsError, PaintContext};
use novadraw::{figure::Figure, geometry::Rectangle};

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

struct StyleProbeFigure {
    bounds: Rectangle,
}

impl StyleProbeFigure {
    fn new(bounds: Rectangle) -> Self {
        Self { bounds }
    }
}

impl Figure for StyleProbeFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "StyleProbeFigure"
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_width(3.0)?;
        context.stroke_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }
}

fn bg_gray() -> novadraw::graphics::Color {
    novadraw::graphics::Color::rgba(0.85, 0.85, 0.85, 1.0)
}

fn gray_container() -> (novadraw::tree::FigureTree, novadraw::tree::FigureId) {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        bg_gray(),
    )
    .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");
    (scene, container_id)
}

fn create_scene_0_fill_colors() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let colors = [
        novadraw::graphics::Color::rgba(0.9, 0.2, 0.2, 1.0),
        novadraw::graphics::Color::rgba(0.2, 0.8, 0.2, 1.0),
        novadraw::graphics::Color::rgba(0.2, 0.2, 0.9, 1.0),
        novadraw::graphics::Color::rgba(0.9, 0.9, 0.2, 1.0),
    ];

    for (i, &color) in colors.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            50.0 + i as f64 * 185.0,
            200.0,
            160.0,
            120.0,
            color,
        )
        .expect("valid figure bounds");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_1_alpha() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let base_rect = novadraw::figure::RectangleFigure::new_with_color(
        100.0,
        150.0,
        200.0,
        200.0,
        novadraw::graphics::Color::WHITE,
    )
    .expect("valid figure bounds");
    scene
        .builder()
        .add_child(container_id, Box::new(base_rect))
        .expect("valid FigureTree construction");

    let alphas = [1.0, 0.75, 0.5, 0.25];
    for (i, &alpha) in alphas.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            200.0 + i as f64 * 120.0,
            200.0,
            160.0,
            120.0,
            novadraw::graphics::Color::rgba(0.9, 0.2, 0.2, alpha),
        )
        .expect("valid figure bounds");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_2_stroke_width() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let widths = [1.0, 2.0, 3.0, 4.0, 5.0];
    let stroke_color = novadraw::graphics::Color::rgba(0.2, 0.2, 0.2, 1.0);
    let fill_color = novadraw::graphics::Color::rgba(0.95, 0.95, 0.95, 1.0);

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            40.0 + i as f64 * 150.0,
            200.0,
            130.0,
            120.0,
            fill_color,
        )
        .expect("valid figure bounds")
        .with_stroke(stroke_color, width)
        .expect("valid detached figure input");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_3_stroke_color() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let stroke_colors = [
        novadraw::graphics::Color::rgba(0.9, 0.2, 0.2, 1.0),
        novadraw::graphics::Color::rgba(0.2, 0.7, 0.2, 1.0),
        novadraw::graphics::Color::rgba(0.2, 0.2, 0.9, 1.0),
        novadraw::graphics::Color::rgba(0.9, 0.5, 0.1, 1.0),
    ];
    let fill_color = novadraw::graphics::Color::rgba(0.95, 0.95, 0.95, 1.0);

    for (i, &stroke_color) in stroke_colors.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            50.0 + i as f64 * 185.0,
            200.0,
            160.0,
            120.0,
            fill_color,
        )
        .expect("valid figure bounds")
        .with_stroke(stroke_color, 3.0)
        .expect("valid detached figure input");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_4_line_cap() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let caps = [
        (
            novadraw::graphics::LineCap::Butt,
            novadraw::graphics::Color::rgba(0.9, 0.3, 0.3, 1.0),
        ),
        (
            novadraw::graphics::LineCap::Round,
            novadraw::graphics::Color::rgba(0.3, 0.9, 0.3, 1.0),
        ),
        (
            novadraw::graphics::LineCap::Square,
            novadraw::graphics::Color::rgba(0.3, 0.3, 0.9, 1.0),
        ),
    ];

    for (i, &(cap, color)) in caps.iter().enumerate() {
        let line = novadraw::figure::PolylineFigure::new_with_color(
            100.0,
            180.0 + i as f64 * 120.0,
            700.0,
            180.0 + i as f64 * 120.0,
            color,
        )
        .expect("valid detached figure input")
        .with_width(12.0)
        .expect("valid detached figure input")
        .with_cap(cap)
        .expect("valid outline geometry");
        scene
            .builder()
            .add_child(container_id, Box::new(line))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_5_line_join() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let joins = [
        (
            novadraw::graphics::LineJoin::Miter,
            novadraw::graphics::Color::rgba(1.0, 0.5, 0.0, 1.0),
        ),
        (
            novadraw::graphics::LineJoin::Round,
            novadraw::graphics::Color::rgba(0.0, 0.8, 0.8, 1.0),
        ),
        (
            novadraw::graphics::LineJoin::Bevel,
            novadraw::graphics::Color::rgba(0.8, 0.0, 0.8, 1.0),
        ),
    ];

    for (i, &(join, color)) in joins.iter().enumerate() {
        let base_x = 80.0 + i as f64 * 240.0;
        let base_y = 150.0;
        let line = novadraw::figure::PolylineFigure::from_points(vec![
            novadraw::geometry::Point::new(base_x, base_y + 200.0),
            novadraw::geometry::Point::new(base_x + 80.0, base_y),
            novadraw::geometry::Point::new(base_x + 160.0, base_y + 200.0),
        ])
        .expect("valid detached figure input")
        .with_width(10.0)
        .expect("valid detached figure input")
        .with_join(join)
        .expect("valid outline geometry")
        .with_color(color);
        scene
            .builder()
            .add_child(container_id, Box::new(line))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_6_stroke_vs_border() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();

    let fill_color = novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0);
    let stroke_color = novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0);
    let border_color = novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0);

    let widths = [2.0, 4.0, 8.0];

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            50.0,
            200.0,
            80.0,
            fill_color,
        )
        .expect("valid figure bounds")
        .with_stroke(stroke_color, width)
        .expect("valid detached figure input");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            200.0,
            200.0,
            80.0,
            fill_color,
        )
        .expect("valid figure bounds")
        .with_border(RectangleBorder::new(border_color, width).expect("valid border"))
        .expect("valid detached figure input");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    let inner_stroke_color = novadraw::graphics::Color::rgba(0.2, 0.6, 0.2, 1.0);
    let outer_border_color = novadraw::graphics::Color::rgba(0.8, 0.2, 0.2, 1.0);

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::figure::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            360.0,
            200.0,
            80.0,
            novadraw::graphics::Color::rgba(0.9, 0.95, 0.9, 1.0),
        )
        .expect("valid figure bounds")
        .with_stroke(inner_stroke_color, width)
        .expect("valid detached figure input")
        .with_border(
            RectangleBorder::new(outer_border_color, width)
                .expect("valid border")
                .with_insets(novadraw::geometry::Insets::uniform(8.0))
                .expect("valid insets"),
        )
        .expect("valid detached figure input");
        scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_7_inherited_figure_style() -> novadraw::tree::FigureTree {
    let (mut scene, container_id) = gray_container();
    let parent = scene
        .builder()
        .add_child(
            container_id,
            Box::new(StyleProbeFigure::new(Rectangle::new(
                80.0, 100.0, 640.0, 380.0,
            ))),
        )
        .expect("valid FigureTree construction");
    scene
        .builder()
        .set_figure_style(
            parent,
            FigureStyle {
                foreground: Some(novadraw::graphics::Color::BLACK),
                background: Some(novadraw::graphics::Color::rgba(0.25, 0.55, 0.85, 1.0)),
                alpha: Some(novadraw::graphics::Opacity::try_new(0.7).unwrap()),
                font: Some(novadraw::text::FontDescriptor::parse("20px sans-serif").unwrap()),
                cursor: Some(CursorIcon::Pointer),
                tooltip: Some(Some("Inherited parent tooltip".to_string())),
                layout_direction: None,
            },
        )
        .expect("valid FigureTree construction");

    scene
        .builder()
        .add_child(
            parent,
            Box::new(StyleProbeFigure::new(Rectangle::new(
                40.0, 100.0, 240.0, 150.0,
            ))),
        )
        .expect("valid FigureTree construction");
    let overridden = scene
        .builder()
        .add_child(
            parent,
            Box::new(StyleProbeFigure::new(Rectangle::new(
                360.0, 100.0, 240.0, 150.0,
            ))),
        )
        .expect("valid FigureTree construction");
    scene
        .builder()
        .set_figure_style(
            overridden,
            FigureStyle {
                foreground: Some(novadraw::graphics::Color::WHITE),
                background: Some(novadraw::graphics::Color::rgba(0.75, 0.2, 0.25, 1.0)),
                alpha: Some(novadraw::graphics::Opacity::OPAQUE),
                font: None,
                cursor: Some(CursorIcon::Crosshair),
                tooltip: Some(None),
                layout_direction: None,
            },
        )
        .expect("valid FigureTree construction");
    scene
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "style",
        "Styles",
        vec![
            SceneSpec::visual(
                "fill-colors",
                "Fill Colors",
                size,
                create_scene_0_fill_colors,
            ),
            SceneSpec::visual("alpha", "Alpha/Transparency", size, create_scene_1_alpha),
            SceneSpec::visual(
                "stroke-width",
                "Stroke Width",
                size,
                create_scene_2_stroke_width,
            ),
            SceneSpec::visual(
                "stroke-color",
                "Stroke Color",
                size,
                create_scene_3_stroke_color,
            ),
            SceneSpec::visual("line-cap", "LineCap", size, create_scene_4_line_cap),
            SceneSpec::visual("line-join", "LineJoin", size, create_scene_5_line_join),
            SceneSpec::visual(
                "stroke-vs-border",
                "Stroke vs Border",
                size,
                create_scene_6_stroke_vs_border,
            ),
            SceneSpec::visual(
                "inherited-figure-style",
                "Inherited FigureStyle",
                size,
                create_scene_7_inherited_figure_style,
            ),
        ],
    )
}
