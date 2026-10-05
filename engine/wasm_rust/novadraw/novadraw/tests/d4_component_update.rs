use novadraw::render::{
    BackendCapabilities, DamageMode, FontDescriptor, NdCanvas, RenderOutcome, SurfaceInfo,
    TextConstraints, TextError,
};
use novadraw::{
    ComponentInvalidation, ComponentUpdateError, ConnectionId, ConnectionRuntimeError, Figure,
    FigureComponentContext, FigureComponentUpdate, FigureEventHandler, FigureLifecycle, FigureTree,
    FocusError, FramePreparation, FramePreparationError, MouseButton, MouseEvent,
    PreparedFigureUpdate, Rectangle, RectangleFigure, ResourceError, Runtime, RuntimeMutationError,
    WidgetError,
};

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

    fn paint_figure(&self, _canvas: &mut NdCanvas) {}

    fn intrinsic_size(&self) -> (f64, f64) {
        (self.measured_width, self.bounds.height)
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for BadgeFigure {
    fn on_mouse_pressed(
        &self,
        _event: &MouseEvent,
        context: &mut novadraw::EventContext<'_>,
    ) -> bool {
        if self.reject_first_callback_update {
            context.update_component_later(SetBadgeText(String::new()));
        }
        context.update_component_later(SetBadgeText("clicked".to_owned()));
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
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if self.0.is_empty() {
            return Err("badge text cannot be empty");
        }
        let snapshot = BadgeSnapshot {
            measured_width: self.0.len() as f64 * 8.0,
            paint_snapshot: format!("badge:{}", self.0),
            text: self.0,
        };
        Ok(PreparedFigureUpdate::new(snapshot))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.text = prepared.text;
        target.measured_width = prepared.measured_width;
        target.paint_snapshot = prepared.paint_snapshot;
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

    fn lifecycle(&mut self) -> Option<&mut dyn FigureLifecycle> {
        Some(self)
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
        .set_contents(Box::new(BadgeFigure::new("old")));
    let mut runtime = Runtime::new(tree);
    let baseline = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert!(runtime.complete_submission(
        baseline.session_id,
        baseline.frame_id,
        RenderOutcome::Presented
    ));
    assert!(!runtime.has_pending_update());
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
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_ne!(updated.damage.mode(), DamageMode::None);
    assert!(runtime.complete_submission(
        updated.session_id,
        updated.frame_id,
        RenderOutcome::Presented
    ));

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

    let pending_before = runtime.has_pending_update();
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
    assert_eq!(runtime.has_pending_update(), pending_before);
}

#[test]
fn external_figure_callback_can_defer_a_typed_self_update() {
    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")));
    let mut runtime = Runtime::new(tree);

    runtime.dispatch_mouse_pressed(20.0, 25.0, MouseButton::Left);

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
    let badge = tree.builder().set_contents(Box::new(
        BadgeFigure::new("old").rejecting_first_callback_update(),
    ));
    let mut runtime = Runtime::new(tree);

    runtime.dispatch_mouse_pressed(20.0, 25.0, MouseButton::Left);

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
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
        .set_contents(Box::new(BadgeFigure::new("old")));
    let mut runtime = Runtime::new(tree);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runtime.figure(badge).unwrap().update_component(PanicCommit);
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert!(matches!(
        runtime.prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL,),
        FramePreparation::Error(FramePreparationError::Faulted)
    ));

    let mut tree = FigureTree::new();
    let badge = tree
        .builder()
        .set_contents(Box::new(BadgeFigure::new("old")));
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
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
        .set_preferred_size((20.0, 20.0))
        .unwrap();
    let baseline = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        baseline.session_id,
        baseline.frame_id,
        RenderOutcome::Presented,
    );
    assert!(runtime.tree().node(figure).unwrap().state().is_valid());
    let image = runtime.register_image();

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
    assert_eq!(runtime.do_click(figure), Err(WidgetError::Faulted));
    assert_eq!(
        runtime.remove_connection_state(ConnectionId::from_figure(figure)),
        Err(ConnectionRuntimeError::Faulted)
    );
    assert_eq!(
        runtime.remove_resource(image.resource_id()),
        Err(ResourceError::Faulted)
    );
    assert_eq!(
        runtime.layout_text(
            "rejected",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        ),
        Err(TextError::RuntimeFaulted)
    );
    assert_eq!(runtime.request_focus(figure), Err(FocusError::Faulted));
}
