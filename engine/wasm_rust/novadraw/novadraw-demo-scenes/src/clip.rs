//! Clip App - 裁剪验证
//!
//! 验证父子裁剪关系的正确性。

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

fn create_scene_0_basic_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let big_rect = novadraw::RectangleFigure::new_with_color(
        200.0,
        100.0,
        500.0,
        500.0,
        novadraw::Color::rgba(0.2, 0.6, 0.9, 1.0),
    );
    let _big = scene
        .builder()
        .add_child_to(container_id, Box::new(big_rect));

    let clip_boundary = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.0, 0.0, 0.0, 1.0), 2.0);
    let _clip = scene
        .builder()
        .add_child_to(container_id, Box::new(clip_boundary));

    scene
}

fn create_scene_1_nested_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let parent = novadraw::RectangleFigure::new_with_color(
        150.0,
        100.0,
        300.0,
        250.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    );
    let parent_id = scene.builder().add_child_to(container_id, Box::new(parent));

    let child = novadraw::RectangleFigure::new_with_color(
        200.0,
        150.0,
        350.0,
        260.0,
        novadraw::Color::rgba(0.2, 0.8, 0.4, 1.0),
    );
    let _child_id = scene.builder().add_child_to(parent_id, Box::new(child));

    scene
}

fn create_scene_2_multi_layer_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let level1 = novadraw::RectangleFigure::new_with_color(
        100.0,
        80.0,
        250.0,
        200.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    let level1_id = scene.builder().add_child_to(container_id, Box::new(level1));

    let level2 = novadraw::RectangleFigure::new_with_color(
        120.0,
        100.0,
        200.0,
        150.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let level2_id = scene.builder().add_child_to(level1_id, Box::new(level2));

    let level3 = novadraw::RectangleFigure::new_with_color(
        140.0,
        120.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
    );
    let _level3_id = scene.builder().add_child_to(level2_id, Box::new(level3));

    scene
}

fn create_scene_3_circle_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let ellipse = novadraw::EllipseFigure::new_with_color(
        400.0,
        300.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.6, 0.4, 0.8, 1.0),
    );
    let _ellipse = scene
        .builder()
        .add_child_to(container_id, Box::new(ellipse));

    let content = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        300.0,
        novadraw::Color::rgba(0.2, 0.7, 0.9, 1.0),
    );
    let _content = scene
        .builder()
        .add_child_to(container_id, Box::new(content));

    scene
}

fn create_scene_4_path_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let poly_clip = novadraw::RectangleFigure::new_with_color(
        300.0,
        100.0,
        200.0,
        100.0,
        novadraw::Color::rgba(0.8, 0.6, 0.2, 1.0),
    );
    let _poly = scene
        .builder()
        .add_child_to(container_id, Box::new(poly_clip));

    let content = novadraw::RectangleFigure::new_with_color(
        200.0,
        100.0,
        400.0,
        300.0,
        novadraw::Color::rgba(0.3, 0.6, 0.9, 1.0),
    );
    let _content = scene
        .builder()
        .add_child_to(container_id, Box::new(content));

    scene
}

fn create_scene_5_clip_with_events() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let event_area = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.4, 0.7, 0.4, 1.0),
    );
    let _event = scene
        .builder()
        .add_child_to(container_id, Box::new(event_area));

    scene
}

fn create_scene_6_transparent_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let bg = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
    );
    let _bg = scene.builder().add_child_to(container_id, Box::new(bg));

    let transparent = novadraw::RectangleFigure::new_with_color(
        300.0,
        200.0,
        200.0,
        200.0,
        novadraw::Color::rgba(0.3, 0.5, 0.8, 0.5),
    );
    let _trans = scene
        .builder()
        .add_child_to(container_id, Box::new(transparent));

    scene
}

fn create_scene_7_clip_animation() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let clip_window = novadraw::RectangleFigure::new_with_color(
        300.0,
        200.0,
        200.0,
        200.0,
        novadraw::Color::rgba(0.6, 0.3, 0.7, 1.0),
    );
    let _clip = scene
        .builder()
        .add_child_to(container_id, Box::new(clip_window));

    scene
}

fn create_scene_8_clip_performance() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    for i in 0..10 {
        for j in 0..8 {
            let rect = novadraw::RectangleFigure::new_with_color(
                50.0 + i as f64 * 70.0,
                50.0 + j as f64 * 70.0,
                60.0,
                60.0,
                novadraw::Color::rgba((i as f64 * 0.1) % 1.0, (j as f64 * 0.1) % 1.0, 0.5, 1.0),
            );
            let _rect = scene.builder().add_child_to(container_id, Box::new(rect));
        }
    }

    scene
}

fn create_scene_9_inverted_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let outer = novadraw::RectangleFigure::new_with_color(
        100.0,
        80.0,
        600.0,
        440.0,
        novadraw::Color::rgba(0.3, 0.5, 0.7, 1.0),
    );
    let _outer = scene.builder().add_child_to(container_id, Box::new(outer));

    let inner = novadraw::RectangleFigure::new_with_color(
        200.0,
        180.0,
        400.0,
        240.0,
        novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
    );
    let _inner = scene.builder().add_child_to(container_id, Box::new(inner));

    scene
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "clip",
        "Clipping",
        vec![
            SceneSpec::visual("basic-clip", "basic_clip", size, create_scene_0_basic_clip),
            SceneSpec::visual(
                "nested-clip",
                "nested_clip",
                size,
                create_scene_1_nested_clip,
            ),
            SceneSpec::visual(
                "multi-layer-clip",
                "multi_layer_clip",
                size,
                create_scene_2_multi_layer_clip,
            ),
            SceneSpec::visual(
                "circle-clip",
                "circle_clip",
                size,
                create_scene_3_circle_clip,
            ),
            SceneSpec::visual("path-clip", "path_clip", size, create_scene_4_path_clip),
            SceneSpec::visual(
                "clip-with-events",
                "clip_with_events",
                size,
                create_scene_5_clip_with_events,
            ),
            SceneSpec::visual(
                "transparent-clip",
                "transparent_clip",
                size,
                create_scene_6_transparent_clip,
            ),
            SceneSpec::visual(
                "clip-animation",
                "clip_animation",
                size,
                create_scene_7_clip_animation,
            ),
            SceneSpec::new(
                "clip-performance",
                "clip_performance",
                size,
                crate::ValidationKind::Performance,
                create_scene_8_clip_performance,
            ),
            SceneSpec::visual(
                "inverted-clip",
                "inverted_clip",
                size,
                create_scene_9_inverted_clip,
            ),
        ],
    )
}
