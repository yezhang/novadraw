mod support;

use support::record_full_submission;

use novadraw::connection::{ConnectionId, ConnectionRuntimeError};
use novadraw::event::{
    EventContext, FigureEventHandler, ListenerDirective, MouseButton, MouseEvent,
    PropertyChangeEvent, PropertyChangeListener, PropertyKey,
};
use novadraw::figure::{
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureLifecycle, INPUT,
    InputCapability, LIFECYCLE, LifecycleCapability, WidgetError,
};
use novadraw::render::{
    BackendCapabilities, DamageMode, FontDescriptor, RenderDisposition, SurfaceInfo,
    TextConstraints, TextError,
};
use novadraw::runtime::{
    ComponentInvalidation, ComponentUpdateError, FigureComponentContext, FigureComponentUpdate,
    FocusError, PreparedFigureUpdate, ResourceError, RuntimeMutationError,
    SubmissionAcknowledgement,
};
use novadraw::{
    figure::Figure, figure::RectangleFigure, geometry::Dimension, geometry::Rectangle,
    runtime::FramePreparation, runtime::FramePreparationError, runtime::Runtime, tree::FigureTree,
};
use std::sync::{Arc, Mutex};

const BADGE_TEXT: PropertyKey<String> = PropertyKey::new("example.badge", "text");

#[derive(Clone, Debug, PartialEq)]
struct BadgeSnapshot {
    text: String,
    measured_width: f64,
    paint_snapshot: String,
}

struct BadgeFigure {
    bounds: Rectangle,
    text: String,
    measured_width: f64,
    paint_snapshot: String,
    reject_first_callback_update: bool,
}

impl BadgeFigure {
    fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            bounds: Rectangle::new(10.0, 20.0, 80.0, 24.0),
            measured_width: text.len() as f64 * 8.0,
            paint_snapshot: format!("badge:{text}"),
            text,
            reject_first_callback_update: false,
        }
    }

    fn rejecting_first_callback_update(mut self) -> Self {
        self.reject_first_callback_update = true;
        self
    }

    fn snapshot(&self) -> BadgeSnapshot {
        BadgeSnapshot {
            text: self.text.clone(),
            measured_width: self.measured_width,
            paint_snapshot: self.paint_snapshot.clone(),
        }
    }
}

impl Figure for BadgeFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "BadgeFigure"
    }

    fn intrinsic_size(&self) -> Dimension {
        Dimension::new(self.measured_width, self.bounds.height)
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for BadgeFigure {
    fn on_mouse_pressed(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if self.reject_first_callback_update {
            context
                .effects()
                .update_component_later(SetBadgeText(String::new()));
        }
        context
            .effects()
            .update_component_later(SetBadgeText("clicked".to_owned()));
        true
    }
}

struct SetBadgeText(String);

impl FigureComponentUpdate for SetBadgeText {
    type Figure = BadgeFigure;
    type Prepared = BadgeSnapshot;
    type Error = &'static str;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if self.0.is_empty() {
            return Err("badge text cannot be empty");
        }
        let old_text = current.text.clone();
        let snapshot = BadgeSnapshot {
            measured_width: self.0.len() as f64 * 8.0,
            paint_snapshot: format!("badge:{}", self.0),
            text: self.0,
        };
        let new_text = snapshot.text.clone();
        Ok(
            PreparedFigureUpdate::new(snapshot)
                .with_property_change(BADGE_TEXT, old_text, new_text),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.text = prepared.text;
        target.measured_width = prepared.measured_width;
        target.paint_snapshot = prepared.paint_snapshot;
    }
}

#[derive(Clone)]
struct PropertyRecorder(Arc<Mutex<Vec<PropertyChangeEvent>>>);

impl PropertyChangeListener for PropertyRecorder {
    fn property_changed(&self, event: &PropertyChangeEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event.clone());
        ListenerDirective::Keep
    }
}

struct ProbeBadge;

impl FigureComponentUpdate for ProbeBadge {
    type Figure = BadgeFigure;
    type Prepared = ();
    type Error = BadgeSnapshot;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        Err(current.snapshot())
    }

    fn commit(_prepared: Self::Prepared, _target: &mut Self::Figure) {
        unreachable!("probe updates never commit");
    }
}

struct PanicCommit;

impl FigureComponentUpdate for PanicCommit {
    type Figure = BadgeFigure;
    type Prepared = ();
    type Error = ();

    fn prepare(
        self,
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        Ok(PreparedFigureUpdate::paint(()))
    }

    fn commit(_prepared: Self::Prepared, target: &mut Self::Figure) {
        target.text = "partially committed".to_owned();
        panic!("component commit failed");
    }
}

struct PanicPrepare;

impl FigureComponentUpdate for PanicPrepare {
    type Figure = BadgeFigure;
    type Prepared = ();
    type Error = ();

    fn prepare(
        self,
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        panic!("component prepare failed");
    }

    fn commit(_prepared: Self::Prepared, _target: &mut Self::Figure) {
        unreachable!("panicking prepare never commits");
    }
}

struct PanicInvalidateFigure {
    bounds: Rectangle,
}

impl Figure for PanicInvalidateFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "PanicInvalidateFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(LIFECYCLE, LifecycleCapability::of::<Self>())
    }
}

impl FigureLifecycle for PanicInvalidateFigure {
    fn invalidate(&mut self) {
        panic!("lifecycle invalidate failed");
    }
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 100,
        pixel_height: 100,
        scale_factor: 1.0,
    }
}

#[test]
fn external_component_update_is_typed_atomic_and_conservatively_invalidated() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let baseline = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert!(
        runtime
            .frames()
            .complete_submission(
                baseline.session_id,
                baseline.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .is_ok()
    );
    assert!(!runtime.frames().has_pending_update());
    let layout_generation = runtime
        .tree()
        .node(badge)
        .unwrap()
        .layout_state()
        .generation();

    let receipt = runtime
        .figure(badge)
        .unwrap()
        .update_component(SetBadgeText("updated".to_owned()))
        .unwrap();
    assert_eq!(receipt.figure, badge);
    assert_eq!(receipt.previous_revision, 0);
    assert_eq!(receipt.revision, 1);
    assert_eq!(
        receipt.invalidation,
        ComponentInvalidation::LayoutGeometryAndPaint
    );
    assert!(
        runtime
            .tree()
            .node(badge)
            .unwrap()
            .layout_state()
            .generation()
            > layout_generation
    );
    let updated = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_ne!(updated.damage.mode(), DamageMode::None);
    assert!(
        runtime
            .frames()
            .complete_submission(
                updated.session_id,
                updated.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .is_ok()
    );

    let observed = runtime
        .figure(badge)
        .unwrap()
        .update_component(ProbeBadge)
        .unwrap_err();
    assert_eq!(
        observed,
        ComponentUpdateError::Rejected(BadgeSnapshot {
            text: "updated".to_owned(),
            measured_width: 56.0,
            paint_snapshot: "badge:updated".to_owned(),
        })
    );
    assert_eq!(runtime.tree().component_revision(badge), Some(1));

    let pending_before = runtime.frames().has_pending_update();
    let rejected = runtime
        .figure(badge)
        .unwrap()
        .update_component(SetBadgeText(String::new()))
        .unwrap_err();
    assert_eq!(
        rejected,
        ComponentUpdateError::Rejected("badge text cannot be empty")
    );
    assert_eq!(runtime.tree().component_revision(badge), Some(1));
    assert_eq!(runtime.frames().has_pending_update(), pending_before);
}

#[test]
fn external_component_update_commits_typed_property_effect_once() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    record_full_submission(&mut runtime);

    let observed = Arc::new(Mutex::new(Vec::new()));
    runtime
        .observations()
        .add_property_listener(Box::new(PropertyRecorder(Arc::clone(&observed))));

    runtime
        .figure(badge)
        .unwrap()
        .update_component(SetBadgeText("updated".to_owned()))
        .unwrap();
    record_full_submission(&mut runtime);

    let events = observed.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].figure_id(), badge);
    assert!(events[0].property().is(BADGE_TEXT));
    drop(events);

    let rejected = runtime
        .figure(badge)
        .unwrap()
        .update_component(SetBadgeText(String::new()))
        .unwrap_err();
    assert_eq!(
        rejected,
        ComponentUpdateError::Rejected("badge text cannot be empty")
    );
    record_full_submission(&mut runtime);
    assert_eq!(observed.lock().unwrap().len(), 1);
}

#[test]
fn external_figure_callback_can_defer_a_typed_self_update() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    runtime
        .input()
        .dispatch_mouse_pressed(20.0, 25.0, MouseButton::Left);

    assert_eq!(runtime.tree().component_revision(badge), Some(1));
    let snapshot = runtime
        .figure(badge)
        .unwrap()
        .update_component(ProbeBadge)
        .unwrap_err();
    assert!(matches!(
        snapshot,
        ComponentUpdateError::Rejected(snapshot)
            if snapshot.text == "clicked" && snapshot.paint_snapshot == "badge:clicked"
    ));
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

#[test]
fn deferred_component_rejection_is_reported_without_losing_fifo_suffix() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(
            BadgeFigure::new("old").rejecting_first_callback_update(),
        ))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    runtime
        .input()
        .dispatch_mouse_pressed(20.0, 25.0, MouseButton::Left);

    assert_eq!(runtime.tree().component_revision(badge), Some(1));
    assert_eq!(
        runtime.take_deferred_mutation_errors(),
        vec![RuntimeMutationError::ComponentUpdateRejected(badge)]
    );
}

#[test]
fn component_update_rejects_wrong_foreign_and_disposed_targets() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    let badge = tree
        .builder()
        .add_child(root, Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    let wrong_type = runtime
        .figure(root)
        .unwrap()
        .update_component(SetBadgeText("new".to_owned()))
        .unwrap_err();
    assert_eq!(
        wrong_type,
        ComponentUpdateError::WrongFigureType {
            figure: root,
            expected: std::any::type_name::<BadgeFigure>(),
            actual: std::any::type_name::<RectangleFigure>(),
        }
    );

    let mut foreign = Runtime::empty();
    let foreign_id = foreign
        .set_contents(Box::new(BadgeFigure::new("foreign")))
        .expect("valid Runtime mutation");
    assert!(matches!(
        runtime.figure(foreign_id),
        Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign_id
    ));

    runtime.dispose_subtree(badge).unwrap();
    assert!(matches!(
        runtime.figure(badge),
        Err(RuntimeMutationError::UnknownOrDisposedFigure(id)) if id == badge
    ));
}

#[test]
fn component_prepare_or_commit_panic_faults_runtime() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runtime.figure(badge).unwrap().update_component(PanicCommit);
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL,),
        FramePreparation::Error(FramePreparationError::Faulted)
    ));

    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runtime
            .figure(badge)
            .unwrap()
            .update_component(PanicPrepare);
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert_eq!(runtime.tree().component_revision(badge), Some(0));
}

#[test]
fn lifecycle_invalidation_panic_faults_all_public_mutation_domains() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    let figure = tree
        .builder()
        .add_child(
            root,
            Box::new(PanicInvalidateFigure {
                bounds: Rectangle::new(0.0, 0.0, 20.0, 20.0),
            }),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime
        .figure(figure)
        .unwrap()
        .set_preferred_size(Dimension::new(20.0, 20.0))
        .unwrap();
    let baseline = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    let acknowledgement = runtime
        .frames()
        .complete_submission(
            baseline.session_id,
            baseline.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("frame completion");
    assert_eq!(acknowledgement, SubmissionAcknowledgement::Presented);
    assert!(runtime.tree().node(figure).unwrap().state().is_valid());
    let image = runtime
        .resources()
        .register_image()
        .expect("image registration");

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime
            .figure(figure)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 30.0, 30.0))
            .expect("valid Runtime mutation");
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());

    assert!(matches!(
        runtime.figure(figure),
        Err(RuntimeMutationError::Faulted)
    ));
    assert_eq!(runtime.input().do_click(figure), Err(WidgetError::Faulted));
    assert_eq!(
        runtime
            .connections()
            .remove_state(ConnectionId::from_figure(figure)),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(
        runtime.resources().remove(image.resource_id()),
        Err(ResourceError::Faulted)
    );
    assert_eq!(
        runtime.text().layout(
            "rejected",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        ),
        Err(TextError::RuntimeFaulted)
    );
    assert_eq!(runtime.focus().request(figure), Err(FocusError::Faulted));
}
