use novadraw::Color;
use novadraw::geometry::{Dimension, Rectangle, Translatable};
use novadraw::{
    BorderConstraint, BorderLayout, BorderRegion, DEFAULT_VALIDATION_BUDGET, FigureId,
    FigureMeasurement, FigureTree, FramePreparationError, GridAlignment, GridConstraint,
    GridLayout, LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot, LineBorder,
    MeasureConstraints, RectangleFigure, Runtime, StackLayout, ToolbarLayout, ValidationError,
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
        .set_contents(Box::new(RectangleFigure::new(10.0, 20.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let first = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 30.0)))
        .expect("valid FigureTree construction");
    let second = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 50.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

    let expected = Rectangle::new(0.0, 0.0, 200.0, 100.0);
    assert_rect(graph.figure_bounds(first), expected);
    assert_rect(graph.figure_bounds(second), expected);
}

#[test]
fn stack_layout_applies_container_insets_once() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).with_border(
                LineBorder::new(Color::BLACK, 1.0).with_insets(10.0, 10.0, 10.0, 10.0),
            ),
        ))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

    assert_rect(
        graph.figure_bounds(child),
        Rectangle::new(0.0, 0.0, 80.0, 80.0),
    );
    let mut projected = Rectangle::new(0.0, 0.0, 80.0, 80.0);
    projected.transform(graph.local_to_surface_transform(child).unwrap());
    assert_eq!(projected, Rectangle::new(10.0, 10.0, 80.0, 80.0));
}

#[test]
fn xy_layout_applies_container_insets_once() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).with_border(
                LineBorder::new(Color::BLACK, 1.0).with_insets(10.0, 10.0, 10.0, 10.0),
            ),
        ))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(child, XYConstraint::at_size(5.0, 6.0, 20.0, 20.0))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(XYLayout::new()))
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

    assert_rect(
        graph.figure_bounds(child),
        Rectangle::new(5.0, 6.0, 20.0, 20.0),
    );
    let mut projected = Rectangle::new(0.0, 0.0, 20.0, 20.0);
    projected.transform(graph.local_to_surface_transform(child).unwrap());
    assert_eq!(projected, Rectangle::new(15.0, 16.0, 20.0, 20.0));
}

#[test]
fn border_layout_uses_the_reserved_south_size_for_placement() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .expect("valid FigureTree construction");
    let center = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    let south = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(center, BorderConstraint::new(BorderRegion::Center))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(
            south,
            BorderConstraint::with_size(BorderRegion::South, 150.0),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(BorderLayout::with_sizes(0.0, 0.0, 0.0, 0.0)))
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .expect("valid FigureTree construction");
    let center = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    let east = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(center, BorderConstraint::new(BorderRegion::Center))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(east, BorderConstraint::with_size(BorderRegion::East, 150.0))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(BorderLayout::with_sizes(0.0, 0.0, 0.0, 0.0)))
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 170.0, 60.0)))
        .expect("valid FigureTree construction");
    let first = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)))
        .expect("valid FigureTree construction");
    let second = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 30.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_minimum_size(first, Some(Dimension::new(60.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_minimum_size(second, Some(Dimension::new(100.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(
                ToolbarLayout::horizontal()
                    .with_spacing(10.0)
                    .with_stretch_minor_axis(true),
            ),
        )
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let first = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)))
        .expect("valid FigureTree construction");
    let second = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 50.0, 30.0)))
        .expect("valid FigureTree construction");
    let third = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 60.0, 25.0)))
        .expect("valid FigureTree construction");
    let fill_cell = GridConstraint {
        horizontal_alignment: GridAlignment::Fill,
        vertical_alignment: GridAlignment::Fill,
        ..GridConstraint::default()
    };
    graph
        .builder()
        .set_layout_constraint(first, fill_cell)
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(second, fill_cell)
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(third, fill_cell)
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(
                GridLayout::new(2)
                    .with_margins(0.0, 0.0)
                    .with_spacing(10.0, 10.0),
            ),
        )
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 210.0, 100.0)))
        .expect("valid FigureTree construction");
    let spanning = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 20.0)))
        .expect("valid FigureTree construction");
    let trailing = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 20.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(spanning, GridConstraint::fill().with_span(2, 1))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_constraint(trailing, GridConstraint::fill())
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(
                GridLayout::new(2)
                    .with_equal_column_widths(true)
                    .with_margins(0.0, 0.0)
                    .with_spacing(10.0, 10.0),
            ),
        )
        .expect("valid FigureTree construction");

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");

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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1024.0, 1024.0)))
        .expect("valid FigureTree construction");
    let mut children = Vec::new();
    for _ in 0..1024 {
        children.push(
            graph
                .builder()
                .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
                .expect("valid FigureTree construction"),
        );
    }
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(
                GridLayout::new(32)
                    .with_margins(0.0, 0.0)
                    .with_spacing(1.0, 1.0),
            ),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(graph);
    runtime.figure(root).unwrap().revalidate().unwrap();
    let canvas = runtime.prepare_frame().expect("queued update");

    assert!(runtime.tree().is_valid(root));
    assert!(
        children
            .into_iter()
            .all(|child| runtime.tree().is_valid(child))
    );
    assert!(runtime.prepare_frame().is_none());
    assert!(!canvas.damage().is_empty());
    assert!(!canvas.commands().is_empty());
}

struct InvalidOutputLayout {
    valid_child: FigureId,
    invalid_child: FigureId,
}

struct InvalidGeometryLayout {
    first_child: FigureId,
    invalid_child: FigureId,
    invalid_bounds: Rectangle,
}

impl LayoutManager for InvalidGeometryLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        FigureMeasurement::default()
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.preferred_measurement(container, constraints, snapshot)
            .size()
    }

    fn layout(
        &mut self,
        _container: FigureId,
        _snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        out.set_child_bounds(self.first_child, Rectangle::new(50.0, 60.0, 70.0, 80.0));
        out.set_child_bounds(self.invalid_child, self.invalid_bounds);
        Ok(())
    }
}

impl LayoutManager for InvalidOutputLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        FigureMeasurement::default()
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.preferred_measurement(container, constraints, snapshot)
            .size()
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(InvalidOutputLayout {
                valid_child: child,
                invalid_child: root,
            }),
        )
        .expect("valid FigureTree construction");

    let error = graph
        .builder()
        .validate_subtree(root)
        .expect_err("invalid output");

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
    assert!(graph.layout_manager(root).is_some());
}

#[test]
fn builder_rejects_negative_layout_geometry_before_any_change_is_committed() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let first = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .unwrap();
    let invalid = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 50.0)))
        .unwrap();
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(InvalidGeometryLayout {
                first_child: first,
                invalid_child: invalid,
                invalid_bounds: Rectangle::new(1.0, 2.0, -1.0, 4.0),
            }),
        )
        .unwrap();

    assert_eq!(
        graph.builder().validate_subtree(root),
        Err(LayoutError::NonFiniteGeometry { figure: invalid })
    );
    assert_rect(
        graph.figure_bounds(first),
        Rectangle::new(10.0, 20.0, 30.0, 40.0),
    );
    assert_rect(
        graph.figure_bounds(invalid),
        Rectangle::new(20.0, 30.0, 40.0, 50.0),
    );
}

#[test]
fn runtime_rejects_non_finite_layout_geometry_before_any_change_is_committed() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let first = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .unwrap();
    let invalid = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 50.0)))
        .unwrap();
    graph.builder().validate_subtree(root).unwrap();
    let mut runtime = Runtime::new(graph);
    runtime
        .container(root)
        .unwrap()
        .set_layout_manager(Box::new(InvalidGeometryLayout {
            first_child: first,
            invalid_child: invalid,
            invalid_bounds: Rectangle::new(f64::NAN, 2.0, 3.0, 4.0),
        }))
        .unwrap();

    assert!(matches!(
        runtime.stabilize_for_query(),
        Err(FramePreparationError::Validation(ValidationError::Layout(
            LayoutError::NonFiniteGeometry { figure }
        ))) if figure == invalid
    ));
    assert_rect(
        runtime.tree().figure_bounds(first),
        Rectangle::new(10.0, 20.0, 30.0, 40.0),
    );
    assert_rect(
        runtime.tree().figure_bounds(invalid),
        Rectangle::new(20.0, 30.0, 40.0, 50.0),
    );
}

#[test]
fn builder_rejects_wrong_constraint_type_without_mutating_layout_state() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(XYLayout::new()))
        .expect("valid FigureTree construction");
    let error = graph
        .builder()
        .set_layout_constraint(child, BorderConstraint::new(BorderRegion::Center))
        .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::ConstraintTypeMismatch {
            container,
            child: error_child,
            ..
        } if container == root && error_child == child
    ));
    assert!(graph.layout_constraint::<BorderConstraint>(child).is_none());
}

#[test]
fn builder_rejects_incompatible_manager_before_replacing_layout_state() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)))
        .unwrap();
    let constraint = XYConstraint::at_size(10.0, 20.0, 30.0, 40.0);
    graph
        .builder()
        .set_layout_constraint(child, constraint)
        .unwrap();

    let error = graph
        .builder()
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .unwrap_err();

    assert!(matches!(
        error,
        LayoutError::UnsupportedConstraint {
            container,
            child: error_child,
            ..
        } if container == root && error_child == child
    ));
    assert!(graph.layout_manager(root).is_none());
    assert_eq!(
        graph.layout_constraint::<XYConstraint>(child),
        Some(&constraint)
    );
}

struct CountingLayout {
    measurements: Arc<AtomicUsize>,
}

impl LayoutManager for CountingLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        self.measurements.fetch_add(1, Ordering::SeqCst);
        FigureMeasurement::new(25.0, 35.0, None)
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.preferred_measurement(container, constraints, snapshot)
            .size()
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(CountingLayout {
                measurements: measurements.clone(),
            }),
        )
        .expect("valid FigureTree construction");

    assert_eq!(
        graph
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(
        graph
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(measurements.load(Ordering::SeqCst), 1);

    let old_generation = graph.node(root).unwrap().layout_state().generation();
    graph
        .builder()
        .set_preferred_size(child, Some(Dimension::new(20.0, 20.0)))
        .expect("valid FigureTree construction");
    let new_generation = graph.node(root).unwrap().layout_state().generation();
    assert!(new_generation > old_generation);
    assert_eq!(
        graph
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(measurements.load(Ordering::SeqCst), 2);

    let width_bounded = MeasureConstraints::width(80.0).unwrap();
    assert_eq!(
        graph
            .preferred_measurement(root, width_bounded)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(
        graph
            .preferred_measurement(root, width_bounded)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(measurements.load(Ordering::SeqCst), 3);
    assert_eq!(
        graph
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(25.0, 35.0))
    );
    assert_eq!(measurements.load(Ordering::SeqCst), 4);

    graph
        .builder()
        .validate_subtree(root)
        .expect("valid FigureTree construction");
    let layout_state = graph.node(root).unwrap().layout_state();
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(
            root,
            Box::new(CountingLayout {
                measurements: measurements.clone(),
            }),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_preferred_size(root, Some(Dimension::ZERO))
        .expect("valid FigureTree construction");

    assert_eq!(
        graph
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::ZERO)
    );
    assert_eq!(measurements.load(Ordering::SeqCst), 0);
}

struct ReinvalidatingLayout {
    child: FigureId,
}

impl LayoutManager for ReinvalidatingLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        FigureMeasurement::default()
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.preferred_measurement(container, constraints, snapshot)
            .size()
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
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");
    let child = graph
        .builder()
        .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid FigureTree construction");
    graph
        .builder()
        .set_layout_manager(root, Box::new(ReinvalidatingLayout { child }))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(graph);
    runtime.figure(root).unwrap().revalidate().unwrap();
    let error = runtime
        .stabilize_for_query()
        .expect_err("validation must not converge");

    assert!(matches!(
        error,
        FramePreparationError::Validation(ValidationError::NonConvergingValidation {
            budget: DEFAULT_VALIDATION_BUDGET,
            ref invalidation_chain,
        }) if invalidation_chain.len() == DEFAULT_VALIDATION_BUDGET
    ));
    assert!(runtime.has_pending_update());
}
