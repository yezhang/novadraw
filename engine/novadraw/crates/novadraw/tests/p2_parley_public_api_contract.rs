//! Unpatched upstream layout cloning and width-only rebreaking contracts.
//! Core stage budgets are covered separately by p2_text_stages.
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, Layout, LayoutContext,
    PositionedLayoutItem, StyleProperty,
    fontique::{Blob, Collection, CollectionOptions},
};
use std::sync::Arc;

fn fonts() -> FontContext {
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    for bytes in [
        include_bytes!("../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
    ] {
        fonts
            .collection
            .register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
    }
    fonts
}

fn build(fonts: &mut FontContext, context: &mut LayoutContext<usize>, text: &str) -> Layout<usize> {
    let mut builder = context.ranged_builder(fonts, text, 1.0, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::from(
        "Inter Variable, Noto Sans SC, Noto Sans Arabic",
    )));
    builder.push_default(StyleProperty::FontSize(18.0));
    builder.push_default(StyleProperty::Brush(0));
    let end = text
        .char_indices()
        .nth(3)
        .map_or(text.len(), |(index, _)| index);
    builder.push(StyleProperty::Brush(1), 0..end);
    builder.push_inline_box(parley::InlineBox {
        id: 1,
        kind: parley::InlineBoxKind::InFlow,
        index: 0,
        width: 21.0,
        height: 26.0,
        baseline: None,
        vertical_align: parley::VerticalAlign::BASELINE,
    });
    builder.build(text)
}

fn signature(layout: &Layout<usize>) -> String {
    let mut result = format!(
        "{:?}",
        (layout.width(), layout.full_width(), layout.height())
    );
    for line in layout.lines() {
        result.push_str(&format!("{:?}{:?}", line.metrics(), line.text_range()));
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(run) => result.push_str(&format!(
                    "{:?}",
                    (
                        run.style().brush,
                        run.offset(),
                        run.baseline(),
                        run.run().text_range(),
                        run.run().is_rtl(),
                        run.run().normalized_coords(),
                        run.glyphs().collect::<Vec<_>>(),
                    )
                )),
                PositionedLayoutItem::InlineBox(inline) => result.push_str(&format!("{inline:?}")),
            }
        }
    }
    result
}

#[test]
fn cloned_width_rebreak_matches_fresh_layout_without_mutating_source() {
    let mut fonts = fonts();
    let mut context = LayoutContext::new();
    for text in [
        "office ffi café a\u{301} multi word\n",
        "中文混排 with English 与标点。\n下一行",
        "العربية مع office 123 mixed text",
    ] {
        let mut source = build(&mut fonts, &mut context, text);
        source.break_all_lines(Some(90.0));
        source.align(Alignment::Justify, AlignmentOptions::default());
        let original = signature(&source);
        for width in [40.0, 90.0, 200.0, 400.0, 90.0] {
            for alignment in [Alignment::Start, Alignment::End, Alignment::Center] {
                let mut copy = source.clone();
                copy.break_all_lines(Some(width));
                copy.align(alignment, AlignmentOptions::default());
                let mut fresh = build(&mut fonts, &mut context, text);
                fresh.break_all_lines(Some(width));
                fresh.align(alignment, AlignmentOptions::default());
                assert_eq!(
                    signature(&copy),
                    signature(&fresh),
                    "{text:?}, {width}, {alignment:?}"
                );
                assert_eq!(signature(&source), original);
            }
        }
    }
}

#[test]
fn retained_layout_survives_builder_context_and_font_collection_reuse() {
    let mut fonts = fonts();
    let mut context = LayoutContext::new();
    let mut retained = build(&mut fonts, &mut context, "office 中文 العربية");
    retained.break_all_lines(Some(90.0));
    retained.align(Alignment::Start, AlignmentOptions::default());
    let original = signature(&retained);
    let _other = build(&mut fonts, &mut context, "replacement");
    drop(context);
    drop(fonts);
    assert_eq!(signature(&retained), original);
    assert!(retained.lines().flat_map(|line| line.items()).any(|item|
        matches!(item, PositionedLayoutItem::GlyphRun(run) if !run.run().font().data.data().is_empty())
    ));
}

#[test]
fn unwrapped_core_lines_align_to_container_and_keep_original_offsets() {
    use novadraw::{
        graphics::Color,
        render::{ParleyTextEngine, ResourceId, TextLayoutEngine},
        text::{
            BuiltinFont, FontDescriptor, ParagraphAlignment, ParagraphDirection, StyledTextRequest,
            TextAffinity, TextConstraints, TextPosition, TextStyle,
        },
    };
    let mut engine = ParleyTextEngine::new();
    engine
        .register_font(
            ResourceId::new(uuid::Uuid::nil(), 1),
            1,
            BuiltinFont::Inter.bytes(),
        )
        .unwrap();
    let container_width = 300.0;
    let narrow = StyledTextRequest::new(
        "this long paragraph must remain unwrapped\nnext",
        TextStyle::new(
            FontDescriptor::new(BuiltinFont::Inter.family(), 18.0).unwrap(),
            Color::BLACK,
        ),
        Vec::new(),
        Vec::new(),
        TextConstraints::new(Some(40.0)).unwrap(),
    )
    .unwrap()
    .with_paragraph(
        ParagraphDirection::LeftToRight,
        ParagraphAlignment::Center,
        false,
    );
    let unwrapped = engine.layout_styled(&narrow).unwrap();
    assert_eq!(unwrapped.layout().line_metrics().len(), 2);
    assert!(unwrapped.layout().width() > 40.0);
    for (text, direction, alignment) in [
        (
            "short\nsecond",
            ParagraphDirection::LeftToRight,
            ParagraphAlignment::Center,
        ),
        (
            "123 / 456",
            ParagraphDirection::RightToLeft,
            ParagraphAlignment::Leading,
        ),
        (
            "",
            ParagraphDirection::RightToLeft,
            ParagraphAlignment::Leading,
        ),
    ] {
        let request = StyledTextRequest::new(
            text,
            TextStyle::new(
                FontDescriptor::new(BuiltinFont::Inter.family(), 18.0).unwrap(),
                Color::BLACK,
            ),
            Vec::new(),
            Vec::new(),
            TextConstraints::new(Some(container_width)).unwrap(),
        )
        .unwrap()
        .with_paragraph(direction, alignment, false);
        let result = engine.layout_styled(&request).unwrap();
        assert_eq!(result.layout().key().text(), text);
        for run in result.runs() {
            assert!(run.source_range.end <= text.len());
            assert!(text.is_char_boundary(run.source_range.start));
        }
        for (index, line) in result.layout().line_metrics().iter().enumerate() {
            let runs = result.runs().iter().filter(|run| run.line_index == index);
            let left = runs.map(|run| run.bounds.x).reduce(f64::min);
            if let Some(left) = left {
                let expected = f64::from(container_width - line.advance)
                    / if alignment == ParagraphAlignment::Center {
                        2.0
                    } else {
                        1.0
                    };
                assert!(
                    (left - expected).abs() < 0.01,
                    "{text:?}: {left} != {expected}"
                );
            }
        }
        let position = TextPosition::new(text.len(), TextAffinity::Upstream);
        let caret = result.layout().caret_geometry(position).unwrap();
        assert!(caret.bounds().x.is_finite());
        assert!(caret.bounds().y.is_finite());
    }
}
