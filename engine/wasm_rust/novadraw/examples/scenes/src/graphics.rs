//! External consumer of the P2-G01 public Graphics contracts.

use novadraw::container::LayerFigure;
use novadraw::graphics::{
    ClipPath, CustomDash, DashPattern, FillRule, GradientStop, LinearGradient, Paint, Path,
    StrokeStyle,
};
use novadraw::render::{
    BuiltinFont, FontDescriptor, ImageData, ImageResourceRef, TextConstraints, TextLayout,
};
use novadraw::{
    Color, Figure, FigureId, NdCanvas, Point, PolylineFigure, Rectangle, RectangleFigure, Runtime,
};

pub const SIZE: (u32, u32) = (800, 600);
const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);
const GREEN: Color = Color::rgba(0.0, 1.0, 0.0, 1.0);
const TEXT_SIZE: f32 = 48.0;

fn gradient(start: Point, end: Point, stops: &[(f64, Color)]) -> Paint {
    LinearGradient::try_new(
        start,
        end,
        &stops
            .iter()
            .map(|&(offset, color)| GradientStop::try_new(offset, color).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap()
    .into()
}

fn red_blue(x: f64, width: f64) -> Paint {
    gradient(
        Point::new(x, 0.0),
        Point::new(x + width, 0.0),
        &[(0.0, RED), (1.0, BLUE)],
    )
}

fn dash() -> StrokeStyle {
    StrokeStyle::default()
        .with_width(4.0)
        .unwrap()
        .with_dash_pattern(DashPattern::Custom(
            CustomDash::try_new(&[12.0, 8.0]).unwrap(),
        ))
        .with_dash_offset(4.0)
        .unwrap()
}

struct GraphicsFigure {
    layout: TextLayout,
    image: ImageResourceRef,
}

impl Figure for GraphicsFigure {
    fn name(&self) -> &'static str {
        "GraphicsExtension"
    }

    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, SIZE.0 as f64, SIZE.1 as f64)
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        // Three analytically predictable interpolation probes.
        gc.set_fill_paint(red_blue(40.0, 200.0));
        gc.fill_rectangle(40.0, 30.0, 200.0, 50.0);
        gc.set_fill_paint(gradient(
            Point::new(280.0, 0.0),
            Point::new(480.0, 0.0),
            &[(0.0, RED), (1.0, BLUE.with_alpha(0.0))],
        ));
        gc.set_alpha(0.5);
        gc.fill_rectangle(280.0, 30.0, 200.0, 50.0);
        gc.set_alpha(1.0);
        gc.set_fill_paint(gradient(
            Point::new(520.0, 0.0),
            Point::new(720.0, 0.0),
            &[(0.0, RED), (0.5, RED), (0.5, BLUE), (1.0, BLUE)],
        ));
        gc.fill_rectangle(520.0, 30.0, 200.0, 50.0);

        // Identical brush coordinates in shape, fill glyph and dashed glyph.
        gc.set_fill_paint(red_blue(40.0, 240.0));
        gc.fill_rectangle(40.0, 110.0, 240.0, 8.0);
        gc.fill_text_layout(&self.layout, 40.0, 125.0);
        gc.set_stroke_paint(red_blue(40.0, 240.0));
        gc.set_stroke(dash());
        gc.stroke_text_layout(&self.layout, 40.0, 195.0);

        // EvenOdd hole, image intersection and an empty clip restored in place.
        let mut path = Path::new();
        path.move_to(360.0, 110.0);
        path.line_to(500.0, 110.0);
        path.quad_to(520.0, 110.0, 520.0, 130.0);
        path.line_to(520.0, 230.0);
        path.quad_to(520.0, 250.0, 500.0, 250.0);
        path.line_to(360.0, 250.0);
        path.quad_to(340.0, 250.0, 340.0, 230.0);
        path.line_to(340.0, 130.0);
        path.quad_to(340.0, 110.0, 360.0, 110.0);
        path.close();
        path.rect(400.0, 150.0, 60.0, 60.0);
        gc.push_state();
        gc.clip_rect(350.0, 100.0, 160.0, 170.0);
        gc.clip_path(&ClipPath::try_new(&path, FillRule::EvenOdd).unwrap());
        gc.set_fill_paint(red_blue(340.0, 180.0));
        gc.fill_rectangle(320.0, 100.0, 220.0, 170.0);
        gc.set_fill_paint(Color::WHITE.into());
        gc.fill_text_layout(&self.layout, 340.0, 125.0);
        gc.draw_image_with_size(self.image, 340.0, 220.0, 180.0, 50.0);
        gc.push_state();
        gc.clip_path(&ClipPath::try_new(&Path::new(), FillRule::NonZero).unwrap());
        gc.fill_rect_with_color(0.0, 0.0, 800.0, 600.0, Color::BLACK);
        gc.reset_clip();
        gc.fill_rect_with_color(700.0, 250.0, 20.0, 20.0, RED);
        gc.restore_state();
        gc.fill_rect_with_color(700.0, 250.0, 20.0, 20.0, GREEN);
        gc.fill_rect_with_color(350.0, 110.0, 20.0, 20.0, GREEN);
        gc.pop_state();
        gc.pop_state();

        // A clip retains the transform at its call, while the brush follows draw.
        gc.push_state();
        gc.clip_rect(560.0, 120.0, 140.0, 120.0);
        gc.translate(620.0, 140.0);
        gc.rotate(90.0);
        gc.scale(1.5, 0.75);
        gc.set_fill_paint(red_blue(0.0, 60.0));
        gc.fill_rectangle(0.0, -40.0, 60.0, 80.0);
        gc.pop_state();

        // Same logical dash lengths after a nonuniform scale.
        gc.set_stroke_paint(Color::BLACK.into());
        gc.set_stroke(dash());
        gc.begin_path();
        gc.move_to(40.0, 300.0);
        gc.line_to(280.0, 300.0);
        gc.stroke();
        gc.push_state();
        gc.translate(340.0, 300.0);
        gc.scale(1.5, 2.0);
        gc.begin_path();
        gc.move_to(0.0, 0.0);
        gc.line_to(160.0, 0.0);
        gc.stroke();
        gc.pop_state();

        // Overlapping light/dark content underneath the retained feedback.
        gc.fill_rect_with_color(40.0, 380.0, 320.0, 160.0, Color::BLACK);
        gc.fill_rect_with_color(200.0, 420.0, 320.0, 120.0, Color::WHITE);
        gc.set_fill_paint(red_blue(80.0, 400.0));
        gc.fill_rectangle(80.0, 460.0, 400.0, 40.0);
        // Deliberately leave a changed clip/paint for traversal to restore.
        gc.clip_path(&ClipPath::try_new(&Path::new(), FillRule::NonZero).unwrap());
    }
}

pub struct GraphicsScene {
    pub runtime: Runtime,
    pub feedback_layer: FigureId,
    pub miter: FigureId,
    pub feedback: Option<FigureId>,
}

impl GraphicsScene {
    pub fn show_feedback(&mut self) {
        self.feedback = Some(
            self.runtime
                .container(self.feedback_layer)
                .unwrap()
                .add(Box::new(
                    RectangleFigure::new_with_color(120.0, 400.0, 260.0, 120.0, Color::TRANSPARENT)
                        .with_stroke(Color::rgba(1.0, 0.6, 0.0, 1.0), 6.0),
                ))
                .unwrap(),
        );
    }

    pub fn move_feedback(&mut self) {
        self.runtime
            .figure(self.feedback.unwrap())
            .unwrap()
            .set_bounds(Rectangle::new(180.0, 410.0, 260.0, 120.0))
            .unwrap();
    }

    pub fn cancel_feedback(&mut self) {
        self.runtime
            .container(self.feedback_layer)
            .unwrap()
            .remove(self.feedback.take().unwrap())
            .unwrap();
    }
}

pub fn fixture() -> GraphicsScene {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let layout = runtime
        .layout_text(
            "HHHHHH",
            &FontDescriptor::new("Inter Variable", TEXT_SIZE).unwrap(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let image = runtime.register_image();
    runtime
        .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 255, 0, 255], 1.0))
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            SIZE.0 as f64,
            SIZE.1 as f64,
            Color::WHITE,
        )))
        .unwrap();
    runtime
        .container(root)
        .unwrap()
        .add(Box::new(GraphicsFigure {
            layout,
            image: ImageResourceRef::new(image.resource_id(), 1, 1, 1, 1.0),
        }))
        .unwrap();
    // A real sibling must survive the preceding Figure's empty clip.
    runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            720.0, 280.0, 40.0, 40.0, GREEN,
        )))
        .unwrap();
    let miter = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            PolylineFigure::from_points(vec![
                Point::new(580.0, 530.0),
                Point::new(620.0, 430.0),
                Point::new(660.0, 530.0),
            ])
            .with_color(Color::BLACK)
            .with_stroke_style(
                StrokeStyle::default()
                    .with_width(16.0)
                    .unwrap()
                    .with_miter_limit(1.0)
                    .unwrap(),
            ),
        ))
        .unwrap();
    let feedback_layer = runtime
        .container(root)
        .unwrap()
        .add(Box::new(LayerFigure::new(
            0.0,
            0.0,
            SIZE.0 as f64,
            SIZE.1 as f64,
        )))
        .unwrap();
    GraphicsScene {
        runtime,
        feedback_layer,
        miter,
        feedback: None,
    }
}

pub fn scene() -> Runtime {
    fixture().runtime
}
