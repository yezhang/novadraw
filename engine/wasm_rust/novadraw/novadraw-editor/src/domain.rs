//! Editor session coordination.

use std::{error::Error, fmt};

use novadraw_geometry::Point;
use novadraw_scene::{DispatchOutcome, KeyModifiers, MouseButton};

use crate::{
    CommandStack, CommandStackError, EditPartFactory, EditorRequest, GraphicalViewer,
    InteractionRevision, InteractionRevisionError, ModelAdapter, SelectionTool, ToolError,
    ViewerError, ViewerInputOutcome,
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
pub struct EditorDomain<A> {
    command_stack: CommandStack<A>,
    selection_tool: SelectionTool,
    next_revision: InteractionRevision,
}

impl<A> Default for EditorDomain<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> EditorDomain<A> {
    /// Creates a domain with an unlimited command history and SelectionTool active.
    pub fn new() -> Self {
        Self {
            command_stack: CommandStack::new(),
            selection_tool: SelectionTool::new(),
            next_revision: InteractionRevision::initial(),
        }
    }

    /// Returns the command history.
    pub const fn command_stack(&self) -> &CommandStack<A> {
        &self.command_stack
    }

    /// Returns whether the active Tool owns a pointer gesture.
    pub const fn has_active_gesture(&self) -> bool {
        self.selection_tool.is_active()
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
    /// Executes a policy-resolved Request and refreshes the model projection.
    pub fn execute_request<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        request: &EditorRequest,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let command = viewer
            .command_for_request(request)?
            .ok_or(EditorDomainError::NoCommand)?;
        self.command_stack.execute(viewer.model_mut(), command)?;
        viewer.refresh()?;
        Ok(())
    }

    /// Undoes the latest command and refreshes the Viewer.
    pub fn undo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.command_stack.undo(viewer.model_mut())?;
        viewer.refresh()?;
        Ok(())
    }

    /// Redoes the latest undone command and refreshes the Viewer.
    pub fn redo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.command_stack.redo(viewer.model_mut())?;
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
        let revision = self.next_interaction_revision()?;
        Ok(self
            .selection_tool
            .pointer_moved(viewer, location, revision)?)
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
        self.selection_tool.cancel(viewer)?;
        Ok(())
    }
}
