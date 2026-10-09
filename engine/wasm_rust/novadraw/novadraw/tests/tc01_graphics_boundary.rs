use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use novadraw::{
    Color, Figure, FramePreparation, FramePreparationError, GraphicsError, PaintContext, Rectangle,
    Runtime,
    render::{BackendCapabilities, CommandRecorder, FrameId, RenderCommandKind, SurfaceInfo},
};

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 120.0,
        logical_height: 80.0,
        pixel_width: 120,
        pixel_height: 80,
        scale_factor: 1.0,
    }
}

fn paint_panel(context: &mut PaintContext<'_>, bounds: Rectangle) -> Result<(), GraphicsError> {
    context.set_fill_paint(Color::rgba(0.2, 0.4, 0.8, 1.0));
    context.fill_rect(bounds)
}

struct ExternalPaintFigure {
    fail: Arc<AtomicBool>,
}

impl Figure for ExternalPaintFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 120.0, 80.0)
    }

    fn name(&self) -> &'static str {
        "ExternalPaintFigure"
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        if self.fail.load(Ordering::Relaxed) {
            context.push_state();
            return context.set_opacity(f64::NAN);
        }
        paint_panel(context, bounds)
    }
}

#[test]
fn command_recorder_owns_standalone_paint_without_exposing_internal_canvas() {
    let mut recorder = CommandRecorder::new();
    paint_panel(&mut recorder.paint(), Rectangle::new(0.0, 0.0, 20.0, 10.0)).unwrap();

    let drawing = recorder.finish().unwrap();
    assert!(matches!(
        drawing.commands()[0].kind,
        RenderCommandKind::FillRect { .. }
    ));
}

#[test]
fn figure_paint_failure_blocks_publication_and_preserves_retry_identity() {
    let fail = Arc::new(AtomicBool::new(true));
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(ExternalPaintFigure { fail: fail.clone() }))
        .unwrap();

    assert!(matches!(
        runtime.prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(FramePreparationError::Graphics(
            GraphicsError::InvalidOpacity
        ))
    ));

    fail.store(false, Ordering::Relaxed);
    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("corrected Figure paint must remain retriable");
    assert_eq!(submission.frame_id, FrameId::INITIAL);
    assert!(
        submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
}
