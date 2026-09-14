//! Active editing tools and gesture-scoped trackers.

use std::{error::Error, fmt};

use novadraw_geometry::{Dimension, Point, Vec2};
use novadraw_scene::{DispatchOutcome, FigureId, KeyModifiers, MouseButton};

use crate::{
    ChangeBoundsRequest, EditPartFactory, EditPartId, EditorRequest, GraphicalViewer, HandleRole,
    InteractionRevision, ModelAdapter, RequestModifiers, ResizeDirection, ViewerError,
    ViewerInputOutcome, ViewerTarget,
};

const DRAG_START_DISTANCE: f64 = 2.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DragKind {
    Move,
    Resize(ResizeDirection),
}

struct DragGesture {
    start: Point,
    parts: Vec<EditPartId>,
    kind: DragKind,
    modifiers: RequestModifiers,
    collapse_on_click: Option<EditPartId>,
    feedback: Vec<FigureId>,
}

/// Completion returned when the SelectionTool releases a pointer gesture.
pub struct ToolRelease {
    dispatch: DispatchOutcome,
    request: Option<EditorRequest>,
}

impl ToolRelease {
    /// Returns the underlying Figure dispatch result.
    pub const fn dispatch(&self) -> DispatchOutcome {
        self.dispatch
    }

    /// Returns the committed request after feedback has been erased.
    pub fn into_request(self) -> Option<EditorRequest> {
        self.request
    }
}

/// Failure while updating or cancelling a Tool gesture.
#[derive(Debug)]
pub enum ToolError {
    /// Viewer targeting, policy feedback, or overlay cleanup failed.
    Viewer(ViewerError),
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Viewer(error) => error.fmt(formatter),
        }
    }
}

impl Error for ToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Viewer(error) => Some(error),
        }
    }
}

impl From<ViewerError> for ToolError {
    fn from(value: ViewerError) -> Self {
        Self::Viewer(value)
    }
}

/// Default selection and bounds-manipulation Tool.
#[derive(Default)]
pub struct SelectionTool {
    gesture: Option<DragGesture>,
}

impl SelectionTool {
    /// Creates an idle SelectionTool.
    pub const fn new() -> Self {
        Self { gesture: None }
    }

    /// Returns whether a Tool-owned pointer gesture is active.
    pub const fn is_active(&self) -> bool {
        self.gesture.is_some()
    }

    /// Dispatches press to Figures first, then locks a stable drag source when eligible.
    pub fn pointer_pressed<A, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
    ) -> Result<ViewerInputOutcome, ToolError>
    where
        A: ModelAdapter,
        F: EditPartFactory<A>,
    {
        self.cancel(viewer)?;
        let outcome =
            viewer.dispatch_mouse_pressed(location.x(), location.y(), button, modifiers)?;
        if outcome.dispatch().is_handled() || button != MouseButton::Left {
            return Ok(outcome);
        }

        let (parts, kind, collapse_on_click) = match outcome.target() {
            ViewerTarget::Part(part) if viewer.selection().items().contains(&part) => {
                let collapse = (!modifiers.shift
                    && !modifiers.control
                    && !modifiers.meta
                    && viewer.selection().items().len() > 1)
                    .then_some(part);
                (
                    viewer.selection().items().to_vec(),
                    DragKind::Move,
                    collapse,
                )
            }
            ViewerTarget::Handle {
                owner,
                role: HandleRole::Resize(direction),
                ..
            } => (vec![owner], DragKind::Resize(direction), None),
            ViewerTarget::Handle {
                role: HandleRole::Selection,
                ..
            }
            | ViewerTarget::Contents(_)
            | ViewerTarget::Part(_) => return Ok(outcome),
        };

        self.gesture = Some(DragGesture {
            start: location,
            parts,
            kind,
            modifiers: request_modifiers(modifiers),
            collapse_on_click,
            feedback: Vec::new(),
        });
        Ok(outcome)
    }

    /// Updates Figure dispatch and Policy feedback without mutating the model.
    pub fn pointer_moved<A, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<DispatchOutcome, ToolError>
    where
        A: ModelAdapter,
        F: EditPartFactory<A>,
    {
        let dispatch = viewer.dispatch_mouse_moved(location.x(), location.y());
        let Some(gesture) = &mut self.gesture else {
            return Ok(dispatch);
        };
        let delta = location - gesture.start;
        if delta.length() < DRAG_START_DISTANCE {
            return Ok(dispatch);
        }

        clear_feedback(viewer, &mut gesture.feedback)?;
        let request = change_bounds_request(gesture, location, delta, revision);
        gesture.feedback = viewer.show_feedback_for_request(&request)?;
        Ok(dispatch)
    }

    /// Erases feedback and returns the final Request for command execution.
    pub fn pointer_released<A, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        revision: InteractionRevision,
    ) -> Result<ToolRelease, ToolError>
    where
        A: ModelAdapter,
        F: EditPartFactory<A>,
    {
        let dispatch = viewer.dispatch_mouse_released(location.x(), location.y(), button);
        let Some(mut gesture) = self.gesture.take() else {
            return Ok(ToolRelease {
                dispatch,
                request: None,
            });
        };
        clear_feedback(viewer, &mut gesture.feedback)?;
        let delta = location - gesture.start;
        if delta.length() < DRAG_START_DISTANCE
            && let Some(part) = gesture.collapse_on_click
        {
            viewer.replace_selection(part)?;
            viewer.set_focus(Some(part))?;
        }
        let request = (button == MouseButton::Left && delta.length() >= DRAG_START_DISTANCE)
            .then(|| change_bounds_request(&gesture, location, delta, revision));
        Ok(ToolRelease { dispatch, request })
    }

    /// Cancels the active gesture and erases all transient feedback.
    pub fn cancel<A, F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), ToolError>
    where
        A: ModelAdapter,
        F: EditPartFactory<A>,
    {
        if let Some(mut gesture) = self.gesture.take() {
            clear_feedback(viewer, &mut gesture.feedback)?;
        }
        Ok(())
    }
}

fn request_modifiers(modifiers: KeyModifiers) -> RequestModifiers {
    RequestModifiers {
        shift: modifiers.shift,
        control: modifiers.control,
        alt: modifiers.alt,
        meta: modifiers.meta,
    }
}

fn change_bounds_request(
    gesture: &DragGesture,
    location: Point,
    delta: Vec2,
    revision: InteractionRevision,
) -> EditorRequest {
    EditorRequest::ChangeBounds(match gesture.kind {
        DragKind::Move => ChangeBoundsRequest::moving(
            gesture.parts.clone(),
            location,
            delta,
            gesture.modifiers,
            revision,
        ),
        DragKind::Resize(direction) => {
            let (move_delta, size_delta) = resize_deltas(direction, delta);
            ChangeBoundsRequest::resizing(
                gesture.parts.clone(),
                location,
                move_delta,
                size_delta,
                direction,
                gesture.modifiers,
                revision,
            )
        }
    })
}

fn resize_deltas(direction: ResizeDirection, delta: Vec2) -> (Vec2, Dimension) {
    let dx = delta.x();
    let dy = delta.y();
    match direction {
        ResizeDirection::North => (Vec2::new(0.0, dy), Dimension::new(0.0, -dy)),
        ResizeDirection::NorthEast => (Vec2::new(0.0, dy), Dimension::new(dx, -dy)),
        ResizeDirection::East => (Vec2::ZERO, Dimension::new(dx, 0.0)),
        ResizeDirection::SouthEast => (Vec2::ZERO, Dimension::new(dx, dy)),
        ResizeDirection::South => (Vec2::ZERO, Dimension::new(0.0, dy)),
        ResizeDirection::SouthWest => (Vec2::new(dx, 0.0), Dimension::new(-dx, dy)),
        ResizeDirection::West => (Vec2::new(dx, 0.0), Dimension::new(-dx, 0.0)),
        ResizeDirection::NorthWest => (delta, Dimension::new(-dx, -dy)),
    }
}

fn clear_feedback<A, F>(
    viewer: &mut GraphicalViewer<A, F>,
    feedback: &mut Vec<FigureId>,
) -> Result<(), ViewerError>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    for figure in feedback.drain(..) {
        viewer.remove_overlay_visual(figure)?;
    }
    Ok(())
}
