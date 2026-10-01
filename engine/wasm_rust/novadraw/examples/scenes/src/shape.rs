//! Shape demo - 形状展示验证
//!
//! 遵循 MECE 原则设计场景：
//! - 图形类型维度: Rectangle, Ellipse, Line
//! - 属性维度: Fill, Stroke, LineCap, LineJoin
//! - 组合维度: 混合图形, 父子嵌套, Z-order

use crate::{DemoSuite, SceneSpec, ValidationKind};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

// ============================================================================
// 维度1: 图形类型验证 (MECE - 按类型分类)
// ============================================================================

/// Scene 0: 矩形图形 - 验证 RectangleFigure
/// MECE: 单独验证矩形填充
fn create_scene_0_rectangle_fill() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 不同尺寸的填充矩形
    let rect_1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0),
    );
    let rect_2 = novadraw::RectangleFigure::new_with_color(
        250.0,
        50.0,
        100.0,
        100.0,
        novadraw::Color::rgba(0.2, 0.8, 0.2, 1.0),
    );
    let rect_3 = novadraw::RectangleFigure::new_with_color(
        400.0,
        50.0,
        200.0,
        80.0,
        novadraw::Color::rgba(0.2, 0.2, 0.8, 1.0),
    );
    let rect_4 = novadraw::RectangleFigure::new_with_color(
        50.0,
        200.0,
        120.0,
        150.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    );
    let rect_5 = novadraw::RectangleFigure::new_with_color(
        200.0,
        200.0,
        100.0,
        100.0,
        novadraw::Color::rgba(0.5, 0.1, 0.9, 1.0),
    );

    scene
        .builder()
        .add_child(container_id, Box::new(rect_1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_3))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_4))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_5))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 1: 椭圆图形 - 验证 EllipseFigure
/// MECE: 单独验证椭圆填充
fn create_scene_1_ellipse_fill() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 不同尺寸的椭圆
    let ellipse_1 = novadraw::EllipseFigure::new_with_color(
        150.0,
        100.0,
        200.0,
        120.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    );
    let ellipse_2 = novadraw::EllipseFigure::new_with_color(
        450.0,
        100.0,
        150.0,
        150.0,
        novadraw::Color::rgba(0.1, 0.5, 0.9, 1.0),
    );
    let ellipse_3 = novadraw::EllipseFigure::new_with_color(
        150.0,
        300.0,
        100.0,
        100.0,
        novadraw::Color::rgba(0.5, 0.9, 0.2, 1.0),
    );
    let ellipse_4 = novadraw::EllipseFigure::new_with_color(
        350.0,
        300.0,
        180.0,
        80.0,
        novadraw::Color::rgba(0.9, 0.2, 0.5, 1.0),
    );
    let ellipse_5 = novadraw::EllipseFigure::new_with_color(
        600.0,
        300.0,
        120.0,
        120.0,
        novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
    );

    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_3))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_4))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_5))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 2: 圆角矩形
fn create_scene_2_rounded_rect() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 不同圆角半径的矩形
    let rect_0 = novadraw::RoundedRectangleFigure::new_with_color(
        50.0,
        50.0,
        150.0,
        80.0,
        0.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 2.0);
    let rect_5 = novadraw::RoundedRectangleFigure::new_with_color(
        250.0,
        50.0,
        150.0,
        80.0,
        5.0,
        novadraw::Color::rgba(0.1, 0.5, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 2.0);
    let rect_15 = novadraw::RoundedRectangleFigure::new_with_color(
        450.0,
        50.0,
        150.0,
        80.0,
        15.0,
        novadraw::Color::rgba(0.2, 0.8, 0.3, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 2.0);
    let rect_30 = novadraw::RoundedRectangleFigure::new_with_color(
        650.0,
        50.0,
        100.0,
        80.0,
        30.0,
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 2.0);

    // 纯填充（无描边）
    let rect_fill = novadraw::RoundedRectangleFigure::new_with_color(
        50.0,
        180.0,
        150.0,
        80.0,
        20.0,
        novadraw::Color::rgba(0.3, 0.3, 0.3, 1.0),
    );

    // 纯描边（无填充）
    let rect_stroke = novadraw::RoundedRectangleFigure::new(250.0, 180.0, 150.0, 80.0, 20.0)
        .with_stroke(novadraw::Color::RED, 3.0);

    // 填充 + 描边
    let rect_both = novadraw::RoundedRectangleFigure::new_with_color(
        450.0,
        180.0,
        150.0,
        80.0,
        20.0,
        novadraw::Color::rgba(0.3, 0.3, 0.3, 1.0),
    )
    .with_stroke(novadraw::Color::GREEN, 3.0);

    // 不同描边宽度
    let rect_sw_1 = novadraw::RoundedRectangleFigure::new_with_color(
        50.0,
        300.0,
        150.0,
        80.0,
        15.0,
        novadraw::Color::rgba(0.6, 0.3, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 1.0);
    let rect_sw_4 = novadraw::RoundedRectangleFigure::new_with_color(
        250.0,
        300.0,
        150.0,
        80.0,
        15.0,
        novadraw::Color::rgba(0.6, 0.3, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 4.0);
    let rect_sw_8 = novadraw::RoundedRectangleFigure::new_with_color(
        450.0,
        300.0,
        150.0,
        80.0,
        15.0,
        novadraw::Color::rgba(0.6, 0.3, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 8.0);

    // 大圆角（接近圆形）
    let rect_circle = novadraw::RoundedRectangleFigure::new_with_color(
        650.0,
        300.0,
        100.0,
        80.0,
        40.0,
        novadraw::Color::rgba(0.9, 0.6, 0.1, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 2.0);

    scene
        .builder()
        .add_child(container_id, Box::new(rect_0))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_5))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_15))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_30))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_fill))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_stroke))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_both))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_sw_1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_sw_4))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_sw_8))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(rect_circle))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 3: 折线图形 - 验证 PolylineFigure
/// MECE: 按折线属性分类（点数、线宽、线帽、连接样式、闭合）
fn create_scene_3_polyline() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::rgba(0.15, 0.15, 0.15, 1.0),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // ============================================================
    // 测试1: 不同点数的折线
    // ============================================================
    // 2点折线（直线）
    let line_2pt =
        novadraw::PolylineFigure::new_with_color(50.0, 40.0, 200.0, 40.0, novadraw::Color::WHITE)
            .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(line_2pt))
        .expect("valid FigureTree construction");

    // 3点折线（折线）
    let line_3pt = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(50.0, 80.0),
        novadraw::Point::new(125.0, 40.0),
        novadraw::Point::new(200.0, 80.0),
    ])
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(line_3pt))
        .expect("valid FigureTree construction");

    // 5点折线（多段折线）
    let line_5pt = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(50.0, 120.0),
        novadraw::Point::new(100.0, 80.0),
        novadraw::Point::new(150.0, 160.0),
        novadraw::Point::new(200.0, 120.0),
        novadraw::Point::new(250.0, 160.0),
    ])
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(line_5pt))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试2: 不同线宽
    // ============================================================
    let widths = [1.0, 2.0, 4.0, 6.0, 8.0];
    for (i, &w) in widths.iter().enumerate() {
        let line = novadraw::PolylineFigure::new_with_color(
            300.0 + i as f64 * 80.0,
            40.0,
            350.0 + i as f64 * 80.0,
            80.0,
            novadraw::Color::WHITE,
        )
        .with_width(w);
        scene
            .builder()
            .add_child(container_id, Box::new(line))
            .expect("valid FigureTree construction");
    }

    // ============================================================
    // 测试3: 不同线帽样式 (LineCap)
    // ============================================================
    let cap_butt = novadraw::PolylineFigure::new_with_color(
        50.0,
        200.0,
        150.0,
        200.0,
        novadraw::Color::rgba(1.0, 0.3, 0.3, 1.0),
    )
    .with_width(8.0)
    .with_cap(novadraw::graphics::LineCap::Butt);
    scene
        .builder()
        .add_child(container_id, Box::new(cap_butt))
        .expect("valid FigureTree construction");

    let cap_round = novadraw::PolylineFigure::new_with_color(
        200.0,
        200.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.3, 1.0, 0.3, 1.0),
    )
    .with_width(8.0)
    .with_cap(novadraw::graphics::LineCap::Round);
    scene
        .builder()
        .add_child(container_id, Box::new(cap_round))
        .expect("valid FigureTree construction");

    let cap_square = novadraw::PolylineFigure::new_with_color(
        350.0,
        200.0,
        450.0,
        200.0,
        novadraw::Color::rgba(0.3, 0.3, 1.0, 1.0),
    )
    .with_width(8.0)
    .with_cap(novadraw::graphics::LineCap::Square);
    scene
        .builder()
        .add_child(container_id, Box::new(cap_square))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试4: 不同连接样式 (LineJoin)
    // ============================================================
    // 尖角连接
    let join_miter = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(50.0, 260.0),
        novadraw::Point::new(100.0, 220.0),
        novadraw::Point::new(150.0, 300.0),
    ])
    .with_width(8.0)
    .with_join(novadraw::graphics::LineJoin::Miter)
    .with_color(novadraw::Color::rgba(1.0, 0.5, 0.0, 1.0));
    scene
        .builder()
        .add_child(container_id, Box::new(join_miter))
        .expect("valid FigureTree construction");

    // 圆角连接
    let join_round = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(200.0, 260.0),
        novadraw::Point::new(250.0, 220.0),
        novadraw::Point::new(300.0, 300.0),
    ])
    .with_width(8.0)
    .with_join(novadraw::graphics::LineJoin::Round)
    .with_color(novadraw::Color::rgba(0.0, 1.0, 1.0, 1.0));
    scene
        .builder()
        .add_child(container_id, Box::new(join_round))
        .expect("valid FigureTree construction");

    // 斜切连接
    let join_bevel = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(350.0, 260.0),
        novadraw::Point::new(400.0, 220.0),
        novadraw::Point::new(450.0, 300.0),
    ])
    .with_width(8.0)
    .with_join(novadraw::graphics::LineJoin::Bevel)
    .with_color(novadraw::Color::rgba(1.0, 0.0, 1.0, 1.0));
    scene
        .builder()
        .add_child(container_id, Box::new(join_bevel))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试5: 水平/垂直/对角线
    // ============================================================
    let h_line = novadraw::PolylineFigure::new_with_color(
        500.0,
        40.0,
        750.0,
        40.0,
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(h_line))
        .expect("valid FigureTree construction");

    let v_line = novadraw::PolylineFigure::new_with_color(
        700.0,
        50.0,
        700.0,
        150.0,
        novadraw::Color::rgba(0.2, 0.2, 0.9, 1.0),
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(v_line))
        .expect("valid FigureTree construction");

    let diag_45 = novadraw::PolylineFigure::new_with_color(
        500.0,
        200.0,
        650.0,
        350.0,
        novadraw::Color::rgba(0.2, 0.9, 0.2, 1.0),
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(diag_45))
        .expect("valid FigureTree construction");

    let diag_135 = novadraw::PolylineFigure::new_with_color(
        650.0,
        200.0,
        500.0,
        350.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(diag_135))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试6: 自相交折线
    // ============================================================
    let self_intersect = novadraw::PolylineFigure::from_points(vec![
        novadraw::Point::new(50.0, 380.0),
        novadraw::Point::new(150.0, 480.0),
        novadraw::Point::new(150.0, 380.0),
        novadraw::Point::new(50.0, 480.0),
    ])
    .with_width(2.0)
    .with_color(novadraw::Color::rgba(1.0, 1.0, 0.0, 1.0));
    scene
        .builder()
        .add_child(container_id, Box::new(self_intersect))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试7: 密集多段折线（波浪形）
    // ============================================================
    let mut points = Vec::new();
    for i in 0..20 {
        let x = 250.0 + i as f64 * 25.0;
        let y = 400.0 + (i as f64 * 25.0).sin() * 50.0;
        points.push(novadraw::Point::new(x, y));
    }
    let wave = novadraw::PolylineFigure::from_points(points)
        .with_width(2.0)
        .with_color(novadraw::Color::rgba(0.0, 0.8, 1.0, 1.0));
    scene
        .builder()
        .add_child(container_id, Box::new(wave))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试8: 短折线（端点测试）
    // ============================================================
    let short_1 = novadraw::PolylineFigure::new_with_color(
        600.0,
        380.0,
        610.0,
        390.0,
        novadraw::Color::WHITE,
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(short_1))
        .expect("valid FigureTree construction");

    let short_2 = novadraw::PolylineFigure::new_with_color(
        640.0,
        380.0,
        650.0,
        380.0,
        novadraw::Color::WHITE,
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(short_2))
        .expect("valid FigureTree construction");

    let short_3 = novadraw::PolylineFigure::new_with_color(
        680.0,
        380.0,
        680.0,
        390.0,
        novadraw::Color::WHITE,
    )
    .with_width(3.0);
    scene
        .builder()
        .add_child(container_id, Box::new(short_3))
        .expect("valid FigureTree construction");

    // ============================================================
    // 测试9: 坐标边界（靠近边界）
    // ============================================================
    let edge_1 = novadraw::PolylineFigure::new_with_color(
        0.0,
        500.0,
        100.0,
        500.0,
        novadraw::Color::rgba(1.0, 0.3, 0.7, 1.0),
    )
    .with_width(4.0);
    scene
        .builder()
        .add_child(container_id, Box::new(edge_1))
        .expect("valid FigureTree construction");

    let edge_2 = novadraw::PolylineFigure::new_with_color(
        700.0,
        500.0,
        800.0,
        500.0,
        novadraw::Color::rgba(1.0, 0.3, 0.7, 1.0),
    )
    .with_width(4.0);
    scene
        .builder()
        .add_child(container_id, Box::new(edge_2))
        .expect("valid FigureTree construction");

    scene
}

// ============================================================================
// 组合验证
// ============================================================================

/// Scene 7: 混合图形组合
/// MECE: 验证多种图形在同一场景中正确渲染
fn create_scene_8_mixed_shapes() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 组合1: 矩形 + 椭圆
    let rect_combo = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 1.0),
    )
    .with_stroke(novadraw::Color::BLACK, 2.0);
    let ellipse_combo = novadraw::EllipseFigure::new_with_color(
        125.0,
        100.0,
        80.0,
        50.0,
        novadraw::Color::rgba(0.1, 0.5, 0.9, 1.0),
    );

    // 组合2: 椭圆 + 直线
    let ellipse_combo2 = novadraw::EllipseFigure::new_with_color(
        300.0,
        80.0,
        100.0,
        60.0,
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
    );
    let line_through = novadraw::PolylineFigure::new_with_color(
        250.0,
        110.0,
        400.0,
        110.0,
        novadraw::Color::BLACK,
    )
    .with_width(2.0);

    // 组合3: 矩形框住多个图形
    let container_rect = novadraw::RectangleFigure::new_with_color(
        450.0,
        50.0,
        200.0,
        150.0,
        novadraw::Color::rgba(0.95, 0.95, 0.95, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.2, 0.2, 0.2, 1.0), 1.0);
    let inner_rect = novadraw::RectangleFigure::new_with_color(
        470.0,
        70.0,
        60.0,
        40.0,
        novadraw::Color::rgba(0.9, 0.3, 0.3, 1.0),
    );
    let inner_ellipse = novadraw::EllipseFigure::new_with_color(
        560.0,
        90.0,
        50.0,
        30.0,
        novadraw::Color::rgba(0.3, 0.9, 0.3, 1.0),
    );
    let inner_line = novadraw::PolylineFigure::new_with_color(
        480.0,
        150.0,
        620.0,
        150.0,
        novadraw::Color::rgba(0.3, 0.3, 0.9, 1.0),
    )
    .with_width(2.0);

    // 组合4: 复杂图形组合
    let complex_rect = novadraw::RectangleFigure::new_with_color(
        50.0,
        250.0,
        120.0,
        80.0,
        novadraw::Color::rgba(0.8, 0.2, 0.5, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 3.0);
    let complex_ellipse = novadraw::EllipseFigure::new_with_color(
        50.0,
        350.0,
        120.0,
        80.0,
        novadraw::Color::rgba(0.2, 0.5, 0.8, 1.0),
    )
    .with_stroke(novadraw::Color::WHITE, 3.0);
    let complex_line1 = novadraw::PolylineFigure::new_with_color(
        200.0,
        250.0,
        300.0,
        280.0,
        novadraw::Color::rgba(0.9, 0.6, 0.1, 1.0),
    )
    .with_width(3.0);
    let complex_line2 = novadraw::PolylineFigure::new_with_color(
        200.0,
        280.0,
        300.0,
        250.0,
        novadraw::Color::rgba(0.1, 0.6, 0.9, 1.0),
    )
    .with_width(3.0);
    let complex_line3 = novadraw::PolylineFigure::new_with_color(
        250.0,
        350.0,
        250.0,
        420.0,
        novadraw::Color::rgba(0.3, 0.8, 0.3, 1.0),
    )
    .with_width(4.0);

    // 组合5: 细线装饰
    let decor_line1 = novadraw::PolylineFigure::new_with_color(
        400.0,
        260.0,
        550.0,
        260.0,
        novadraw::Color::rgba(0.3, 0.3, 0.3, 1.0),
    )
    .with_width(1.0);
    let decor_line2 = novadraw::PolylineFigure::new_with_color(
        400.0,
        300.0,
        550.0,
        300.0,
        novadraw::Color::rgba(0.3, 0.3, 0.3, 1.0),
    )
    .with_width(2.0);
    let decor_line3 = novadraw::PolylineFigure::new_with_color(
        400.0,
        350.0,
        550.0,
        350.0,
        novadraw::Color::rgba(0.3, 0.3, 0.3, 1.0),
    )
    .with_width(4.0);

    // 边框装饰
    let border_outer = novadraw::RectangleFigure::new_with_color(
        600.0,
        250.0,
        150.0,
        150.0,
        novadraw::Color::rgba(0.9, 0.9, 0.9, 1.0),
    )
    .with_stroke(novadraw::Color::BLACK, 1.0);
    let border_inner = novadraw::RectangleFigure::new_with_color(
        610.0,
        260.0,
        130.0,
        130.0,
        novadraw::Color::WHITE,
    )
    .with_stroke(novadraw::Color::rgba(0.5, 0.5, 0.5, 1.0), 1.0);

    scene
        .builder()
        .add_child(container_id, Box::new(rect_combo))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_combo))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(ellipse_combo2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(line_through))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(container_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(inner_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(inner_ellipse))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(inner_line))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(complex_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(complex_ellipse))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(complex_line1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(complex_line2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(complex_line3))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(decor_line1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(decor_line2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(decor_line3))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(border_outer))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(border_inner))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 8: Z-order 遮挡关系
/// MECE: 验证后添加的图形遮挡先添加的图形
fn create_scene_9_zorder() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 底层图形（先添加）
    let bottom_rect = novadraw::RectangleFigure::new_with_color(
        100.0,
        100.0,
        300.0,
        200.0,
        novadraw::Color::BLUE,
    );
    let bottom_ellipse =
        novadraw::EllipseFigure::new_with_color(250.0, 150.0, 200.0, 100.0, novadraw::Color::GREEN);
    let bottom_line = novadraw::PolylineFigure::new_with_color(
        150.0,
        200.0,
        400.0,
        250.0,
        novadraw::Color::rgba(1.0, 1.0, 0.0, 1.0),
    )
    .with_width(8.0);

    // 中间层（遮挡部分底层）
    let mid_rect =
        novadraw::RectangleFigure::new_with_color(200.0, 150.0, 200.0, 120.0, novadraw::Color::RED);
    let mid_ellipse = novadraw::EllipseFigure::new_with_color(
        350.0,
        200.0,
        120.0,
        80.0,
        novadraw::Color::rgba(1.0, 0.65, 0.0, 1.0),
    );

    // 顶层（最后添加，遮挡所有底层）
    let top_rect = novadraw::RectangleFigure::new_with_color(
        300.0,
        180.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.5, 0.0, 0.5, 1.0),
    );

    // 验证 Z-order 的辅助标记
    let marker_1 = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        30.0,
        30.0,
        novadraw::Color::rgba(0.2, 0.2, 0.8, 1.0),
    );
    let marker_2 = novadraw::RectangleFigure::new_with_color(
        80.0,
        50.0,
        30.0,
        30.0,
        novadraw::Color::rgba(0.2, 0.8, 0.2, 1.0),
    );
    let marker_3 = novadraw::RectangleFigure::new_with_color(
        110.0,
        50.0,
        30.0,
        30.0,
        novadraw::Color::rgba(0.8, 0.2, 0.2, 1.0),
    );

    scene
        .builder()
        .add_child(container_id, Box::new(bottom_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(bottom_ellipse))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(bottom_line))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(mid_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(mid_ellipse))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(top_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(marker_1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(marker_2))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(marker_3))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 10: 三角形图形 - 验证 TriangleFigure
/// MECE: 验证等边三角形、直角三角形、描边宽度、纯填充/纯描边
fn create_scene_10_triangle() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
    );
    let container_id = scene.builder().set_contents(Box::new(container));

    // 不同线宽的三角形测试
    let tri_w1 = novadraw::TriangleFigure::new_with_direction(
        50.0,
        30.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#e74c3c").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(1.0);

    let tri_w3 = novadraw::TriangleFigure::new_with_direction(
        150.0,
        30.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#2ecc71").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(3.0);

    let tri_w5 = novadraw::TriangleFigure::new_with_direction(
        250.0,
        30.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#3498db").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(5.0);

    let tri_w10 = novadraw::TriangleFigure::new_with_direction(
        350.0,
        30.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#9b59b6").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(10.0);

    let tri_w20 = novadraw::TriangleFigure::new_with_direction(
        450.0,
        30.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#f39c12").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(20.0);

    // 不同方向的三角形
    let tri_north = novadraw::TriangleFigure::new_with_direction(
        50.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#e74c3c").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(3.0);

    let tri_south = novadraw::TriangleFigure::new_with_direction(
        150.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::South,
    )
    .with_fill_color(novadraw::Color::from_hex("#2ecc71").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(3.0);

    let tri_east = novadraw::TriangleFigure::new_with_direction(
        250.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::East,
    )
    .with_fill_color(novadraw::Color::from_hex("#3498db").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(3.0);

    let tri_west = novadraw::TriangleFigure::new_with_direction(
        350.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::West,
    )
    .with_fill_color(novadraw::Color::from_hex("#9b59b6").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::BLACK)
    .with_stroke_width(3.0);

    // 纯填充和纯描边
    let tri_fill = novadraw::TriangleFigure::new_with_direction(
        450.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::from_hex("#e91e63").expect("valid color literal"))
    .with_stroke_color(novadraw::Color::TRANSPARENT)
    .with_stroke_width(0.0);

    let tri_stroke = novadraw::TriangleFigure::new_with_direction(
        550.0,
        150.0,
        60.0,
        60.0,
        novadraw::figure::Direction::North,
    )
    .with_fill_color(novadraw::Color::TRANSPARENT)
    .with_stroke_color(novadraw::Color::from_hex("#e91e63").expect("valid color literal"))
    .with_stroke_width(3.0);

    scene
        .builder()
        .add_child(container_id, Box::new(tri_w1))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_w3))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_w5))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_w10))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_w20))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_north))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_south))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_east))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_west))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_fill))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(container_id, Box::new(tri_stroke))
        .expect("valid FigureTree construction");

    scene
}

/// Scene 9: 父子嵌套结构
/// MECE: 验证 Figure 嵌套渲染
fn create_scene_11_parent_child() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();

    let container = novadraw::RectangleFigure::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT);
    let container_id = scene.builder().set_contents(Box::new(container));

    // 父矩形
    let parent_rect = novadraw::RectangleFigure::new_with_color(
        50.0,
        50.0,
        350.0,
        250.0,
        novadraw::Color::rgba(0.9, 0.5, 0.1, 0.3),
    )
    .with_stroke(novadraw::Color::rgba(1.0, 0.65, 0.0, 1.0), 2.0);
    let parent_id = scene
        .builder()
        .add_child(container_id, Box::new(parent_rect))
        .expect("valid FigureTree construction");

    // 子图形 - 直接添加到父矩形中
    let child_rect = novadraw::RectangleFigure::new_with_color(
        80.0,
        80.0,
        100.0,
        60.0,
        novadraw::Color::rgba(0.2, 0.5, 0.9, 1.0),
    );
    let child_ellipse = novadraw::EllipseFigure::new_with_color(
        200.0,
        100.0,
        80.0,
        50.0,
        novadraw::Color::rgba(0.9, 0.2, 0.2, 1.0),
    );
    let child_line = novadraw::PolylineFigure::new_with_color(
        100.0,
        200.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.2, 0.8, 0.2, 1.0),
    )
    .with_width(3.0);

    scene
        .builder()
        .add_child(parent_id, Box::new(child_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(parent_id, Box::new(child_ellipse))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(parent_id, Box::new(child_line))
        .expect("valid FigureTree construction");

    // 另一个父容器
    let parent_rect2 = novadraw::RectangleFigure::new_with_color(
        450.0,
        50.0,
        300.0,
        200.0,
        novadraw::Color::rgba(0.5, 0.2, 0.8, 0.3),
    )
    .with_stroke(novadraw::Color::rgba(0.5, 0.0, 0.5, 1.0), 2.0);
    let parent_id2 = scene
        .builder()
        .add_child(container_id, Box::new(parent_rect2))
        .expect("valid FigureTree construction");

    // 子图形 - 更深层次
    let grandchild_rect = novadraw::RectangleFigure::new_with_color(
        480.0,
        80.0,
        80.0,
        60.0,
        novadraw::Color::rgba(0.3, 0.8, 0.5, 1.0),
    );
    let grandchild_ellipse = novadraw::EllipseFigure::new_with_color(
        580.0,
        100.0,
        60.0,
        40.0,
        novadraw::Color::rgba(0.8, 0.5, 0.2, 1.0),
    );

    scene
        .builder()
        .add_child(parent_id2, Box::new(grandchild_rect))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(parent_id2, Box::new(grandchild_ellipse))
        .expect("valid FigureTree construction");

    // 独立图形（不嵌套）
    let standalone = novadraw::RectangleFigure::new_with_color(
        50.0,
        350.0,
        150.0,
        100.0,
        novadraw::Color::rgba(0.1, 0.7, 0.7, 1.0),
    )
    .with_stroke(novadraw::Color::rgba(0.0, 1.0, 1.0, 1.0), 2.0);
    scene
        .builder()
        .add_child(container_id, Box::new(standalone))
        .expect("valid FigureTree construction");

    scene
}

// ============================================================================
// 场景映射
// ============================================================================

fn create_m10_runtime_mutations() -> novadraw::Runtime {
    let mut runtime = novadraw::Runtime::empty();
    let root = runtime
        .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
        )))
        .expect("valid Runtime mutation");
    let polyline = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            novadraw::PolylineFigure::from_points(vec![
                novadraw::Point::new(70.0, 100.0),
                novadraw::Point::new(210.0, 100.0),
            ])
            .with_color(novadraw::Color::from_hex("#2563eb").expect("valid color literal"))
            .with_width(5.0),
        ))
        .expect("valid Runtime mutation");
    let rounded = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            novadraw::RoundedRectangleFigure::new_with_color(
                300.0,
                70.0,
                180.0,
                110.0,
                12.0,
                novadraw::Color::from_hex("#16a34a").expect("valid color literal"),
            )
            .with_stroke(
                novadraw::Color::from_hex("#14532d").expect("valid color literal"),
                3.0,
            ),
        ))
        .expect("valid Runtime mutation");
    let triangle = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            novadraw::TriangleFigure::new(570.0, 70.0, 120.0, 120.0).with_fill_color(
                novadraw::Color::from_hex("#f59e0b").expect("valid color literal"),
            ),
        ))
        .expect("valid Runtime mutation");

    runtime
        .point_list(polyline)
        .unwrap()
        .insert_point(1, novadraw::Point::new(140.0, 180.0))
        .expect("valid polyline mutation");
    runtime
        .rounded_rectangle(rounded)
        .unwrap()
        .set_corner_dimensions(novadraw::Dimension::new(64.0, 28.0))
        .expect("valid corner mutation");
    runtime
        .triangle(triangle)
        .unwrap()
        .set_direction(novadraw::figure::Direction::West)
        .expect("valid direction mutation");
    runtime
}

fn create_p2_scalable_polygon() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let root = scene
        .builder()
        .set_contents(Box::new(novadraw::RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            novadraw::Color::from_hex("#eeeeee").expect("valid color literal"),
        )));
    let template = novadraw::PointList::from_points(vec![
        novadraw::Point::new(0.0, 12.0),
        novadraw::Point::new(18.0, 0.0),
        novadraw::Point::new(36.0, 12.0),
        novadraw::Point::new(27.0, 32.0),
        novadraw::Point::new(9.0, 32.0),
    ]);
    let stretch = novadraw::ScalablePolygonFigure::new(
        novadraw::Rectangle::new(80.0, 90.0, 260.0, 140.0),
        template.clone(),
    )
    .expect("valid scalable polygon")
    .with_fill_color(novadraw::Color::from_hex("#0ea5e9").expect("valid color literal"))
    .with_stroke(
        novadraw::Color::from_hex("#0c4a6e").expect("valid color literal"),
        4.0,
        novadraw::render::LineJoin::Miter,
    );
    let preserved = novadraw::ScalablePolygonFigure::new(
        novadraw::Rectangle::new(430.0, 90.0, 260.0, 140.0),
        template,
    )
    .expect("valid scalable polygon")
    .with_scale_mode(novadraw::PolygonScaleMode::PreserveAspect)
    .with_fill_color(novadraw::Color::from_hex("#22c55e").expect("valid color literal"))
    .with_stroke(
        novadraw::Color::from_hex("#14532d").expect("valid color literal"),
        4.0,
        novadraw::render::LineJoin::Round,
    );
    scene
        .builder()
        .add_child(root, Box::new(stretch))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(root, Box::new(preserved))
        .expect("valid FigureTree construction");
    scene
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "shape",
        "Shapes",
        vec![
            SceneSpec::new(
                "rectangle-fill",
                "0:Rectangle Fill",
                size,
                ValidationKind::Visual,
                create_scene_0_rectangle_fill,
            ),
            SceneSpec::new(
                "ellipse-fill",
                "1:Ellipse Fill",
                size,
                ValidationKind::Visual,
                create_scene_1_ellipse_fill,
            ),
            SceneSpec::new(
                "rounded-rect",
                "2:Rounded Rect",
                size,
                ValidationKind::Visual,
                create_scene_2_rounded_rect,
            ),
            SceneSpec::new(
                "polyline",
                "3:Polyline",
                size,
                ValidationKind::Visual,
                create_scene_3_polyline,
            ),
            SceneSpec::new(
                "mixed-shapes",
                "4:Mixed Shapes",
                size,
                ValidationKind::Visual,
                create_scene_8_mixed_shapes,
            ),
            SceneSpec::new(
                "z-order",
                "5:Z-Order",
                size,
                ValidationKind::Visual,
                create_scene_9_zorder,
            ),
            SceneSpec::new(
                "triangle",
                "6:Triangle",
                size,
                ValidationKind::Visual,
                create_scene_10_triangle,
            ),
            SceneSpec::new(
                "parent-child",
                "7:Parent-Child",
                size,
                ValidationKind::Visual,
                create_scene_11_parent_child,
            ),
            SceneSpec::runtime(
                "m10-runtime-mutations",
                "8:M10 Runtime Mutations",
                size,
                ValidationKind::Visual,
                create_m10_runtime_mutations,
            ),
            SceneSpec::new(
                "p2-scalable-polygon",
                "9:P2 Scalable Polygon",
                size,
                ValidationKind::Visual,
                create_p2_scalable_polygon,
            ),
        ],
    )
}
