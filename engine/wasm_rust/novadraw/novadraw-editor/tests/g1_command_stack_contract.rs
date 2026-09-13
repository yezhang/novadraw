use novadraw_editor::{
    Command, CommandError, CommandOperation, CommandStack, CommandStackError,
    CommandStackEventKind, CompoundCommand,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Counter {
    value: i32,
}

struct AddCommand {
    label: &'static str,
    amount: i32,
    executable: bool,
    undoable: bool,
    redoable: bool,
    fail_execute: bool,
    fail_undo: bool,
    fail_redo: bool,
    panic_execute: bool,
    panic_can_execute: bool,
    state_unknown: bool,
    dropped: Option<Arc<AtomicUsize>>,
}

impl AddCommand {
    fn new(label: &'static str, amount: i32) -> Self {
        Self {
            label,
            amount,
            executable: true,
            undoable: true,
            redoable: true,
            fail_execute: false,
            fail_undo: false,
            fail_redo: false,
            panic_execute: false,
            panic_can_execute: false,
            state_unknown: false,
            dropped: None,
        }
    }

    fn rejected(mut self) -> Self {
        self.executable = false;
        self
    }

    fn failing_execute(mut self) -> Self {
        self.fail_execute = true;
        self
    }

    fn failing_undo(mut self) -> Self {
        self.fail_undo = true;
        self
    }

    fn not_undoable(mut self) -> Self {
        self.undoable = false;
        self
    }

    fn failing_redo(mut self) -> Self {
        self.fail_redo = true;
        self
    }

    fn not_redoable(mut self) -> Self {
        self.redoable = false;
        self
    }

    fn panicking_execute(mut self) -> Self {
        self.panic_execute = true;
        self
    }

    fn panicking_can_execute(mut self) -> Self {
        self.panic_can_execute = true;
        self
    }

    fn leaving_state_unknown(mut self) -> Self {
        self.state_unknown = true;
        self
    }

    fn tracking_drop(mut self, dropped: Arc<AtomicUsize>) -> Self {
        self.dropped = Some(dropped);
        self
    }
}

impl Drop for AddCommand {
    fn drop(&mut self) {
        if let Some(dropped) = &self.dropped {
            dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl Command<Counter> for AddCommand {
    fn label(&self) -> &str {
        self.label
    }

    fn can_execute(&self, _model: &Counter) -> bool {
        assert!(!self.panic_can_execute, "can_execute panic");
        self.executable
    }

    fn execute(&mut self, model: &mut Counter) -> Result<(), CommandError> {
        assert!(!self.panic_execute, "execute panic");
        if self.fail_execute {
            return Err(CommandError::operation("execute failed"));
        }
        if self.state_unknown {
            model.value += self.amount;
            return Err(CommandError::state_unknown("model state is unknown"));
        }
        model.value += self.amount;
        Ok(())
    }

    fn undo(&mut self, model: &mut Counter) -> Result<(), CommandError> {
        if self.fail_undo {
            return Err(CommandError::operation("undo failed"));
        }
        model.value -= self.amount;
        Ok(())
    }

    fn can_undo(&self) -> bool {
        self.undoable
    }

    fn redo(&mut self, model: &mut Counter) -> Result<(), CommandError> {
        if self.fail_redo {
            return Err(CommandError::operation("redo failed"));
        }
        self.execute(model)
    }

    fn can_redo(&self) -> bool {
        self.redoable
    }
}

#[test]
fn command_stack_executes_undoes_redoes_and_reports_stable_events() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    stack
        .execute(&mut model, Box::new(AddCommand::new("add two", 2)))
        .unwrap();
    assert_eq!(model.value, 2);
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
    assert!(stack.is_dirty());

    stack.undo(&mut model).unwrap();
    assert_eq!(model.value, 0);
    assert!(stack.can_redo());

    stack.redo(&mut model).unwrap();
    assert_eq!(model.value, 2);

    let kinds: Vec<_> = stack
        .take_events()
        .into_iter()
        .map(|event| event.kind())
        .collect();
    assert_eq!(
        kinds,
        vec![
            CommandStackEventKind::Executed,
            CommandStackEventKind::Undone,
            CommandStackEventKind::Redone,
        ]
    );
}

#[test]
fn rejected_or_failed_execute_does_not_change_history_or_clear_redo() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    stack
        .execute(&mut model, Box::new(AddCommand::new("first", 1)))
        .unwrap();
    stack.undo(&mut model).unwrap();

    assert!(matches!(
        stack.execute(
            &mut model,
            Box::new(AddCommand::new("rejected", 10).rejected())
        ),
        Err(CommandStackError::Rejected { .. })
    ));
    assert!(stack.can_redo());

    assert!(matches!(
        stack.execute(
            &mut model,
            Box::new(AddCommand::new("failed", 10).failing_execute())
        ),
        Err(CommandStackError::Command(_))
    ));
    assert_eq!(model.value, 0);
    assert!(stack.can_redo());
}

#[test]
fn dirty_state_tracks_history_identity_across_branching() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    stack
        .execute(&mut model, Box::new(AddCommand::new("first", 1)))
        .unwrap();
    stack.mark_save_location().unwrap();
    assert!(!stack.is_dirty());

    stack
        .execute(&mut model, Box::new(AddCommand::new("second", 2)))
        .unwrap();
    assert!(stack.is_dirty());
    stack.undo(&mut model).unwrap();
    assert!(!stack.is_dirty());

    stack
        .execute(&mut model, Box::new(AddCommand::new("branch", 3)))
        .unwrap();
    assert!(stack.is_dirty());
    assert!(!stack.can_redo());
}

#[test]
fn failed_undo_preserves_the_undo_entry_and_model_state() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();
    stack
        .execute(
            &mut model,
            Box::new(AddCommand::new("cannot undo", 4).failing_undo()),
        )
        .unwrap();

    assert!(matches!(
        stack.undo(&mut model),
        Err(CommandStackError::Command(_))
    ));
    assert_eq!(model.value, 4);
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
}

#[test]
fn failed_redo_preserves_the_redo_entry_and_model_state() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();
    stack
        .execute(
            &mut model,
            Box::new(AddCommand::new("cannot redo", 4).failing_redo()),
        )
        .unwrap();
    stack.undo(&mut model).unwrap();

    assert!(matches!(
        stack.redo(&mut model),
        Err(CommandStackError::Command(_))
    ));
    assert_eq!(model.value, 0);
    assert!(!stack.can_undo());
    assert!(stack.can_redo());
}

#[test]
fn command_capabilities_reject_undo_and_redo_without_moving_history() {
    let mut model = Counter::default();
    let mut undo_stack = CommandStack::new();
    undo_stack
        .execute(
            &mut model,
            Box::new(AddCommand::new("fixed", 4).not_undoable()),
        )
        .unwrap();
    assert!(!undo_stack.can_undo());
    assert!(matches!(
        undo_stack.undo(&mut model),
        Err(CommandStackError::Rejected {
            operation: CommandOperation::Undo,
            ..
        })
    ));
    assert_eq!(model.value, 4);
    assert_eq!(undo_stack.undo_len(), 1);

    let mut redo_stack = CommandStack::new();
    redo_stack
        .execute(
            &mut model,
            Box::new(AddCommand::new("one way", 2).not_redoable()),
        )
        .unwrap();
    redo_stack.undo(&mut model).unwrap();
    assert!(!redo_stack.can_redo());
    assert!(matches!(
        redo_stack.redo(&mut model),
        Err(CommandStackError::Rejected {
            operation: CommandOperation::Redo,
            ..
        })
    ));
    assert_eq!(redo_stack.redo_len(), 1);
}

#[test]
fn compound_command_rolls_back_executed_prefix_and_undoes_in_reverse_order() {
    let mut model = Counter::default();
    let mut failing = CompoundCommand::new("failing compound");
    failing.push(Box::new(AddCommand::new("first", 1)));
    failing.push(Box::new(AddCommand::new("second", 2).failing_execute()));

    let mut stack = CommandStack::new();
    assert!(matches!(
        stack.execute(&mut model, Box::new(failing)),
        Err(CommandStackError::Command(_))
    ));
    assert_eq!(model.value, 0);
    assert!(!stack.can_undo());

    let mut successful = CompoundCommand::new("successful compound");
    successful.push(Box::new(AddCommand::new("first", 1)));
    successful.push(Box::new(AddCommand::new("second", 2)));
    stack.execute(&mut model, Box::new(successful)).unwrap();
    assert_eq!(model.value, 3);
    stack.undo(&mut model).unwrap();
    assert_eq!(model.value, 0);
}

#[test]
fn compound_undo_failure_restores_the_executed_prefix() {
    let mut model = Counter::default();
    let mut compound = CompoundCommand::new("recoverable undo");
    compound.push(Box::new(AddCommand::new("first", 1).failing_undo()));
    compound.push(Box::new(AddCommand::new("second", 2)));

    let mut stack = CommandStack::new();
    stack.execute(&mut model, Box::new(compound)).unwrap();
    assert!(matches!(
        stack.undo(&mut model),
        Err(CommandStackError::Command(_))
    ));
    assert_eq!(model.value, 3);
    assert!(stack.can_undo());
    assert!(!stack.is_faulted());
}

#[test]
fn failed_compound_compensation_faults_the_stack() {
    let mut model = Counter::default();
    let mut compound = CompoundCommand::new("broken compensation");
    compound.push(Box::new(AddCommand::new("first", 1).failing_undo()));
    compound.push(Box::new(AddCommand::new("second", 2).failing_execute()));

    let mut stack = CommandStack::new();
    assert!(matches!(
        stack.execute(&mut model, Box::new(compound)),
        Err(CommandStackError::Command(ref error)) if !error.is_recoverable()
    ));
    assert!(stack.is_faulted());
    assert!(stack.is_dirty());
}

#[test]
fn undo_limit_discards_oldest_history_without_changing_the_model() {
    let mut model = Counter::default();
    let mut stack = CommandStack::with_undo_limit(2);

    for amount in [1, 2, 3] {
        stack
            .execute(&mut model, Box::new(AddCommand::new("add", amount)))
            .unwrap();
    }
    assert_eq!(model.value, 6);
    assert_eq!(stack.undo_len(), 2);

    stack.undo(&mut model).unwrap();
    stack.undo(&mut model).unwrap();
    assert_eq!(model.value, 1);
    assert!(matches!(
        stack.undo(&mut model),
        Err(CommandStackError::NothingToUndo)
    ));
}

#[test]
fn flush_drops_history_and_marks_the_current_document_clean() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut model = Counter::default();
    let mut stack = CommandStack::new();
    stack
        .execute(
            &mut model,
            Box::new(AddCommand::new("tracked", 1).tracking_drop(Arc::clone(&dropped))),
        )
        .unwrap();

    stack.flush().unwrap();
    assert_eq!(model.value, 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(!stack.can_undo());
    assert!(!stack.can_redo());
    assert!(!stack.is_dirty());
}

#[test]
fn command_panic_faults_the_stack_and_blocks_later_operations() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    assert!(matches!(
        stack.execute(
            &mut model,
            Box::new(AddCommand::new("panic", 1).panicking_execute())
        ),
        Err(CommandStackError::Panicked {
            operation: CommandOperation::Execute,
            ..
        })
    ));
    assert!(stack.is_faulted());
    assert!(stack.is_dirty());
    assert!(matches!(
        stack.execute(&mut model, Box::new(AddCommand::new("later", 1))),
        Err(CommandStackError::Faulted)
    ));
    assert!(matches!(
        stack.mark_save_location(),
        Err(CommandStackError::Faulted)
    ));
    assert!(matches!(stack.flush(), Err(CommandStackError::Faulted)));
}

#[test]
fn explicit_unknown_state_failure_faults_the_stack() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    assert!(matches!(
        stack.execute(
            &mut model,
            Box::new(AddCommand::new("unknown", 3).leaving_state_unknown())
        ),
        Err(CommandStackError::Command(ref error)) if !error.is_recoverable()
    ));
    assert_eq!(model.value, 3);
    assert!(stack.is_faulted());
    assert!(stack.is_dirty());
}

#[test]
fn command_query_panic_faults_the_stack_before_model_mutation() {
    let mut model = Counter::default();
    let mut stack = CommandStack::new();

    assert!(matches!(
        stack.execute(
            &mut model,
            Box::new(AddCommand::new("query panic", 3).panicking_can_execute())
        ),
        Err(CommandStackError::Panicked {
            operation: CommandOperation::Execute,
            ..
        })
    ));
    assert_eq!(model.value, 0);
    assert!(stack.is_faulted());
}
