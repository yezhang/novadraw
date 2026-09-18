use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_scene::{
    BorderConstraint, BorderLayout, BorderRegion, FigureId, FigureTree, GridAlignment,
    GridConstraint, GridLayout, LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot,
    LineBorder, RectangleFigure, StackLayout, ToolbarLayout, UpdateManager, ValidationError,
    XYConstraint, XYLayout,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn assert_rect(actual: Option<Rectangle>, expected: Rectangle) {
    assert_eq!(actual, Some(expected));
}

#[test]
fn stack_layout_places_every_child_in_the_client_area() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(10.0, 20.0, 200.0, 100.0)));
    let first = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 30.0)));
    let second = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 50.0)));
    graph.set_block_layout_manager(root, Box::new(StackLayout::new()));

    graph.revalidate(root);

    let expected = Rectangle::new(0.0, 0.0, 200.0, 100.0);
    assert_rect(graph.figure_bounds(first), expected);
    assert_rect(graph.figure_bounds(second), expected);
}

#[test]
fn stack_layout_applies_container_insets_once() {
    let mut graph = FigureTree::new();
    let root = graph.builder().set_contents(Box::new(
        RectangleFigure::new(0.0, 0.0, 100.0, 100.0)
            .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(10.0, 10.0, 10.0, 10.0)),
    ));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    graph.set_block_layout_manager(root, Box::new(StackLayout::new()));

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(child),
        Rectangle::new(0.0, 0.0, 80.0, 80.0),
    );
    let mut projected = Rectangle::new(0.0, 0.0, 80.0, 80.0);
    graph.translate_to_absolute_mut(child, &mut projected);
    assert_eq!(projected, Rectangle::new(10.0, 10.0, 80.0, 80.0));
}

#[test]
fn xy_layout_applies_container_insets_once() {
    let mut graph = FigureTree::new();
    let root = graph.builder().set_contents(Box::new(
        RectangleFigure::new(0.0, 0.0, 100.0, 100.0)
            .with_border(LineBorder::new(Color::BLACK, 1.0).with_insets(10.0, 10.0, 10.0, 10.0)),
    ));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    graph.set_constraint(child, XYConstraint::at_size(5.0, 6.0, 20.0, 20.0));
    graph.set_block_layout_manager(root, Box::new(XYLayout::new()));

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(child),
        Rectangle::new(5.0, 6.0, 20.0, 20.0),
    );
    let mut projected = Rectangle::new(0.0, 0.0, 20.0, 20.0);
    graph.translate_to_absolute_mut(child, &mut projected);
    assert_eq!(projected, Rectangle::new(15.0, 16.0, 20.0, 20.0));
}

#[test]
fn border_layout_uses_the_reserved_south_size_for_placement() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let center = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    let south = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    graph.set_constraint(center, BorderConstraint::new(BorderRegion::Center));
    graph.set_constraint(
        south,
        BorderConstraint::with_size(BorderRegion::South, 150.0),
    );
    graph.set_block_layout_manager(root, Box::new(BorderLayout::with_sizes(0.0, 0.0, 0.0, 0.0)));

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(center),
        Rectangle::new(0.0, 0.0, 200.0, 50.0),
    );
    assert_rect(
        graph.figure_bounds(south),
        Rectangle::new(0.0, 50.0, 200.0, 150.0),
    );
}

#[test]
fn border_layout_uses_the_reserved_east_size_for_placement() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
    let center = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    let east = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    graph.set_constraint(center, BorderConstraint::new(BorderRegion::Center));
    graph.set_constraint(east, BorderConstraint::with_size(BorderRegion::East, 150.0));
    graph.set_block_layout_manager(root, Box::new(BorderLayout::with_sizes(0.0, 0.0, 0.0, 0.0)));

    graph.revalidate(root);

    assert_rect(
        graph.figure_bounds(center),
        Rectangle::new(0.0, 0.0, 50.0, 200.0),
    );
    assert_rect(
        graph.figure_bounds(east),
        Rectangle::new(50.0, 0.0, 150.0, 200.0),
    );
}

#[test]
fn toolbar_layout_compresses_main_axis_and_stretches_minor_axis() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 170.0, 60.0)));
    let first = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)));
    let second = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 30.0)));
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
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let first = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)));
    let second = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 30.0)));
    let third = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 60.0, 25.0)));
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
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 210.0, 100.0)));
    let spanning = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)));
    let trailing = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)));
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
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1024.0, 1024.0)));
    let mut children = Vec::new();
    for _ in 0..1024 {
        children.push(
            graph
                .builder()
                .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0))),
        );
    }
    graph.set_block_layout_manager(
        root,
        Box::new(
            GridLayout::new(32)
                .with_margins(0.0, 0.0)
                .with_spacing(1.0, 1.0),
        ),
    );
    let mut update_manager = UpdateManager::new();
    graph.mark_invalid(&mut update_manager, root);

    let canvas = graph.perform_update(&mut update_manager);

    assert!(graph.is_valid(root));
    assert!(children.into_iter().all(|child| graph.is_valid(child)));
    assert!(!update_manager.is_update_queued());
    assert!(!canvas.damage().is_empty());
    assert!(!canvas.commands().is_empty());
}

struct InvalidOutputLayout {
    valid_child: FigureId,
    invalid_child: FigureId,
}

impl LayoutManager for InvalidOutputLayout {
    fn get_preferred_size(
        &self,
        _container: FigureId,
        _w_hint: f64,
        _h_hint: f64,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn get_minimum_size(
        &self,
        container: FigureId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.get_preferred_size(container, w_hint, h_hint, snapshot)
    }

    fn layout(
        &mut self,
        _container: FigureId,
        _snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        out.set_child_bounds(self.valid_child, Rectangle::new(50.0, 60.0, 70.0, 80.0));
        out.set_child_bounds(self.invalid_child, Rectangle::new(1.0, 2.0, 3.0, 4.0));
        Ok(())
    }
}

#[test]
fn layout_output_is_validated_before_any_change_is_committed() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)));
    graph.set_block_layout_manager(
        root,
        Box::new(InvalidOutputLayout {
            valid_child: child,
            invalid_child: root,
        }),
    );

    let error = graph.try_revalidate(root).expect_err("invalid output");

    assert_eq!(
        error,
        LayoutError::InvalidChild {
            container: root,
            child: root,
        }
    );
    assert_rect(
        graph.figure_bounds(child),
        Rectangle::new(10.0, 20.0, 30.0, 40.0),
    );
    assert!(graph.get_block_layout_manager(root).is_some());
}

#[test]
fn wrong_constraint_type_is_reported_and_invalid_work_is_preserved() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)));
    graph.set_block_layout_manager(root, Box::new(XYLayout::new()));
    graph.set_constraint(child, BorderConstraint::new(BorderRegion::Center));
    let mut updates = UpdateManager::new();
    graph.mark_invalid(&mut updates, root);

    graph.perform_update(&mut updates);

    assert!(matches!(
        updates.last_validation_error(),
        Some(ValidationError::Layout(
            LayoutError::ConstraintTypeMismatch {
                container,
                child: error_child,
                ..
            }
        )) if *container == root && *error_child == child
    ));
    assert!(updates.has_pending_layout());
    assert!(!graph.is_valid(root));
}

struct CountingLayout {
    measurements: Arc<AtomicUsize>,
}

impl LayoutManager for CountingLayout {
    fn get_preferred_size(
        &self,
        _container: FigureId,
        _w_hint: f64,
        _h_hint: f64,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.measurements.fetch_add(1, Ordering::SeqCst);
        (25.0, 35.0)
    }

    fn get_minimum_size(
        &self,
        container: FigureId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.get_preferred_size(container, w_hint, h_hint, snapshot)
    }

    fn layout(
        &mut self,
        _container: FigureId,
        _snapshot: &LayoutSnapshot<'_>,
        _out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        Ok(())
    }
}

#[test]
fn layout_measurements_are_cached_until_generation_changes() {
    let measurements = Arc::new(AtomicUsize::new(0));
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    graph.set_block_layout_manager(
        root,
        Box::new(CountingLayout {
            measurements: measurements.clone(),
        }),
    );

    assert_eq!(graph.preferred_size(root, -1.0, -1.0), Some((25.0, 35.0)));
    assert_eq!(graph.preferred_size(root, -1.0, -1.0), Some((25.0, 35.0)));
    assert_eq!(measurements.load(Ordering::SeqCst), 1);

    let old_generation = graph.get_block(root).unwrap().layout_state().generation();
    graph.set_preferred_size(child, Some((20.0, 20.0)));
    let new_generation = graph.get_block(root).unwrap().layout_state().generation();
    assert!(new_generation > old_generation);
    assert_eq!(graph.preferred_size(root, -1.0, -1.0), Some((25.0, 35.0)));
    assert_eq!(measurements.load(Ordering::SeqCst), 2);

    graph.revalidate(root);
    let layout_state = graph.get_block(root).unwrap().layout_state();
    assert_eq!(
        layout_state.validated_generation(),
        Some(layout_state.generation())
    );
}

#[test]
fn explicit_zero_size_is_not_treated_as_a_missing_measurement() {
    let measurements = Arc::new(AtomicUsize::new(0));
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    graph.set_block_layout_manager(
        root,
        Box::new(CountingLayout {
            measurements: measurements.clone(),
        }),
    );
    graph.set_preferred_size(root, Some((0.0, 0.0)));

    assert_eq!(graph.preferred_size(root, -1.0, -1.0), Some((0.0, 0.0)));
    assert_eq!(measurements.load(Ordering::SeqCst), 0);
}

struct ReinvalidatingLayout {
    child: FigureId,
}

impl LayoutManager for ReinvalidatingLayout {
    fn get_preferred_size(
        &self,
        _container: FigureId,
        _w_hint: f64,
        _h_hint: f64,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn get_minimum_size(
        &self,
        container: FigureId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.get_preferred_size(container, w_hint, h_hint, snapshot)
    }

    fn layout(
        &mut self,
        _container: FigureId,
        _snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        out.invalidate(self.child);
        Ok(())
    }
}

#[test]
fn non_converging_validation_returns_diagnostic_and_keeps_work_queued() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = graph
        .builder()
        .add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
    graph.set_block_layout_manager(root, Box::new(ReinvalidatingLayout { child }));
    let mut updates = UpdateManager::new();
    graph.mark_invalid(&mut updates, root);

    let error = graph
        .perform_validation_cycle_with_budget(&mut updates, 3)
        .expect_err("validation must not converge");

    assert!(matches!(
        error,
        ValidationError::NonConvergingValidation {
            budget: 3,
            ref invalidation_chain,
        } if invalidation_chain.len() == 3
    ));
    assert!(updates.has_pending_layout());
}
