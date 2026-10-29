use super::*;
use crate::{
    EditPartId, InteractionRevision, ModelAdapter, Tool, ToolContext, ToolDirective, ToolError,
    ToolKey, ToolMouseEvent, ViewerInputOutcome, ViewerTarget,
};
use novadraw::{
    event::{DispatchOutcome, MouseButton},
    geometry::Point,
    tree::FigureId,
};
use std::marker::PhantomData;

struct Gesture<I> {
    batch: MoveBatch<I>,
    parts: Vec<EditPartId>,
    start: Point,
    dragging: bool,
    feedback: Vec<FigureId>,
    visited: Vec<(EditPartId, PolicyGeneration)>,
}

/// Optional containment move tool, shared by every application protocol and layout.
pub struct ContainerMoveTool<A: ModelAdapter, P: ContainerMoveProtocol<A>> {
    gesture: Option<Gesture<A::ModelId>>,
    pending: Option<(
        Vec<EditPartId>,
        Option<EditPartId>,
        Point,
        InteractionRevision,
    )>,
    protocol: PhantomData<fn() -> P>,
}

impl<A: ModelAdapter, P: ContainerMoveProtocol<A>> Default for ContainerMoveTool<A, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: ModelAdapter, P: ContainerMoveProtocol<A>> ContainerMoveTool<A, P> {
    /// Creates an idle selection-and-move tool with a fixed typed protocol.
    pub const fn new() -> Self {
        Self {
            gesture: None,
            pending: None,
            protocol: PhantomData,
        }
    }

    /// Starts from explicit programmatic input once installed through `set_active_tool`.
    /// A missing anchor selects the first ordered operation source; the start point is required.
    pub fn from_sources(
        sources: Vec<EditPartId>,
        anchor: Option<EditPartId>,
        start: Point,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            pending: Some((sources, anchor, start, revision)),
            ..Self::new()
        }
    }
}

impl<A: ModelAdapter + 'static, P: ContainerMoveProtocol<A>> ContainerMoveTool<A, P> {
    fn begin(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        input: &[EditPartId],
        anchor: Option<EditPartId>,
        start: Point,
        revision: InteractionRevision,
        dragging: bool,
    ) -> Result<(), ToolError> {
        let batch = ctx.begin_container_move::<P>(input, anchor, start, revision)?;
        let parts = batch.sources().iter().map(|source| source.part()).collect();
        self.gesture = Some(Gesture {
            batch,
            parts,
            start,
            dragging,
            feedback: Vec::new(),
            visited: Vec::new(),
        });
        Ok(())
    }

    fn plan(
        ctx: &mut ToolContext<'_, A>,
        gesture: &mut Gesture<A::ModelId>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<Option<CheckedMovePlan<A, P>>, ToolError> {
        ctx.validate_container_move(&gesture.batch)?;
        if gesture
            .visited
            .iter()
            .any(|(host, generation)| ctx.policy_generation(*host) != *generation)
        {
            return Err(ContainerEditError::Invalidated.into());
        }
        let mut target = ctx.container_target_at(location, &gesture.parts)?;
        for _ in 0..MAX_CONTAINMENT_DEPTH {
            let generation = ctx.policy_generation(target);
            if !gesture.visited.iter().any(|(host, _)| *host == target) {
                gesture.visited.push((target, generation));
            }
            let candidate =
                ctx.container_move_candidate(&gesture.batch, target, location, revision)?;
            if let Some(plan) = ctx.resolve_container_move::<P>(&candidate)? {
                return Ok(Some(plan));
            }
            let Some(parent) = ctx.part_tree().parent(target) else {
                return Ok(None);
            };
            if parent == ctx.part_tree().root() {
                return Ok(None);
            }
            target = parent;
        }
        Err(ContainerEditError::Batch("container candidate depth exceeds 10000").into())
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
        ctx.validate_container_move(&gesture.batch)?;
        let delta = Point::new(
            location.x() - gesture.start.x(),
            location.y() - gesture.start.y(),
        );
        super::geometry::finite_point(delta)?;
        gesture.dragging |= delta.x().abs() >= crate::tool::DRAG_START_DISTANCE
            || delta.y().abs() >= crate::tool::DRAG_START_DISTANCE;
        if !gesture.dragging {
            return Ok(());
        }
        if let Some(checked) = Self::plan(ctx, gesture, location, revision)? {
            gesture.feedback = ctx.show_container_move_feedback(&checked)?;
            ctx.add_autoexpose_range_reserve(location, &mut gesture.feedback)?;
        }
        Ok(())
    }
}

impl<A: ModelAdapter + 'static, P: ContainerMoveProtocol<A>> Tool<A> for ContainerMoveTool<A, P> {
    fn key(&self) -> crate::ErasedToolKey {
        ToolKey::<Self>::new("container-move").erase()
    }

    fn accepts_handle_activation(&self) -> bool {
        true
    }

    fn activate(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some((input, anchor, start, revision)) = self.pending.take() {
            self.begin(ctx, &input, anchor, start, revision, true)?;
        }
        Ok(())
    }

    fn cancel(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
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
        ctx.synchronize_model()?;
        let target = ctx.target_at(event.location());
        let pressed = match target {
            ViewerTarget::Part(part) => Some(part),
            _ => None,
        };
        // Preserve a mixed selection when pressing one of its items.
        let mut selected_or_covered = false;
        let mut current = pressed;
        for _ in 0..MAX_CONTAINMENT_DEPTH {
            let Some(part) = current else {
                break;
            };
            if ctx.selection().contains(&part) {
                selected_or_covered = true;
                break;
            }
            current = ctx.part_tree().parent(part);
        }
        let update_selection = !selected_or_covered
            || event.modifiers().control
            || event.modifiers().shift
            || event.modifiers().meta;
        let outcome = ctx.dispatch_pressed(event, update_selection)?;
        if !outcome.dispatch().is_handled()
            && event.button() == MouseButton::Left
            && let Some(pressed) = pressed
            && ctx.as_connection_part(pressed).is_none()
        {
            let input = ctx.selection().to_vec();
            self.begin(
                ctx,
                &input,
                Some(pressed),
                event.location(),
                event.revision(),
                false,
            )?;
        }
        Ok((ToolDirective::Continue, outcome))
    }

    fn mouse_moved(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        let dispatch = ctx.dispatch_moved(event.location());
        match self.update(ctx, event.location(), event.revision()) {
            Ok(()) => Ok((ToolDirective::Continue, dispatch)),
            Err(ToolError::Container(ContainerEditError::Invalidated)) => {
                self.cancel(ctx)?;
                Ok((ToolDirective::ReleaseCapture, dispatch))
            }
            Err(error) => {
                self.cancel(ctx)?;
                Err(error)
            }
        }
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
        let result = (|| {
            let delta = super::geometry::finite_point(Point::new(
                event.location().x() - gesture.start.x(),
                event.location().y() - gesture.start.y(),
            ))?;
            gesture.dragging |= delta.x().abs() >= crate::tool::DRAG_START_DISTANCE
                || delta.y().abs() >= crate::tool::DRAG_START_DISTANCE;
            if gesture.dragging {
                Self::plan(ctx, &mut gesture, event.location(), event.revision())
            } else {
                ctx.validate_container_move(&gesture.batch).map(|()| None)
            }
        })();
        // Cleanup failure prevents both command construction and submission.
        if let Err(error) = ctx.remove_overlay_visuals(&gesture.feedback) {
            self.gesture = Some(gesture);
            return Err(error);
        }
        match result {
            Ok(Some(checked)) => {
                ctx.submit_container_move(checked)?;
            }
            Ok(None) | Err(ToolError::Container(ContainerEditError::Invalidated)) => {}
            Err(error) => return Err(error),
        }
        Ok((ToolDirective::ReleaseCapture, dispatch))
    }

    fn has_active_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn supports_autoexpose(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.dragging)
    }
    fn dragged_parts(&self) -> Option<&[EditPartId]> {
        self.gesture
            .as_ref()
            .filter(|gesture| gesture.dragging)
            .map(|gesture| gesture.parts.as_slice())
    }
    fn refresh(
        &mut self,
        ctx: &mut ToolContext<'_, A>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), ToolError> {
        match self.update(ctx, location, revision) {
            Ok(()) => Ok(()),
            Err(ToolError::Container(ContainerEditError::Invalidated)) => self.cancel(ctx),
            Err(error) => {
                self.cancel(ctx)?;
                Err(error)
            }
        }
    }
    fn clear_transient_feedback(&mut self, ctx: &mut ToolContext<'_, A>) -> Result<(), ToolError> {
        if let Some(gesture) = &mut self.gesture {
            ctx.remove_overlay_visuals(&gesture.feedback)?;
            gesture.feedback.clear();
        }
        Ok(())
    }
}
