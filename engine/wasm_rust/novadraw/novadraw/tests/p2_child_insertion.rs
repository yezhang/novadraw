use std::{cell::RefCell, rc::Rc};

use novadraw::render::{BackendCapabilities, RenderOutcome, SurfaceInfo};
use novadraw::tree::ChildInsertionError;
use novadraw::{
    AncestorEvent, AncestorEventKind, AncestorListener, ChildPolicy, Dimension, Figure,
    FigureContainer, FigureId, FigureLifecycle, FigureLifecycleContext, FigureMeasurement,
    FigureTree, GraphMutationError, LayoutConstraint, LayoutError, LayoutEvent, LayoutEventKind,
    LayoutListener, LayoutManager, LayoutOutput, LayoutSnapshot, ListenerDirective,
    MeasureConstraints, Rectangle, RectangleFigure, Runtime, RuntimeMutationError, XYLayout,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement(Rectangle);

struct PanicConstraint;

type Trace = Rc<RefCell<Vec<(&'static str, FigureId)>>>;

struct ExternalLayout(Trace);

impl LayoutManager for ExternalLayout {
    fn validate_constraint(
        &self,
        container: FigureId,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        assert!(!child.is_null());
        assert_eq!(child.namespace(), container.namespace());
        self.0.borrow_mut().push(("validate", child));
        assert!(
            !constraint.as_any().is::<PanicConstraint>(),
            "validator panic"
        );
        if constraint.as_any().is::<Placement>() {
            Ok(())
        } else {
            Err(LayoutError::UnsupportedConstraint {
                container,
                child,
                actual: constraint.type_name(),
            })
        }
    }

    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        FigureMeasurement::new(100.0, 100.0, None)
    }

    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSnapshot<'_>,
    ) -> Dimension {
        Dimension::new(100.0, 100.0)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        for (child, _) in snapshot.children(container) {
            if let Some(placement) = snapshot.constraint_as::<Placement>(container, child)? {
                out.set_child_bounds(child, placement.0);
            }
        }
        Ok(())
    }
}

struct ExternalFigure(Trace);

impl Figure for ExternalFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(10.0, 10.0, 20.0, 20.0)
    }

    fn name(&self) -> &'static str {
        "ExternalFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut novadraw::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::LIFECYCLE,
            novadraw::LifecycleCapability::of::<Self>(),
        )
    }
}

impl FigureLifecycle for ExternalFigure {
    fn on_attached(&mut self, context: FigureLifecycleContext) {
        self.0.borrow_mut().push(("attached", context.figure_id));
    }
}

struct Events(Trace);

impl AncestorListener for Events {
    fn ancestor_changed(&self, event: AncestorEvent) -> ListenerDirective {
        if event.kind == AncestorEventKind::Added {
            self.0.borrow_mut().push(("added", event.figure_id));
        }
        ListenerDirective::Keep
    }
}

impl LayoutListener for Events {
    fn layout_changed(&self, event: LayoutEvent) -> ListenerDirective {
        if event.kind == LayoutEventKind::ConstraintChanged {
            self.0
                .borrow_mut()
                .push(("constraint", event.child_id.unwrap()));
        }
        ListenerDirective::Keep
    }
}

fn rectangle() -> Box<dyn Figure> {
    Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0))
}

fn tree_with_layout(trace: Trace) -> (FigureTree, FigureId) {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(rectangle())
        .expect("valid FigureTree construction");
    tree.builder()
        .set_layout_manager(parent, Box::new(ExternalLayout(trace)))
        .unwrap();
    (tree, parent)
}

#[test]
fn builder_and_runtime_insert_preserve_order_and_external_constraints() {
    for build_first in [true, false] {
        let trace = Trace::default();
        let (mut tree, parent) = tree_with_layout(trace.clone());
        let first = tree.builder().add_child(parent, rectangle()).unwrap();
        let last = tree.builder().add_child(parent, rectangle()).unwrap();
        let placement = Placement(Rectangle::new(12.0, 13.0, 30.0, 40.0));
        let mut runtime;
        let inserted;
        if build_first {
            inserted = tree
                .builder()
                .insert_child_with_constraint(
                    parent,
                    1,
                    Box::new(ExternalFigure(trace.clone())),
                    placement,
                )
                .unwrap();
            runtime = Runtime::new(tree);
        } else {
            runtime = Runtime::new(tree);
            inserted = runtime
                .container(parent)
                .unwrap()
                .insert_with_constraint(1, Box::new(ExternalFigure(trace.clone())), placement)
                .unwrap();
        }
        assert_eq!(
            runtime.tree().child_order(parent),
            Some(vec![first, inserted, last])
        );
        assert_eq!(
            runtime.tree().layout_constraint::<Placement>(inserted),
            Some(&placement)
        );
        runtime.prepare_frame().unwrap();
        assert_eq!(runtime.tree().figure_bounds(inserted), Some(placement.0));
        assert_eq!(
            &*trace.borrow(),
            &[("validate", inserted), ("attached", inserted)]
        );
    }
}

#[test]
fn rejected_constraint_never_publishes_node_or_reuses_failed_identity() {
    let trace = Trace::default();
    let (mut tree, parent) = tree_with_layout(trace.clone());
    let error = tree
        .builder()
        .insert_child_with_constraint(
            parent,
            0,
            Box::new(ExternalFigure(trace.clone())),
            "unsupported",
        )
        .unwrap_err();
    let ChildInsertionError::Layout(LayoutError::UnsupportedConstraint { child: failed, .. }) =
        error
    else {
        panic!("expected layout rejection");
    };
    assert!(tree.node(failed).is_none());
    assert_eq!(tree.child_order(parent), Some(vec![]));
    let next = tree.builder().insert_child(parent, 0, rectangle()).unwrap();
    assert_ne!(next, failed);
    assert_eq!(&*trace.borrow(), &[("validate", failed)]);

    let mut runtime = Runtime::new(tree);
    let baseline = runtime
        .prepare_submission(
            SurfaceInfo {
                logical_width: 100.0,
                logical_height: 100.0,
                pixel_width: 100,
                pixel_height: 100,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    assert!(runtime.complete_submission(
        baseline.session_id,
        baseline.frame_id,
        RenderOutcome::Presented
    ));
    assert!(!runtime.has_pending_update());
    let generation = runtime
        .tree()
        .node(parent)
        .unwrap()
        .layout_state()
        .generation();
    runtime.add_ancestor_listener(Box::new(Events(trace.clone())));
    runtime.add_layout_listener(Box::new(Events(trace.clone())));
    trace.borrow_mut().clear();
    let result = runtime.container(parent).unwrap().insert_with_constraint(
        0,
        Box::new(ExternalFigure(trace.clone())),
        "unsupported",
    );
    let Err(RuntimeMutationError::Layout(LayoutError::UnsupportedConstraint { child, .. })) =
        result
    else {
        panic!("expected layout rejection");
    };
    assert!(runtime.tree().node(child).is_none());
    assert_eq!(runtime.tree().child_order(parent), Some(vec![next]));
    assert_eq!(
        runtime
            .tree()
            .node(parent)
            .unwrap()
            .layout_state()
            .generation(),
        generation
    );
    assert!(!runtime.has_pending_update());
    assert!(runtime.prepare_frame().is_none());
    assert_eq!(&*trace.borrow(), &[("validate", child)]);
}

#[test]
fn runtime_publishes_one_added_and_one_constraint_event() {
    let trace = Trace::default();
    let (tree, parent) = tree_with_layout(trace.clone());
    let mut runtime = Runtime::new(tree);
    runtime.prepare_frame().unwrap();
    runtime.add_ancestor_listener(Box::new(Events(trace.clone())));
    runtime.add_layout_listener(Box::new(Events(trace.clone())));
    let child = runtime
        .container(parent)
        .unwrap()
        .insert_with_constraint(
            0,
            Box::new(ExternalFigure(trace.clone())),
            Placement(Rectangle::new(10.0, 10.0, 20.0, 20.0)),
        )
        .unwrap();
    assert!(runtime.has_pending_update());
    assert_eq!(
        &*trace.borrow(),
        &[("validate", child), ("attached", child)]
    );
    runtime.prepare_frame().unwrap();
    assert_eq!(
        &*trace.borrow(),
        &[
            ("validate", child),
            ("attached", child),
            ("added", child),
            ("constraint", child),
        ]
    );
}

#[test]
fn index_boundaries_are_checked_before_constraint_validation() {
    let trace = Trace::default();
    let (mut tree, parent) = tree_with_layout(trace.clone());
    assert!(matches!(
        tree.builder()
            .insert_child_with_constraint(parent, 1, rectangle(), "unsupported"),
        Err(ChildInsertionError::Graph(
            GraphMutationError::InvalidChildIndex { .. }
        ))
    ));
    assert!(trace.borrow().is_empty());
    let last = tree.builder().insert_child(parent, 0, rectangle()).unwrap();
    let first = tree.builder().insert_child(parent, 0, rectangle()).unwrap();
    let mut runtime = Runtime::new(tree);
    let appended = runtime
        .container(parent)
        .unwrap()
        .insert(2, rectangle())
        .unwrap();
    assert!(
        runtime
            .container(parent)
            .unwrap()
            .insert(4, rectangle())
            .is_err()
    );
    assert_eq!(
        runtime.tree().child_order(parent),
        Some(vec![first, last, appended])
    );
    assert_eq!(runtime.tree().hit_test_simple((30.0, 30.0)), Some(appended));
}

#[test]
fn constraint_without_manager_is_retained_and_checked_on_manager_installation() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(rectangle())
        .expect("valid FigureTree construction");
    let placement = Placement(Rectangle::new(1.0, 2.0, 3.0, 4.0));
    let child = tree
        .builder()
        .insert_child_with_constraint(parent, 0, rectangle(), placement)
        .unwrap();
    assert!(
        tree.builder()
            .set_layout_manager(parent, Box::new(XYLayout::new()))
            .is_err()
    );
    assert_eq!(tree.layout_constraint::<Placement>(child), Some(&placement));
    let mut runtime = Runtime::new(tree);
    runtime
        .container(parent)
        .unwrap()
        .set_layout_manager(Box::new(ExternalLayout(Trace::default())))
        .unwrap();
    runtime.prepare_frame().unwrap();
    assert_eq!(runtime.tree().figure_bounds(child), Some(placement.0));
}

struct AdmissionFigure(ChildPolicy);

impl Figure for AdmissionFigure {
    fn name(&self) -> &'static str {
        "AdmissionFigure"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 100.0, 100.0)
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::CONTAINER,
            novadraw::ContainerCapability::of::<Self>(),
        )
    }
}

impl FigureContainer for AdmissionFigure {
    fn child_policy(&self) -> ChildPolicy {
        self.0
    }
}

struct InvalidInitialBoundsFigure(Rectangle);

impl Figure for InvalidInitialBoundsFigure {
    fn name(&self) -> &'static str {
        "InvalidInitialBoundsFigure"
    }

    fn initial_bounds(&self) -> Rectangle {
        self.0
    }
}

#[test]
fn initial_bounds_are_rejected_before_topology_publication() {
    for bounds in [
        Rectangle::new(f64::NAN, 0.0, 10.0, 10.0),
        Rectangle::new(0.0, f64::INFINITY, 10.0, 10.0),
        Rectangle::new(0.0, 0.0, -1.0, 10.0),
        Rectangle::new(0.0, 0.0, 10.0, -1.0),
    ] {
        let mut tree = FigureTree::new();
        assert_eq!(
            tree.builder()
                .set_contents(Box::new(InvalidInitialBoundsFigure(bounds))),
            Err(GraphMutationError::InvalidInitialBounds)
        );
        assert_eq!(tree.contents(), None);

        let parent = tree
            .builder()
            .set_contents(rectangle())
            .expect("valid FigureTree construction");
        assert_eq!(
            tree.builder()
                .add_child(parent, Box::new(InvalidInitialBoundsFigure(bounds))),
            Err(GraphMutationError::InvalidInitialBounds)
        );
        assert_eq!(tree.child_order(parent), Some(vec![]));
    }
}

#[test]
fn admission_and_target_errors_do_not_reach_constraint_validator() {
    for policy in [ChildPolicy::Single, ChildPolicy::Layered] {
        let trace = Trace::default();
        let mut tree = FigureTree::new();
        let parent = tree
            .builder()
            .set_contents(Box::new(AdmissionFigure(policy)))
            .expect("valid FigureTree construction");
        tree.builder()
            .set_layout_manager(parent, Box::new(ExternalLayout(trace.clone())))
            .unwrap();
        if policy == ChildPolicy::Single {
            tree.builder().insert_child(parent, 0, rectangle()).unwrap();
        }
        let before = tree.child_order(parent);
        assert!(matches!(
            tree.builder()
                .insert_child_with_constraint(parent, 0, rectangle(), "unused"),
            Err(ChildInsertionError::Graph(
                GraphMutationError::ChildLimitExceeded { .. }
                    | GraphMutationError::LayerKeyRequired
            ))
        ));
        assert_eq!(tree.child_order(parent), before);
        let mut runtime = Runtime::new(tree);
        assert!(matches!(
            runtime
                .container(parent)
                .unwrap()
                .insert_with_constraint(0, rectangle(), "unused"),
            Err(
                RuntimeMutationError::Graph(GraphMutationError::ChildLimitExceeded { .. })
                    | RuntimeMutationError::LayeredParent(_)
            )
        ));
        assert_eq!(runtime.tree().child_order(parent), before);
        assert!(trace.borrow().is_empty());
    }
    let mut foreign = FigureTree::new();
    let foreign_parent = foreign
        .builder()
        .set_contents(rectangle())
        .expect("valid FigureTree construction");
    let (mut tree, parent) = tree_with_layout(Trace::default());
    assert_eq!(
        tree.builder().insert_child(foreign_parent, 0, rectangle()),
        Err(GraphMutationError::ParentNotFound)
    );
    let child = tree.builder().insert_child(parent, 0, rectangle()).unwrap();
    let mut runtime = Runtime::new(tree);
    assert!(matches!(
        runtime.container(foreign_parent),
        Err(RuntimeMutationError::ForeignRuntime(_))
    ));
    assert!(matches!(
        runtime.container(runtime.tree().root_id()),
        Err(RuntimeMutationError::SyntheticRootOperation(_))
    ));
    runtime.container(parent).unwrap().remove(child).unwrap();
    assert!(matches!(
        runtime.container(child),
        Err(RuntimeMutationError::UnknownOrDisposedFigure(_))
    ));
}

#[test]
fn insertion_keeps_the_depth_limit() {
    let mut tree = FigureTree::new();
    let mut parent = tree
        .builder()
        .set_contents(rectangle())
        .expect("valid FigureTree construction");
    for _ in 1..novadraw::tree::MAX_TREE_DEPTH {
        parent = tree.builder().insert_child(parent, 0, rectangle()).unwrap();
    }
    assert_eq!(
        tree.builder().insert_child(parent, 0, rectangle()),
        Err(GraphMutationError::DepthLimitExceeded {
            limit: novadraw::tree::MAX_TREE_DEPTH
        })
    );
    assert_eq!(tree.child_order(parent), Some(vec![]));
}

#[test]
fn validator_panic_retires_unpublished_identity_and_faults_runtime() {
    let trace = Trace::default();
    let (tree, parent) = tree_with_layout(trace.clone());
    let mut runtime = Runtime::new(tree);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.container(parent).unwrap().insert_with_constraint(
            0,
            Box::new(ExternalFigure(trace.clone())),
            PanicConstraint,
        )
    }));
    assert!(result.is_err());
    let failed = trace.borrow()[0].1;
    assert!(runtime.tree().node(failed).is_none());
    assert_eq!(runtime.tree().child_order(parent), Some(vec![]));
    assert!(matches!(
        runtime.container(parent),
        Err(RuntimeMutationError::Faulted)
    ));
    assert_eq!(&*trace.borrow(), &[("validate", failed)]);
}
