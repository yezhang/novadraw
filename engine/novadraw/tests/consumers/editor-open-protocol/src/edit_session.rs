//! External numeric editing domain: no TextFlow or plain-text draft implementation.
use novadraw::{
    figure::{Figure, RectangleFigure, RootFigure},
    geometry::{Point, Rectangle},
    graphics::Color,
    render::CaretGeometry,
};
use novadraw_editor::*;
use std::{
    cell::Cell,
    convert::Infallible,
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    InputReject,
    InputWrites,
    InputUnknown,
    SnapshotPanic,
    GeometryReject,
    GeometryPanic,
    InvalidGeometry,
    PlanReject,
}
struct Model {
    value: i32,
    revision: ModelRevision,
    present: bool,
    events: Vec<ModelEvent<u64, ()>>,
    fault: Rc<Cell<Fault>>,
}
impl Model {
    fn publish(&mut self, subject: u64) {
        self.revision = self.revision.next().unwrap();
        self.events
            .push(ModelEvent::new(self.revision, subject, ()));
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
        Ok(if id == 1 && self.present {
            vec![2]
        } else {
            vec![]
        })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
}
#[derive(Clone)]
struct Counter {
    initial: i32,
    value: i32,
    composition: Option<i32>,
    fault: Rc<Cell<Fault>>,
    lifetime: Rc<()>,
}
impl EditController for Counter {
    fn input(
        &mut self,
        e: &TextInputEvent,
        context: &mut EditGeometryContext<'_>,
    ) -> Result<bool, ViewerError> {
        match e {
            TextInputEvent::InsertText(v) => {
                self.value += v
                    .parse::<i32>()
                    .map_err(|_| PolicyError::operation("number expected"))?
            }
            TextInputEvent::Preedit { text, .. } => self.composition = Some(text.parse().unwrap()),
            TextInputEvent::CancelComposition => self.composition = None,
            _ => return Ok(false),
        }
        if matches!(self.fault.get(), Fault::InputReject) {
            return Err(PolicyError::operation("candidate rejected after mutation").into());
        }
        if matches!(self.fault.get(), Fault::InputUnknown) {
            return Err(ViewerError::InconsistentState);
        }
        if matches!(self.fault.get(), Fault::InputWrites) {
            context.set_bounds(Rectangle::new(0.0, 0.0, 1.0, 1.0))?;
        }
        Ok(true)
    }
    fn select_at(
        &mut self,
        _: Point,
        _: bool,
        _: &mut EditGeometryContext<'_>,
    ) -> Result<(), ViewerError> {
        Ok(())
    }
    fn geometry(&mut self, c: &mut EditGeometryContext<'_>) -> Result<EditGeometry, ViewerError> {
        match self.fault.get() {
            Fault::GeometryReject => return Err(PolicyError::operation("geometry rejected").into()),
            Fault::GeometryPanic => panic!("external geometry panic"),
            _ => {}
        }
        let mut bounds = c.viewport()?;
        if matches!(self.fault.get(), Fault::InvalidGeometry) {
            bounds.x = f64::NAN;
        }
        Ok(EditGeometry {
            caret: CaretGeometry::new(Rectangle::new(bounds.x, bounds.y, 2.0, 20.0), 0),
            viewport: bounds,
            selection: vec![],
            preedit: vec![],
            caret_visible: true,
        })
    }
    fn is_changed(&self) -> bool {
        self.value != self.initial
    }
    fn has_composition(&self) -> bool {
        self.composition.is_some()
    }
    fn input_snapshot(&self) -> Result<TextInputSnapshot, ViewerError> {
        if matches!(self.fault.get(), Fault::SnapshotPanic) {
            panic!("external snapshot panic");
        }
        Ok(TextInputSnapshot::new(self.value.to_string(), 0..0, None))
    }
    fn range_bounds(
        &self,
        _: Range<usize>,
        c: &mut EditGeometryContext<'_>,
    ) -> Result<Rectangle, ViewerError> {
        c.viewport()
    }
}
struct Plan;
impl EditSessionPlan<Model, Counter> for Plan {
    fn feedback(&mut self, state: &Counter, _: &Model) -> Result<DirectTextFeedback, PolicyError> {
        if matches!(state.fault.get(), Fault::PlanReject) {
            return Err(PolicyError::operation("feedback rejected"));
        }
        DirectTextFeedback::new(
            vec![FeedbackVisual::scaled(Box::new(
                RectangleFigure::new_with_color(20.0, 20.0, 80.0, 40.0, Color::BLACK).unwrap(),
            ))],
            0,
        )
    }
    fn command(
        &mut self,
        state: &Counter,
        _: &Model,
    ) -> Result<Box<dyn Command<Model>>, PolicyError> {
        Ok(Box::new(Set {
            before: state.initial,
            after: state.value,
        }))
    }
}
struct Set {
    before: i32,
    after: i32,
}
impl Command<Model> for Set {
    fn label(&self) -> &str {
        "Set number"
    }
    fn can_execute(&self, _: &Model) -> bool {
        self.after != 99
    }
    fn execute(&mut self, m: &mut Model) -> Result<(), CommandError> {
        m.value = self.after;
        m.publish(2);
        Ok(())
    }
    fn undo(&mut self, m: &mut Model) -> Result<(), CommandError> {
        m.value = self.before;
        m.publish(2);
        Ok(())
    }
}
struct Policy {
    lifetime: Rc<()>,
}
impl EditPolicy<Model> for Policy {
    fn register_handlers(b: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        b.handle::<StartEditSession>(Self::start);
    }
}
impl Policy {
    fn start(
        &mut self,
        _: PolicyHost<u64>,
        _: &RequestMetadata,
        r: &DirectTextEditRequest,
        m: &Model,
    ) -> Result<Option<EditSessionContribution<Model>>, PolicyError> {
        Ok(Some(EditSessionContribution::new(
            EditControllerKey::new(),
            EditSessionDescriptor::new(
                r.feature().clone(),
                m.revision(),
                TextEditMode::SingleLine,
                FocusLossPolicy::Accept,
            ),
            Counter {
                initial: m.value,
                value: m.value,
                composition: None,
                fault: m.fault.clone(),
                lifetime: self.lifetime.clone(),
            },
            Plan,
        )))
    }
}
struct Part {
    lifetime: Rc<()>,
}
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, _: &Model, id: u64) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == 1 {
            Box::new(RootFigure::new(0.0, 0.0, 200.0, 120.0).unwrap())
        } else {
            Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 40.0).unwrap())
        })
    }
    fn create_policies(
        &mut self,
        _: &Model,
        id: u64,
    ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        Ok(if id == 2 {
            vec![
                PolicyInstallation::new(
                    DirectTextEditSlot::key(),
                    Policy {
                        lifetime: self.lifetime.clone(),
                    },
                )
                .unwrap(),
            ]
        } else {
            vec![]
        })
    }
}
struct Factory {
    lifetime: Rc<()>,
}
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part {
            lifetime: self.lifetime.clone(),
        }))
    }
}
type Viewer = GraphicalViewer<Model, Factory>;
fn setup() -> (Viewer, EditorDomain<Model>, Rc<Cell<Fault>>, Rc<()>) {
    let fault = Rc::new(Cell::new(Fault::None));
    let lifetime = Rc::new(());
    let viewer = GraphicalViewer::new(
        Model {
            value: 10,
            revision: ModelRevision::initial(),
            present: true,
            events: vec![],
            fault: fault.clone(),
        },
        Factory {
            lifetime: lifetime.clone(),
        },
        Rectangle::new(0.0, 0.0, 200.0, 120.0),
    )
    .unwrap();
    (viewer, EditorDomain::new(), fault, lifetime)
}
fn start(v: &mut Viewer, d: &mut EditorDomain<Model>) -> DirectTextEditSessionId {
    d.start_edit_session(
        v,
        v.part_for_model(2).unwrap(),
        DirectTextFeature::new("number").unwrap(),
    )
    .unwrap()
}
fn input(
    v: &mut Viewer,
    d: &mut EditorDomain<Model>,
    e: TextInputEvent,
) -> Result<(), EditorDomainError> {
    let session = v.edit_session().unwrap().session();
    d.handle_text_input_event(v, SessionTextInputEvent::new(session, e))
}
fn value(v: &Viewer) -> i32 {
    v.edit_session()
        .unwrap()
        .controller(EditControllerKey::<Counter>::new())
        .unwrap()
        .value
}
#[test]
fn external_controller_accepts_one_model_command_and_undo_redo() {
    let (mut v, mut d, _, _) = setup();
    let s = start(&mut v, &mut d);
    assert!(v.direct_text_edit().is_none());
    assert_eq!(v.edit_session().unwrap().session(), s);
    input(&mut v, &mut d, TextInputEvent::InsertText("4".into())).unwrap();
    assert_eq!(v.model().value, 10);
    assert_eq!(value(&v), 14);
    input(&mut v, &mut d, TextInputEvent::Accept).unwrap();
    assert_eq!(v.model().value, 14);
    assert!(v.edit_session().is_none());
    d.undo(&mut v).unwrap();
    assert_eq!(v.model().value, 10);
    assert!(!d.has_undo());
    d.redo(&mut v).unwrap();
    assert_eq!(v.model().value, 14);
}
#[test]
fn candidate_input_feedback_and_geometry_rejections_preserve_session() {
    for fault in [
        Fault::InputReject,
        Fault::InputWrites,
        Fault::PlanReject,
        Fault::GeometryReject,
        Fault::InvalidGeometry,
    ] {
        let (mut v, mut d, control, _) = setup();
        let s = start(&mut v, &mut d);
        v.take_text_input_effects();
        let before = v
            .runtime()
            .tree()
            .child_order(v.root_layers().scaled_feedback())
            .unwrap()
            .to_vec();
        control.set(fault);
        assert!(input(&mut v, &mut d, TextInputEvent::InsertText("4".into())).is_err());
        assert_eq!(value(&v), 10);
        assert_eq!(v.edit_session().unwrap().session(), s);
        assert!(!v.is_faulted());
        assert_eq!(
            v.runtime()
                .tree()
                .child_order(v.root_layers().scaled_feedback())
                .unwrap(),
            before
        );
        assert!(v.take_text_input_effects().is_empty());
    }
}
#[test]
fn snapshot_panic_uses_viewer_fault_boundary() {
    let (mut v, mut d, control, _) = setup();
    start(&mut v, &mut d);
    assert_eq!(v.edit_input_snapshot().unwrap().text(), "10");
    control.set(Fault::SnapshotPanic);
    assert!(catch_unwind(AssertUnwindSafe(|| v.edit_input_snapshot())).is_err());
    assert!(v.is_faulted());
    assert!(v.edit_session().is_none());
}
#[test]
fn unknown_controller_state_faults_and_releases_input() {
    let (mut v, mut d, control, _) = setup();
    let s = start(&mut v, &mut d);
    v.take_text_input_effects();
    control.set(Fault::InputUnknown);
    assert!(input(&mut v, &mut d, TextInputEvent::InsertText("4".into())).is_err());
    assert!(v.is_faulted());
    assert!(v.edit_session().is_none());
    assert_eq!(v.model().value, 10);
    assert!(
        v.take_text_input_effects()
            .iter()
            .any(|e| matches!(e,TextInputEffect::Release{session} if *session==s))
    );
}
#[test]
fn geometry_panic_releases_lease_and_cleans_old_and_candidate_feedback() {
    let (mut v, mut d, control, _) = setup();
    let s = start(&mut v, &mut d);
    v.take_text_input_effects();
    control.set(Fault::GeometryPanic);
    assert!(
        catch_unwind(AssertUnwindSafe(|| input(
            &mut v,
            &mut d,
            TextInputEvent::InsertText("4".into())
        )))
        .is_err()
    );
    assert!(v.is_faulted());
    assert!(v.edit_session().is_none());
    assert!(
        v.runtime()
            .tree()
            .child_order(v.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
    assert!(
        v.take_text_input_effects()
            .iter()
            .any(|e| matches!(e,TextInputEffect::Release{session} if *session==s))
    );
}
#[test]
fn cancellation_nochange_composition_stale_lease_and_asset_release() {
    let (mut v, mut d, _, lifetime) = setup();
    let base = Rc::strong_count(&lifetime);
    let s = start(&mut v, &mut d);
    assert_eq!(Rc::strong_count(&lifetime), base + 1);
    assert_eq!(
        Rc::strong_count(
            &v.edit_session()
                .unwrap()
                .controller(EditControllerKey::<Counter>::new())
                .unwrap()
                .lifetime
        ),
        base + 1
    );
    input(
        &mut v,
        &mut d,
        TextInputEvent::Preedit {
            text: "1".into(),
            selection: Some(0..1),
        },
    )
    .unwrap();
    assert!(input(&mut v, &mut d, TextInputEvent::Accept).is_err());
    input(&mut v, &mut d, TextInputEvent::CancelComposition).unwrap();
    input(&mut v, &mut d, TextInputEvent::Accept).unwrap();
    assert!(!d.has_undo());
    assert_eq!(Rc::strong_count(&lifetime), base);
    start(&mut v, &mut d);
    assert!(
        d.handle_text_input_event(
            &mut v,
            SessionTextInputEvent::new(s, TextInputEvent::InsertText("3".into()))
        )
        .is_err()
    );
    input(&mut v, &mut d, TextInputEvent::LeaseLost).unwrap();
    assert_eq!(v.model().value, 10);
}
#[test]
fn revision_conflict_and_retirement_do_not_commit() {
    let (mut v, mut d, _, _) = setup();
    let s = start(&mut v, &mut d);
    input(&mut v, &mut d, TextInputEvent::InsertText("2".into())).unwrap();
    v.synchronize_model(|m| m.publish(2)).unwrap();
    assert!(input(&mut v, &mut d, TextInputEvent::Accept).is_err());
    assert_eq!(v.model().value, 10);
    input(&mut v, &mut d, TextInputEvent::Cancel).unwrap();
    start(&mut v, &mut d);
    v.take_text_input_effects();
    v.synchronize_model(|m| {
        m.present = false;
        m.publish(1)
    })
    .unwrap();
    assert!(v.edit_session().is_none());
    assert!(
        v.take_text_input_effects()
            .iter()
            .any(|e| matches!(e,TextInputEffect::Release{session} if *session!=s))
    );
}
#[test]
fn rejected_command_restores_controller_and_lease() {
    let (mut v, mut d, _, _) = setup();
    let s = start(&mut v, &mut d);
    input(&mut v, &mut d, TextInputEvent::InsertText("89".into())).unwrap();
    v.take_text_input_effects();
    assert!(input(&mut v, &mut d, TextInputEvent::Accept).is_err());
    assert_eq!(value(&v), 99);
    assert_eq!(v.model().value, 10);
    let effects = v.take_text_input_effects();
    assert!(
        matches!(effects.as_slice(),[TextInputEffect::Release{session:r},TextInputEffect::Acquire{session:a,..}] if *r==s && *a==s)
    );
}
#[test]
fn rejected_switch_preserves_current_session_and_input_owner() {
    let (mut v, mut d, _, _) = setup();
    let s = start(&mut v, &mut d);
    input(
        &mut v,
        &mut d,
        TextInputEvent::Preedit {
            text: "1".into(),
            selection: Some(0..1),
        },
    )
    .unwrap();
    v.take_text_input_effects();
    assert!(d.set_active_tool(&mut v, SelectionTool::new()).is_err());
    let source = v.part_for_model(2).unwrap();
    assert!(
        d.start_edit_session(&mut v, source, DirectTextFeature::new("number").unwrap())
            .is_err()
    );
    assert_eq!(v.edit_session().unwrap().session(), s);
    assert!(v.take_text_input_effects().is_empty());
    assert!(!v.is_faulted());
    input(&mut v, &mut d, TextInputEvent::CancelComposition).unwrap();
    d.set_active_tool(&mut v, SelectionTool::new()).unwrap();
    assert!(v.edit_session().is_none());
    assert!(!d.has_undo());
}
