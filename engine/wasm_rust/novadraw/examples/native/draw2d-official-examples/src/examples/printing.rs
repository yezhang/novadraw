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

use super::runtime_with_fonts;

const WIDTH: f64 = 300.0;
const HEIGHT: f64 = 500.0;
const SHAPE_SIZE: f64 = 50.0;
const CORNER_SIZE: f64 = 8.0;

pub fn print_example() -> Runtime {
    let mut runtime = runtime_with_fonts();
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
    // Built-in faces make the fixture portable. Text preserves the source names.
    for (name, font) in [
        ("Helvetica", "16px Inter Variable"),
        ("Times New Roman", "bold 24px Inter Variable"),
        ("Perpetua", "48px Inter Variable"),
    ] {
        let id = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(
                LabelFigure::new(name).with_border(LineBorder::default_border()),
            ))
            .unwrap();
        runtime
            .figure(id)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(Color::BLACK),
                font: Some(font.to_owned()),
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
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        geometry::Rectangle,
        render::{BackendCapabilities, SurfaceInfo, command::RenderCommandKind},
    };

    #[test]
    fn canvas_outputs_fonts_icon_and_rerouted_connection_after_resize() {
        let mut runtime = print_example();
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
