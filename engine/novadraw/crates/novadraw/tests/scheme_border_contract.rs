use novadraw::{
    figure::{
        RectangleFigure,
        border::{Border, CompoundBorder, MarginBorder, SchemeBorder},
    },
    geometry::{Insets, Point, Rectangle},
    graphics::{Color, Paint},
    render::{CommandRecorder, command::RenderCommandKind},
    runtime::Runtime,
};

#[test]
fn external_consumer_reserves_missing_layers_and_accumulates_insets() {
    let scheme = SchemeBorder::new(vec![Some(Color::WHITE), None], vec![Some(Color::BLACK)]);
    assert_eq!(scheme.get_insets(), Insets::new(2.0, 2.0, 1.0, 1.0));
    assert!(!scheme.is_opaque());
    let border = CompoundBorder::new(
        MarginBorder::new(Insets::new(3.0, 4.0, 5.0, 6.0)).unwrap(),
        scheme,
    )
    .unwrap();
    let mut recorder = CommandRecorder::new();
    border
        .paint(
            Rectangle::new(10.0, 20.0, 50.0, 40.0),
            &mut recorder.paint(),
        )
        .unwrap();
    let lines: Vec<_> = recorder
        .commands()
        .iter()
        .filter_map(|c| {
            if let RenderCommandKind::Line {
                p1,
                p2,
                paint: Paint::Solid(color),
                stroke,
                ..
            } = &c.kind
            {
                Some((*p1, *p2, *color, stroke.width()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines[0],
        (
            Point::new(53.5, 54.5),
            Point::new(53.5, 23.5),
            Color::BLACK,
            1.0
        )
    );
    assert_eq!(
        lines[2],
        (
            Point::new(14.5, 23.5),
            Point::new(52.5, 23.5),
            Color::WHITE,
            1.0
        )
    );
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 200.0, 100.0)
                .expect("valid figure bounds")
                .with_border(border)
                .expect("valid detached figure input"),
        ))
        .unwrap();
    assert_eq!(
        runtime.tree().insets(root),
        Ok(Insets::new(5.0, 6.0, 6.0, 7.0))
    );
}

#[test]
fn opacity_requires_solid_complete_bands_and_small_bounds_never_reverse_lines() {
    assert!(SchemeBorder::new(vec![Some(Color::WHITE)], vec![Some(Color::BLACK)]).is_opaque());
    assert!(
        !SchemeBorder::new(vec![Some(Color::WHITE)], vec![Some(Color::TRANSPARENT)]).is_opaque()
    );
    assert!(!SchemeBorder::new(vec![Some(Color::WHITE); 2], vec![Some(Color::BLACK)]).is_opaque());
    let border = SchemeBorder::new(vec![Some(Color::WHITE); 4], vec![Some(Color::BLACK); 4]);
    for size in [0.0, 0.5, 1.0, 2.0, 3.0] {
        let mut recorder = CommandRecorder::new();
        border
            .paint(
                Rectangle::new(10.0, 20.0, size, size),
                &mut recorder.paint(),
            )
            .unwrap();
        for command in recorder.commands() {
            if let RenderCommandKind::Line { p1, p2, .. } = command.kind {
                assert!(p1.x() >= 10.0 && p2.x() >= 10.0 && p1.y() >= 20.0 && p2.y() >= 20.0);
                assert!((p1.x() - p2.x()).abs() <= size && (p1.y() - p2.y()).abs() <= size);
            }
        }
    }
}
