use novadraw::{
    container::{
        FreeformLayerFigure, FreeformLayeredPane, LayerFigure, LayeredPane,
        ScalableFreeformLayeredPane, ScalableLayeredPaneFigure, ViewportFigure,
    },
    figure::{
        Bounded, ButtonFigure, ClickableFigure, Direction, EllipseFigure, Figure, FlowPage,
        GraphProjectionFigure, LabelFigure, PolygonFigure, PolylineFigure, RectangleFigure,
        RootFigure, RoundedRectangleFigure, ScalablePolygonFigure, Shape, ShapeGeometryError,
        TextFlowFigure, ToggleFigure, TriangleFigure, flow::FlowPageFigure,
    },
    geometry::{Dimension, Point, PointList, Rectangle},
    graphics::{Color, CustomDash, DashPattern, GraphicsInputError, LineCap, StrokeStyle},
    render::CommandRecorder,
    runtime::{ComponentUpdateError, ReplaceScalablePolygonTemplate, Runtime},
};

fn erased<T: Figure + 'static>(
    figure: Result<T, ShapeGeometryError>,
) -> Result<Box<dyn Figure>, ShapeGeometryError> {
    figure.map(|figure| Box::new(figure) as Box<dyn Figure>)
}

fn constructors(bounds: Rectangle) -> Vec<Result<Box<dyn Figure>, ShapeGeometryError>> {
    let Rectangle {
        x,
        y,
        width,
        height,
    } = bounds;
    vec![
        erased(RectangleFigure::new(x, y, width, height)),
        erased(RectangleFigure::from_bounds(bounds)),
        erased(RectangleFigure::new_with_color(
            x,
            y,
            width,
            height,
            Color::RED,
        )),
        erased(EllipseFigure::new(x, y, width, height)),
        erased(EllipseFigure::from_bounds(bounds)),
        erased(EllipseFigure::new_with_color(
            x,
            y,
            width,
            height,
            Color::RED,
        )),
        erased(RoundedRectangleFigure::new(x, y, width, height, 5.0)),
        erased(RoundedRectangleFigure::from_bounds(bounds, 5.0)),
        erased(RoundedRectangleFigure::new_with_color(
            x,
            y,
            width,
            height,
            5.0,
            Color::RED,
        )),
        erased(RootFigure::new(x, y, width, height)),
        erased(TriangleFigure::new(bounds)),
        erased(TriangleFigure::new_with_direction(bounds, Direction::East)),
        erased(LayerFigure::new(x, y, width, height)),
        erased(FreeformLayerFigure::new(x, y, width, height)),
        erased(LayeredPane::new(x, y, width, height)),
        erased(FreeformLayeredPane::new(x, y, width, height)),
        erased(ScalableLayeredPaneFigure::new(x, y, width, height)),
        erased(ScalableFreeformLayeredPane::new(x, y, width, height)),
        erased(ViewportFigure::new(x, y, width, height)),
        erased(ClickableFigure::new(bounds)),
        erased(ButtonFigure::new("button").with_bounds(bounds)),
        erased(ToggleFigure::new("toggle").with_bounds(bounds)),
        erased(LabelFigure::new("label").with_bounds(bounds)),
        erased(TextFlowFigure::new(bounds, FlowPage::from_text("flow"))),
        erased(FlowPageFigure::new(bounds)),
        erased(GraphProjectionFigure::new(bounds)),
    ]
}

#[test]
fn bounds_constructors_reject_raw_and_endpoint_overflow_and_preserve_legal_geometry() {
    for bounds in [
        Rectangle::new(f64::NAN, 0.0, 1.0, 1.0),
        Rectangle::new(0.0, f64::INFINITY, 1.0, 1.0),
        Rectangle::new(0.0, 0.0, -1.0, 1.0),
        Rectangle::new(0.0, 0.0, 1.0, f64::NEG_INFINITY),
        Rectangle::new(f64::MAX, 0.0, f64::MAX, 1.0),
        Rectangle::new(0.0, f64::MAX, 1.0, f64::MAX),
    ] {
        for (index, result) in constructors(bounds).into_iter().enumerate() {
            assert!(result.is_err(), "constructor {index} admitted {bounds:?}");
        }
    }
    for bounds in [
        Rectangle::ZERO,
        Rectangle::new(-30.0, -40.0, 0.0, 10.0),
        Rectangle::new(-10.0, -20.0, 5.0, 0.0),
        Rectangle::new(-100.0, -80.0, 30.0, 20.0),
    ] {
        for result in constructors(bounds) {
            assert_eq!(result.unwrap().initial_bounds(), bounds);
        }
    }
}

#[test]
fn bounded_trait_cannot_bypass_checked_update_or_destroy_previous_configuration() {
    let original = Rectangle::new(1.0, 2.0, 30.0, 20.0);
    let mut figures: Vec<Box<dyn Bounded>> = vec![
        Box::new(RectangleFigure::from_bounds(original).unwrap()),
        Box::new(EllipseFigure::from_bounds(original).unwrap()),
        Box::new(RoundedRectangleFigure::from_bounds(original, 8.0).unwrap()),
        Box::new(TriangleFigure::new(original).unwrap()),
        Box::new(RootFigure::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(LabelFigure::new("label").with_bounds(original).unwrap()),
        Box::new(ButtonFigure::new("button").with_bounds(original).unwrap()),
        Box::new(ToggleFigure::new("toggle").with_bounds(original).unwrap()),
        Box::new(ClickableFigure::new(original).unwrap()),
        Box::new(TextFlowFigure::new(original, FlowPage::from_text("flow")).unwrap()),
        Box::new(LayerFigure::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(FreeformLayerFigure::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(LayeredPane::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(FreeformLayeredPane::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(ScalableLayeredPaneFigure::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(ScalableFreeformLayeredPane::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(ViewportFigure::new(1.0, 2.0, 30.0, 20.0).unwrap()),
        Box::new(ScalablePolygonFigure::new(original, PointList::new()).unwrap()),
    ];
    for figure in &mut figures {
        assert!(figure.set_bounds(f64::MAX, 0.0, f64::MAX, 1.0).is_err());
        assert_eq!(figure.bounds(), original);
        assert!(figure.set_bounds(0.0, 0.0, -1.0, 1.0).is_err());
        assert_eq!(figure.bounds(), original);
        figure.set_bounds(-10.0, -20.0, 0.0, 5.0).unwrap();
        assert_eq!(figure.bounds(), Rectangle::new(-10.0, -20.0, 0.0, 5.0));
    }
}

#[test]
fn raw_stroke_rejection_preserves_shape_zero_width_and_original_error_source() {
    for width in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
        assert!(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0)
                .unwrap()
                .with_stroke(Color::BLACK, width)
                .is_err()
        );
        assert!(
            EllipseFigure::new(0.0, 0.0, 10.0, 10.0)
                .unwrap()
                .with_stroke(Color::BLACK, width)
                .is_err()
        );
        assert!(
            RoundedRectangleFigure::new(0.0, 0.0, 10.0, 10.0, 4.0)
                .unwrap()
                .with_stroke(Color::BLACK, width)
                .is_err()
        );
        assert!(
            PolylineFigure::new(0.0, 0.0, 10.0, 10.0)
                .unwrap()
                .with_width(width)
                .is_err()
        );
        assert!(
            PolygonFigure::from_points(vec![Point::ZERO])
                .unwrap()
                .with_stroke(Color::BLACK, width)
                .is_err()
        );
        assert!(
            ScalablePolygonFigure::new(Rectangle::ZERO, PointList::new())
                .unwrap()
                .with_stroke(Color::BLACK, width, Default::default())
                .is_err()
        );
    }
    assert_eq!(
        RectangleFigure::new(0.0, 0.0, 10.0, 10.0)
            .unwrap()
            .with_stroke(Color::BLACK, 0.0)
            .unwrap()
            .stroke_width(),
        0.0
    );
    let error = PolylineFigure::new(0.0, 0.0, 10.0, 10.0)
        .unwrap()
        .with_width(-1.0)
        .err()
        .unwrap();
    assert_eq!(
        error,
        ShapeGeometryError::StrokeInput(GraphicsInputError::NegativeStrokeWidth)
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn rounded_dimensions_and_translation_fail_before_writing_and_keep_diameter_units() {
    for radius in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(RoundedRectangleFigure::new(0.0, 0.0, 10.0, 10.0, radius).is_err());
    }
    let mut figure = RoundedRectangleFigure::new(-10.0, -20.0, 30.0, 40.0, 8.0).unwrap();
    let bounds = figure.bounds();
    assert!(
        figure
            .set_corner_dimensions(Dimension::new(12.0, f64::NAN))
            .is_err()
    );
    assert_eq!(figure.corner_dimensions(), Dimension::new(8.0, 8.0));
    assert!(figure.set_corner_radius(-2.0).is_err());
    assert_eq!(figure.corner_dimensions(), Dimension::new(8.0, 8.0));
    assert!(figure.translate(f64::INFINITY, 0.0).is_err());
    assert_eq!(figure.bounds(), bounds);
    figure.set_corner_radius(20.0).unwrap();
    assert_eq!(figure.corner_dimensions(), Dimension::new(20.0, 20.0));
    figure.translate(-3.0, 4.0).unwrap();
    assert_eq!(figure.bounds(), Rectangle::new(-13.0, -16.0, 30.0, 40.0));
}

#[test]
fn point_lists_reject_nan_span_stroke_envelope_and_scaling_overflow_atomically() {
    let invalid = vec![Point::new(-f64::MAX, 0.0), Point::new(f64::MAX, 1.0)];
    assert!(PolylineFigure::from_points(invalid.clone()).is_err());
    assert!(PolygonFigure::from_points(invalid).is_err());
    assert!(PolylineFigure::from_points(vec![Point::new(f64::NAN, 0.0)]).is_err());
    let mut line = PolylineFigure::new(0.0, 0.0, 1e-6, 1e-6)
        .unwrap()
        .with_width(0.0)
        .unwrap();
    let bounds = line.bounds();
    let points = line.get_points().to_vec();
    assert!(line.set_bounds(0.0, 0.0, f64::MAX, f64::MAX).is_err());
    assert_eq!(line.bounds(), bounds);
    assert_eq!(line.get_points(), points);
    assert!(line.add_point(f64::NAN, 2.0).is_err());
    assert_eq!(line.get_points(), points);
    assert!(
        line.set_points(vec![Point::new(f64::INFINITY, 0.0)])
            .is_err()
    );
    assert_eq!(line.get_points(), points);
    line.set_points(vec![Point::new(-10.0, -20.0), Point::new(10.0, 20.0)])
        .unwrap();
    assert_eq!(line.bounds(), Rectangle::new(-10.0, -20.0, 20.0, 40.0));
    let broad = PolylineFigure::new(-f64::MAX / 2.0, 0.0, f64::MAX / 2.0, 1.0).unwrap();
    let stroke = StrokeStyle::default().with_width(f64::MAX / 8.0).unwrap();
    assert!(broad.with_stroke_style(stroke).is_err());
}

#[test]
fn scaled_template_checks_span_and_avoids_overflow_from_tiny_valid_source_span() {
    let invalid =
        PointList::from_points(vec![Point::new(-f64::MAX, 0.0), Point::new(f64::MAX, 1.0)]);
    assert!(ScalablePolygonFigure::new(Rectangle::ZERO, invalid.clone()).is_err());
    let tiny = PointList::from_points(vec![Point::ZERO, Point::new(1e-10, 1e-10)]);
    let polygon =
        ScalablePolygonFigure::new(Rectangle::new(0.0, 0.0, f64::MAX, f64::MAX), tiny).unwrap();
    assert!(
        polygon
            .scaled_points(polygon.bounds())
            .iter()
            .all(|p| p.x().is_finite() && p.y().is_finite())
    );
    let mut runtime = Runtime::empty();
    let id = runtime.set_contents(Box::new(polygon)).unwrap();
    assert!(matches!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(ReplaceScalablePolygonTemplate::new(invalid)),
        Err(ComponentUpdateError::Rejected { .. })
    ));
}

#[test]
fn rectangle_and_ellipse_keep_custom_dash_and_checked_stroke_at_supplied_bounds() {
    let stroke = StrokeStyle::default()
        .with_width(3.0)
        .unwrap()
        .with_cap(LineCap::Round)
        .with_dash_pattern(DashPattern::Custom(
            CustomDash::try_new(&[2.0, 5.0]).unwrap(),
        ));
    let rectangle = RectangleFigure::new(0.0, 0.0, 100.0, 60.0)
        .unwrap()
        .with_stroke(Color::BLACK, 3.0)
        .unwrap()
        .with_stroke_style(stroke.clone());
    let ellipse = EllipseFigure::new(0.0, 0.0, 100.0, 60.0)
        .unwrap()
        .with_stroke(Color::BLACK, 3.0)
        .unwrap()
        .with_stroke_style(stroke);
    let bounds = Rectangle::new(0.0, 0.0, 80.0, 40.0);
    for figure in [&rectangle as &dyn Figure, &ellipse] {
        let mut recorder = CommandRecorder::new();
        {
            let mut context = recorder.paint();
            context.set_stroke_paint(Color::BLACK);
            context.set_fill_paint(Color::WHITE);
            figure.paint(&mut context, bounds).unwrap();
        }
        assert!(
            recorder
                .commands()
                .iter()
                .any(|command| match &command.kind {
                    novadraw::render::RenderCommandKind::StrokeRect { stroke: style, .. }
                    | novadraw::render::RenderCommandKind::Ellipse { stroke: style, .. } =>
                        style.width() == 3.0
                            && style.cap() == LineCap::Round
                            && style.dash_lengths().as_ref() == [2.0, 5.0],
                    _ => false,
                })
        );
    }
}

#[test]
fn resource_figures_check_raw_bounds_before_admission_and_allow_valid_retry() {
    use novadraw::{
        container::{DefaultRangeModel, ScrollBarFigure},
        figure::ImageFigure,
        scene::{ASPECT_FIT_VIEW, AspectFitView, SceneViewFigure},
    };
    let mut runtime = Runtime::empty();
    let image = runtime.resources().register_image().unwrap();
    let view = runtime
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let invalid = Rectangle::new(f64::MAX, 0.0, f64::MAX, 1.0);
    let valid = Rectangle::new(-5.0, -10.0, 0.0, 20.0);
    assert!(ImageFigure::new(image).with_bounds(invalid).is_err());
    assert!(
        ScrollBarFigure::new(std::sync::Arc::new(DefaultRangeModel::default()))
            .with_bounds(invalid)
            .is_err()
    );
    assert!(SceneViewFigure::new(view).with_bounds(invalid).is_err());
    let view = runtime
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    assert_eq!(
        SceneViewFigure::new(view)
            .with_bounds(valid)
            .unwrap()
            .initial_bounds(),
        valid
    );
    assert_eq!(
        ScrollBarFigure::new(std::sync::Arc::new(DefaultRangeModel::default()))
            .with_bounds(valid)
            .unwrap()
            .initial_bounds(),
        valid
    );
    let mut figures: Vec<Box<dyn Bounded>> = vec![Box::new(
        ImageFigure::new(image).with_bounds(valid).unwrap(),
    )];
    for figure in &mut figures {
        assert!(figure.set_bounds(f64::MAX, 0.0, f64::MAX, 1.0).is_err());
        assert_eq!(figure.bounds(), valid);
        figure.set_bounds(1.0, 2.0, 30.0, 40.0).unwrap();
        assert_eq!(figure.bounds(), Rectangle::new(1.0, 2.0, 30.0, 40.0));
    }
}

#[test]
fn viewport_origin_checks_both_coordinates_before_mutating_shared_ranges() {
    let viewport = ViewportFigure::new(0.0, 0.0, 100.0, 80.0)
        .unwrap()
        .with_origin(20.0, 30.0)
        .unwrap();
    let before = Bounded::child_transform(&viewport);
    for (x, y) in [(f64::NAN, 1.0), (40.0, f64::INFINITY)] {
        assert!(viewport.clone().with_origin(x, y).is_err());
        assert_eq!(Bounded::child_transform(&viewport), before);
    }
    assert!(viewport.clone().with_origin(-10.0, -20.0).is_ok());
}

#[test]
fn scroll_pane_host_checks_geometry_before_publishing_any_child() {
    let mut tree = novadraw::tree::FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::from_bounds(Rectangle::ZERO).unwrap(),
        ))
        .unwrap();
    assert!(
        tree.builder()
            .add_scroll_pane_to(root, Rectangle::new(f64::MAX, 0.0, f64::MAX, 1.0))
            .is_err()
    );
    assert!(tree.child_order(root).unwrap().is_empty());
    let pane = tree
        .builder()
        .add_scroll_pane_to(root, Rectangle::new(-5.0, -10.0, 0.0, 20.0))
        .unwrap();
    assert_eq!(
        tree.figure_bounds(pane.pane_id()).unwrap(),
        Rectangle::new(-5.0, -10.0, 0.0, 20.0)
    );
}

#[test]
fn rounded_triangle_resize_checks_derived_contour_before_replacing_bounds() {
    let mut triangle = TriangleFigure::new(Rectangle::new(0.0, 0.0, 30.0, 40.0))
        .unwrap()
        .with_corner_radii([4.0; 3])
        .unwrap();
    let before = triangle.bounds();
    assert!(triangle.set_bounds(0.0, 0.0, f64::MAX, f64::MAX).is_err());
    assert_eq!(triangle.bounds(), before);
    triangle.set_bounds(0.0, 0.0, 60.0, 80.0).unwrap();
}
