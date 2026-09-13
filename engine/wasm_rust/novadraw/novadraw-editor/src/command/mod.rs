//! Model-only commands and undo/redo history.
//!
//! Commands may retain stable application model identity and owned model data. They must not
//! retain live EditPart or Figure handles because views are rebuilt after model restoration.

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Failure returned by a model command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandError {
    kind: CommandErrorKind,
    message: String,
}

impl CommandError {
    /// Creates an application command failure.
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Operation,
            message: message.into(),
        }
    }

    /// Creates a failure after which the model may no longer match its pre-call state.
    pub fn state_unknown(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Inconsistent,
            message: message.into(),
        }
    }

    /// Returns the application-defined failure message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns whether the command guarantees that the model remains at its pre-call state.
    pub const fn is_recoverable(&self) -> bool {
        matches!(self.kind, CommandErrorKind::Operation)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for CommandError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandErrorKind {
    Operation,
    Inconsistent,
}

/// Reversible operation on an application-owned model.
///
/// Implementors must not panic from `Drop`; history trimming and flushing cannot recover from
/// arbitrary destructor side effects.
pub trait Command<M> {
    /// Returns a user-facing description of the operation.
    fn label(&self) -> &str;

    /// Returns whether the command can execute against the current model.
    fn can_execute(&self, _model: &M) -> bool {
        true
    }

    /// Applies the operation to the model.
    ///
    /// Returning an error must leave the model and command in their pre-call state. A panic faults
    /// the owning [`CommandStack`] because arbitrary extension side effects cannot be rolled back.
    fn execute(&mut self, model: &mut M) -> Result<(), CommandError>;

    /// Reverts the operation.
    ///
    /// Returning an error must leave the model and command in their pre-call state.
    fn undo(&mut self, model: &mut M) -> Result<(), CommandError>;

    /// Returns whether the command can currently be undone.
    fn can_undo(&self) -> bool {
        true
    }

    /// Applies the operation again after a successful undo.
    ///
    /// Returning an error must leave the model and command in their pre-call state.
    fn redo(&mut self, model: &mut M) -> Result<(), CommandError> {
        self.execute(model)
    }

    /// Returns whether the command can currently be redone.
    fn can_redo(&self) -> bool {
        true
    }
}

/// Ordered collection of commands treated as one history entry.
pub struct CompoundCommand<M> {
    label: String,
    commands: Vec<Box<dyn Command<M>>>,
    executed: usize,
}

impl<M> CompoundCommand<M> {
    /// Creates an empty compound command.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            commands: Vec::new(),
            executed: 0,
        }
    }

    /// Appends a command to the execution order.
    pub fn push(&mut self, command: Box<dyn Command<M>>) {
        self.commands.push(command);
    }

    /// Returns the number of command contributions.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns whether the compound has no contributions.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    fn execute_from_start(&mut self, model: &mut M, redo: bool) -> Result<(), CommandError> {
        self.executed = 0;
        for index in 0..self.commands.len() {
            let result = if redo {
                self.commands[index].redo(model)
            } else {
                self.commands[index].execute(model)
            };
            if let Err(error) = result {
                for rollback in (0..self.executed).rev() {
                    if let Err(rollback_error) = self.commands[rollback].undo(model) {
                        return Err(CommandError::state_unknown(format!(
                            "{}; compound rollback failed: {}",
                            error, rollback_error
                        )));
                    }
                }
                self.executed = 0;
                return Err(error);
            }
            self.executed += 1;
        }
        Ok(())
    }
}

impl<M> Command<M> for CompoundCommand<M> {
    fn label(&self) -> &str {
        &self.label
    }

    fn can_execute(&self, model: &M) -> bool {
        !self.commands.is_empty()
            && self
                .commands
                .iter()
                .all(|command| command.can_execute(model))
    }

    fn execute(&mut self, model: &mut M) -> Result<(), CommandError> {
        self.execute_from_start(model, false)
    }

    fn undo(&mut self, model: &mut M) -> Result<(), CommandError> {
        let original_executed = self.executed;
        while self.executed > 0 {
            let index = self.executed - 1;
            match self.commands[index].undo(model) {
                Ok(()) => self.executed = index,
                Err(error) => {
                    for restore in index + 1..original_executed {
                        if let Err(restore_error) = self.commands[restore].redo(model) {
                            return Err(CommandError::state_unknown(format!(
                                "{}; compound undo compensation failed: {}",
                                error, restore_error
                            )));
                        }
                        self.executed = restore + 1;
                    }
                    self.executed = original_executed;
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn redo(&mut self, model: &mut M) -> Result<(), CommandError> {
        self.execute_from_start(model, true)
    }

    fn can_undo(&self) -> bool {
        self.executed == self.commands.len()
            && self.commands.iter().all(|command| command.can_undo())
    }

    fn can_redo(&self) -> bool {
        self.executed == 0 && self.commands.iter().all(|command| command.can_redo())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HistoryState(u64);

impl HistoryState {
    const INITIAL: Self = Self(0);
}

struct HistoryEntry<M> {
    before: HistoryState,
    after: HistoryState,
    command: Box<dyn Command<M>>,
}

/// Kind of a committed command stack transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStackEventKind {
    /// A new command executed.
    Executed,
    /// The latest command was undone.
    Undone,
    /// The next command was redone.
    Redone,
    /// The save location was updated.
    SaveLocationMarked,
    /// Undo and redo history was cleared.
    Flushed,
}

/// Committed command stack transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandStackEvent {
    kind: CommandStackEventKind,
    label: Option<String>,
}

impl CommandStackEvent {
    fn command(kind: CommandStackEventKind, label: &str) -> Self {
        Self {
            kind,
            label: Some(label.to_owned()),
        }
    }

    fn stack(kind: CommandStackEventKind) -> Self {
        Self { kind, label: None }
    }

    /// Returns the transition kind.
    pub const fn kind(&self) -> CommandStackEventKind {
        self.kind
    }

    /// Returns the command label when the event describes a command.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Failure returned by a command stack operation.
#[derive(Debug, Eq, PartialEq)]
pub enum CommandStackError {
    /// The command declined execution for the current model.
    Rejected {
        /// Operation rejected by the command.
        operation: CommandOperation,
        /// Label of the rejected command.
        label: String,
    },
    /// A command operation failed.
    Command(CommandError),
    /// There is no command available to undo.
    NothingToUndo,
    /// There is no command available to redo.
    NothingToRedo,
    /// The internal history identity space is exhausted.
    HistoryExhausted,
    /// A command panicked and left the model's state unknown.
    Panicked {
        /// Operation that panicked.
        operation: CommandOperation,
        /// Label of the command that panicked.
        label: String,
    },
    /// The stack was faulted by a previous command panic.
    Faulted,
}

impl fmt::Display for CommandStackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { operation, label } => {
                write!(formatter, "command rejected {operation}: {label}")
            }
            Self::Command(error) => error.fmt(formatter),
            Self::NothingToUndo => write!(formatter, "there is no command to undo"),
            Self::NothingToRedo => write!(formatter, "there is no command to redo"),
            Self::HistoryExhausted => write!(formatter, "command history identity is exhausted"),
            Self::Panicked { operation, label } => {
                write!(formatter, "command panicked during {operation}: {label}")
            }
            Self::Faulted => write!(formatter, "command stack is faulted"),
        }
    }
}

impl Error for CommandStackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Command(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CommandError> for CommandStackError {
    fn from(value: CommandError) -> Self {
        Self::Command(value)
    }
}

/// Command operation executed by the stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandOperation {
    /// Initial execution.
    Execute,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
}

impl fmt::Display for CommandOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execute => write!(formatter, "execute"),
            Self::Undo => write!(formatter, "undo"),
            Self::Redo => write!(formatter, "redo"),
        }
    }
}

/// Undo/redo history for commands that operate on one application model.
pub struct CommandStack<M> {
    undoable: Vec<HistoryEntry<M>>,
    redoable: Vec<HistoryEntry<M>>,
    current_state: HistoryState,
    save_state: HistoryState,
    next_state: u64,
    undo_limit: Option<usize>,
    events: Vec<CommandStackEvent>,
    faulted: bool,
}

impl<M> Default for CommandStack<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> CommandStack<M> {
    /// Creates an unlimited command stack.
    pub fn new() -> Self {
        Self {
            undoable: Vec::new(),
            redoable: Vec::new(),
            current_state: HistoryState::INITIAL,
            save_state: HistoryState::INITIAL,
            next_state: 1,
            undo_limit: None,
            events: Vec::new(),
            faulted: false,
        }
    }

    /// Creates a command stack retaining at most `limit` undo entries.
    pub fn with_undo_limit(limit: usize) -> Self {
        Self {
            undo_limit: Some(limit),
            ..Self::new()
        }
    }

    /// Executes a new command and records it after successful completion.
    pub fn execute(
        &mut self,
        model: &mut M,
        mut command: Box<dyn Command<M>>,
    ) -> Result<(), CommandStackError> {
        self.ensure_ready()?;
        let label = match catch_unwind(AssertUnwindSafe(|| command.label().to_owned())) {
            Ok(label) => label,
            Err(_) => {
                self.faulted = true;
                return Err(CommandStackError::Panicked {
                    operation: CommandOperation::Execute,
                    label: "<unavailable>".to_owned(),
                });
            }
        };
        let executable = match catch_unwind(AssertUnwindSafe(|| command.can_execute(model))) {
            Ok(executable) => executable,
            Err(_) => {
                self.faulted = true;
                return Err(CommandStackError::Panicked {
                    operation: CommandOperation::Execute,
                    label,
                });
            }
        };
        if !executable {
            return Err(CommandStackError::Rejected {
                operation: CommandOperation::Execute,
                label,
            });
        }
        let after = self.allocate_state()?;
        let result = catch_unwind(AssertUnwindSafe(|| command.execute(model)));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                if !error.is_recoverable() {
                    self.faulted = true;
                }
                return Err(error.into());
            }
            Err(_) => {
                self.faulted = true;
                return Err(CommandStackError::Panicked {
                    operation: CommandOperation::Execute,
                    label,
                });
            }
        }
        let event = CommandStackEvent::command(CommandStackEventKind::Executed, &label);
        self.redoable.clear();
        self.undoable.push(HistoryEntry {
            before: self.current_state,
            after,
            command,
        });
        self.current_state = after;
        self.enforce_undo_limit();
        self.events.push(event);
        Ok(())
    }

    /// Undoes the latest successful command.
    pub fn undo(&mut self, model: &mut M) -> Result<(), CommandStackError> {
        self.ensure_ready()?;
        let (label, allowed) = {
            let entry = self
                .undoable
                .last()
                .ok_or(CommandStackError::NothingToUndo)?;
            let label = catch_unwind(AssertUnwindSafe(|| entry.command.label().to_owned()));
            let allowed = catch_unwind(AssertUnwindSafe(|| entry.command.can_undo()));
            (label, allowed)
        };
        let label = match label {
            Ok(label) => label,
            Err(_) => return self.fault_from_query(CommandOperation::Undo, "<unavailable>"),
        };
        let allowed = match allowed {
            Ok(allowed) => allowed,
            Err(_) => return self.fault_from_query(CommandOperation::Undo, &label),
        };
        if !allowed {
            return Err(CommandStackError::Rejected {
                operation: CommandOperation::Undo,
                label,
            });
        }
        let mut entry = self
            .undoable
            .pop()
            .ok_or(CommandStackError::NothingToUndo)?;
        match catch_unwind(AssertUnwindSafe(|| entry.command.undo(model))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error.is_recoverable() => {
                self.undoable.push(entry);
                return Err(error.into());
            }
            Ok(Err(error)) => {
                self.undoable.push(entry);
                self.faulted = true;
                return Err(error.into());
            }
            Err(_) => {
                self.undoable.push(entry);
                self.faulted = true;
                return Err(CommandStackError::Panicked {
                    operation: CommandOperation::Undo,
                    label,
                });
            }
        }
        let event = CommandStackEvent::command(CommandStackEventKind::Undone, &label);
        self.current_state = entry.before;
        self.redoable.push(entry);
        self.events.push(event);
        Ok(())
    }

    /// Redoes the next successfully undone command.
    pub fn redo(&mut self, model: &mut M) -> Result<(), CommandStackError> {
        self.ensure_ready()?;
        let (label, allowed) = {
            let entry = self
                .redoable
                .last()
                .ok_or(CommandStackError::NothingToRedo)?;
            let label = catch_unwind(AssertUnwindSafe(|| entry.command.label().to_owned()));
            let allowed = catch_unwind(AssertUnwindSafe(|| entry.command.can_redo()));
            (label, allowed)
        };
        let label = match label {
            Ok(label) => label,
            Err(_) => return self.fault_from_query(CommandOperation::Redo, "<unavailable>"),
        };
        let allowed = match allowed {
            Ok(allowed) => allowed,
            Err(_) => return self.fault_from_query(CommandOperation::Redo, &label),
        };
        if !allowed {
            return Err(CommandStackError::Rejected {
                operation: CommandOperation::Redo,
                label,
            });
        }
        let mut entry = self
            .redoable
            .pop()
            .ok_or(CommandStackError::NothingToRedo)?;
        match catch_unwind(AssertUnwindSafe(|| entry.command.redo(model))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error.is_recoverable() => {
                self.redoable.push(entry);
                return Err(error.into());
            }
            Ok(Err(error)) => {
                self.redoable.push(entry);
                self.faulted = true;
                return Err(error.into());
            }
            Err(_) => {
                self.redoable.push(entry);
                self.faulted = true;
                return Err(CommandStackError::Panicked {
                    operation: CommandOperation::Redo,
                    label,
                });
            }
        }
        let event = CommandStackEvent::command(CommandStackEventKind::Redone, &label);
        self.current_state = entry.after;
        self.undoable.push(entry);
        self.events.push(event);
        Ok(())
    }

    /// Returns whether an undo command is available.
    pub fn can_undo(&mut self) -> bool {
        if self.faulted {
            return false;
        }
        let Some(entry) = self.undoable.last() else {
            return false;
        };
        match catch_unwind(AssertUnwindSafe(|| entry.command.can_undo())) {
            Ok(allowed) => allowed,
            Err(_) => {
                self.faulted = true;
                false
            }
        }
    }

    /// Returns whether a redo command is available.
    pub fn can_redo(&mut self) -> bool {
        if self.faulted {
            return false;
        }
        let Some(entry) = self.redoable.last() else {
            return false;
        };
        match catch_unwind(AssertUnwindSafe(|| entry.command.can_redo())) {
            Ok(allowed) => allowed,
            Err(_) => {
                self.faulted = true;
                false
            }
        }
    }

    /// Returns the number of retained undo entries.
    pub fn undo_len(&self) -> usize {
        self.undoable.len()
    }

    /// Returns the number of retained redo entries.
    pub fn redo_len(&self) -> usize {
        self.redoable.len()
    }

    /// Marks the current history identity as the saved document state.
    pub fn mark_save_location(&mut self) -> Result<(), CommandStackError> {
        self.ensure_ready()?;
        self.save_state = self.current_state;
        self.events.push(CommandStackEvent::stack(
            CommandStackEventKind::SaveLocationMarked,
        ));
        Ok(())
    }

    /// Returns whether the current history state differs from the saved state.
    pub fn is_dirty(&self) -> bool {
        self.faulted || self.current_state != self.save_state
    }

    /// Returns whether a command panic made the model state unsafe to continue editing.
    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    /// Clears undo and redo history while preserving the model state.
    pub fn flush(&mut self) -> Result<(), CommandStackError> {
        self.ensure_ready()?;
        self.undoable.clear();
        self.redoable.clear();
        self.current_state = HistoryState::INITIAL;
        self.save_state = HistoryState::INITIAL;
        self.events
            .push(CommandStackEvent::stack(CommandStackEventKind::Flushed));
        Ok(())
    }

    /// Drains committed stack events in causal order.
    pub fn take_events(&mut self) -> Vec<CommandStackEvent> {
        std::mem::take(&mut self.events)
    }

    fn allocate_state(&mut self) -> Result<HistoryState, CommandStackError> {
        let state = HistoryState(self.next_state);
        self.next_state = self
            .next_state
            .checked_add(1)
            .ok_or(CommandStackError::HistoryExhausted)?;
        Ok(state)
    }

    fn ensure_ready(&self) -> Result<(), CommandStackError> {
        if self.faulted {
            return Err(CommandStackError::Faulted);
        }
        Ok(())
    }

    fn fault_from_query<T>(
        &mut self,
        operation: CommandOperation,
        label: &str,
    ) -> Result<T, CommandStackError> {
        self.faulted = true;
        Err(CommandStackError::Panicked {
            operation,
            label: label.to_owned(),
        })
    }

    fn enforce_undo_limit(&mut self) {
        let Some(limit) = self.undo_limit else {
            return;
        };
        if self.undoable.len() > limit {
            let excess = self.undoable.len() - limit;
            self.undoable.drain(0..excess);
        }
    }
}
