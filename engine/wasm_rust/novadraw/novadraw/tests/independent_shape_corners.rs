mod support;

use kurbo::{ParamCurve as _, Shape as _};
use novadraw::{
    figure::{
        EdgeStroke, Figure, PolygonFigure, RectangleBorderGeometry, RectangleCorners,
        RectangleEdges, RectangleFigure, RoundedRectangleFigure, Shape, ShapeContour,
        ShapeGeometryError, TriangleFigure,
        border::{Border, RectangleBorder},
    },
    geometry::{Dimension, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext, Path},
    render::{CommandRecorder, command::RenderCommandKind},
    runtime::{
        ComponentUpdateError, ReplacePolygonGeometry, Runtime, SetPolygonCornerRadii,
        SetRectangleCorners, SetRectangleEdges, SetRoundedRectangleCorners, SetTriangleCornerRadii,
    },
};

const BOX: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 100.0,
    height: 80.0,
};

fn triangle_points() -> Vec<Point> {
    vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(50.0, 80.0),
    ]
}

fn bez(path: &Path) -> kurbo::BezPath {
    use novadraw::render::backend_support::{NormalizedPathOp, for_each_normalized};
    let mut out = kurbo::BezPath::new();
    let p = |p: Point| kurbo::Point::new(p.x(), p.y());
    for_each_normalized(path.operations(), 1.0, |op| match op {
        NormalizedPathOp::MoveTo(v) => out.move_to(p(v)),
        NormalizedPathOp::LineTo(v) => out.line_to(p(v)),
        NormalizedPathOp::CubicTo(v) => out.curve_to(p(v.control1), p(v.control2), p(v.end)),
        NormalizedPathOp::QuadTo { control, end } => out.quad_to(p(control), p(end)),
        NormalizedPathOp::Close => out.close_path(),
    });
    out
}

#[test]
fn top_only_corners_leave_bottom_vertices_and_hit_geometry_intact() {
    let corners = RectangleCorners::circular([20.0, 10.0, 0.0, 0.0]).unwrap();
    let shape = RoundedRectangleFigure::from_bounds(BOX, 0.0)
        .with_corners(corners)
        .unwrap();
    assert!(!shape.precise_hit(0.0, 0.0, BOX));
    assert!(!shape.precise_hit(99.0, 1.0, BOX));
    assert!(shape.precise_hit(0.0, 80.0, BOX));
    assert!(shape.precise_hit(100.0, 80.0, BOX));
    assert!(shape.precise_hit(50.0, 1.0, BOX));
    assert_eq!(shape.corners(), Some(corners));
}

#[test]
fn elliptical_corner_matches_drawn_arc_instead_of_quadratic_approximation() {
    let corners = RectangleCorners::new([
        Dimension::new(40.0, 20.0),
        Dimension::ZERO,
        Dimension::ZERO,
        Dimension::ZERO,
    ])
    .unwrap();
    let contour = ShapeContour::rectangle(BOX, corners).unwrap();
    assert!(contour.contains(Point::new(40.0, 0.0)));
    assert!(contour.contains(Point::new(0.0, 20.0)));
    assert!(!contour.contains(Point::new(4.0, 2.0)));
    assert!(contour.contains(Point::new(16.0, 6.0)));
    let bounds = contour.path().bounding_box().unwrap();
    assert!(bounds.x.abs() < 1e-12 && bounds.y.abs() < 1e-12);
    assert!((bounds.width - BOX.width).abs() < 1e-12);
    assert!((bounds.height - BOX.height).abs() < 1e-12);
    let mut edge_only = RoundedRectangleFigure::from_bounds(BOX, 0.0);
    edge_only.set_corner_dimensions(Dimension::new(40.0, 20.0));
    let edge_only = edge_only
        .with_edges(RectangleEdges([
            EdgeStroke::Inherit,
            EdgeStroke::Inherit,
            EdgeStroke::None,
            EdgeStroke::Inherit,
        ]))
        .unwrap();
    let shared = ShapeContour::rectangle(
        BOX,
        RectangleCorners::new([Dimension::new(20.0, 10.0); 4]).unwrap(),
    )
    .unwrap();
    for segment in bez(shared.path()).segments() {
        for t in [0.2, 0.4, 0.6, 0.8] {
            let point = segment.eval(t);
            assert!(edge_only.precise_hit(point.x, point.y, BOX));
        }
    }
}

#[test]
fn oversized_corners_scale_together_without_changing_request() {
    let corners = RectangleCorners::circular([80.0, 80.0, 0.0, 0.0]).unwrap();
    let contour = ShapeContour::rectangle(BOX, corners).unwrap();
    assert!(contour.contains(Point::new(50.0, 0.0)));
    assert!(!contour.contains(Point::new(20.0, 5.0)));
    assert_eq!(corners.radii()[0], Dimension::new(80.0, 80.0));
    let b = contour.path().bounding_box().unwrap();
    assert!(b.x.abs() < 1e-12 && b.y.abs() < 1e-12);
    assert!((b.width - BOX.width).abs() < 1e-12);
    assert!((b.height - BOX.height).abs() < 1e-12);
}

#[test]
fn polygon_fillets_cut_all_three_vertices_and_share_paint_and_hit_path() {
    let points = triangle_points();
    let contour = ShapeContour::polygon(&points, &[15.0, 10.0, 20.0]).unwrap();
    for point in points {
        assert!(!contour.contains(point));
    }
    assert!(contour.contains(Point::new(50.0, 30.0)));
    assert!(!contour.contains(Point::new(50.0, 90.0)));
    assert_eq!(contour.effective_radii(), &[15.0, 10.0, 20.0]);
}

#[test]
fn excessive_polygon_radii_normalize_to_nonoverlapping_tangents() {
    let contour = ShapeContour::polygon(&triangle_points(), &[1_000.0; 3]).unwrap();
    let radii = contour.effective_radii();
    assert!(radii.iter().all(|r| *r > 0.0 && *r < 50.0));
    assert_eq!(radii[0], radii[1]);
    let bounds = contour.path().bounding_box().unwrap();
    assert!(bounds.x >= -1e-6 && bounds.y >= -1e-6);
    assert!(bounds.x + bounds.width <= 100.0 + 1e-6);
    assert!(bounds.y + bounds.height <= 80.0 + 1e-6);
}

#[test]
fn concave_corners_and_reversed_winding_have_same_region() {
    let mut points = vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(100.0, 80.0),
        Point::new(50.0, 40.0),
        Point::new(0.0, 80.0),
    ];
    let forward = ShapeContour::polygon(&points, &[8.0; 5]).unwrap();
    points.reverse();
    let reverse = ShapeContour::polygon(&points, &[8.0; 5]).unwrap();
    assert!(forward.contains(Point::new(50.0, 10.0)));
    assert!(!forward.contains(Point::new(50.0, 70.0)));
    for y in 0..80 {
        for x in 0..100 {
            let p = Point::new(f64::from(x) + 0.31, f64::from(y) + 0.27);
            assert_eq!(forward.contains(p), reverse.contains(p), "{p:?}");
        }
    }
}

#[test]
fn invalid_radii_and_topologies_return_structured_errors() {
    assert_eq!(
        RectangleCorners::uniform(f64::NAN),
        Err(ShapeGeometryError::NonFinite)
    );
    assert_eq!(
        RectangleCorners::uniform(-1.0),
        Err(ShapeGeometryError::NegativeMetric)
    );
    assert!(matches!(
        ShapeContour::polygon(&triangle_points(), &[1.0]),
        Err(ShapeGeometryError::RadiusCount {
            vertices: 3,
            radii: 1
        })
    ));
    let crossing = vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 80.0),
        Point::new(100.0, 0.0),
        Point::new(0.0, 80.0),
    ];
    assert!(matches!(
        ShapeContour::polygon(&crossing, &[5.0; 4]),
        Err(ShapeGeometryError::SelfIntersection)
    ));
    assert!(ShapeContour::polygon(&crossing, &[0.0; 4]).is_ok());
    let repeated_ring = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
    ];
    assert!(
        ShapeContour::polygon(&repeated_ring, &[0.0; 8])
            .unwrap()
            .contains(Point::new(5.0, 5.0))
    );
    assert!(matches!(
        ShapeContour::polygon(&[Point::new(0.0, 0.0); 3], &[5.0; 3]),
        Err(ShapeGeometryError::DegeneratePolygon)
    ));
    assert!(
        ShapeContour::polygon(&[], &[])
            .unwrap()
            .path()
            .operations()
            .is_empty()
    );
}

#[test]
fn rectangle_border_partitions_cover_ring_once_for_all_corner_sizes() {
    for radii in [[0.0; 4], [20.0, 10.0, 0.0, 30.0], [200.0; 4]] {
        let corners = RectangleCorners::circular(radii).unwrap();
        let outer = bez(ShapeContour::rectangle(BOX, corners).unwrap().path());
        let geometry = RectangleBorderGeometry::new(BOX, corners, 6.0).unwrap();
        let regions = geometry.regions().each_ref().map(bez);
        let inner_radii = if radii == [200.0; 4] {
            [34.0; 4]
        } else {
            radii.map(|r| (r - 6.0).max(0.0))
        };
        let inner = bez(ShapeContour::rectangle(
            Rectangle::new(6.0, 6.0, 88.0, 68.0),
            RectangleCorners::circular(inner_radii).unwrap(),
        )
        .unwrap()
        .path());
        for y in 0..80 {
            for x in 0..100 {
                let p = kurbo::Point::new(f64::from(x) + 0.17, f64::from(y) + 0.43);
                let count = regions.iter().filter(|path| path.winding(p) != 0).count();
                assert!(count <= 1, "overlapping edge colors at {p:?}");
                assert_eq!(
                    count == 1,
                    outer.winding(p) != 0 && inner.winding(p) == 0,
                    "missing ring coverage at {p:?}, radii={radii:?}"
                );
            }
        }
    }
}

#[test]
fn thick_border_and_zero_size_never_create_negative_geometry() {
    for bounds in [Rectangle::new(0.0, 0.0, 8.0, 3.0), Rectangle::ZERO] {
        let geometry =
            RectangleBorderGeometry::new(bounds, RectangleCorners::uniform(20.0).unwrap(), 100.0)
                .unwrap();
        for region in geometry.regions() {
            if let Some(b) = region.bounding_box() {
                assert!(b.width.is_finite() && b.height.is_finite());
                assert!(b.x >= 0.0 && b.y >= 0.0);
                assert!(b.width <= bounds.width + 1e-6 && b.height <= bounds.height + 1e-6);
            }
        }
    }
}

#[test]
fn disabled_bottom_produces_three_colors_and_no_bottom_region() {
    let edges = RectangleEdges([
        EdgeStroke::Color(Color::rgba8(255, 0, 0, 128)),
        EdgeStroke::Color(Color::WHITE),
        EdgeStroke::None,
        EdgeStroke::Color(Color::BLACK),
    ]);
    let border = RectangleBorder::new(Color::WHITE, 4.0)
        .with_edges(edges)
        .with_corners(RectangleCorners::circular([20.0, 10.0, 0.0, 0.0]).unwrap());
    assert!(!border.is_opaque());
    let mut recorder = CommandRecorder::new();
    border.paint(BOX, &mut recorder.paint()).unwrap();
    let paths: Vec<_> = recorder
        .commands()
        .iter()
        .filter_map(|c| match &c.kind {
            RenderCommandKind::FillPath { path, paint, .. } => Some((path, paint)),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 3);
    assert!(
        paths
            .iter()
            .all(|(p, _)| bez(p).winding((50.0, 79.0).into()) == 0)
    );
}

fn painted_edge_paths(
    geometry: &RectangleBorderGeometry,
    edges: RectangleEdges,
) -> Vec<kurbo::BezPath> {
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_antialias(novadraw::graphics::AntialiasMode::Off);
        geometry
            .paint(&mut context, edges, Some(Color::BLACK.into()))
            .unwrap();
    }
    recorder
        .commands()
        .iter()
        .filter_map(|c| match &c.kind {
            RenderCommandKind::FillPath { path, .. } => Some(bez(path)),
            _ => None,
        })
        .collect()
}

#[test]
fn all_sharp_edge_masks_cover_full_enabled_strips_without_overlap() {
    const WIDTH: f64 = 6.0;
    let geometry = RectangleBorderGeometry::new(BOX, RectangleCorners::ZERO, WIDTH).unwrap();
    let colors = [
        Color::BLACK,
        Color::WHITE,
        Color::rgba8(255, 0, 0, 255),
        Color::rgba8(0, 0, 255, 255),
    ];
    for mask in 0_u8..16 {
        let edges = RectangleEdges(std::array::from_fn(|i| {
            if mask & (1 << i) != 0 {
                EdgeStroke::Color(colors[i])
            } else {
                EdgeStroke::None
            }
        }));
        let paths = painted_edge_paths(&geometry, edges);
        for y in 0..80 {
            for x in 0..100 {
                let p = kurbo::Point::new(f64::from(x) + 0.31, f64::from(y) + 0.43);
                let strips = [
                    p.y < WIDTH,
                    p.x > BOX.width - WIDTH,
                    p.y > BOX.height - WIDTH,
                    p.x < WIDTH,
                ];
                let expected = strips
                    .iter()
                    .enumerate()
                    .any(|(i, inside)| *inside && mask & (1 << i) != 0);
                let count = paths.iter().filter(|path| path.winding(p) != 0).count();
                assert_eq!(count, usize::from(expected), "mask={mask:04b}, point={p:?}");
            }
        }
    }
}

#[test]
fn rounded_free_ends_stop_at_straight_tangents_and_keep_the_fill_contour() {
    let corners = RectangleCorners::uniform(20.0).unwrap();
    let geometry = RectangleBorderGeometry::new(BOX, corners, 6.0).unwrap();
    let paths = painted_edge_paths(
        &geometry,
        RectangleEdges([
            EdgeStroke::None,
            EdgeStroke::None,
            EdgeStroke::None,
            EdgeStroke::Color(Color::BLACK),
        ]),
    );
    assert_eq!(paths.len(), 1);
    assert_ne!(paths[0].winding((3.0, 40.0).into()), 0);
    assert_eq!(paths[0].winding((3.0, 12.0).into()), 0);
    assert_eq!(paths[0].winding((3.0, 68.0).into()), 0);
    // The curved corner remains part of the shape's fill/hit geometry.
    assert!(
        ShapeContour::rectangle(BOX, corners)
            .unwrap()
            .contains(Point::new(3.0, 12.0))
    );
}

#[test]
fn runtime_geometry_updates_are_atomic_and_identical_updates_are_noops() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::from_bounds(BOX)))
        .unwrap();
    let id = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(PolygonFigure::from_points(triangle_points())))
        .unwrap();
    support::record_full_submission(&mut runtime);
    let before = runtime.tree().component_revision(id).unwrap();
    let bounds = runtime.tree().figure_bounds(id).unwrap();
    assert!(matches!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetPolygonCornerRadii::new(vec![5.0, -1.0, 5.0])),
        Err(ComponentUpdateError::Rejected(
            ShapeGeometryError::NegativeMetric
        ))
    ));
    assert_eq!(runtime.tree().component_revision(id).unwrap(), before);
    assert_eq!(runtime.tree().figure_bounds(id).unwrap(), bounds);
    assert!(!runtime.frames().has_pending_update());
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetPolygonCornerRadii::new(vec![5.0; 3]))
            .unwrap()
            .changed
    );
    support::record_full_submission(&mut runtime);
    let revision = runtime.tree().component_revision(id).unwrap();
    assert!(
        !runtime
            .figure(id)
            .unwrap()
            .update_component(SetPolygonCornerRadii::new(vec![5.0; 3]))
            .unwrap()
            .changed
    );
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert!(!runtime.frames().has_pending_update());
    let points = vec![
        Point::new(20.0, 30.0),
        Point::new(80.0, 30.0),
        Point::new(80.0, 70.0),
        Point::new(20.0, 70.0),
    ];
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(ReplacePolygonGeometry::new(
                points,
                vec![8.0, 0.0, 5.0, 0.0]
            ))
            .unwrap()
            .changed
    );
    assert_ne!(runtime.tree().figure_bounds(id).unwrap(), bounds);
    let huge = Rectangle::new(0.0, 0.0, 1e200, 1e200);
    runtime.figure(id).unwrap().set_bounds(huge).unwrap();
    let revision = runtime.tree().component_revision(id).unwrap();
    assert!(matches!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetPolygonCornerRadii::new(vec![9.0; 4])),
        Err(ComponentUpdateError::Rejected(
            ShapeGeometryError::NonFinite
        ))
    ));
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert_eq!(runtime.tree().figure_bounds(id).unwrap(), huge);
}

#[test]
fn runtime_corner_mode_can_return_to_legacy_even_with_same_dimensions() {
    let mut runtime = Runtime::empty();
    let id = runtime
        .set_contents(Box::new(RoundedRectangleFigure::from_bounds(BOX, 10.0)))
        .unwrap();
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetRectangleCorners::new(Some(
                RectangleCorners::circular([20.0, 20.0, 0.0, 0.0]).unwrap(),
            )))
            .unwrap()
            .changed
    );
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetRoundedRectangleCorners::new(Dimension::new(10.0, 10.0)))
            .unwrap()
            .changed
    );
    assert!(
        !runtime
            .figure(id)
            .unwrap()
            .update_component(SetRoundedRectangleCorners::new(Dimension::new(10.0, 10.0)))
            .unwrap()
            .changed
    );
    let edges = RectangleEdges([
        EdgeStroke::Inherit,
        EdgeStroke::Inherit,
        EdgeStroke::None,
        EdgeStroke::Inherit,
    ]);
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetRectangleEdges::new(edges))
            .unwrap()
            .changed
    );
    assert!(
        !runtime
            .figure(id)
            .unwrap()
            .update_component(SetRectangleEdges::new(edges))
            .unwrap()
            .changed
    );
}

#[test]
fn rounded_triangle_updates_use_runtime_bounds_and_remove_tip_from_hit() {
    let mut runtime = Runtime::empty();
    let id = runtime
        .set_contents(Box::new(TriangleFigure::new(BOX)))
        .unwrap();
    assert!(
        runtime
            .figure(id)
            .unwrap()
            .update_component(SetTriangleCornerRadii::new([10.0, 8.0, 6.0]))
            .unwrap()
            .changed
    );
    assert!(
        !runtime
            .figure(id)
            .unwrap()
            .update_component(SetTriangleCornerRadii::new([10.0, 8.0, 6.0]))
            .unwrap()
            .changed
    );
    let figure = TriangleFigure::new(BOX)
        .with_corner_radii([10.0; 3])
        .unwrap();
    assert!(!figure.precise_hit(49.5, 0.0, BOX));
    assert!(figure.precise_hit(49.5, 30.0, BOX));
}

struct ExternalContourFigure(ShapeContour);

impl Figure for ExternalContourFigure {
    fn name(&self) -> &'static str {
        "ExternalContourFigure"
    }
    fn initial_bounds(&self) -> Rectangle {
        BOX
    }
    fn initial_style(&self) -> novadraw::figure::FigureStyle {
        novadraw::figure::FigureStyle {
            background: Some(Color::WHITE),
            ..Default::default()
        }
    }
    fn paint(&self, context: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        context.fill_path(self.0.path())
    }
    fn precise_hit(&self, x: f64, y: f64, _: Rectangle) -> bool {
        self.0.contains(Point::new(x, y))
    }
}

#[test]
fn external_figure_reuses_public_contour_without_registry_or_backend_changes() {
    let shape =
        ExternalContourFigure(ShapeContour::polygon(&triangle_points(), &[12.0; 3]).unwrap());
    let mut runtime = Runtime::empty();
    runtime.set_contents(Box::new(shape)).unwrap();
    let frame = support::record_full_submission(&mut runtime);
    assert!(
        frame
            .commands
            .iter()
            .any(|c| matches!(c.kind, RenderCommandKind::FillPath { .. }))
    );
}

#[test]
fn polygon_shape_entry_points_use_the_same_rounded_path() {
    let polygon = PolygonFigure::from_points(triangle_points())
        .with_corner_radii(vec![12.0; 3])
        .unwrap();
    let mut direct = CommandRecorder::new();
    {
        let mut context = direct.paint();
        context.set_fill_paint(Color::WHITE);
        polygon.fill_shape(&mut context).unwrap();
    }
    let mut figure = CommandRecorder::new();
    {
        let mut context = figure.paint();
        context.set_fill_paint(Color::WHITE);
        Figure::paint(&polygon, &mut context, polygon.initial_bounds()).unwrap();
    }
    let fill = |recorder: &CommandRecorder| {
        recorder
            .commands()
            .iter()
            .find_map(|c| match &c.kind {
                RenderCommandKind::FillPath { path, .. } => Some(path.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(fill(&direct), fill(&figure));
}

#[test]
fn rectangle_shape_border_records_through_runtime_snapshot_and_preserves_legacy_literal() {
    use novadraw::figure::border::BorderStyle;
    use novadraw::geometry::Insets;
    let legacy = RectangleBorder {
        color: Color::BLACK,
        width: 4.0,
        style: BorderStyle::Solid,
        insets: Insets::uniform(4.0),
    };
    let border = legacy
        .with_corners(RectangleCorners::uniform(20.0).unwrap())
        .with_edges(RectangleEdges([
            EdgeStroke::Color(Color::WHITE),
            EdgeStroke::Inherit,
            EdgeStroke::None,
            EdgeStroke::Inherit,
        ]));
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(
            RectangleFigure::from_bounds(BOX).with_border(border),
        ))
        .unwrap();
    let frame = runtime
        .frames()
        .prepare_submission(
            novadraw::render::SurfaceInfo {
                logical_width: 100.0,
                logical_height: 80.0,
                pixel_width: 100,
                pixel_height: 80,
                scale_factor: 1.0,
            },
            novadraw::render::BackendCapabilities::RETAINED_PARTIAL
                .with_path_clips()
                .with_per_operation_antialias(),
        )
        .into_ready()
        .expect("border snapshot records within its advertised budget");
    assert_eq!(
        frame
            .commands
            .iter()
            .filter(|c| matches!(c.kind, RenderCommandKind::FillPath { .. }))
            .count(),
        3
    );
}

#[test]
#[ignore = "release CPU construction and stable access cost evidence"]
fn contour_cost() {
    use std::{hint::black_box, time::Instant};
    for n in [4, 32, 128] {
        let points: Vec<_> = (0..n)
            .map(|i| {
                let theta = i as f64 * std::f64::consts::TAU / n as f64;
                Point::new(100.0 + 100.0 * theta.cos(), 100.0 + 100.0 * theta.sin())
            })
            .collect();
        let radii = vec![3.0; n];
        let mut times = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            black_box(ShapeContour::polygon(black_box(&points), black_box(&radii)).unwrap());
            times.push(start.elapsed().as_secs_f64() * 1e6);
        }
        times.sort_by(f64::total_cmp);
        println!("n={n}: construction p95={} us", times[95]);
    }
    let contour = ShapeContour::polygon(&triangle_points(), &[10.0; 3]).unwrap();
    let start = Instant::now();
    for _ in 0..100_000 {
        black_box(black_box(&contour).path());
    }
    println!(
        "stable path access={} ns/op",
        start.elapsed().as_secs_f64() * 1e9 / 100_000.0
    );
}
