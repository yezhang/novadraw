//! Canvas port of path/{PathExample,PathFigure,ZoomFigure}.java (EPL-2.0).
use super::runtime_with_fonts;
use novadraw::{
    container::{
        DefaultRangeModel, ScalableLayeredPaneFigure, ScrollBarConfig, ScrollBarFigure,
        ScrollBarPart, ScrollBarValueConsumer, ScrollOrientation,
    },
    event::EventContext,
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
        FigureMeasurement, FigurePreparation, FigurePresentation, LabelFigure, PREPARATION,
        PreparationCapability, RectangleFigure,
    },
    geometry::{Affine2D, Dimension, Point, Rectangle, Translatable},
    graphics::{Color, GraphicsError, PaintContext, Path},
    layout::{BorderConstraint, BorderLayout, BorderRegion, ToolbarLayout},
    runtime::Runtime,
    text::{
        FontDecorationMetrics, MeasureContext, SelectionQuad, TextAffinity, TextConstraints,
        TextError, TextLayout, TextPosition, TextRange,
    },
    tree::FigureId,
};
use std::sync::Arc;

const WINDOW: f64 = 800.0;
const BAR: f64 = 24.0;
const LINE: f64 = 3.0;
const ROTATION: f64 = 45.0;
const CENTER: f64 = 85.0;
const TEXT_OFFSET: f64 = 20.0;

struct ZoomValue(FigureId);
impl ScrollBarValueConsumer for ZoomValue {
    fn value_changed(&self, _: f64, value: f64, context: &mut EventContext<'_>) {
        context
            .effects()
            .set_scale_later(self.0, f64::from((value as f32 + 50.0) * 0.02_f32));
    }
}

struct PathFigure;
fn rotated(point: Point) -> Point {
    let angle = ROTATION.to_radians();
    let x = point.x() - CENTER;
    let y = point.y() - CENTER;
    // The reference helper rounds each rotated component through Java float.
    Point::new(
        f64::from((x * angle.cos() - y * angle.sin()) as f32) + CENTER,
        f64::from((x * angle.sin() + y * angle.cos()) as f32) + CENTER,
    )
}
fn path_geometry() -> (Path, Rectangle) {
    let points = [(50.0, 50.0), (120.0, 50.0), (120.0, 120.0), (50.0, 120.0)]
        .map(|(x, y)| rotated(Point::new(x, y)));
    let left = points.iter().map(|p| p.x()).fold(f64::INFINITY, f64::min) - LINE / 2.0;
    let top = points.iter().map(|p| p.y()).fold(f64::INFINITY, f64::min) - LINE / 2.0;
    let right = points
        .iter()
        .map(|p| p.x())
        .fold(f64::NEG_INFINITY, f64::max)
        + LINE / 2.0;
    let bottom = points
        .iter()
        .map(|p| p.y())
        .fold(f64::NEG_INFINITY, f64::max)
        + LINE / 2.0;
    let mut path = Path::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(p.x(), p.y());
        } else {
            path.line_to(p.x(), p.y());
        }
    }
    path.close();
    (path, Rectangle::new(left, top, right - left, bottom - top))
}
impl Figure for PathFigure {
    fn initial_bounds(&self) -> Rectangle {
        path_geometry().1
    }
    fn name(&self) -> &'static str {
        "OfficialPathFigure"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
fn quads(layout: &TextLayout, start: usize, end: usize) -> Result<Vec<SelectionQuad>, TextError> {
    layout
        .selection_geometry(TextRange::new(
            TextPosition::new(start, TextAffinity::Downstream),
            TextPosition::new(end, TextAffinity::Upstream),
        ))
        .map_err(|_| TextError::InvalidInteractionMap)
}
struct Drawing {
    path: Path,
    origin: Point,
    text: TextLayout,
    selection: Vec<SelectionQuad>,
    underline: Vec<SelectionQuad>,
    decoration: FontDecorationMetrics,
}
impl FigurePreparation for PathFigure {
    fn measure_scene(
        &self,
        _: &mut MeasureContext<'_>,
        _: novadraw::figure::MeasureConstraints,
        _: &novadraw::figure::PreparationScene<'_>,
    ) -> Result<novadraw::figure::PreparationMeasurement, novadraw::figure::PreparationMeasureError>
    {
        let (_, bounds) = path_geometry();
        Ok(novadraw::figure::PreparationMeasurement {
            preferred: FigureMeasurement::new(bounds.width, bounds.height, None),
            minimum: Dimension::new(bounds.width, bounds.height),
        })
    }

    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        let (path, bounds) = path_geometry();
        let text = context.layout_text("zoom", TextConstraints::UNBOUNDED)?;
        let origin = rotated(Point::new(bounds.x, bounds.y - TEXT_OFFSET));
        let drawing = Drawing {
            selection: quads(&text, 0, 2)?,
            underline: quads(&text, 1, 3)?,
            decoration: context.font_metrics()?.underline()?,
            path,
            origin,
            text,
        };
        // Rotate the text envelope through the same graphics transform.
        let mut envelope = Rectangle::new(
            origin.x(),
            origin.y(),
            f64::from(drawing.text.width()),
            f64::from(drawing.text.height().max(
                drawing.text.baseline()
                    + drawing.decoration.offset()
                    + drawing.decoration.thickness(),
            )),
        );
        envelope.transform(Affine2D::from_rotation_radians(ROTATION.to_radians()));
        let visual = bounds.union(envelope);
        FigurePresentation::new(
            FigureMeasurement::new(bounds.width, bounds.height, None),
            Dimension::new(bounds.width, bounds.height),
            visual,
            Arc::new(drawing),
        )
    }
}
impl Drawing {
    fn text(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.push_state();
        let result = (|| {
            context.rotate_degrees(ROTATION)?;
            context.set_fill_paint(Color::rgba(1.0, 1.0, 0.0, 1.0));
            for quad in &self.selection {
                let b = quad.bounds();
                context.fill_rect(Rectangle::new(
                    b.x + self.origin.x(),
                    b.y + self.origin.y(),
                    b.width,
                    b.height,
                ))?;
            }
            context.set_fill_paint(Color::BLACK);
            context.fill_text(&self.text, self.origin)?;
            context.push_state();
            let selected = (|| {
                for quad in &self.selection {
                    let b = quad.bounds();
                    context.clip_rect(Rectangle::new(
                        b.x + self.origin.x(),
                        b.y + self.origin.y(),
                        b.width,
                        b.height,
                    ))?;
                }
                context.set_fill_paint(Color::WHITE);
                context.fill_text(&self.text, self.origin)
            })();
            let restored = context.pop_state();
            selected.and(restored)?;
            context.set_fill_paint(Color::BLUE);
            for quad in &self.underline {
                let b = quad.bounds();
                context.fill_rect(Rectangle::new(
                    b.x + self.origin.x(),
                    self.origin.y() + f64::from(self.text.baseline() + self.decoration.offset()),
                    b.width,
                    f64::from(self.decoration.thickness()),
                ))?;
            }
            Ok(())
        })();
        let restored = context.pop_state();
        result.and(restored)
    }
}
impl FigureDrawing for Drawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::RED);
        context.fill_path(&self.path)?;
        self.text(context)?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(LINE)?;
        context.stroke_path(&self.path)?;
        // Upstream draws the TextLayout once in each shape pass.
        self.text(context)
    }
}
pub fn path_example() -> Runtime {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW,
            WINDOW,
            Color::WHITE,
        )))
        .expect("root");
    let zoom = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ScalableLayeredPaneFigure::new(
            0.0, BAR, 500.0, 800.0,
        )))
        .unwrap();
    runtime
        .figure(zoom)
        .unwrap()
        .set_preferred_size(Dimension::new(500.0, 800.0))
        .unwrap();
    runtime
        .figure(zoom)
        .unwrap()
        .set_layout_manager(Box::new(BorderLayout::new()))
        .unwrap();
    let path = runtime
        .figure(zoom)
        .unwrap()
        .add(Box::new(PathFigure))
        .unwrap();
    runtime
        .figure(path)
        .unwrap()
        .set_layout_constraint(BorderConstraint::new(BorderRegion::Center))
        .unwrap();
    let scrollbar =
        ScrollBarFigure::new(Arc::new(DefaultRangeModel::new(0.0, 25.0, 200.0).unwrap()))
            .with_config(ScrollBarConfig {
                orientation: ScrollOrientation::Horizontal,
                thickness: BAR,
                ..ScrollBarConfig::default()
            })
            .unwrap()
            .with_consumer(Arc::new(ZoomValue(zoom)))
            .with_part(
                ScrollBarPart::Thumb,
                Box::new(LabelFigure::new("<Zoom>").with_border(super::scrollbar_label_border())),
            )
            .into_subtree();
    let receipt = runtime
        .figure(root)
        .unwrap()
        .add_subtree(scrollbar.subtree)
        .unwrap();
    let bar = receipt.figure(scrollbar.root).unwrap();
    runtime
        .figure(root)
        .unwrap()
        .move_child_to_index(bar, 0)
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .unwrap();
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        event::MouseButton,
        render::{BackendCapabilities, SurfaceInfo, command::RenderCommandKind},
    };

    #[test]
    fn official_path_draws_two_text_passes_and_preserves_closed_geometry() {
        let mut runtime = path_example();
        runtime.frames().stabilize_for_query().unwrap();
        let root = runtime.tree().contents().unwrap();
        let children = runtime.tree().child_order(root).unwrap();
        let zoom = children[1];
        let path = runtime.tree().child_order(zoom).unwrap()[0];
        let expected = path_geometry().1;
        let bounds = runtime.tree().figure_bounds(path).unwrap();
        assert_eq!(bounds.x, 0.0);
        assert_eq!(bounds.y, 0.0);
        assert!(bounds.width > expected.width && bounds.height > expected.height);
        let submission = runtime
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: WINDOW,
                    logical_height: WINDOW,
                    pixel_width: 800,
                    pixel_height: 800,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL,
            )
            .into_ready()
            .unwrap();
        let glyphs = submission
            .commands
            .iter()
            .filter(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count();
        // Thumb + two original text passes and their white selected overlays.
        assert_eq!(glyphs, 5);
        let underline_rects: Vec<_> = submission
            .commands
            .iter()
            .filter_map(|command| {
                if let RenderCommandKind::FillRect {
                    rect,
                    paint: novadraw::graphics::Paint::Solid(color),
                    ..
                } = &command.kind
                    && *color == Color::BLUE
                {
                    Some(rect)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(underline_rects.len(), 2);
        assert!(
            underline_rects
                .iter()
                .all(|rect| rect.height > 0.0 && rect.height != 1.0)
        );
        assert!(submission.commands.iter().any(|c| matches!(&c.kind, RenderCommandKind::StrokePath { stroke, .. } if stroke.width()==LINE)));
        assert_eq!(path_geometry().0.operations().len(), 5);
    }

    #[test]
    fn scrollbar_uses_path_formula_and_keeps_center_in_logical_coordinates() {
        let mut runtime = path_example();
        runtime.frames().stabilize_for_query().unwrap();
        let root = runtime.tree().contents().unwrap();
        let children = runtime.tree().child_order(root).unwrap();
        let bar = children[0];
        let zoom = children[1];
        let path = runtime.tree().child_order(zoom).unwrap()[0];
        for _ in 0..5 {
            let width = runtime.tree().figure_bounds(bar).unwrap().width;
            runtime
                .input()
                .dispatch_mouse_pressed(width - 8.0, 12.0, MouseButton::Left);
            runtime
                .input()
                .dispatch_mouse_released(width - 8.0, 12.0, MouseButton::Left);
            runtime.frames().stabilize_for_query().unwrap();
        }
        let scale = runtime.viewports().scale_handle(zoom).unwrap().scale();
        assert_eq!(scale, f64::from(2.0_f32));
        let pane = runtime.tree().figure_bounds(zoom).unwrap();
        let child = runtime.tree().figure_bounds(path).unwrap();
        assert_eq!(child.x, 0.0);
        assert_eq!(child.y, 0.0);
        assert!((child.width * scale - pane.width).abs() < 1e-8);
        assert!((child.height * scale - pane.height).abs() < 1e-8);
        assert_eq!(runtime.tree().figure_bounds(bar).unwrap().height, BAR);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
}
