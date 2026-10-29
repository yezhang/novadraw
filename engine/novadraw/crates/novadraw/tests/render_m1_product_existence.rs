use novadraw::geometry::{Point, Rectangle};
use novadraw::graphics::Color;
use novadraw::graphics::{DashPattern, Path, StrokeStyle};
use novadraw::render::{CommandRecorder, ImageResourceRef, RenderCommandKind, ResourceId};
use uuid::Uuid;

#[test]
fn m1_graphics_shape_and_style_entries_emit_commands() {
    let mut recorder = CommandRecorder::new();
    let mut polygon = Path::new();
    polygon.move_to(0.0, 0.0);
    polygon.line_to(10.0, 0.0);
    polygon.line_to(5.0, 10.0);
    polygon.close();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::RED);
        context.set_stroke_paint(Color::BLUE);
        context.set_stroke_style(
            StrokeStyle::default()
                .with_width(3.0)
                .unwrap()
                .with_dash_pattern(DashPattern::Dash),
        );
        context
            .fill_rect(Rectangle::new(0.0, 0.0, 10.0, 20.0))
            .unwrap();
        context
            .stroke_rect(Rectangle::new(1.0, 2.0, 30.0, 40.0))
            .unwrap();
        context
            .fill_ellipse(Rectangle::new(2.0, 4.0, 20.0, 10.0))
            .unwrap();
        context
            .stroke_ellipse(Rectangle::new(4.0, 8.0, 10.0, 6.0))
            .unwrap();
        context.fill_path(&polygon).unwrap();
        context
            .stroke_polyline(&[
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(5.0, 10.0),
                Point::new(0.0, 0.0),
            ])
            .unwrap();
    }

    let commands = recorder.commands();
    assert!(matches!(
        commands[0].kind,
        RenderCommandKind::FillRect { .. }
    ));

    let RenderCommandKind::StrokeRect { stroke, .. } = &commands[1].kind else {
        panic!("expected draw_rectangle to emit StrokeRect");
    };
    assert_eq!(stroke.width(), 3.0);
    assert_eq!(
        stroke.dash_pattern(),
        &novadraw::graphics::DashPattern::Dash
    );

    assert!(matches!(
        commands[2].kind,
        RenderCommandKind::Ellipse {
            fill_paint: Some(_),
            stroke_paint: None,
            ..
        }
    ));
    assert!(matches!(
        commands[3].kind,
        RenderCommandKind::Ellipse {
            fill_paint: None,
            stroke_paint: Some(_),
            ref stroke,
            ..
        } if stroke.dash_pattern() == &novadraw::graphics::DashPattern::Dash
    ));
    assert!(matches!(
        commands[4].kind,
        RenderCommandKind::FillPath { .. }
    ));
    assert!(matches!(
        commands[5].kind,
        RenderCommandKind::Polyline {
            ref stroke,
            ..
        } if stroke.dash_pattern() == &novadraw::graphics::DashPattern::Dash
    ));
}

#[test]
fn m1_graphics_image_clip_and_alpha_entries_emit_commands() {
    let mut recorder = CommandRecorder::new();
    let image = ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 1, 2, 2, 1.0);
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::rgba(0.0, 1.0, 0.0, 0.8));
        context
            .set_opacity(novadraw::graphics::Opacity::try_new(0.5).unwrap())
            .unwrap();
        context
            .clip_rect(Rectangle::new(1.0, 2.0, 30.0, 40.0))
            .unwrap();
        context
            .draw_image(image, Rectangle::new(7.0, 8.0, 2.0, 2.0))
            .unwrap();
    }

    let commands = recorder.commands();
    assert!(matches!(
        commands[0].kind,
        RenderCommandKind::SetGlobalAlpha { alpha } if alpha == 0.5
    ));
    assert!(matches!(commands[1].kind, RenderCommandKind::Clip { .. }));
    assert!(matches!(
        commands[2].kind,
        RenderCommandKind::Image { alpha, .. } if alpha == 0.5
    ));
}
