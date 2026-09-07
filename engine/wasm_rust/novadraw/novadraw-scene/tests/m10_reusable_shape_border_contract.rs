use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::{Dimension, Rectangle, Vec2};
use novadraw_render::{NdCanvas, command::RenderCommandKind};
use novadraw_scene::{
    BevelBorder, BevelStyle, Border, CompoundBorder, Direction, EtchedBorder, Figure, FigureStyle,
    LineBorder, MarginBorder, NotificationEffect, PolygonFigure, PolylineFigure, PropertyValue,
    RectangleFigure, RoundedRectangleFigure, Runtime, ShapeMutationError, TriangleFigure,
};

#[test]
fn ellipse_fill_and_outline_share_optimized_bounds_and_preserve_stroke_width() {
    let ellipse =
        novadraw_scene::EllipseFigure::new_with_color(0.0, 0.0, 100.0, 60.0, Color::WHITE)
            .with_stroke(Color::BLACK, 4.0);
    let mut canvas = NdCanvas::new();
    canvas.fill_style(Color::WHITE);
    canvas.stroke_style(Color::BLACK);

    ellipse.paint_figure_in_bounds(&mut canvas, Rectangle::new(0.0, 0.0, 100.0, 60.0));

    let ellipses = canvas
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill_color,
                stroke_color,
                stroke_width,
                ..
            } => Some((
                *cx,
                *cy,
                *rx,
                *ry,
                *fill_color,
                *stroke_color,
                *stroke_width,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(ellipses.len(), 2);
    assert_eq!(
        ellipses[0],
        (50.0, 30.0, 48.0, 28.0, Some(Color::WHITE), None, 1.0)
    );
    assert_eq!(
        ellipses[1],
        (50.0, 30.0, 48.0, 28.0, None, Some(Color::BLACK), 4.0)
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
    let polyline = PolylineFigure::from_points(vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)])
        .with_width(2.0);
    let bounds = polyline.initial_bounds();

    assert!(polyline.precise_hit(50.0, 50.0, bounds));
    assert!(!polyline.precise_hit(5.0, 95.0, bounds));
}

#[test]
fn polygon_precise_hit_uses_closed_interior() {
    let polygon = PolygonFigure::from_points(vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(50.0, 100.0),
    ]);
    let bounds = polygon.initial_bounds();

    assert!(polygon.precise_hit(50.0, 30.0, bounds));
    assert!(!polygon.precise_hit(5.0, 90.0, bounds));
}

#[test]
fn triangle_uses_draw2d_resize_and_centering_geometry() {
    let triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
    let mut canvas = NdCanvas::new();
    canvas.fill_style(Color::hex("#e74c3c"));
    canvas.stroke_style(Color::hex("#c0392b"));

    triangle.paint_figure_in_bounds(&mut canvas, Rectangle::new(0.0, 0.0, 20.0, 20.0));

    let bounds = canvas
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::FillPath { path, .. } => path.bounding_box(),
            _ => None,
        })
        .expect("triangle fill path");

    assert!((bounds.x - 0.0).abs() < f64::EPSILON);
    assert!((bounds.y - 4.75).abs() < f64::EPSILON);
    assert!((bounds.z - 19.0).abs() < f64::EPSILON);
    assert!((bounds.w - 14.25).abs() < f64::EPSILON);
}

#[test]
fn runtime_point_mutations_commit_bounds_points_damage_and_notification_atomically() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let line = runtime.add_figure(
        root,
        Box::new(PolylineFigure::from_points(vec![
            Vec2::new(10.0, 20.0),
            Vec2::new(110.0, 20.0),
        ])),
    );

    assert_eq!(
        runtime.point_list_points(line).unwrap(),
        vec![Vec2::new(10.0, 20.0), Vec2::new(110.0, 20.0)]
    );
    assert!(runtime.set_point(line, 1, Vec2::new(210.0, 50.0)).unwrap());
    assert_eq!(
        runtime.tree().figure_bounds(line),
        Some(Rectangle::new(9.0, 19.0, 202.0, 32.0))
    );
    assert_eq!(
        runtime.point_list_points(line).unwrap(),
        vec![Vec2::new(10.0, 20.0), Vec2::new(210.0, 50.0)]
    );
    assert!(
        runtime
            .insert_point(line, 1, Vec2::new(80.0, 70.0))
            .unwrap()
    );
    assert!(runtime.remove_point(line, 1).unwrap());
    assert!(
        runtime
            .replace_points(line, vec![Vec2::new(20.0, 30.0), Vec2::new(120.0, 80.0)])
            .unwrap()
    );
    assert!(runtime.has_pending_update());
    assert!(
        runtime
            .tree()
            .notification_effects()
            .iter()
            .any(|effect| matches!(
                effect,
                NotificationEffect::EmitProperty(event)
                    if event.block_id == line
                        && event.property == "points"
                        && matches!(event.new_value, PropertyValue::PointList(_))
            ))
    );

    let stable_points = runtime.point_list_points(line).unwrap();
    let stable_bounds = runtime.tree().figure_bounds(line);
    assert_eq!(
        runtime.remove_point(line, 9),
        Err(ShapeMutationError::PointIndexOutOfRange { index: 9, len: 2 })
    );
    assert_eq!(
        runtime.replace_points(line, vec![Vec2::new(f64::NAN, 0.0)]),
        Err(ShapeMutationError::NonFiniteGeometry)
    );
    assert_eq!(runtime.point_list_points(line).unwrap(), stable_points);
    assert_eq!(runtime.tree().figure_bounds(line), stable_bounds);

    assert!(runtime.clear_points(line).unwrap());
    assert!(runtime.point_list_points(line).unwrap().is_empty());
    assert_eq!(runtime.tree().figure_bounds(line), Some(Rectangle::ZERO));
}

struct MetricBorder {
    insets: (f64, f64, f64, f64),
    preferred: (f64, f64),
}

impl Border for MetricBorder {
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    fn paint(&self, _figure_bounds: Rectangle, _gc: &mut NdCanvas) {}

    fn preferred_size(&self) -> (f64, f64) {
        self.preferred
    }
}

#[test]
fn border_metrics_and_compound_formula_match_draw2d() {
    let line = LineBorder::new(Color::BLACK, 2.0);
    assert_eq!(line.get_insets(), (2.0, 2.0, 2.0, 2.0));
    assert_eq!(line.preferred_size(), (0.0, 0.0));
    assert!(line.is_opaque());

    let margin = MarginBorder::new(Color::TRANSPARENT, 1.0).with_margins(1.0, 2.0, 3.0, 4.0);
    assert_eq!(margin.get_insets(), (1.0, 2.0, 3.0, 4.0));
    assert!(!margin.is_opaque());

    let compound = CompoundBorder::new(
        line,
        MetricBorder {
            insets: (3.0, 4.0, 5.0, 6.0),
            preferred: (10.0, 20.0),
        },
    );
    assert_eq!(compound.get_insets(), (5.0, 6.0, 7.0, 8.0));
    assert_eq!(compound.preferred_size(), (14.0, 24.0));
    assert!(!compound.is_opaque());
}

#[test]
fn compound_border_isolates_outer_state_and_offsets_inner_paint() {
    let compound = CompoundBorder::new(
        LineBorder::new(Color::RED, 2.0),
        LineBorder::new(Color::BLUE, 2.0),
    );
    let mut canvas = NdCanvas::new();

    compound.paint(Rectangle::new(0.0, 0.0, 40.0, 30.0), &mut canvas);

    let strokes = canvas
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::StrokeRect { rect, color, .. } => Some((*rect, *color)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(strokes.len(), 2);
    assert_eq!(strokes[0].0[0], glam::DVec2::new(1.0, 1.0));
    assert_eq!(strokes[1].0[0], glam::DVec2::new(3.0, 3.0));
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::PushState) })
    );
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::PopState) })
    );
}

#[test]
fn runtime_border_corner_and_direction_mutations_use_typed_transactions() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)));
    let rounded = runtime.add_figure(
        root,
        Box::new(RoundedRectangleFigure::new(10.0, 10.0, 100.0, 60.0, 8.0)),
    );
    let triangle = runtime.add_figure(root, Box::new(TriangleFigure::new(150.0, 20.0, 40.0, 40.0)));

    let shared_border: Arc<dyn Border> = Arc::new(LineBorder::new(Color::BLACK, 3.0));
    assert!(
        runtime
            .replace_border(rounded, Some(Arc::clone(&shared_border)))
            .unwrap()
    );
    assert_eq!(runtime.tree().insets(rounded), Some((3.0, 3.0, 3.0, 3.0)));
    assert!(
        runtime
            .set_corner_dimensions(rounded, Dimension::new(24.0, 12.0))
            .unwrap()
    );
    assert!(
        runtime
            .set_triangle_direction(triangle, Direction::West)
            .unwrap()
    );
    assert!(runtime.has_pending_update());

    assert_eq!(
        runtime.set_corner_dimensions(rounded, Dimension::new(-1.0, 2.0)),
        Err(ShapeMutationError::NegativeMetric)
    );
    assert_eq!(
        runtime.set_triangle_direction(rounded, Direction::South),
        Err(ShapeMutationError::WrongCapability(rounded))
    );
    let effect_count = runtime.tree().notification_effects().len();
    assert!(
        !runtime
            .replace_border(rounded, Some(shared_border))
            .unwrap()
    );
    assert_eq!(runtime.tree().notification_effects().len(), effect_count);
}

#[test]
fn reusable_shapes_consume_runtime_figure_style_as_color_truth() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)));
    let rectangle = runtime.add_figure(
        root,
        Box::new(
            RectangleFigure::new_with_color(20.0, 20.0, 80.0, 50.0, Color::RED)
                .with_stroke(Color::BLUE, 2.0),
        ),
    );
    let fill = Color::rgba(0.2, 0.7, 0.3, 1.0);
    let stroke = Color::rgba(0.8, 0.2, 0.6, 1.0);
    assert!(runtime.set_figure_style(
        rectangle,
        FigureStyle {
            foreground: Some(stroke),
            background: Some(fill),
            ..FigureStyle::default()
        },
    ));

    let canvas = runtime.tree().render();
    assert!(canvas.commands().iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::FillRect { color, .. } if color == fill
        )
    }));
    assert!(canvas.commands().iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::StrokeRect { color, width, .. }
                if color == stroke && width == 2.0
        )
    }));
}

#[test]
fn border_preferred_size_and_effective_opacity_join_figure_protocol() {
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)));
    let bordered = runtime.add_figure(
        root,
        Box::new(
            RectangleFigure::new(10.0, 10.0, 100.0, 50.0)
                .with_border(LineBorder::new(Color::BLACK, 3.0)),
        ),
    );

    assert_eq!(
        runtime.tree().preferred_size(bordered, -1.0, -1.0),
        Some((106.0, 56.0))
    );
    assert_eq!(
        runtime.tree().border_is_effectively_opaque(bordered),
        Some(true)
    );

    assert!(runtime.set_figure_style(
        bordered,
        FigureStyle {
            alpha: Some(0.5),
            ..runtime.tree().figure_style(bordered).unwrap().clone()
        },
    ));
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

    assert_eq!(etched.get_insets(), (2.0, 2.0, 2.0, 2.0));
    assert_eq!(raised.get_insets(), (2.0, 2.0, 2.0, 2.0));
    assert!(etched.is_opaque());
    assert!(raised.is_opaque());
    assert!(lowered.is_opaque());

    let mut canvas = NdCanvas::new();
    etched.paint(Rectangle::new(0.0, 0.0, 40.0, 30.0), &mut canvas);
    assert_eq!(
        canvas
            .commands()
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::Line { .. }))
            .count(),
        8
    );
}
