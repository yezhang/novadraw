use novadraw::{
    figure::{MeasureConstraints, RectangleFigure},
    geometry::Rectangle,
    layout::{GridConstraint, GridLayout},
    tree::FigureTree,
};

#[test]
fn fractional_grid_space_preserves_extent_and_nonnegative_grab_tracks() {
    for (equal, width) in [(false, 15.25), (false, 300.25), (true, 300.25)] {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, width, 80.0).expect("valid figure bounds"),
            ))
            .unwrap();
        tree.builder()
            .set_layout_manager(
                root,
                Box::new(GridLayout::new(2).with_equal_column_widths(equal)),
            )
            .unwrap();
        let mut children = Vec::new();
        for _ in 0..2 {
            let child = tree
                .builder()
                .add_child(
                    root,
                    Box::new(
                        RectangleFigure::new(0.0, 0.0, 70.0, 70.0).expect("valid figure bounds"),
                    ),
                )
                .unwrap();
            tree.builder()
                .set_layout_constraint(child, GridConstraint::fill())
                .unwrap();
            children.push(child);
        }
        tree.builder().validate_subtree(root).unwrap();
        let first = tree.figure_bounds(children[0]).unwrap();
        let last = tree.figure_bounds(children[1]).unwrap();
        assert!(first.width >= 0.0 && last.width >= 0.0);
        assert_eq!(last.x + last.width + 5.0, width);
        if equal {
            assert_eq!(first.width, last.width);
        }
    }
}

#[test]
fn replacing_a_spanning_hint_invalidates_measurement_and_preserves_fixed_columns() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 155.0, 160.0).expect("valid figure bounds"),
        ))
        .unwrap();
    tree.builder()
        .set_layout_manager(root, Box::new(GridLayout::new(2)))
        .unwrap();
    let mut children = Vec::new();
    for _ in 0..3 {
        children.push(
            tree.builder()
                .add_child(
                    root,
                    Box::new(
                        RectangleFigure::new(0.0, 0.0, 70.0, 70.0).expect("valid figure bounds"),
                    ),
                )
                .unwrap(),
        );
    }
    tree.builder()
        .set_layout_constraint(
            children[2],
            GridConstraint {
                grab_horizontal: true,
                horizontal_alignment: novadraw::layout::GridAlignment::Fill,
                ..GridConstraint::default()
            },
        )
        .unwrap();
    for (hint, expected_width, trailing_width) in [(200.0, 210.0, 125.0), (90.0, 155.0, 70.0)] {
        tree.builder()
            .set_layout_constraint(
                children[0],
                GridConstraint {
                    horizontal_span: 2,
                    width_hint: Some(hint),
                    ..GridConstraint::default()
                },
            )
            .unwrap();
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .width,
            expected_width
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .width,
            expected_width
        );
        tree.builder().validate_subtree(root).unwrap();
        assert_eq!(
            tree.figure_bounds(children[1]).unwrap(),
            Rectangle::new(5.0, 80.0, 70.0, 70.0)
        );
        assert_eq!(
            tree.figure_bounds(children[2]).unwrap(),
            Rectangle::new(80.0, 80.0, trailing_width, 70.0)
        );
    }
}
