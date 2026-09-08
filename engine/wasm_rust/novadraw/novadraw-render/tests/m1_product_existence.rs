use glam::DVec2;
use novadraw_core::Color;
use novadraw_render::{ImageResourceRef, LineStyle, NdCanvas, RenderCommandKind, ResourceId};
use uuid::Uuid;

#[test]
fn m1_graphics_shape_and_style_entries_emit_commands() {
    let mut canvas = NdCanvas::new();
    canvas.set_background_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
    canvas.set_foreground_color(Color::rgba(0.0, 0.0, 1.0, 1.0));
    canvas.set_line_width(3.0);
    canvas.set_line_style(LineStyle::Dash);

    canvas.fill_rectangle(0.0, 0.0, 10.0, 20.0);
    canvas.draw_rectangle(1.0, 2.0, 30.0, 40.0);
    canvas.fill_oval(2.0, 4.0, 20.0, 10.0);
    canvas.draw_oval(4.0, 8.0, 10.0, 6.0);
    canvas.fill_polygon(&[
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
        DVec2::new(5.0, 10.0),
    ]);
    canvas.draw_polygon(&[
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
        DVec2::new(5.0, 10.0),
    ]);

    let commands = canvas.commands();
    assert!(matches!(
        commands[0].kind,
        RenderCommandKind::FillRect { .. }
    ));

    let RenderCommandKind::StrokeRect {
        width, line_style, ..
    } = commands[1].kind
    else {
        panic!("expected draw_rectangle to emit StrokeRect");
    };
    assert_eq!(width, 3.0);
    assert_eq!(line_style, LineStyle::Dash);

    assert!(matches!(
        commands[2].kind,
        RenderCommandKind::Ellipse {
            fill_color: Some(_),
            stroke_color: None,
            ..
        }
    ));
    assert!(matches!(
        commands[3].kind,
        RenderCommandKind::Ellipse {
            fill_color: None,
            stroke_color: Some(_),
            line_style: LineStyle::Dash,
            ..
        }
    ));
    assert!(matches!(
        commands[4].kind,
        RenderCommandKind::FillPath { .. }
    ));
    assert!(matches!(
        commands[5].kind,
        RenderCommandKind::Polyline {
            line_style: LineStyle::Dash,
            ..
        }
    ));
}

#[test]
fn m1_graphics_image_clip_and_alpha_entries_emit_commands() {
    let mut canvas = NdCanvas::new();
    canvas.set_background_color(Color::rgba(0.0, 1.0, 0.0, 0.8));

    canvas.set_alpha(0.5);
    canvas.set_clip(1.0, 2.0, 30.0, 40.0);
    let image = ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 1, 2, 2, 1.0);
    canvas.draw_image(image, 7.0, 8.0);

    let commands = canvas.commands();
    assert!(matches!(
        commands[0].kind,
        RenderCommandKind::SetGlobalAlpha { alpha } if alpha == 0.5
    ));
    assert!(matches!(commands[1].kind, RenderCommandKind::ResetClip));
    assert!(matches!(commands[2].kind, RenderCommandKind::Clip { .. }));
    assert!(matches!(
        commands[3].kind,
        RenderCommandKind::Image { alpha, .. } if alpha == 0.5
    ));
}
