use novadraw::figure::{Figure, RootFigure};
use novadraw::geometry::Rectangle;
use novadraw_editor::{
    EditPartBehavior, EditPartError, EditPartFactory, EditorDomain, EditorDomainError,
    InteractionRevision, PartFactoryContext, RequestKey, RequestMetadata, RequestProtocol,
    command::{
        Command, CommandError, CommandErrorKind, CommandOperation, CommandStack, CommandStackError,
        CompoundCommand,
    },
    diagnostics::PanicPayloadKind,
    model::{ModelAdapter, ModelEvent, ModelRevision},
    policy::{
        EditPolicy, PolicyError, PolicyHandlerBuilder, PolicyHost, PolicyInstallation, PolicySlot,
    },
    tool::{Tool, ToolContext, ToolError, ToolKey},
    viewer::{GraphicalViewer, ViewerError},
};
use std::{
    cell::Cell,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    rc::Rc,
};

// Deliberately neither Clone nor Send/Sync.
#[derive(Debug)]
struct Leaf {
    code: u32,
    alive: Rc<Cell<usize>>,
}
impl fmt::Display for Leaf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "leaf {}", self.code)
    }
}
impl Error for Leaf {}
impl Drop for Leaf {
    fn drop(&mut self) {
        self.alive.set(self.alive.get() - 1);
    }
}
#[derive(Debug)]
struct Layer(Leaf);
impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("application layer")
    }
}
impl Error for Layer {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
fn cause(code: u32) -> Layer {
    Layer(Leaf {
        code,
        alive: Rc::new(Cell::new(1)),
    })
}
fn leaf<'a>(error: &'a (dyn Error + 'static)) -> &'a Leaf {
    let mut cursor = error;
    loop {
        if let Some(found) = cursor.downcast_ref::<Leaf>() {
            return found;
        }
        cursor = cursor
            .source()
            .expect("original leaf must remain in the source chain");
    }
}

struct Reject;
impl Command<()> for Reject {
    fn label(&self) -> &str {
        "reject"
    }
    fn execute(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Err(CommandError::operation_with_source(
            "display context",
            cause(17),
        ))
    }
    fn undo(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Ok(())
    }
}

#[test]
fn command_source_survives_stack_domain_and_clone_without_error_clone_or_thread_bounds() {
    let mut stack = CommandStack::new();
    let error: EditorDomainError = stack.execute(&mut (), Box::new(Reject)).unwrap_err().into();
    assert_eq!(leaf(&error).code, 17);
    let EditorDomainError::Command(CommandStackError::Command(command)) = error else {
        panic!("command category")
    };
    assert_eq!(command.kind(), CommandErrorKind::Operation);
    assert_eq!(command.failed_operation(), Some(CommandOperation::Execute));
    let alive = Rc::clone(&leaf(&command).alive);
    let copy = command.clone();
    drop(command);
    assert_eq!(alive.get(), 1);
    assert_eq!(leaf(&copy).code, 17);
    drop(copy);
    assert_eq!(alive.get(), 0);
    assert!(!stack.is_faulted());
    assert_eq!(stack.undo_len(), 0);
    assert!(stack.take_events().is_empty());
}

struct Compensation;
impl Command<()> for Compensation {
    fn label(&self) -> &str {
        "compensate"
    }
    fn execute(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Ok(())
    }
    fn undo(&mut self, _: &mut ()) -> Result<(), CommandError> {
        Err(CommandError::operation_with_source("rollback", cause(23)))
    }
}

#[test]
fn failed_compensation_retains_both_causes_and_faults_without_history_publication() {
    let mut compound = CompoundCommand::new("compound");
    compound.push(Box::new(Compensation));
    compound.push(Box::new(Reject));
    let mut stack = CommandStack::new();
    let CommandStackError::Command(error) = stack.execute(&mut (), Box::new(compound)).unwrap_err()
    else {
        panic!("command category")
    };
    assert_eq!(error.kind(), CommandErrorKind::Inconsistent);
    assert_eq!(leaf(&error).code, 17);
    assert_eq!(leaf(error.compensation_error().unwrap()).code, 23);
    assert!(stack.is_faulted());
    assert_eq!(stack.undo_len(), 0);
    assert!(stack.take_events().is_empty());
}

struct SendOpaque;
impl fmt::Debug for SendOpaque {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("must not format payload")
    }
}
impl fmt::Display for SendOpaque {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("must not format payload")
    }
}

struct Panics {
    phase: &'static str,
    text: bool,
}
impl Panics {
    fn at(&self, phase: &'static str) {
        if self.phase == phase {
            if self.text {
                panic_any(String::from("original panic text"));
            }
            panic_any(SendOpaque);
        }
    }
}
impl Command<()> for Panics {
    fn label(&self) -> &str {
        self.at("label");
        "panic command"
    }
    fn can_execute(&self, _: &()) -> bool {
        self.at("can_execute");
        true
    }
    fn execute(&mut self, _: &mut ()) -> Result<(), CommandError> {
        self.at("execute");
        Ok(())
    }
    fn can_undo(&self) -> bool {
        self.at("can_undo");
        true
    }
    fn undo(&mut self, _: &mut ()) -> Result<(), CommandError> {
        self.at("undo");
        Ok(())
    }
    fn can_redo(&self) -> bool {
        self.at("can_redo");
        true
    }
    fn redo(&mut self, _: &mut ()) -> Result<(), CommandError> {
        self.at("redo");
        Ok(())
    }
}

#[test]
fn all_command_panic_boundaries_retain_text_or_nontext_classification_and_committed_prefix() {
    for phase in [
        "label",
        "can_execute",
        "execute",
        "can_undo",
        "undo",
        "can_redo",
        "redo",
    ] {
        for text in [false, true] {
            let mut stack = CommandStack::new();
            let command = Box::new(Panics { phase, text });
            let error = match phase {
                "label" | "can_execute" | "execute" => stack.execute(&mut (), command).unwrap_err(),
                "can_undo" | "undo" => {
                    stack.execute(&mut (), command).unwrap();
                    stack.take_events();
                    stack.undo(&mut ()).unwrap_err()
                }
                _ => {
                    stack.execute(&mut (), command).unwrap();
                    stack.undo(&mut ()).unwrap();
                    stack.take_events();
                    stack.redo(&mut ()).unwrap_err()
                }
            };
            assert!(error.source().is_none());
            let CommandStackError::Panicked {
                diagnostic,
                operation,
                ..
            } = error
            else {
                panic!("panic category")
            };
            assert_eq!(diagnostic.location(), format!("command.{phase}"));
            assert_eq!(diagnostic.message(), text.then_some("original panic text"));
            assert_eq!(
                diagnostic.kind(),
                if text {
                    PanicPayloadKind::Text
                } else {
                    PanicPayloadKind::NonText
                }
            );
            assert!(diagnostic.call_site().file().ends_with("command/mod.rs"));
            assert!(stack.is_faulted());
            assert!(stack.take_events().is_empty());
            match operation {
                CommandOperation::Execute => assert_eq!(stack.undo_len(), 0),
                CommandOperation::Undo => assert_eq!(stack.undo_len(), 1),
                CommandOperation::Redo => assert_eq!(stack.redo_len(), 1),
            }
        }
    }
}

#[derive(Default)]
struct Model {
    fail_query: bool,
}
impl ModelAdapter for Model {
    type ModelId = u8;
    type Event = ();
    type Error = Layer;
    fn root(&self) -> u8 {
        1
    }
    fn revision(&self) -> ModelRevision {
        ModelRevision::initial()
    }
    fn children(&self, _: u8) -> Result<Vec<u8>, Layer> {
        if self.fail_query {
            Err(cause(31))
        } else {
            Ok(Vec::new())
        }
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u8, ()>> {
        Vec::new()
    }
}
struct Request;
impl RequestProtocol<Model> for Request {
    type Payload = u8;
    type Contribution = ();
    fn key() -> RequestKey<Self> {
        RequestKey::new("external.failure")
    }
}
struct Slot;
impl PolicySlot for Slot {
    const NAME: &'static str = "external.failure.slot";
}
struct Policy;
impl EditPolicy<Model> for Policy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle_routed::<Request>(Self::target, Self::contribute);
    }
}
impl Policy {
    fn target(
        _: &mut Self,
        host: PolicyHost<u8>,
        _: &RequestMetadata,
        payload: &u8,
        _: &Model,
    ) -> Result<Option<novadraw_editor::EditPartId>, PolicyError> {
        if *payload == 1 {
            return Err(PolicyError::operation_with_source(
                "policy target",
                cause(41),
            ));
        }
        Ok(Some(host.part()))
    }
    fn contribute(
        _: &mut Self,
        _: PolicyHost<u8>,
        _: &RequestMetadata,
        payload: &u8,
        _: &Model,
    ) -> Result<Option<()>, PolicyError> {
        if *payload == 2 {
            panic_any("static policy panic");
        }
        if *payload == 3 {
            panic_any(SendOpaque);
        }
        Err(PolicyError::operation_with_source(
            "policy contribution",
            cause(43),
        ))
    }
}
struct Part;
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, _: &Model, _: u8) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(RootFigure::new(0.0, 0.0, 100.0, 100.0).unwrap()))
    }
    fn create_policies(
        &mut self,
        _: &Model,
        _: u8,
    ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        Ok(vec![PolicyInstallation::new(Slot::key(), Policy).map_err(
            |e| EditPartError::operation_with_source("install", e),
        )?])
    }
}
struct Factory;
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u8>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part))
    }
}
fn viewer() -> GraphicalViewer<Model, Factory> {
    GraphicalViewer::new(
        Model::default(),
        Factory,
        Rectangle::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap()
}

#[test]
fn external_policy_source_survives_real_registry_viewer_tool_domain_and_clone() {
    for (payload, operation, code) in [(0, "policy.contribute", 43), (1, "policy.target", 41)] {
        let mut viewer = viewer();
        let metadata =
            RequestMetadata::new(InteractionRevision::initial(), vec![viewer.contents()]);
        let error = viewer
            .dispatch_request::<Request>(&metadata, &payload)
            .err()
            .unwrap();
        let ViewerError::Policy(policy) = error else {
            panic!("policy category")
        };
        assert_eq!(policy.failed_operation(), Some(operation));
        let copy = policy.clone();
        drop(policy);
        let domain: EditorDomainError = ToolError::from(ViewerError::Policy(copy)).into();
        assert_eq!(leaf(&domain).code, code);
        assert!(viewer.panic_diagnostic().is_none());
        viewer.synchronize().unwrap();
    }
}

#[test]
fn external_policy_panics_preserve_payload_and_fault_viewer_without_formatting() {
    for payload in [2, 3] {
        let mut viewer = viewer();
        let metadata =
            RequestMetadata::new(InteractionRevision::initial(), vec![viewer.contents()]);
        assert!(
            catch_unwind(AssertUnwindSafe(
                || viewer.dispatch_request::<Request>(&metadata, &payload)
            ))
            .is_err()
        );
        let diagnostic = viewer.panic_diagnostic().unwrap();
        assert_eq!(diagnostic.location(), "viewer.extension");
        assert_eq!(
            diagnostic.message(),
            if payload == 2 {
                Some("static policy panic")
            } else {
                None
            }
        );
        assert_eq!(
            diagnostic.kind(),
            if payload == 2 {
                PanicPayloadKind::Text
            } else {
                PanicPayloadKind::NonText
            }
        );
        assert!(matches!(viewer.synchronize(), Err(ViewerError::Faulted)));
    }
}

struct ExternalTool(bool);
impl Tool<Model> for ExternalTool {
    fn key(&self) -> novadraw_editor::tool::ErasedToolKey {
        ToolKey::<Self>::new("external").erase()
    }
    fn activate(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        if self.0 {
            panic_any(SendOpaque);
        }
        Err(ToolError::protocol_with_source("tool refusal", cause(59)))
    }
    fn cancel(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        Ok(())
    }
    fn mouse_pressed(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: novadraw_editor::tool::ToolMouseEvent,
    ) -> Result<
        (
            novadraw_editor::tool::ToolDirective,
            novadraw_editor::viewer::ViewerInputOutcome,
        ),
        ToolError,
    > {
        Err(ToolError::protocol("inactive test tool"))
    }
    fn mouse_moved(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: novadraw_editor::tool::ToolMouseEvent,
    ) -> Result<
        (
            novadraw_editor::tool::ToolDirective,
            novadraw::event::DispatchOutcome,
        ),
        ToolError,
    > {
        Err(ToolError::protocol("inactive test tool"))
    }
    fn mouse_released(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: novadraw_editor::tool::ToolMouseEvent,
    ) -> Result<
        (
            novadraw_editor::tool::ToolDirective,
            novadraw::event::DispatchOutcome,
        ),
        ToolError,
    > {
        Err(ToolError::protocol("inactive test tool"))
    }
}

#[test]
fn external_tool_panic_is_retained_at_activation_boundary_and_rethrown() {
    let mut viewer = viewer();
    let mut domain = EditorDomain::new();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || domain.set_active_tool(&mut viewer, ExternalTool(true))
        ))
        .is_err()
    );
    let diagnostic = viewer.panic_diagnostic().unwrap();
    assert_eq!(diagnostic.kind(), PanicPayloadKind::NonText);
    assert_eq!(diagnostic.location(), "tool.activate");
    assert!(matches!(viewer.synchronize(), Err(ViewerError::Faulted)));
}

#[test]
fn external_tool_rejection_keeps_source_operation_and_previous_tool_without_fault() {
    let mut viewer = viewer();
    let mut domain = EditorDomain::new();
    let previous = domain.active_tool_key();
    let error = domain
        .set_active_tool(&mut viewer, ExternalTool(false))
        .unwrap_err();
    assert_eq!(leaf(&error).code, 59);
    assert!(matches!(
        error,
        EditorDomainError::Tool(ToolError::Extension {
            operation: Some("tool.activate"),
            ..
        })
    ));
    assert_eq!(domain.active_tool_key(), previous);
    assert!(viewer.panic_diagnostic().is_none());
    viewer.synchronize().unwrap();
}

#[test]
fn adjacent_model_editpart_and_tool_errors_keep_owned_sources() {
    let error = GraphicalViewer::new(
        Model { fail_query: true },
        Factory,
        Rectangle::new(0.0, 0.0, 100.0, 100.0),
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        ViewerError::ModelSource {
            operation: "model.children",
            ..
        }
    ));
    assert_eq!(leaf(&error).code, 31);
    let error: ViewerError = EditPartError::operation_with_source("part", cause(47)).into();
    assert_eq!(leaf(&error).code, 47);
    let error: EditorDomainError = ToolError::protocol_with_source("tool", cause(53)).into();
    assert_eq!(leaf(&error).code, 53);
}
