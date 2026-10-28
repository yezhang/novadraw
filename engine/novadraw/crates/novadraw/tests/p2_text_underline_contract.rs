use novadraw::{
    figure::LayoutDirection,
    geometry::Point,
    graphics::{Color, Graphics},
    render::{CommandRecorder, FontData, RenderCommandKind, ResourceId},
    text::{
        BuiltinFont, FontDescriptor, FontMetrics, ParagraphAlignment, ParagraphDirection,
        StyledTextLayout, StyledTextRequest, TextConstraints, TextError, TextLayout, TextStyle,
        TextStyleSpan, TextSystem, TextUnderline,
        shaping::{GlyphRun, ParleyTextEngine, TextLayoutEngine},
    },
};

fn system() -> TextSystem {
    let mut system = TextSystem::new();
    for font in BuiltinFont::ALL {
        system.register_builtin_font(font).unwrap();
    }
    system
}
fn request(width: f32, mirrored: bool) -> StyledTextRequest {
    StyledTextRequest::new(
        "office e\u{301} 中文 العربية\nlarge text after newline",
        TextStyle::new(
            FontDescriptor::new(BuiltinFont::Inter.family(), 16.0).unwrap(),
            Color::BLACK,
        ),
        vec![TextStyleSpan {
            range: 0..6,
            style: TextStyle::new(
                FontDescriptor::new(BuiltinFont::JetBrainsMono.family(), 28.0).unwrap(),
                Color::BLUE,
            ),
        }],
        vec![],
        TextConstraints::new(Some(width)).unwrap(),
    )
    .unwrap()
    .with_paragraph(
        ParagraphDirection::RightToLeft,
        ParagraphAlignment::Leading,
        true,
    )
    .with_layout_direction(if mirrored {
        LayoutDirection::RightToLeft
    } else {
        LayoutDirection::LeftToRight
    })
}

#[test]
fn fragments_follow_actual_faces_sizes_lines_and_bidi_geometry() {
    let mut system = system();
    for mirrored in [false, true] {
        for width in [90.0, 300.0] {
            let input = request(width, mirrored);
            let mut measure = system.measure(input.default_style().font().clone());
            let layout = measure.layout_styled(&input).unwrap();
            let underline = measure
                .prepare_underlines(
                    &layout,
                    &[TextUnderline {
                        range: 0..input.text().len(),
                        color: Color::BLUE,
                    }],
                )
                .unwrap();
            assert_eq!(underline.layout_revision(), layout.layout().revision());
            assert!(underline.fragments().len() <= layout.runs().len());
            assert!(underline.fragments().iter().any(|f| f.line_index() > 0));
            let faces = underline
                .fragments()
                .iter()
                .map(|f| f.font_metrics().face().resource_id())
                .collect::<std::collections::HashSet<_>>();
            assert!(faces.len() >= 3, "{faces:?}");
            for fragment in underline.fragments() {
                let index = layout
                    .runs()
                    .iter()
                    .position(|run| {
                        run.line_index == fragment.line_index()
                            && run.source_range.start <= fragment.source_range().start
                            && run.source_range.end >= fragment.source_range().end
                    })
                    .unwrap();
                let run = &layout.layout().glyph_runs()[index];
                let metrics = fragment.font_metrics();
                assert_eq!(metrics.face(), &run.font);
                assert_eq!(metrics.size(), run.font_size);
                assert_eq!(metrics.normalized_coords(), run.normalized_coords.as_ref());
                assert_eq!(
                    fragment.bounds().y,
                    f64::from(layout.runs()[index].baseline)
                        + f64::from(metrics.underline().unwrap().offset())
                );
                assert_eq!(
                    fragment.bounds().height,
                    f64::from(metrics.underline().unwrap().thickness())
                );
                assert!(fragment.bounds().width > 0.0);
                assert!(fragment.bounds().x >= layout.runs()[index].bounds.x);
                assert!(
                    fragment.bounds().x + fragment.bounds().width
                        <= layout.runs()[index].bounds.x + layout.runs()[index].bounds.width
                );
            }
            let mut recorder = CommandRecorder::new();
            let mut graphics = Graphics::new(&mut system, &mut recorder);
            graphics.set_fill_paint(Color::RED);
            graphics
                .fill_underlines(&underline, Point::new(10.0, 20.0))
                .unwrap();
            graphics
                .fill_rect(novadraw::geometry::Rectangle::new(0.0, 0.0, 1.0, 1.0))
                .unwrap();
            drop(graphics);
            assert!(recorder.commands().iter().any(|command| matches!(&command.kind,
                RenderCommandKind::FillRect { paint: novadraw::graphics::Paint::Solid(color), .. } if *color == Color::RED)));
        }
    }
}

#[test]
fn invalid_grapheme_ranges_and_replaced_fonts_fail_without_changing_owned_output() {
    let mut system = TextSystem::new();
    let id = system.register_builtin_font(BuiltinFont::Inter).unwrap();
    let font = FontDescriptor::default();
    let input = StyledTextRequest::new(
        "e\u{301} word",
        TextStyle::new(font.clone(), Color::BLACK),
        vec![],
        vec![],
        TextConstraints::UNBOUNDED,
    )
    .unwrap();
    let mut measure = system.measure(font.clone());
    let layout = measure.layout_styled(&input).unwrap();
    assert_eq!(
        measure.prepare_underlines(
            &layout,
            &[TextUnderline {
                range: 0..1,
                color: Color::BLACK
            }]
        ),
        Err(TextError::InvalidStyleRange)
    );
    let snapshot = measure
        .prepare_underlines(
            &layout,
            &[TextUnderline {
                range: 0..input.text().len(),
                color: Color::BLACK,
            }],
        )
        .unwrap();
    let before = snapshot.clone();
    system
        .replace_font(id, FontData::new(BuiltinFont::Inter.bytes().to_vec()))
        .unwrap();
    assert_eq!(
        system.measure(font).prepare_underlines(
            &layout,
            &[TextUnderline {
                range: 0..input.text().len(),
                color: Color::BLACK
            }]
        ),
        Err(TextError::NoUsableFont)
    );
    assert_eq!(snapshot, before);
}

struct ExternalEngine {
    delegate: ParleyTextEngine,
    mismatched: bool,
    missing: bool,
}
impl TextLayoutEngine for ExternalEngine {
    fn revision(&self) -> u64 {
        self.delegate.revision()
    }
    fn register_font(
        &mut self,
        id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        self.delegate.register_font(id, revision, bytes)
    }
    fn remove_font(&mut self, id: ResourceId) {
        TextLayoutEngine::remove_font(&mut self.delegate, id);
    }
    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.delegate.layout(text, font, constraints)
    }
    fn layout_styled(&mut self, input: &StyledTextRequest) -> Result<StyledTextLayout, TextError> {
        self.delegate.layout_styled(input)
    }
    fn resolved_font_metrics(&mut self, run: &GlyphRun) -> Result<FontMetrics, TextError> {
        if self.missing {
            return Err(TextError::FontMetricsUnsupported);
        }
        let metrics = self.delegate.resolved_font_metrics(run)?;
        if self.mismatched {
            FontMetrics::new(
                metrics.face().clone(),
                metrics.size() * 2.0,
                metrics.normalized_coords().to_vec(),
                metrics.ascent(),
                metrics.descent(),
                metrics.leading(),
            )
        } else {
            Ok(metrics)
        }
    }
}

#[test]
fn external_metrics_use_the_same_protocol_and_cannot_substitute_a_requested_font() {
    for (mismatched, missing) in [(false, false), (true, false), (false, true)] {
        let mut system = TextSystem::with_engine(Box::new(ExternalEngine {
            delegate: ParleyTextEngine::new(),
            mismatched,
            missing,
        }));
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let input = StyledTextRequest::new(
            "office",
            TextStyle::new(FontDescriptor::default(), Color::BLACK),
            vec![],
            vec![],
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
        let mut measure = system.measure(FontDescriptor::default());
        let layout = measure.layout_styled(&input).unwrap();
        let output = measure.prepare_underlines(
            &layout,
            &[TextUnderline {
                range: 0..6,
                color: Color::BLACK,
            }],
        );
        match (mismatched, missing) {
            (true, _) => assert_eq!(output, Err(TextError::InvalidStyledLayout)),
            (_, true) => assert_eq!(output, Err(TextError::FontMetricsUnsupported)),
            _ => assert!(!output.unwrap().fragments().is_empty()),
        }
    }
}

#[test]
#[ignore = "release cost evidence; run with --release --ignored"]
fn underline_direction_preparation_cost() {
    let mut system = system();
    let text = "office e\u{301} 中文 العربية words ".repeat(128);
    let mut rows = Vec::new();
    for mirrored in [false, true] {
        for width in [120.0, 600.0] {
            let input = StyledTextRequest::new(
                &text,
                TextStyle::new(FontDescriptor::default(), Color::BLACK),
                vec![],
                vec![],
                TextConstraints::new(Some(width)).unwrap(),
            )
            .unwrap()
            .with_layout_direction(if mirrored {
                LayoutDirection::RightToLeft
            } else {
                LayoutDirection::LeftToRight
            });
            let ranges = [TextUnderline {
                range: 0..text.len(),
                color: Color::BLUE,
            }];
            let mut layout_times = Vec::new();
            let mut underline_times = Vec::new();
            let mut recording_times = Vec::new();
            let mut counts = (0, 0, 0);
            for sample in 0..32 {
                let mut measure = system.measure(FontDescriptor::default());
                let start = std::time::Instant::now();
                let layout = measure.layout_styled(&input).unwrap();
                let layout_time = start.elapsed().as_secs_f64() * 1000.0;
                let start = std::time::Instant::now();
                let underline = measure.prepare_underlines(&layout, &ranges).unwrap();
                let underline_time = start.elapsed().as_secs_f64() * 1000.0;
                drop(measure);
                let start = std::time::Instant::now();
                let mut recorder = CommandRecorder::new();
                let mut graphics = Graphics::new(&mut system, &mut recorder);
                graphics
                    .fill_styled_text(&layout, Point::new(0.0, 0.0))
                    .unwrap();
                graphics
                    .fill_underlines(&underline, Point::new(0.0, 0.0))
                    .unwrap();
                drop(graphics);
                let recording_time = start.elapsed().as_secs_f64() * 1000.0;
                counts = (
                    layout.runs().len(),
                    underline.fragments().len(),
                    recorder.commands().len(),
                );
                if sample >= 2 {
                    layout_times.push(layout_time);
                    underline_times.push(underline_time);
                    recording_times.push(recording_time);
                }
            }
            layout_times.sort_by(f64::total_cmp);
            underline_times.sort_by(f64::total_cmp);
            recording_times.sort_by(f64::total_cmp);
            rows.push(serde_json::json!({
                "mirrored": mirrored, "width": width, "utf8_bytes": text.len(),
                "layout_p95_ms": layout_times[28], "underline_p95_ms": underline_times[28],
                "recording_p95_ms": recording_times[28],
                "runs": counts.0, "fragments": counts.1, "commands": counts.2,
            }));
        }
    }
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/verification/official-example-capability-increment/cost.json");
    std::fs::write(output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
