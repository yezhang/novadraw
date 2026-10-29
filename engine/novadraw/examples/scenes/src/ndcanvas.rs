//! Graphics API 测试应用
//!
//! 通过 Figure PaintContext 测试公开绘制语言。

use novadraw::figure::Bounded;
use novadraw::graphics::{GraphicsError, LineCap, LineJoin, PaintContext};
use novadraw::{
    figure::Figure, figure::RectangleFigure, geometry::Point, geometry::Rectangle, graphics::Color,
};

use crate::{DemoSuite, SceneSpec};

// 窗口大小
const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;

// ============================================================
// 测试 Figure
// ============================================================

struct TestFigure {
    name: &'static str,
    test_fn: fn(&mut PaintContext<'_>) -> Result<(), GraphicsError>,
}

impl TestFigure {
    fn new(
        name: &'static str,
        test_fn: fn(&mut PaintContext<'_>) -> Result<(), GraphicsError>,
    ) -> Self {
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

    fn set_bounds(
        &mut self,
        _x: f64,
        _y: f64,
        _width: f64,
        _height: f64,
    ) -> Result<(), novadraw::figure::ShapeGeometryError> {
        // 不支持设置 bounds
        Ok(())
    }
}

impl Figure for TestFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        _bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        (self.test_fn)(context)
    }
}

// ============================================================
// 测试场景创建
// ============================================================

fn create_scene_clear() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let _bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_fill_rect() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 直接调用 fill_rect
    let test = TestFigure::new("fill_rect", |gc| {
        fill_rect(gc, Rectangle::new(50.0, 50.0, 200.0, 100.0), Color::RED)?;
        fill_rect(gc, Rectangle::new(300.0, 50.0, 200.0, 100.0), Color::GREEN)?;
        fill_rect(gc, Rectangle::new(550.0, 50.0, 200.0, 100.0), Color::BLUE)
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn stroke(width: f64, cap: LineCap, join: LineJoin) -> novadraw::graphics::StrokeStyle {
    novadraw::graphics::StrokeStyle::default()
        .with_width(width)
        .expect("valid demo stroke")
        .with_cap(cap)
        .with_join(join)
}

fn fill_rect(
    context: &mut PaintContext<'_>,
    rect: Rectangle,
    color: Color,
) -> Result<(), GraphicsError> {
    context.set_fill_paint(color);
    context.fill_rect(rect)
}

fn stroke_rect(
    context: &mut PaintContext<'_>,
    rect: Rectangle,
    color: Color,
    stroke: novadraw::graphics::StrokeStyle,
) -> Result<(), GraphicsError> {
    context.set_stroke_paint(color);
    context.set_stroke_style(stroke);
    context.stroke_rect(rect)
}

fn stroke_line(
    context: &mut PaintContext<'_>,
    start: Point,
    end: Point,
    color: Color,
    stroke: novadraw::graphics::StrokeStyle,
) -> Result<(), GraphicsError> {
    context.set_stroke_paint(color);
    context.set_stroke_style(stroke);
    context.stroke_line(start, end)
}

fn stroke_polyline(
    context: &mut PaintContext<'_>,
    points: &[Point],
    color: Color,
    stroke: novadraw::graphics::StrokeStyle,
) -> Result<(), GraphicsError> {
    context.set_stroke_paint(color);
    context.set_stroke_style(stroke);
    context.stroke_polyline(points)
}

fn create_scene_stroke_rect() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 直接调用 stroke_rect
    let test = TestFigure::new("stroke_rect", |gc| {
        stroke_rect(
            gc,
            Rectangle::new(50.0, 50.0, 200.0, 100.0),
            Color::RED,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;
        stroke_rect(
            gc,
            Rectangle::new(300.0, 50.0, 200.0, 100.0),
            Color::GREEN,
            stroke(5.0, LineCap::Round, LineJoin::Round),
        )?;
        stroke_rect(
            gc,
            Rectangle::new(550.0, 50.0, 200.0, 100.0),
            Color::BLUE,
            stroke(8.0, LineCap::Square, LineJoin::Bevel),
        )
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_ellipse() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 直接调用 ellipse
    let test = TestFigure::new("ellipse", |gc| {
        // 填充椭圆
        gc.set_fill_paint(Color::RED);
        gc.fill_ellipse(Rectangle::new(70.0, 100.0, 160.0, 100.0))?;
        // 描边椭圆
        gc.set_stroke_paint(Color::GREEN);
        gc.set_stroke_style(stroke(3.0, LineCap::Butt, LineJoin::Miter));
        gc.stroke_ellipse(Rectangle::new(320.0, 100.0, 160.0, 100.0))?;
        // 填充+描边椭圆
        let bounds = Rectangle::new(570.0, 100.0, 160.0, 100.0);
        gc.set_fill_paint(Color::BLUE);
        gc.fill_ellipse(bounds)?;
        gc.set_stroke_paint(Color::WHITE);
        gc.set_stroke_style(stroke(3.0, LineCap::Butt, LineJoin::Miter));
        gc.stroke_ellipse(bounds)
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_line() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 直接调用 line
    let test = TestFigure::new("line", |gc| {
        // 水平线
        stroke_line(
            gc,
            Point::new(50.0, 50.0),
            Point::new(250.0, 50.0),
            Color::RED,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;
        // 垂直线
        stroke_line(
            gc,
            Point::new(300.0, 30.0),
            Point::new(300.0, 200.0),
            Color::GREEN,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;
        // 斜线
        stroke_line(
            gc,
            Point::new(400.0, 30.0),
            Point::new(550.0, 200.0),
            Color::BLUE,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;
        // 不同线宽
        stroke_line(
            gc,
            Point::new(50.0, 250.0),
            Point::new(150.0, 250.0),
            Color::BLACK,
            stroke(1.0, LineCap::Butt, LineJoin::Miter),
        )?;
        stroke_line(
            gc,
            Point::new(170.0, 250.0),
            Point::new(270.0, 250.0),
            Color::BLACK,
            stroke(2.0, LineCap::Butt, LineJoin::Miter),
        )?;
        stroke_line(
            gc,
            Point::new(290.0, 250.0),
            Point::new(390.0, 250.0),
            Color::BLACK,
            stroke(4.0, LineCap::Butt, LineJoin::Miter),
        )?;
        stroke_line(
            gc,
            Point::new(410.0, 250.0),
            Point::new(510.0, 250.0),
            Color::BLACK,
            stroke(8.0, LineCap::Butt, LineJoin::Miter),
        )
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_polyline() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 直接调用 polyline
    let test = TestFigure::new("polyline", |gc| {
        // 简单折线
        let points1 = vec![
            Point::new(50.0, 50.0),
            Point::new(150.0, 100.0),
            Point::new(250.0, 50.0),
        ];
        stroke_polyline(
            gc,
            &points1,
            Color::RED,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // 多段折线
        let points2 = vec![
            Point::new(300.0, 30.0),
            Point::new(350.0, 80.0),
            Point::new(400.0, 30.0),
            Point::new(450.0, 80.0),
            Point::new(500.0, 30.0),
        ];
        stroke_polyline(
            gc,
            &points2,
            Color::GREEN,
            stroke(3.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // 不同线宽
        let points3 = vec![Point::new(50.0, 200.0), Point::new(200.0, 200.0)];
        stroke_polyline(
            gc,
            &points3,
            Color::BLACK,
            stroke(1.0, LineCap::Butt, LineJoin::Miter),
        )?;

        let points4 = vec![Point::new(230.0, 200.0), Point::new(380.0, 200.0)];
        stroke_polyline(
            gc,
            &points4,
            Color::BLACK,
            stroke(2.0, LineCap::Butt, LineJoin::Miter),
        )?;

        let points5 = vec![Point::new(410.0, 200.0), Point::new(560.0, 200.0)];
        stroke_polyline(
            gc,
            &points5,
            Color::BLACK,
            stroke(4.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // 不同线帽
        let points6 = vec![Point::new(50.0, 300.0), Point::new(150.0, 300.0)];
        stroke_polyline(
            gc,
            &points6,
            Color::RED,
            stroke(8.0, LineCap::Butt, LineJoin::Miter),
        )?;

        let points7 = vec![Point::new(200.0, 300.0), Point::new(300.0, 300.0)];
        stroke_polyline(
            gc,
            &points7,
            Color::GREEN,
            stroke(8.0, LineCap::Round, LineJoin::Miter),
        )?;

        let points8 = vec![Point::new(350.0, 300.0), Point::new(450.0, 300.0)];
        stroke_polyline(
            gc,
            &points8,
            Color::BLUE,
            stroke(8.0, LineCap::Square, LineJoin::Miter),
        )
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_line_join() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 测试不同连接样式
    let test = TestFigure::new("line_join", |gc| {
        // Miter
        let points1 = vec![
            Point::new(50.0, 50.0),
            Point::new(100.0, 100.0),
            Point::new(150.0, 50.0),
        ];
        stroke_polyline(
            gc,
            &points1,
            Color::RED,
            stroke(8.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // Round
        let points2 = vec![
            Point::new(200.0, 50.0),
            Point::new(250.0, 100.0),
            Point::new(300.0, 50.0),
        ];
        stroke_polyline(
            gc,
            &points2,
            Color::GREEN,
            stroke(8.0, LineCap::Butt, LineJoin::Round),
        )?;

        // Bevel
        let points3 = vec![
            Point::new(350.0, 50.0),
            Point::new(400.0, 100.0),
            Point::new(450.0, 50.0),
        ];
        stroke_polyline(
            gc,
            &points3,
            Color::BLUE,
            stroke(8.0, LineCap::Butt, LineJoin::Bevel),
        )
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

fn create_scene_transform() -> novadraw::tree::FigureTree {
    let mut scene = novadraw::tree::FigureTree::new();
    let bg = RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    let bg_id = scene
        .builder()
        .set_contents(Box::new(bg))
        .expect("valid FigureTree construction");

    // 测试变换
    let test = TestFigure::new("transform", |gc| {
        // 原始矩形
        stroke_rect(
            gc,
            Rectangle::new(50.0, 50.0, 100.0, 60.0),
            Color::RED,
            stroke(2.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // 平移
        gc.concat_transform(novadraw::geometry::Affine2D::from_translation(50.0, 50.0))?;
        stroke_rect(
            gc,
            Rectangle::new(50.0, 50.0, 100.0, 60.0),
            Color::GREEN,
            stroke(2.0, LineCap::Butt, LineJoin::Miter),
        )?;

        // 旋转
        gc.concat_transform(novadraw::geometry::Affine2D::from_translation(100.0, 50.0))?;
        gc.rotate_degrees(45.0)?;
        stroke_rect(
            gc,
            Rectangle::new(50.0, 50.0, 100.0, 60.0),
            Color::BLUE,
            stroke(2.0, LineCap::Butt, LineJoin::Miter),
        )
    });
    scene
        .builder()
        .add_child(bg_id, Box::new(test))
        .expect("valid FigureTree construction");
    scene
}

// ============================================================
// 主程序
// ============================================================

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "ndcanvas",
        "Graphics",
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
            SceneSpec::runtime_visual(
                "graphics-extension",
                "8:Graphics_Extension",
                size,
                crate::graphics::scene,
            ),
        ],
    )
}
