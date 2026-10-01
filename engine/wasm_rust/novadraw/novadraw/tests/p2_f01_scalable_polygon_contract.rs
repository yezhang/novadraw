use novadraw::geometry::{Point, PointList, Rectangle};
use novadraw::render::{LineJoin, NdCanvas, command::RenderCommandKind};
use novadraw::{
    Alignment, Figure, PolygonScaleMode, RectangleFigure, Runtime, ScalablePolygonError,
    ScalablePolygonFigure, ShapeMutationError,
};

fn wide_template() -> PointList {
    PointList::from_points(vec![
        Point::new(0.0, 0.0),
        Point::new(20.0, 0.0),
        Point::new(20.0, 10.0),
        Point::new(0.0, 10.0),
    ])
}

#[test]
fn scalable_polygon_maps_one_template_with_stretch_and_preserved_aspect() {
    let bounds = Rectangle::new(10.0, 20.0, 100.0, 60.0);
    let stretch = ScalablePolygonFigure::new(bounds, wide_template()).unwrap();
    assert_eq!(
        stretch
            .scaled_points(Rectangle::new(0.0, 0.0, 100.0, 60.0))
            .as_slice(),
        &[
            Point::new(4.0, 4.0),
            Point::new(96.0, 4.0),
            Point::new(96.0, 56.0),
            Point::new(4.0, 56.0),
        ]
    );

    let preserved = ScalablePolygonFigure::new(bounds, wide_template())
        .unwrap()
        .with_scale_mode(PolygonScaleMode::PreserveAspect)
        .with_alignment(Alignment::End, Alignment::Start);
    assert_eq!(
        preserved
            .scaled_points(Rectangle::new(0.0, 0.0, 100.0, 60.0))
            .as_slice(),
        &[
            Point::new(4.0, 4.0),
            Point::new(96.0, 4.0),
            Point::new(96.0, 50.0),
            Point::new(4.0, 50.0),
        ]
    );
}

#[test]
fn scalable_polygon_handles_empty_and_single_axis_templates_without_nan() {
    let empty =
        ScalablePolygonFigure::new(Rectangle::new(0.0, 0.0, 80.0, 40.0), PointList::new()).unwrap();
    assert!(
        empty
            .scaled_points(Rectangle::new(0.0, 0.0, 80.0, 40.0))
            .is_empty()
    );

    let vertical = ScalablePolygonFigure::new(
        Rectangle::new(0.0, 0.0, 80.0, 100.0),
        PointList::from_points(vec![
            Point::new(5.0, 0.0),
            Point::new(5.0, 10.0),
            Point::new(5.0, 20.0),
        ]),
    )
    .unwrap()
    .with_scale_mode(PolygonScaleMode::PreserveAspect)
    .with_alignment(Alignment::End, Alignment::Center);
    let points = vertical.scaled_points(Rectangle::new(0.0, 0.0, 80.0, 100.0));
    assert!(points.iter().all(|point| point.x() == 76.0));
    assert!(
        points
            .iter()
            .all(|point| point.x().is_finite() && point.y().is_finite())
    );

    assert_eq!(
        ScalablePolygonFigure::new(
            Rectangle::new(0.0, 0.0, 80.0, 40.0),
            PointList::from_points(vec![Point::new(f64::NAN, 0.0)]),
        )
        .err(),
        Some(ScalablePolygonError::NonFiniteTemplate)
    );
}

#[test]
fn scalable_polygon_reserves_miter_outset_and_uses_precise_hit() {
    let figure = ScalablePolygonFigure::new(
        Rectangle::new(0.0, 0.0, 100.0, 80.0),
        PointList::from_points(vec![
            Point::new(0.0, 10.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 10.0),
        ]),
    )
    .unwrap()
    .with_stroke(novadraw::Color::BLACK, 4.0, LineJoin::Miter);
    let points = figure.scaled_points(Rectangle::new(0.0, 0.0, 100.0, 80.0));
    assert!(
        points.iter().all(|point| {
            (8.0..=92.0).contains(&point.x()) && (8.0..=72.0).contains(&point.y())
        })
    );
    assert!(figure.precise_hit(50.0, 30.0, figure.initial_bounds()));
    assert!(!figure.precise_hit(5.0, 5.0, figure.initial_bounds()));

    let mut canvas = NdCanvas::new();
    canvas.fill_style(novadraw::Color::WHITE);
    canvas.stroke_style(novadraw::Color::BLACK);
    figure.paint_figure_in_bounds(&mut canvas, figure.initial_bounds());
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillPath { .. }))
    );
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::StrokePath { .. }))
    );
}

#[test]
fn runtime_mutations_and_resize_use_current_template_without_stale_geometry() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .unwrap();
    let polygon = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ScalablePolygonFigure::new(Rectangle::new(20.0, 20.0, 100.0, 60.0), wide_template())
                .unwrap(),
        ))
        .unwrap();

    assert_eq!(runtime.tree().hit_test_simple((30.0, 25.0)), Some(polygon));
    assert!(
        runtime
            .scalable_polygon(polygon)
            .unwrap()
            .set_scale_mode(PolygonScaleMode::PreserveAspect)
            .unwrap()
    );
    assert_ne!(runtime.tree().hit_test_simple((30.0, 25.0)), Some(polygon));
    assert!(
        runtime
            .scalable_polygon(polygon)
            .unwrap()
            .set_alignment(Alignment::Center, Alignment::Start)
            .unwrap()
    );
    assert_eq!(runtime.tree().hit_test_simple((30.0, 25.0)), Some(polygon));
    assert!(
        runtime
            .figure(polygon)
            .unwrap()
            .set_bounds(Rectangle::new(20.0, 20.0, 200.0, 100.0))
            .unwrap()
    );
    assert_eq!(
        runtime.tree().figure_bounds(polygon),
        Some(Rectangle::new(20.0, 20.0, 200.0, 100.0))
    );

    let stable_frame = runtime.record_full_frame();
    assert!(
        stable_frame
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillPath { .. }))
    );
    assert_eq!(
        runtime
            .scalable_polygon(polygon)
            .unwrap()
            .replace_template(PointList::from_points(vec![
                Point::new(f64::INFINITY, 0.0,)
            ])),
        Err(ShapeMutationError::NonFiniteGeometry)
    );
    assert!(
        runtime
            .scalable_polygon(polygon)
            .unwrap()
            .replace_template(PointList::new())
            .unwrap()
    );
    let frame = runtime.record_full_frame();
    assert!(
        !frame
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillPath { .. }))
    );
}
