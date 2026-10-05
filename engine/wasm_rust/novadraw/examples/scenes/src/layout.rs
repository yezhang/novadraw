//! Layout App - 布局管理器验证
//!
//! 验证各种布局管理器的正确性。
//! 使用新的 LayoutManager 架构进行实际布局测试。

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

/// 创建使用 XYLayout 的场景
/// 演示基于约束的定位
fn create_scene_xy_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 设置 XYLayout
    let xy_layout = Box::new(novadraw::XYLayout::new());
    scene
        .builder()
        .set_layout_manager(container_id, xy_layout)
        .expect("valid FigureTree construction");

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
            novadraw::Color::from_hex(match _name {
                "red" => "#e74c3c",
                "green" => "#2ecc71",
                "purple" => "#9b59b6",
                "yellow" => "#f1c40f",
                _ => "#95a5a6",
            })
            .expect("valid color literal"),
        );
        let child_id = scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");

        // 设置约束（位置和尺寸）
        let constraint = novadraw::Rectangle::new(x, y, w, h);
        scene
            .builder()
            .set_layout_constraint(child_id, constraint)
            .expect("valid FigureTree construction");
    }

    // 执行布局
    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

/// 创建使用 FillLayout 的场景
/// 演示第一个子元素填充容器
fn create_scene_fill_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 设置 FillLayout
    let fill_layout = Box::new(novadraw::FillLayout::new());
    scene
        .builder()
        .set_layout_manager(container_id, fill_layout)
        .expect("valid FigureTree construction");

    // 第一个子元素会填充容器；FillLayout 不接受子节点约束。
    let first = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        100.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    scene
        .builder()
        .add_child(container_id, Box::new(first))
        .expect("valid FigureTree construction");

    // 其他子元素保持原位
    let second = novadraw::RectangleFigure::new_with_color(
        100.0,
        100.0,
        50.0,
        50.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let _second = scene
        .builder()
        .add_child(container_id, Box::new(second))
        .expect("valid FigureTree construction");

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

/// 创建 FlowLayout 场景
/// 演示流式布局：按顺序排列，自动换行
fn create_scene_flow_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 设置 FlowLayout
    let flow_layout = Box::new(
        novadraw::FlowLayout::new()
            .with_spacing(15.0)
            .with_row_spacing(15.0),
    );
    scene
        .builder()
        .set_layout_manager(container_id, flow_layout)
        .expect("valid FigureTree construction");

    // 添加多个小方块，会自动换行（需要足够数量填满一行）
    let colors = [
        "#e74c3c", "#2ecc71", "#3498db", "#f1c40f", "#9b59b6", "#1abc9c", "#e67e22", "#e91e63",
        "#00bcd4", "#8bc34a", "#ff5722", "#673ab7", "#009688", "#ffc107", "#795548", "#607d8b",
    ];

    for (i, color) in colors.iter().enumerate() {
        let w = 100.0 + (i % 3) as f64 * 20.0; // 固定宽度范围
        let h = 60.0 + (i % 2) as f64 * 15.0;

        let rect = novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            w,
            h,
            novadraw::Color::from_hex(color).expect("valid color literal"),
        );
        let _child_id = scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
    }

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

/// 创建嵌套布局场景
/// 外层使用 XYLayout，内层使用 FillLayout
fn create_scene_nested_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 外层：XYLayout
    let outer_layout = Box::new(novadraw::XYLayout::new());
    scene
        .builder()
        .set_layout_manager(container_id, outer_layout)
        .expect("valid FigureTree construction");

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
        let region_id = scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");

        // 设置约束（外层 XYLayout）
        let constraint = novadraw::Rectangle::new(x, y, w, h);
        scene
            .builder()
            .set_layout_constraint(region_id, constraint)
            .expect("valid FigureTree construction");

        // 为区域设置布局管理器（这样子元素才会应用约束）
        let region_layout = Box::new(novadraw::XYLayout::new());
        scene
            .builder()
            .set_layout_manager(region_id, region_layout)
            .expect("valid FigureTree construction");

        // 添加子元素（填充整个区域）
        let child = novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            w,
            h,
            novadraw::Color::rgba(0.2, 0.5, 0.9, 1.0),
        );
        let child_id = scene
            .builder()
            .add_child(region_id, Box::new(child))
            .expect("valid FigureTree construction");

        // 为子元素设置约束，让它填充整个区域
        let child_constraint = novadraw::Rectangle::new(0.0, 0.0, w, h);
        scene
            .builder()
            .set_layout_constraint(child_id, child_constraint)
            .expect("valid FigureTree construction");
    }

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

/// 创建测试约束动态更新的场景
/// 可以通过重新设置约束来测试布局重算
fn create_scene_constraint_update() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 设置 XYLayout
    let xy_layout = Box::new(novadraw::XYLayout::new());
    scene
        .builder()
        .set_layout_manager(container_id, xy_layout)
        .expect("valid FigureTree construction");

    // 创建三个可移动的方块
    let colors = ["#e74c3c", "#2ecc71", "#3498db"];
    let mut child_ids = Vec::new();

    for (i, color) in colors.iter().enumerate() {
        let rect = novadraw::RectangleFigure::new_with_color(
            50.0 + i as f64 * 100.0,
            50.0,
            80.0,
            80.0,
            novadraw::Color::from_hex(color).expect("valid color literal"),
        );
        let child_id = scene
            .builder()
            .add_child(container_id, Box::new(rect))
            .expect("valid FigureTree construction");
        child_ids.push(child_id);
    }

    // 设置初始约束
    for (i, child_id) in child_ids.iter().enumerate() {
        let x = 50.0 + i as f64 * 200.0;
        let constraint = novadraw::Rectangle::new(x, 100.0, 80.0, 80.0);
        scene
            .builder()
            .set_layout_constraint(*child_id, constraint)
            .expect("valid FigureTree construction");
    }

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

/// 创建 GridLayout 场景。
fn create_scene_grid_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    scene
        .builder()
        .set_layout_manager(
            container_id,
            Box::new(
                novadraw::GridLayout::new(3)
                    .with_equal_column_widths(true)
                    .with_margins(40.0, 40.0)
                    .with_spacing(20.0, 20.0),
            ),
        )
        .expect("valid FigureTree construction");

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
            let child_id = scene
                .builder()
                .add_child(container_id, Box::new(rect))
                .expect("valid FigureTree construction");
            scene
                .builder()
                .set_layout_constraint(child_id, novadraw::layout::GridConstraint::fill())
                .expect("valid FigureTree construction");
        }
    }

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_toolbar_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container_id =
        scene
            .builder()
            .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
            )));
    scene
        .builder()
        .set_layout_manager(
            container_id,
            Box::new(
                novadraw::ToolbarLayout::horizontal()
                    .with_spacing(16.0)
                    .with_stretch_minor_axis(true),
            ),
        )
        .expect("valid FigureTree construction");
    for (index, color) in ["#e74c3c", "#2ecc71", "#3498db", "#f1c40f"]
        .iter()
        .enumerate()
    {
        let child = scene
            .builder()
            .add_child(
                container_id,
                Box::new(novadraw::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    240.0 - index as f64 * 20.0,
                    80.0,
                    novadraw::Color::from_hex(color).expect("valid color literal"),
                )),
            )
            .expect("valid FigureTree construction");
        scene
            .builder()
            .set_minimum_size(child, Some((100.0, 40.0)))
            .expect("valid FigureTree construction");
    }
    scene
        .builder()
        .validate_subtree(container_id)
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_stack_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container_id =
        scene
            .builder()
            .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
                80.0,
                60.0,
                640.0,
                480.0,
                novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
            )));
    scene
        .builder()
        .set_layout_manager(container_id, Box::new(novadraw::StackLayout::new()))
        .expect("valid FigureTree construction");
    for color in ["#e74c3c", "#3498db", "#2ecc71"] {
        scene
            .builder()
            .add_child(
                container_id,
                Box::new(novadraw::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    100.0,
                    100.0,
                    novadraw::Color::from_hex(color).expect("valid color literal"),
                )),
            )
            .expect("valid FigureTree construction");
    }
    scene
        .builder()
        .validate_subtree(container_id)
        .expect("valid FigureTree construction");
    scene
}

/// 创建没有布局器的场景（对比测试）
/// 子元素保持原始位置
fn create_scene_no_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

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

    let _r1 = scene
        .builder()
        .add_child(container_id, Box::new(rect1))
        .expect("valid FigureTree construction");
    let _r2 = scene
        .builder()
        .add_child(container_id, Box::new(rect2))
        .expect("valid FigureTree construction");
    let _r3 = scene
        .builder()
        .add_child(container_id, Box::new(rect3))
        .expect("valid FigureTree construction");

    scene
}

/// 创建 BorderLayout 场景
/// 演示 BorderLayout 的五个区域：北、南、东、西、中
fn create_scene_border_layout() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    // 创建容器（浅灰色背景）
    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 设置 BorderLayout
    let border_layout = Box::new(novadraw::BorderLayout::new());
    scene
        .builder()
        .set_layout_manager(container_id, border_layout)
        .expect("valid FigureTree construction");

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
        novadraw::Color::from_hex("#e74c3c").expect("valid color literal"),
    );
    let north_id = scene
        .builder()
        .add_child(container_id, Box::new(north))
        .expect("valid FigureTree construction");
    // height < 0 表示 North
    scene
        .builder()
        .set_layout_constraint(north_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, -60.0))
        .expect("valid FigureTree construction");

    // South (底部，绿色)
    let south = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        50.0,
        novadraw::Color::from_hex("#2ecc71").expect("valid color literal"),
    );
    let south_id = scene
        .builder()
        .add_child(container_id, Box::new(south))
        .expect("valid FigureTree construction");
    // height > 0 表示 South
    scene
        .builder()
        .set_layout_constraint(south_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, 60.0))
        .expect("valid FigureTree construction");

    // West (左侧，蓝色)
    let west = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        50.0,
        100.0,
        novadraw::Color::from_hex("#3498db").expect("valid color literal"),
    );
    let west_id = scene
        .builder()
        .add_child(container_id, Box::new(west))
        .expect("valid FigureTree construction");
    // width < 0 表示 West
    scene
        .builder()
        .set_layout_constraint(west_id, novadraw::Rectangle::new(0.0, 0.0, -100.0, 0.0))
        .expect("valid FigureTree construction");

    // East (右侧，黄色)
    let east = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        50.0,
        100.0,
        novadraw::Color::from_hex("#f1c40f").expect("valid color literal"),
    );
    let east_id = scene
        .builder()
        .add_child(container_id, Box::new(east))
        .expect("valid FigureTree construction");
    // width > 0 表示 East
    scene
        .builder()
        .set_layout_constraint(east_id, novadraw::Rectangle::new(0.0, 0.0, 100.0, 0.0))
        .expect("valid FigureTree construction");

    // Center (中间，紫色)
    let center = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        100.0,
        100.0,
        novadraw::Color::from_hex("#9b59b6").expect("valid color literal"),
    );
    let center_id = scene
        .builder()
        .add_child(container_id, Box::new(center))
        .expect("valid FigureTree construction");
    // 默认 Center
    scene
        .builder()
        .set_layout_constraint(center_id, novadraw::Rectangle::new(0.0, 0.0, 0.0, 0.0))
        .expect("valid FigureTree construction");

    if let Some(contents) = scene.contents() {
        scene
            .builder()
            .validate_subtree(contents)
            .expect("valid FigureTree construction");
    }

    scene
}

fn create_scene_root_viewport_resize() -> novadraw::FigureTree {
    const HEADER_HEIGHT: f64 = 72.0;
    const FOOTER_HEIGHT: f64 = 56.0;
    const SIDEBAR_WIDTH: f64 = 120.0;

    let mut scene = novadraw::FigureTree::new();
    let contents =
        scene
            .builder()
            .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                novadraw::Color::from_hex("#20252b").expect("valid color literal"),
            )));
    scene
        .builder()
        .set_layout_manager(
            contents,
            Box::new(novadraw::BorderLayout::with_sizes(
                HEADER_HEIGHT,
                FOOTER_HEIGHT,
                SIDEBAR_WIDTH,
                SIDEBAR_WIDTH,
            )),
        )
        .expect("valid FigureTree construction");

    for (region, size, color) in [
        (
            novadraw::layout::BorderRegion::North,
            Some(HEADER_HEIGHT),
            "#e74c3c",
        ),
        (
            novadraw::layout::BorderRegion::South,
            Some(FOOTER_HEIGHT),
            "#2ecc71",
        ),
        (
            novadraw::layout::BorderRegion::West,
            Some(SIDEBAR_WIDTH),
            "#3498db",
        ),
        (
            novadraw::layout::BorderRegion::East,
            Some(SIDEBAR_WIDTH),
            "#f1c40f",
        ),
        (novadraw::layout::BorderRegion::Center, None, "#9b59b6"),
    ] {
        let child = scene
            .builder()
            .add_child(
                contents,
                Box::new(novadraw::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    10.0,
                    10.0,
                    novadraw::Color::from_hex(color).expect("valid color literal"),
                )),
            )
            .expect("valid FigureTree construction");
        scene
            .builder()
            .set_layout_constraint(
                child,
                size.map_or_else(
                    || novadraw::layout::BorderConstraint::new(region),
                    |size| novadraw::layout::BorderConstraint::with_size(region, size),
                ),
            )
            .expect("valid FigureTree construction");
    }

    scene
        .builder()
        .validate_subtree(contents)
        .expect("valid FigureTree construction");
    scene
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "layout",
        "Layouts",
        vec![
            SceneSpec::visual(
                "xy-layout",
                "XYLayout + Constraints",
                size,
                create_scene_xy_layout,
            ),
            SceneSpec::visual(
                "fill-layout",
                "FillLayout (Horizontal)",
                size,
                create_scene_fill_layout,
            ),
            SceneSpec::visual("flow-layout", "FlowLayout", size, create_scene_flow_layout),
            SceneSpec::visual(
                "nested-layouts",
                "Nested Layouts",
                size,
                create_scene_nested_layout,
            ),
            SceneSpec::visual(
                "constraint-update",
                "Constraint Update",
                size,
                create_scene_constraint_update,
            ),
            SceneSpec::visual("grid-layout", "GridLayout", size, create_scene_grid_layout),
            SceneSpec::visual(
                "toolbar-layout",
                "ToolbarLayout",
                size,
                create_scene_toolbar_layout,
            ),
            SceneSpec::visual(
                "stack-layout",
                "StackLayout",
                size,
                create_scene_stack_layout,
            ),
            SceneSpec::visual("no-layout", "No Layout (Raw)", size, create_scene_no_layout),
            SceneSpec::visual(
                "border-layout",
                "Border Layout (XY)",
                size,
                create_scene_border_layout,
            ),
            SceneSpec::visual(
                "root-viewport-resize",
                "root_viewport_resize",
                size,
                create_scene_root_viewport_resize,
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use novadraw::Runtime;
    use novadraw::render::{BackendCapabilities, SurfaceInfo};

    use super::*;

    #[test]
    fn root_viewport_resize_reflows_border_regions() {
        let tree = create_scene_root_viewport_resize();
        let contents = tree.contents().expect("contents");
        let children = tree.child_order(contents).expect("layout children");
        let north = children[0];
        let center = children[4];
        let mut runtime = Runtime::new(tree);
        let surface = SurfaceInfo {
            logical_width: 1_000.0,
            logical_height: 700.0,
            pixel_width: 1_000,
            pixel_height: 700,
            scale_factor: 1.0,
        };

        runtime.resize_logical_viewport(1_000.0, 700.0).unwrap();
        runtime
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .expect("resize frame");

        assert_eq!(
            runtime.tree().figure_bounds(contents),
            Some(novadraw::Rectangle::new(0.0, 0.0, 1_000.0, 700.0))
        );
        assert_eq!(
            runtime.tree().figure_bounds(north),
            Some(novadraw::Rectangle::new(0.0, 0.0, 1_000.0, 72.0))
        );
        assert_eq!(
            runtime.tree().figure_bounds(center),
            Some(novadraw::Rectangle::new(120.0, 72.0, 760.0, 572.0))
        );
    }
}
