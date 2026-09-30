use std::convert::Infallible;

use novadraw::render::{BuiltinFont, command::RenderCommandKind};
use novadraw::{
    Alignment, Color, Figure, FigureStyle, FlowPage, FlowTextPosition, FlowTextRange, FlowWrapping,
    Rectangle, RectangleFigure, RootFigure, TextAffinity, TextFlowFigure, TextMovement,
};
use novadraw_editor::{
    Command, CommandError, CommandStackError, DirectTextEdit, DirectTextEditDescriptor,
    DirectTextEditError, DirectTextEditRequest, DirectTextEditState, DirectTextFeature,
    DirectTextFeedback, EditPartBehavior, EditPartError, EditPartFactory, EditPolicy, EditorDomain,
    EditorDomainError, ExtendTextSelection, FeedbackVisual, FocusLossPolicy, GraphicalViewer,
    ModelAdapter, ModelEvent, ModelRevision, PartFactoryContext, PolicyError, PolicyHost,
    PolicyInstallation, PolicyRole, SessionTextInputEvent, TextDelete, TextEditMode,
    TextInputEffect, TextInputEvent, TextInputPurpose, TextInputSnapshot, ViewerError,
    VisualUpdateContext,
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
    bounds: Rectangle,
    children: Vec<ModelId>,
    events: Vec<ModelEvent<ModelId, Event>>,
}

impl TextModel {
    fn new(text: &str) -> Self {
        Self {
            revision: ModelRevision::initial(),
            text: text.to_owned(),
            bounds: Rectangle::new(40.0, 40.0, 240.0, 120.0),
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

    fn set_bounds(&mut self, bounds: Rectangle) {
        self.bounds = bounds;
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
        model: &TextModel,
    ) -> Result<DirectTextFeedback, PolicyError> {
        DirectTextFeedback::new(
            vec![
                FeedbackVisual::scaled(Box::new(
                    TextFlowFigure::new(model.bounds, FlowPage::from_text(state.draft()))
                        .with_wrapping(FlowWrapping::NoWrap)
                        .with_alignment(Alignment::Center, Alignment::Center),
                ))
                .with_style(FigureStyle {
                    foreground: Some(Color::WHITE),
                    font: Some("16px Inter Variable".to_owned()),
                    ..FigureStyle::default()
                }),
            ],
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
                TextEditMode::SingleLine,
                FocusLossPolicy::Cancel,
            ),
        })))
    }
}

struct TextPart;

impl EditPartBehavior<TextModel> for TextPart {
    fn create_figure(
        &mut self,
        model: &TextModel,
        model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if model_id == ROOT {
            Box::new(RootFigure::new(0.0, 0.0, 480.0, 320.0))
        } else {
            Box::new(RectangleFigure::from_bounds(model.bounds))
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &TextModel,
        model_id: ModelId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        if model_id == TEXT {
            context.set_primary_bounds(model.bounds)?;
        }
        Ok(())
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

fn direct_text_decoration_count(viewer: &GraphicalViewer<TextModel, TextFactory>) -> usize {
    viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().feedback())
        .unwrap()
        .len()
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
    assert_eq!(direct_text_decoration_count(&viewer), 1);
    let text_feedback = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().scaled_feedback())
        .unwrap()[0];
    assert_eq!(
        viewer.runtime().tree().figure_style(text_feedback),
        Some(&FigureStyle {
            foreground: Some(Color::WHITE),
            font: Some("16px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
    );
    let caret = viewer.direct_text_caret_geometry().unwrap().bounds();
    let caret_figure = *viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().feedback())
        .unwrap()
        .last()
        .unwrap();
    let caret_visual = viewer.runtime().tree().figure_bounds(caret_figure).unwrap();
    assert_eq!(caret_visual.x, caret.x);
    assert_eq!(caret_visual.y, caret.y);
    assert_eq!(caret_visual.height, caret.height);
    assert!(caret_visual.width >= 1.0);
    assert!(viewer.runtime().tree().is_visible(caret_figure));
    let frame = viewer.runtime_mut().prepare_frame().unwrap();
    assert!(frame.commands().iter().any(|command| matches!(
        command.kind,
        RenderCommandKind::FillRect { rect, color }
            if color == Color::rgba(0.07, 0.09, 0.12, 1.0)
                && rect.width >= 1.0
                && rect.height == caret.height
    )));
    let blink_off_at = viewer.next_wake_deadline().unwrap();
    assert!(viewer.advance_time(blink_off_at).unwrap());
    assert!(!viewer.runtime().tree().is_visible(caret_figure));
    let blink_on_at = viewer.next_wake_deadline().unwrap();
    assert!(viewer.advance_time(blink_on_at).unwrap());
    assert!(viewer.runtime().tree().is_visible(caret_figure));
    domain.insert_direct_text(&mut viewer, " beta").unwrap();

    assert_eq!(viewer.model().text, "alpha");
    assert_eq!(domain.command_stack().undo_len(), 0);
    assert_eq!(viewer.selection().items(), viewer_selection);
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "alpha beta");
    assert!(viewer.direct_text_caret_geometry().is_ok());
    assert!(viewer.direct_text_selection_geometry().is_ok());
    assert_eq!(direct_text_decoration_count(&viewer), 1);

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
    assert_eq!(direct_text_decoration_count(&viewer), 0);
    assert_eq!(viewer.next_wake_deadline(), None);

    domain.undo(&mut viewer).unwrap();
    assert_eq!(viewer.model().text, "alpha");
    domain.redo(&mut viewer).unwrap();
    assert_eq!(viewer.model().text, "alpha beta");
}

#[test]
fn long_single_line_draft_is_clipped_and_scrolls_to_reveal_the_caret() {
    let text = "long 直接编辑 👩‍💻 text ".repeat(8);
    let mut viewer = viewer(&text);
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    let text_feedback = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().scaled_feedback())
        .unwrap()[0];
    let layout = viewer.runtime().text_flow_layout(text_feedback).unwrap();

    assert!(!layout.is_truncated());
    assert_eq!(layout.visible_range(), 0..text.len());
    assert!(f64::from(layout.width()) > 240.0);
    let last_character = text.len() - 1..text.len();
    let last_character_bounds = viewer.direct_text_range_bounds(last_character).unwrap();
    assert!(!last_character_bounds.is_empty());
    assert!(last_character_bounds.x >= 40.0);
    assert!(last_character_bounds.x + last_character_bounds.width <= 280.0);
    let end = viewer
        .runtime()
        .text_flow_local_caret_geometry(
            text_feedback,
            FlowTextPosition::new(0, text.len(), TextAffinity::Upstream),
        )
        .unwrap()
        .bounds();
    assert!(end.x >= 0.0);
    assert!(end.x + end.width <= 240.0);

    let frame = viewer.runtime_mut().prepare_frame().unwrap();
    let clip = frame
        .commands()
        .iter()
        .position(|command| {
            matches!(
                command.kind,
                RenderCommandKind::Clip { rect }
                    if rect == Rectangle::new(0.0, 0.0, 240.0, 120.0)
            )
        })
        .expect("direct-edit TextFlow must clip to its fixed viewport");
    let glyph = frame
        .commands()
        .iter()
        .position(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
        .expect("long draft must remain a full glyph layout");
    assert!(clip < glyph);

    let start = FlowTextPosition::new(0, 0, TextAffinity::Downstream);
    domain
        .set_direct_text_selection(&mut viewer, FlowTextRange::new(start, start))
        .unwrap();
    let text_feedback = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().scaled_feedback())
        .unwrap()[0];
    let start = viewer
        .runtime()
        .text_flow_local_caret_geometry(text_feedback, start)
        .unwrap()
        .bounds();
    assert!(start.x >= 0.0);
    assert!(start.x + start.width <= 240.0);

    domain
        .set_direct_text_selection(
            &mut viewer,
            FlowTextRange::new(
                FlowTextPosition::new(0, 0, TextAffinity::Downstream),
                FlowTextPosition::new(0, text.len(), TextAffinity::Upstream),
            ),
        )
        .unwrap();
    for quad in viewer.direct_text_selection_geometry().unwrap() {
        let bounds = quad.bounds();
        assert!(bounds.x >= 40.0);
        assert!(bounds.x + bounds.width <= 280.0);
        assert!(bounds.y >= 40.0);
        assert!(bounds.y + bounds.height <= 160.0);
    }
    let frame = viewer.runtime_mut().prepare_frame().unwrap();
    let damage = frame
        .damage()
        .union()
        .expect("selection replacement must damage the edit viewport");
    assert!(damage.x >= 40.0);
    assert!(damage.x + damage.width <= 280.0);
    assert!(damage.y >= 40.0);
    assert!(damage.y + damage.height <= 160.0);
    let before_scale = viewer.direct_text_caret_geometry().unwrap().bounds();
    assert!(viewer.set_viewport_scale_at(1.25, None).unwrap());
    assert!(viewer.synchronize_direct_text_input_area().unwrap());
    let after_scale = viewer.direct_text_caret_geometry().unwrap().bounds();
    assert_ne!(after_scale, before_scale);
    assert!(matches!(
        viewer.take_text_input_effects().last(),
        Some(TextInputEffect::SetArea { area, .. })
            if area.x <= after_scale.x && area.x + area.width >= after_scale.x
    ));
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), text);
}

#[test]
fn target_move_replaces_the_complete_edit_projection_and_damages_old_and_new_bounds() {
    let text = "moving long direct edit text ".repeat(6);
    let mut viewer = viewer(&text);
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();
    viewer.runtime_mut().prepare_frame().unwrap();
    let old_feedback = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().scaled_feedback())
        .unwrap()[0];
    let old_bounds = viewer.runtime().tree().figure_bounds(old_feedback).unwrap();
    let new_bounds = Rectangle::new(150.0, 95.0, 180.0, 90.0);

    viewer.model_mut().unwrap().set_bounds(new_bounds);
    assert!(viewer.refresh().unwrap());

    let new_feedback = viewer
        .runtime()
        .tree()
        .child_order(viewer.root_layers().scaled_feedback())
        .unwrap()[0];
    assert_ne!(new_feedback, old_feedback);
    assert!(!viewer.runtime().tree().is_attached(old_feedback));
    assert_eq!(
        viewer.runtime().tree().figure_bounds(new_feedback),
        Some(new_bounds)
    );
    let caret = viewer.direct_text_caret_geometry().unwrap().bounds();
    assert!(caret.x >= new_bounds.x);
    assert!(caret.x <= new_bounds.x + new_bounds.width);
    assert!(caret.y >= new_bounds.y);
    assert!(caret.y <= new_bounds.y + new_bounds.height);

    let damage = viewer
        .runtime_mut()
        .prepare_frame()
        .unwrap()
        .damage()
        .union()
        .unwrap();
    for bounds in [old_bounds, new_bounds] {
        assert!(damage.contains(bounds.top_left()));
        assert!(damage.contains(bounds.bottom_right()));
    }
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
            purpose: TextInputPurpose::SingleLine,
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
    assert_eq!(direct_text_decoration_count(&viewer), 2);

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
    assert_eq!(direct_text_decoration_count(&viewer), 2);
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
    assert_eq!(direct_text_decoration_count(&viewer), 1);
    assert_eq!(viewer.next_wake_deadline(), None);
    assert!(domain.cancel_direct_text_preedit(&mut viewer).unwrap());
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "alpha");
    assert_eq!(direct_text_decoration_count(&viewer), 2);
    assert!(viewer.next_wake_deadline().is_some());

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
    assert_eq!(direct_text_decoration_count(&viewer), 0);
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

#[test]
fn edit_context_snapshot_flows_through_domain_into_the_draft() {
    let mut viewer = viewer("alpha");
    let source = viewer.part_for_model(TEXT).unwrap();
    let mut domain = EditorDomain::new();
    let session = domain
        .start_direct_text_edit(&mut viewer, source, feature())
        .unwrap();

    domain
        .handle_text_input_event(
            &mut viewer,
            SessionTextInputEvent::new(
                session,
                TextInputEvent::Synchronize(TextInputSnapshot::new("alpha拼", 8..8, Some(5..8))),
            ),
        )
        .unwrap();
    assert_eq!(viewer.direct_text_edit().unwrap().draft(), "alpha拼");
    assert!(viewer.direct_text_edit().unwrap().composition().is_some());
    assert_eq!(viewer.model().text, "alpha");

    domain
        .handle_text_input_event(
            &mut viewer,
            SessionTextInputEvent::new(
                session,
                TextInputEvent::Synchronize(TextInputSnapshot::new("alpha拼", 8..8, None)),
            ),
        )
        .unwrap();
    assert!(viewer.direct_text_edit().unwrap().composition().is_none());
    assert!(domain.accept_direct_text_edit(&mut viewer).unwrap());
    assert_eq!(viewer.model().text, "alpha拼");
}
