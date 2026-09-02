//! Clip App - 裁剪验证
//!
//! 验证父子裁剪关系的正确性。

use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};
use std::io::Write;

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

fn create_scene_0_basic_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let big_rect = novadraw::RectangleFigure::new_with_color(
        200.0,
        100.0,
        500.0,
        500.0,
        novadraw::Color::rgba(0.2, 0.6, 0.9, 1.0),
    );
    let _big = scene.add_child_to(container_id, Box::new(big_rect));

    let clip_boundary = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.0, 0.0, 0.0, 1.0), 2.0);
    let _clip = scene.add_child_to(container_id, Box::new(clip_boundary));

    scene
}

fn create_scene_1_nested_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let parent = novadraw::RectangleFigure::new_with_color(
        150.0,
        100.0,
        300.0,
        250.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    );
    let parent_id = scene.add_child_to(container_id, Box::new(parent));

    let child = novadraw::RectangleFigure::new_with_color(
        200.0,
        150.0,
        350.0,
        260.0,
        novadraw::Color::rgba(0.2, 0.8, 0.4, 1.0),
    );
    let _child_id = scene.add_child_to(parent_id, Box::new(child));

    scene
}

fn create_scene_2_multi_layer_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let level1 = novadraw::RectangleFigure::new_with_color(
        100.0,
        80.0,
        250.0,
        200.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    let level1_id = scene.add_child_to(container_id, Box::new(level1));

    let level2 = novadraw::RectangleFigure::new_with_color(
        120.0,
        100.0,
        200.0,
        150.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let level2_id = scene.add_child_to(level1_id, Box::new(level2));

    let level3 = novadraw::RectangleFigure::new_with_color(
        140.0,
        120.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
    );
    let _level3_id = scene.add_child_to(level2_id, Box::new(level3));

    scene
}

fn create_scene_3_circle_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let ellipse = novadraw::EllipseFigure::new_with_color(
        400.0,
        300.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.6, 0.4, 0.8, 1.0),
    );
    let _ellipse = scene.add_child_to(container_id, Box::new(ellipse));

    let content = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        300.0,
        novadraw::Color::rgba(0.2, 0.7, 0.9, 1.0),
    );
    let _content = scene.add_child_to(container_id, Box::new(content));

    scene
}

fn create_scene_4_path_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let poly_clip = novadraw::RectangleFigure::new_with_color(
        300.0,
        100.0,
        200.0,
        100.0,
        novadraw::Color::rgba(0.8, 0.6, 0.2, 1.0),
    );
    let _poly = scene.add_child_to(container_id, Box::new(poly_clip));

    let content = novadraw::RectangleFigure::new_with_color(
        200.0,
        100.0,
        400.0,
        300.0,
        novadraw::Color::rgba(0.3, 0.6, 0.9, 1.0),
    );
    let _content = scene.add_child_to(container_id, Box::new(content));

    scene
}

fn create_scene_5_clip_with_events() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let event_area = novadraw::RectangleFigure::new_with_color(
        250.0,
        150.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.4, 0.7, 0.4, 1.0),
    );
    let _event = scene.add_child_to(container_id, Box::new(event_area));

    scene
}

fn create_scene_6_transparent_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let bg = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
    );
    let _bg = scene.add_child_to(container_id, Box::new(bg));

    let transparent = novadraw::RectangleFigure::new_with_color(
        300.0,
        200.0,
        200.0,
        200.0,
        novadraw::Color::rgba(0.3, 0.5, 0.8, 0.5),
    );
    let _trans = scene.add_child_to(container_id, Box::new(transparent));

    scene
}

fn create_scene_7_clip_animation() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let clip_window = novadraw::RectangleFigure::new_with_color(
        300.0,
        200.0,
        200.0,
        200.0,
        novadraw::Color::rgba(0.6, 0.3, 0.7, 1.0),
    );
    let _clip = scene.add_child_to(container_id, Box::new(clip_window));

    scene
}

fn create_scene_8_clip_performance() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    for i in 0..10 {
        for j in 0..8 {
            let rect = novadraw::RectangleFigure::new_with_color(
                50.0 + i as f64 * 70.0,
                50.0 + j as f64 * 70.0,
                60.0,
                60.0,
                novadraw::Color::rgba((i as f64 * 0.1) % 1.0, (j as f64 * 0.1) % 1.0, 0.5, 1.0),
            );
            let _rect = scene.add_child_to(container_id, Box::new(rect));
        }
    }

    scene
}

fn create_scene_9_inverted_clip() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.set_contents(Box::new(container));

    let outer = novadraw::RectangleFigure::new_with_color(
        100.0,
        80.0,
        600.0,
        440.0,
        novadraw::Color::rgba(0.3, 0.5, 0.7, 1.0),
    );
    let _outer = scene.add_child_to(container_id, Box::new(outer));

    let inner = novadraw::RectangleFigure::new_with_color(
        200.0,
        180.0,
        400.0,
        240.0,
        novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
    );
    let _inner = scene.add_child_to(container_id, Box::new(inner));

    scene
}

type SceneEntry = (&'static str, Box<dyn FnMut() -> novadraw::FigureTree>);

fn scenes() -> Vec<SceneEntry> {
    vec![
        ("basic_clip", Box::new(create_scene_0_basic_clip)),
        ("nested_clip", Box::new(create_scene_1_nested_clip)),
        (
            "multi_layer_clip",
            Box::new(create_scene_2_multi_layer_clip),
        ),
        ("circle_clip", Box::new(create_scene_3_circle_clip)),
        ("path_clip", Box::new(create_scene_4_path_clip)),
        (
            "clip_with_events",
            Box::new(create_scene_5_clip_with_events),
        ),
        (
            "transparent_clip",
            Box::new(create_scene_6_transparent_clip),
        ),
        ("clip_animation", Box::new(create_scene_7_clip_animation)),
        (
            "clip_performance",
            Box::new(create_scene_8_clip_performance),
        ),
        ("inverted_clip", Box::new(create_scene_9_inverted_clip)),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let title = "Clip App - 裁剪验证 (按数字键 0-9 切换场景)";
    let app_name = "clip-app";

    if args.len() > 1 {
        match args[1].as_str() {
            "--screenshot-all" => {
                println!("截图所有场景...");
                std::io::stdout().flush().ok();
                run_demo_app_with_screenshot(title, app_name, scenes(), true)
                    .expect("Failed to run app with screenshot");
            }
            arg if arg.starts_with("--screenshot=") => {
                let scene_idx = arg
                    .strip_prefix("--screenshot=")
                    .and_then(|s| s.parse::<usize>().ok());
                match scene_idx {
                    Some(idx) => {
                        run_demo_app_with_scene_screenshot(title, app_name, scenes(), idx)
                            .expect("Failed to run app with scene screenshot");
                    }
                    None => {
                        eprintln!("无效的场景索引: {}", &arg[13..]);
                        eprintln!(
                            "用法: cargo run -p clip-app -- --screenshot=<0-9> 或 --screenshot-all"
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!("用法: cargo run -p clip-app -- [选项]");
                println!("选项:");
                println!("  --screenshot-all    截图所有场景");
                println!("  --screenshot=<N>   截图指定场景 (0-9)");
                println!("  --help, -h         显示帮助");
                std::process::exit(0);
            }
            _ => {
                eprintln!("未知参数: {}", args[1]);
                eprintln!("用法: cargo run -p clip-app -- --screenshot=<0-9> 或 --screenshot-all");
                std::process::exit(1);
            }
        }
    } else {
        run_demo_app(title, app_name, scenes()).expect("Failed to run app");
    }
}
