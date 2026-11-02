use super::*;
use crate::{
    EditPartId, InteractionRevision, ModelAdapter, ResizeAction, ResizeDirection, Tool,
    ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent, ViewerInputOutcome,
    ViewerTarget,
};
use novadraw::{
    event::{DispatchOutcome, MouseButton},
    geometry::Point,
    tree::FigureId,
};
use std::marker::PhantomData;

struct Gesture<I> {
    start: ResizeStart<I>,
    session: SnapSession,
    feedback: Vec<FigureId>,
    parts: [EditPartId; 1],
}

/// Open resize tool using ordinary typed policies, rules and Viewer-owned feedback renderers.
pub struct ResizeTool<
    A: ModelAdapter,
    P: ResizeProtocol<A>,
    R: EvaluationRule<Payload = ResizeLimitsPayload> = ResizeLimits,
> {
    settings: SnapSettings,
    limits: RuleBinding<R>,
    gesture: Option<Gesture<A::ModelId>>,
    pending: Option<(EditPartId, ResizeDirection, Point)>,
    protocol: PhantomData<fn() -> P>,
}
impl<A: ModelAdapter, P: ResizeProtocol<A>, R: EvaluationRule<Payload = ResizeLimitsPayload>>
    ResizeTool<A, P, R>
{
    /// Installs explicit budgets and a limits evaluator registered by this Viewer.
    pub fn new(settings: SnapSettings, limits: RuleBinding<R>) -> Self {
        Self {
            settings,
            limits,
            gesture: None,
            pending: None,
            protocol: PhantomData,
        }
    }
    /// Starts an explicit programmatic gesture on activation; handles use the same path.
    pub fn from_source(
        settings: SnapSettings,
        limits: RuleBinding<R>,
        source: EditPartId,
        direction: ResizeDirection,
        pointer: Point,
    ) -> Self {
        Self {
            pending: Some((source, direction, pointer)),
            ..Self::new(settings, limits)
        }
    }
}
impl<
    A: ModelAdapter + 'static,
    P: ResizeProtocol<A>,
    R: EvaluationRule<Payload = ResizeLimitsPayload>,
> ResizeTool<A, P, R>
{
    fn begin(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        source: EditPartId,
        direction: ResizeDirection,
        pointer: Point,
    ) -> Result<(), ToolError> {
        let start = ctx.begin_resize::<P>(source, direction, pointer)?;
        let session = ctx.begin_snap_session()?;
        self.gesture = Some(Gesture {
            start,
            session,
            feedback: vec![],
            parts: [source],
        });
        Ok(())
    }
    fn plan(
        ctx: &mut ToolContext<'_, A>,
        gesture: &mut Gesture<A::ModelId>,
        location: Point,
        revision: InteractionRevision,
        settings: SnapSettings,
        limits: &RuleBinding<R>,
    ) -> Result<Option<SnappedResizePlan<A, P>>, ToolError> {
        let candidate = ctx.resize_candidate(&gesture.start, location, revision)?;
        ctx.resolve_snapped_resize::<P>(&mut gesture.session, &candidate, settings, limits)
    }
    fn update(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        ctx.remove_overlay_visuals(&gesture.feedback)?;
        gesture.feedback.clear();
        if let Some(plan) = Self::plan(
            ctx,
            gesture,
            location,
            revision,
            self.settings,
            &self.limits,
        )? {
            gesture.feedback = ctx.show_snapped_resize_feedback(&mut gesture.session, &plan)?;
            ctx.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
        }
        Ok(())
    }
    fn refresh_checked(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        match self.update(ctx, location, revision) {
            Ok(()) => Ok(()),
            Err(ToolError::Snap(SnapError::Invalidated)) => self.cancel(ctx),
            Err(error) => {
                self.cancel(ctx)?;
                Err(error)
            }
        }
    }
}
impl<
    A: ModelAdapter + 'static,
    P: ResizeProtocol<A>,
    R: EvaluationRule<Payload = ResizeLimitsPayload>,
> Tool<A> for ResizeTool<A, P, R>
{
    fn key(&self) -> crate::ErasedToolKey {
        ToolKey::<Self>::new("resize").erase()
    }
    fn accepts_handle_activation(&self) -> bool {
        true
    }
    fn activate(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some((source, direction, pointer)) = self.pending.take() {
            self.begin(ctx, source, direction, pointer)?;
        }
        Ok(())
    }
    fn cancel(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            // Retire the receipt even if the overlay removal itself fails.
            gesture.session.cancel();
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        self.gesture = None;
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
        self.cancel(ctx)?;
        let outcome = ctx.dispatch_pressed(event, true)?;
        if event.button() == MouseButton::Left
            && !outcome.dispatch().is_handled()
            && let ViewerTarget::Handle { owner, action, .. } = outcome.target()
            && let Some(direction) = action.get::<ResizeAction>().copied()
        {
            self.begin(ctx, owner, direction, event.location())?;
        }
        Ok((ToolDirective::Continue, outcome))
    }
    fn mouse_moved(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_moved(event.location());
        self.refresh_checked(ctx, event.location(), event.revision())?;
        Ok((
            if self.gesture.is_some() {
                ToolDirective::Continue
            } else {
                ToolDirective::ReleaseCapture
            },
            dispatch,
        ))
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
        let Some(mut gesture) = self.gesture.take() else {
            return Ok((ToolDirective::ReleaseCapture, dispatch));
        };
        let result = Self::plan(
            ctx,
            &mut gesture,
            event.location(),
            event.revision(),
            self.settings,
            &self.limits,
        );
        if let Err(error) = ctx.remove_overlay_visuals(&gesture.feedback) {
            gesture.session.cancel();
            self.gesture = Some(gesture);
            return Err(error);
        }
        match result {
            Ok(Some(plan)) => ctx.submit_snapped_resize(&mut gesture.session, plan)?,
            Ok(None) | Err(ToolError::Snap(SnapError::Invalidated)) => gesture.session.cancel(),
            Err(error) => {
                gesture.session.cancel();
                return Err(error);
            }
        }
        Ok((ToolDirective::ReleaseCapture, dispatch))
    }
    fn has_active_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn supports_autoexpose(&self) -> bool {
        self.gesture.is_some()
    }
    fn dragged_parts(&self) -> Option<&[EditPartId]> {
        self.gesture.as_ref().map(|g| g.parts.as_slice())
    }
    fn refresh(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        self.refresh_checked(ctx, location, revision)
    }
    fn clear_transient_feedback(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}
