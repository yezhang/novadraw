mod support;

use support::record_full_submission;

use std::sync::Arc;

use novadraw::figure::{
    Direction, ShapeMutationError,
    border::{
        BevelBorder, BevelStyle, Border, CompoundBorder, EtchedBorder, LineBorder, MarginBorder,
    },
};
use novadraw::geometry::{Dimension, Insets, Point, Rectangle};
use novadraw::graphics::Color;
use novadraw::graphics::{GraphicsError, Paint, PaintContext};
use novadraw::render::{
    CommandRecorder, DEFAULT_STROKE_MITER_LIMIT, LineJoin, command::RenderCommandKind,
};
use novadraw::runtime::{
    ClearPolylinePoints, ComponentUpdateError, InsertPolylinePoint, RemovePolylinePoint,
    ReplaceFigureBorder, ReplacePolylinePoints, SetPolylineLineJoin, SetPolylinePoint,
    SetPolylineStrokeWidth, SetRoundedRectangleCorners, SetTriangleDirection,
};
use novadraw::{
    figure::Figure, figure::FigureStyle, figure::MeasureConstraints, figure::PolygonFigure,
    figure::PolylineFigure, figure::RectangleFigure, figure::RoundedRectangleFigure,
    figure::TriangleFigure, runtime::Runtime,
};

#[test]
fn ellipse_fill_and_outline_share_optimized_bounds_and_preserve_stroke_width() {
    let ellipse =
        novadraw::figure::EllipseFigure::new_with_color(0.0, 0.0, 100.0, 60.0, Color::WHITE)
            .with_stroke(Color::BLACK, 4.0);
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::WHITE);
        context.set_stroke_paint(Color::BLACK);
        ellipse
            .paint(&mut context, Rectangle::new(0.0, 0.0, 100.0, 60.0))
            .unwrap();
    }

    let ellipses = recorder
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill_paint,
                stroke_paint,
                stroke,
                ..
            } => Some((
                *cx,
                *cy,
                *rx,
                *ry,
                fill_paint.clone(),
                stroke_paint.clone(),
                stroke.width(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(ellipses.len(), 2);
    assert_eq!(
        ellipses[0],
        (
            50.0,
            30.0,
            48.0,
            28.0,
            Some(Paint::Solid(Color::WHITE)),
            None,
            1.0
        )
    );
    assert_eq!(
        ellipses[1],
        (
            50.0,
            30.0,
            48.0,
            28.0,
            None,
            Some(Paint::Solid(Color::BLACK)),
            4.0
        )
    );
}

#[test]
fn rounded_rectangle_fill_and_outline_share_optimized_path() {
    for stroke_width in [1.0_f64, 2.0, 4.0, 8.0] {
        let rounded =
            RoundedRectangleFigure::new_with_color(0.0, 0.0, 100.0, 80.0, 30.0, Color::RED)
                .with_stroke(Color::WHITE, stroke_width);
        let mut recorder = CommandRecorder::new();
        {
            let mut context = recorder.paint();
            context.set_fill_paint(Color::RED);
            context.set_stroke_paint(Color::WHITE);
            rounded
                .paint(&mut context, Rectangle::new(0.0, 0.0, 100.0, 80.0))
                .unwrap();
        }

        let fill_path = recorder
            .commands()
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::FillPath { path, .. } => Some(path),
                _ => None,
            })
            .expect("rounded rectangle fill path");
        let (stroke_path, actual_stroke_width) = recorder
            .commands()
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::StrokePath { path, stroke, .. } => Some((path, stroke.width())),
                _ => None,
            })
            .expect("rounded rectangle stroke path");
        let inset = stroke_width.max(1.0) / 2.0;

        assert_eq!(fill_path, stroke_path);
        assert_eq!(
            fill_path.bounding_box(),
            Some(Rectangle::new(
                inset,
                inset,
                100.0 - inset * 2.0,
                80.0 - inset * 2.0,
            ))
        );
        assert_eq!(actual_stroke_width, stroke_width);
    }
}

#[test]
fn rounded_rectangle_fill_without_outline_uses_full_bounds() {
    let rounded = RoundedRectangleFigure::new_with_color(0.0, 0.0, 100.0, 80.0, 30.0, Color::RED);
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::RED);
        rounded
            .paint(&mut context, Rectangle::new(0.0, 0.0, 100.0, 80.0))
            .unwrap();
    }

    let fill_bounds = recorder
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::FillPath { path, .. } => path.bounding_box(),
            _ => None,
        })
        .expect("rounded rectangle fill path");

    assert_eq!(fill_bounds, Rectangle::new(0.0, 0.0, 100.0, 80.0));
    assert!(
        recorder
            .commands()
            .iter()
            .all(|command| !matches!(command.kind, RenderCommandKind::StrokePath { .. }))
    );
}

#[test]
fn rounded_rectangle_precise_hit_rejects_clipped_corner() {
    let rounded = RoundedRectangleFigure::new(0.0, 0.0, 100.0, 60.0, 20.0);
    let bounds = Rectangle::new(0.0, 0.0, 100.0, 60.0);

    assert!(!rounded.precise_hit(1.0, 1.0, bounds));
    assert!(rounded.precise_hit(50.0, 30.0, bounds));
    assert!(rounded.precise_hit(50.0, 1.0, bounds));
}

#[test]
fn polyline_precise_hit_uses_segment_distance() {
    let polyline =
        PolylineFigure::from_points(vec![Point::new(0.0, 0.0), Point::new(100.0, 100.0)])
            .with_width(2.0);
    let bounds = polyline.initial_bounds();

    assert!(polyline.precise_hit(50.0, 50.0, bounds));
    assert!(!polyline.precise_hit(5.0, 95.0, bounds));
}

#[test]
fn point_list_visual_bounds_follow_line_join_contract() {
    let points = vec![
        Point::new(10.0, 20.0),
        Point::new(60.0, 21.0),
        Point::new(110.0, 20.0),
    ];
    let miter = PolylineFigure::from_points(points.clone()).with_width(4.0);
    let round = PolylineFigure::from_points(points.clone())
        .with_width(4.0)
        .with_join(LineJoin::Round);
    let bevel = PolygonFigure::from_points(points)
        .with_stroke(Color::BLACK, 4.0)
        .with_join(LineJoin::Bevel);

    let miter_outset = 2.0 * DEFAULT_STROKE_MITER_LIMIT;
    assert_eq!(
        miter.initial_bounds(),
        Rectangle::new(
            10.0 - miter_outset,
            20.0 - miter_outset,
            100.0 + miter_outset * 2.0,
            1.0 + miter_outset * 2.0,
        )
    );
    assert_eq!(
        round.initial_bounds(),
        Rectangle::new(8.0, 18.0, 104.0, 5.0)
    );
    assert_eq!(
        bevel.initial_bounds(),
        Rectangle::new(8.0, 18.0, 104.0, 5.0)
    );
}

#[test]
fn polygon_precise_hit_uses_closed_interior() {
    let polygon = PolygonFigure::from_points(vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(50.0, 100.0),
    ]);
    let bounds = polygon.initial_bounds();

    assert!(polygon.precise_hit(50.0, 30.0, bounds));
    assert!(!polygon.precise_hit(5.0, 90.0, bounds));
}

#[test]
fn triangle_uses_draw2d_resize_and_centering_geometry() {
    let triangle = TriangleFigure::new(Rectangle::new(0.0, 0.0, 20.0, 20.0));
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::from_hex("#e74c3c").expect("valid color literal"));
        context.set_stroke_paint(Color::from_hex("#c0392b").expect("valid color literal"));
        triangle
            .paint(&mut context, Rectangle::new(0.0, 0.0, 20.0, 20.0))
            .unwrap();
    }

    let bounds = recorder
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::FillPath { path, .. } => path.bounding_box(),
            _ => None,
        })
        .expect("triangle fill path");

    assert!((bounds.x - 0.0).abs() < f64::EPSILON);
    assert!((bounds.y - 4.75).abs() < f64::EPSILON);
    assert!((bounds.x + bounds.width - 19.0).abs() < f64::EPSILON);
    assert!((bounds.y + bounds.height - 14.25).abs() < f64::EPSILON);
}

#[test]
fn runtime_point_mutations_commit_bounds_points_damage_and_notification_atomically() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)))
        .expect("valid Runtime mutation");
    let line = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(PolylineFigure::from_points(vec![
            Point::new(10.0, 20.0),
            Point::new(110.0, 20.0),
        ])))
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime.tree().point_list_points(line).unwrap(),
        vec![Point::new(10.0, 20.0), Point::new(110.0, 20.0)]
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(SetPolylinePoint::new(1, Point::new(210.0, 50.0)))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert_eq!(
        runtime.tree().figure_bounds(line),
        Some(Rectangle::new(6.0, 16.0, 208.0, 38.0))
    );
    assert_eq!(
        runtime.tree().point_list_points(line).unwrap(),
        vec![Point::new(10.0, 20.0), Point::new(210.0, 50.0)]
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(InsertPolylinePoint::new(1, Point::new(80.0, 70.0)))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(RemovePolylinePoint::new(1))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(ReplacePolylinePoints::new(vec![
                Point::new(20.0, 30.0),
                Point::new(120.0, 80.0),
            ]))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(runtime.frames().has_pending_update());
    let stable_points = runtime.tree().point_list_points(line).unwrap();
    let stable_bounds = runtime.tree().figure_bounds(line);
    assert_eq!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(RemovePolylinePoint::new(9)),
        Err(ComponentUpdateError::Rejected(
            ShapeMutationError::PointIndexOutOfRange { index: 9, len: 2 }
        ))
    );
    assert_eq!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(ReplacePolylinePoints::new(vec![Point::new(f64::NAN, 0.0)])),
        Err(ComponentUpdateError::Rejected(
            ShapeMutationError::NonFiniteGeometry
        ))
    );
    assert_eq!(
        runtime.tree().point_list_points(line).unwrap(),
        stable_points
    );
    assert_eq!(runtime.tree().figure_bounds(line), stable_bounds);

    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(ClearPolylinePoints)
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(runtime.tree().point_list_points(line).unwrap().is_empty());
    assert_eq!(runtime.tree().figure_bounds(line), Some(Rectangle::ZERO));
}

#[test]
fn runtime_point_list_stroke_style_renormalizes_geometry_atomically() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let line = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(PolylineFigure::from_points(vec![
            Point::new(20.0, 30.0),
            Point::new(100.0, 32.0),
            Point::new(180.0, 30.0),
        ])))
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime.tree().figure_bounds(line),
        Some(Rectangle::new(16.0, 26.0, 168.0, 10.0))
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(SetPolylineLineJoin::new(LineJoin::Bevel))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert_eq!(
        runtime.tree().figure_bounds(line),
        Some(Rectangle::new(19.0, 29.0, 162.0, 4.0))
    );
    assert!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(SetPolylineStrokeWidth::new(6.0))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert_eq!(
        runtime.tree().figure_bounds(line),
        Some(Rectangle::new(17.0, 27.0, 166.0, 8.0))
    );

    let stable_bounds = runtime.tree().figure_bounds(line);
    assert_eq!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(SetPolylineStrokeWidth::new(f64::NAN)),
        Err(ComponentUpdateError::Rejected(
            ShapeMutationError::NonFiniteGeometry
        ))
    );
    assert_eq!(
        runtime
            .figure(line)
            .unwrap()
            .update_component(SetPolylineStrokeWidth::new(-1.0)),
        Err(ComponentUpdateError::Rejected(
            ShapeMutationError::NegativeMetric
        ))
    );
    assert_eq!(runtime.tree().figure_bounds(line), stable_bounds);
}

struct MetricBorder {
    insets: Insets,
    preferred: Dimension,
}

impl Border for MetricBorder {
    fn get_insets(&self) -> Insets {
        self.insets
    }

    fn paint(
        &self,
        _figure_bounds: Rectangle,
        _context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        Ok(())
    }

    fn preferred_size(&self) -> Dimension {
        self.preferred
    }
}

#[test]
fn border_metrics_and_compound_formula_match_draw2d() {
    let line = LineBorder::new(Color::BLACK, 2.0);
    assert_eq!(line.get_insets(), Insets::uniform(2.0));
    assert_eq!(line.preferred_size(), Dimension::ZERO);
    assert!(line.is_opaque());

    let margin = MarginBorder::new(Color::TRANSPARENT, 1.0).with_margins(1.0, 2.0, 3.0, 4.0);
    assert_eq!(margin.get_insets(), Insets::new(1.0, 2.0, 3.0, 4.0));
    assert!(!margin.is_opaque());

    let compound = CompoundBorder::new(
        line,
        MetricBorder {
            insets: Insets::new(3.0, 4.0, 5.0, 6.0),
            preferred: Dimension::new(10.0, 20.0),
        },
    );
    assert_eq!(compound.get_insets(), Insets::new(5.0, 6.0, 7.0, 8.0));
    assert_eq!(compound.preferred_size(), Dimension::new(14.0, 24.0));
    assert!(!compound.is_opaque());
}

#[test]
fn compound_border_isolates_outer_state_and_offsets_inner_paint() {
    let compound = CompoundBorder::new(
        LineBorder::new(Color::RED, 2.0),
        LineBorder::new(Color::BLUE, 2.0),
    );
    let mut recorder = CommandRecorder::new();
    compound
        .paint(Rectangle::new(0.0, 0.0, 40.0, 30.0), &mut recorder.paint())
        .unwrap();

    let strokes = recorder
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::StrokeRect {
                rect,
                paint: Paint::Solid(color),
                ..
            } => Some((*rect, *color)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(strokes.len(), 2);
    assert_eq!(strokes[0].0.top_left(), Point::new(1.0, 1.0));
    assert_eq!(strokes[1].0.top_left(), Point::new(3.0, 3.0));
    assert!(
        recorder
            .commands()
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::PushState) })
    );
    assert!(
        recorder
            .commands()
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::PopState) })
    );
}

#[test]
fn runtime_border_corner_and_direction_mutations_use_typed_transactions() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .expect("valid Runtime mutation");
    let rounded = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RoundedRectangleFigure::new(
            10.0, 10.0, 100.0, 60.0, 8.0,
        )))
        .expect("valid Runtime mutation");
    let triangle = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(TriangleFigure::new(Rectangle::new(
            150.0, 20.0, 40.0, 40.0,
        ))))
        .expect("valid Runtime mutation");

    let shared_border: Arc<dyn Border> = Arc::new(LineBorder::new(Color::BLACK, 3.0));
    assert!(
        runtime
            .figure(rounded)
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(Arc::clone(&shared_border))))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert_eq!(runtime.tree().insets(rounded), Some(Insets::uniform(3.0)));
    assert!(
        runtime
            .figure(rounded)
            .unwrap()
            .update_component(SetRoundedRectangleCorners::new(Dimension::new(24.0, 12.0)))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(
        runtime
            .figure(triangle)
            .unwrap()
            .update_component(SetTriangleDirection::new(Direction::West))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
    assert!(runtime.frames().has_pending_update());

    assert_eq!(
        runtime
            .figure(rounded)
            .unwrap()
            .update_component(SetRoundedRectangleCorners::new(Dimension::new(-1.0, 2.0))),
        Err(ComponentUpdateError::Rejected(
            ShapeMutationError::NegativeMetric
        ))
    );
    assert!(matches!(
        runtime
            .figure(rounded)
            .unwrap()
            .update_component(SetTriangleDirection::new(Direction::West)),
        Err(ComponentUpdateError::WrongFigureType { figure, .. }) if figure == rounded
    ));
    assert!(
        !runtime
            .figure(rounded)
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(shared_border)))
            .map(|receipt| receipt.changed)
            .unwrap()
    );
}

#[test]
fn reusable_shapes_consume_runtime_figure_style_as_color_truth() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let rectangle = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(20.0, 20.0, 80.0, 50.0, Color::RED)
                .with_stroke(Color::BLUE, 2.0),
        ))
        .expect("valid Runtime mutation");
    let fill = Color::rgba(0.2, 0.7, 0.3, 1.0);
    let stroke = Color::rgba(0.8, 0.2, 0.6, 1.0);
    assert!(
        runtime
            .figure(rectangle)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(stroke),
                background: Some(fill),
                ..FigureStyle::default()
            },)
            .expect("valid Runtime mutation")
    );

    let canvas = record_full_submission(&mut runtime);
    assert!(canvas.commands.iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::FillRect { paint: Paint::Solid(color), .. } if color == fill
        )
    }));
    assert!(canvas.commands.iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::StrokeRect { paint: Paint::Solid(color), stroke: ref style, .. }
                if color == stroke && style.width() == 2.0
        )
    }));
}

#[test]
fn border_preferred_size_and_effective_opacity_join_figure_protocol() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let bordered = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(10.0, 10.0, 100.0, 50.0)
                .with_border(LineBorder::new(Color::BLACK, 3.0)),
        ))
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime
            .tree()
            .preferred_measurement(bordered, MeasureConstraints::UNBOUNDED)
            .map(|measurement| measurement.size()),
        Some(Dimension::new(106.0, 56.0))
    );
    assert_eq!(
        runtime.tree().border_is_effectively_opaque(bordered),
        Some(true)
    );

    let translucent_style = FigureStyle {
        alpha: Some(0.5),
        ..runtime.tree().figure_style(bordered).unwrap().clone()
    };
    assert!(
        runtime
            .figure(bordered)
            .unwrap()
            .set_style(translucent_style)
            .expect("valid Runtime mutation")
    );
    assert_eq!(
        runtime.tree().border_is_effectively_opaque(bordered),
        Some(false)
    );
}

#[test]
fn etched_and_bevel_borders_expose_product_metrics_and_commands() {
    let highlight = Color::WHITE;
    let shadow = Color::BLACK;
    let etched = EtchedBorder::new(highlight, shadow);
    let raised = BevelBorder::new(BevelStyle::Raised, highlight, shadow, 2);
    let lowered = BevelBorder::new(BevelStyle::Lowered, highlight, shadow, 2);

    assert_eq!(etched.get_insets(), Insets::uniform(2.0));
    assert_eq!(raised.get_insets(), Insets::uniform(2.0));
    assert!(etched.is_opaque());
    assert!(raised.is_opaque());
    assert!(lowered.is_opaque());

    let mut recorder = CommandRecorder::new();
    etched
        .paint(Rectangle::new(0.0, 0.0, 40.0, 30.0), &mut recorder.paint())
        .unwrap();
    assert_eq!(
        recorder
            .commands()
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::Line { .. }))
            .count(),
        8
    );
}
