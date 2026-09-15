//! Active editing tools and gesture-scoped trackers.

use std::{error::Error, fmt};

use novadraw_geometry::{Dimension, Point, Vec2};
use novadraw_scene::{DispatchOutcome, FigureId, KeyModifiers, MouseButton};

use crate::{
    ChangeBoundsRequest, Command, ConnectionCreation, ConnectionEndpoint, ConnectionPartId,
    ConnectionReconnection, CreateConnectionRequest, CreationType, EditPartFactory, EditPartId,
    EditorRequest, GraphicalViewer, HandleRole, InteractionRevision, ModelAdapter,
    ReconnectConnectionRequest, RequestModifiers, ResizeDirection, ViewerError, ViewerInputOutcome,
    ViewerTarget,
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

/// Result of a press handled by the connection-creation Tool.
pub struct ConnectionToolPress<A: ModelAdapter> {
    outcome: ViewerInputOutcome,
    command: Option<Box<dyn Command<A>>>,
    completed: bool,
}

/// Completion returned when an endpoint reconnect drag releases.
pub struct ConnectionEndpointRelease<A: ModelAdapter> {
    dispatch: DispatchOutcome,
    command: Option<Box<dyn Command<A>>>,
}

impl<A: ModelAdapter> ConnectionEndpointRelease<A> {
    pub(crate) fn into_parts(self) -> (DispatchOutcome, Option<Box<dyn Command<A>>>) {
        (self.dispatch, self.command)
    }
}

impl<A: ModelAdapter> ConnectionToolPress<A> {
    /// Returns the underlying Viewer input result.
    pub const fn outcome(&self) -> &ViewerInputOutcome {
        &self.outcome
    }

    /// Returns whether the second stage completed successfully.
    pub const fn completed(&self) -> bool {
        self.completed
    }

    pub(crate) fn into_parts(self) -> (ViewerInputOutcome, Option<Box<dyn Command<A>>>) {
        (self.outcome, self.command)
    }
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
            | ViewerTarget::Handle {
                role: HandleRole::ConnectionEndpoint(_),
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

struct ConnectionGesture<A: ModelAdapter> {
    source: EditPartId,
    connection_type: CreationType,
    modifiers: RequestModifiers,
    plan: Box<dyn ConnectionCreation<A>>,
    feedback: Vec<FigureId>,
}

/// One-shot, two-stage connection-creation Tool.
pub struct ConnectionCreationTool<A: ModelAdapter> {
    connection_type: CreationType,
    gesture: Option<ConnectionGesture<A>>,
}

struct ReconnectGesture<A: ModelAdapter> {
    start: Point,
    modifiers: RequestModifiers,
    plan: Box<dyn ConnectionReconnection<A>>,
    feedback: Vec<FigureId>,
}

/// Drag Tool for moving one endpoint of an existing connection.
pub struct ConnectionEndpointTool<A: ModelAdapter> {
    connection: ConnectionPartId,
    endpoint: ConnectionEndpoint,
    gesture: Option<ReconnectGesture<A>>,
}

impl<A: ModelAdapter> ConnectionEndpointTool<A> {
    /// Creates a Tool locked to one connection endpoint.
    pub const fn new(connection: ConnectionPartId, endpoint: ConnectionEndpoint) -> Self {
        Self {
            connection,
            endpoint,
            gesture: None,
        }
    }

    /// Returns whether the endpoint drag was accepted by a policy.
    pub const fn is_active(&self) -> bool {
        self.gesture.is_some()
    }

    /// Starts reconnect tracking from an endpoint handle press.
    pub fn pointer_pressed<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
        revision: InteractionRevision,
    ) -> Result<ViewerInputOutcome, ToolError>
    where
        A: 'static,
        F: EditPartFactory<A>,
    {
        let outcome =
            viewer.dispatch_mouse_pressed_without_selection(location.x(), location.y(), button);
        if outcome.dispatch().is_handled() || button != MouseButton::Left {
            return Ok(outcome);
        }
        let request = ReconnectConnectionRequest::new(
            self.connection,
            self.endpoint,
            location,
            request_modifiers(modifiers),
            revision,
        );
        if let Some(plan) = viewer.start_connection_reconnection(&request)? {
            self.gesture = Some(ReconnectGesture {
                start: location,
                modifiers: request_modifiers(modifiers),
                plan,
                feedback: Vec::new(),
            });
        }
        Ok(outcome)
    }

    /// Updates the endpoint candidate and transient feedback.
    pub fn pointer_moved<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<DispatchOutcome, ToolError>
    where
        A: 'static,
        F: EditPartFactory<A>,
    {
        let dispatch = viewer.dispatch_mouse_moved(location.x(), location.y());
        let Some(gesture) = &mut self.gesture else {
            return Ok(dispatch);
        };
        if (location - gesture.start).length() < DRAG_START_DISTANCE {
            return Ok(dispatch);
        }
        let candidate = match viewer.target_at(location.x(), location.y()) {
            ViewerTarget::Part(part) => Some(part),
            ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
        };
        let request = ReconnectConnectionRequest::new(
            self.connection,
            self.endpoint,
            location,
            gesture.modifiers,
            revision,
        )
        .with_target_candidate(candidate);
        clear_feedback(viewer, &mut gesture.feedback)?;
        gesture.feedback = viewer.show_reconnection_feedback(gesture.plan.as_mut(), &request)?;
        Ok(dispatch)
    }

    /// Clears feedback and returns a reconnect Command for a valid drop.
    pub fn pointer_released<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        revision: InteractionRevision,
    ) -> Result<ConnectionEndpointRelease<A>, ToolError>
    where
        A: 'static,
        F: EditPartFactory<A>,
    {
        let dispatch = viewer.dispatch_mouse_released(location.x(), location.y(), button);
        let Some(mut gesture) = self.gesture.take() else {
            return Ok(ConnectionEndpointRelease {
                dispatch,
                command: None,
            });
        };
        clear_feedback(viewer, &mut gesture.feedback)?;
        if button != MouseButton::Left || (location - gesture.start).length() < DRAG_START_DISTANCE
        {
            return Ok(ConnectionEndpointRelease {
                dispatch,
                command: None,
            });
        }
        let candidate = match viewer.target_at(location.x(), location.y()) {
            ViewerTarget::Part(part) => Some(part),
            ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
        };
        let request = ReconnectConnectionRequest::new(
            self.connection,
            self.endpoint,
            location,
            gesture.modifiers,
            revision,
        )
        .with_target_candidate(candidate);
        let command = viewer.reconnection_command(gesture.plan.as_mut(), &request)?;
        Ok(ConnectionEndpointRelease { dispatch, command })
    }

    /// Cancels reconnect tracking and clears transient feedback.
    pub fn cancel<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), ToolError>
    where
        F: EditPartFactory<A>,
    {
        if let Some(mut gesture) = self.gesture.take() {
            clear_feedback(viewer, &mut gesture.feedback)?;
        }
        Ok(())
    }
}

impl<A: ModelAdapter> ConnectionCreationTool<A> {
    /// Arms a Tool for one connection of the supplied application type.
    pub fn new(connection_type: CreationType) -> Self {
        Self {
            connection_type,
            gesture: None,
        }
    }

    /// Returns whether the source stage has been accepted.
    pub const fn is_started(&self) -> bool {
        self.gesture.is_some()
    }

    /// Locks a source on the first press or produces a Command on a valid second press.
    pub fn pointer_pressed<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
        revision: InteractionRevision,
    ) -> Result<ConnectionToolPress<A>, ToolError>
    where
        A: 'static,
        F: EditPartFactory<A>,
    {
        let outcome =
            viewer.dispatch_mouse_pressed_without_selection(location.x(), location.y(), button);
        if outcome.dispatch().is_handled() || button != MouseButton::Left {
            return Ok(ConnectionToolPress {
                outcome,
                command: None,
                completed: false,
            });
        }
        let ViewerTarget::Part(target) = outcome.target() else {
            return Ok(ConnectionToolPress {
                outcome,
                command: None,
                completed: false,
            });
        };

        if let Some(gesture) = &mut self.gesture {
            let request = CreateConnectionRequest::new(
                gesture.connection_type.clone(),
                gesture.source,
                location,
                gesture.modifiers,
                revision,
            )
            .with_target_candidate(Some(target));
            clear_feedback(viewer, &mut gesture.feedback)?;
            let command = viewer.connection_command(gesture.plan.as_mut(), &request)?;
            if command.is_some() {
                self.gesture = None;
                return Ok(ConnectionToolPress {
                    outcome,
                    command,
                    completed: true,
                });
            }
            gesture.feedback = viewer.show_connection_feedback(gesture.plan.as_mut(), &request)?;
            return Ok(ConnectionToolPress {
                outcome,
                command: None,
                completed: false,
            });
        }

        let request = CreateConnectionRequest::new(
            self.connection_type.clone(),
            target,
            location,
            request_modifiers(modifiers),
            revision,
        );
        let Some(mut plan) = viewer.start_connection_creation(&request)? else {
            return Ok(ConnectionToolPress {
                outcome,
                command: None,
                completed: false,
            });
        };
        let feedback = viewer.show_connection_feedback(plan.as_mut(), &request)?;
        self.gesture = Some(ConnectionGesture {
            source: target,
            connection_type: self.connection_type.clone(),
            modifiers: request_modifiers(modifiers),
            plan,
            feedback,
        });
        Ok(ConnectionToolPress {
            outcome,
            command: None,
            completed: false,
        })
    }

    /// Updates the target candidate and replaces transient feedback.
    pub fn pointer_moved<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<DispatchOutcome, ToolError>
    where
        A: 'static,
        F: EditPartFactory<A>,
    {
        let dispatch = viewer.dispatch_mouse_moved(location.x(), location.y());
        let Some(gesture) = &mut self.gesture else {
            return Ok(dispatch);
        };
        let target = match viewer.target_at(location.x(), location.y()) {
            ViewerTarget::Part(part) => Some(part),
            ViewerTarget::Handle { .. } | ViewerTarget::Contents(_) => None,
        };
        let request = CreateConnectionRequest::new(
            gesture.connection_type.clone(),
            gesture.source,
            location,
            gesture.modifiers,
            revision,
        )
        .with_target_candidate(target);
        clear_feedback(viewer, &mut gesture.feedback)?;
        gesture.feedback = viewer.show_connection_feedback(gesture.plan.as_mut(), &request)?;
        Ok(dispatch)
    }

    /// Dispatches release without completing the two-press gesture.
    pub fn pointer_released<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
    ) -> DispatchOutcome
    where
        F: EditPartFactory<A>,
    {
        viewer.dispatch_mouse_released(location.x(), location.y(), button)
    }

    /// Cancels the source-locked gesture and removes transient feedback.
    pub fn cancel<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), ToolError>
    where
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
