//! Border App - Border 装饰器验证
//!
//! 验证 Stroke (Shape 级别) 和 Border (装饰器级别) 的功能和区别。

use novadraw::border::{
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
fn create_scene_4_rectangle_border() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let rect1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0),
        2.0,
    ));
    let rect2 = novadraw::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0),
        4.0,
    ));
    let rect3 = novadraw::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.3, 0.5, 0.2, 1.0),
        6.0,
    ));

    let _r1 = scene.builder().add_child_to(container_id, Box::new(rect1));
    let _r2 = scene.builder().add_child_to(container_id, Box::new(rect2));
    let _r3 = scene.builder().add_child_to(container_id, Box::new(rect3));

    scene
}

/// 场景 5: Border 装饰器 + insets
fn create_scene_5_border_with_insets() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 带 insets 的 RectangleBorder - insets 会影响子元素布局（需要布局系统支持）
    // 当前展示 insets 对边框位置的影响
    let rect1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        150.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0)
            .with_insets(10.0, 10.0, 10.0, 10.0),
    );
    let rect2 = novadraw::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        150.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0), 3.0)
            .with_insets(20.0, 20.0, 20.0, 20.0),
    );
    let rect3 = novadraw::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        150.0,
        novadraw::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.3, 0.5, 0.2, 1.0), 4.0)
            .with_insets(30.0, 30.0, 30.0, 30.0),
    );

    let _r1 = scene.builder().add_child_to(container_id, Box::new(rect1));
    let _r2 = scene.builder().add_child_to(container_id, Box::new(rect2));
    let _r3 = scene.builder().add_child_to(container_id, Box::new(rect3));

    scene
}

/// 场景 6: LineBorder 装饰器
fn create_scene_6_line_border() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    let rect1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_border(LineBorder::new(
        novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0),
        2.0,
    ));
    let rect2 = novadraw::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(LineBorder::new(
        novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0),
        3.0,
    ));
    let rect3 = novadraw::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .with_border(LineBorder::new(
        novadraw::Color::rgba(0.3, 0.5, 0.2, 1.0),
        4.0,
    ));

    let _r1 = scene.builder().add_child_to(container_id, Box::new(rect1));
    let _r2 = scene.builder().add_child_to(container_id, Box::new(rect2));
    let _r3 = scene.builder().add_child_to(container_id, Box::new(rect3));

    scene
}

/// 场景 7: MarginBorder 装饰器
fn create_scene_7_margin_border() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 子 Figure 故意大于 client area，由父 Figure 的 margin client clip 截断。
    // 因此露出的父背景宽度就是各方向的实际 margin。
    let rect1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_border(
        MarginBorder::new(novadraw::Color::TRANSPARENT, 0.0).with_margins(10.0, 10.0, 10.0, 10.0),
    );
    let rect2 = novadraw::RectangleFigure::new_with_color(
        300.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(
        MarginBorder::new(novadraw::Color::TRANSPARENT, 0.0).with_margins(10.0, 20.0, 30.0, 40.0),
    );
    let rect3 = novadraw::RectangleFigure::new_with_color(
        550.0,
        50.0,
        200.0,
        120.0,
        novadraw::Color::rgba(1.0, 0.95, 0.9, 1.0),
    )
    .with_border(
        MarginBorder::new(novadraw::Color::TRANSPARENT, 0.0).with_margins(30.0, 40.0, 10.0, 20.0),
    );

    let r1 = scene.builder().add_child_to(container_id, Box::new(rect1));
    let r2 = scene.builder().add_child_to(container_id, Box::new(rect2));
    let r3 = scene.builder().add_child_to(container_id, Box::new(rect3));

    scene.builder().add_child_to(
        r1,
        Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            200.0,
            120.0,
            novadraw::Color::hex("#2563eb"),
        )),
    );
    scene.builder().add_child_to(
        r2,
        Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            200.0,
            120.0,
            novadraw::Color::hex("#7c3aed"),
        )),
    );
    scene.builder().add_child_to(
        r3,
        Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            200.0,
            120.0,
            novadraw::Color::hex("#dc2626"),
        )),
    );

    scene
}

// ============================================================================
// 对比场景
// ============================================================================

/// 场景 8: Stroke vs Border 对比
fn create_scene_8_stroke_vs_border() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 第一行：with_stroke (Shape 级别描边)
    // 描边绘制在图形边界上
    let stroke1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0), 2.0);
    let stroke2 = novadraw::RectangleFigure::new_with_color(
        250.0,
        50.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0), 4.0);
    let stroke3 = novadraw::RectangleFigure::new_with_color(
        450.0,
        50.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 1.0, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.3, 0.5, 1.0), 8.0);

    let _s1 = scene
        .builder()
        .add_child_to(container_id, Box::new(stroke1));
    let _s2 = scene
        .builder()
        .add_child_to(container_id, Box::new(stroke2));
    let _s3 = scene
        .builder()
        .add_child_to(container_id, Box::new(stroke3));

    // 第二行：with_border (Border 装饰器)
    // 边框绘制在 paintBorder 阶段，可以有 insets 等高级特性
    let border1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        180.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0),
        2.0,
    ));
    let border2 = novadraw::RectangleFigure::new_with_color(
        250.0,
        180.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0),
        4.0,
    ));
    let border3 = novadraw::RectangleFigure::new_with_color(
        450.0,
        180.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.95, 0.9, 1.0, 1.0),
    )
    .with_border(RectangleBorder::new(
        novadraw::Color::rgba(0.5, 0.2, 0.3, 1.0),
        8.0,
    ));

    let _b1 = scene
        .builder()
        .add_child_to(container_id, Box::new(border1));
    let _b2 = scene
        .builder()
        .add_child_to(container_id, Box::new(border2));
    let _b3 = scene
        .builder()
        .add_child_to(container_id, Box::new(border3));

    // 第三行：同时有 border 和 outline（两者叠加）
    // 使用不同颜色：stroke=绿色（内），border=红色（外），insets=8 让两者分开
    let both1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        320.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.6, 0.2, 1.0), 2.0)
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0), 2.0)
            .with_insets(8.0, 8.0, 8.0, 8.0),
    );
    let both2 = novadraw::RectangleFigure::new_with_color(
        250.0,
        320.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.6, 0.2, 1.0), 4.0)
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0), 4.0)
            .with_insets(8.0, 8.0, 8.0, 8.0),
    );
    let both3 = novadraw::RectangleFigure::new_with_color(
        450.0,
        320.0,
        150.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.95, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.6, 0.2, 1.0), 6.0)
    .with_border(
        RectangleBorder::new(novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0), 6.0)
            .with_insets(8.0, 8.0, 8.0, 8.0),
    );

    let _both1 = scene.builder().add_child_to(container_id, Box::new(both1));
    let _both2 = scene.builder().add_child_to(container_id, Box::new(both2));
    let _both3 = scene.builder().add_child_to(container_id, Box::new(both3));

    scene
}

fn create_m10_composed_borders() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let container_id =
        scene
            .builder()
            .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
                0.0,
                0.0,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                novadraw::Color::hex("#eeeeee"),
            )));
    let highlight = novadraw::Color::hex("#ffffff");
    let shadow = novadraw::Color::hex("#4b5563");
    let fill = novadraw::Color::hex("#dbeafe");

    let compound = novadraw::RectangleFigure::new_with_color(60.0, 70.0, 190.0, 120.0, fill)
        .with_border(CompoundBorder::new(
            LineBorder::new(novadraw::Color::hex("#1d4ed8"), 3.0),
            MarginBorder::new(novadraw::Color::TRANSPARENT, 1.0)
                .with_margins(10.0, 10.0, 10.0, 10.0),
        ));
    let etched = novadraw::RectangleFigure::new_with_color(305.0, 70.0, 190.0, 120.0, fill)
        .with_border(EtchedBorder::new(highlight, shadow));
    let raised = novadraw::RectangleFigure::new_with_color(550.0, 70.0, 190.0, 120.0, fill)
        .with_border(BevelBorder::new(BevelStyle::Raised, highlight, shadow, 2));
    let lowered = novadraw::RectangleFigure::new_with_color(305.0, 250.0, 190.0, 120.0, fill)
        .with_border(BevelBorder::new(BevelStyle::Lowered, highlight, shadow, 2));

    for figure in [
        Box::new(compound) as Box<dyn novadraw::Figure>,
        Box::new(etched),
        Box::new(raised),
        Box::new(lowered),
    ] {
        scene.builder().add_child_to(container_id, figure);
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
