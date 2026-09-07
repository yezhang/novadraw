use novadraw_core::Color;
use novadraw_render::{
    FontDescriptor, GlyphPaint, NdCanvas, RenderBackend, RenderCommandKind, RenderOutcome,
    RenderSubmission, SurfaceInfo, TextConstraints, TextEngine,
};

#[derive(Default)]
struct RecordingBackend {
    glyph_runs: usize,
}

impl RenderBackend for RecordingBackend {
    fn submit(&mut self, submission: &RenderSubmission) -> RenderOutcome {
        self.glyph_runs += submission
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count();
        RenderOutcome::Presented
    }

    fn resize(&mut self, _pixel_width: u32, _pixel_height: u32, _scale_factor: f64) {}
}

#[test]
fn backend_neutral_layout_can_be_consumed_without_vello_types() {
    let mut engine = TextEngine::new();
    let layout = engine
        .layout(
            "backend neutral",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let mut canvas = NdCanvas::new();
    canvas.set_foreground_color(Color::BLACK);
    canvas.draw_text_layout(&layout, 12.0, 24.0);

    let command = canvas
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::DrawGlyphRun { run, paint, .. } => Some((run, paint)),
            _ => None,
        })
        .expect("text layout should lower to DrawGlyphRun");
    assert!(!command.0.glyphs.is_empty());
    assert!(matches!(command.1, GlyphPaint::Fill(color) if *color == Color::BLACK));

    let mut backend = RecordingBackend::default();
    let outcome = backend.submit(&canvas.to_submission_for_surface(SurfaceInfo {
        logical_width: 200.0,
        logical_height: 80.0,
        pixel_width: 200,
        pixel_height: 80,
        scale_factor: 1.0,
    }));

    assert_eq!(outcome, RenderOutcome::Presented);
    assert_eq!(backend.glyph_runs, layout.glyph_runs().len());
}
