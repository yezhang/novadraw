use novadraw::{
    figure::{
        EdgeStroke, LabelFigure, PolygonFigure, RectangleCorners, RectangleEdges, RectangleFigure,
        RoundedRectangleFigure, TriangleFigure,
    },
    geometry::{Point, Rectangle},
    graphics::Color,
    runtime::Runtime,
};
use novadraw_example_support::{run_demo_app, run_demo_app_with_scene_screenshot};

const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;
const CARD_WIDTH: f64 = 300.0;
const CARD_HEIGHT: f64 = 240.0;
const TITLE_HEIGHT: f64 = 52.0;
const RADIUS: f64 = 20.0;
const BORDER_WIDTH: f64 = 3.0;
type Scene = (&'static str, Box<dyn FnMut() -> Runtime>);

fn scene() -> Runtime {
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(novadraw::render::BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(
                0.0,
                0.0,
                WIDTH,
                HEIGHT,
                Color::rgba8(242, 245, 250, 255),
            )
            .expect("valid scene bounds"),
        ))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_style(novadraw::figure::FigureStyle {
            font: Some(novadraw::text::FontDescriptor::parse("16px Inter Variable").unwrap()),
            background: Some(Color::rgba8(242, 245, 250, 255)),
            ..Default::default()
        })
        .unwrap();
    let ink = Color::rgba8(35, 62, 90, 255);
    let blue = Color::rgba8(45, 108, 210, 255);
    let green = Color::rgba8(33, 144, 113, 255);
    let orange = Color::rgba8(216, 119, 47, 255);
    // 普通 child tree 组合标题区域；Core 不拥有 title/body 概念。
    for (x, title, accent) in [(60.0, "Service", blue), (440.0, "Repository", green)] {
        let card = RoundedRectangleFigure::new_with_color(
            x,
            62.0,
            CARD_WIDTH,
            CARD_HEIGHT,
            0.0,
            Color::WHITE,
        )
        .expect("valid card bounds")
        .with_stroke(ink, BORDER_WIDTH)
        .expect("valid card stroke")
        .with_corners(RectangleCorners::uniform(RADIUS).unwrap())
        .unwrap();
        let card_id = runtime.figure(root).unwrap().add(Box::new(card)).unwrap();
        let header = RoundedRectangleFigure::new_with_color(
            BORDER_WIDTH,
            BORDER_WIDTH,
            CARD_WIDTH - BORDER_WIDTH * 2.0,
            TITLE_HEIGHT,
            0.0,
            accent,
        )
        .expect("valid header bounds")
        .with_stroke(ink, BORDER_WIDTH)
        .expect("valid header stroke")
        .with_corners(
            RectangleCorners::circular([RADIUS - BORDER_WIDTH, RADIUS - BORDER_WIDTH, 0.0, 0.0])
                .unwrap(),
        )
        .unwrap()
        .with_edges(RectangleEdges([
            EdgeStroke::None,
            EdgeStroke::None,
            EdgeStroke::Inherit,
            EdgeStroke::None,
        ]))
        .unwrap();
        runtime
            .figure(card_id)
            .unwrap()
            .add(Box::new(header))
            .unwrap();
        let label = LabelFigure::new(title)
            .with_bounds(Rectangle::new(
                18.0,
                12.0,
                CARD_WIDTH - 36.0,
                TITLE_HEIGHT - 16.0,
            ))
            .expect("valid label bounds");
        let label_id = runtime
            .figure(card_id)
            .unwrap()
            .add(Box::new(label))
            .unwrap();
        runtime
            .figure(label_id)
            .unwrap()
            .set_style(novadraw::figure::FigureStyle {
                foreground: Some(Color::WHITE),
                ..Default::default()
            })
            .unwrap();
        for (line, text) in ["id: String", "status: Ready", "execute()"]
            .into_iter()
            .enumerate()
        {
            runtime
                .figure(card_id)
                .unwrap()
                .add(Box::new(
                    LabelFigure::new(text)
                        .with_bounds(Rectangle::new(
                            20.0,
                            TITLE_HEIGHT + 20.0 + line as f64 * 36.0,
                            CARD_WIDTH - 40.0,
                            30.0,
                        ))
                        .expect("valid label bounds"),
                ))
                .unwrap();
        }
    }
    let open = RoundedRectangleFigure::new_with_color(60.0, 368.0, 220.0, 150.0, 0.0, Color::WHITE)
        .expect("valid shape bounds")
        .with_stroke(ink, 8.0)
        .expect("valid shape stroke")
        .with_corners(RectangleCorners::circular([30.0, 12.0, 0.0, 0.0]).unwrap())
        .unwrap()
        .with_edges(RectangleEdges([
            EdgeStroke::Color(blue),
            EdgeStroke::Color(orange),
            EdgeStroke::None,
            EdgeStroke::Color(green),
        ]))
        .unwrap();
    runtime.figure(root).unwrap().add(Box::new(open)).unwrap();
    let triangle = TriangleFigure::new(Rectangle::new(325.0, 358.0, 180.0, 180.0))
        .expect("valid triangle bounds")
        .with_fill_color(blue)
        .with_stroke_color(Color::TRANSPARENT)
        .with_corner_radii([24.0, 12.0, 18.0])
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(triangle))
        .unwrap();
    let polygon = PolygonFigure::from_points(vec![
        Point::new(570.0, 368.0),
        Point::new(744.0, 378.0),
        Point::new(720.0, 526.0),
        Point::new(636.0, 466.0),
        Point::new(558.0, 520.0),
    ])
    .expect("valid polygon points")
    .with_fill_color(green)
    .with_stroke(ink, BORDER_WIDTH)
    .expect("valid polygon stroke")
    .with_corner_radii(vec![18.0, 30.0, 10.0, 16.0, 26.0])
    .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(polygon))
        .unwrap();
    runtime
}

fn main() {
    let scenes: Vec<Scene> = vec![("Independent corners and edges", Box::new(scene))];
    if std::env::args().any(|arg| arg == "--screenshot") {
        run_demo_app_with_scene_screenshot("Composite Shapes", "composite-shapes-app", scenes, 0)
            .unwrap();
    } else {
        run_demo_app("Composite Shapes", "composite-shapes-app", scenes).unwrap();
    }
}
