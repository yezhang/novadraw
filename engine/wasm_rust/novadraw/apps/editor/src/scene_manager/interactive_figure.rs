use novadraw::NdCanvas;
use novadraw::{
    Bounded, Color, MouseButton, MouseEvent, NovadrawContext, Rectangle, Shape, Updatable,
    command::{LineCap, LineJoin},
};

pub struct InteractiveRectFigure {
    bounds: Rectangle,
    normal_fill: Color,
    normal_stroke: Color,
}

impl InteractiveRectFigure {
    #[allow(clippy::too_many_arguments)]
    pub fn with_palette(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        normal_fill: Color,
        _hover_fill: Color,
        _pressed_fill: Color,
        normal_stroke: Color,
        _selected_stroke: Color,
    ) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            normal_fill,
            normal_stroke,
        }
    }

    fn fill(&self) -> Color {
        self.normal_fill
    }

    fn stroke(&self) -> Color {
        self.normal_stroke
    }
}

impl Bounded for InteractiveRectFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "InteractiveRectFigure"
    }
}

impl Updatable for InteractiveRectFigure {
    fn validate(&mut self) {}

    fn invalidate(&mut self) {}
}

impl Shape for InteractiveRectFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(self.stroke())
    }

    fn stroke_width(&self) -> f64 {
        4.0
    }

    fn fill_color(&self) -> Option<Color> {
        Some(self.fill())
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        gc.fill_rect(0.0, 0.0, bounds.width, bounds.height, self.fill());
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.bounds;
        gc.stroke_rect(
            2.0,
            2.0,
            (bounds.width - 4.0).max(0.0),
            (bounds.height - 4.0).max(0.0),
            self.stroke(),
            4.0,
            LineCap::default(),
            LineJoin::default(),
        );
    }

    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        if event.button == MouseButton::Left {
            ctx.select_target();
        }
        ctx.repaint(None);
        true
    }

    fn on_mouse_released(&self, _event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        ctx.repaint(None);
        true
    }

    fn on_mouse_entered(&self, _event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        ctx.repaint(None);
        true
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        ctx.repaint(None);
        true
    }
}
