use novadraw::figure::border::LineBorder;
use novadraw::figure::{FlowPage, TextFlowFigure};
use novadraw::geometry::Rectangle;
use novadraw::layout::StackLayout;
use novadraw::runtime::{
    CapabilityUpdateError, ComponentUpdateError, ReplaceFigureBorder, ReplacePolylinePoints,
    RuntimeMutationError, SetClickableSelected, SetImageAlignment, SetLabelText,
    SetRoundedRectangleCorners, SetScalablePolygonScaleMode, SetTextFlowWrapping,
    SetTriangleDirection,
};
use novadraw::{
    figure::{Alignment, Direction, PolygonScaleMode, RectangleFigure},
    geometry::{Dimension, Point},
    runtime::Runtime,
    tree::FigureTree,
};

fn runtime_with_child() -> (Runtime, novadraw::tree::FigureId, novadraw::tree::FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 160.0)))
        .expect("valid FigureTree construction");
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
    let builder_root = builder_tree
        .builder()
        .set_contents(Box::new(detached))
        .expect("valid FigureTree construction");
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
        .set_contents(Box::new(RectangleFigure::from_bounds(root_bounds)))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(runtime_tree);
    let runtime_child = runtime
        .figure(runtime_root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(1.0, 2.0, 10.0, 12.0)))
        .unwrap();
    runtime
        .figure(runtime_root)
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
        builder_tree.layout_manager(builder_root).unwrap().is_some(),
        runtime
            .tree()
            .layout_manager(runtime_root)
            .unwrap()
            .is_some()
    );
    assert!(runtime.frames().has_pending_update());
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

    assert!(runtime.figure(root).unwrap().remove(child).unwrap());
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    let attached = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
        .expect("valid FigureTree construction");
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
        .figure(root)
        .unwrap()
        .add(Box::new(TextFlowFigure::new(
            Rectangle::new(0.0, 0.0, 20.0, 20.0),
            FlowPage::from_text("text"),
        )))
        .unwrap();
    let pending_before = runtime.frames().has_pending_update();
    let bounds_before = runtime.tree().figure_bounds(child);

    assert!(matches!(
        runtime.viewports().handle(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "viewport mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime.viewports().scale_handle(child),
        Err(RuntimeMutationError::WrongCapability {
            figure,
            capability: "scale mutation",
        }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetLabelText::new("wrong")),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_capability(SetClickableSelected::new(true)),
        Err(CapabilityUpdateError::Missing { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetImageAlignment::new(Alignment::End)),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(ReplacePolylinePoints::new(vec![Point::new(0.0, 0.0)])),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetScalablePolygonScaleMode::new(PolygonScaleMode::PreserveAspect)),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetTextFlowWrapping::new(
                novadraw::figure::FlowWrapping::NoWrap,
            )),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(text_flow)
            .unwrap()
            .update_capability(ReplaceFigureBorder::clear()),
        Err(CapabilityUpdateError::Missing { figure, .. }) if figure == text_flow
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetRoundedRectangleCorners::new(Dimension::new(1.0, 1.0))),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert!(matches!(
        runtime
            .figure(child)
            .unwrap()
            .update_component(SetTriangleDirection::new(Direction::West)),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == child
    ));
    assert_eq!(runtime.frames().has_pending_update(), pending_before);
    assert_eq!(runtime.tree().figure_bounds(child), bounds_before);
}
