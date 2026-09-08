use novadraw_core::Color;
use novadraw_render::{
    BuiltinFont, FontData, FontDescriptor, FrameId, GlyphPaint, NdCanvas, RenderBackend,
    RenderCommandKind, RenderOutcome, RenderSubmission, ResourceDelta, ResourceId, ResourcePayload,
    ResourceUpdate, SurfaceInfo, TextConstraints, TextEngine,
};
use std::sync::Arc;
use uuid::Uuid;

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
    let font_id = ResourceId::new(Uuid::nil(), 1);
    engine
        .register_font(font_id, 1, BuiltinFont::Inter.bytes())
        .unwrap();
    let layout = engine
        .layout(
            "backend neutral",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    assert_eq!(layout.visible_range(), 0.."backend neutral".len());
    assert!(!layout.is_truncated());
    assert_eq!(layout.key().text(), "backend neutral");
    assert_eq!(layout.key().engine_revision(), engine.revision());
    assert!(layout.baseline() > 0.0);
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

    let submission = canvas.to_submission_for_frame(
        SurfaceInfo {
            logical_width: 200.0,
            logical_height: 80.0,
            pixel_width: 200,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        ResourceDelta {
            added: vec![ResourceUpdate {
                id: font_id,
                revision: 1,
                payload: ResourcePayload::Font(Arc::new(FontData::new(
                    BuiltinFont::Inter.bytes().to_vec(),
                ))),
            }],
            removed: Vec::new(),
        },
        FrameId::INITIAL,
    );

    let mut backend = RecordingBackend::default();
    let outcome = backend.submit(&submission);

    assert_eq!(outcome, RenderOutcome::Presented);
    assert_eq!(backend.glyph_runs, layout.glyph_runs().len());
    assert_eq!(submission.resources.added.len(), 1);
}
