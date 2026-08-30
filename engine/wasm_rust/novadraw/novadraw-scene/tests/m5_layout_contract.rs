use novadraw_geometry::Rectangle;
use novadraw_scene::{
    FigureGraph, GridAlignment, GridConstraint, GridLayout, RectangleFigure, SceneUpdateManager,
    StackLayout, ToolbarLayout,
};

fn assert_rect(actual: Option<Rectangle>, expected: Rectangle) {
    assert_eq!(actual, Some(expected));
}

#[test]
fn stack_layout_places_every_child_in_the_client_area() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(10.0, 20.0, 200.0, 100.0)));
    let first = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 30.0)));
    let second = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 50.0)));
    graph.set_block_layout_manager(root, Box::new(StackLayout::new()));

    graph.revalidate(root);

    let expected = Rectangle::new(10.0, 20.0, 200.0, 100.0);
    assert_rect(graph.figure_bounds(first), expected);
    assert_rect(graph.figure_bounds(second), expected);
}

#[test]
fn toolbar_layout_compresses_main_axis_and_stretches_minor_axis() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 170.0, 60.0)));
    let first = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)));
    let second = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 30.0)));
    graph.set_minimum_size(first, Some((60.0, 10.0)));
    graph.set_minimum_size(second, Some((100.0, 10.0)));
    graph.set_block_layout_manager(
        root,
        Box::new(
            ToolbarLayout::horizontal()
                .with_spacing(10.0)
                .with_stretch_minor_axis(true),
        ),
    );

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(first),
        Rectangle::new(0.0, 0.0, 60.0, 60.0),
    );
    assert_rect(
        graph.figure_bounds(second),
        Rectangle::new(70.0, 0.0, 100.0, 60.0),
    );
}

#[test]
fn grid_layout_uses_track_maxima_and_fill_alignment() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let first = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)));
    let second = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 30.0)));
    let third = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 60.0, 25.0)));
    let fill_cell = GridConstraint {
        horizontal_alignment: GridAlignment::Fill,
        vertical_alignment: GridAlignment::Fill,
        ..GridConstraint::default()
    };
    graph.set_constraint(first, fill_cell);
    graph.set_constraint(second, fill_cell);
    graph.set_constraint(third, fill_cell);
    graph.set_block_layout_manager(
        root,
        Box::new(
            GridLayout::new(2)
                .with_margins(0.0, 0.0)
                .with_spacing(10.0, 10.0),
        ),
    );

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(first),
        Rectangle::new(0.0, 0.0, 60.0, 30.0),
    );
    assert_rect(
        graph.figure_bounds(second),
        Rectangle::new(70.0, 0.0, 50.0, 30.0),
    );
    assert_rect(
        graph.figure_bounds(third),
        Rectangle::new(0.0, 40.0, 60.0, 25.0),
    );
}

#[test]
fn grid_layout_honors_column_span_and_excess_space() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 210.0, 100.0)));
    let spanning = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)));
    let trailing = graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)));
    graph.set_constraint(spanning, GridConstraint::fill().with_span(2, 1));
    graph.set_constraint(trailing, GridConstraint::fill());
    graph.set_block_layout_manager(
        root,
        Box::new(
            GridLayout::new(2)
                .with_equal_column_widths(true)
                .with_margins(0.0, 0.0)
                .with_spacing(10.0, 10.0),
        ),
    );

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(spanning),
        Rectangle::new(0.0, 0.0, 210.0, 45.0),
    );
    assert_rect(
        graph.figure_bounds(trailing),
        Rectangle::new(0.0, 55.0, 100.0, 45.0),
    );
}

#[test]
fn update_manager_completes_a_1024_figure_layout_transaction() {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1024.0, 1024.0)));
    let mut children = Vec::new();
    for _ in 0..1024 {
        children
            .push(graph.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0))));
    }
    graph.set_block_layout_manager(
        root,
        Box::new(
            GridLayout::new(32)
                .with_margins(0.0, 0.0)
                .with_spacing(1.0, 1.0),
        ),
    );
    let mut update_manager = SceneUpdateManager::new();
    graph.mark_invalid(&mut update_manager, root);

    let canvas = graph.perform_update(&mut update_manager);

    assert!(graph.is_valid(root));
    assert!(children.into_iter().all(|child| graph.is_valid(child)));
    assert!(!update_manager.is_update_queued());
    assert!(!canvas.damage().is_empty());
    assert!(!canvas.commands().is_empty());
}
