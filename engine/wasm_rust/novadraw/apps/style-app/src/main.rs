use novadraw::border::RectangleBorder;
use novadraw_apps::{run_demo_app, run_demo_app_with_screenshot};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

fn bg_gray() -> novadraw::Color {
    novadraw::Color::rgba(0.85, 0.85, 0.85, 1.0)
}

fn gray_container() -> (novadraw::FigureTree, novadraw::FigureId) {
    let mut scene = novadraw::FigureTree::new();
    let container =
        novadraw::RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, bg_gray());
    let container_id = scene.set_contents(Box::new(container));
    (scene, container_id)
}

fn create_scene_0_fill_colors() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let colors = [
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
        novadraw::Color::rgba(0.2, 0.8, 0.2, 1.0),
        novadraw::Color::rgba(0.2, 0.2, 0.9, 1.0),
        novadraw::Color::rgba(0.9, 0.9, 0.2, 1.0),
    ];

    for (i, &color) in colors.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 185.0,
            200.0,
            160.0,
            120.0,
            color,
        );
        scene.add_child_to(container_id, Box::new(rect));
    }

    scene
}

fn create_scene_1_alpha() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let base_rect = novadraw::RectangleFigure::new_with_color(
        100.0,
        150.0,
        200.0,
        200.0,
        novadraw::Color::WHITE,
    );
    scene.add_child_to(container_id, Box::new(base_rect));

    let alphas = [1.0, 0.75, 0.5, 0.25];
    for (i, &alpha) in alphas.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            200.0 + i as f64 * 120.0,
            200.0,
            160.0,
            120.0,
            novadraw::Color::rgba(0.9, 0.2, 0.2, alpha),
        );
        scene.add_child_to(container_id, Box::new(rect));
    }

    scene
}

fn create_scene_2_stroke_width() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let widths = [1.0, 2.0, 3.0, 4.0, 5.0];
    let stroke_color = novadraw::Color::rgba(0.2, 0.2, 0.2, 1.0);
    let fill_color = novadraw::Color::rgba(0.95, 0.95, 0.95, 1.0);

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            40.0 + i as f64 * 150.0,
            200.0,
            130.0,
            120.0,
            fill_color,
        )
        .with_stroke(stroke_color, width);
        scene.add_child_to(container_id, Box::new(rect));
    }

    scene
}

fn create_scene_3_stroke_color() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let stroke_colors = [
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
        novadraw::Color::rgba(0.2, 0.7, 0.2, 1.0),
        novadraw::Color::rgba(0.2, 0.2, 0.9, 1.0),
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    ];
    let fill_color = novadraw::Color::rgba(0.95, 0.95, 0.95, 1.0);

    for (i, &stroke_color) in stroke_colors.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 185.0,
            200.0,
            160.0,
            120.0,
            fill_color,
        )
        .with_stroke(stroke_color, 3.0);
        scene.add_child_to(container_id, Box::new(rect));
    }

    scene
}

fn create_scene_4_line_cap() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let caps = [
        (
            novadraw::render::command::LineCap::Butt,
            novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
        ),
        (
            novadraw::render::command::LineCap::Round,
            novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
        ),
        (
            novadraw::render::command::LineCap::Square,
            novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
        ),
    ];

    for (i, &(cap, color)) in caps.iter().enumerate() {
        let line = novadraw::PolylineFigure::new_with_color(
            100.0,
            180.0 + i as f64 * 120.0,
            700.0,
            180.0 + i as f64 * 120.0,
            color,
        )
        .with_width(12.0)
        .with_cap(cap);
        scene.add_child_to(container_id, Box::new(line));
    }

    scene
}

fn create_scene_5_line_join() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let joins = [
        (
            novadraw::render::command::LineJoin::Miter,
            novadraw::Color::rgba(1.0, 0.5, 0.0, 1.0),
        ),
        (
            novadraw::render::command::LineJoin::Round,
            novadraw::Color::rgba(0.0, 0.8, 0.8, 1.0),
        ),
        (
            novadraw::render::command::LineJoin::Bevel,
            novadraw::Color::rgba(0.8, 0.0, 0.8, 1.0),
        ),
    ];

    for (i, &(join, color)) in joins.iter().enumerate() {
        let base_x = 80.0 + i as f64 * 240.0;
        let base_y = 150.0;
        let line = novadraw::PolylineFigure::from_points(vec![
            novadraw_geometry::Vec2::new(base_x, base_y + 200.0),
            novadraw_geometry::Vec2::new(base_x + 80.0, base_y),
            novadraw_geometry::Vec2::new(base_x + 160.0, base_y + 200.0),
        ])
        .with_width(10.0)
        .with_join(join)
        .with_color(color);
        scene.add_child_to(container_id, Box::new(line));
    }

    scene
}

fn create_scene_6_stroke_vs_border() -> novadraw::FigureTree {
    let (mut scene, container_id) = gray_container();

    let fill_color = novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0);
    let stroke_color = novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0);
    let border_color = novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0);

    let widths = [2.0, 4.0, 8.0];

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            50.0,
            200.0,
            80.0,
            fill_color,
        )
        .with_stroke(stroke_color, width);
        scene.add_child_to(container_id, Box::new(rect));
    }

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            200.0,
            200.0,
            80.0,
            fill_color,
        )
        .with_border(RectangleBorder::new(border_color, width));
        scene.add_child_to(container_id, Box::new(rect));
    }

    let inner_stroke_color = novadraw::Color::rgba(0.2, 0.6, 0.2, 1.0);
    let outer_border_color = novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0);

    for (i, &width) in widths.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 250.0,
            360.0,
            200.0,
            80.0,
            novadraw::Color::rgba(0.9, 0.95, 0.9, 1.0),
        )
        .with_stroke(inner_stroke_color, width)
        .with_border(
            RectangleBorder::new(outer_border_color, width).with_insets(8.0, 8.0, 8.0, 8.0),
        );
        scene.add_child_to(container_id, Box::new(rect));
    }

    scene
}

type SceneEntry = (&'static str, Box<dyn FnMut() -> novadraw::FigureTree>);

fn main() {
    let scenes: Vec<SceneEntry> = vec![
        ("Fill Colors", Box::new(create_scene_0_fill_colors)),
        ("Alpha/Transparency", Box::new(create_scene_1_alpha)),
        ("Stroke Width", Box::new(create_scene_2_stroke_width)),
        ("Stroke Color", Box::new(create_scene_3_stroke_color)),
        ("LineCap", Box::new(create_scene_4_line_cap)),
        ("LineJoin", Box::new(create_scene_5_line_join)),
        (
            "Stroke vs Border",
            Box::new(create_scene_6_stroke_vs_border),
        ),
    ];

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--screenshot" {
        run_demo_app_with_screenshot(
            "Style App - Visual Style Properties",
            "style-app",
            scenes,
            true,
        )
        .expect("Failed to run app in screenshot mode");
    } else {
        run_demo_app(
            "Style App - Visual Style Properties (按方向键/鼠标滚轮切换)",
            "style-app",
            scenes,
        )
        .expect("Failed to run app");
    }
}
