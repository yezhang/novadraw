use novadraw::{
    figure::flow::{FlowBorderFragment, FlowBorderMetrics, FlowError, FlowFragmentBorder},
    geometry::{Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext, Path},
    text::{MeasureContext, TextConstraints, TextLayout, TextStyle},
};
use std::sync::Arc;
pub struct TestBorder;
struct PreparedBorder {
    begin: TextLayout,
    end: TextLayout,
}
impl FlowFragmentBorder for TestBorder {
    fn prepare_renderer(
        &self,
        context: &mut MeasureContext<'_>,
        style: &TextStyle,
    ) -> Result<Option<Arc<dyn FlowFragmentBorder>>, FlowError> {
        context.set_font(style.font().clone());
        Ok(Some(Arc::new(PreparedBorder {
            begin: context.layout_text("begin", TextConstraints::UNBOUNDED)?,
            end: context.layout_text("end", TextConstraints::UNBOUNDED)?,
        })))
    }
    fn metrics(
        &self,
        _: &mut MeasureContext<'_>,
        _: &TextStyle,
    ) -> Result<FlowBorderMetrics, FlowError> {
        Err(FlowError::InvalidStructure)
    }
    fn paint_fragment(
        &self,
        _: &FlowBorderFragment,
        _: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        Ok(())
    }
}
impl FlowFragmentBorder for PreparedBorder {
    fn metrics(
        &self,
        _: &mut MeasureContext<'_>,
        _: &TextStyle,
    ) -> Result<FlowBorderMetrics, FlowError> {
        Ok(FlowBorderMetrics {
            leading_width: f64::from(self.begin.width() + 3.0 + self.begin.height() / 2.0),
            trailing_width: f64::from(self.end.width() + 3.0 + self.end.height() / 2.0),
            height: f64::from(self.begin.height().max(self.end.height())),
            ..FlowBorderMetrics::default()
        })
    }
    fn paint_fragment(
        &self,
        fragment: &FlowBorderFragment,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        for (bounds, label, leading) in [
            (
                fragment.leading_box.filter(|_| fragment.leading),
                &self.begin,
                true,
            ),
            (
                fragment.trailing_box.filter(|_| fragment.trailing),
                &self.end,
                false,
            ),
        ] {
            let Some(Rectangle {
                x,
                y,
                width,
                height,
            }) = bounds
            else {
                continue;
            };
            let mut path = Path::new();
            if leading {
                path.move_to(x, y);
                path.line_to(x + width - height / 2.0, y);
                path.line_to(x + width, y + height / 2.0);
                path.line_to(x + width - height / 2.0, y + height);
                path.line_to(x, y + height);
            } else {
                path.move_to(x, y + height / 2.0);
                path.line_to(x + height / 2.0, y);
                path.line_to(x + width, y);
                path.line_to(x + width, y + height);
                path.line_to(x + height / 2.0, y + height);
            }
            path.close();
            context.set_fill_paint(Color::rgba(1.0, 1.0, 0.88, 1.0));
            context.fill_path(&path)?;
            context.set_stroke_paint(Color::BLACK);
            context.set_stroke_width(1.0)?;
            context.stroke_path(&path)?;
            context.set_fill_paint(Color::BLACK);
            context.fill_text(
                label,
                Point::new(x + if leading { 1.0 } else { height / 2.0 }, y),
            )?;
        }
        Ok(())
    }
}
