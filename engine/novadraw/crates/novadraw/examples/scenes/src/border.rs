//! Border App - Border 装饰器验证
//!
//! 验证 Stroke (Shape 级别) 和 Border (装饰器级别) 的功能和区别。

use novadraw::figure::border::{
    BevelBorder, BevelStyle, CompoundBorder, EtchedBorder, LineBorder, MarginBorder,
    RectangleBorder,
};

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

// ============================================================================
// Border 装饰器场景 (使用 with_border)
// ============================================================================

/// 场景 4: RectangleBorder 装饰器
fn create_scene_4_rectangle_border() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");

    let rect1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let rect2 = novadraw::figure::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 4.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let rect3 = novadraw::figure::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.3, 0.5, 0.2, 1.0), 6.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");

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

/// 场景 5: Border 装饰器 + insets
fn create_scene_5_border_with_insets() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");

    // 带 insets 的 RectangleBorder - insets 会影响子元素布局（需要布局系统支持）
    // 当前展示 insets 对边框位置的影响
    let rect1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        150.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(10.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");
    let rect2 = novadraw::figure::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        150.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 3.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(20.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");
    let rect3 = novadraw::figure::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        150.0,
        novadraw::graphics::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.3, 0.5, 0.2, 1.0), 4.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(30.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");

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

/// 场景 6: LineBorder 装饰器
fn create_scene_6_line_border() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");

    let rect1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        LineBorder::new(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let rect2 = novadraw::figure::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        LineBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 3.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let rect3 = novadraw::figure::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        LineBorder::new(novadraw::graphics::Color::rgba(0.3, 0.5, 0.2, 1.0), 4.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");

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

/// 场景 7: MarginBorder 装饰器
fn create_scene_7_margin_border() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");

    // 子 Figure 故意大于 client area，由父 Figure 的 margin client clip 截断。
    // 因此露出的父背景宽度就是各方向的实际 margin。
    let rect1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        MarginBorder::new(novadraw::geometry::Insets::uniform(10.0)).expect("valid margins"),
    )
    .expect("valid detached figure input");
    let rect2 = novadraw::figure::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        MarginBorder::new(novadraw::geometry::Insets::new(10.0, 20.0, 30.0, 40.0))
            .expect("valid margins"),
    )
    .expect("valid detached figure input");
    let rect3 = novadraw::figure::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::graphics::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        MarginBorder::new(novadraw::geometry::Insets::new(30.0, 40.0, 10.0, 20.0))
            .expect("valid margins"),
    )
    .expect("valid detached figure input");

    let r1 = scene
        .builder()
        .add_child(container_id, Box::new(rect1))
        .expect("valid FigureTree construction");
    let r2 = scene
        .builder()
        .add_child(container_id, Box::new(rect2))
        .expect("valid FigureTree construction");
    let r3 = scene
        .builder()
        .add_child(container_id, Box::new(rect3))
        .expect("valid FigureTree construction");

    scene
        .builder()
        .add_child(
            r1,
            Box::new(
                novadraw::figure::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    200.0,
                    120.0,
                    novadraw::graphics::Color::from_hex("#2563eb").expect("valid color literal"),
                )
                .expect("valid figure bounds"),
            ),
        )
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(
            r2,
            Box::new(
                novadraw::figure::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    200.0,
                    120.0,
                    novadraw::graphics::Color::from_hex("#7c3aed").expect("valid color literal"),
                )
                .expect("valid figure bounds"),
            ),
        )
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(
            r3,
            Box::new(
                novadraw::figure::RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    200.0,
                    120.0,
                    novadraw::graphics::Color::from_hex("#dc2626").expect("valid color literal"),
                )
                .expect("valid figure bounds"),
            ),
        )
        .expect("valid FigureTree construction");

    scene
}

// ============================================================================
// 对比场景
// ============================================================================

/// 场景 8: Stroke vs Border 对比
fn create_scene_8_stroke_vs_border() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container = novadraw::figure::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
        .expect("valid figure bounds");
    let container_id = scene
        .builder()
        .set_contents(Box::new(container))
        .expect("valid FigureTree construction");

    // 第一行：with_stroke (Shape 级别描边)
    // 描边绘制在图形边界上
    let stroke1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        50.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0)
    .expect("valid detached figure input");
    let stroke2 = novadraw::figure::RectangleFigure::new_with_color(
        250.0,
        50.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 4.0)
    .expect("valid detached figure input");
    let stroke3 = novadraw::figure::RectangleFigure::new_with_color(
        450.0,
        50.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.3, 0.5, 1.0), 8.0)
    .expect("valid detached figure input");

    let _s1 = scene
        .builder()
        .add_child(container_id, Box::new(stroke1))
        .expect("valid FigureTree construction");
    let _s2 = scene
        .builder()
        .add_child(container_id, Box::new(stroke2))
        .expect("valid FigureTree construction");
    let _s3 = scene
        .builder()
        .add_child(container_id, Box::new(stroke3))
        .expect("valid FigureTree construction");

    // 第二行：with_border (Border 装饰器)
    // 边框绘制在 paintBorder 阶段，可以有 insets 等高级特性
    let border1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        180.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 2.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let border2 = novadraw::figure::RectangleFigure::new_with_color(
        250.0,
        180.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 4.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");
    let border3 = novadraw::figure::RectangleFigure::new_with_color(
        450.0,
        180.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .expect("valid figure bounds")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.5, 0.2, 0.3, 1.0), 8.0)
            .expect("valid border"),
    )
    .expect("valid detached figure input");

    let _b1 = scene
        .builder()
        .add_child(container_id, Box::new(border1))
        .expect("valid FigureTree construction");
    let _b2 = scene
        .builder()
        .add_child(container_id, Box::new(border2))
        .expect("valid FigureTree construction");
    let _b3 = scene
        .builder()
        .add_child(container_id, Box::new(border3))
        .expect("valid FigureTree construction");

    // 第三行：同时有 border 和 outline（两者叠加）
    // 使用不同颜色：stroke=绿色（内），border=红色（外），insets=8 让两者分开
    let both1 = novadraw::figure::RectangleFigure::new_with_color(
        50.0,
        320.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.6, 0.2, 1.0), 2.0)
    .expect("valid detached figure input")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.8, 0.2, 0.2, 1.0), 2.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(8.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");
    let both2 = novadraw::figure::RectangleFigure::new_with_color(
        250.0,
        320.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.6, 0.2, 1.0), 4.0)
    .expect("valid detached figure input")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.8, 0.2, 0.2, 1.0), 4.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(8.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");
    let both3 = novadraw::figure::RectangleFigure::new_with_color(
        450.0,
        320.0,
        150.0,
        80.0,
        novadraw::graphics::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .expect("valid figure bounds")
    .with_stroke(novadraw::graphics::Color::rgba(0.2, 0.6, 0.2, 1.0), 6.0)
    .expect("valid detached figure input")
    .with_border(
        RectangleBorder::new(novadraw::graphics::Color::rgba(0.8, 0.2, 0.2, 1.0), 6.0)
            .expect("valid border")
            .with_insets(novadraw::geometry::Insets::uniform(8.0))
            .expect("valid insets"),
    )
    .expect("valid detached figure input");

    let _both1 = scene
        .builder()
        .add_child(container_id, Box::new(both1))
        .expect("valid FigureTree construction");
    let _both2 = scene
        .builder()
        .add_child(container_id, Box::new(both2))
        .expect("valid FigureTree construction");
    let _both3 = scene
        .builder()
        .add_child(container_id, Box::new(both3))
        .expect("valid FigureTree construction");

    scene
}

fn create_m10_composed_borders() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let container_id = scene
        .builder()
        .set_contents(Box::new(
            novadraw::figure::RectangleFigure::new_with_color(
                0.0,
                0.0,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                novadraw::graphics::Color::from_hex("#eeeeee").expect("valid color literal"),
            )
            .expect("valid figure bounds"),
        ))
        .expect("valid FigureTree construction");
    let highlight = novadraw::graphics::Color::from_hex("#ffffff").expect("valid color literal");
    let shadow = novadraw::graphics::Color::from_hex("#4b5563").expect("valid color literal");
    let fill = novadraw::graphics::Color::from_hex("#dbeafe").expect("valid color literal");

    let compound =
        novadraw::figure::RectangleFigure::new_with_color(60.0, 70.0, 190.0, 120.0, fill)
            .expect("valid figure bounds")
            .with_border(
                CompoundBorder::new(
                    LineBorder::new(
                        novadraw::graphics::Color::from_hex("#1d4ed8")
                            .expect("valid color literal"),
                        3.0,
                    )
                    .expect("valid border"),
                    MarginBorder::new(novadraw::geometry::Insets::uniform(10.0))
                        .expect("valid margins"),
                )
                .expect("valid compound border"),
            )
            .expect("valid detached figure input");
    let etched = novadraw::figure::RectangleFigure::new_with_color(305.0, 70.0, 190.0, 120.0, fill)
        .expect("valid figure bounds")
        .with_border(EtchedBorder::new(highlight, shadow))
        .expect("valid detached figure input");
    let raised = novadraw::figure::RectangleFigure::new_with_color(550.0, 70.0, 190.0, 120.0, fill)
        .expect("valid figure bounds")
        .with_border(
            BevelBorder::new(BevelStyle::Raised, highlight, shadow, 2).expect("valid bevel"),
        )
        .expect("valid detached figure input");
    let lowered =
        novadraw::figure::RectangleFigure::new_with_color(305.0, 250.0, 190.0, 120.0, fill)
            .expect("valid figure bounds")
            .with_border(
                BevelBorder::new(BevelStyle::Lowered, highlight, shadow, 2).expect("valid bevel"),
            )
            .expect("valid detached figure input");

    for figure in [
        Box::new(compound) as Box<dyn novadraw::figure::Figure>,
        Box::new(etched),
        Box::new(raised),
        Box::new(lowered),
    ] {
        scene
            .builder()
            .add_child(container_id, figure)
            .expect("valid FigureTree construction");
    }
    scene
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "border",
        "Borders",
        vec![
            SceneSpec::visual(
                "rectangle-border",
                "RectangleBorder",
                size,
                create_scene_4_rectangle_border,
            ),
            SceneSpec::visual(
                "border-insets",
                "Border+insets",
                size,
                create_scene_5_border_with_insets,
            ),
            SceneSpec::visual(
                "line-border",
                "LineBorder",
                size,
                create_scene_6_line_border,
            ),
            SceneSpec::visual(
                "margin-border",
                "MarginBorder",
                size,
                create_scene_7_margin_border,
            ),
            SceneSpec::visual(
                "stroke-vs-border",
                "Stroke vs Border",
                size,
                create_scene_8_stroke_vs_border,
            ),
            SceneSpec::visual(
                "m10-composed-borders",
                "M10 Compound / Etched / Bevel",
                size,
                create_m10_composed_borders,
            ),
        ],
    )
}
