//! Editor session coordination.

use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    time::Duration,
};

use novadraw::event::{DispatchOutcome, KeyEvent, KeyModifiers, MonotonicTime, MouseButton};
use novadraw::figure::{FlowTextPosition, FlowTextRange};
use novadraw::geometry::Point;
use novadraw::text::TextMovement;

use crate::{
    AutoexposeTick, BendpointCreateAction, BendpointMoveAction, BendpointOperation, Command,
    CommandStack, CommandStackError, ConnectionBendpointTool, ConnectionCreationTool,
    ConnectionEndpointAction, ConnectionEndpointTool, CreationType, DirectTextEditRequest,
    DirectTextEditSessionId, DirectTextEditState, DirectTextFeature, EditPartFactory, EditPartId,
    EditingContribution, ErasedToolKey, ExtendTextSelection, GraphicalViewer, HandleAction,
    HandleActionProtocol, InteractionRevision, InteractionRevisionError, ModelAdapter,
    RequestMetadata, RequestProtocol, SelectionTool, SessionTextInputEvent, TextDelete,
    TextInputEvent, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent,
    ViewerError, ViewerInputOutcome, ViewerTarget, autoexpose,
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

fn command_error_preserves_model(error: &CommandStackError) -> bool {
    match error {
        CommandStackError::Rejected { .. }
        | CommandStackError::NothingToUndo
        | CommandStackError::NothingToRedo
        | CommandStackError::HistoryExhausted => true,
        CommandStackError::Command(error) => error.is_recoverable(),
        CommandStackError::Panicked { .. } | CommandStackError::Faulted => false,
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

type HandleToolFactory<A> = Box<
    dyn for<'a> Fn(
        &mut ToolContext<'a, A>,
        EditPartId,
        &HandleAction,
    ) -> Result<Box<dyn Tool<A>>, ToolError>,
>;
type ToolCallResult<A, T> = Result<(T, Option<Box<dyn Command<A>>>), EditorDomainError>;

/// Coordinates one active Tool, command history, and Viewer interaction state.
pub struct EditorDomain<A: ModelAdapter> {
    command_stack: CommandStack<A>,
    active_tool: Box<dyn Tool<A>>,
    handle_tools: HashMap<TypeId, HandleToolFactory<A>>,
    next_revision: InteractionRevision,
    pointer: Option<Point>,
    autoexpose_requested: bool,
}

impl<A: ModelAdapter + 'static> Default for EditorDomain<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: ModelAdapter + 'static> EditorDomain<A> {
    /// Creates a domain with an unlimited command history and SelectionTool active.
    pub fn new() -> Self {
        let mut domain = Self {
            command_stack: CommandStack::new(),
            active_tool: Box::new(SelectionTool::new()),
            handle_tools: HashMap::new(),
            next_revision: InteractionRevision::initial(),
            pointer: None,
            autoexpose_requested: false,
        };
        domain
            .register_handle_tool::<ConnectionEndpointAction, ConnectionEndpointTool<A>>(
                |context, owner, endpoint| {
                    let connection = context
                        .as_connection_part(owner)
                        .ok_or_else(|| ToolError::protocol("handle owner is not a connection"))?;
                    Ok(ConnectionEndpointTool::new(connection, *endpoint))
                },
            )
            .expect("built-in connection endpoint action is unique");
        domain
            .register_handle_tool::<BendpointMoveAction, ConnectionBendpointTool>(
                |context, owner, index| {
                    let connection = context
                        .as_connection_part(owner)
                        .ok_or_else(|| ToolError::protocol("handle owner is not a connection"))?;
                    Ok(ConnectionBendpointTool::for_handle(
                        connection,
                        BendpointOperation::Move { index: *index },
                    ))
                },
            )
            .expect("built-in bendpoint move action is unique");
        domain
            .register_handle_tool::<BendpointCreateAction, ConnectionBendpointTool>(
                |context, owner, index| {
                    let connection = context
                        .as_connection_part(owner)
                        .ok_or_else(|| ToolError::protocol("handle owner is not a connection"))?;
                    Ok(ConnectionBendpointTool::for_handle(
                        connection,
                        BendpointOperation::Create { index: *index },
                    ))
                },
            )
            .expect("built-in bendpoint create action is unique");
        domain
    }

    /// Returns the command history.
    pub const fn command_stack(&self) -> &CommandStack<A> {
        &self.command_stack
    }

    /// Returns whether the ready command stack has undo history without invoking extension code.
    pub fn has_undo(&self) -> bool {
        self.command_stack.has_undo()
    }

    /// Returns whether the ready command stack has redo history without invoking extension code.
    pub fn has_redo(&self) -> bool {
        self.command_stack.has_redo()
    }

    /// Returns whether history differs from the last successfully saved state.
    pub fn is_dirty(&self) -> bool {
        self.command_stack.is_dirty()
    }

    /// Marks the current command-history identity after application storage succeeds.
    pub fn mark_save_location(&mut self) -> Result<(), EditorDomainError> {
        Ok(self.command_stack.mark_save_location()?)
    }

    /// Executes an application command through the same model transaction as typed requests.
    pub fn execute_command<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        command: Box<dyn Command<A>>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))
    }

    /// Drops an application template through its typed request and installed policies.
    /// The location is in logical surface coordinates, as with pointer input.
    pub fn drop_template<T, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        template: &T,
        location: Point,
        modifiers: crate::RequestModifiers,
    ) -> Result<(), EditorDomainError>
    where
        T: crate::tool::CreationTemplate<A>,
        F: EditPartFactory<A>,
    {
        self.cancel_active_tools(viewer)?;
        viewer.stabilize_for_query()?;
        let location = viewer.model_point_from_surface(location)?;
        let size = template.default_size();
        let revision = self.next_interaction_revision()?;
        let (metadata, payload) = template.request(
            viewer.contents(),
            novadraw::geometry::Rectangle::new(location.x(), location.y(), size.width, size.height),
            modifiers,
            revision,
        );
        self.execute::<T::Request, F>(viewer, &metadata, &payload)
    }

    /// Checks whether the top command currently permits undo.
    pub fn check_undo(&mut self) -> Result<bool, EditorDomainError> {
        Ok(self.command_stack.check_undo()?)
    }

    /// Checks whether the top command currently permits redo.
    pub fn check_redo(&mut self) -> Result<bool, EditorDomainError> {
        Ok(self.command_stack.check_redo()?)
    }

    /// Returns whether the active Tool owns a pointer gesture.
    pub fn has_active_gesture(&self) -> bool {
        self.active_tool.has_active_gesture()
    }

    /// Returns whether the one-shot connection-creation Tool is armed.
    pub fn is_connection_creation_active(&self) -> bool {
        self.active_tool.key()
            == ToolKey::<ConnectionCreationTool<A>>::new("connection-creation").erase()
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

    /// Registers a Tool factory for one typed handle action.
    pub fn register_handle_tool<R, T>(
        &mut self,
        factory: impl for<'a> Fn(
            &mut ToolContext<'a, A>,
            EditPartId,
            &R::Payload,
        ) -> Result<T, ToolError>
        + 'static,
    ) -> Result<(), ToolError>
    where
        R: HandleActionProtocol,
        T: Tool<A>,
    {
        let key = TypeId::of::<R>();
        if self.handle_tools.contains_key(&key) {
            return Err(ToolError::protocol(format!(
                "duplicate handle Tool factory: {}",
                R::key().name()
            )));
        }
        self.handle_tools.insert(
            key,
            Box::new(move |context, owner, action| {
                let payload = action
                    .get::<R>()
                    .ok_or_else(|| ToolError::protocol("handle action descriptor mismatch"))?;
                Ok(Box::new(factory(context, owner, payload)?) as Box<dyn Tool<A>>)
            }),
        );
        Ok(())
    }

    /// Returns the active Tool identity.
    pub fn active_tool_key(&self) -> ErasedToolKey {
        self.active_tool.key()
    }
}

impl<A> EditorDomain<A>
where
    A: ModelAdapter + 'static,
{
    fn apply_command_operation<F, T>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        operation: impl FnOnce(&mut CommandStack<A>, &mut A) -> Result<T, CommandStackError>,
    ) -> Result<T, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let mut session = viewer.begin_edit_session()?;
        let result = operation(&mut self.command_stack, session.model());
        match result {
            Ok(value) => {
                session.commit()?;
                Ok(value)
            }
            Err(error) if command_error_preserves_model(&error) => {
                session.reject()?;
                Err(error.into())
            }
            Err(error) => {
                session.fault();
                Err(error.into())
            }
        }
    }

    fn call_active_tool<F, T>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        callback: impl FnOnce(&mut dyn Tool<A>, &mut ToolContext<'_, A>) -> Result<T, ToolError>,
    ) -> ToolCallResult<A, T>
    where
        F: EditPartFactory<A>,
    {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut context = ToolContext::new(viewer);
            let value = callback(self.active_tool.as_mut(), &mut context)?;
            Ok::<_, ToolError>((value, context.take_command()))
        }));
        match result {
            Ok(result) => Ok(result?),
            Err(payload) => {
                viewer.fault_extension();
                resume_unwind(payload)
            }
        }
    }

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
        let metadata = RequestMetadata::new(self.next_interaction_revision()?, vec![source]);
        Ok(viewer
            .start_direct_text_edit(&metadata, &DirectTextEditRequest::new(source, feature))?)
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
        match self.apply_command_operation(viewer, |stack, model| stack.execute(model, command)) {
            Ok(()) => Ok(true),
            Err(error) => {
                if matches!(
                    &error,
                    EditorDomainError::Command(command)
                        if command_error_preserves_model(command)
                ) {
                    viewer.restore_direct_text_edit(prepared)?;
                }
                Err(error)
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
            TextInputEvent::Synchronize(snapshot) => {
                viewer.synchronize_direct_text_input(&snapshot)?;
            }
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
        self.cancel_active_tools(viewer)?;
        self.replace_active_tool(
            viewer,
            Box::new(ConnectionCreationTool::new(connection_type)),
        )?;
        Ok(())
    }

    /// Executes a policy-resolved Request and refreshes the model projection.
    pub fn execute<R, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        let command = viewer
            .command_for::<R>(metadata, payload)?
            .ok_or(EditorDomainError::NoCommand)?;
        self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        Ok(())
    }

    /// Undoes the latest command and refreshes the Viewer.
    pub fn undo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        self.apply_command_operation(viewer, CommandStack::undo)?;
        Ok(())
    }

    /// Redoes the latest undone command and refreshes the Viewer.
    pub fn redo<F>(&mut self, viewer: &mut GraphicalViewer<A, F>) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.cancel_direct_text_edit()?;
        self.cancel_active_tools(viewer)?;
        self.apply_command_operation(viewer, CommandStack::redo)?;
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
        if self.active_tool.accepts_handle_activation() {
            if self.has_active_gesture() {
                self.cancel_active_tools(viewer)?;
            }
            if let ViewerTarget::Handle { owner, action, .. } =
                viewer.target_at(location.x(), location.y())
                && let Some(factory) = self.handle_tools.remove(&action.type_id())
            {
                let created = catch_unwind(AssertUnwindSafe(|| {
                    let mut context = ToolContext::new(viewer);
                    factory(&mut context, owner, &action)
                }));
                self.handle_tools.insert(action.type_id(), factory);
                let created = match created {
                    Ok(created) => created?,
                    Err(payload) => {
                        viewer.fault_extension();
                        resume_unwind(payload)
                    }
                };
                self.replace_active_tool(viewer, created)?;
            }
        }
        let revision = self.next_interaction_revision()?;
        let event = ToolMouseEvent::new(location, button, modifiers, revision);
        let ((directive, outcome), command) =
            self.call_active_tool(viewer, |tool, context| tool.mouse_pressed(context, event))?;
        self.apply_tool_directive(viewer, directive)?;
        if let Some(command) = command {
            self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        }
        Ok(outcome)
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
        let event = ToolMouseEvent::new(
            location,
            MouseButton::None,
            KeyModifiers::default(),
            revision,
        );
        let ((directive, dispatch), command) =
            self.call_active_tool(viewer, |tool, context| tool.mouse_moved(context, event))?;
        self.apply_tool_directive(viewer, directive)?;
        if let Some(command) = command {
            self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        }
        self.pointer = self.has_active_gesture().then_some(location);
        if self.supports_autoexpose() {
            self.stabilize_active_tool_projection(viewer, location)?;
        }
        self.autoexpose_requested =
            self.supports_autoexpose() && autoexpose::detects(viewer, location)?;
        Ok(dispatch)
    }

    /// Routes one normalized key event to the active Tool.
    pub fn key_event<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        event: KeyEvent,
    ) -> Result<ToolDirective, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let (directive, command) =
            self.call_active_tool(viewer, |tool, context| tool.key_event(context, event))?;
        self.apply_tool_directive(viewer, directive)?;
        if let Some(command) = command {
            self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        }
        Ok(directive)
    }

    /// Routes one monotonic timer tick to the active Tool.
    pub fn tool_tick<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        now: MonotonicTime,
    ) -> Result<ToolDirective, EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        let (directive, command) =
            self.call_active_tool(viewer, |tool, context| tool.tick(context, now))?;
        self.apply_tool_directive(viewer, directive)?;
        if let Some(command) = command {
            self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        }
        Ok(directive)
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
        let revision = self.next_interaction_revision()?;
        let event = ToolMouseEvent::new(location, button, KeyModifiers::default(), revision);
        let ((directive, dispatch), command) =
            self.call_active_tool(viewer, |tool, context| tool.mouse_released(context, event))?;
        self.apply_tool_directive(viewer, directive)?;
        let command_executed = command.is_some();
        if let Some(command) = command {
            self.apply_command_operation(viewer, |stack, model| stack.execute(model, command))?;
        }
        viewer.stabilize_for_query()?;
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
        viewer.stabilize_for_query()?;
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
        self.call_active_tool(viewer, |tool, context| {
            tool.cancel(context)?;
            tool.deactivate(context)
        })?;
        self.active_tool = Box::new(SelectionTool::new());
        self.call_active_tool(viewer, |tool, context| tool.activate(context))?;
        Ok(())
    }

    fn supports_autoexpose(&self) -> bool {
        self.active_tool.supports_autoexpose()
    }

    /// Replaces the active Tool through the same lifecycle used by built-in Tools.
    pub fn set_active_tool<T, F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        tool: T,
    ) -> Result<(), EditorDomainError>
    where
        T: Tool<A>,
        F: EditPartFactory<A>,
    {
        self.pointer = None;
        self.autoexpose_requested = false;
        self.call_active_tool(viewer, |tool, context| tool.cancel(context))?;
        self.replace_active_tool(viewer, Box::new(tool))
    }

    fn replace_active_tool<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        mut replacement: Box<dyn Tool<A>>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.call_active_tool(viewer, |tool, context| tool.deactivate(context))?;
        let activation = catch_unwind(AssertUnwindSafe(|| {
            let mut context = ToolContext::new(viewer);
            replacement.activate(&mut context)
        }));
        match activation {
            Ok(Ok(())) => {
                self.active_tool = replacement;
                Ok(())
            }
            Ok(Err(error)) => {
                self.call_active_tool(viewer, |tool, context| tool.activate(context))?;
                Err(error.into())
            }
            Err(payload) => {
                viewer.fault_extension();
                resume_unwind(payload)
            }
        }
    }

    fn apply_tool_directive<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        directive: ToolDirective,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        match directive {
            ToolDirective::Continue => {}
            ToolDirective::ReleaseCapture => viewer.pointer_exited(),
            ToolDirective::Deactivate => {
                self.pointer = None;
                self.replace_active_tool(viewer, Box::new(SelectionTool::new()))?;
            }
        }
        Ok(())
    }

    /// Recomputes the active Tool after an externally driven viewport layout change.
    pub fn refresh_after_viewport_change<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        viewer.stabilize_for_query()?;
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
        viewer.stabilize_for_query()?;
        if let Some(parts) = self.active_tool.dragged_parts().map(<[_]>::to_vec) {
            viewer.clamp_viewport_to_feedback_replacement(&parts)?;
        }
        let origin_after_feedback = viewer.viewport_origin()?;
        if origin_after_feedback == origin_before {
            return Ok(());
        }

        self.clear_active_tool_feedback(viewer)?;
        viewer.stabilize_for_query()?;
        let permanent_origin = viewer.viewport_origin()?;
        let revision = self.next_interaction_revision()?;
        self.refresh_active_tool(viewer, location, revision)?;
        viewer.stabilize_for_query()?;
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
        self.call_active_tool(viewer, |tool, context| {
            tool.refresh(context, location, revision)
        })?;
        Ok(())
    }

    fn clear_active_tool_feedback<F>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
    ) -> Result<(), EditorDomainError>
    where
        F: EditPartFactory<A>,
    {
        self.call_active_tool(viewer, |tool, context| {
            tool.clear_transient_feedback(context)
        })?;
        Ok(())
    }
}
