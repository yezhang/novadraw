use std::convert::Infallible;

use novadraw::render::BuiltinFont;
use novadraw::{
    Figure, FlowPage, FlowTextPosition, FlowTextRange, Rectangle, RectangleFigure, RootFigure,
    TextAffinity, TextFlowFigure, TextMovement,
};
use novadraw_editor::{
    Command, CommandError, CommandStackError, DirectTextEdit, DirectTextEditDescriptor,
    DirectTextEditError, DirectTextEditRequest, DirectTextEditState, DirectTextFeature,
    DirectTextFeedback, EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorDomain,
    EditorDomainError, ExtendTextSelection, FeedbackVisual, FocusLossPolicy, GraphicalViewer,
    ModelAdapter, ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHost,
    PolicyInstallation, PolicyRole, SessionTextInputEvent, TextDelete, TextEditMode,
    TextInputEffect, TextInputEvent, TextInputPurpose, ViewerError,
};

const ROOT: ModelId = ModelId(1);
const TEXT: ModelId = ModelId(2);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ModelId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Changed,
}

struct TextModel {
    revision: ModelRevision,
    text: String,
    children: Vec<ModelId>,
    events: Vec<ModelEvent<ModelId, Event>>,
}

impl TextModel {
    fn new(text: &str) -> Self {
        Self {
            revision: ModelRevision::initial(),
            text: text.to_owned(),
            children: vec![TEXT],
            events: Vec::new(),
        }
    }

    fn publish(&mut self, subject: ModelId) {
        self.revision = self.revision.next().unwrap();
        self.events
            .push(ModelEvent::new(self.revision, subject, Event::Changed));
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
        self.publish(TEXT);
    }

    fn remove_text(&mut self) {
        self.children.clear();
        self.publish(ROOT);
    }
}

impl ModelAdapter for TextModel {
    type ModelId = ModelId;
    type Event = Event;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        ROOT
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == ROOT {
            self.children.clone()
        } else {
            Vec::new()
        })
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct SetTextCommand {
    before: String,
    after: String,
}

impl Command<TextModel> for SetTextCommand {
    fn label(&self) -> &str {
        "Set text"
    }

    fn can_execute(&self, _model: &TextModel) -> bool {
        self.after != "reject"
    }

    fn execute(&mut self, model: &mut TextModel) -> Result<(), CommandError> {
        model.set_text(self.after.clone());
        Ok(())
    }

    fn undo(&mut self, model: &mut TextModel) -> Result<(), CommandError> {
        model.set_text(self.before.clone());
        Ok(())
    }
}

struct TextEditPlan {
    descriptor: DirectTextEditDescriptor,
}

impl DirectTextEdit<TextModel> for TextEditPlan {
    fn descriptor(&self) -> &DirectTextEditDescriptor {
        &self.descriptor
    }

    fn feedback(
        &mut self,
        state: &DirectTextEditState,
        _model: &TextModel,
    ) -> Result<DirectTextFeedback, PolicyError> {
        DirectTextFeedback::new(
            vec![FeedbackVisual::scaled(Box::new(TextFlowFigure::new(
                Rectangle::new(40.0, 40.0, 240.0, 120.0),
                FlowPage::from_text(state.draft()),
            )))],
            0,
        )
    }

    fn command(
        &mut self,
        state: &DirectTextEditState,
        model: &TextModel,
    ) -> Result<Box<dyn Command<TextModel>>, PolicyError> {
        Ok(Box::new(SetTextCommand {
            before: model.text.clone(),
            after: state.draft().to_owned(),
        }))
    }
}

struct TextPolicy;

impl EditPolicy<TextModel> for TextPolicy {
    fn understands(&self, _request: &novadraw_editor::EditorRequest) -> bool {
        false
    }

    fn command(
        &mut self,
        _host: PolicyHost<ModelId>,
        _request: &novadraw_editor::EditorRequest,
        _model: &TextModel,
    ) -> Result<Option<Box<dyn Command<TextModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_direct_text_edit(
        &mut self,
        host: PolicyHost<ModelId>,
        request: &DirectTextEditRequest,
        model: &TextModel,
    ) -> Result<Option<Box<dyn DirectTextEdit<TextModel>>>, PolicyError> {
        if host.model() != TEXT || request.feature().as_str() != "label" {
            return Ok(None);
        }
        Ok(Some(Box::new(TextEditPlan {
            descriptor: DirectTextEditDescriptor::new(
                request.feature().clone(),
                model.text.clone(),
                model.revision(),
                TextEditMode::Multiline,
                FocusLossPolicy::Cancel,
            ),
        })))
    }
}

struct TextPart;

impl EditPartBehavior<TextModel> for TextPart {
    fn create_figure(
        &mut self,
        _model: &TextModel,
        model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if model_id == ROOT {
            Box::new(RootFigure::new(0.0, 0.0, 480.0, 320.0))
        } else {
            Box::new(RectangleFigure::new(40.0, 40.0, 240.0, 120.0))
        })
    }

    fn create_policies(
        &mut self,
        _model: &TextModel,
        model_id: ModelId,
    ) -> Result<Vec<PolicyInstallation<TextModel>>, EditPartError> {
        Ok(if model_id == TEXT {
            vec![(PolicyRole::DirectTextEdit, Box::new(TextPolicy))]
        } else {
            Vec::new()
        })
    }
}

struct TextFactory;

impl EditPartFactory<TextModel> for TextFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<ModelId>,
        _model: &TextModel,
    ) -> Result<Box<dyn EditPartBehavior<TextModel>>, EditPartError> {
        Ok(Box::new(TextPart))
    }
}

fn feature() -> DirectTextFeature {
    DirectTextFeature::new("label").unwrap()
}

fn viewer(text: &str) -> GraphicalViewer<TextModel, TextFactory> {
    let mut viewer = GraphicalViewer::new(
        TextModel::new(text),
        TextFactory,
        Rectangle::new(0.0, 0.0, 480.0, 320.0),
    )
    .unwrap();
    viewer
        .runtime_mut()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    viewer
        .runtime_mut()
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    viewer
}

#[test]
fn draft_is_transient_and_accept_creates_one_undoable_command() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    viewer.replace_selection(source).unwrap();
    let viewer_selection = viewer.selection().items().to_vec();
    let mut domain = EditorDomain::new();

    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    assert_eq!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .len(),
        1
    );
    domain.insert_direct_text(&mut viewer, " beta").unwrap();

    assert_eq!(viewer.model().text, "alpha");
    assert_eq!(domain.command_stack().undo_len(), 0);
    assert_eq!(viewer.selection().items(), viewer_selection);
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "alpha beta");
    assert!(viewer.direct_text_caret_geometry().is_ok());
    assert!(viewer.direct_text_selection_geometry().is_ok());

    assert!(domain.accept_direct_text_edit(&mut viewer).unwrap());
    assert_eq!(viewer.model().text, "alpha beta");
    assert!(viewer.direct_text_edit().is_none());
    assert_eq!(domain.command_stack().undo_len(), 1);
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );

    domain.undo(&mut viewer).unwrap();
    assert_eq!(viewer.model().text, "alpha");
    domain.redo(&mut viewer).unwrap();
    assert_eq!(viewer.model().text, "alpha beta");
}

#[test]
fn interaction_map_moves_and_deletes_whole_grapheme_clusters() {
    let mut viewer = viewer("a👩‍💻z");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    let caret = FlowTextPosition::new(0, 1, TextAffinity::Downstream);
    domain
        .set_direct_text_selection(&mut viewer, FlowTextRange::new(caret, caret))
        .unwrap();

    let moved = domain
        .move_direct_text(
            &mut viewer,
            TextMovement::NextVisual,
            ExtendTextSelection::No,
        )
        .unwrap();
    assert_eq!(moved.byte_offset(), "a👩‍💻".len());
    assert_eq!(
        domain
            .move_direct_text(
                &mut viewer,
                TextMovement::PreviousVisual,
                ExtendTextSelection::No,
            )
            .unwrap()
            .byte_offset(),
        1
    );
    domain
        .move_direct_text(
            &mut viewer,
            TextMovement::NextVisual,
            ExtendTextSelection::No,
        )
        .unwrap();
    domain
        .delete_direct_text(&mut viewer, TextDelete::PreviousVisual)
        .unwrap();

    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "az");
    assert_eq!(viewer.model().text, "a👩‍💻z");
}

#[test]
fn one_session_unsupported_feature_cancel_and_no_change_are_deterministic() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();

    assert!(matches!(
        domain.start_direct_text_edit(&mut viewer, source, feature()),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::SessionAlreadyActive
        )))
    ));
    assert!(domain.cancel_direct_text_edit(&mut viewer).unwrap());
    assert_eq!(domain.command_stack().undo_len(), 0);

    assert!(matches!(
        domain.start_direct_text_edit(
            &mut viewer,
            source,
            DirectTextFeature::new("missing").unwrap()
        ),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::UnsupportedFeature
        )))
    ));

    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    assert!(!domain.accept_direct_text_edit(&mut viewer).unwrap());
    assert_eq!(domain.command_stack().undo_len(), 0);
}

#[test]
fn stale_revision_and_recoverable_command_rejection_preserve_the_draft() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    domain
        .set_direct_text_selection(
            &mut viewer,
            FlowTextRange::new(
                FlowTextPosition::new(0, 0, TextAffinity::Downstream),
                FlowTextPosition::new(0, "alpha".len(), TextAffinity::Upstream),
            ),
        )
        .unwrap();
    domain.insert_direct_text(&mut viewer, "reject").unwrap();

    assert!(matches!(
        domain.accept_direct_text_edit(&mut viewer),
        Err(EditorDomainError::Command(
            CommandStackError::Rejected { .. }
        ))
    ));
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "reject");

    viewer.model_mut().unwrap().set_text("external".to_owned());
    assert!(matches!(
        domain.accept_direct_text_edit(&mut viewer),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::StaleSourceRevision
        )))
    ));
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "reject");
    assert_eq!(viewer.model().text, "external");
}

#[test]
fn source_retirement_cancels_session_and_removes_owned_feedback() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();

    viewer.model_mut().unwrap().remove_text();
    viewer.refresh().unwrap();

    assert!(viewer.direct_text_edit().is_none());
    assert!(viewer.part_for_model(TEXT).is_none());
    assert!(
        viewer
            .runtime()
            .tree()
            .child_order(viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn preedit_updates_are_transient_and_cancel_restores_the_composition_base() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    let session = viewer.direct_text_edit().unwrap().session();
    assert!(matches!(
        viewer.take_text_input_effects().as_slice(),
        [TextInputEffect::Acquire {
            session: acquired,
            purpose: TextInputPurpose::Multiline,
            ..
        }] if *acquired == session
    ));
    domain
        .set_direct_text_selection(
            &mut viewer,
            FlowTextRange::new(
                FlowTextPosition::new(0, 0, TextAffinity::Downstream),
                FlowTextPosition::new(0, "alpha".len(), TextAffinity::Upstream),
            ),
        )
        .unwrap();

    domain
        .handle_text_input_event(
            &mut viewer,
            SessionTextInputEvent::new(
                session,
                TextInputEvent::Preedit {
                    text: "ni".to_owned(),
                    selection: Some(2..2),
                },
            ),
        )
        .unwrap();
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "ni");
    domain
        .handle_text_input_event(
            &mut viewer,
            SessionTextInputEvent::new(
                session,
                TextInputEvent::Preedit {
                    text: "你".to_owned(),
                    selection: None,
                },
            ),
        )
        .unwrap();
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "你");
    assert!(
        viewer
            .direct_text_edit()
            .unwrap()
            .composition()
            .unwrap()
            .selection()
            .is_none()
    );
    assert!(domain.cancel_direct_text_preedit(&mut viewer).unwrap());
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "alpha");

    assert!(matches!(
        domain.set_direct_text_preedit(&mut viewer, "你", Some(1..2)),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::InvalidPreeditRange
        )))
    ));
    domain
        .set_direct_text_preedit(&mut viewer, "你", Some(3..3))
        .unwrap();
    assert!(matches!(
        domain.accept_direct_text_edit(&mut viewer),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::ActiveComposition
        )))
    ));
    domain.insert_direct_text(&mut viewer, "你").unwrap();
    assert!(viewer.direct_text_edit().unwrap().composition().is_none());
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "你");
    assert_eq!(viewer.model().text, "alpha");

    domain.cancel_direct_text_edit(&mut viewer).unwrap();
    assert!(matches!(
        viewer.take_text_input_effects().last(),
        Some(TextInputEffect::Release { session: released }) if *released == session
    ));
    assert!(matches!(
        domain.handle_text_input_event(
            &mut viewer,
            SessionTextInputEvent::new(session, TextInputEvent::InsertText("late".to_owned()))
        ),
        Err(EditorDomainError::Viewer(ViewerError::DirectTextEdit(
            DirectTextEditError::TextInputLeaseLost
        )))
    ));
}
