//! Canvas composition of printing/PrintExample.java (IBM and contributors, EPL-2.0).
use novadraw::{
    connection::{ChopboxAnchor, ConnectionFigure, RouterBinding},
    figure::{
        ButtonFigure, EllipseFigure, FigureStyle, LabelFigure, RectangleFigure,
        RoundedRectangleFigure, border::LineBorder,
    },
    geometry::Dimension,
    graphics::Color,
    layout::FlowLayout,
    runtime::Runtime,
};

use super::{
    runtime_with_fonts,
    system_fonts::{
        BOLD_FONT_WEIGHT, NORMAL_FONT_WEIGHT, RegisteredHostFont, SystemFontRequest,
        font_descriptor, register_system_fonts,
    },
};

const WIDTH: f64 = 300.0;
const HEIGHT: f64 = 500.0;
const SHAPE_SIZE: f64 = 50.0;
const CORNER_SIZE: f64 = 8.0;
const PRINT_FONT_REQUESTS: [SystemFontRequest; 3] = [
    SystemFontRequest {
        family: "Helvetica",
        weight: NORMAL_FONT_WEIGHT,
        fallback: novadraw::render::BuiltinFont::Inter,
    },
    SystemFontRequest {
        family: "Times New Roman",
        weight: BOLD_FONT_WEIGHT,
        fallback: novadraw::render::BuiltinFont::Inter,
    },
    SystemFontRequest {
        family: "Perpetua",
        weight: NORMAL_FONT_WEIGHT,
        fallback: novadraw::render::BuiltinFont::Inter,
    },
];
const PRINT_FONT_POINTS: [f32; 3] = [16.0, 24.0, 48.0];

pub fn print_example() -> Runtime {
    build_print_example().0
}

fn build_print_example() -> (Runtime, [RegisteredHostFont; 3]) {
    let mut runtime = runtime_with_fonts();
    let fonts: [RegisteredHostFont; 3] = register_system_fonts(&mut runtime, &PRINT_FONT_REQUESTS)
        .try_into()
        .expect("three print fonts");
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("print contents");
    runtime
        .figure(root)
        .unwrap()
        .set_minimum_size(Dimension::new(5.0, 5.0))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(FlowLayout::new()))
        .unwrap();
    for ((font, points), request) in fonts.iter().zip(PRINT_FONT_POINTS).zip(PRINT_FONT_REQUESTS) {
        let id = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                LabelFigure::new(request.family).with_border(LineBorder::default_border()),
            ))
            .unwrap();
        runtime
            .figure(id)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(Color::BLACK),
                font: Some(font_descriptor(font, points, request.weight)),
                ..FigureStyle::default()
            })
            .unwrap();
    }
    let rectangle = || {
        RectangleFigure::new_with_color(0.0, 0.0, SHAPE_SIZE, SHAPE_SIZE, Color::WHITE)
            .with_stroke(Color::BLACK, 1.0)
    };
    let shapes: Vec<Box<dyn novadraw::figure::Figure>> = vec![
        Box::new(rectangle()),
        Box::new(
            EllipseFigure::new_with_color(0.0, 0.0, SHAPE_SIZE, SHAPE_SIZE, Color::WHITE)
                .with_stroke(Color::BLACK, 1.0),
        ),
        Box::new(
            RoundedRectangleFigure::new_with_color(
                0.0,
                0.0,
                SHAPE_SIZE,
                SHAPE_SIZE,
                CORNER_SIZE,
                Color::WHITE,
            )
            .with_stroke(Color::BLACK, 1.0),
        ),
    ];
    for shape in shapes {
        let id = runtime.figure(root).unwrap().add(shape).unwrap();
        runtime
            .figure(id)
            .unwrap()
            .set_preferred_size(Dimension::new(SHAPE_SIZE, SHAPE_SIZE))
            .unwrap();
    }
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ButtonFigure::new("Button")))
        .unwrap();
    let source = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(rectangle()))
        .unwrap();
    let target = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(rectangle()))
        .unwrap();
    for id in [source, target] {
        runtime
            .figure(id)
            .unwrap()
            .set_preferred_size(Dimension::new(SHAPE_SIZE, SHAPE_SIZE))
            .unwrap();
    }
    let image = runtime.resources().register_image().unwrap();
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .unwrap();
    runtime
        .resources()
        .complete_image_request_bytes(request, include_bytes!("../../assets/brazil.ico"), 1.0)
        .expect("official brazil.ico");
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(LabelFigure::new("").with_icon(image)))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            ConnectionFigure::new().with_stroke(Color::BLACK, 1.0),
        ))
        .unwrap();
    let source = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)));
    let target = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = runtime.connections().direct_router();
    runtime
        .connections()
        .register_state(
            figure,
            Some(source),
            Some(target),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    (runtime, fonts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        geometry::{Point, Rectangle},
        render::{BackendCapabilities, SurfaceInfo, command::RenderCommandKind},
    };

    #[test]
    fn connection_follows_anchors_across_flow_wrap_thresholds() {
        let mut runtime = print_example();
        let root = runtime.tree().contents().unwrap();
        let children = runtime.tree().child_order(root).unwrap();
        let widths: Vec<_> = (260..=1000).step_by(10).collect();
        let mut first_pass = Vec::new();
        let mut saw_cross_row = false;
        let mut saw_same_row = false;
        for (index, width) in widths.iter().rev().chain(widths.iter()).enumerate() {
            runtime
                .frames()
                .resize_logical_viewport(f64::from(*width), HEIGHT)
                .unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let source = runtime.tree().figure_bounds(children[7]).unwrap();
            let target = runtime.tree().figure_bounds(children[8]).unwrap();
            saw_cross_row |= source.y != target.y;
            saw_same_row |= source.y == target.y;
            let route = runtime
                .tree()
                .connection_route_points(children[10])
                .unwrap()
                .unwrap();
            let transform = runtime
                .tree()
                .local_to_surface_transform(children[10])
                .unwrap();
            let points: Vec<_> = route
                .as_slice()
                .iter()
                .map(|point| transform.transform_point(*point))
                .collect();
            assert_eq!(points.len(), 2);
            for (actual, expected) in points.iter().zip([
                rectangle_endpoint(source, target),
                rectangle_endpoint(target, source),
            ]) {
                assert!(
                    (actual.x() - expected.x()).abs() < 1e-6
                        && (actual.y() - expected.y()).abs() < 1e-6,
                    "width={width}: endpoint {actual:?} must lie at {expected:?}"
                );
            }
            if index < widths.len() {
                first_pass.push(points);
            } else {
                assert_eq!(points, first_pass[widths.len() * 2 - index - 1]);
            }
        }
        assert!(saw_cross_row && saw_same_row);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }

    fn rectangle_endpoint(owner: Rectangle, other: Rectangle) -> Point {
        let center = Point::new(owner.x + owner.width / 2.0, owner.y + owner.height / 2.0);
        let dx = other.x + other.width / 2.0 - center.x();
        let dy = other.y + other.height / 2.0 - center.y();
        let factor = (dx.abs() / (owner.width / 2.0)).max(dy.abs() / (owner.height / 2.0));
        Point::new(center.x() + dx / factor, center.y() + dy / factor)
    }

    #[test]
    #[ignore = "requires native GPU; validates Print connection pixels across flow wrapping"]
    fn gpu_connection_pixels_stay_between_rectangle_anchors() {
        for dpi in [1.0, 2.0] {
            let mut canvas = super::super::gpu_test_support::GpuCanvas::new(
                (576.0 * dpi) as u32,
                (256.0 * dpi) as u32,
                dpi,
            );
            for width in [560.0, 500.0, 480.0] {
                let mut runtime = print_example();
                runtime
                    .frames()
                    .resize_logical_viewport(width, 256.0)
                    .unwrap();
                let connected = canvas.render(&mut runtime, true);
                let root = runtime.tree().contents().unwrap();
                let children = runtime.tree().child_order(root).unwrap();
                let source = runtime.tree().figure_bounds(children[7]).unwrap();
                let target = runtime.tree().figure_bounds(children[8]).unwrap();
                let start = rectangle_endpoint(source, target);
                let end = rectangle_endpoint(target, source);
                runtime
                    .figure(children[10])
                    .unwrap()
                    .set_visible(false)
                    .unwrap();
                let disconnected = canvas.render(&mut runtime, true);
                let dx = end.x() - start.x();
                let dy = end.y() - start.y();
                let mut changed_pixels = 0;
                for (index, (actual, absent)) in connected
                    .chunks_exact(4)
                    .zip(disconnected.chunks_exact(4))
                    .enumerate()
                {
                    if actual == absent {
                        continue;
                    }
                    changed_pixels += 1;
                    let pixel_width = (576.0 * dpi) as usize;
                    let x = (index % pixel_width) as f64 / dpi + 0.5 / dpi;
                    let y = (index / pixel_width) as f64 / dpi + 0.5 / dpi;
                    let t = (((x - start.x()) * dx + (y - start.y()) * dy) / (dx * dx + dy * dy))
                        .clamp(0.0, 1.0);
                    let distance = (x - start.x() - t * dx).hypot(y - start.y() - t * dy);
                    assert!(
                        distance <= 2.0,
                        "width={width}, dpi={dpi}: stray connection pixel at ({x}, {y})"
                    );
                }
                assert!(
                    changed_pixels > 0,
                    "width={width}, dpi={dpi}: missing connection"
                );
            }
        }
    }

    #[test]
    fn canvas_outputs_fonts_icon_and_rerouted_connection_after_resize() {
        let (mut runtime, fonts) = build_print_example();
        for (font, request) in fonts.iter().zip(PRINT_FONT_REQUESTS) {
            if font.system {
                assert_eq!(font.family, request.family);
            } else {
                assert_eq!(font.family, request.fallback.family());
            }
        }
        #[cfg(target_os = "macos")]
        assert!(
            fonts[0].system && fonts[1].system,
            "macOS must provide Helvetica and Times New Roman"
        );
        let root = runtime.tree().contents().unwrap();
        let children = runtime.tree().child_order(root).unwrap();
        assert_eq!(children.len(), 11);
        let mut routes = Vec::new();
        for width in [WIDTH, 600.0, WIDTH] {
            runtime
                .figure(root)
                .unwrap()
                .set_bounds(Rectangle::new(0.0, 0.0, width, HEIGHT))
                .unwrap();
            let submission = runtime
                .frames()
                .prepare_submission(
                    SurfaceInfo {
                        logical_width: width,
                        logical_height: HEIGHT,
                        pixel_width: width as u32,
                        pixel_height: HEIGHT as u32,
                        scale_factor: 1.0,
                    },
                    BackendCapabilities::RETAINED_PARTIAL,
                )
                .into_ready()
                .expect("graphics output");
            assert!(
                submission
                    .commands
                    .iter()
                    .any(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. }))
            );
            assert!(
                submission
                    .commands
                    .iter()
                    .any(|c| matches!(c.kind, RenderCommandKind::Image { .. }))
            );
            let glyph_runs = submission
                .commands
                .iter()
                .filter_map(|command| match &command.kind {
                    RenderCommandKind::DrawGlyphRun { run, .. } => Some(run),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (font, points) in fonts.iter().zip(PRINT_FONT_POINTS) {
                assert!(glyph_runs.iter().any(|run| {
                    run.font.resource_id() == font.resource_id
                        && (run.font_size
                            - points * super::super::system_fonts::LOGICAL_PIXELS_PER_POINT)
                            .abs()
                            < 1e-4
                }));
            }
            for id in &children[3..6] {
                let bounds = runtime.tree().figure_bounds(*id).unwrap();
                assert_eq!(bounds.width, SHAPE_SIZE);
                assert_eq!(bounds.height, SHAPE_SIZE);
            }
            let route = runtime
                .tree()
                .connection_route_points(children[10])
                .unwrap()
                .expect("print route");
            let transform = runtime
                .tree()
                .local_to_surface_transform(children[10])
                .unwrap();
            routes.push(
                route
                    .as_slice()
                    .iter()
                    .map(|point| transform.transform_point(*point))
                    .collect::<Vec<_>>(),
            );
            let _ = runtime
                .frames()
                .complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    Ok(novadraw::render::RenderDisposition::Presented),
                )
                .unwrap();
        }
        assert_ne!(routes[0], routes[1]);
        assert_eq!(routes[0], routes[2]);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
}
