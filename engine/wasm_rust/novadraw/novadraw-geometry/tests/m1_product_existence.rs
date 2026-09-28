use novadraw_geometry::{Affine2D, ApproxEq, Dimension, Insets, Point, PointList, Rectangle, Vec2};
use std::any::TypeId;

fn accepts_point(_: Point) {}
fn accepts_vector(_: Vec2) {}

#[test]
fn m1_geometry_canonical_type_names_are_importable() {
    let point = Point::new(1.0, 2.0);
    let rect = Rectangle::new(0.0, 0.0, 10.0, 20.0);
    let dimension = Dimension::new(10.0, 20.0);
    let vector = Vec2::new(3.0, 4.0);
    let transform = Affine2D::from_translation(5.0, 6.0);

    accepts_point(point);
    accepts_vector(vector);
    assert_eq!(rect.width, dimension.width);
    assert_eq!(transform.translation(), Vec2::new(5.0, 6.0));

    let insets = Insets::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(insets.left, 2.0);

    let points = PointList::from_points(vec![point, Point::new(4.0, 6.0)]);
    assert_eq!(points.bounds(), Some(Rectangle::new(1.0, 2.0, 3.0, 4.0)));
}

#[test]
fn m1_canonical_values_keep_approx_eq_contract() {
    let precise_point = Point::new(1.0, 1.0 + 1e-11);
    let expected_point = Point::new(1.0, 1.0);
    assert!(precise_point.approx_eq_default(expected_point));

    let precise_rect = Rectangle::new(0.0, 0.0, 10.0, 10.0 + 1e-11);
    let expected_rect = Rectangle::new(0.0, 0.0, 10.0, 10.0);
    assert!(precise_rect.approx_eq_default(expected_rect));

    let affine = Affine2D::from_translation(3.0, 4.0);
    let expected = Affine2D::new(1.0, 0.0, 0.0, 1.0, 3.0, 4.0);
    assert!(affine.approx_eq_default(expected));
}

#[test]
fn point_and_vector_have_distinct_arithmetic_semantics() {
    assert_ne!(TypeId::of::<Point>(), TypeId::of::<Vec2>());

    let start = Point::new(10.0, 20.0);
    let end = Point::new(16.0, 28.0);
    let delta: Vec2 = end - start;
    let moved: Point = start + delta;
    let restored: Point = moved - delta;

    assert_eq!(delta, Vec2::new(6.0, 8.0));
    assert_eq!(moved, end);
    assert_eq!(restored, start);
}
