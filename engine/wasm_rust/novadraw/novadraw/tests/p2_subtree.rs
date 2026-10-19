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
fn paired_binding_is_published_with_the_complete_subtree_and_retired_with_it() {
    use novadraw::{
        connection::{
            AnchorError, AnchorSite, ChopboxAnchor, ConnectionAnchor, ConnectionFigure,
            ConnectionId, CoordinateSpace, RouteSceneQuery, RouterBinding,
        },
        geometry::Point,
        tree::{SubtreeBindingNodes, SubtreeConnection},
    };
    struct ExternalAnchor(ChopboxAnchor);
    impl ConnectionAnchor for ExternalAnchor {
        fn owner(&self) -> Option<FigureId> {
            self.0.owner()
        }
        fn reference_point(
            &self,
            scene: &mut dyn RouteSceneQuery,
            output: CoordinateSpace,
        ) -> Result<Point, AnchorError> {
            self.0.reference_point(scene, output)
        }
        fn location(
            &self,
            scene: &mut dyn RouteSceneQuery,
            reference: Point,
            space: CoordinateSpace,
            output: CoordinateSpace,
        ) -> Result<AnchorSite, AnchorError> {
            self.0.location(scene, reference, space, output)
        }
    }
    let (mut runtime, root) = runtime();
    let router = runtime.connections().direct_router();
    let mut subtree = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let source = subtree
        .add(subtree.root(), Box::new(LabelFigure::new("source")))
        .unwrap();
    let target = subtree
        .add(subtree.root(), Box::new(LabelFigure::new("target")))
        .unwrap();
    let line = subtree
        .add(subtree.root(), Box::new(ConnectionFigure::new()))
        .unwrap();
    subtree
        .set_connection_binding(line, move |nodes: &SubtreeBindingNodes| {
            Ok(SubtreeConnection {
                source: Some(Box::new(ExternalAnchor(ChopboxAnchor::new(
                    nodes.figure(source).unwrap(),
                )))),
                target: Some(Box::new(ChopboxAnchor::new(nodes.figure(target).unwrap()))),
                router: RouterBinding::Explicit { router },
                constraint: None,
            })
        })
        .unwrap();
    let receipt = runtime.figure(root).unwrap().add_subtree(subtree).unwrap();
    let connection = ConnectionId::from_figure(receipt.figure(line).unwrap());
    let state = runtime.connections().state(connection).unwrap();
    assert!(state.source.is_some());
    assert!(state.target.is_some());
    runtime.dispose_subtree(receipt.root()).unwrap();
    assert!(runtime.connections().state(connection).is_err());
    assert!(
        runtime
            .connections()
            .remove_anchor(state.source.unwrap())
            .is_err()
    );
    assert!(
        runtime
            .connections()
            .remove_anchor(state.target.unwrap())
            .is_err()
    );
}

#[test]
fn last_binding_rejection_retires_reserved_ids_without_publishing_a_connection() {
    use novadraw::{
        connection::{ConnectionFigure, ConnectionId, ConnectionRuntimeError, RouterBinding},
        tree::{SubtreeBindingNodes, SubtreeConnection},
    };
    let (mut runtime, root) = runtime();
    let mut foreign_runtime = Runtime::empty();
    let foreign_router = foreign_runtime.connections().direct_router();
    let router = runtime.connections().direct_router();
    let before = runtime.tree().descendant_ids(root).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut subtree = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    for router in [router, foreign_router] {
        let line = subtree
            .add(subtree.root(), Box::new(ConnectionFigure::new()))
            .unwrap();
        let observed = observed.clone();
        subtree
            .set_connection_binding(line, move |nodes: &SubtreeBindingNodes| {
                observed.lock().unwrap().push(nodes.figure(line).unwrap());
                Ok(SubtreeConnection {
                    source: None,
                    target: None,
                    router: RouterBinding::Explicit { router },
                    constraint: None,
                })
            })
            .unwrap();
    }
    assert!(matches!(
        runtime.figure(root).unwrap().add_subtree(subtree),
        Err(RuntimeMutationError::FigureBatch { index: 2, source })
            if matches!(*source, RuntimeMutationError::Connection(ConnectionRuntimeError::UnknownRouter(_)))
    ));
    assert_eq!(runtime.tree().descendant_ids(root).unwrap(), before);
    for figure in observed.lock().unwrap().iter().copied() {
        assert!(runtime.tree().is_attached(figure).is_err());
        assert!(
            runtime
                .connections()
                .state(ConnectionId::from_figure(figure))
                .is_err()
        );
    }
    assert!(!runtime.is_faulted());
}

#[test]
fn binding_rejects_non_connection_foreign_owner_and_constraint_without_replacing_scene() {
    use novadraw::{
        connection::{ChopboxAnchor, ConnectionFigure, RouterBinding},
        tree::{SubtreeBindingNodes, SubtreeConnection},
    };
    struct Constraint;
    let (mut runtime, root) = runtime();
    let router = runtime.connections().direct_router();
    for case in 0..3 {
        let mut subtree =
            FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let node = subtree
            .add(
                subtree.root(),
                if case == 0 {
                    Box::new(LabelFigure::new("invalid")) as Box<dyn Figure>
                } else {
                    Box::new(ConnectionFigure::new())
                },
            )
            .unwrap();
        subtree
            .set_connection_binding(node, move |_: &SubtreeBindingNodes| {
                Ok(SubtreeConnection {
                    source: (case == 1).then(|| Box::new(ChopboxAnchor::new(root)) as _),
                    target: None,
                    router: RouterBinding::Explicit { router },
                    constraint: (case == 2).then(|| Box::new(Constraint) as _),
                })
            })
            .unwrap();
        assert!(matches!(
            runtime.set_contents_subtree(subtree),
            Err(RuntimeMutationError::FigureBatch { index: 1, .. })
        ));
        assert_eq!(runtime.tree().contents(), Some(root));
        assert!(runtime.tree().descendant_ids(root).unwrap().is_empty());
        assert!(!runtime.is_faulted());
    }
}

#[test]
fn binding_preparation_panic_publishes_no_reserved_nodes() {
    use novadraw::{connection::ConnectionFigure, tree::SubtreeBindingNodes};
    let (mut runtime, root) = runtime();
    let before = runtime.tree().descendant_ids(root).unwrap();
    let mut subtree = FigureSubtree::new(Box::new(ConnectionFigure::new()));
    subtree
        .set_connection_binding(subtree.root(), |_: &SubtreeBindingNodes| {
            panic!("binding preparation");
        })
        .unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.figure(root).unwrap().add_subtree(subtree)
        }))
        .is_err()
    );
    assert_eq!(runtime.tree().descendant_ids(root).unwrap(), before);
    assert!(runtime.is_faulted());
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
    router: novadraw::connection::RouterId,
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
        let mut subtree =
            FigureSubtree::new(Box::new(novadraw::connection::ConnectionFigure::new()));
        let router = self.router;
        subtree
            .set_connection_binding(
                subtree.root(),
                move |_: &novadraw::tree::SubtreeBindingNodes| {
                    Ok(novadraw::tree::SubtreeConnection {
                        source: None,
                        target: None,
                        router: novadraw::connection::RouterBinding::Explicit { router },
                        constraint: None,
                    })
                },
            )
            .unwrap();
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
        let router = runtime.connections().direct_router();
        runtime
            .figure(root)
            .unwrap()
            .add(Box::new(Trigger {
                parent: if foreign { other_root } else { root },
                router,
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
        if let Some(receipt) = receipt.lock().unwrap().as_ref() {
            assert!(
                runtime
                    .connections()
                    .state(novadraw::connection::ConnectionId::from_figure(
                        receipt.root()
                    ))
                    .is_ok()
            );
        }
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

struct ExternalResource(novadraw::render::ResourceId);
impl Figure for ExternalResource {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalResource"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::RESOURCE_DEPENDENCIES,
            novadraw::figure::ResourceDependenciesCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigureResourceDependencies for ExternalResource {
    fn resource_dependencies(&self) -> Vec<novadraw::render::ResourceId> {
        vec![self.0, self.0]
    }
}

#[test]
fn implicit_resources_reject_foreign_and_retired_ids_for_builtin_and_external_figures() {
    use novadraw::figure::{ButtonFigure, ImageFigure, ToggleFigure};
    for retired in [false, true] {
        let (mut runtime, root) = runtime();
        let (mut other, _) = self::runtime();
        let image = if retired {
            let image = runtime.resources().register_image().unwrap();
            runtime.resources().remove(image.resource_id()).unwrap();
            image
        } else {
            other.resources().register_image().unwrap()
        };
        let figures: Vec<Box<dyn Figure>> = vec![
            Box::new(LabelFigure::new("icon").with_icon(image)),
            Box::new(ImageFigure::new(image)),
            Box::new(ButtonFigure::new("button").with_icon(image)),
            Box::new(ToggleFigure::new("toggle").with_icon(image)),
            Box::new(ExternalResource(image.resource_id())),
        ];
        for figure in figures {
            let mut subtree = FigureSubtree::new(Box::new(Single));
            subtree.add(subtree.root(), figure).unwrap();
            assert!(matches!(
                runtime.figure(root).unwrap().add_subtree(subtree),
                Err(RuntimeMutationError::FigureBatch { index: 1, source })
                    if matches!(*source, RuntimeMutationError::Resource(_))
            ));
            assert!(runtime.tree().descendant_ids(root).unwrap().is_empty());
            assert!(!runtime.is_faulted());
        }
    }
}

#[test]
fn pending_resource_is_admitted_and_duplicate_dependencies_register_once() {
    let (mut runtime, root) = runtime();
    let image = runtime.resources().register_image().unwrap();
    let mut subtree = FigureSubtree::new(Box::new(Single));
    let child = subtree
        .add(
            subtree.root(),
            Box::new(ExternalResource(image.resource_id())),
        )
        .unwrap();
    subtree
        .add_resource_dependency(child, image.resource_id())
        .unwrap();
    let receipt = runtime.figure(root).unwrap().add_subtree(subtree).unwrap();
    assert!(
        runtime
            .resources()
            .remove_dependency(image.resource_id(), receipt.figure(child).unwrap())
            .unwrap()
    );
    assert!(
        !runtime
            .resources()
            .remove_dependency(image.resource_id(), receipt.figure(child).unwrap())
            .unwrap()
    );
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

#[test]
fn scene_replacement_preserves_old_tree_on_last_node_rejection() {
    let (mut runtime, old) = runtime();
    let child = runtime.figure(old).unwrap().add(Box::new(Single)).unwrap();
    let (mut other, _) = self::runtime();
    let foreign = other.resources().register_image().unwrap().resource_id();
    let mut subtree = FigureSubtree::new(Box::new(Single));
    subtree.add(subtree.root(), Box::new(Single)).unwrap();
    subtree
        .add(subtree.root(), Box::new(ExternalResource(foreign)))
        .unwrap();
    assert!(matches!(
        runtime.set_contents_subtree(subtree),
        Err(RuntimeMutationError::FigureBatch { index: 2, .. })
    ));
    assert_eq!(runtime.tree().contents(), Some(old));
    assert_eq!(runtime.tree().child_order(old).unwrap(), [child]);
    assert!(!runtime.is_faulted());
}

struct LifecycleRecord {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    panic_on_detach: bool,
}
impl Figure for LifecycleRecord {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        self.name
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
impl novadraw::figure::FigureLifecycle for LifecycleRecord {
    fn on_attached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        self.events
            .lock()
            .unwrap()
            .push(format!("attach {}", self.name));
    }
    fn on_detached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        self.events
            .lock()
            .unwrap()
            .push(format!("detach {}", self.name));
        assert!(!self.panic_on_detach, "retirement fault");
    }
}

#[test]
fn replacement_retires_old_tree_before_new_attachment_and_returns_complete_mapping() {
    let (mut runtime, old) = runtime();
    let events = Arc::new(Mutex::new(Vec::new()));
    let old_child = runtime
        .figure(old)
        .unwrap()
        .add(Box::new(LifecycleRecord {
            name: "old",
            events: events.clone(),
            panic_on_detach: false,
        }))
        .unwrap();
    events.lock().unwrap().clear();
    let mut subtree = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)));
    let root = subtree.root();
    let child = subtree
        .add(
            root,
            Box::new(LifecycleRecord {
                name: "new",
                events: events.clone(),
                panic_on_detach: false,
            }),
        )
        .unwrap();
    subtree
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .unwrap();
    subtree
        .set_style(
            root,
            FigureStyle {
                foreground: Some(Color::WHITE),
                ..FigureStyle::default()
            },
        )
        .unwrap();
    let receipt = runtime.set_contents_subtree(subtree).unwrap();
    assert_eq!(runtime.tree().contents(), Some(receipt.root()));
    assert_eq!(
        runtime.tree().child_order(receipt.root()).unwrap(),
        [receipt.figure(child).unwrap()]
    );
    assert!(runtime.tree().figure_bounds(old).is_err());
    assert!(runtime.tree().figure_bounds(old_child).is_err());
    assert_eq!(*events.lock().unwrap(), ["detach old", "attach new"]);
    assert_eq!(
        runtime
            .tree()
            .resolved_style(receipt.figure(child).unwrap())
            .unwrap()
            .foreground,
        Color::WHITE
    );
}

#[test]
fn replacement_retirement_panic_keeps_new_contents_and_blocks_further_mutation() {
    let (mut runtime, old) = runtime();
    runtime
        .figure(old)
        .unwrap()
        .add(Box::new(LifecycleRecord {
            name: "old",
            events: Arc::new(Mutex::new(Vec::new())),
            panic_on_detach: true,
        }))
        .unwrap();
    let image = runtime.resources().register_image().unwrap();
    let mut subtree = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)));
    subtree
        .add(
            subtree.root(),
            Box::new(ExternalResource(image.resource_id())),
        )
        .unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.set_contents_subtree(subtree)
        }))
        .is_err()
    );
    assert!(runtime.is_faulted());
    let new = runtime.tree().contents().unwrap();
    assert_ne!(new, old);
    assert!(runtime.tree().figure_bounds(old).is_err());
    let child = runtime.tree().child_order(new).unwrap()[0];
    assert!(matches!(
        runtime
            .resources()
            .remove_dependency(image.resource_id(), child),
        Err(novadraw::runtime::ResourceError::Faulted)
    ));
}

struct ReplaceTrigger {
    old: FigureId,
    viewport: Option<FigureId>,
    outcomes: Arc<Mutex<Vec<bool>>>,
    receipt: Arc<Mutex<Option<SubtreeReceipt>>>,
}
impl Figure for ReplaceTrigger {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ReplaceTrigger"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureEventHandler for ReplaceTrigger {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        for valid in [false, true] {
            let mut subtree =
                FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 50.0)));
            subtree
                .add(
                    subtree.root(),
                    Box::new(RectangleFigure::new(
                        0.0,
                        0.0,
                        if valid { 1.0 } else { f64::NAN },
                        1.0,
                    )),
                )
                .unwrap();
            let outcomes = self.outcomes.clone();
            let receipt = self.receipt.clone();
            let completed = move |result: Result<SubtreeReceipt, &RuntimeMutationError>| {
                outcomes.lock().unwrap().push(result.is_ok());
                if let Ok(value) = result {
                    *receipt.lock().unwrap() = Some(value);
                }
            };
            if let Some(viewport) = self.viewport {
                context
                    .effects()
                    .set_viewport_contents_subtree_later(viewport, subtree, completed);
            } else {
                context
                    .effects()
                    .set_contents_subtree_later(subtree, completed);
            }
        }
        // This follows the replacement and must reject its now-retired parent.
        let outcomes = self.outcomes.clone();
        context.effects().add_subtree_later(
            self.old,
            FigureSubtree::new(Box::new(Single)),
            move |result| outcomes.lock().unwrap().push(result.is_ok()),
        );
        assert!(context.scene().children(self.old).is_ok());
        assert!(self.outcomes.lock().unwrap().is_empty());
        true
    }
}

#[test]
fn event_replacement_commits_after_dispatch_and_later_old_target_rejects_in_fifo_order() {
    let (mut runtime, old) = runtime();
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let receipt = Arc::new(Mutex::new(None));
    runtime
        .figure(old)
        .unwrap()
        .add(Box::new(ReplaceTrigger {
            old,
            viewport: None,
            outcomes: outcomes.clone(),
            receipt: receipt.clone(),
        }))
        .unwrap();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(*outcomes.lock().unwrap(), [false, true, false]);
    assert_eq!(runtime.take_deferred_mutation_errors().len(), 2);
    let receipt = receipt.lock().unwrap();
    let new = receipt.as_ref().unwrap().root();
    assert_eq!(runtime.tree().contents(), Some(new));
    assert!(runtime.tree().figure_bounds(old).is_err());
    assert_eq!(runtime.tree().child_order(new).unwrap().len(), 1);
    assert!(!runtime.is_faulted());
}

#[test]
fn single_figure_contents_uses_the_same_resource_admission() {
    let (mut runtime, old) = runtime();
    let (mut other, _) = self::runtime();
    let foreign = other.resources().register_image().unwrap();
    assert!(matches!(
        runtime.set_contents(Box::new(LabelFigure::new("foreign").with_icon(foreign))),
        Err(RuntimeMutationError::Resource(_))
    ));
    assert_eq!(runtime.tree().contents(), Some(old));
}

#[test]
fn viewport_replacement_admits_new_branch_without_removing_old_contents_first() {
    let (mut runtime, root) = runtime();
    let viewport = runtime
        .viewports()
        .add(root, Rectangle::new(0.0, 0.0, 100.0, 100.0))
        .unwrap();
    let id = viewport.figure_id();
    let old = runtime
        .viewports()
        .set_contents(id, Box::new(Single))
        .unwrap();
    assert!(
        runtime
            .viewports()
            .set_contents(id, Box::new(RectangleFigure::new(0.0, 0.0, f64::NAN, 1.0)))
            .is_err()
    );
    assert_eq!(viewport.contents(runtime.tree()), Ok(Some(old)));
    let mut subtree = FigureSubtree::new(Box::new(Single));
    let child = subtree
        .add(subtree.root(), Box::new(LabelFigure::new("new")))
        .unwrap();
    let receipt = runtime
        .viewports()
        .set_contents_subtree(id, subtree)
        .unwrap();
    assert_eq!(viewport.contents(runtime.tree()), Ok(Some(receipt.root())));
    assert_eq!(runtime.tree().child_order(id).unwrap(), [receipt.root()]);
    assert_eq!(
        runtime.tree().child_order(receipt.root()).unwrap(),
        [receipt.figure(child).unwrap()]
    );
    assert!(runtime.tree().figure_bounds(old).is_err());
}

#[test]
fn event_viewport_replacement_preserves_outer_scene_and_rejects_retired_targets() {
    let (mut runtime, root) = runtime();
    let viewport = runtime
        .viewports()
        .add(root, Rectangle::new(0.0, 0.0, 100.0, 100.0))
        .unwrap();
    let id = viewport.figure_id();
    let old = runtime
        .viewports()
        .set_contents(id, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .unwrap();
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let receipt = Arc::new(Mutex::new(None));
    runtime
        .figure(old)
        .unwrap()
        .add(Box::new(ReplaceTrigger {
            old,
            viewport: Some(id),
            outcomes: outcomes.clone(),
            receipt: receipt.clone(),
        }))
        .unwrap();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(*outcomes.lock().unwrap(), [false, true, false]);
    assert_eq!(runtime.tree().contents(), Some(root));
    let receipt = receipt.lock().unwrap();
    assert_eq!(
        viewport.contents(runtime.tree()),
        Ok(Some(receipt.as_ref().unwrap().root()))
    );
    assert!(runtime.tree().figure_bounds(old).is_err());
    assert_eq!(runtime.take_deferred_mutation_errors().len(), 2);
}
