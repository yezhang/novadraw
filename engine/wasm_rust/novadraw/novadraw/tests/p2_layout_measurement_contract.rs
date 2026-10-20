use novadraw::{
    figure::{Figure, FigureMeasurement, MeasureConstraints, RectangleFigure},
    geometry::{Dimension, Rectangle},
    layout::{
        FlowDirection, FlowLayout, GridLayout, LayoutManager, ToolbarLayout, ToolbarOrientation,
    },
    tree::FigureTree,
};

struct AxisSensitiveFigure;
impl Figure for AxisSensitiveFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 70.0, 40.0)
    }
    fn name(&self) -> &'static str {
        "AxisSensitiveFigure"
    }
    fn intrinsic_measurement(&self, c: MeasureConstraints) -> FigureMeasurement {
        FigureMeasurement::new(
            c.max_width().unwrap_or(70.0).max(30.0),
            c.max_height().unwrap_or(40.0).max(20.0),
            None,
        )
    }
    fn intrinsic_minimum_measurement(&self, c: MeasureConstraints) -> FigureMeasurement {
        FigureMeasurement::new(
            c.max_width().unwrap_or(30.0).max(30.0),
            c.max_height().unwrap_or(20.0).max(20.0),
            None,
        )
    }
}

#[test]
fn layout_strategies_select_their_measurement_axis() {
    let cases: Vec<(Box<dyn LayoutManager>, Dimension, Dimension)> = vec![
        (
            Box::new(FlowLayout::new()),
            Dimension::new(51.0, 40.0),
            Dimension::new(51.0, 40.0),
        ),
        (
            Box::new(FlowLayout::with_direction(FlowDirection::Vertical)),
            Dimension::new(70.0, 61.0),
            Dimension::new(70.0, 61.0),
        ),
        (
            Box::new(ToolbarLayout::new()),
            Dimension::new(51.0, 40.0),
            Dimension::new(51.0, 20.0),
        ),
        (
            Box::new(ToolbarLayout::new().with_orientation(ToolbarOrientation::Horizontal)),
            Dimension::new(70.0, 61.0),
            Dimension::new(30.0, 61.0),
        ),
        (
            Box::new(GridLayout::new(1)),
            Dimension::new(51.0, 61.0),
            Dimension::new(51.0, 61.0),
        ),
    ];
    for (layout, preferred, minimum) in cases {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 301.0, 100.0)))
            .unwrap();
        tree.builder().set_layout_manager(root, layout).unwrap();
        tree.builder()
            .add_child(root, Box::new(AxisSensitiveFigure))
            .unwrap();
        let hints = MeasureConstraints::bounded(51.0, 61.0).unwrap();
        assert_eq!(
            tree.preferred_measurement(root, hints).unwrap().size(),
            preferred
        );
        assert_eq!(tree.minimum_size(root, hints).unwrap(), minimum);
    }
}

#[test]
fn toolbar_distributes_integer_shrink_without_losing_extent() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 61.0)))
        .unwrap();
    tree.builder()
        .set_layout_manager(root, Box::new(ToolbarLayout::new().with_spacing(5.0)))
        .unwrap();
    let mut children = Vec::new();
    for height in [20.0, 30.0, 40.0] {
        let child = tree
            .builder()
            .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 40.0, height)))
            .unwrap();
        tree.builder()
            .set_minimum_size(child, Some(Dimension::new(20.0, 10.0)))
            .unwrap();
        children.push(child);
    }
    tree.builder().validate_subtree(root).unwrap();
    let bounds: Vec<_> = children
        .iter()
        .map(|id| tree.figure_bounds(*id).unwrap())
        .collect();
    assert_eq!(
        bounds.iter().map(|r| r.height).collect::<Vec<_>>(),
        [14.0, 17.0, 20.0]
    );
    assert_eq!(bounds[2].y + bounds[2].height, 61.0);
}
