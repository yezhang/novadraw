//! Layout App - 布局管理器验证
//!
//! 验证各种布局管理器的正确性。
//! 使用新的 LayoutManager 架构进行实际布局测试。

use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};
use std::io::Write;
use std::sync::Arc;

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

/// 创建使用 XYLayout 的场景
/// 演示基于约束的定位
fn create_scene_xy_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 设置 XYLayout
    let xy_layout = Arc::new(novadraw::XYLayout::new());
    scene.set_block_layout_manager(container_id, xy_layout);

    // 创建子元素并设置约束
    let positions = [
        (50.0, 50.0, 150.0, 100.0, "red"),
        (250.0, 100.0, 200.0, 80.0, "green"),
        (500.0, 50.0, 120.0, 120.0, "purple"),
        (100.0, 300.0, 180.0, 150.0, "yellow"),
    ];

    for (x, y, w, h, _name) in positions {
        let rect = novadraw::RectangleFigure::new_with_color(
            0.0, // 初始位置由布局器设置
            0.0,
            w,
            h,
            novadraw::Color::hex(match _name {
                "red" => "#e74c3c",
                "green" => "#2ecc71",
                "purple" => "#9b59b6",
                "yellow" => "#f1c40f",
                _ => "#95a5a6",
            }),
        );
        let child_id = scene.add_child_to(container_id, Box::new(rect));

        // 设置约束（位置和尺寸）
        let constraint = novadraw::Rectangle::new(x, y, w, h);
        scene.set_constraint(child_id, constraint);
    }

    // 执行布局
    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

/// 创建使用 FillLayout 的场景
/// 演示第一个子元素填充容器
fn create_scene_fill_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 设置 FillLayout
    let fill_layout = Arc::new(novadraw::FillLayout::new());
    scene.set_block_layout_manager(container_id, fill_layout);

    // 第一个子元素会填充容器（设置约束为容器大小）
    let first = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        100.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    let first_id = scene.add_child_to(container_id, Box::new(first));
    // 设置约束让第一个子元素填充容器
    scene.set_constraint(
        first_id,
        novadraw::Rectangle::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT),
    );

    // 其他子元素保持原位
    let second = novadraw::RectangleFigure::new_with_color(
        100.0,
        100.0,
        50.0,
        50.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let _second = scene.add_child_to(container_id, Box::new(second));

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

/// 创建 FlowLayout 场景
/// 演示流式布局：按顺序排列，自动换行
fn create_scene_flow_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 设置 FlowLayout
    let flow_layout = Arc::new(
        novadraw::FlowLayout::new()
            .with_spacing(15.0)
            .with_row_spacing(15.0),
    );
    scene.set_block_layout_manager(container_id, flow_layout);

    // 添加多个小方块，会自动换行（需要足够数量填满一行）
    let colors = [
        "#e74c3c", "#2ecc71", "#3498db", "#f1c40f", "#9b59b6", "#1abc9c", "#e67e22", "#e91e63",
        "#00bcd4", "#8bc34a", "#ff5722", "#673ab7", "#009688", "#ffc107", "#795548", "#607d8b",
    ];

    for (i, color) in colors.iter().enumerate() {
        let w = 100.0 + (i % 3) as f64 * 20.0; // 固定宽度范围
        let h = 60.0 + (i % 2) as f64 * 15.0;

        let rect =
            novadraw::RectangleFigure::new_with_color(0.0, 0.0, w, h, novadraw::Color::hex(color));
        let _child_id = scene.add_child_to(container_id, Box::new(rect));
    }

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

/// 创建嵌套布局场景
/// 外层使用 XYLayout，内层使用 FillLayout
fn create_scene_nested_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 外层：XYLayout
    let outer_layout = Arc::new(novadraw::XYLayout::new());
    scene.set_block_layout_manager(container_id, outer_layout);

    // 创建四个区域容器
    let regions = [
        (50.0, 50.0, 300.0, 200.0, "top-left"),
        (400.0, 50.0, 350.0, 200.0, "top-right"),
        (50.0, 300.0, 300.0, 250.0, "bottom-left"),
        (400.0, 300.0, 350.0, 250.0, "bottom-right"),
    ];

    for (x, y, w, h, _name) in regions {
        let rect = novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            w,
            h,
            novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
        );
        let region_id = scene.add_child_to(container_id, Box::new(rect));

        // 设置约束（外层 XYLayout）
        let constraint = novadraw::Rectangle::new(x, y, w, h);
        scene.set_constraint(region_id, constraint);

        // 为区域设置布局管理器（这样子元素才会应用约束）
        let region_layout = Arc::new(novadraw::XYLayout::new());
        scene.set_block_layout_manager(region_id, region_layout);

        // 添加子元素（填充整个区域）
        let child = novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            w,
            h,
            novadraw::Color::rgba(0.2, 0.5, 0.9, 1.0),
        );
        let child_id = scene.add_child_to(region_id, Box::new(child));

        // 为子元素设置约束，让它填充整个区域
        let child_constraint = novadraw::Rectangle::new(0.0, 0.0, w, h);
        scene.set_constraint(child_id, child_constraint);
    }

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

/// 创建测试约束动态更新的场景
/// 可以通过重新设置约束来测试布局重算
fn create_scene_constraint_update() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 设置 XYLayout
    let xy_layout = Arc::new(novadraw::XYLayout::new());
    scene.set_block_layout_manager(container_id, xy_layout);

    // 创建三个可移动的方块
    let colors = ["#e74c3c", "#2ecc71", "#3498db"];
    let mut child_ids = Vec::new();

    for (i, color) in colors.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 100.0,
            50.0,
            80.0,
            80.0,
            novadraw::Color::hex(color),
        );
        let child_id = scene.add_child_to(container_id, Box::new(rect));
        child_ids.push(child_id);
    }

    // 设置初始约束
    for (i, child_id) in child_ids.iter().enumerate() {
        let x = 50.0 + i as f64 * 200.0;
        let constraint = novadraw::Rectangle::new(x, 100.0, 80.0, 80.0);
        scene.set_constraint(*child_id, constraint);
    }

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

/// 创建 GridLayout 场景。
fn create_scene_grid_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    scene.set_block_layout_manager(
        container_id,
        Arc::new(
            novadraw::GridLayout::new(3)
                .with_equal_column_widths(true)
                .with_margins(40.0, 40.0)
                .with_spacing(20.0, 20.0),
        ),
    );

    // 创建 3x3 网格
    for row in 0..3 {
        for col in 0..3 {
            let rect = novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                150.0,
                120.0,
                novadraw::Color::rgba((col as f64 * 0.3) % 1.0, (row as f64 * 0.3) % 1.0, 0.6, 1.0),
            );
            let child_id = scene.add_child_to(container_id, Box::new(rect));
            scene.set_constraint(child_id, novadraw::GridConstraint::fill());
        }
    }

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

fn create_scene_toolbar_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let container_id = scene.set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    )));
    scene.set_block_layout_manager(
        container_id,
        Arc::new(
            novadraw::ToolbarLayout::horizontal()
                .with_spacing(16.0)
                .with_stretch_minor_axis(true),
        ),
    );
    for (index, color) in ["#e74c3c", "#2ecc71", "#3498db", "#f1c40f"]
        .iter()
        .enumerate()
    {
        let child = scene.add_child_to(
            container_id,
            Box::new(novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                240.0 - index as f64 * 20.0,
                80.0,
                novadraw::Color::hex(color),
            )),
        );
        scene.set_minimum_size(child, Some((100.0, 40.0)));
    }
    scene.revalidate(container_id);
    scene
}

fn create_scene_stack_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();
    let container_id = scene.set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
        80.0,
        60.0,
        640.0,
        480.0,
        novadraw::Color::hex("#eeeeee"),
    )));
    scene.set_block_layout_manager(container_id, Arc::new(novadraw::StackLayout::new()));
    for color in ["#e74c3c", "#3498db", "#2ecc71"] {
        scene.add_child_to(
            container_id,
            Box::new(novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                100.0,
                100.0,
                novadraw::Color::hex(color),
            )),
        );
    }
    scene.revalidate(container_id);
    scene
}

/// 创建没有布局器的场景（对比测试）
/// 子元素保持原始位置
fn create_scene_no_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 不设置布局器，子元素保持原位
    let rect1 = novadraw::RectangleFigure::new_with_color(
        100.0,
        100.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    let rect2 = novadraw::RectangleFigure::new_with_color(
        300.0,
        150.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let rect3 = novadraw::RectangleFigure::new_with_color(
        500.0,
        200.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
    );

    let _r1 = scene.add_child_to(container_id, Box::new(rect1));
    let _r2 = scene.add_child_to(container_id, Box::new(rect2));
    let _r3 = scene.add_child_to(container_id, Box::new(rect3));

    scene
}

/// 创建 BorderLayout 场景
/// 演示 BorderLayout 的五个区域：北、南、东、西、中
fn create_scene_border_layout() -> novadraw::FigureGraph {
    let mut scene = novadraw::FigureGraph::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::hex("#eeeeee"),
    );
    let container_id = scene.set_contents(Box::new(container));

    // 设置 BorderLayout
    let border_layout = Arc::new(novadraw::BorderLayout::new());
    scene.set_block_layout_manager(container_id, border_layout);

    // 添加五个区域：北、南、东、西、中
    // 约束格式：Rectangle::new(x, y, width, height)
    // - height < 0 → North
    // - height > 0 → South
    // - width < 0 → West
    // - width > 0 → East
    // - 其他 → Center

    // North (顶部，红色)
    let north = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        50.0,
        novadraw::Color::hex("#e74c3c"),
    );
    let north_id = scene.add_child_to(container_id, Box::new(north));
    // height < 0 表示 North
    scene.set_constraint(north_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, -60.0));

    // South (底部，绿色)
    let south = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        50.0,
        novadraw::Color::hex("#2ecc71"),
    );
    let south_id = scene.add_child_to(container_id, Box::new(south));
    // height > 0 表示 South
    scene.set_constraint(south_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, 60.0));

    // West (左侧，蓝色)
    let west = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        50.0,
        100.0,
        novadraw::Color::hex("#3498db"),
    );
    let west_id = scene.add_child_to(container_id, Box::new(west));
    // width < 0 表示 West
    scene.set_constraint(west_id, novadraw::Rectangle::new(0.0, 0.0, -100.0, 0.0));

    // East (右侧，黄色)
    let east = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        50.0,
        100.0,
        novadraw::Color::hex("#f1c40f"),
    );
    let east_id = scene.add_child_to(container_id, Box::new(east));
    // width > 0 表示 East
    scene.set_constraint(east_id, novadraw::Rectangle::new(0.0, 0.0, 100.0, 0.0));

    // Center (中间，紫色)
    let center = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        100.0,
        novadraw::Color::hex("#9b59b6"),
    );
    let center_id = scene.add_child_to(container_id, Box::new(center));
    // 默认 Center
    scene.set_constraint(center_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, 0.0));

    if let Some(contents) = scene.get_contents() {
        scene.revalidate(contents);
    }

    scene
}

type SceneEntry = (&'static str, Box<dyn FnMut() -> novadraw::FigureGraph>);

fn main() {
    let title = "Layout App - 布局管理器验证";
    let app_name = "layout-app";

    let scenes: Vec<SceneEntry> = vec![
        ("XYLayout + Constraints", Box::new(create_scene_xy_layout)),
        (
            "FillLayout (First Fills)",
            Box::new(create_scene_fill_layout),
        ),
        ("FlowLayout", Box::new(create_scene_flow_layout)),
        ("Nested Layouts", Box::new(create_scene_nested_layout)),
        (
            "Constraint Update",
            Box::new(create_scene_constraint_update),
        ),
        ("GridLayout", Box::new(create_scene_grid_layout)),
        ("ToolbarLayout", Box::new(create_scene_toolbar_layout)),
        ("StackLayout", Box::new(create_scene_stack_layout)),
        ("No Layout (Raw)", Box::new(create_scene_no_layout)),
        ("Border Layout (XY)", Box::new(create_scene_border_layout)),
    ];

    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--screenshot-all" => {
                // 截图所有场景
                println!("截图所有场景...");
                std::io::stdout().flush().ok();
                run_demo_app_with_screenshot(title, app_name, scenes, true)
                    .expect("Failed to run app with screenshot");
            }
            arg if arg.starts_with("--screenshot=") => {
                // 截图指定场景，例如 --screenshot=0 或 --screenshot=5
                let scene_idx = arg
                    .strip_prefix("--screenshot=")
                    .and_then(|s| s.parse::<usize>().ok());
                match scene_idx {
                    Some(idx) => {
                        run_demo_app_with_scene_screenshot(title, app_name, scenes, idx)
                            .expect("Failed to run app with scene screenshot");
                    }
                    None => {
                        eprintln!("无效的场景索引: {}", &arg[12..]);
                        eprintln!(
                            "用法: cargo run --package layout-app -- --screenshot=<0-{}>",
                            scenes.len() - 1
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!("用法: cargo run --package layout-app -- [选项]");
                println!("选项:");
                println!("  --screenshot-all    截图所有场景");
                println!("  --screenshot=<N>   截图指定场景 (0-{})", scenes.len() - 1);
                println!("  --help, -h         显示此帮助信息");
            }
            _ => {
                eprintln!("未知选项: {}", args[1]);
                eprintln!(
                    "用法: cargo run --package layout-app -- --screenshot=<0-{}>",
                    scenes.len() - 1
                );
                std::process::exit(1);
            }
        }
    } else {
        run_demo_app(title, app_name, scenes).expect("Failed to run app");
    }
}
