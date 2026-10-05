//! P2-G01 contracts exercised exclusively through the public graphics API.

use novadraw::graphics::{
    ClipPath, CustomDash, DashPattern, FillRule, GraphicsInputError, LineCap, LineJoin, Path,
    StrokeStyle,
};

#[test]
fn clip_paths_validate_structure_and_own_curves_without_consuming_current_path() {
    use novadraw::render::RenderCommandKind;
    use novadraw::{Color, NdCanvas};
    let mut source = Path::new();
    source.move_to(0.0, 0.0);
    source.cubic_to(0.0, 10.0, 10.0, 10.0, 10.0, 0.0);
    let clip = ClipPath::try_new(&source, FillRule::EvenOdd).unwrap();
    source.line_to(100.0, 100.0);
    assert_eq!(clip.path().operations().len(), 2);
    assert_eq!(clip.rule(), FillRule::EvenOdd);

    let mut canvas = NdCanvas::new();
    canvas.set_background_color(Color::BLACK);
    canvas.begin_path();
    canvas.rect_path(1.0, 2.0, 3.0, 4.0);
    canvas.clip_path(&clip);
    canvas.fill();
    assert_eq!(canvas.clip_depth(), 1);
    assert!(matches!(&canvas.commands()[0].kind,
        RenderCommandKind::ClipPath { clip: recorded } if recorded == &clip));
    assert!(matches!(&canvas.commands()[1].kind,
        RenderCommandKind::FillPath { path, .. }
        if path.bounding_box() == Some(novadraw::Rectangle::new(1.0, 2.0, 3.0, 4.0))));

    let mut invalid = Path::new();
    invalid.line_to(1.0, 2.0);
    assert_eq!(
        ClipPath::try_new(&invalid, FillRule::NonZero),
        Err(GraphicsInputError::InvalidPath { index: 0 })
    );
    invalid = Path::new();
    invalid.move_to(0.0, 0.0);
    invalid.quad_to(f64::NAN, 0.0, 1.0, 1.0);
    assert_eq!(
        ClipPath::try_new(&invalid, FillRule::NonZero),
        Err(GraphicsInputError::NonFinite {
            field: "clip path",
            index: Some(1)
        })
    );
}

#[test]
fn mixed_clip_depth_and_fill_rule_restore_after_reset_and_pop() {
    use novadraw::render::{BackendCapabilities, RenderCapability, RenderCommandKind};
    use novadraw::{Color, NdCanvas};
    let empty = ClipPath::try_new(&Path::new(), FillRule::EvenOdd).unwrap();
    let mut canvas = NdCanvas::new();
    canvas.set_background_color(Color::BLACK);
    canvas.clip_rect(0.0, 0.0, 100.0, 100.0);
    canvas.clip_path(&empty);
    canvas.set_fill_rule(FillRule::EvenOdd);
    canvas.push_state();
    canvas.reset_clip();
    canvas.set_fill_rule(FillRule::NonZero);
    canvas.restore_state();
    assert_eq!(canvas.clip_depth(), 2);
    canvas.begin_path();
    canvas.rect_path(0.0, 0.0, 10.0, 10.0);
    canvas.fill();
    assert!(matches!(
        canvas.commands().last().unwrap().kind,
        RenderCommandKind::FillPath {
            rule: FillRule::EvenOdd,
            ..
        }
    ));
    canvas.reset_clip();
    canvas.pop_state();
    assert_eq!(canvas.clip_depth(), 2);
    let missing = BackendCapabilities::FULL_FRAME_ONLY
        .validate_capabilities(canvas.commands())
        .unwrap_err();
    assert_eq!(missing.capability, RenderCapability::PathClips);
    assert!(
        BackendCapabilities::FULL_FRAME_ONLY
            .with_path_clips()
            .validate_capabilities(canvas.commands())
            .is_ok()
    );
}

#[test]
fn linear_gradient_owns_ordered_stops_and_preserves_hard_edges() {
    use novadraw::graphics::{GradientStop, LinearGradient};
    use novadraw::{Color, Point};
    let mut stops = vec![
        GradientStop::try_new(0.0, Color::BLACK).unwrap(),
        GradientStop::try_new(0.5, Color::BLACK).unwrap(),
        GradientStop::try_new(0.5, Color::WHITE).unwrap(),
        GradientStop::try_new(1.0, Color::WHITE).unwrap(),
    ];
    let gradient = LinearGradient::try_new(Point::ORIGIN, Point::new(10.0, 0.0), &stops).unwrap();
    stops[1] = GradientStop::try_new(0.25, Color::WHITE).unwrap();
    assert_eq!(gradient.stops()[1].offset(), 0.5);
    assert_eq!(gradient.stops()[1].color(), Color::BLACK);
    assert_eq!(gradient.stops()[2].color(), Color::WHITE);
    assert_eq!(gradient.start(), Point::ORIGIN);
    assert_eq!(gradient.end(), Point::new(10.0, 0.0));
}

#[test]
fn invalid_gradients_reject_degenerate_endpoints_and_unordered_stops() {
    use novadraw::graphics::{GradientStop, LinearGradient};
    use novadraw::{Color, Point};
    let stop = |offset| GradientStop::try_new(offset, Color::BLACK).unwrap();
    let start = Point::ORIGIN;
    let end = Point::new(10.0, 0.0);
    for offset in [-1.0, 2.0, f64::NAN, f64::INFINITY] {
        assert!(GradientStop::try_new(offset, Color::BLACK).is_err());
    }
    assert_eq!(
        LinearGradient::try_new(start, start, &[stop(0.0), stop(1.0)]),
        Err(GraphicsInputError::DegenerateGradient)
    );
    assert_eq!(
        LinearGradient::try_new(start, end, &[stop(0.0)]),
        Err(GraphicsInputError::InsufficientGradientStops)
    );
    assert_eq!(
        LinearGradient::try_new(start, end, &[stop(0.1), stop(1.0)]),
        Err(GraphicsInputError::GradientEndpointStops)
    );
    assert_eq!(
        LinearGradient::try_new(start, end, &[stop(0.0), stop(0.8), stop(0.2), stop(1.0)]),
        Err(GraphicsInputError::GradientStopOrder { index: 2 })
    );
    assert!(
        LinearGradient::try_new(
            start,
            Point::new(f64::INFINITY, 0.0),
            &[stop(0.0), stop(1.0)]
        )
        .is_err()
    );
}

fn stroke(dash: DashPattern, offset: f64, miter: f64) -> StrokeStyle {
    StrokeStyle::try_new(2.0, LineCap::Butt, LineJoin::Miter, dash, offset, miter)
        .expect("valid stroke")
}

fn gradient_paint() -> novadraw::graphics::Paint {
    use novadraw::graphics::{GradientStop, LinearGradient};
    use novadraw::{Color, Point};
    LinearGradient::try_new(
        Point::ORIGIN,
        Point::new(100.0, 0.0),
        &[
            GradientStop::try_new(0.0, Color::BLACK).unwrap(),
            GradientStop::try_new(1.0, Color::WHITE.with_alpha(0.5)).unwrap(),
        ],
    )
    .unwrap()
    .into()
}

#[test]
fn paint_snapshots_apply_alpha_once_and_color_setters_replace_gradients() {
    use novadraw::graphics::Paint;
    use novadraw::render::RenderCommandKind;
    use novadraw::{Color, NdCanvas};
    let paint = gradient_paint();
    let mut canvas = NdCanvas::new();
    canvas.set_fill_paint(paint.clone());
    canvas.set_stroke_paint(paint.clone());
    canvas.set_alpha(0.5);
    canvas.push_state();
    canvas.set_background_color(Color::BLACK);
    canvas.fill_rectangle(0.0, 0.0, 10.0, 10.0);
    canvas.restore_state();
    canvas.fill_oval(0.0, 0.0, 10.0, 10.0);
    canvas.set_foreground_color(Color::WHITE);
    canvas.draw_rectangle(0.0, 0.0, 10.0, 10.0);
    canvas.pop_state();
    canvas.begin_path();
    canvas.rect_path(0.0, 0.0, 10.0, 10.0);
    canvas.fill_and_stroke();
    let paints: Vec<_> = canvas
        .commands()
        .iter()
        .flat_map(|c| c.kind.paints())
        .collect();
    assert_eq!(paints.len(), 5);
    assert_eq!(paints[0], &Paint::Solid(Color::BLACK.with_alpha(0.5)));
    assert_eq!(paints[2], &Paint::Solid(Color::WHITE.with_alpha(0.5)));
    for index in [1, 3, 4] {
        let Paint::LinearGradient(gradient) = paints[index] else {
            panic!("gradient lost")
        };
        assert_eq!(gradient.stops()[0].color().alpha(), 0.5);
        assert_eq!(gradient.stops()[1].color().alpha(), 0.25);
    }
    let Paint::LinearGradient(original) = paint else {
        unreachable!()
    };
    assert_eq!(original.stops()[0].color().alpha(), 1.0);
    assert!(
        canvas
            .commands()
            .iter()
            .any(|c| matches!(c.kind, RenderCommandKind::StrokePath { .. }))
    );
}

#[test]
fn gradient_glyph_stroke_requires_each_capability_and_preserves_full_style() {
    use novadraw::render::text::{FontDescriptor, GlyphPaint, TextConstraints};
    use novadraw::render::{BackendCapabilities, RenderCapability};
    use novadraw::{NdCanvas, Runtime};
    let mut runtime = Runtime::empty();
    runtime
        .register_builtin_font(novadraw::render::BuiltinFont::Inter)
        .unwrap();
    let layout = runtime
        .layout_text(
            "Paint",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let style = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        2.0,
        8.0,
    );
    let mut canvas = NdCanvas::new();
    canvas.set_stroke_paint(gradient_paint());
    canvas.set_stroke(style.clone());
    canvas.stroke_text_layout(&layout, 12.0, 24.0);
    let commands = canvas.commands();
    assert!(!commands.is_empty());
    for command in commands {
        assert!(matches!(&command.kind,
            novadraw::render::RenderCommandKind::DrawGlyphRun {
                paint: GlyphPaint::Stroke { paint, stroke }, ..
            } if paint == &gradient_paint() && stroke == &style));
        assert_eq!(
            command.kind.required_capabilities().collect::<Vec<_>>(),
            vec![
                RenderCapability::GlyphRuns,
                RenderCapability::CustomStrokes,
                RenderCapability::LinearGradients
            ]
        );
    }
    let all = BackendCapabilities::FULL_FRAME_ONLY
        .with_glyph_runs()
        .with_custom_strokes()
        .with_linear_gradients();
    assert_eq!(all.validate_capabilities(commands), Ok(()));
    for missing in [
        RenderCapability::GlyphRuns,
        RenderCapability::CustomStrokes,
        RenderCapability::LinearGradients,
    ] {
        let mut caps = all;
        match missing {
            RenderCapability::GlyphRuns => caps.glyph_runs = false,
            RenderCapability::CustomStrokes => caps.custom_strokes = false,
            RenderCapability::LinearGradients => caps.linear_gradients = false,
            _ => unreachable!(),
        }
        assert_eq!(
            caps.validate_capabilities(commands).unwrap_err().capability,
            missing
        );
    }
}

#[test]
fn ellipse_checks_both_paints_and_its_stroke() {
    use novadraw::render::{BackendCapabilities, RenderCapability};
    use novadraw::{Color, NdCanvas};
    for fill_gradient in [true, false] {
        let (fill, outline) = if fill_gradient {
            (gradient_paint(), Color::BLACK.into())
        } else {
            (Color::WHITE.into(), gradient_paint())
        };
        let mut canvas = NdCanvas::new();
        canvas.ellipse_with_paints(
            10.0,
            10.0,
            8.0,
            4.0,
            Some(fill),
            Some(outline),
            stroke(DashPattern::Dash, 2.0, 8.0),
            FillRule::EvenOdd,
        );
        assert_eq!(
            BackendCapabilities::FULL_FRAME_ONLY
                .with_custom_strokes()
                .validate_capabilities(canvas.commands())
                .unwrap_err()
                .capability,
            RenderCapability::LinearGradients
        );
        assert_eq!(
            BackendCapabilities::FULL_FRAME_ONLY
                .with_linear_gradients()
                .validate_capabilities(canvas.commands())
                .unwrap_err()
                .capability,
            RenderCapability::CustomStrokes
        );
    }
}

#[test]
fn device_preflight_rejects_overflow_and_gradient_collapse_with_command_index() {
    use novadraw::render::validate_graphics_input;
    use novadraw::{Color, NdCanvas};
    let mut canvas = NdCanvas::new();
    canvas.set_fill_paint(gradient_paint());
    canvas.translate(1e20, 0.0);
    canvas.fill_rectangle(0.0, 0.0, 100.0, 10.0);
    let error = validate_graphics_input(canvas.commands(), 1.0).unwrap_err();
    assert_eq!(error.command_index, 1);
    assert_eq!(error.reason, GraphicsInputError::DegenerateDeviceGradient);

    let mut canvas = NdCanvas::new();
    canvas.stroke_rect_with_style(
        0.0,
        0.0,
        10.0,
        10.0,
        Color::BLACK,
        StrokeStyle::default().with_width(1e38).unwrap(),
    );
    assert!(matches!(
        validate_graphics_input(canvas.commands(), 10.0)
            .unwrap_err()
            .reason,
        GraphicsInputError::DeviceValueOverflow {
            field: "stroke width"
        }
    ));
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.cubic_to(1e38, 0.0, 1e38, 10.0, 0.0, 10.0);
    let mut canvas = NdCanvas::new();
    canvas.clip_path(&ClipPath::try_new(&path, FillRule::NonZero).unwrap());
    assert!(validate_graphics_input(canvas.commands(), 10.0).is_err());
}

#[test]
fn device_preflight_replays_transform_restore_for_paints_and_clips() {
    use novadraw::NdCanvas;
    use novadraw::render::validate_graphics_input;
    let mut canvas = NdCanvas::new();
    canvas.set_fill_paint(gradient_paint());
    canvas.rotate_degrees(30.0);
    canvas.scale(2.0, 0.5);
    canvas.push_state();
    canvas.scale(0.0, 0.0);
    canvas.restore_state();
    canvas.fill_rectangle(0.0, 0.0, 100.0, 10.0);
    canvas.pop_state();
    canvas.fill_rectangle(0.0, 0.0, 100.0, 10.0);
    for dpi in [1.0, 2.0] {
        assert_eq!(validate_graphics_input(canvas.commands(), dpi), Ok(()));
    }
}

#[test]
fn rejected_runtime_graphics_preserve_frame_number_and_resource_snapshot_for_retry() {
    use novadraw::render::{BackendCapabilities, BuiltinFont, FrameId, ResourceSync, SurfaceInfo};
    use novadraw::{Figure, FramePreparation, FramePreparationError, NdCanvas, Rectangle, Runtime};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct ExternalGradient(Arc<AtomicBool>);
    impl Figure for ExternalGradient {
        fn name(&self) -> &'static str {
            "external-gradient"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn paint_figure(&self, canvas: &mut NdCanvas) {
            canvas.set_fill_paint(gradient_paint());
            if self.0.load(Ordering::Relaxed) {
                canvas.translate(1e20, 0.0);
            }
            canvas.fill_rectangle(0.0, 0.0, 100.0, 10.0);
        }
    }
    let invalid = Arc::new(AtomicBool::new(true));
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .set_contents(Box::new(ExternalGradient(invalid.clone())))
        .unwrap();
    let surface = SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 200,
        pixel_height: 200,
        scale_factor: 2.0,
    };
    let capabilities = BackendCapabilities::FULL_FRAME_ONLY.with_linear_gradients();
    assert!(matches!(
        runtime.prepare_submission(surface, BackendCapabilities::FULL_FRAME_ONLY),
        FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(_))
    ));
    for _ in 0..2 {
        assert!(matches!(
            runtime.prepare_submission(surface, capabilities),
            FramePreparation::Error(FramePreparationError::InvalidGraphicsInput(_))
        ));
    }
    invalid.store(false, Ordering::Relaxed);
    let FramePreparation::Ready(submission) = runtime.prepare_submission(surface, capabilities)
    else {
        panic!("corrected paint must be retriable")
    };
    assert_eq!(submission.frame_id, FrameId::INITIAL);
    let ResourceSync::Snapshot(snapshot) = submission.resources else {
        panic!("initial snapshot lost")
    };
    assert_eq!(snapshot.ready.len(), 1);
}

#[test]
fn custom_dash_is_owned_and_odd_patterns_have_an_even_period() {
    let mut source = vec![6.0, 2.0, 1.0];
    let dash = CustomDash::try_new(&source).unwrap();
    source[0] = 100.0;
    assert_eq!(dash.lengths(), &[6.0, 2.0, 1.0, 6.0, 2.0, 1.0]);
    assert_eq!(dash.period(), 18.0);
    let style = stroke(DashPattern::Custom(dash), -2.0, 4.0);
    assert_eq!(style.normalized_dash_offset(), 16.0);
}

#[test]
fn builtin_dash_scales_with_width_but_custom_dash_does_not() {
    let builtin = stroke(DashPattern::Dash, 0.0, 4.0);
    let custom = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        2.0,
        4.0,
    );
    assert_eq!(builtin.dash_lengths().as_ref(), &[6.0, 2.0]);
    assert_eq!(
        builtin.with_width(4.0).unwrap().dash_lengths().as_ref(),
        &[12.0, 4.0]
    );
    assert_eq!(
        custom.with_width(4.0).unwrap().dash_lengths().as_ref(),
        &[6.0, 2.0]
    );
    assert_eq!(custom.normalized_dash_offset(), 2.0);
}

#[test]
fn invalid_dash_and_stroke_values_are_rejected() {
    for lengths in [
        vec![],
        vec![0.0],
        vec![-1.0],
        vec![f64::NAN],
        vec![f64::INFINITY],
    ] {
        assert!(CustomDash::try_new(&lengths).is_err());
    }
    assert_eq!(
        CustomDash::try_new(&[f64::MAX, f64::MAX]),
        Err(GraphicsInputError::DashPeriodOverflow)
    );
    assert_eq!(
        CustomDash::try_new(&[f64::MAX]),
        Err(GraphicsInputError::DashPeriodOverflow)
    );
    for width in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(StrokeStyle::default().with_width(width).is_err());
    }
    for miter in [0.0, 0.5, f64::NAN, f64::INFINITY] {
        assert!(StrokeStyle::default().with_miter_limit(miter).is_err());
    }
    assert!(StrokeStyle::default().with_dash_offset(f64::NAN).is_err());
}

#[test]
fn stroke_outset_uses_actual_miter_and_cap() {
    let sharp = stroke(DashPattern::Solid, 0.0, 8.0);
    assert_eq!(sharp.visual_outset(), 8.0);
    assert_eq!(
        sharp.clone().with_join(LineJoin::Round).visual_outset(),
        1.0
    );
    assert_eq!(
        sharp
            .with_join(LineJoin::Bevel)
            .with_cap(LineCap::Square)
            .visual_outset(),
        std::f64::consts::SQRT_2
    );
    assert_eq!(
        StrokeStyle::default()
            .with_width(0.0)
            .unwrap()
            .visual_outset(),
        0.0
    );
}

#[test]
fn solid_offset_is_preserved_until_a_dash_pattern_is_selected() {
    let style = stroke(DashPattern::Solid, -2.0, 4.0);
    assert_eq!(style.dash_offset(), -2.0);
    assert_eq!(style.normalized_dash_offset(), 0.0);
    let dashed = style.with_dash_pattern(DashPattern::Custom(
        CustomDash::try_new(&[6.0, 2.0]).unwrap(),
    ));
    assert_eq!(dashed.normalized_dash_offset(), 6.0);
}

#[test]
fn canvas_snapshots_complete_strokes_and_restores_them() {
    use novadraw::render::command::RenderCommandKind;
    use novadraw::{Color, NdCanvas};
    let mut canvas = NdCanvas::new();
    let custom = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        -2.0,
        8.0,
    );
    canvas.set_foreground_color(Color::BLACK);
    canvas.set_stroke(custom.clone());
    canvas.push_state();
    canvas.set_stroke(StrokeStyle::default());
    canvas.restore_state();
    canvas.draw_rectangle(0.0, 0.0, 20.0, 20.0);
    canvas.pop_state();
    canvas.begin_path();
    canvas.move_to(0.0, 0.0);
    canvas.line_to(20.0, 20.0);
    canvas.stroke();
    let styles: Vec<_> = canvas
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::StrokeRect { stroke, .. }
            | RenderCommandKind::StrokePath { stroke, .. } => Some(stroke),
            _ => None,
        })
        .collect();
    assert_eq!(styles, vec![&custom, &custom]);
}

#[test]
fn rejected_canvas_stroke_setters_preserve_path_commands_and_damage() {
    use novadraw::render::command::RenderCommandKind;
    use novadraw::{Color, NdCanvas};
    let mut canvas = NdCanvas::new();
    canvas.set_foreground_color(Color::BLACK);
    let original = stroke(DashPattern::Dash, 2.0, 6.0);
    canvas.set_stroke(original.clone());
    canvas.begin_path();
    canvas.move_to(1.0, 2.0);
    canvas.line_to(3.0, 4.0);
    assert!(canvas.set_line_width(f64::NAN).is_err());
    assert!(canvas.set_miter_limit(0.5).is_err());
    assert!(canvas.set_dash_offset(f64::INFINITY).is_err());
    assert!(canvas.commands().is_empty());
    assert!(canvas.damage().is_empty());
    canvas.stroke();
    let RenderCommandKind::StrokePath { stroke, path, .. } = &canvas.commands()[0].kind else {
        panic!("expected preserved path");
    };
    assert_eq!(stroke, &original);
    assert_eq!(path.operations().len(), 2);
}

#[test]
fn explicit_recorders_share_one_style_without_reading_canvas_dash() {
    use novadraw::render::{BackendCapabilities, RenderCapability, RenderCommandKind};
    use novadraw::{Color, NdCanvas, Point};
    let mut canvas = NdCanvas::new();
    canvas.set_dash_pattern(DashPattern::Dot);
    let style = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        2.0,
        8.0,
    );
    canvas.stroke_rect_with_style(0.0, 0.0, 20.0, 20.0, Color::BLACK, style.clone());
    canvas.ellipse_with_style(
        10.0,
        10.0,
        10.0,
        5.0,
        None,
        Some(Color::BLACK),
        style.clone(),
    );
    canvas.line_with_style(
        Point::ORIGIN,
        Point::new(20.0, 0.0),
        Color::BLACK,
        style.clone(),
    );
    canvas.polyline_with_style(
        &[Point::ORIGIN, Point::new(20.0, 20.0)],
        Color::BLACK,
        style.clone(),
    );
    for command in canvas.commands() {
        let recorded = match &command.kind {
            RenderCommandKind::StrokeRect { stroke, .. }
            | RenderCommandKind::Ellipse { stroke, .. }
            | RenderCommandKind::Line { stroke, .. }
            | RenderCommandKind::Polyline { stroke, .. } => stroke,
            _ => panic!("unexpected command"),
        };
        assert_eq!(recorded, &style);
        assert_eq!(
            command.kind.required_capabilities().collect::<Vec<_>>(),
            vec![RenderCapability::CustomStrokes]
        );
    }
    let baseline = BackendCapabilities::FULL_FRAME_ONLY;
    assert_eq!(
        baseline
            .validate_capabilities(canvas.commands())
            .unwrap_err()
            .capability,
        RenderCapability::CustomStrokes
    );
    assert_eq!(
        baseline
            .with_custom_strokes()
            .validate_capabilities(canvas.commands()),
        Ok(())
    );
    canvas.clear_commands();
    canvas.stroke_rect_with_style(0.0, 0.0, 20.0, 20.0, Color::BLACK, StrokeStyle::default());
    assert_eq!(baseline.validate_capabilities(canvas.commands()), Ok(()));
}

#[test]
fn runtime_miter_growth_and_shrink_preserve_points_and_damage_both_envelopes() {
    use novadraw::{
        Point, PolylineFigure, Rectangle, RectangleFigure, Runtime, ShapeMutationError,
    };
    let points = vec![
        Point::new(40.0, 60.0),
        Point::new(160.0, 62.0),
        Point::new(80.0, 60.0),
    ];
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .unwrap();
    let line = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            PolylineFigure::from_points(points.clone()).with_stroke_style(stroke(
                DashPattern::Solid,
                0.0,
                2.0,
            )),
        ))
        .unwrap();
    runtime.prepare_frame().unwrap();
    let small = Rectangle::new(38.0, 58.0, 124.0, 6.0);
    let large = Rectangle::new(28.0, 48.0, 144.0, 26.0);
    assert_eq!(runtime.tree().figure_bounds(line), Some(small));

    for (miter, expected) in [(12.0, large), (2.0, small)] {
        assert!(
            runtime
                .point_list(line)
                .unwrap()
                .set_miter_limit(miter)
                .unwrap()
        );
        assert_eq!(runtime.tree().figure_bounds(line), Some(expected));
        assert_eq!(runtime.point_list_points(line).unwrap(), points);
        let canvas = runtime.prepare_frame().unwrap();
        assert_eq!(canvas.damage().union(), Some(large));
    }
    assert!(runtime.prepare_frame().is_none());
    let original = runtime.point_list(line).unwrap().stroke_style().unwrap();
    assert_eq!(
        runtime.point_list(line).unwrap().set_miter_limit(0.5),
        Err(ShapeMutationError::InvalidStroke(
            GraphicsInputError::InvalidMiterLimit
        ))
    );
    assert_eq!(
        runtime.point_list(line).unwrap().stroke_style().unwrap(),
        original
    );
    assert_eq!(runtime.tree().figure_bounds(line), Some(small));
    assert!(runtime.prepare_frame().is_none());
    assert!(
        !runtime
            .point_list(line)
            .unwrap()
            .set_stroke_style(original)
            .unwrap()
    );
    assert!(runtime.prepare_frame().is_none());
}

#[test]
fn runtime_dash_changes_repaint_without_moving_geometry() {
    use novadraw::render::RenderCommandKind;
    use novadraw::{Point, PolylineFigure, RectangleFigure, Runtime};
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 200.0)))
        .unwrap();
    let line = runtime
        .container(root)
        .unwrap()
        .add(Box::new(PolylineFigure::new(40.0, 60.0, 160.0, 62.0)))
        .unwrap();
    runtime.prepare_frame().unwrap();
    let bounds = runtime.tree().figure_bounds(line);
    let style = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        -2.0,
        4.0,
    );
    assert!(
        runtime
            .point_list(line)
            .unwrap()
            .set_stroke_style(style.clone())
            .unwrap()
    );
    assert_eq!(runtime.tree().figure_bounds(line), bounds);
    assert_eq!(
        runtime.point_list_points(line).unwrap(),
        vec![Point::new(40.0, 60.0), Point::new(160.0, 62.0)]
    );
    let canvas = runtime.prepare_frame().unwrap();
    assert_eq!(canvas.damage().union(), bounds);
    assert!(canvas.commands().iter().any(|command|
        matches!(&command.kind, RenderCommandKind::StrokePath { stroke, .. } if stroke == &style)));
}

#[test]
fn connection_and_decorations_use_actual_stroke_for_geometry_and_recording() {
    use novadraw::render::RenderCommandKind;
    use novadraw::{
        ConnectionFigure, ConnectionFigureBehavior, Figure, LocatorPlacement, NdCanvas, Point,
        PointList, PolygonDecorationFigure, PolylineDecorationFigure, Rectangle,
    };
    let style = stroke(
        DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0]).unwrap()),
        2.0,
        12.0,
    );
    let mut connection = ConnectionFigure::new().with_stroke_style(style.clone());
    let route = PointList::from_points(vec![Point::new(40.0, 60.0), Point::new(160.0, 60.0)]);
    let geometry = connection.prepare_route_geometry(&route).unwrap();
    assert_eq!(
        geometry.path_bounds(),
        Rectangle::new(28.0, 48.0, 144.0, 24.0)
    );
    connection.commit_route_points(geometry.local_points().clone());
    let mut canvas = NdCanvas::new();
    connection.paint_figure_in_bounds(&mut canvas, geometry.path_bounds());
    assert!(matches!(&canvas.commands()[0].kind,
        RenderCommandKind::Polyline { stroke, .. } if stroke == &style));

    let template = PointList::from_points(vec![
        Point::new(-10.0, -5.0),
        Point::ZERO,
        Point::new(-10.0, 5.0),
    ]);
    let mut closed = PolygonDecorationFigure::from_template(template.clone())
        .unwrap()
        .with_stroke_style(style.clone());
    let mut open = PolylineDecorationFigure::from_template(template)
        .unwrap()
        .with_stroke_style(style.clone());
    let placement = LocatorPlacement {
        point: Point::new(100.0, 100.0),
        reference: Point::new(90.0, 100.0),
    };
    for decoration in [&mut closed as &mut dyn Figure, &mut open as &mut dyn Figure] {
        let geometry = decoration
            .connection_decoration()
            .unwrap()
            .prepare_decoration_geometry(placement)
            .unwrap();
        assert_eq!(geometry.bounds(), Rectangle::new(78.0, 83.0, 34.0, 34.0));
        let bounds = geometry.bounds();
        decoration
            .connection_decoration_mut()
            .unwrap()
            .commit_decoration_geometry(geometry);
        canvas.clear_commands();
        canvas.set_foreground_color(novadraw::Color::BLACK);
        canvas.set_background_color(novadraw::Color::WHITE);
        decoration.paint_figure_in_bounds(&mut canvas, bounds);
        assert!(canvas.commands().iter().any(|command|
            matches!(&command.kind, RenderCommandKind::StrokePath { stroke, .. } if stroke == &style)));
    }
}

#[test]
fn polygon_and_scalable_polygon_record_complete_strokes() {
    use novadraw::render::RenderCommandKind;
    use novadraw::{
        Figure, NdCanvas, Point, PointList, PolygonFigure, Rectangle, ScalablePolygonFigure,
    };
    let style = stroke(DashPattern::Dash, 2.0, 12.0);
    let points = vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(50.0, 100.0),
    ];
    let polygon = PolygonFigure::from_points(points.clone()).with_stroke_style(style.clone());
    assert_eq!(
        polygon.initial_bounds(),
        Rectangle::new(-12.0, -12.0, 124.0, 124.0)
    );
    let scalable = ScalablePolygonFigure::new(
        Rectangle::new(0.0, 0.0, 100.0, 100.0),
        PointList::from_points(points),
    )
    .unwrap()
    .with_stroke_style(style.clone());
    assert_eq!(
        scalable.scaled_points(scalable.initial_bounds()).bounds(),
        Some(Rectangle::new(12.0, 12.0, 76.0, 76.0))
    );
    for figure in [&polygon as &dyn Figure, &scalable as &dyn Figure] {
        let mut canvas = NdCanvas::new();
        canvas.set_foreground_color(novadraw::Color::BLACK);
        canvas.set_background_color(novadraw::Color::WHITE);
        figure.paint_figure_in_bounds(&mut canvas, figure.initial_bounds());
        assert!(canvas.commands().iter().any(|command|
            matches!(&command.kind, RenderCommandKind::StrokePath { stroke, .. } if stroke == &style)));
    }
}

#[test]
fn miter_transaction_updates_freeform_extent_with_one_move_notification() {
    use novadraw::{
        FigureEvent, FigureListener, FreeformLayerFigure, ListenerDirective, PolylineFigure,
        Rectangle, Runtime,
    };
    use std::sync::{Arc, Mutex};
    struct Moves(Arc<Mutex<Vec<FigureEvent>>>);
    impl FigureListener for Moves {
        fn figure_moved(&self, event: FigureEvent) -> ListenerDirective {
            self.0.lock().unwrap().push(event);
            ListenerDirective::Keep
        }
    }
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(FreeformLayerFigure::new(0.0, 0.0, 300.0, 200.0)))
        .unwrap();
    let line = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            PolylineFigure::new(40.0, 60.0, 160.0, 62.0).with_stroke_style(stroke(
                DashPattern::Solid,
                0.0,
                2.0,
            )),
        ))
        .unwrap();
    runtime.prepare_frame().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime.add_figure_listener(Box::new(Moves(Arc::clone(&events))));
    for miter in [12.0, 2.0] {
        let before = runtime.tree().figure_bounds(line).unwrap();
        runtime
            .point_list(line)
            .unwrap()
            .set_miter_limit(miter)
            .unwrap();
        runtime.prepare_frame().unwrap();
        let after = Rectangle::new(
            40.0 - miter,
            60.0 - miter,
            120.0 + miter * 2.0,
            2.0 + miter * 2.0,
        );
        assert_eq!(runtime.freeform_extent(root), Ok(after));
        assert_eq!(
            *events.lock().unwrap(),
            vec![FigureEvent::FigureMoved {
                figure_id: line,
                old_bounds: before,
                new_bounds: after,
            }]
        );
        events.lock().unwrap().clear();
    }
}
