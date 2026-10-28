use novadraw::{
    geometry::{Point, Rectangle},
    graphics::Color,
    render::{FontData, ResourceId},
    text::{
        BuiltinFont, FontDescriptor, FontFaceRef, ParagraphAlignment, ParagraphDirection,
        StyledGlyphRun, StyledTextLayout, StyledTextRequest, TextAffinity, TextConstraints,
        TextError, TextInlineBox, TextInlinePlacement, TextInteractionError, TextLayout,
        TextLineMetrics, TextPosition, TextStyle, TextStyleSpan, TextSystem,
        shaping::{GlyphRun, PositionedGlyph, TextLayoutEngine, TextLayoutParts},
    },
};

fn style(size: f32, color: Color) -> TextStyle {
    TextStyle::new(
        FontDescriptor::new(BuiltinFont::Inter.family(), size).unwrap(),
        color,
    )
}

fn system() -> TextSystem {
    let mut system = TextSystem::new();
    system.register_builtin_font(BuiltinFont::Inter).unwrap();
    system
        .register_builtin_font(BuiltinFont::NotoSansArabic)
        .unwrap();
    system
}

#[test]
fn fractional_inline_height_ends_exactly_at_the_published_baseline() {
    let mut system = system();
    let request = request(
        "before after",
        vec![],
        vec![TextInlineBox {
            id: 1,
            byte_offset: 7,
            width: 160.0,
            height: 49.185493,
        }],
    );
    let layout = system
        .measure(style(14.0, Color::BLACK).font().clone())
        .layout_styled(&request)
        .unwrap();
    let placement = &layout.inline_placements()[0];
    assert_eq!(
        placement.bounds.y + placement.bounds.height,
        f64::from(layout.layout().line_metrics()[placement.line_index].baseline)
    );
}

fn request(text: &str, spans: Vec<TextStyleSpan>, boxes: Vec<TextInlineBox>) -> StyledTextRequest {
    StyledTextRequest::new(
        text,
        style(14.0, Color::BLACK),
        spans,
        boxes,
        TextConstraints::new(Some(160.0)).unwrap(),
    )
    .unwrap()
}

#[test]
fn rejects_overlapping_unordered_and_non_utf8_styles() {
    for ranges in [
        std::iter::once(0..1).collect(),
        vec![0..3, 2..4],
        vec![3..4, 0..3],
        std::iter::once(0..5).collect(),
    ] {
        let spans = ranges
            .into_iter()
            .map(|range| TextStyleSpan {
                range,
                style: style(14.0, Color::BLACK),
            })
            .collect();
        assert_eq!(
            StyledTextRequest::new(
                "中a",
                style(14.0, Color::BLACK),
                spans,
                vec![],
                TextConstraints::UNBOUNDED
            ),
            Err(TextError::InvalidStyleRange)
        );
    }
}

#[test]
fn rejects_nonfinite_boxes_and_duplicate_identity() {
    let input = TextInlineBox {
        id: 1,
        byte_offset: 0,
        width: f32::NAN,
        height: 4.0,
    };
    assert_eq!(
        StyledTextRequest::new(
            "a",
            style(14.0, Color::BLACK),
            vec![],
            vec![input],
            TextConstraints::UNBOUNDED
        ),
        Err(TextError::InvalidMetric)
    );
    let input = TextInlineBox {
        id: 1,
        byte_offset: 0,
        width: 4.0,
        height: 4.0,
    };
    assert_eq!(
        StyledTextRequest::new(
            "a",
            style(14.0, Color::BLACK),
            vec![],
            vec![input.clone(), input],
            TextConstraints::UNBOUNDED
        ),
        Err(TextError::InvalidInlineBox)
    );
}

#[test]
fn font_sizes_and_colors_share_a_paragraph_baseline() {
    let mut system = system();
    let input = request(
        "small BIG small",
        vec![TextStyleSpan {
            range: 6..9,
            style: style(30.0, Color::RED),
        }],
        vec![],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    assert_eq!(output.layout().line_metrics().len(), 1);
    let baseline = output.layout().baseline();
    assert!(
        output
            .layout()
            .glyph_runs()
            .iter()
            .all(|run| run.glyphs.iter().all(|g| g.y == baseline))
    );
    assert!(output.runs().iter().any(|run| run.style_index == 1));
    assert!(
        output
            .layout()
            .glyph_runs()
            .iter()
            .any(|run| run.font_size == 30.0)
    );
    assert_eq!(output.request().style(1).unwrap().color(), Color::RED);
}

#[test]
fn newline_only_layout_retains_explicit_line_breaks() {
    let mut system = system();
    let input = request(
        "a long paragraph that exceeds the line width\nsecond line",
        vec![],
        vec![],
    );
    let wrapped = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    let no_wrap =
        input.with_paragraph(ParagraphDirection::Auto, ParagraphAlignment::Leading, false);
    let unwrapped = system
        .measure(FontDescriptor::default())
        .layout_styled(&no_wrap)
        .unwrap();
    assert!(wrapped.layout().line_metrics().len() > 2);
    assert_eq!(unwrapped.layout().line_metrics().len(), 2);
    assert!(unwrapped.layout().width() > 160.0);
}

#[test]
fn inline_box_uses_the_line_baseline_and_keeps_identity() {
    let mut system = system();
    let input = request(
        "before after",
        vec![],
        vec![TextInlineBox {
            id: 42,
            byte_offset: 7,
            width: 70.0,
            height: 40.0,
        }],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    let placement = &output.inline_placements()[0];
    assert_eq!(placement.id, 42);
    assert_eq!(placement.bounds.width, 70.0);
    let line = output.layout().line_metrics()[placement.line_index];
    assert_eq!(
        placement.bounds.y + placement.bounds.height,
        f64::from(line.baseline)
    );
}

#[test]
fn rtl_logical_alignment_and_utf8_interaction_use_original_source() {
    let mut system = system();
    for direction in [
        ParagraphDirection::LeftToRight,
        ParagraphDirection::RightToLeft,
    ] {
        for alignment in [
            ParagraphAlignment::Leading,
            ParagraphAlignment::Trailing,
            ParagraphAlignment::Left,
            ParagraphAlignment::Right,
        ] {
            let input =
                request("abc العربية", vec![], vec![]).with_paragraph(direction, alignment, true);
            let output = system
                .measure(FontDescriptor::default())
                .layout_styled(&input)
                .unwrap();
            assert_eq!(output.layout().key().text(), input.text());
            assert!(
                output
                    .runs()
                    .iter()
                    .all(|run| run.source_range.end <= input.text().len())
            );
            for index in [0, 1, 2, 4, input.text().len()] {
                let caret = output
                    .layout()
                    .caret_geometry(TextPosition::new(index, TextAffinity::Downstream))
                    .unwrap();
                let hit = output
                    .layout()
                    .hit_test_text(Point::new(caret.bounds().x, caret.bounds().center().y()))
                    .unwrap();
                assert!(hit.byte_offset() <= input.text().len());
                assert!(input.text().is_char_boundary(hit.byte_offset()));
            }
            let physical_right = match alignment {
                ParagraphAlignment::Leading => direction == ParagraphDirection::RightToLeft,
                ParagraphAlignment::Trailing => direction == ParagraphDirection::LeftToRight,
                ParagraphAlignment::Right => true,
                _ => false,
            };
            let min_x = output
                .layout()
                .glyph_runs()
                .iter()
                .flat_map(|run| &run.glyphs)
                .map(|g| g.x)
                .fold(f32::INFINITY, f32::min);
            assert_eq!(min_x > 20.0, physical_right);
        }
    }
}

#[test]
fn paint_only_style_change_invalidates_old_caret_revision() {
    let mut system = system();
    let first = request(
        "abc",
        vec![TextStyleSpan {
            range: 0..3,
            style: style(14.0, Color::RED),
        }],
        vec![],
    );
    let second = request(
        "abc",
        vec![TextStyleSpan {
            range: 0..3,
            style: style(14.0, Color::BLACK),
        }],
        vec![],
    );
    let a = system
        .measure(FontDescriptor::default())
        .layout_styled(&first)
        .unwrap();
    let old = a.layout().hit_test_text(Point::new(0.0, 5.0)).unwrap();
    let b = system
        .measure(FontDescriptor::default())
        .layout_styled(&second)
        .unwrap();
    assert_eq!(
        b.layout().caret_geometry(old),
        Err(TextInteractionError::StaleTextLayoutRevision)
    );
}

#[derive(Default)]
struct ExternalEngine {
    face: Option<FontFaceRef>,
}

impl TextLayoutEngine for ExternalEngine {
    fn revision(&self) -> u64 {
        1
    }
    fn register_font(&mut self, id: ResourceId, revision: u64, _: &[u8]) -> Result<(), TextError> {
        self.face = Some(FontFaceRef::new(id, revision, 0));
        Ok(())
    }
    fn remove_font(&mut self, _: ResourceId) {
        self.face = None;
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
        let layout = TextLayout::from_parts(TextLayoutParts {
            text: request.text().to_owned(),
            font: request.default_style().font().clone(),
            constraints: request.constraints(),
            engine_revision: self.revision(),
            width: 40.0,
            full_width: 40.0,
            height: 24.0,
            lines: vec![TextLineMetrics {
                ascent: 20.0,
                descent: 4.0,
                leading: 0.0,
                baseline: 20.0,
                advance: 40.0,
            }],
            glyph_runs: vec![GlyphRun {
                font: self.face.clone().ok_or(TextError::NoUsableFont)?,
                font_size: 14.0,
                normalized_coords: std::sync::Arc::from([]),
                skew_degrees: None,
                glyphs: novadraw::text::shaping::GlyphRange::new(vec![PositionedGlyph {
                    id: 1,
                    x: 0.0,
                    y: 20.0,
                }])?,
            }],
            visible_range: 0..request.text().len(),
            truncated: false,
        })?;
        StyledTextLayout::new(
            request.clone(),
            layout,
            vec![StyledGlyphRun {
                source_range: 0..request.text().len(),
                style_index: 0,
                line_index: 0,
                baseline: 20.0,
                bounds: Rectangle::new(0.0, 0.0, 40.0, 24.0),
                right_to_left: false,
            }],
            request
                .inline_boxes()
                .iter()
                .map(|b| TextInlinePlacement {
                    id: b.id,
                    bounds: Rectangle::new(
                        20.0,
                        20.0 - f64::from(b.height),
                        f64::from(b.width),
                        f64::from(b.height),
                    ),
                    line_index: 0,
                })
                .collect(),
        )
    }
}

#[test]
fn external_engine_constructs_checked_output_without_parley() {
    let mut system = TextSystem::with_engine(Box::<ExternalEngine>::default());
    let id = system
        .register_font(FontData::new(BuiltinFont::Inter.bytes().to_vec()))
        .unwrap();
    let input = request(
        "external",
        vec![],
        vec![TextInlineBox {
            id: 7,
            byte_offset: 3,
            width: 10.0,
            height: 12.0,
        }],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    assert_eq!(
        output.layout().glyph_runs()[0].font.resource_id(),
        id.resource_id()
    );
    assert_eq!(output.inline_placements()[0].id, 7);
    system.remove_font(id).unwrap();
    assert_eq!(
        system
            .measure(FontDescriptor::default())
            .layout_styled(&input),
        Err(TextError::NoUsableFont)
    );
}

#[test]
fn checked_output_rejects_run_attributed_to_another_source_style() {
    let mut engine = ExternalEngine::default();
    let mut system = system();
    let input = request(
        "two spans",
        vec![
            TextStyleSpan {
                range: 0..3,
                style: style(14.0, Color::RED),
            },
            TextStyleSpan {
                range: 3..9,
                style: style(14.0, Color::BLACK),
            },
        ],
        vec![],
    );
    let valid = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    let mut runs = valid.runs().to_vec();
    runs[0].style_index = if runs[0].style_index == 1 { 2 } else { 1 };
    assert_eq!(
        StyledTextLayout::new(input.clone(), valid.layout().clone(), runs, vec![]),
        Err(TextError::InvalidStyledLayout)
    );
    // Equal font sizes and a valid style identity do not permit crossing its source span.
    engine
        .register_font(valid.layout().glyph_runs()[0].font.resource_id(), 1, &[])
        .unwrap();
    assert_eq!(
        engine.layout_styled(&input),
        Err(TextError::InvalidStyledLayout)
    );
}

#[test]
fn rejects_external_placement_that_does_not_match_measured_box() {
    let mut system = system();
    let input = request(
        "a",
        vec![],
        vec![TextInlineBox {
            id: 1,
            byte_offset: 0,
            width: 20.0,
            height: 10.0,
        }],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    let mut placements = output.inline_placements().to_vec();
    placements[0].bounds.width = 21.0;
    assert_eq!(
        StyledTextLayout::new(
            input.clone(),
            output.layout().clone(),
            output.runs().to_vec(),
            placements
        ),
        Err(TextError::InvalidStyledLayout)
    );
    assert!(
        StyledTextLayout::new(
            input,
            output.layout().clone(),
            output.runs().to_vec(),
            output.inline_placements().to_vec()
        )
        .is_ok()
    );
}

#[test]
fn normalized_inline_lines_keep_caret_and_selection_in_the_same_coordinates() {
    let mut system = system();
    let input = request(
        "abc def ghi jkl mno pqr stu vwx yz",
        vec![],
        vec![TextInlineBox {
            id: 1,
            byte_offset: 4,
            width: 90.0,
            height: 60.0,
        }],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    for index in [0, 1, 6, 12, 24] {
        let caret = output
            .layout()
            .caret_geometry(TextPosition::new(index, TextAffinity::Downstream))
            .unwrap();
        let hit = output
            .layout()
            .hit_test_text(Point::new(caret.bounds().x, caret.bounds().center().y()))
            .unwrap();
        assert_eq!(hit.byte_offset(), index);
        assert!(caret.bounds().y >= 0.0);
    }
    let selected = output
        .layout()
        .selection_geometry(novadraw::text::TextRange::new(
            TextPosition::new(0, TextAffinity::Downstream),
            TextPosition::new(input.text().len(), TextAffinity::Upstream),
        ))
        .unwrap();
    assert!(
        selected.iter().all(|quad| quad.bounds().y >= 0.0
            && quad.bounds().y + quad.bounds().height <= f64::from(output.layout().height())),
        "selection {selected:?}, height {}",
        output.layout().height()
    );
}

#[test]
fn styled_paint_keeps_colors_for_both_glyph_and_outline_paths() {
    use novadraw::{
        render::{CommandRecorder, GlyphPaint, RenderCommandKind},
        text::{
            outline::{OutlineTextEngine, SkrifaOutlineProvider},
            shaping::ParleyTextEngine,
        },
    };
    for outlines in [false, true] {
        let engine: Box<dyn TextLayoutEngine> = if outlines {
            Box::new(OutlineTextEngine::new(
                Box::new(ParleyTextEngine::new()),
                Box::new(SkrifaOutlineProvider),
            ))
        } else {
            Box::new(ParleyTextEngine::new())
        };
        let mut system = TextSystem::with_engine(engine);
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let input = request(
            "red black",
            vec![TextStyleSpan {
                range: 0..3,
                style: style(14.0, Color::RED),
            }],
            vec![],
        );
        let output = system
            .measure(FontDescriptor::default())
            .layout_styled(&input)
            .unwrap();
        let mut recorder = CommandRecorder::new();
        {
            let mut paint = recorder.paint();
            paint
                .fill_styled_text(&output, Point::new(10.0, 20.0))
                .unwrap();
        }
        let drawing = recorder.finish().unwrap();
        let colors = drawing
            .commands()
            .iter()
            .filter_map(|c| match &c.kind {
                RenderCommandKind::DrawGlyphRun {
                    paint: GlyphPaint::Fill(novadraw::graphics::Paint::Solid(color)),
                    ..
                }
                | RenderCommandKind::FillPath {
                    paint: novadraw::graphics::Paint::Solid(color),
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(colors.contains(&Color::RED));
        assert!(colors.contains(&Color::BLACK));
        assert_eq!(output.layout().outlines().is_some(), outlines);
    }
}

#[test]
fn same_font_color_boundaries_preserve_exact_source_ranges() {
    let mut system = system();
    let input = request(
        "one two three",
        vec![TextStyleSpan {
            range: 4..7,
            style: style(14.0, Color::RED),
        }],
        vec![],
    );
    let output = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    assert!(
        output
            .runs()
            .iter()
            .any(|run| run.source_range == (4..7) && run.style_index == 1)
    );
    assert!(
        output
            .runs()
            .iter()
            .filter(|run| run.style_index == 0)
            .all(|run| run.source_range.end <= 4 || run.source_range.start >= 7)
    );
}

#[test]
fn empty_text_and_box_only_requests_are_valid() {
    let mut system = system();
    for boxes in [
        vec![],
        vec![TextInlineBox {
            id: 3,
            byte_offset: 0,
            width: 40.0,
            height: 20.0,
        }],
    ] {
        let input = request("", vec![], boxes);
        let output = system
            .measure(FontDescriptor::default())
            .layout_styled(&input)
            .unwrap();
        assert_eq!(output.layout().key().text(), "");
        assert_eq!(output.inline_placements().len(), input.inline_boxes().len());
    }
}
