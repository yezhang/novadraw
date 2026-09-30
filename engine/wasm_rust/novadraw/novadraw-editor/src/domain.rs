//! Editor session coordination.

use std::{error::Error, fmt, time::Duration};

use novadraw::geometry::Point;
use novadraw::{
    DispatchOutcome, FlowTextPosition, FlowTextRange, KeyModifiers, MouseButton, TextMovement,
};

use crate::{
    AutoexposeTick, BendpointOperation, CommandStack, CommandStackError, ConnectionBendpointTool,
    ConnectionCreationTool, ConnectionEndpointTool, CreationType, DirectTextEditRequest,
    DirectTextEditSessionId, DirectTextEditState, DirectTextFeature, EditPartFactory, EditPartId,
    EditorRequest, ExtendTextSelection, GraphicalViewer, HandleRole, InteractionRevision,
    InteractionRevisionError, ModelAdapter, SelectionTool, SessionTextInputEvent, TextDelete,
    TextInputEvent, ToolError, ViewerError, ViewerInputOutcome, ViewerTarget, autoexpose,
};

/// Failure while coordinating Tool, CommandStack, model, and Viewer.
#[derive(Debug)]
pub enum EditorDomainError {
    /// Tool targeting or feedback failed.
    Tool(ToolError),
    /// No policy contributed a command for the request.
    NoCommand,
    /// Command history rejected or failed an operation.
    Command(CommandStackError),
    /// Model projection failed after a command transition.
    Viewer(ViewerError),
    /// Interaction revision identity was exhausted.
    Revision(InteractionRevisionError),
}

impl fmt::Display for EditorDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tool(error) => error.fmt(formatter),
            Self::NoCommand => formatter.write_str("no EditPolicy contributed a command"),
            Self::Command(error) => error.fmt(formatter),
            Self::Viewer(error) => error.fmt(formatter),
            Self::Revision(error) => error.fmt(formatter),
        }
    }
}

impl Error for EditorDomainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Tool(error) => Some(error),
            Self::NoCommand => None,
            Self::Command(error) => Some(error),
            Self::Viewer(error) => Some(error),
            Self::Revision(error) => Some(error),
        }
    }
}

impl From<ToolError> for EditorDomainError {
    fn from(value: ToolError) -> Self {
        Self::Tool(value)
    }
}

impl From<CommandStackError> for EditorDomainError {
    fn from(value: CommandStackError) -> Self {
        Self::Command(value)
    }
}

impl From<ViewerError> for EditorDomainError {
    fn from(value: ViewerError) -> Self {
        Self::Viewer(value)
    }
}

impl From<InteractionRevisionError> for EditorDomainError {
    fn from(value: InteractionRevisionError) -> Self {
        Self::Revision(value)
    }
}

/// Pointer release result after optional command execution.
#[derive(Clone, Copy, Debug)]
pub struct DomainPointerRelease {
    dispatch: DispatchOutcome,
    command_executed: bool,
}

impl DomainPointerRelease {
    /// Returns Figure dispatch state after release.
    pub const fn dispatch(self) -> DispatchOutcome {
        self.dispatch
    }

    /// Returns whether release committed a model Command.
    pub const fn command_executed(self) -> bool {
        self.command_executed
    }
}

/// Owns one active Tool and the command history shared by an editor session.
pub struct EditorDomain<A: ModelAdapter> {
    command_stack: CommandStack<A>,
    selection_tool: SelectionTool,
    connection_tool: Option<ConnectionCreationTool<A>>,
    endpoint_tool: Option<ConnectionEndpointTool<A>>,
    bendpoint_tool: ConnectionBendpointTool,
    next_revision: InteractionRevision,
    pointer: Option<Point>,
    autoexpose_requested: bool,
}

impl<A: ModelAdapter> Default for EditorDomain<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: ModelAdapter> EditorDomain<A> {
    /// Creates a domain with an unlimited command history and SelectionTool active.
    pub fn new() -> Self {
        Self {
            command_stack: CommandStack::new(),
            selection_tool: SelectionTool::new(),
            connection_tool: None,
            endpoint_tool: None,
            bendpoint_tool: ConnectionBendpointTool::new(),
            next_revision: InteractionRevision::initial(),
            pointer: None,
            autoexpose_requested: false,
        }
    }

    /// Returns the command history.
    pub const fn command_stack(&self) -> &CommandStack<A> {
        &self.command_stack
    }

    /// Returns whether the active Tool owns a pointer gesture.
    pub const fn has_active_gesture(&self) -> bool {
        self.selection_tool.is_active()
            || match &self.connection_tool {
                Some(tool) => tool.is_started(),
                None => false,
            }
            || match &self.endpoint_tool {
                Some(tool) => tool.is_active(),
                None => false,
            }
            || self.bendpoint_tool.is_active()
    }

    /// Returns whether the one-shot connection-creation Tool is armed.
    pub const fn is_connection_creation_active(&self) -> bool {
        self.connection_tool.is_some()
    }

    /// Returns whether the host should schedule a viewport auto-expose step.
    pub const fn autoexpose_requested(&self) -> bool {
        self.autoexpose_requested
    }

    /// Returns whether this Viewer currently routes text input to direct editing.
    pub fn has_active_direct_text_edit<F>(&self, viewer: &GraphicalViewer<A, F>) -> bool
    where
        F: EditPartFactory<A>,
    {
        viewer.direct_text_edit().is_some()
    }

    /// Allocates the next request revision.
    pub fn next_interaction_revision(&mut self) -> Result<InteractionRevision, EditorDomainError> {
        let revision = self.next_revision;
        self.next_revision = revision.next()?;
        Ok(revision)
    }
}

impl<A> EditorDomain<A>
where
    A: ModelAdapter + 'static,
{
    /// Starts direct editing for one stable application text feature.
    pub fn start_direct_text_edit<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        source: EditPartId,
        feature: DirectTextFeature,
    ) -> Result<DirectTextEditSessionId, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.cancel_active_tools(viewer)?;
        Ok(viewer.start_direct_text_edit(&DirectTextEditRequest::new(source, feature))?)
    }

    /// Replaces the current text selection with committed draft text.
    pub fn insert_direct_text<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        text: &str,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.insert_direct_text(text)?)
    }

    /// Replaces or updates the active IME preedit without committing the edit session.
    pub fn set_direct_text_preedit<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        text: &str,
        selection: Option<std::ops::Range<usize>>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.set_direct_text_preedit(text, selection)?)
    }

    /// Cancels IME composition and restores its captured draft base.
    pub fn cancel_direct_text_preedit<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.cancel_direct_text_preedit()?)
    }

    /// Replaces the directed text selection without changing Viewer selection.
    pub fn set_direct_text_selection<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        selection: FlowTextRange,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.set_direct_text_selection(selection)?)
    }

    /// Moves the direct-edit focus through the current immutable interaction map.
    pub fn move_direct_text<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        movement: TextMovement,
        extend: ExtendTextSelection,
    ) -> Result<FlowTextPosition, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.move_direct_text(movement, matches!(extend, ExtendTextSelection::Yes))?)
    }

    /// Deletes a selected range or a visual grapheme/word adjacent to the caret.
    pub fn delete_direct_text<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        deletion: TextDelete,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.delete_direct_text(deletion)?)
    }

    /// Resolves a surface point through the draft TextFlow interaction map.
    pub fn hit_test_direct_text<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        extend: ExtendTextSelection,
    ) -> Result<FlowTextPosition, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.hit_test_direct_text(location, matches!(extend, ExtendTextSelection::Yes))?)
    }

    /// Accepts the active draft as at most one model Command.
    pub fn accept_direct_text_edit<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let mut prepared = viewer.prepare_direct_text_accept()?;
        let Some(command) = prepared.command.take() else {
            return Ok(false);
        };
        match self.command_stack.execute(viewer.model_mut()?, command) {
            Ok(()) => {
                viewer.refresh()?;
                Ok(true)
            }
            Err(error) => {
                let recoverable = match &error {
                    CommandStackError::Rejected { .. } => true,
                    CommandStackError::Command(command) => command.is_recoverable(),
                    _ => false,
                };
                if recoverable {
                    viewer.restore_direct_text_edit(prepared)?;
                }
                Err(error.into())
            }
        }
    }

    /// Cancels the active draft without creating command history.
    pub fn cancel_direct_text_edit<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        Ok(viewer.cancel_direct_text_edit()?)
    }

    /// Applies the descriptor's policy when direct-edit input focus is lost.
    pub fn direct_text_focus_lost<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let focus_loss = viewer
            .direct_text_edit()
            .ok_or(ViewerError::DirectTextEdit(
                crate::DirectTextEditError::NoActiveSession,
            ))?
            .focus_loss();
        match focus_loss {
            crate::FocusLossPolicy::Accept => self.accept_direct_text_edit(viewer),
            crate::FocusLossPolicy::Cancel => self.cancel_direct_text_edit(viewer),
        }
    }

    /// Applies one normalized event only when its session still owns the text-input lease.
    pub fn handle_text_input_event<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        input: SessionTextInputEvent,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let active = viewer.direct_text_edit().map(DirectTextEditState::session);
        if active != Some(input.session()) {
            return Err(ViewerError::DirectTextEdit(
                crate::DirectTextEditError::TextInputLeaseLost,
            )
            .into());
        }
        match input.into_event() {
            TextInputEvent::Preedit { text, selection } => {
                self.set_direct_text_preedit(viewer, &text, selection)?;
            }
            TextInputEvent::CancelComposition => {
                self.cancel_direct_text_preedit(viewer)?;
            }
            TextInputEvent::InsertText(text) => {
                self.insert_direct_text(viewer, &text)?;
            }
            TextInputEvent::Delete(deletion) => {
                self.delete_direct_text(viewer, deletion)?;
            }
            TextInputEvent::Move { movement, extend } => {
                self.move_direct_text(viewer, movement, extend)?;
            }
            TextInputEvent::SelectAll => viewer.select_all_direct_text()?,
            TextInputEvent::Accept => {
                self.accept_direct_text_edit(viewer)?;
            }
            TextInputEvent::Cancel => {
                self.cancel_direct_text_edit(viewer)?;
            }
            TextInputEvent::FocusLost => {
                self.direct_text_focus_lost(viewer)?;
            }
            TextInputEvent::LeaseLost => {
                self.cancel_direct_text_edit(viewer)?;
            }
        }
        Ok(())
    }

    /// Cancels the current gesture and arms one connection-creation Tool.
    pub fn activate_connection_creation<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        connection_type: CreationType,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.selection_tool.cancel(viewer)?;
        if let Some(tool) = &mut self.connection_tool {
            tool.cancel(viewer)?;
        }
        if let Some(tool) = &mut self.endpoint_tool {
            tool.cancel(viewer)?;
        }
        self.endpoint_tool = None;
        self.bendpoint_tool.cancel(viewer)?;
        self.connection_tool = Some(ConnectionCreationTool::new(connection_type));
        Ok(())
    }

    /// Executes a policy-resolved Request and refreshes the model projection.
    pub fn execute_request<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        request: &EditorRequest,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        let command = viewer
            .command_for_request(request)?
            .ok_or(EditorDomainError::NoCommand)?;
        self.command_stack.execute(viewer.model_mut()?, command)?;
        viewer.refresh()?;
        Ok(())
    }

    /// Undoes the latest command and refreshes the Viewer.
    pub fn undo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        self.command_stack.undo(viewer.model_mut()?)?;
        viewer.refresh()?;
        Ok(())
    }

    /// Redoes the latest undone command and refreshes the Viewer.
    pub fn redo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        self.command_stack.redo(viewer.model_mut()?)?;
        viewer.refresh()?;
        Ok(())
    }

    /// Dispatches pointer press and lets the active Tool lock a drag source.
    pub fn pointer_pressed<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
    ) -> Result<ViewerInputOutcome, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.pointer = Some(location);
        self.autoexpose_requested = false;
        if viewer.direct_text_edit().is_some() {
            let targets_handle = matches!(
                viewer.target_at(location.x(), location.y()),
                ViewerTarget::Handle { .. }
            );
            if button == MouseButton::Left
                && !targets_handle
                && viewer.direct_text_feedback_contains(location)
            {
                viewer.hit_test_direct_text(location, modifiers.shift)?;
                return Ok(viewer.dispatch_mouse_pressed_without_selection(
                    location.x(),
                    location.y(),
                    button,
                ));
            }
            self.direct_text_focus_lost(viewer)?;
            if viewer.direct_text_edit().is_some() {
                return Ok(viewer.dispatch_mouse_pressed_without_selection(
                    location.x(),
                    location.y(),
                    button,
                ));
            }
        }
        if let Some(tool) = &mut self.connection_tool {
            let revision = self.next_revision;
            self.next_revision = revision.next()?;
            let press = tool.pointer_pressed(viewer, location, button, modifiers, revision)?;
            let completed = press.completed();
            let (outcome, command) = press.into_parts();
            if completed {
                self.connection_tool = None;
                self.pointer = None;
            }
            if let Some(command) = command {
                self.command_stack.execute(viewer.model_mut()?, command)?;
                viewer.refresh()?;
            }
            return Ok(outcome);
        }
        if let ViewerTarget::Handle {
            owner,
            role: HandleRole::ConnectionEndpoint(endpoint),
            ..
        } = viewer.target_at(location.x(), location.y())
            && let Some(connection) = viewer.as_connection_part(owner)
        {
            let revision = self.next_interaction_revision()?;
            let mut tool = ConnectionEndpointTool::new(connection, endpoint);
            let outcome = tool.pointer_pressed(viewer, location, button, modifiers, revision)?;
            if tool.is_active() {
                self.endpoint_tool = Some(tool);
            }
            return Ok(outcome);
        }
        if let ViewerTarget::Handle { owner, role, .. } =
            viewer.target_at(location.x(), location.y())
            && let Some(connection) = viewer.as_connection_part(owner)
        {
            let operation = match role {
                HandleRole::BendpointMove(index) => BendpointOperation::Move { index },
                HandleRole::BendpointCreate(index) => BendpointOperation::Create { index },
                HandleRole::Selection
                | HandleRole::Resize(_)
                | HandleRole::ConnectionEndpoint(_) => {
                    return Ok(self
                        .selection_tool
                        .pointer_pressed(viewer, location, button, modifiers)?);
                }
            };
            return Ok(self
                .bendpoint_tool
                .pointer_pressed(viewer, connection, operation, location, button, modifiers)?);
        }
        Ok(self
            .selection_tool
            .pointer_pressed(viewer, location, button, modifiers)?)
    }

    /// Updates the active gesture and Policy feedback.
    pub fn pointer_moved<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
    ) -> Result<DispatchOutcome, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        if viewer.direct_text_edit().is_some() {
            self.pointer = Some(location);
            self.autoexpose_requested = false;
            return Ok(viewer.dispatch_mouse_moved(location.x(), location.y()));
        }
        let revision = self.next_interaction_revision()?;
        let dispatch = if self.bendpoint_tool.is_active() {
            self.bendpoint_tool
                .pointer_moved(viewer, location, revision)?
        } else if let Some(tool) = &mut self.endpoint_tool {
            tool.pointer_moved(viewer, location, revision)?
        } else if let Some(tool) = &mut self.connection_tool {
            tool.pointer_moved(viewer, location, revision)?
        } else {
            self.selection_tool
                .pointer_moved(viewer, location, revision)?
        };
        self.pointer = self.has_active_gesture().then_some(location);
        if self.supports_autoexpose() {
            self.stabilize_active_tool_projection(viewer, location)?;
        }
        self.autoexpose_requested =
            self.supports_autoexpose() && autoexpose::detects(viewer, location)?;
        Ok(dispatch)
    }

    /// Advances viewport auto-expose using host-provided monotonic elapsed time.
    pub fn autoexpose_tick<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        elapsed: Duration,
    ) -> Result<AutoexposeTick, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let Some(pointer) = self.pointer.filter(|_| self.supports_autoexpose()) else {
            self.autoexpose_requested = false;
            return Ok(AutoexposeTick::default());
        };
        let outcome = autoexpose::step(viewer, pointer, elapsed)?;
        if outcome.scrolled() {
            let revision = self.next_interaction_revision()?;
            self.refresh_active_tool(viewer, pointer, revision)?;
            self.stabilize_active_tool_projection(viewer, pointer)?;
        }
        self.autoexpose_requested =
            outcome.continue_requested() && autoexpose::detects(viewer, pointer)?;
        Ok(AutoexposeTick::new(
            outcome.scrolled(),
            self.autoexpose_requested,
        ))
    }

    /// Changes zoom and refreshes an active gesture at its fixed surface pointer.
    pub fn set_viewport_scale_at<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let changed = viewer.set_viewport_scale_at(scale, anchor)?;
        if changed {
            self.refresh_after_viewport_change(viewer)?;
        }
        Ok(changed)
    }

    /// Changes the root Viewport origin and refreshes an active gesture.
    pub fn set_viewport_origin<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        origin: Point,
    ) -> Result<bool, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let changed = viewer.set_viewport_origin(origin)?;
        if changed {
            self.refresh_after_viewport_change(viewer)?;
        }
        Ok(changed)
    }

    /// Clears pointer-derived scheduling when the pointer leaves the Viewer.
    pub fn pointer_exited<F>(&mut self, viewer: &mut GraphicalViewer<A, F>)
    where
        F: EditPartFactory<A>,
    {
        self.pointer = None;
        self.autoexpose_requested = false;
        viewer.pointer_exited();
    }

    /// Releases the active gesture, clears feedback, and executes its Request.
    pub fn pointer_released<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        button: MouseButton,
    ) -> Result<DomainPointerRelease, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.pointer = None;
        self.autoexpose_requested = false;
        if viewer.direct_text_edit().is_some() {
            return Ok(DomainPointerRelease {
                dispatch: viewer.dispatch_mouse_released(location.x(), location.y(), button),
                command_executed: false,
            });
        }
        if let Some(mut tool) = self.endpoint_tool.take() {
            let revision = self.next_interaction_revision()?;
            let (dispatch, command) = tool
                .pointer_released(viewer, location, button, revision)?
                .into_parts();
            let command_executed = command.is_some();
            if let Some(command) = command {
                self.command_stack.execute(viewer.model_mut()?, command)?;
                viewer.refresh()?;
            }
            viewer
                .runtime_mut()
                .stabilize_for_query()
                .map_err(ViewerError::from)?;
            return Ok(DomainPointerRelease {
                dispatch,
                command_executed,
            });
        }
        if self.bendpoint_tool.is_active() {
            let revision = self.next_interaction_revision()?;
            let release = self
                .bendpoint_tool
                .pointer_released(viewer, location, button, revision)?;
            let dispatch = release.dispatch();
            let request = release.into_request();
            let command_executed = if let Some(request) = request {
                self.execute_request(viewer, &request)?;
                true
            } else {
                false
            };
            viewer
                .runtime_mut()
                .stabilize_for_query()
                .map_err(ViewerError::from)?;
            return Ok(DomainPointerRelease {
                dispatch,
                command_executed,
            });
        }
        if let Some(tool) = &mut self.connection_tool {
            return Ok(DomainPointerRelease {
                dispatch: tool.pointer_released(viewer, location, button),
                command_executed: false,
            });
        }
        let revision = self.next_interaction_revision()?;
        let release = self
            .selection_tool
            .pointer_released(viewer, location, button, revision)?;
        let dispatch = release.dispatch();
        let request = release.into_request();
        let command_executed = if let Some(request) = request {
            self.execute_request(viewer, &request)?;
            true
        } else {
            false
        };
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        Ok(DomainPointerRelease {
            dispatch,
            command_executed,
        })
    }

    /// Cancels the active Tool gesture and removes transient feedback.
    pub fn cancel_tool<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.cancel_active_tools(viewer)?;
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        Ok(())
    }

    fn cancel_active_tools<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.pointer = None;
        self.autoexpose_requested = false;
        self.selection_tool.cancel(viewer)?;
        if let Some(tool) = &mut self.connection_tool {
            tool.cancel(viewer)?;
        }
        self.connection_tool = None;
        if let Some(tool) = &mut self.endpoint_tool {
            tool.cancel(viewer)?;
        }
        self.endpoint_tool = None;
        self.bendpoint_tool.cancel(viewer)?;
        Ok(())
    }

    fn supports_autoexpose(&self) -> bool {
        self.selection_tool.is_dragging()
            || self
                .connection_tool
                .as_ref()
                .is_some_and(ConnectionCreationTool::is_started)
            || self
                .endpoint_tool
                .as_ref()
                .is_some_and(ConnectionEndpointTool::is_dragging)
            || self.bendpoint_tool.is_dragging()
    }

    /// Recomputes the active Tool after an externally driven viewport layout change.
    pub fn refresh_after_viewport_change<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        viewer.synchronize_direct_text_input_area()?;
        let Some(pointer) = self.pointer else {
            return Ok(());
        };
        let revision = self.next_interaction_revision()?;
        self.refresh_active_tool(viewer, pointer, revision)?;
        self.stabilize_active_tool_projection(viewer, pointer)?;
        self.autoexpose_requested =
            self.supports_autoexpose() && autoexpose::detects(viewer, pointer)?;
        Ok(())
    }

    fn stabilize_active_tool_projection<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let origin_before = viewer.viewport_origin()?;
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        if let Some(parts) = self
            .selection_tool
            .dragged_parts()
            .map(|parts| parts.to_vec())
        {
            viewer.clamp_viewport_to_feedback_replacement(&parts)?;
        }
        let origin_after_feedback = viewer.viewport_origin()?;
        if origin_after_feedback == origin_before {
            return Ok(());
        }

        self.clear_active_tool_feedback(viewer)?;
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        let permanent_origin = viewer.viewport_origin()?;
        let revision = self.next_interaction_revision()?;
        self.refresh_active_tool(viewer, location, revision)?;
        viewer
            .runtime_mut()
            .stabilize_for_query()
            .map_err(ViewerError::from)?;
        let final_origin = viewer.viewport_origin()?;
        if final_origin != permanent_origin {
            return Err(ViewerError::InconsistentState.into());
        }
        Ok(())
    }

    fn refresh_active_tool<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        location: Point,
        revision: InteractionRevision,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        if self.bendpoint_tool.is_active() {
            self.bendpoint_tool.refresh(viewer, location, revision)?;
        } else if let Some(tool) = &mut self.endpoint_tool {
            tool.refresh(viewer, location, revision)?;
        } else if let Some(tool) = &mut self.connection_tool {
            tool.refresh(viewer, location, revision)?;
        } else {
            self.selection_tool.refresh(viewer, location, revision)?;
        }
        Ok(())
    }

    fn clear_active_tool_feedback<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        if self.bendpoint_tool.is_active() {
            self.bendpoint_tool.clear_transient_feedback(viewer)?;
        } else if let Some(tool) = &mut self.endpoint_tool {
            tool.clear_transient_feedback(viewer)?;
        } else if let Some(tool) = &mut self.connection_tool {
            tool.clear_transient_feedback(viewer)?;
        } else {
            self.selection_tool.clear_transient_feedback(viewer)?;
        }
        Ok(())
    }
}
