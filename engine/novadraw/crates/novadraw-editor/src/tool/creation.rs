use super::{ErasedToolKey, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent};
use crate::{
    EditPartId, EditingContribution, InteractionRevision, ModelAdapter, RequestMetadata,
    RequestModifiers, RequestProtocol, ViewerInputOutcome,
};
use novadraw::{
    event::{DispatchOutcome, MouseButton},
    geometry::{Dimension, Point, Rectangle},
    tree::FigureId,
};

/// Application-owned typed template. Tools and native drops use the same payload factory.
pub trait CreationTemplate<A: ModelAdapter>: 'static {
    /// Open request family resolved by the installed policy registry.
    type Request: RequestProtocol<A, Contribution = EditingContribution<A>>;

    /// Size used when creating by click or dropping a template.
    fn default_size(&self) -> Dimension;

    /// Builds a request and its target metadata for model-content bounds.
    /// The template may select an application-specific parent through its own typed payload.
    fn request(
        &self,
        contents: EditPartId,
        bounds: Rectangle,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> (
        RequestMetadata,
        <Self::Request as RequestProtocol<A>>::Payload,
    );
}

struct Gesture {
    start: Point,
    modifiers: RequestModifiers,
    feedback: Vec<FigureId>,
}

/// Click or drag creation through an application's typed template and policy registry.
pub struct CreationTool<T> {
    template: T,
    gesture: Option<Gesture>,
}

impl<T> CreationTool<T> {
    /// Creates an idle tool. Successful creation returns to the default selection tool.
    pub const fn new(template: T) -> Self {
        Self {
            template,
            gesture: None,
        }
    }
}

impl<A: ModelAdapter + 'static, T: CreationTemplate<A>> Tool<A> for CreationTool<T> {
    fn key(&self) -> ErasedToolKey {
        ToolKey::<Self>::new("creation").erase()
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
                start: ctx.model_point_from_surface(event.location())?,
                modifiers: RequestModifiers {
                    shift: event.modifiers().shift,
                    control: event.modifiers().control,
                    alt: event.modifiers().alt,
                    meta: event.modifiers().meta,
                },
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
        let bounds = creation_bounds(
            gesture.start,
            ctx.model_point_from_surface(event.location())?,
            self.template.default_size(),
        );
        let (metadata, payload) =
            self.template
                .request(ctx.contents(), bounds, gesture.modifiers, event.revision());
        if let Some(command) = ctx.command_for::<T::Request>(&metadata, &payload)? {
            ctx.submit_command(command)?;
        }
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
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
            let bounds = creation_bounds(
                gesture.start,
                ctx.model_point_from_surface(location)?,
                self.template.default_size(),
            );
            let (metadata, payload) =
                self.template
                    .request(ctx.contents(), bounds, gesture.modifiers, revision);
            gesture.feedback = ctx.show_feedback::<T::Request>(&metadata, &payload)?;
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

fn creation_bounds(start: Point, end: Point, default: Dimension) -> Rectangle {
    let width = (end.x() - start.x()).abs();
    let height = (end.y() - start.y()).abs();
    if width < super::DRAG_START_DISTANCE && height < super::DRAG_START_DISTANCE {
        Rectangle::new(start.x(), start.y(), default.width, default.height)
    } else {
        Rectangle::new(
            start.x().min(end.x()),
            start.y().min(end.y()),
            width,
            height,
        )
    }
}
