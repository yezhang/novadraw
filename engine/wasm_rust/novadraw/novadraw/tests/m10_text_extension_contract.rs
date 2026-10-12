use novadraw::graphics::Color;
use novadraw::render::{
    BackendSessionId, BuiltinFont, CommandRecorder, DamageSet, FontData, FontDescriptor,
    FontFaceRef, FrameId, GlyphPaint, GlyphRun, PositionedGlyph, RenderBackend, RenderCommandKind,
    RenderDisposition, RenderResult, RenderSubmission, ResourceDelta, ResourceId, ResourceOp,
    ResourcePayload, ResourceSync, ResourceUpdate, SurfaceInfo, TextConstraints, TextEngine,
    TextError, TextLayout, TextLayoutEngine, TextLayoutParts, TextLineMetrics,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
struct RecordingBackend {
    glyph_runs: usize,
}

#[derive(Default)]
struct ExternalTextEngine {
    face: Option<FontFaceRef>,
    revision: u64,
}

impl TextLayoutEngine for ExternalTextEngine {
    fn revision(&self) -> u64 {
        self.revision
    }

    fn register_font(
        &mut self,
        resource_id: ResourceId,
        revision: u64,
        _bytes: &[u8],
    ) -> Result<(), TextError> {
        self.face = Some(FontFaceRef::new(resource_id, revision, 0));
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    fn remove_font(&mut self, resource_id: ResourceId) {
        if self
            .face
            .as_ref()
            .is_some_and(|face| face.resource_id() == resource_id)
        {
            self.face = None;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        let face = self.face.clone().ok_or(TextError::NoUsableFont)?;
        TextLayout::from_parts(TextLayoutParts {
            text: text.to_owned(),
            font: font.clone(),
            constraints,
            engine_revision: self.revision,
            width: 24.0,
            full_width: 24.0,
            height: 16.0,
            lines: vec![TextLineMetrics {
                ascent: 11.0,
                descent: 3.0,
                leading: 2.0,
                baseline: 11.0,
                advance: 24.0,
            }],
            glyph_runs: vec![GlyphRun {
                font: face,
                font_size: font.size(),
                normalized_coords: Vec::new(),
                skew_degrees: None,
                glyphs: vec![PositionedGlyph {
                    id: 1,
                    x: 0.0,
                    y: 11.0,
                }],
            }],
            visible_range: 0..text.len(),
            truncated: false,
        })
    }
}

impl RenderBackend for RecordingBackend {
    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        self.glyph_runs += submission
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count();
        Ok(RenderDisposition::Presented)
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
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::BLACK);
        context
            .fill_text(&layout, novadraw::geometry::Point::new(12.0, 24.0))
            .unwrap();
    }

    let command = recorder
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::DrawGlyphRun { run, paint, .. } => Some((run, paint)),
            _ => None,
        })
        .expect("text layout should lower to DrawGlyphRun");
    assert!(!command.0.glyphs.is_empty());
    assert!(
        matches!(command.1, GlyphPaint::Fill(novadraw::graphics::Paint::Solid(color)) if *color == Color::BLACK)
    );

    let drawing = recorder.finish().unwrap();
    let mut damage = DamageSet::default();
    damage.set_full();
    let submission = RenderSubmission {
        commands: drawing.commands().to_vec(),
        damage,
        surface: SurfaceInfo {
            logical_width: 200.0,
            logical_height: 80.0,
            pixel_width: 200,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        resources: ResourceSync::Delta(ResourceDelta {
            ops: vec![ResourceOp::Upsert(ResourceUpdate {
                id: font_id,
                revision: 1,
                payload: ResourcePayload::Font(Arc::new(FontData::new(
                    BuiltinFont::Inter.bytes().to_vec(),
                ))),
            })],
        }),
        session_id: BackendSessionId::default(),
        frame_id: FrameId::INITIAL,
    };

    let mut backend = RecordingBackend::default();
    let outcome = backend.submit(&submission);

    assert_eq!(outcome, Ok(RenderDisposition::Presented));
    assert_eq!(backend.glyph_runs, layout.glyph_runs().len());
    assert!(matches!(
        submission.resources,
        ResourceSync::Delta(ResourceDelta { ref ops })
            if matches!(ops.as_slice(), [ResourceOp::Upsert(_)])
    ));
}

#[test]
fn external_text_engine_constructs_non_empty_backend_neutral_layout() {
    let font_id = ResourceId::new(Uuid::nil(), 7);
    let mut engine = ExternalTextEngine::default();
    engine.register_font(font_id, 3, b"external-font").unwrap();
    let layout = engine
        .layout(
            "external",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();

    assert!(!layout.is_empty());
    assert_eq!(layout.key().text(), "external");
    assert_eq!(layout.key().engine_revision(), engine.revision());
    assert_eq!(layout.glyph_runs()[0].font.resource_id(), font_id);

    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context.set_fill_paint(Color::BLACK);
        context
            .fill_text(&layout, novadraw::geometry::Point::new(4.0, 12.0))
            .unwrap();
    }
    assert!(
        recorder
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
}
