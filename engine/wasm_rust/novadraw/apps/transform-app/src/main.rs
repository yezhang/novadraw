//! M4 坐标域与变换闭环验证。

use novadraw::{
    Bounded, Color, Figure, FigureEventHandler, LineBorder, MouseEvent, NdCanvas, NovadrawContext,
    Point, Rectangle, RectangleFigure, SceneUpdateManager, Shape, Updatable,
    command::{LineCap, LineJoin},
};
use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const BACKGROUND: Color = Color {
    r: 0.933,
    g: 0.933,
    b: 0.933,
    a: 1.0,
};
const OUTER_COLOR: Color = Color {
    r: 0.36,
    g: 0.61,
    b: 0.84,
    a: 1.0,
};
const INNER_COLOR: Color = Color {
    r: 0.95,
    g: 0.67,
    b: 0.24,
    a: 1.0,
};
const CHILD_COLOR: Color = Color {
    r: 0.33,
    g: 0.73,
    b: 0.53,
    a: 1.0,
};
const TARGET_COLOR: Color = Color {
    r: 0.86,
    g: 0.32,
    b: 0.32,
    a: 1.0,
};
const OLD_BOUNDS_COLOR: Color = Color {
    r: 0.55,
    g: 0.55,
    b: 0.55,
    a: 1.0,
};
const BORDER_WIDTH: f64 = 3.0;

fn background() -> RectangleFigure {
    RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, BACKGROUND)
}

fn coordinate_root(x: f64, y: f64, width: f64, height: f64, color: Color) -> RectangleFigure {
    RectangleFigure::new_with_color(x, y, width, height, color)
        .with_border(LineBorder::new(Color::BLACK, BORDER_WIDTH).with_insets(8.0, 12.0, 8.0, 12.0))
}

fn add_nested_roots(
    scene: &mut novadraw::FigureGraph,
    contents: novadraw::BlockId,
) -> (novadraw::BlockId, novadraw::BlockId) {
    let outer = scene.add_child_to(
        contents,
        Box::new(coordinate_root(120.0, 90.0, 520.0, 400.0, OUTER_COLOR)),
    );
    let inner = scene.add_child_to(
        outer,
        Box::new(coordinate_root(70.0, 60.0, 330.0, 240.0, INNER_COLOR)),
    );
    (outer, inner)
}

fn create_nested_coordinate_roots() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let contents = scene.set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    scene.add_child_to(
        inner,
        Box::new(RectangleFigure::new_with_color(
            45.0,
            40.0,
            150.0,
            100.0,
            CHILD_COLOR,
        )),
    );
    scene
}

fn create_coordinate_roundtrip_overlay() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let contents = scene.set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    let local_bounds = Rectangle::new(45.0, 40.0, 150.0, 100.0);
    let child = scene.add_child_to(
        inner,
        Box::new(
            RectangleFigure::from_bounds(local_bounds).with_stroke(Color::WHITE, BORDER_WIDTH),
        ),
    );

    let mut absolute_bounds = Rectangle::new(0.0, 0.0, local_bounds.width, local_bounds.height);
    scene.translate_to_absolute_mut(child, &mut absolute_bounds);
    let mut roundtrip = absolute_bounds;
    scene.translate_to_relative(child, &mut roundtrip);
    assert_eq!(
        roundtrip,
        Rectangle::new(0.0, 0.0, local_bounds.width, local_bounds.height)
    );

    scene.add_child_to(
        contents,
        Box::new(
            RectangleFigure::new_with_color(
                absolute_bounds.x,
                absolute_bounds.y,
                absolute_bounds.width,
                absolute_bounds.height,
                Color::rgba(0.0, 0.0, 0.0, 0.0),
            )
            .with_stroke(TARGET_COLOR, BORDER_WIDTH),
        ),
    );
    scene
}

fn create_coordinate_root_move() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let contents = scene.set_contents(Box::new(background()));
    scene.add_child_to(
        contents,
        Box::new(
            RectangleFigure::new_with_color(
                120.0,
                100.0,
                300.0,
                230.0,
                Color::rgba(0.0, 0.0, 0.0, 0.0),
            )
            .with_stroke(OLD_BOUNDS_COLOR, BORDER_WIDTH),
        ),
    );
    let coordinate_root = scene.add_child_to(
        contents,
        Box::new(coordinate_root(120.0, 100.0, 300.0, 230.0, OUTER_COLOR)),
    );
    scene.add_child_to(
        coordinate_root,
        Box::new(RectangleFigure::new_with_color(
            35.0,
            35.0,
            120.0,
            80.0,
            CHILD_COLOR,
        )),
    );

    let mut update_manager = SceneUpdateManager::new();
    scene.set_bounds_with_update(
        &mut update_manager,
        coordinate_root,
        330.0,
        220.0,
        340.0,
        250.0,
    );
    scene
}

#[derive(Clone)]
struct TargetDomainFigure {
    bounds: Rectangle,
}

impl Bounded for TargetDomainFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "TargetDomainFigure"
    }
}

impl Updatable for TargetDomainFigure {
    fn validate(&mut self) {}
}

impl Figure for TargetDomainFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl Shape for TargetDomainFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(Color::WHITE)
    }

    fn stroke_width(&self) -> f64 {
        BORDER_WIDTH
    }

    fn fill_color(&self) -> Option<Color> {
        Some(TARGET_COLOR)
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        gc.fill_rect(0.0, 0.0, bounds.width, bounds.height, TARGET_COLOR);
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        gc.stroke_rect(
            0.0,
            0.0,
            bounds.width,
            bounds.height,
            Color::WHITE,
            BORDER_WIDTH,
            LineCap::default(),
            LineJoin::default(),
        );
    }
}

impl FigureEventHandler for TargetDomainFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let point = Point::new(event.x, event.y);
        let local_bounds = Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height);
        if local_bounds.contains(point) {
            ctx.select_target();
            return true;
        }
        false
    }
}

fn create_event_point_reduction() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let contents = scene.set_contents(Box::new(background()));
    let (_, inner) = add_nested_roots(&mut scene, contents);
    scene.add_child_to(
        inner,
        Box::new(TargetDomainFigure {
            bounds: Rectangle::new(55.0, 50.0, 180.0, 110.0),
        }),
    );
    scene
}

type SceneEntry = (&'static str, Box<dyn FnMut() -> novadraw::FigureGraph>);

fn scenes() -> Vec<SceneEntry> {
    vec![
        (
            "nested_coordinate_roots",
            Box::new(create_nested_coordinate_roots),
        ),
        (
            "coordinate_roundtrip_overlay",
            Box::new(create_coordinate_roundtrip_overlay),
        ),
        (
            "coordinate_root_move",
            Box::new(create_coordinate_root_move),
        ),
        (
            "event_point_reduction",
            Box::new(create_event_point_reduction),
        ),
    ]
}

fn main() {
    let title = "M4 Coordinates - 0-3 切换场景，场景 3 点击红色目标";
    let app_name = "transform-app";
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        None => run_demo_app(title, app_name, scenes()).expect("failed to run transform-app"),
        Some("--screenshot-all") => {
            run_demo_app_with_screenshot(title, app_name, scenes(), true)
                .expect("failed to capture transform-app scenes");
        }
        Some(arg) if arg.starts_with("--screenshot=") => {
            let index = arg
                .strip_prefix("--screenshot=")
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|index| *index < scenes().len())
                .unwrap_or_else(|| {
                    eprintln!("场景索引必须在 0..{} 范围内", scenes().len());
                    std::process::exit(2);
                });
            run_demo_app_with_scene_screenshot(title, app_name, scenes(), index)
                .expect("failed to capture transform-app scene");
        }
        Some("--help" | "-h") => {
            println!("cargo run -p transform-app -- [--screenshot-all|--screenshot=N]");
        }
        Some(arg) => {
            eprintln!("未知参数: {arg}");
            std::process::exit(2);
        }
    }
}
