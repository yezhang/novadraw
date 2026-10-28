//! A deliberately small ASCII engine proves flow consumes only public shaping/interaction APIs.
mod support;
use novadraw::{
    figure::{FigureStyle, flow::*},
    geometry::{Point, Rectangle},
    render::ResourceId,
    runtime::Runtime,
    text::{
        BuiltinFont, CaretGeometry, FontDescriptor, FontFaceRef, SelectionQuad, StyledGlyphRun,
        StyledTextLayout, StyledTextRequest, TextAffinity, TextConstraints, TextError,
        TextInteractionError, TextLayout, TextLineMetrics, TextMovement, TextPosition, TextRange,
        shaping::{
            GlyphRun, PositionedGlyph, TextInteractionMap, TextInteractionProvider,
            TextLayoutEngine, TextLayoutParts,
        },
    },
    tree::FigureTree,
};
use std::sync::Arc;

const ADVANCE: f64 = 8.0;
const HEIGHT: f64 = 20.0;
const BASELINE: f32 = 16.0;

struct AsciiInteraction(usize);
impl TextInteractionProvider for AsciiInteraction {
    fn text_len(&self) -> usize {
        self.0
    }
    fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        Ok(TextPosition::new(
            (point.x() / ADVANCE).round().clamp(0.0, self.0 as f64) as usize,
            TextAffinity::Downstream,
        ))
    }
    fn caret_geometry(&self, p: TextPosition) -> Result<CaretGeometry, TextInteractionError> {
        Ok(CaretGeometry::new(
            Rectangle::new(p.byte_offset() as f64 * ADVANCE, 0.0, 1.0, HEIGHT),
            0,
        ))
    }
    fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        let a = range
            .anchor()
            .byte_offset()
            .min(range.focus().byte_offset());
        let b = range
            .anchor()
            .byte_offset()
            .max(range.focus().byte_offset());
        Ok(if a == b {
            vec![]
        } else {
            vec![SelectionQuad::new(
                Rectangle::new(a as f64 * ADVANCE, 0.0, (b - a) as f64 * ADVANCE, HEIGHT),
                0,
            )]
        })
    }
    fn move_position(
        &self,
        p: TextPosition,
        m: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        let offset = match m {
            TextMovement::NextVisual | TextMovement::NextWord => (p.byte_offset() + 1).min(self.0),
            TextMovement::PreviousVisual | TextMovement::PreviousWord => {
                p.byte_offset().saturating_sub(1)
            }
            TextMovement::LineStart => 0,
            TextMovement::LineEnd => self.0,
            _ => p.byte_offset(),
        };
        Ok(TextPosition::new(offset, p.affinity()))
    }
}

#[derive(Default)]
struct AsciiEngine {
    font: Option<FontFaceRef>,
    revision: u64,
}
impl TextLayoutEngine for AsciiEngine {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn register_font(&mut self, id: ResourceId, revision: u64, _: &[u8]) -> Result<(), TextError> {
        self.font = Some(FontFaceRef::new(id, revision, 0));
        self.revision += 1;
        Ok(())
    }
    fn remove_font(&mut self, _: ResourceId) {
        self.font = None;
        self.revision += 1;
    }
    fn layout(
        &mut self,
        _: &str,
        _: &FontDescriptor,
        _: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        Err(TextError::StyledLayoutUnsupported)
    }
    fn layout_styled(
        &mut self,
        request: &StyledTextRequest,
    ) -> Result<StyledTextLayout, TextError> {
        if !request.text().is_ascii() || !request.inline_boxes().is_empty() {
            return Err(TextError::StyledLayoutUnsupported);
        }
        let width = request.text().len() as f32 * ADVANCE as f32;
        let mut glyph_runs = Vec::new();
        let mut runs = Vec::new();
        // Flow supplies every source as an attributed span. The independent engine
        // retains those ranges rather than calling Parley or approximating a source map.
        for (index, span) in request.spans().iter().enumerate() {
            glyph_runs.push(GlyphRun {
                font: self.font.clone().ok_or(TextError::NoUsableFont)?,
                font_size: span.style.font().size(),
                normalized_coords: Arc::from([]),
                skew_degrees: None,
                glyphs: novadraw::text::shaping::GlyphRange::new(
                    span.range
                        .clone()
                        .map(|offset| PositionedGlyph {
                            id: 1,
                            x: offset as f32 * ADVANCE as f32,
                            y: BASELINE,
                        })
                        .collect(),
                )?,
            });
            runs.push(StyledGlyphRun {
                source_range: span.range.clone(),
                style_index: index + 1,
                line_index: 0,
                baseline: BASELINE,
                bounds: Rectangle::new(
                    span.range.start as f64 * ADVANCE,
                    0.0,
                    span.range.len() as f64 * ADVANCE,
                    HEIGHT,
                ),
                right_to_left: false,
            });
        }
        let layout = TextLayout::from_parts(TextLayoutParts {
            text: request.text().into(),
            font: request.default_style().font().clone(),
            constraints: request.constraints(),
            engine_revision: self.revision(),
            width,
            full_width: width,
            height: HEIGHT as f32,
            lines: vec![TextLineMetrics {
                ascent: BASELINE,
                descent: HEIGHT as f32 - BASELINE,
                leading: 0.0,
                baseline: BASELINE,
                advance: width,
            }],
            glyph_runs,
            visible_range: 0..request.text().len(),
            truncated: false,
        })?
        .with_interaction_map(
            TextInteractionMap::new(
                request.text(),
                Arc::new(AsciiInteraction(request.text().len())),
            )
            .map_err(|_| TextError::InvalidInteractionMap)?,
        )?;
        StyledTextLayout::new(request.clone(), layout, runs, vec![])
    }

    fn layout_styled_with_diagnostics(
        &mut self,
        request: &StyledTextRequest,
        diagnostics: &novadraw::text::TextLayoutDiagnostics,
    ) -> Result<StyledTextLayout, TextError> {
        let _scope = diagnostics.time("external-ascii-layout");
        self.layout_styled(request)
    }
}

#[test]
fn independent_engine_drives_flow_mapping_paint_hit_caret_and_selection() {
    let mut tree = FigureTree::new();
    let page = tree
        .builder()
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, 200.0, 100.0)).with_style(FigureStyle {
                font: Some("16px Inter".into()),
                ..FigureStyle::default()
            }),
        ))
        .unwrap();
    let a = tree
        .builder()
        .add_child(page, Box::new(StructuredTextFigure::new("first ")))
        .unwrap();
    let b = tree
        .builder()
        .add_child(page, Box::new(StructuredTextFigure::new("second")))
        .unwrap();
    let mut runtime = Runtime::with_text_layout_engine(tree, Box::<AsciiEngine>::default());
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    runtime.frames().set_timing_enabled(true);
    let _ = support::record_full_submission(&mut runtime);
    let s = runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .data::<FlowSnapshot>()
        .unwrap();
    assert_eq!(s.paragraphs[0].layout.request().text(), "first second");
    assert_eq!(s.hit_text(Point::new(64.0, 5.0)).unwrap().source, b);
    let position = |source, offset| SourceTextPosition {
        source,
        byte_offset: offset,
        affinity: TextAffinity::Downstream,
        revision: s.revision,
    };
    assert_eq!(s.caret(position(b, 2)).unwrap().bounds().x, 64.0);
    let moved = s
        .move_position(position(a, 6), TextMovement::NextVisual)
        .unwrap();
    assert_eq!(moved.source, b);
    assert_eq!(moved.byte_offset, 1);
    assert_eq!(
        s.selection(position(a, 1), position(b, 3)).unwrap()[0]
            .bounds()
            .width,
        64.0
    );
    assert_eq!(s.paragraphs[0].layout.layout().glyph_runs().len(), 2);
    let old_request = s.paragraphs[0].layout.request().clone();
    let observations = runtime.frames().take_flow_layout_observations();
    assert_eq!(observations.dropped, 0);
    assert!(!observations.records.is_empty());
    assert!(observations.records.iter().all(|record| {
        *record.request == old_request
            && record.engine_identity.is_none()
            && !record.cache_hit
            && record.succeeded
            && record.sources.contains(&(b, 0))
    }));
    assert!(
        runtime
            .frames()
            .take_flow_layout_observations()
            .records
            .is_empty()
    );
    assert!(
        runtime
            .frames()
            .take_timings()
            .iter()
            .any(|entry| { entry.stage == "external-ascii-layout" && entry.calls > 0 })
    );
    let receipt = runtime
        .figure(b)
        .unwrap()
        .update_component(SetStructuredFlowText("中文".into()))
        .unwrap();
    assert!(runtime.frames().stabilize_for_query().is_err());
    let failed = runtime.frames().take_flow_layout_observations();
    assert_eq!(failed.dropped, 0);
    assert!(failed.records.iter().any(|record| {
        !record.succeeded
            && !record.cache_hit
            && record.request.text() == "first 中文"
            && record.sources.contains(&(b, receipt.revision))
            && record.elapsed_ns > 0
    }));
    assert!(
        runtime
            .frames()
            .take_timings()
            .iter()
            .any(|entry| { entry.stage == "external-ascii-layout" && entry.calls > 0 })
    );
    runtime.frames().set_timing_enabled(false);
    assert!(runtime.frames().stabilize_for_query().is_err());
    assert!(
        runtime
            .frames()
            .take_flow_layout_observations()
            .records
            .is_empty()
    );
}
