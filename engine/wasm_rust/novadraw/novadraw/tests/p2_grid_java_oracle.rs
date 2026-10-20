use novadraw::{
    figure::{MeasureConstraints, RectangleFigure},
    geometry::{Dimension, Rectangle},
    layout::{GridConstraint, GridLayout},
    tree::FigureTree,
};

const ORACLE: &str =
    include_str!("../../verification/evidence/draw2d-official-examples/grid-measurements.tsv");

fn numbers(text: &str) -> Vec<f64> {
    text.split(',')
        .map(|value| value.parse().unwrap())
        .collect()
}

#[test]
fn grid_fixed_measurement_and_bounds_match_java() {
    let mut differences = Vec::new();
    for row in ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = row.split('\t').collect();
        let mode: usize = fields[0].parse().unwrap();
        let width: f64 = fields[1].parse().unwrap();
        let height: f64 = fields[2].parse().unwrap();
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, width, height)))
            .unwrap();
        tree.builder()
            .set_layout_manager(
                root,
                Box::new(GridLayout::new(2).with_equal_column_widths(mode == 1 || mode == 4)),
            )
            .unwrap();
        let mut children = Vec::new();
        for i in 0..3 {
            let child = tree
                .builder()
                .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 70.0, 70.0)))
                .unwrap();
            tree.builder()
                .set_preferred_size(child, Some(Dimension::new(70.0, 70.0)))
                .unwrap();
            let mut constraint = GridConstraint::default();
            if mode == 2 && i == 0 {
                constraint.horizontal_span = 2;
                constraint.horizontal_indent = 9.0;
                constraint.width_hint = Some(90.0);
                constraint.height_hint = Some(30.0);
            }
            if mode == 3 || mode == 4 || (mode == 5 && i != 1) {
                constraint = GridConstraint::fill();
            }
            tree.builder()
                .set_layout_constraint(child, constraint)
                .unwrap();
            children.push(child);
        }
        let preferred = tree
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .unwrap()
            .size();
        let minimum = tree
            .minimum_size(root, MeasureConstraints::UNBOUNDED)
            .unwrap();
        for (kind, actual, expected) in [
            ("preferred", preferred, numbers(fields[3])),
            ("minimum", minimum, numbers(fields[4])),
        ] {
            if actual != Dimension::new(expected[0], expected[1]) {
                differences.push(format!(
                    "mode={mode} width={width} {kind}: {actual:?} != {expected:?}"
                ));
            }
        }
        tree.builder().validate_subtree(root).unwrap();
        for (child, expected) in children.into_iter().zip(fields[5].split(';')) {
            let values = numbers(expected);
            let expected = Rectangle::new(values[0], values[1], values[2], values[3]);
            let actual = tree.figure_bounds(child).unwrap();
            if actual != expected {
                differences.push(format!(
                    "mode={mode} width={width} child: {actual:?} != {expected:?}"
                ));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn fractional_grid_space_preserves_extent_and_nonnegative_grab_tracks() {
    for (equal, width) in [(false, 15.25), (false, 300.25), (true, 300.25)] {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, width, 80.0)))
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
                .add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 70.0, 70.0)))
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
