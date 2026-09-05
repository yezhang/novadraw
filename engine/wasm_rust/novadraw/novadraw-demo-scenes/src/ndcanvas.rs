//! NdCanvas API 测试应用
//!
//! 直接调用 NdCanvas API 测试每个渲染命令

use novadraw::command::{LineCap, LineJoin};
use novadraw::{Bounded, Color, Figure, NdCanvas, Rectangle, RectangleFigure};

use crate::{DemoSuite, SceneSpec};

// 窗口大小
const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

// ============================================================
// 测试 Figure - 直接调用 NdCanvas API
// ============================================================

struct TestFigure {
    name: &'static str,
    test_fn: fn(&mut NdCanvas),
}

impl TestFigure {
    fn new(name: &'static str, test_fn: fn(&mut NdCanvas)) -> Self {
        Self { name, test_fn }
    }
}

impl Bounded for TestFigure {
    fn bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT)
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn set_bounds(&mut self, _x: f64, _y: f64, _width: f64, _height: f64) {
        // 不支持设置 bounds
    }
}

impl Figure for TestFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        (self.test_fn)(gc);
    }
}

// ============================================================
// 测试场景创建
// ============================================================

fn create_scene_clear() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let _bg_id = scene.builder().set_contents(Box::new(bg));
    scene
}

fn create_scene_fill_rect() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 直接调用 fill_rect
    let test = TestFigure::new("fill_rect", |gc| {
        gc.fill_rect(50.0, 50.0, 200.0, 100.0, Color::rgba(1.0, 0.0, 0.0, 1.0)); // 红色
        gc.fill_rect(300.0, 50.0, 200.0, 100.0, Color::rgba(0.0, 1.0, 0.0, 1.0)); // 绿色
        gc.fill_rect(550.0, 50.0, 200.0, 100.0, Color::rgba(0.0, 0.0, 1.0, 1.0)); // 蓝色
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_stroke_rect() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 直接调用 stroke_rect
    let test = TestFigure::new("stroke_rect", |gc| {
        gc.stroke_rect(
            50.0,
            50.0,
            200.0,
            100.0,
            Color::RED,
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        gc.stroke_rect(
            300.0,
            50.0,
            200.0,
            100.0,
            Color::GREEN,
            5.0,
            LineCap::Round,
            LineJoin::Round,
        );
        gc.stroke_rect(
            550.0,
            50.0,
            200.0,
            100.0,
            Color::BLUE,
            8.0,
            LineCap::Square,
            LineJoin::Bevel,
        );
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_ellipse() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 直接调用 ellipse
    let test = TestFigure::new("ellipse", |gc| {
        // 填充椭圆
        gc.ellipse(
            150.0,
            150.0,
            80.0,
            50.0,
            Some(Color::RED),
            None,
            0.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        // 描边椭圆
        gc.ellipse(
            400.0,
            150.0,
            80.0,
            50.0,
            None,
            Some(Color::GREEN),
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        // 填充+描边椭圆
        gc.ellipse(
            650.0,
            150.0,
            80.0,
            50.0,
            Some(Color::BLUE),
            Some(Color::WHITE),
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_line() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 直接调用 line
    let test = TestFigure::new("line", |gc| {
        // 水平线
        gc.line(
            glam::DVec2::new(50.0, 50.0),
            glam::DVec2::new(250.0, 50.0),
            Color::RED,
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        // 垂直线
        gc.line(
            glam::DVec2::new(300.0, 30.0),
            glam::DVec2::new(300.0, 200.0),
            Color::GREEN,
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        // 斜线
        gc.line(
            glam::DVec2::new(400.0, 30.0),
            glam::DVec2::new(550.0, 200.0),
            Color::BLUE,
            3.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        // 不同线宽
        gc.line(
            glam::DVec2::new(50.0, 250.0),
            glam::DVec2::new(150.0, 250.0),
            Color::BLACK,
            1.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        gc.line(
            glam::DVec2::new(170.0, 250.0),
            glam::DVec2::new(270.0, 250.0),
            Color::BLACK,
            2.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        gc.line(
            glam::DVec2::new(290.0, 250.0),
            glam::DVec2::new(390.0, 250.0),
            Color::BLACK,
            4.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
        gc.line(
            glam::DVec2::new(410.0, 250.0),
            glam::DVec2::new(510.0, 250.0),
            Color::BLACK,
            8.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_polyline() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 直接调用 polyline
    let test = TestFigure::new("polyline", |gc| {
        // 简单折线
        let points1 = vec![
            glam::DVec2::new(50.0, 50.0),
            glam::DVec2::new(150.0, 100.0),
            glam::DVec2::new(250.0, 50.0),
        ];
        gc.polyline(&points1, Color::RED, 3.0, LineCap::Butt, LineJoin::Miter);

        // 多段折线
        let points2 = vec![
            glam::DVec2::new(300.0, 30.0),
            glam::DVec2::new(350.0, 80.0),
            glam::DVec2::new(400.0, 30.0),
            glam::DVec2::new(450.0, 80.0),
            glam::DVec2::new(500.0, 30.0),
        ];
        gc.polyline(&points2, Color::GREEN, 3.0, LineCap::Butt, LineJoin::Miter);

        // 不同线宽
        let points3 = vec![
            glam::DVec2::new(50.0, 200.0),
            glam::DVec2::new(200.0, 200.0),
        ];
        gc.polyline(&points3, Color::BLACK, 1.0, LineCap::Butt, LineJoin::Miter);

        let points4 = vec![
            glam::DVec2::new(230.0, 200.0),
            glam::DVec2::new(380.0, 200.0),
        ];
        gc.polyline(&points4, Color::BLACK, 2.0, LineCap::Butt, LineJoin::Miter);

        let points5 = vec![
            glam::DVec2::new(410.0, 200.0),
            glam::DVec2::new(560.0, 200.0),
        ];
        gc.polyline(&points5, Color::BLACK, 4.0, LineCap::Butt, LineJoin::Miter);

        // 不同线帽
        let points6 = vec![
            glam::DVec2::new(50.0, 300.0),
            glam::DVec2::new(150.0, 300.0),
        ];
        gc.polyline(&points6, Color::RED, 8.0, LineCap::Butt, LineJoin::Miter);

        let points7 = vec![
            glam::DVec2::new(200.0, 300.0),
            glam::DVec2::new(300.0, 300.0),
        ];
        gc.polyline(&points7, Color::GREEN, 8.0, LineCap::Round, LineJoin::Miter);

        let points8 = vec![
            glam::DVec2::new(350.0, 300.0),
            glam::DVec2::new(450.0, 300.0),
        ];
        gc.polyline(&points8, Color::BLUE, 8.0, LineCap::Square, LineJoin::Miter);
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_line_join() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 测试不同连接样式
    let test = TestFigure::new("line_join", |gc| {
        // Miter
        let points1 = vec![
            glam::DVec2::new(50.0, 50.0),
            glam::DVec2::new(100.0, 100.0),
            glam::DVec2::new(150.0, 50.0),
        ];
        gc.polyline(&points1, Color::RED, 8.0, LineCap::Butt, LineJoin::Miter);

        // Round
        let points2 = vec![
            glam::DVec2::new(200.0, 50.0),
            glam::DVec2::new(250.0, 100.0),
            glam::DVec2::new(300.0, 50.0),
        ];
        gc.polyline(&points2, Color::GREEN, 8.0, LineCap::Butt, LineJoin::Round);

        // Bevel
        let points3 = vec![
            glam::DVec2::new(350.0, 50.0),
            glam::DVec2::new(400.0, 100.0),
            glam::DVec2::new(450.0, 50.0),
        ];
        gc.polyline(&points3, Color::BLUE, 8.0, LineCap::Butt, LineJoin::Bevel);
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

fn create_scene_transform() -> novadraw::FigureTree {
    let mut scene = novadraw::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE);
    let bg_id = scene.builder().set_contents(Box::new(bg));

    // 测试变换
    let test = TestFigure::new("transform", |gc| {
        // 原始矩形
        gc.stroke_rect(
            50.0,
            50.0,
            100.0,
            60.0,
            Color::RED,
            2.0,
            LineCap::Butt,
            LineJoin::Miter,
        );

        // 平移
        gc.translate(50.0, 50.0);
        gc.stroke_rect(
            50.0,
            50.0,
            100.0,
            60.0,
            Color::GREEN,
            2.0,
            LineCap::Butt,
            LineJoin::Miter,
        );

        // 旋转
        gc.translate(100.0, 50.0);
        gc.rotate(45.0);
        gc.stroke_rect(
            50.0,
            50.0,
            100.0,
            60.0,
            Color::BLUE,
            2.0,
            LineCap::Butt,
            LineJoin::Miter,
        );
    });
    scene.builder().add_child_to(bg_id, Box::new(test));
    scene
}

// ============================================================
// 主程序
// ============================================================

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "ndcanvas",
        "NdCanvas",
        vec![
            SceneSpec::visual("clear", "0:Clear", size, create_scene_clear),
            SceneSpec::visual("fill-rect", "1:FillRect", size, create_scene_fill_rect),
            SceneSpec::visual(
                "stroke-rect",
                "2:StrokeRect",
                size,
                create_scene_stroke_rect,
            ),
            SceneSpec::visual("ellipse", "3:Ellipse", size, create_scene_ellipse),
            SceneSpec::visual("line", "4:Line", size, create_scene_line),
            SceneSpec::visual("polyline", "5:Polyline", size, create_scene_polyline),
            SceneSpec::visual("line-join", "6:LineJoin", size, create_scene_line_join),
            SceneSpec::visual("transform", "7:Transform", size, create_scene_transform),
        ],
    )
}
