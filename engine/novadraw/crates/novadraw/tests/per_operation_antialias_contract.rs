use novadraw::{
    figure::{Figure, RectangleFigure},
    geometry::{Point, Rectangle},
    graphics::{AntialiasMode, Color, Graphics, GraphicsError, PaintContext},
    render::{
        BackendCapabilities, CommandRecorder, RenderBackend, RenderCapability, RenderCommandKind,
        RenderFailure, RenderResult, RenderSubmission, SurfaceInfo,
    },
    runtime::{FramePreparation, FramePreparationError, Runtime, SubmissionAcknowledgement},
    scene::{
        ASPECT_FIT_VIEW, AspectFitView, SOURCE_LOCAL_CAPTURE, SceneBindingGroup, SceneViewFigure,
        SourceLocalCapture,
    },
    text::{BuiltinFont, TextConstraints, TextSystem},
};

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 100,
        pixel_height: 100,
        scale_factor: 1.0,
    }
}

#[test]
fn independent_hints_restore_and_commands_freeze_their_original_values() {
    let mut recorder = CommandRecorder::new();
    let mut paint = recorder.paint();
    paint.set_fill_paint(Color::BLACK);
    let rect = Rectangle::new(0.0, 0.0, 5.0, 5.0);
    paint.fill_rect(rect).unwrap();
    paint.set_antialias(AntialiasMode::Off);
    paint.set_text_antialias(AntialiasMode::On);
    paint.fill_rect(rect).unwrap();
    paint.push_state();
    paint.set_antialias(AntialiasMode::On);
    paint.set_text_antialias(AntialiasMode::Off);
    paint.fill_rect(rect).unwrap();
    paint.restore_state().unwrap();
    assert_eq!(paint.antialias(), AntialiasMode::Off);
    assert_eq!(paint.text_antialias(), AntialiasMode::On);
    paint.fill_rect(rect).unwrap();
    paint.set_antialias(AntialiasMode::On);
    paint.pop_state().unwrap();
    assert_eq!(paint.antialias(), AntialiasMode::Off);
    paint.set_antialias(AntialiasMode::Default);
    paint.fill_rect(rect).unwrap();
    drop(paint);
    let drawing = recorder.finish().unwrap();
    assert_eq!(
        drawing
            .commands()
            .iter()
            .filter_map(|c| c.kind.antialias())
            .collect::<Vec<_>>(),
        [
            AntialiasMode::Default,
            AntialiasMode::Off,
            AntialiasMode::On,
            AntialiasMode::Off,
            AntialiasMode::Default
        ]
    );
}

#[test]
fn text_hint_is_independent_for_glyph_runs_and_prepared_outlines() {
    use novadraw::{
        render::ParleyTextEngine,
        text::outline::{OutlineTextEngine, SkrifaOutlineProvider},
    };
    for outlined in [false, true] {
        let mut text = if outlined {
            TextSystem::with_engine(Box::new(OutlineTextEngine::new(
                Box::new(ParleyTextEngine::new()),
                Box::new(SkrifaOutlineProvider::default()),
            )))
        } else {
            TextSystem::new()
        };
        text.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut recorder = CommandRecorder::new();
        let mut graphics = Graphics::new(&mut text, &mut recorder);
        let layout = graphics
            .layout_text("AA", TextConstraints::UNBOUNDED)
            .unwrap();
        graphics.set_fill_paint(Color::BLACK);
        graphics.set_stroke_paint(Color::BLACK);
        graphics.set_antialias(AntialiasMode::Off);
        graphics.fill_text(&layout, Point::ZERO).unwrap();
        graphics.set_text_antialias(AntialiasMode::Off);
        graphics.stroke_text(&layout, Point::ZERO).unwrap();
        drop(graphics);
        let drawing = recorder.finish().unwrap();
        let hints: Vec<_> = drawing
            .commands()
            .iter()
            .filter_map(|c| c.kind.antialias())
            .collect();
        assert!(hints.contains(&AntialiasMode::Default));
        assert!(hints.contains(&AntialiasMode::Off));
        assert!(!hints.contains(&AntialiasMode::On));
        assert_eq!(
            drawing
                .commands()
                .iter()
                .any(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. })),
            !outlined
        );
    }
}

struct ExternalFigure;
impl Figure for ExternalFigure {
    fn name(&self) -> &'static str {
        "ExternalAntialiasFigure"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 100.0, 100.0)
    }
    fn paint(&self, paint: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        paint.set_antialias(AntialiasMode::Off);
        paint.set_fill_paint(Color::BLACK);
        paint.fill_ellipse(Rectangle::new(5.25, 6.75, 40.0, 30.0))
    }
}

struct ExternalBackend;
impl RenderBackend for ExternalBackend {
    fn resize(&mut self, _: u32, _: u32, _: f64) {}
    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        self.capabilities()
            .validate_capabilities(&submission.commands)
            .map_err(RenderFailure::Unsupported)?;
        unreachable!("OFF must be rejected before this external backend accepts a frame")
    }
}

#[test]
fn runtime_and_external_backend_reject_the_same_hint_without_consuming_retry() {
    let mut runtime = Runtime::empty();
    runtime.set_contents(Box::new(ExternalFigure)).unwrap();
    assert!(matches!(
        runtime.frames().prepare_submission(surface(), BackendCapabilities::FULL_FRAME_ONLY),
        FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(error))
            if error.capability == RenderCapability::PerOperationAntialias
    ));
    let frame = runtime
        .frames()
        .prepare_submission(
            surface(),
            BackendCapabilities::FULL_FRAME_ONLY.with_per_operation_antialias(),
        )
        .into_ready()
        .unwrap();
    let result = ExternalBackend.submit(&frame);
    assert!(matches!(result, Err(RenderFailure::Unsupported(error))
        if error.capability == RenderCapability::PerOperationAntialias));
    assert!(matches!(
        runtime
            .frames()
            .complete_submission(frame.session_id, frame.frame_id, result,)
            .unwrap(),
        SubmissionAcknowledgement::Rejected(_)
    ));
}

#[test]
fn scene_capture_and_destination_replay_preserve_aa_admission() {
    let group = SceneBindingGroup::default();
    let mut source = Runtime::empty();
    let root = source.set_contents(Box::new(ExternalFigure)).unwrap();
    source.scenes().join_group(group.clone()).unwrap();
    let source_id = source.scenes().register_source(root).unwrap();
    let (subscription, _, _) = source
        .scenes()
        .subscribe(
            source_id,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .unwrap();
    let snapshot = source.scenes().capture(subscription).unwrap();
    assert!(
        snapshot
            .drawing()
            .commands()
            .iter()
            .any(|c| c.kind.antialias() == Some(AntialiasMode::Off))
    );
    let mut destination = Runtime::empty();
    destination.scenes().join_group(group).unwrap();
    let handle = destination
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let root = destination
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let view = destination
        .figure(root)
        .unwrap()
        .add(Box::new(
            SceneViewFigure::new(handle)
                .with_bounds(Rectangle::new(0.0, 0.0, 100.0, 100.0))
                .expect("valid detached figure input"),
        ))
        .unwrap();
    let binding = destination
        .scenes()
        .bind_view(
            view,
            snapshot.source(),
            snapshot.subscription(),
            snapshot.config_revision(),
        )
        .unwrap();
    destination
        .scenes()
        .install_view(binding, snapshot)
        .unwrap();
    assert!(matches!(
        destination
            .frames()
            .prepare_submission(surface(), BackendCapabilities::FULL_FRAME_ONLY),
        FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(_))
    ));
    let frame = destination
        .frames()
        .prepare_submission(
            surface(),
            BackendCapabilities::FULL_FRAME_ONLY.with_per_operation_antialias(),
        )
        .into_ready()
        .unwrap();
    assert!(
        frame
            .commands
            .iter()
            .any(|c| c.kind.antialias() == Some(AntialiasMode::Off))
    );
}
