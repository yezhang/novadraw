//! Runs unchanged APIs against original and patched dependencies separately.
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, InlineBox, Layout, LayoutContext,
    OverflowWrap, PositionedLayoutItem, StyleProperty, TextWrapMode,
    fontique::{Blob, Collection, CollectionOptions},
};
use std::sync::Arc;

fn signature(layout: &Layout<usize>) -> String {
    let mut result = format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?};",
        layout.width().to_bits(),
        layout.full_width().to_bits(),
        layout.height().to_bits(),
        layout.is_rtl(),
        layout.styles(),
        layout.inline_boxes(),
    );
    for line in layout.lines() {
        result.push_str(&format!("{:?};", line.metrics()));
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyphs) => {
                    let run = glyphs.run();
                    result.push_str(&format!(
                        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?};",
                        run.font().data.id(),
                        run.font().index,
                        run.font_size().to_bits(),
                        run.normalized_coords(),
                        glyphs.offset().to_bits(),
                        glyphs.baseline().to_bits(),
                    ));
                    for glyph in glyphs.glyphs() {
                        result.push_str(&format!("{glyph:?};"));
                    }
                }
                PositionedLayoutItem::InlineBox(inline) => {
                    result.push_str(&format!("{inline:?};"));
                }
            }
        }
    }
    result
}

fn main() {
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    for bytes in [
        &include_bytes!("../../../../assets/fonts/InterVariable.ttf")[..],
        &include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf")[..],
        &include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf")[..],
    ] {
        fonts
            .collection
            .register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
    }
    let families = FontFamily::parse_list("Inter Variable, Noto Sans Arabic, Noto Sans SC")
        .collect::<Vec<_>>();
    let mut context = LayoutContext::<usize>::new();
    let mut cases = 0;
    for text in [
        "",
        "office a\u{301} 👩‍💻 spaces   \n",
        "中文 العربية mixed\nlast\n",
        "one extraordinarilylongword two  three\t\n\n",
    ] {
        for inline in [false, true] {
            for wrap in [
                OverflowWrap::Normal,
                OverflowWrap::Anywhere,
                OverflowWrap::BreakWord,
            ] {
                for mode in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
                    let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
                    builder.push_default(families.as_slice());
                    builder.push_default(StyleProperty::FontSize(14.0));
                    builder.push_default(StyleProperty::OverflowWrap(wrap));
                    builder.push_default(StyleProperty::TextWrapMode(mode));
                    if inline {
                        builder.push_inline_box(InlineBox {
                            id: 1,
                            index: 0,
                            width: 24.0,
                            height: 40.0,
                        });
                    }
                    let template = builder.build(text);
                    for width in [None, Some(0.0), Some(1.0), Some(80.0), Some(400.0)] {
                        for alignment in [
                            Alignment::Start,
                            Alignment::End,
                            Alignment::Center,
                            Alignment::Justify,
                        ] {
                            let mut layout = template.clone();
                            layout.break_all_lines(width);
                            layout.align(width, alignment, AlignmentOptions::default());
                            println!("case {cases}: {}", signature(&layout));
                            // Reflow from an already aligned layout, then exercise
                            // the public iterator's rollback and Drop path.
                            layout.break_all_lines(Some(32.0));
                            println!("reflow {cases}: {}", signature(&layout));
                            {
                                let mut breaker = layout.break_lines();
                                let first = breaker.break_next(60.0);
                                assert!(breaker.revert());
                                assert_eq!(first, breaker.break_next(60.0));
                            }
                            println!("partial {cases}: {}", signature(&layout));
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 960);
    println!("cases={cases}");
}
