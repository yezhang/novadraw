//! Viewport App - 视口 Figure 树语义验证
//!
//! 用可视化场景验证 ViewportFigure 的 content 裁剪、origin、zoom 和嵌套父链协议。

use novadraw_apps::{
    VerificationCli, run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const VIEWPORT_X: f64 = 110.0;
const VIEWPORT_Y: f64 = 80.0;
const VIEWPORT_WIDTH: f64 = 300.0;
const VIEWPORT_HEIGHT: f64 = 200.0;
const CONTENT_WIDTH: f64 = 500.0;
const CONTENT_HEIGHT: f64 = 350.0;
const STROKE_WIDTH: f64 = 3.0;
const CLIP_TO_VIEWPORT_SCENE: usize = 0;

fn color(r: f64, g: f64, b: f64) -> novadraw::Color {
    novadraw::Color::rgba(r, g, b, 1.0)
}

fn transparent() -> novadraw::Color {
    novadraw::Color::rgba(0.0, 0.0, 0.0, 0.0)
}

fn empty_scene() -> (novadraw::FigureGraph, novadraw::BlockId) {
    let mut scene = novadraw::FigureGraph::new();
    let root = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        color(0.94, 0.94, 0.94),
    );
    let root_id = scene.set_contents(Box::new(root));
    (scene, root_id)
}

#[allow(clippy::too_many_arguments)]
fn add_viewport(
    scene: &mut novadraw::FigureGraph,
    parent_id: novadraw::BlockId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    origin: (f64, f64),
    zoom: f64,
) -> novadraw::BlockId {
    let viewport = scene
        .add_viewport_to(parent_id, novadraw::Rectangle::new(x, y, width, height))
        .expect("attach viewport");
    let scalable = scene
        .add_scalable_layered_pane_to(
            viewport.block_id(),
            novadraw::Rectangle::new(0.0, 0.0, CONTENT_WIDTH, CONTENT_HEIGHT),
        )
        .expect("attach scalable pane");
    let mut update_manager = novadraw::SceneUpdateManager::new();
    scene.revalidate(viewport.block_id());
    novadraw::ZoomManager::new(scalable.clone(), viewport.clone())
        .set_zoom(scene, &mut update_manager, zoom)
        .expect("set zoom");
    viewport
        .set_view_location(scene, &mut update_manager, origin.0 * zoom, origin.1 * zoom)
        .expect("set viewport origin");
    scalable.block_id()
}

fn add_boundary(
    scene: &mut novadraw::FigureGraph,
    parent_id: novadraw::BlockId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke: novadraw::Color,
) {
    let boundary = novadraw::RectangleFigure::new_with_color(x, y, width, height, transparent())
        .with_stroke(stroke, STROKE_WIDTH);
    scene.add_child_to(parent_id, Box::new(boundary));
}

fn add_rect(
    scene: &mut novadraw::FigureGraph,
    parent_id: novadraw::BlockId,
    rect: novadraw::Rectangle,
    fill: novadraw::Color,
) {
    scene.add_child_to(
        parent_id,
        Box::new(novadraw::RectangleFigure::new_with_color(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            fill,
        )),
    );
}

fn add_content_grid(scene: &mut novadraw::FigureGraph, parent_id: novadraw::BlockId) {
    for row in 0..4 {
        for col in 0..5 {
            let fill = if (row + col) % 2 == 0 {
                color(0.25, 0.62, 0.95)
            } else {
                color(0.32, 0.78, 0.52)
            };
            add_rect(
                scene,
                parent_id,
                novadraw::Rectangle::new(
                    20.0 + col as f64 * 70.0,
                    20.0 + row as f64 * 55.0,
                    52.0,
                    38.0,
                ),
                fill,
            );
        }
    }
}

fn add_scroll_reference_content(
    scene: &mut novadraw::FigureGraph,
    content_parent: novadraw::BlockId,
) {
    add_content_grid(scene, content_parent);
    add_rect(
        scene,
        content_parent,
        novadraw::Rectangle::new(0.0, 0.0, 42.0, 42.0),
        color(1.0, 0.88, 0.12),
    );
    add_rect(
        scene,
        content_parent,
        novadraw::Rectangle::new(80.0, 60.0, 50.0, 50.0),
        color(0.08, 0.85, 0.25),
    );
    add_rect(
        scene,
        content_parent,
        novadraw::Rectangle::new(260.0, 180.0, 120.0, 80.0),
        color(0.95, 0.18, 0.22),
    );
}

fn create_scene_0_clip_to_viewport() -> novadraw::FigureGraph {
    let (mut scene, root_id) = empty_scene();
    let viewport_id = add_viewport(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        (0.0, 0.0),
        1.0,
    );

    add_content_grid(&mut scene, viewport_id);
    add_rect(
        &mut scene,
        viewport_id,
        novadraw::Rectangle::new(0.0, 0.0, 34.0, 34.0),
        color(1.0, 0.88, 0.12),
    );
    add_rect(
        &mut scene,
        viewport_id,
        novadraw::Rectangle::new(250.0, 150.0, 130.0, 90.0),
        color(0.95, 0.18, 0.22),
    );
    add_boundary(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        color(0.02, 0.02, 0.02),
    );
    scene
}

fn create_scene_1_origin_scroll() -> novadraw::FigureGraph {
    let (mut scene, root_id) = empty_scene();
    let viewport_id = add_viewport(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        (80.0, 60.0),
        1.0,
    );

    add_scroll_reference_content(&mut scene, viewport_id);
    add_boundary(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        color(0.02, 0.02, 0.02),
    );
    scene
}

fn create_scene_2_zoomed_content() -> novadraw::FigureGraph {
    let (mut scene, root_id) = empty_scene();
    let viewport_id = add_viewport(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        (80.0, 60.0),
        2.0,
    );

    add_scroll_reference_content(&mut scene, viewport_id);
    add_boundary(
        &mut scene,
        root_id,
        VIEWPORT_X,
        VIEWPORT_Y,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT,
        color(0.02, 0.02, 0.02),
    );
    scene
}

fn create_scene_3_nested_viewports() -> novadraw::FigureGraph {
    let (mut scene, root_id) = empty_scene();
    let outer_id = add_viewport(
        &mut scene,
        root_id,
        80.0,
        60.0,
        520.0,
        360.0,
        (40.0, 30.0),
        1.2,
    );
    add_content_grid(&mut scene, outer_id);
    add_rect(
        &mut scene,
        outer_id,
        novadraw::Rectangle::new(80.0, 70.0, 360.0, 240.0),
        color(0.82, 0.84, 0.88),
    );

    let inner_id = add_viewport(
        &mut scene,
        outer_id,
        150.0,
        105.0,
        210.0,
        130.0,
        (30.0, 20.0),
        1.5,
    );
    add_content_grid(&mut scene, inner_id);
    add_rect(
        &mut scene,
        inner_id,
        novadraw::Rectangle::new(30.0, 20.0, 34.0, 34.0),
        color(0.08, 0.85, 0.25),
    );
    add_rect(
        &mut scene,
        inner_id,
        novadraw::Rectangle::new(130.0, 90.0, 100.0, 70.0),
        color(0.95, 0.18, 0.22),
    );
    add_boundary(
        &mut scene,
        root_id,
        80.0,
        60.0,
        520.0,
        360.0,
        color(0.02, 0.02, 0.02),
    );
    scene
}

type SceneFactory = (&'static str, Box<dyn FnMut() -> novadraw::FigureGraph>);

fn scenes() -> Vec<SceneFactory> {
    vec![
        (
            "clip_to_viewport",
            Box::new(create_scene_0_clip_to_viewport),
        ),
        ("origin_scroll", Box::new(create_scene_1_origin_scroll)),
        ("zoomed_content", Box::new(create_scene_2_zoomed_content)),
        (
            "nested_viewports",
            Box::new(create_scene_3_nested_viewports),
        ),
    ]
}

fn main() {
    let title = "Viewport App - Figure 树视口验证 (按数字键 0-3 切换场景)";
    let app_name = "viewport-app";
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });

    let scene_list = scenes();
    let result = if cli.screenshot_all {
        run_demo_app_with_screenshot(title, app_name, scene_list, true)
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scene_list.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
        run_demo_app_with_scene_screenshot(title, app_name, scene_list, index)
    } else if std::env::args().any(|arg| arg == "--screenshot-clip") {
        run_demo_app_with_scene_screenshot(title, app_name, scene_list, CLIP_TO_VIEWPORT_SCENE)
    } else {
        run_demo_app(title, app_name, scene_list)
    };

    result.expect("Failed to run app");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_geometry(zoom: f64) -> (novadraw::Point, novadraw::Point, novadraw::Rectangle) {
        let (mut scene, root) = empty_scene();
        let content_parent = add_viewport(
            &mut scene,
            root,
            VIEWPORT_X,
            VIEWPORT_Y,
            VIEWPORT_WIDTH,
            VIEWPORT_HEIGHT,
            (80.0, 60.0),
            zoom,
        );
        let marker = scene.add_child_to(
            content_parent,
            Box::new(novadraw::RectangleFigure::new(80.0, 60.0, 50.0, 50.0)),
        );
        let mut anchor = novadraw::Point::new(0.0, 0.0);
        let mut offset = novadraw::Point::new(10.0, 10.0);
        scene.translate_to_absolute_mut(marker, &mut anchor);
        scene.translate_to_absolute_mut(marker, &mut offset);
        (anchor, offset, scene.figure_bounds(content_parent).unwrap())
    }

    #[test]
    fn zoom_scene_rebuilds_the_same_logical_content_without_scale_accumulation() {
        let (anchor_1x, offset_1x, pane_1x) = reference_geometry(1.0);
        let (anchor_2x, offset_2x, pane_2x) = reference_geometry(2.0);

        assert_eq!(anchor_1x, novadraw::Point::new(VIEWPORT_X, VIEWPORT_Y));
        assert_eq!(anchor_2x, anchor_1x);
        assert_eq!(
            offset_1x,
            novadraw::Point::new(VIEWPORT_X + 10.0, VIEWPORT_Y + 10.0)
        );
        assert_eq!(
            offset_2x,
            novadraw::Point::new(VIEWPORT_X + 20.0, VIEWPORT_Y + 20.0)
        );
        assert_eq!(pane_1x.width, CONTENT_WIDTH);
        assert_eq!(pane_1x.height, CONTENT_HEIGHT);
        assert_eq!(pane_2x.width, CONTENT_WIDTH * 2.0);
        assert_eq!(pane_2x.height, CONTENT_HEIGHT * 2.0);
    }
}
