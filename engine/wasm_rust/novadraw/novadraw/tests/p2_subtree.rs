use std::sync::{Arc, Mutex};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        CONTAINER, ChildPolicy, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureMeasurement, FigureStyle, INPUT,
        InputCapability, LabelFigure, MeasureConstraints, RectangleFigure,
    },
    geometry::{Dimension, Rectangle},
    graphics::Color,
    layout::{
        LayoutConstraint, LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery, StackLayout,
    },
    runtime::{Runtime, RuntimeMutationError},
    tree::{FigureId, FigureSubtree, SubtreeReceipt},
};

fn runtime() -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .unwrap();
    (runtime, root)
}

#[test]
fn external_description_preserves_order_layout_style_and_scoped_identity_mapping() {
    let (mut runtime, root) = runtime();
    let mut subtree = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)));
    let parent = subtree.root();
    let first = subtree
        .add(
            parent,
            Box::new(LabelFigure::new("first").with_border(
                novadraw::figure::border::MarginBorder::new(Color::BLACK, 2.0),
            )),
        )
        .unwrap();
    let second = subtree
        .add(parent, Box::new(LabelFigure::new("second")))
        .unwrap();
    subtree
        .set_layout_manager(parent, Box::new(StackLayout::new()))
        .unwrap();
    subtree
        .set_style(
            parent,
            FigureStyle {
                foreground: Some(Color::WHITE),
                ..FigureStyle::default()
            },
        )
        .unwrap();
    let foreign = FigureSubtree::new(Box::new(LabelFigure::new("foreign"))).root();
    assert!(
        subtree
            .add(foreign, Box::new(LabelFigure::new("invalid")))
            .is_err()
    );
    let receipt = runtime.figure(root).unwrap().add_subtree(subtree).unwrap();
    assert_eq!(receipt.root(), receipt.figure(parent).unwrap());
    assert_eq!(
        runtime.tree().child_order(receipt.root()).unwrap(),
        [
            receipt.figure(first).unwrap(),
            receipt.figure(second).unwrap()
        ]
    );
    assert_eq!(
        runtime
            .tree()
            .resolved_style(receipt.figure(first).unwrap())
            .unwrap()
            .foreground,
        Color::WHITE
    );
    assert!(
        runtime
            .tree()
            .layout_manager(receipt.root())
            .unwrap()
            .is_some()
    );
    assert!(receipt.figure(foreign).is_err());
}

struct Single;
impl Figure for Single {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalSingle"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}
impl FigureContainer for Single {
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }
}

#[test]
fn last_input_failure_and_internal_child_policy_publish_no_nodes() {
    let (mut runtime, root) = runtime();
    for invalid_bounds in [false, true] {
        let before = runtime.tree().descendant_ids(root).unwrap();
        let mut subtree = FigureSubtree::new(Box::new(Single));
        subtree
            .add(subtree.root(), Box::new(LabelFigure::new("first")))
            .unwrap();
        let bounds = if invalid_bounds {
            Rectangle::new(0.0, 0.0, f64::NAN, 1.0)
        } else {
            Rectangle::ZERO
        };
        subtree
            .add(
                subtree.root(),
                Box::new(RectangleFigure::new(
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                )),
            )
            .unwrap();
        assert!(matches!(
            runtime.figure(root).unwrap().add_subtree(subtree),
            Err(RuntimeMutationError::FigureBatch { index: 2, .. })
        ));
        assert_eq!(runtime.tree().descendant_ids(root).unwrap(), before);
        assert!(!runtime.is_faulted());
    }
}

#[derive(Clone)]
struct RejectConstraint;
struct RejectLayout(Arc<Mutex<Vec<FigureId>>>);
impl LayoutManager for RejectLayout {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(0.0, 0.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        _: FigureId,
        _: &LayoutSceneQuery<'_>,
        _: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        Ok(())
    }
    fn validate_constraint(
        &self,
        parent: FigureId,
        child: FigureId,
        _: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        self.0.lock().unwrap().push(child);
        Err(LayoutError::InvalidChild {
            container: parent,
            child,
        })
    }
}

#[test]
fn rejected_constraint_retires_reserved_ids_and_keeps_existing_topology() {
    let (mut runtime, root) = runtime();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut subtree = FigureSubtree::new(Box::new(Single));
    let child = subtree
        .add(subtree.root(), Box::new(LabelFigure::new("reject")))
        .unwrap();
    subtree
        .set_layout_manager(subtree.root(), Box::new(RejectLayout(seen.clone())))
        .unwrap();
    subtree
        .set_constraint(child, Box::new(RejectConstraint))
        .unwrap();
    assert!(runtime.figure(root).unwrap().add_subtree(subtree).is_err());
    assert!(runtime.tree().descendant_ids(root).unwrap().is_empty());
    let rejected = seen.lock().unwrap()[0];
    assert!(runtime.tree().figure_bounds(rejected).is_err());
    let next = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(LabelFigure::new("next")))
        .unwrap();
    assert_ne!(rejected, next);
}

struct Trigger {
    parent: FigureId,
    receipt: Arc<Mutex<Option<SubtreeReceipt>>>,
    rejection: Arc<Mutex<Option<String>>>,
    observed: Arc<Mutex<usize>>,
}
impl Figure for Trigger {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalSubtreeTrigger"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureEventHandler for Trigger {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let subtree = FigureSubtree::new(Box::new(LabelFigure::new("deferred")));
        let receipt = self.receipt.clone();
        let rejection = self.rejection.clone();
        context
            .effects()
            .add_subtree_later(self.parent, subtree, move |result| match result {
                Ok(value) => *receipt.lock().unwrap() = Some(value),
                Err(error) => *rejection.lock().unwrap() = Some(error.to_string()),
            });
        *self.observed.lock().unwrap() =
            context.scene().children(context.target_id()).unwrap().len();
        assert!(self.receipt.lock().unwrap().is_none());
        true
    }
}

#[test]
fn deferred_success_receipt_follows_commit_and_foreign_rejection_does_not_publish_model() {
    for foreign in [false, true] {
        let (mut runtime, root) = runtime();
        let (_other, other_root) = self::runtime();
        let receipt = Arc::new(Mutex::new(None));
        let rejection = Arc::new(Mutex::new(None));
        let observed = Arc::new(Mutex::new(usize::MAX));
        runtime
            .figure(root)
            .unwrap()
            .add(Box::new(Trigger {
                parent: if foreign { other_root } else { root },
                receipt: receipt.clone(),
                rejection: rejection.clone(),
                observed: observed.clone(),
            }))
            .unwrap();
        runtime
            .input()
            .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
        assert_eq!(*observed.lock().unwrap(), 0);
        assert_eq!(receipt.lock().unwrap().is_some(), !foreign);
        let errors = runtime.take_deferred_mutation_errors();
        assert_eq!(errors.len(), usize::from(foreign));
        assert_eq!(
            rejection.lock().unwrap().as_deref(),
            errors.first().map(|error| error.to_string()).as_deref()
        );
        assert_eq!(
            runtime.tree().descendant_ids(root).unwrap().len(),
            if foreign { 1 } else { 2 }
        );
        if let Some(receipt) = receipt.lock().unwrap().as_ref() {
            assert!(runtime.tree().figure_bounds(receipt.root()).is_ok());
        }
    }
}

#[test]
fn depth_limit_is_checked_before_publication() {
    let (mut runtime, root) = runtime();
    let mut subtree = FigureSubtree::new(Box::new(Single));
    let mut parent = subtree.root();
    for _ in 1..novadraw::tree::MAX_TREE_DEPTH {
        parent = subtree.add(parent, Box::new(Single)).unwrap();
    }
    assert!(runtime.figure(root).unwrap().add_subtree(subtree).is_err());
    assert!(runtime.tree().descendant_ids(root).unwrap().is_empty());
    assert!(!runtime.is_faulted());
}

#[test]
fn resource_preflight_rejects_foreign_dependency_without_publication() {
    let (mut runtime, root) = runtime();
    let (mut other, _) = self::runtime();
    let foreign = other.resources().register_image().unwrap().resource_id();
    let mut subtree = FigureSubtree::new(Box::new(Single));
    subtree
        .add_resource_dependency(subtree.root(), foreign)
        .unwrap();
    assert!(matches!(
        runtime.figure(root).unwrap().add_subtree(subtree),
        Err(RuntimeMutationError::FigureBatch { index: 0, .. })
    ));
    assert!(runtime.tree().descendant_ids(root).unwrap().is_empty());
}

struct PanicAttach;
impl Figure for PanicAttach {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalPanicAttach"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::LIFECYCLE,
            novadraw::figure::LifecycleCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigureLifecycle for PanicAttach {
    fn on_attached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        panic!("attachment fault");
    }
}

#[test]
fn attachment_panic_faults_runtime_and_retains_truthful_published_topology() {
    let (mut runtime, root) = runtime();
    let mut subtree = FigureSubtree::new(Box::new(PanicAttach));
    subtree
        .add(subtree.root(), Box::new(LabelFigure::new("child")))
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.figure(root).unwrap().add_subtree(subtree)
    }));
    assert!(result.is_err());
    assert!(runtime.is_faulted());
    assert_eq!(runtime.tree().descendant_ids(root).unwrap().len(), 2);
}

struct OrderedTrigger {
    parent: FigureId,
    foreign: FigureId,
    outcomes: Arc<Mutex<Vec<bool>>>,
    panic_on_result: bool,
}
impl Figure for OrderedTrigger {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalOrderedTrigger"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureEventHandler for OrderedTrigger {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let before = context.scene().children(self.parent).unwrap();
        for parent in [self.foreign, self.parent, self.parent] {
            let outcomes = self.outcomes.clone();
            let panic_on_result = self.panic_on_result;
            context.effects().add_subtree_later(
                parent,
                FigureSubtree::new(Box::new(LabelFigure::new("queued"))),
                move |result| {
                    outcomes.lock().unwrap().push(result.is_ok());
                    assert!(!panic_on_result, "completion fault");
                },
            );
        }
        assert_eq!(context.scene().children(self.parent).unwrap(), before);
        assert!(self.outcomes.lock().unwrap().is_empty());
        true
    }
}

#[test]
fn deferred_rejection_preserves_fifo_and_later_requests_still_commit() {
    let (mut runtime, root) = runtime();
    let (_other, foreign) = self::runtime();
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(OrderedTrigger {
            parent: root,
            foreign,
            outcomes: outcomes.clone(),
            panic_on_result: false,
        }))
        .unwrap();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(*outcomes.lock().unwrap(), [false, true, true]);
    assert_eq!(runtime.take_deferred_mutation_errors().len(), 1);
    assert_eq!(runtime.tree().child_order(root).unwrap().len(), 3);
    assert!(!runtime.is_faulted());
}

#[test]
fn result_callback_panic_faults_runtime_and_stops_later_requests() {
    for reject in [false, true] {
        let (mut runtime, root) = runtime();
        let (_other, other_root) = self::runtime();
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        runtime
            .figure(root)
            .unwrap()
            .add(Box::new(OrderedTrigger {
                parent: root,
                foreign: if reject { other_root } else { root },
                outcomes: outcomes.clone(),
                panic_on_result: true,
            }))
            .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime
                    .input()
                    .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left)
            }))
            .is_err()
        );
        assert!(runtime.is_faulted());
        assert_eq!(*outcomes.lock().unwrap(), [!reject]);
        assert_eq!(
            runtime.take_deferred_mutation_errors().len(),
            usize::from(reject)
        );
        assert_eq!(
            runtime.tree().child_order(root).unwrap().len(),
            if reject { 1 } else { 2 }
        );
    }
}
