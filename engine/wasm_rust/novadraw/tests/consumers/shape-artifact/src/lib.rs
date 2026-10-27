use novadraw::{
    figure::{Figure, PolygonFigure, RoundedRectangleFigure, TriangleFigure},
    geometry::{Point, Rectangle},
    render::CommandRecorder,
};

/// Shared source/configuration; the feature changes only use of independent corner/edge APIs.
#[unsafe(no_mangle)]
pub extern "C" fn record_shapes(radius: f64) -> usize {
    let bounds = Rectangle::new(0.0, 0.0, 100.0, 80.0);
    let rectangle = RoundedRectangleFigure::from_bounds(bounds, radius);
    let triangle = TriangleFigure::new(bounds);
    let polygon = PolygonFigure::from_points(vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(50.0, 80.0),
    ]);
    #[cfg(feature = "independent")]
    let (rectangle, triangle, polygon) = {
        use novadraw::figure::{EdgeStroke, RectangleCorners, RectangleEdges};
        (
            rectangle
                .with_corners(RectangleCorners::circular([radius, radius, 0.0, 0.0]).unwrap())
                .unwrap()
                .with_edges(RectangleEdges([
                    EdgeStroke::Inherit,
                    EdgeStroke::Inherit,
                    EdgeStroke::None,
                    EdgeStroke::Inherit,
                ]))
                .unwrap(),
            triangle.with_corner_radii([radius; 3]).unwrap(),
            polygon.with_corner_radii(vec![radius; 3]).unwrap(),
        )
    };
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(novadraw::graphics::Color::WHITE);
        for figure in [&rectangle as &dyn Figure, &triangle, &polygon] {
            figure.paint(&mut context, bounds).unwrap();
        }
    }
    recorder.commands().len()
}
