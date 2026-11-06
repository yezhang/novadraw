//! Independent text-policy consumer; no changes to Core/Editor are required.
use egui::{Event, ImeEvent, Key};
use novadraw::{
    figure::{Figure, FlowPage, RectangleFigure, RootFigure, TextFlowFigure},
    geometry::Rectangle,
    render::BuiltinFont,
};
use novadraw_editor::*;
use novadraw_ui::editor_canvas::EditorCanvasInput;
use std::convert::Infallible;

const BOUNDS: Rectangle = Rectangle {
    x: 40.0,
    y: 40.0,
    width: 240.0,
    height: 120.0,
};

struct Model {
    text: String,
    revision: ModelRevision,
    events: Vec<ModelEvent<u64, ()>>,
}
impl Model {
    fn replace(&mut self, text: String) {
        self.text = text;
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(self.revision, 2, ()));
    }
}
impl ModelAdapter for Model {
    type ModelId = u64;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u64 {
        1
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: u64) -> Result<Vec<u64>, Infallible> {
        Ok(if id == 1 { vec![2] } else { Vec::new() })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
}
struct SetText {
    before: String,
    after: String,
}
impl Command<Model> for SetText {
    fn label(&self) -> &str {
        "Text"
    }
    fn execute(&mut self, model: &mut Model) -> Result<(), CommandError> {
        model.replace(self.after.clone());
        Ok(())
    }
    fn undo(&mut self, model: &mut Model) -> Result<(), CommandError> {
        model.replace(self.before.clone());
        Ok(())
    }
}
struct Plan {
    descriptor: DirectTextEditDescriptor,
}
impl DirectTextEdit<Model> for Plan {
    fn descriptor(&self) -> &DirectTextEditDescriptor {
        &self.descriptor
    }
    fn feedback(
        &mut self,
        state: &DirectTextEditState,
        _: &Model,
    ) -> Result<DirectTextFeedback, PolicyError> {
        DirectTextFeedback::new(
            vec![FeedbackVisual::scaled(Box::new(
                TextFlowFigure::new(BOUNDS, FlowPage::from_text(state.draft())).unwrap(),
            ))],
            0,
        )
    }
    fn command(
        &mut self,
        state: &DirectTextEditState,
        model: &Model,
    ) -> Result<Box<dyn Command<Model>>, PolicyError> {
        Ok(Box::new(SetText {
            before: model.text.clone(),
            after: state.draft().into(),
        }))
    }
}
struct Policy {
    loss: FocusLossPolicy,
}
impl EditPolicy<Model> for Policy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle::<StartDirectTextEdit>(Self::start);
    }
}
impl Policy {
    fn start(
        &mut self,
        _: PolicyHost<u64>,
        _: &RequestMetadata,
        request: &DirectTextEditRequest,
        model: &Model,
    ) -> Result<Option<<StartDirectTextEdit as RequestProtocol<Model>>::Contribution>, PolicyError>
    {
        Ok(Some(Box::new(Plan {
            descriptor: DirectTextEditDescriptor::new(
                request.feature().clone(),
                model.text.clone(),
                model.revision,
                TextEditMode::SingleLine,
                self.loss,
            ),
        })))
    }
}
struct Part {
    loss: FocusLossPolicy,
}
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, _: &Model, id: u64) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == 1 {
            Box::new(RootFigure::new(0.0, 0.0, 480.0, 320.0).unwrap())
        } else {
            Box::new(RectangleFigure::from_bounds(BOUNDS).unwrap())
        })
    }
    fn create_policies(
        &mut self,
        _: &Model,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        Ok(if id == 2 {
            vec![
                PolicyInstallation::new(DirectTextEditSlot::key(), Policy { loss: self.loss })
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            ]
        } else {
            Vec::new()
        })
    }
}
struct Factory {
    loss: FocusLossPolicy,
}
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part { loss: self.loss }))
    }
}
struct Harness {
    context: egui::Context,
    input: EditorCanvasInput,
    viewer: GraphicalViewer<Model, Factory>,
    domain: EditorDomain<Model>,
    time: f64,
    id: egui::Id,
}
impl Harness {
    fn new(loss: FocusLossPolicy) -> Self {
        let model = Model {
            text: "abc".into(),
            revision: ModelRevision::initial(),
            events: Vec::new(),
        };
        let mut viewer = GraphicalViewer::new(
            model,
            Factory { loss },
            Rectangle::new(0.0, 0.0, 480.0, 320.0),
        )
        .unwrap();
        viewer
            .drive()
            .resize_logical_viewport(480.0, 320.0)
            .unwrap();
        viewer
            .drive()
            .register_builtin_font(BuiltinFont::Inter)
            .unwrap();
        viewer.stabilize_for_query().unwrap();
        let id = egui::Id::unique("independent-text-canvas");
        let mut h = Self {
            context: egui::Context::default(),
            input: EditorCanvasInput::new(id),
            viewer,
            domain: EditorDomain::new(),
            time: 0.0,
            id,
        };
        h.frame(Vec::new(), true);
        h.start();
        h
    }
    fn start(&mut self) {
        let source = self.viewer.part_for_model(2).unwrap();
        self.domain
            .start_direct_text_edit(
                &mut self.viewer,
                source,
                DirectTextFeature::new("external.label").unwrap(),
            )
            .unwrap();
        self.context.memory_mut(|m| m.request_focus(self.id));
        self.frame(Vec::new(), true);
    }
    fn frame(&mut self, events: Vec<Event>, focused: bool) -> egui::FullOutput {
        self.time += 0.1;
        let mut out = self.context.run_ui(
            egui::RawInput {
                time: Some(self.time),
                events,
                focused,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 340.0),
                )),
                ..Default::default()
            },
            |ui| {
                let out = self.input.show(ui, &mut self.viewer, &mut self.domain);
                assert!(out.error.is_none(), "{:?}", out.error);
            },
        );
        out.textures_delta.clear();
        out
    }
    fn draft(&mut self) -> String {
        self.viewer.edit_input_snapshot().unwrap().text().into()
    }
    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.context.tex_manager().write().take_delta().clear();
    }
}

#[test]
fn ime_preedit_commit_echo_and_confirm_preserve_one_model_transaction() {
    let mut h = Harness::new(FocusLossPolicy::Cancel);
    let out = h.frame(
        vec![Event::Ime(ImeEvent::Preedit {
            text: "你".into(),
            active_range_chars: Some(1..1),
        })],
        true,
    );
    assert!(out.platform_output.ime.is_some());
    assert_eq!(h.viewer.model().text, "abc");
    let out = h.frame(
        vec![
            Event::Ime(ImeEvent::Commit("你".into())),
            Event::Text("你".into()),
            Harness::key(Key::Enter),
        ],
        true,
    );
    assert!(out.platform_output.ime.is_some());
    assert_eq!(h.draft().matches('你').count(), 1);
    assert_eq!(h.viewer.model().text, "abc"); // IME's confirm Enter must not accept.
    h.frame(vec![Harness::key(Key::Enter)], true);
    assert!(h.viewer.edit_session().is_none());
    assert!(h.viewer.model().text.contains('你'));
    h.domain.undo(&mut h.viewer).unwrap();
    assert_eq!(h.viewer.model().text, "abc");
    assert!(!h.domain.has_undo());
    h.domain.redo(&mut h.viewer).unwrap();
    assert!(h.viewer.model().text.contains('你'));
}

#[test]
fn text_focus_loss_follows_policy_and_clears_ime_descriptor() {
    for loss in [FocusLossPolicy::Accept, FocusLossPolicy::Cancel] {
        let mut h = Harness::new(loss);
        h.frame(vec![Event::Text("x".into())], true);
        let out = h.frame(Vec::new(), false);
        assert!(h.viewer.edit_session().is_none());
        assert!(out.platform_output.ime.is_none());
        assert_eq!(
            h.viewer.model().text.contains('x'),
            loss == FocusLossPolicy::Accept
        );
    }
}

#[test]
fn completed_text_lease_cannot_receive_later_events_in_same_pass() {
    let mut h = Harness::new(FocusLossPolicy::Cancel);
    h.frame(
        vec![Harness::key(Key::Escape), Event::Text("late".into())],
        true,
    );
    assert!(h.viewer.edit_session().is_none());
    h.start();
    assert!(!h.draft().contains("late"));
}

#[test]
fn input_failure_cancels_draft_even_when_focus_loss_would_accept() {
    let mut h = Harness::new(FocusLossPolicy::Accept);
    h.frame(vec![Event::Text("x".into())], true);
    h.frame(vec![Event::PointerMoved(egui::pos2(100.0, 100.0))], true);
    let mut error = None;
    let mut output = h.context.run_ui(
        egui::RawInput {
            events: vec![Event::Zoom(f32::NAN)],
            time: Some(h.time + 0.1),
            focused: true,
            ..Default::default()
        },
        |ui| {
            error = h.input.show(ui, &mut h.viewer, &mut h.domain).error;
        },
    );
    output.textures_delta.clear();
    assert!(error.is_some());
    assert!(h.viewer.edit_session().is_none());
    assert_eq!(h.viewer.model().text, "abc");
    assert!(!h.domain.has_undo());
}
