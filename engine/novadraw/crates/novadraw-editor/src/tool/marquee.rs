use super::{ErasedToolKey, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent};
use crate::{FeedbackVisual, ModelAdapter, ViewerInputOutcome};
use novadraw::{
    event::{DispatchOutcome, KeyModifiers, MouseButton},
    figure::RectangleFigure,
    geometry::{Point, Rectangle},
    graphics::Color,
    tree::FigureId,
};

const MARQUEE_COLOR: Color = Color::rgba(0.2, 0.5, 0.8, 1.0);
const MARQUEE_WIDTH: f64 = 1.0;

struct Gesture {
    start_model: Point,
    modifiers: KeyModifiers,
    feedback: Vec<FigureId>,
}

/// Enclosed-node marquee with replace, Shift append, and Control/Meta toggle modes.
/// Selection is committed only on release; cancellation leaves the original selection intact.
#[derive(Default)]
pub struct MarqueeTool {
    gesture: Option<Gesture>,
}

impl MarqueeTool {
    /// Creates an idle marquee tool.
    pub const fn new() -> Self {
        Self { gesture: None }
    }
}

impl<A: ModelAdapter + 'static> Tool<A> for MarqueeTool {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("marquee").erase()
    }
    fn cancel(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = self.gesture.take() {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
        }
        Ok(())
    }
    fn deactivate(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        self.cancel(ctx)
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let outcome = ctx.dispatch_pressed(event, false)?;
        if outcome.dispatch().is_handled() {
            return Ok((ToolDirective::Continue, outcome));
        }
        if event.button() == MouseButton::Left {
            self.cancel(ctx)?;
            self.gesture = Some(Gesture {
                start_model: ctx.model_point_from_surface(event.location())?,
                modifiers: event.modifiers(),
                feedback: Vec::new(),
            });
        }
        Ok((ToolDirective::Continue, outcome))
    }
    fn mouse_moved(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_moved(event.location());
        self.refresh(ctx, event.location(), event.revision())?;
        Ok((ToolDirective::Continue, dispatch))
    }
    fn mouse_released(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_released(event.location(), event.button());
        if event.button() != MouseButton::Left {
            return Ok((ToolDirective::Continue, dispatch));
        }
        let Some(gesture) = self.gesture.take() else {
            return Ok((ToolDirective::Continue, dispatch));
        };
        ctx.remove_overlay_visuals(&gesture.feedback)?;
        let candidates = ctx.parts_enclosed_by(bounds(
            ctx.surface_point_from_model(gesture.start_model)?,
            event.location(),
        ));
        let mut selected =
            if gesture.modifiers.shift || gesture.modifiers.control || gesture.modifiers.meta {
                ctx.selection().to_vec()
            } else {
                Vec::new()
            };
        for part in candidates {
            if let Some(index) = selected.iter().position(|p| *p == part) {
                if gesture.modifiers.control || gesture.modifiers.meta {
                    selected.remove(index);
                }
            } else {
                selected.push(part);
            }
        }
        ctx.set_selection(&selected)?;
        Ok((ToolDirective::Deactivate, dispatch))
    }
    fn has_active_gesture(&self) -> bool {
        self.gesture.is_some()
    }

    fn supports_autoexpose(&self) -> bool {
        self.gesture.is_some()
    }

    fn refresh(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        _: crate::InteractionRevision,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
            let b = bounds(gesture.start_model, ctx.model_point_from_surface(location)?);
            let figure = RectangleFigure::from_bounds(b).with_stroke(MARQUEE_COLOR, MARQUEE_WIDTH);
            gesture.feedback = ctx.attach_feedback(
                ctx.contents(),
                vec![FeedbackVisual::scaled(Box::new(figure))],
            )?;
            ctx.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
        }
        Ok(())
    }

    fn clear_transient_feedback(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}

fn bounds(a: Point, b: Point) -> Rectangle {
    Rectangle::new(
        a.x().min(b.x()),
        a.y().min(b.y()),
        (a.x() - b.x()).abs(),
        (a.y() - b.y()).abs(),
    )
}
