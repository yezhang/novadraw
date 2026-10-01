use novadraw::geometry::Rectangle;
use novadraw::{
    FigureTree, FlowPage, LineBorder, RectangleFigure, Runtime, RuntimeMutationError, StackLayout,
    TextFlowFigure,
};

fn runtime_with_child() -> (Runtime, novadraw::FigureId, novadraw::FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 160.0)));
    let child = tree
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 12.0, 40.0, 30.0)))
        .unwrap();
    (Runtime::new(tree), root, child)
}

#[test]
fn builder_and_scoped_editors_cover_distinct_lifecycle_phases() {
    let root_bounds = Rectangle::new(0.0, 0.0, 80.0, 60.0);
    let child_bounds = Rectangle::new(8.0, 9.0, 24.0, 28.0);

    let detached =
        RectangleFigure::from_bounds(root_bounds).with_border(LineBorder::default_border());
    let mut builder_tree = FigureTree::new();
    let builder_root = builder_tree.builder().set_contents(Box::new(detached));
    let builder_child = builder_tree
        .builder()
        .add_child(
            builder_root,
            Box::new(RectangleFigure::new(1.0, 2.0, 10.0, 12.0)),
        )
        .unwrap();
    builder_tree
        .builder()
        .set_layout_manager(builder_root, Box::new(StackLayout::new()))
        .unwrap();
    builder_tree
        .builder()
        .set_bounds(builder_child, child_bounds)
        .unwrap();

    let mut runtime_tree = FigureTree::new();
    let runtime_root = runtime_tree
        .builder()
        .set_contents(Box::new(RectangleFigure::from_bounds(root_bounds)));
    let mut runtime = Runtime::new(runtime_tree);
    let runtime_child = runtime
        .container(runtime_root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(1.0, 2.0, 10.0, 12.0)))
        .unwrap();
    runtime
        .container(runtime_root)
        .unwrap()
        .set_layout_manager(Box::new(StackLayout::new()))
        .unwrap();
    assert!(
        runtime
            .figure(runtime_child)
            .unwrap()
            .set_bounds(child_bounds)
            .unwrap()
    );

    assert_eq!(
        builder_tree
            .child_order(builder_root)
            .map(|order| order.len()),
        runtime
            .tree()
            .child_order(runtime_root)
            .map(|order| order.len())
    );
    assert_eq!(
        builder_tree.figure_bounds(builder_child),
        runtime.tree().figure_bounds(runtime_child)
    );
    assert_eq!(
        builder_tree.layout_manager(builder_root).is_some(),
        runtime.tree().layout_manager(runtime_root).is_some()
    );
    assert!(runtime.has_pending_update());
}

#[test]
fn scoped_editor_acquisition_rejects_foreign_and_disposed_figures() {
    let (mut runtime, root, child) = runtime_with_child();
    let (foreign_runtime, foreign, _) = runtime_with_child();

    assert!(matches!(
        runtime.figure(foreign),
        Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign
    ));
    drop(foreign_runtime);

    assert!(runtime.container(root).unwrap().remove(child).unwrap());
    assert!(matches!(
        runtime.figure(child),
        Err(RuntimeMutationError::UnknownOrDisposedFigure(id)) if id == child
    ));
}

#[test]
fn scoped_editor_acquisition_rejects_replaced_contents() {
    let mut tree = FigureTree::new();
    let replaced = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    let attached = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    let mut runtime = Runtime::new(tree);

    assert!(runtime.figure(attached).is_ok());
    assert!(matches!(
        runtime.figure(replaced),
        Err(RuntimeMutationError::UnknownOrDisposedFigure(id)) if id == replaced
    ));
}

#[test]
fn specialized_editor_rejects_a_figure_without_the_capability() {
    let (mut runtime, root, child) = runtime_with_child();
    let text_flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(TextFlowFigure::new(
            Rectangle::new(0.0, 0.0, 20.0, 20.0),
            FlowPage::from_text("text"),
        )))
        .unwrap();
    let pending_before = runtime.has_pending_update();
    let bounds_before = runtime.tree().figure_bounds(child);

    assert!(matches!(
        runtime.viewport(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "viewport mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.scalable(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "scale mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.label(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "label mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.clickable(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "clickable mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.image(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "image mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.point_list(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "point-list mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.scalable_polygon(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "scalable-polygon mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.text_flow(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "text-flow mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.border(text_flow),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "border mutation",
        }) if figure == text_flow
    ));
    assert!(matches!(
        runtime.rounded_rectangle(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "rounded-rectangle mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.triangle(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "triangle mutation",
        }) if figure == child
    ));
    assert_eq!(runtime.has_pending_update(), pending_before);
    assert_eq!(runtime.tree().figure_bounds(child), bounds_before);
}
