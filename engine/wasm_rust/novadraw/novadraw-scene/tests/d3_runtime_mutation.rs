use novadraw_render::command::RenderCommandKind;
use novadraw_scene::{
    BorderConstraint, BorderRegion, ChildClippingStrategy, ChildPolicy, EventContext, Figure,
    FigureContainer, FigureEventHandler, FigureId, FigureTree, GraphMutationError, GridLayout,
    LayerFigure, LayerKey, LayerPlacement, LayoutError, MouseButton, MouseEvent, Rectangle,
    RectangleFigure, Runtime, RuntimeMutationError, XYConstraint, XYLayout,
};

#[test]
fn runtime_replaces_layout_and_constraints_only_after_validation() {
    let (mut runtime, root, child) = runtime_with_child();

    assert!(
        runtime
            .set_layout_manager(root, Box::new(XYLayout::new()))
            .unwrap()
    );
    assert!(
        runtime
            .set_layout_constraint(child, XYConstraint::at_size(10.0, 20.0, 30.0, 40.0))
            .unwrap()
    );

    let incompatible_constraint = runtime
        .set_layout_constraint(child, BorderConstraint::new(BorderRegion::Center))
        .unwrap_err();
    assert!(matches!(
        incompatible_constraint,
        RuntimeMutationError::Layout(LayoutError::ConstraintTypeMismatch { .. })
    ));
    assert_eq!(
        runtime.tree().layout_constraint::<XYConstraint>(child),
        Some(&XYConstraint::at_size(10.0, 20.0, 30.0, 40.0))
    );

    let incompatible_manager = runtime
        .set_layout_manager(root, Box::new(GridLayout::new(2)))
        .unwrap_err();
    assert!(matches!(
        incompatible_manager,
        RuntimeMutationError::Layout(LayoutError::ConstraintTypeMismatch { .. })
    ));
    assert!(
        runtime
            .set_layout_constraint(child, XYConstraint::at_size(30.0, 40.0, 50.0, 60.0))
            .unwrap()
    );

    assert!(runtime.remove_layout_constraint(child).unwrap());
    assert!(
        runtime
            .set_layout_manager(root, Box::new(GridLayout::new(2)))
            .unwrap()
    );
    assert!(runtime.clear_layout_manager(root).unwrap());
    assert!(!runtime.clear_layout_manager(root).unwrap());
}

#[test]
fn runtime_size_overrides_are_checked_and_clearable() {
    let (mut runtime, _root, child) = runtime_with_child();

    assert!(runtime.set_preferred_size(child, (50.0, 60.0)).unwrap());
    assert!(runtime.set_minimum_size(child, (20.0, 30.0)).unwrap());
    assert!(runtime.set_maximum_size(child, (100.0, 120.0)).unwrap());
    assert_eq!(
        runtime.tree().preferred_size(child, -1.0, -1.0),
        Some((50.0, 60.0))
    );
    assert_eq!(
        runtime.tree().minimum_size(child, -1.0, -1.0),
        Some((20.0, 30.0))
    );
    assert_eq!(runtime.tree().maximum_size(child), Some((100.0, 120.0)));

    let invalid = runtime
        .set_preferred_size(child, (f64::NAN, 10.0))
        .unwrap_err();
    assert!(matches!(
        invalid,
        RuntimeMutationError::InvalidSize { figure, .. } if figure == child
    ));
    assert_eq!(
        runtime.tree().preferred_size(child, -1.0, -1.0),
        Some((50.0, 60.0))
    );

    assert!(runtime.clear_preferred_size(child).unwrap());
    assert!(runtime.clear_minimum_size(child).unwrap());
    assert!(runtime.clear_maximum_size(child).unwrap());
    assert_eq!(
        runtime.tree().preferred_size(child, -1.0, -1.0),
        Some((30.0, 40.0))
    );
    assert_eq!(
        runtime.tree().minimum_size(child, -1.0, -1.0),
        Some((30.0, 40.0))
    );
    assert_eq!(
        runtime.tree().maximum_size(child),
        Some((f64::INFINITY, f64::INFINITY))
    );
}

#[test]
fn runtime_node_mutations_distinguish_noop_invalid_bounds_and_foreign_figures() {
    let (mut runtime, root, _) = runtime_with_child();
    let mut foreign = Runtime::empty();
    let foreign_figure = foreign
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let original = runtime.tree().figure_bounds(root).unwrap();

    assert_eq!(runtime.set_visible(root, true), Ok(false));
    assert_eq!(
        runtime.set_visible(foreign_figure, false),
        Err(RuntimeMutationError::ForeignRuntime(foreign_figure))
    );

    let invalid = Rectangle::new(0.0, 0.0, f64::NAN, 10.0);
    let Err(RuntimeMutationError::InvalidBounds { figure, bounds }) =
        runtime.set_bounds(root, invalid)
    else {
        panic!("invalid bounds must return RuntimeMutationError::InvalidBounds");
    };
    assert_eq!(figure, root);
    assert_eq!((bounds.x, bounds.y, bounds.height), (0.0, 0.0, 10.0));
    assert!(bounds.width.is_nan());
    assert_eq!(runtime.tree().figure_bounds(root), Some(original));
}

#[test]
fn runtime_child_order_controls_paint_and_reverse_hit_order_atomically() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let bottom = tree
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        )
        .expect("valid FigureTree construction");
    let top = tree
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    assert_eq!(runtime.tree().hit_test_simple((30.0, 30.0)), Some(top));
    assert!(runtime.bring_child_to_front(root, bottom).unwrap());
    assert_eq!(runtime.tree().child_order(root), Some(vec![top, bottom]));
    assert_eq!(runtime.tree().hit_test_simple((30.0, 30.0)), Some(bottom));
    assert!(!runtime.set_child_order(root, &[top, bottom]).unwrap());

    let before = runtime.tree().child_order(root);
    let duplicate = runtime.set_child_order(root, &[top, top]).unwrap_err();
    assert_eq!(
        duplicate,
        RuntimeMutationError::InvalidChildOrder { parent: root }
    );
    assert_eq!(runtime.tree().child_order(root), before);

    let invalid = runtime.move_child_to_index(root, bottom, 2).unwrap_err();
    assert_eq!(
        invalid,
        RuntimeMutationError::InvalidChildIndex {
            parent: root,
            index: 2,
            child_count: 2,
        }
    );
    assert_eq!(runtime.tree().child_order(root), before);
    assert!(runtime.set_child_order(root, &[bottom, top]).unwrap());
    assert_eq!(runtime.tree().child_order(root), Some(vec![bottom, top]));
    assert!(runtime.bring_child_to_front(root, bottom).unwrap());

    let mut expected = FigureTree::new();
    let expected_root = expected
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let expected_bottom = expected
        .builder()
        .add_child(
            expected_root,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        )
        .expect("valid FigureTree construction");
    let expected_top = expected
        .builder()
        .add_child(
            expected_root,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        )
        .expect("valid FigureTree construction");
    assert!(
        expected
            .builder()
            .bring_child_to_front(expected_root, expected_bottom)
            .unwrap()
    );
    assert_eq!(
        runtime
            .tree()
            .child_order(root)
            .unwrap()
            .iter()
            .map(|id| *id == bottom)
            .collect::<Vec<_>>(),
        expected
            .child_order(expected_root)
            .unwrap()
            .iter()
            .map(|id| *id == expected_bottom)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        expected.child_z_index(expected_root, expected_top),
        runtime.tree().child_z_index(root, top)
    );
}

#[test]
fn runtime_clipping_replacement_changes_render_protocol_and_forces_full_damage() {
    let (mut runtime, root, child) = runtime_with_child();
    let initial = runtime.prepare_frame().expect("initial frame");
    let initial_clips = clip_count(&initial);

    assert!(
        runtime
            .set_bounds(child, Rectangle::new(20.0, 20.0, 30.0, 40.0))
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .set_child_clipping_strategy(root, ChildClippingStrategy::OverflowVisible)
            .unwrap()
    );
    assert_eq!(
        runtime.tree().child_clipping_strategy(root),
        Some(ChildClippingStrategy::OverflowVisible)
    );
    assert_eq!(
        runtime
            .tree()
            .node(root)
            .unwrap()
            .state()
            .child_clipping_strategy_override(),
        Some(ChildClippingStrategy::OverflowVisible)
    );
    let changed = runtime.prepare_frame().expect("clipping replacement frame");
    assert!(changed.damage().is_full());
    assert!(clip_count(&changed) < initial_clips);
}

#[test]
fn ordinary_child_order_api_rejects_layered_panes() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let mut runtime = Runtime::new(tree);
    let (pane, first) = {
        let mut handle = runtime
            .add_layered_pane(root, Rectangle::new(0.0, 0.0, 200.0, 200.0))
            .unwrap();
        let pane = handle.pane_id();
        let first = handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 200.0)),
                LayerKey::new("first").unwrap(),
                LayerPlacement::Last,
            )
            .unwrap();
        handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 200.0)),
                LayerKey::new("second").unwrap(),
                LayerPlacement::Last,
            )
            .unwrap();
        (pane, first)
    };

    assert_eq!(
        runtime.bring_child_to_front(pane, first),
        Err(RuntimeMutationError::LayeredParent(pane))
    );
}

#[test]
fn checked_topology_mutations_preserve_error_categories() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let single = tree
        .builder()
        .add_child(root, Box::new(SingleChildFigure))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    runtime
        .add_figure(single, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();

    assert_eq!(
        runtime.add_figure(single, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),),
        Err(RuntimeMutationError::Graph(
            GraphMutationError::ChildLimitExceeded { limit: 1 }
        ))
    );

    let (foreign_runtime, foreign_root, _) = runtime_with_child();
    assert_eq!(
        runtime.reparent(root, foreign_root),
        Err(RuntimeMutationError::ForeignRuntime(foreign_root))
    );
    drop(foreign_runtime);

    let child = runtime
        .add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    runtime.dispose_subtree(child).unwrap();
    assert_eq!(
        runtime.remove_figure(root, child),
        Err(RuntimeMutationError::UnknownOrDisposedFigure(child))
    );
}

#[test]
fn callback_mutations_preserve_fifo_and_report_failure_without_losing_suffix() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)));
    let child = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 10.0, 30.0, 40.0)))
        .expect("valid FigureTree construction");
    tree.builder()
        .add_child(
            root,
            Box::new(DeferredMutationFigure {
                bounds: Rectangle::new(200.0, 20.0, 50.0, 50.0),
                parent: root,
                child,
            }),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    runtime.dispatch_mouse_pressed(210.0, 30.0, MouseButton::Left);

    assert_eq!(
        runtime.tree().preferred_size(child, -1.0, -1.0),
        Some((60.0, 70.0))
    );
    assert_eq!(
        runtime.tree().minimum_size(child, -1.0, -1.0),
        Some((20.0, 30.0))
    );
    assert_eq!(
        runtime.take_deferred_mutation_errors(),
        vec![RuntimeMutationError::InvalidChildIndex {
            parent: root,
            index: 99,
            child_count: 2,
        }]
    );
    assert!(runtime.take_deferred_mutation_errors().is_empty());
}

fn runtime_with_child() -> (Runtime, FigureId, FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let child = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .expect("valid FigureTree construction");
    (Runtime::new(tree), root, child)
}

fn clip_count(canvas: &novadraw_render::NdCanvas) -> usize {
    canvas
        .commands()
        .iter()
        .filter(|command| matches!(command.kind, RenderCommandKind::Clip { .. }))
        .count()
}

struct DeferredMutationFigure {
    bounds: Rectangle,
    parent: FigureId,
    child: FigureId,
}

impl Figure for DeferredMutationFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "DeferredMutationFigure"
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for DeferredMutationFigure {
    fn on_mouse_pressed(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        context.set_preferred_size_later(self.child, (40.0, 50.0));
        context.move_child_to_index_later(self.parent, self.child, 99);
        context.set_preferred_size_later(self.child, (60.0, 70.0));
        context.set_minimum_size_later(self.child, (20.0, 30.0));
        true
    }
}

struct SingleChildFigure;

impl Figure for SingleChildFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 100.0, 100.0)
    }

    fn name(&self) -> &'static str {
        "SingleChildFigure"
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }
}

impl FigureContainer for SingleChildFigure {
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }
}
