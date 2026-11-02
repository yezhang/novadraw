//! Isolated dependency-candidate tests. This crate does not depend on Novadraw.
use parley::{
    Layout, LayoutOperation, LayoutOperationCost, LayoutOperationError, LayoutOwnedStorage,
};

/// Keeps the read-only API present in Native and Wasm library builds.
pub fn observe(layout: &Layout<usize>) -> LayoutOwnedStorage {
    layout.owned_storage()
}

/// Keeps the admitted copy API present in Native and Wasm library builds.
pub fn copy<E>(
    layout: &Layout<usize>,
    admit: impl FnMut(LayoutOperation, LayoutOperationCost) -> Result<(), E>,
) -> Result<Layout<usize>, LayoutOperationError<E>> {
    layout.try_clone_admitted(admit)
}

/// Keeps the consuming admitted line-breaking API in Native/Wasm builds.
pub fn reflow<E>(
    layout: Layout<usize>,
    width: Option<f32>,
    admit: impl FnMut(LayoutOperation, LayoutOperationCost) -> Result<(), E>,
) -> Result<Layout<usize>, LayoutOperationError<E>> {
    layout.try_break_all_lines_admitted(width, admit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley::{
        Alignment, AlignmentOptions, FontContext, FontFamily, InlineBox, LayoutContext,
        LayoutStorageField as Field, PositionedLayoutItem, StyleProperty,
        fontique::{Blob, Collection, CollectionOptions},
    };
    use std::sync::Arc;

    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/InterVariable.ttf");
    const ARABIC: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf");
    const CJK: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf");

    fn fonts() -> FontContext {
        let mut context = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: Default::default(),
        };
        for bytes in [FONT, ARABIC, CJK] {
            context
                .collection
                .register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
        }
        context
    }

    fn shape(
        context: &mut FontContext,
        layout_context: &mut LayoutContext<usize>,
        text: &str,
        inline: bool,
    ) -> Layout<usize> {
        shape_options(
            context,
            layout_context,
            text,
            inline,
            parley::OverflowWrap::Normal,
            parley::TextWrapMode::Wrap,
        )
    }

    fn shape_options(
        context: &mut FontContext,
        layout_context: &mut LayoutContext<usize>,
        text: &str,
        inline: bool,
        wrap: parley::OverflowWrap,
        mode: parley::TextWrapMode,
    ) -> Layout<usize> {
        let mut builder = layout_context.ranged_builder(context, text, 1.0, true);
        let families = FontFamily::parse_list("Inter Variable, Noto Sans Arabic, Noto Sans SC")
            .collect::<Vec<_>>();
        builder.push_default(families.as_slice());
        builder.push_default(StyleProperty::FontSize(14.0));
        builder.push_default(StyleProperty::Brush(0));
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
        builder.build(text)
    }

    fn signature(layout: &Layout<usize>) -> String {
        let mut result = format!(
            "{:?}/{:?}/{:?}/{:?};",
            layout.width().to_bits(),
            layout.full_width().to_bits(),
            layout.height().to_bits(),
            layout.is_rtl()
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

    #[test]
    fn observations_leave_geometry_and_reflow_unchanged() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let mut cases = 0;
        for text in ["", "office a\u{301} 👩‍💻", "中文 العربية mixed\nlast\n"] {
            for inline in [false, true] {
                let template = shape(&mut fonts, &mut context, text, inline);
                for width in [None, Some(0.0), Some(1.0), Some(120.0), Some(400.0)] {
                    for alignment in [
                        Alignment::Start,
                        Alignment::End,
                        Alignment::Center,
                        Alignment::Justify,
                    ] {
                        let mut observed = template.clone();
                        let mut control = template.clone();
                        let initial = observe(&observed);
                        assert_eq!(initial, observe(&observed));
                        observed.break_all_lines(width);
                        let broken = observe(&observed);
                        observed.align(width, alignment, AlignmentOptions::default());
                        let aligned = observe(&observed);
                        assert_eq!(broken, aligned, "alignment allocates no Layout arrays");
                        let before_read = signature(&observed);
                        assert_eq!(aligned, observe(&observed));
                        assert_eq!(before_read, signature(&observed));
                        control.break_all_lines(width);
                        control.align(width, alignment, AlignmentOptions::default());
                        assert_eq!(before_read, signature(&control));
                        assert!(aligned.owned_bytes().unwrap() >= aligned.object_bytes);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 120);
    }

    #[test]
    fn template_clone_and_line_result_have_distinct_observed_storage() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let template = shape(&mut fonts, &mut context, &"office 中文 ".repeat(512), false);
        let template_storage = observe(&template);
        assert_eq!(template_storage.array(Field::Lines).unwrap().len, 0);
        let mut clone = template.clone();
        for (original, copied) in template_storage.arrays.iter().zip(observe(&clone).arrays) {
            assert_eq!(original.len, copied.len);
            assert!(copied.capacity >= copied.len);
        }
        clone.break_all_lines(Some(60.0));
        let result = observe(&clone);
        assert!(result.array(Field::Lines).unwrap().len > 100);
        assert!(result.array(Field::LineItems).unwrap().len > 100);
        assert_eq!(
            observe(&template),
            template_storage,
            "reflow must not mutate the template"
        );
        assert!(result.owned_bytes().unwrap() > template_storage.object_bytes);
    }

    #[test]
    fn backing_resolution_accounts_for_capacity_and_lifetime() {
        let mut bytes = Vec::with_capacity(FONT.len() + 1024);
        bytes.extend_from_slice(FONT);
        let capacity = bytes.capacity();
        let bytes = Arc::new(bytes);
        let weak = Arc::downgrade(&bytes);
        let mut fonts = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: Default::default(),
        };
        let blob = Blob::new(bytes.clone());
        let blob_id = blob.id();
        fonts.collection.register_fonts(blob, None);
        let mut context = LayoutContext::new();
        let layout = shape(&mut fonts, &mut context, "office", false);
        assert!(layout.retained_fonts().len() > 0);
        let mut identities = std::collections::BTreeSet::new();
        for font in layout.retained_fonts() {
            assert_eq!(font.data.id(), blob_id);
            assert_eq!(font.data.data().len(), FONT.len());
            identities.insert(font.data.id());
        }
        assert_eq!(identities.len(), 1);
        assert!(
            capacity > FONT.len(),
            "blob length alone would undercount this backing"
        );
        let clone = layout.clone();
        assert_eq!(
            layout
                .retained_fonts()
                .map(|font| font.data.id())
                .collect::<Vec<_>>(),
            clone
                .retained_fonts()
                .map(|font| font.data.id())
                .collect::<Vec<_>>()
        );
        drop(context);
        drop(fonts);
        drop(bytes);
        drop(layout);
        assert!(
            weak.upgrade().is_some(),
            "the clone still pins the font backing"
        );
        drop(clone);
        assert!(
            weak.upgrade().is_none(),
            "the final layout releases the font backing"
        );
    }

    #[test]
    fn admitted_copy_and_alignment_preserve_all_alignment_transitions() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let alignments = [
            Alignment::Start,
            Alignment::End,
            Alignment::Center,
            Alignment::Justify,
        ];
        let mut copies = 0;
        let mut transitions = 0;
        for text in ["", "office a\u{301} 👩‍💻", "中文 العربية mixed\nlast\n"] {
            for inline in [false, true] {
                let template = shape(&mut fonts, &mut context, text, inline);
                for width in [None, Some(0.0), Some(1.0), Some(120.0), Some(400.0)] {
                    for previous in alignments {
                        let mut source = template.clone();
                        source.break_all_lines(width);
                        source.align(width, previous, AlignmentOptions::default());
                        let source_signature = signature(&source);
                        let mut steps = Vec::new();
                        let clone = copy(&source, |step, cost| {
                            steps.push((step, cost));
                            Ok::<_, ()>(())
                        })
                        .unwrap();
                        assert_eq!(source_signature, signature(&clone));
                        assert_eq!(source.styles(), clone.styles());
                        assert_eq!(source.inline_boxes(), clone.inline_boxes());
                        copies += 1;
                        let elements = observe(&source)
                            .arrays
                            .iter()
                            .map(|array| array.len)
                            .sum::<usize>();
                        assert_eq!(steps.len(), 2);
                        assert_eq!(steps[1].0, LayoutOperation::Clone);
                        assert_eq!(steps[1].1.work, elements);
                        assert_eq!(steps[1].1.allocation, elements);
                        for array in observe(&clone).arrays {
                            assert_eq!(array.capacity, array.len);
                        }
                        for next in alignments {
                            for overflowing in [false, true] {
                                let mut admitted = clone.clone();
                                let mut control = source.clone();
                                let options = AlignmentOptions {
                                    align_when_overflowing: overflowing,
                                };
                                let storage = observe(&admitted);
                                let mut costs = Vec::new();
                                admitted
                                    .try_align_admitted(width, next, options, |step, cost| {
                                        costs.push((step, cost));
                                        Ok::<_, ()>(())
                                    })
                                    .unwrap();
                                control.align(width, next, options);
                                assert_eq!(signature(&admitted), signature(&control));
                                assert_eq!(observe(&admitted), storage);
                                assert_eq!(costs.len(), 2);
                                transitions += 1;
                                assert!(
                                    costs.iter().all(|(_, cost)| cost.allocation == 0),
                                    "alignment must not reserve arrays"
                                );
                            }
                        }
                        assert_eq!(source_signature, signature(&source));
                    }
                }
            }
        }
        assert_eq!(copies, 120);
        assert_eq!(transitions, 960);
    }

    #[test]
    fn admitted_breaking_matches_ordinary_reflow_and_capacity_costs() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let mut cases = 0;
        for text in [
            "",
            "office a\u{301} 👩‍💻 spaces   \n",
            "中文 العربية mixed\nlast\n",
            "one extraordinarilylongword two  three\t\n\n",
        ] {
            for inline in [false, true] {
                for (wrap, mode) in [
                    (parley::OverflowWrap::Normal, parley::TextWrapMode::Wrap),
                    (parley::OverflowWrap::Anywhere, parley::TextWrapMode::Wrap),
                    (parley::OverflowWrap::BreakWord, parley::TextWrapMode::Wrap),
                    (parley::OverflowWrap::Normal, parley::TextWrapMode::NoWrap),
                ] {
                    let template =
                        shape_options(&mut fonts, &mut context, text, inline, wrap, mode);
                    for old_width in [Some(24.0), Some(300.0)] {
                        for old_alignment in [Alignment::Start, Alignment::Justify] {
                            let mut source = template.clone();
                            source.break_all_lines(old_width);
                            source.align(old_width, old_alignment, AlignmentOptions::default());
                            let before = signature(&source);
                            for width in [None, Some(0.0), Some(1.0), Some(80.0), Some(400.0)] {
                                let mut ordinary = source.clone();
                                ordinary.break_all_lines(width);
                                let candidate = source.clone();
                                let initial = observe(&candidate);
                                let mut allocated = 0;
                                let mut final_scans = 0;
                                let result = reflow(candidate, width, |step, cost| {
                                    if step == LayoutOperation::LineStorage {
                                        allocated += cost.allocation;
                                    }
                                    if step == LayoutOperation::LineFinalize {
                                        final_scans += cost.work;
                                    }
                                    Ok::<_, ()>(())
                                })
                                .unwrap();
                                assert_eq!(signature(&result), signature(&ordinary));
                                assert_eq!(result.styles(), ordinary.styles());
                                assert_eq!(result.inline_boxes(), ordinary.inline_boxes());
                                assert_eq!(before, signature(&source));
                                let storage = observe(&result);
                                let growth = [Field::Lines, Field::LineItems]
                                    .into_iter()
                                    .map(|field| {
                                        storage.array(field).unwrap().capacity
                                            - initial.array(field).unwrap().capacity
                                    })
                                    .sum::<usize>();
                                assert!(allocated >= growth);
                                assert_eq!(
                                    final_scans,
                                    result.len() + usize::from(text.is_empty())
                                );
                                for field in [
                                    Field::Fonts,
                                    Field::Coordinates,
                                    Field::Styles,
                                    Field::InlineBoxes,
                                    Field::Runs,
                                    Field::Items,
                                    Field::Clusters,
                                    Field::Glyphs,
                                ] {
                                    assert_eq!(storage.array(field), initial.array(field));
                                }
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 640);
    }

    #[test]
    fn every_breaking_rejection_stops_callbacks_and_preserves_template() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        for text in ["", "abc العربية def\n", "one two\n\n"] {
            let template = shape(&mut fonts, &mut context, text, true);
            let before = observe(&template);
            let mut steps = Vec::new();
            reflow(template.clone(), Some(20.0), |step, cost| {
                steps.push((step, cost));
                Ok::<_, usize>(())
            })
            .unwrap();
            assert!(
                steps
                    .iter()
                    .any(|(s, _)| *s == LayoutOperation::LineStorage)
            );
            assert!(
                steps
                    .iter()
                    .any(|(s, _)| *s == LayoutOperation::LineFinalize)
            );
            for denied in 0..steps.len() {
                let mut calls = 0;
                let result = reflow(template.clone(), Some(20.0), |step, cost| {
                    assert_eq!((step, cost), steps[calls]);
                    let current = calls;
                    calls += 1;
                    if current == denied {
                        Err(denied)
                    } else {
                        Ok(())
                    }
                });
                assert!(matches!(result, Err(LayoutOperationError::Admission(i)) if i == denied));
                assert_eq!(calls, denied + 1, "no callbacks follow rejection");
                assert_eq!(observe(&template), before);
            }
        }
    }

    #[test]
    fn long_reflow_admission_work_stays_linear_for_repeated_paragraphs() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let mut measurements = Vec::new();
        for repeats in [128, 256, 512] {
            let text = "office 中文 العربية a\u{301} one two three\n".repeat(repeats);
            let template = shape(&mut fonts, &mut context, &text, false);
            let mut work = 0;
            let mut allocation = 0;
            let mut ordinary = template.clone();
            ordinary.break_all_lines(Some(80.0));
            let result = reflow(template, Some(80.0), |_, cost| {
                work += cost.work;
                allocation += cost.allocation;
                Ok::<_, ()>(())
            })
            .unwrap();
            assert_eq!(signature(&result), signature(&ordinary));
            measurements.push((work, allocation));
        }
        // A fixed repeated paragraph exercises repeated rollback, metrics and
        // bidi passes. Doubling input must not introduce quadratic charging.
        for pair in measurements.windows(2) {
            assert!(pair[1].0 <= pair[0].0 * 2 + 32);
            assert!(pair[1].1 <= pair[0].1 * 2 + 32);
        }
    }

    #[test]
    fn either_admission_rejection_preserves_template_and_geometry() {
        let mut fonts = fonts();
        let mut context = LayoutContext::new();
        let mut source = shape(
            &mut fonts,
            &mut context,
            "office 中文 العربية a b c d e f g h i j",
            true,
        );
        source.break_all_lines(Some(100.0));
        source.align(Some(100.0), Alignment::Justify, AlignmentOptions::default());
        let storage = observe(&source);
        let geometry = signature(&source);
        for rejected in [
            LayoutOperation::ClonePlanning,
            LayoutOperation::Clone,
            LayoutOperation::AlignmentPlanning,
            LayoutOperation::Alignment,
        ] {
            let mut calls = Vec::new();
            let mut admit = |step, _| {
                calls.push(step);
                if step == rejected {
                    Err(rejected)
                } else {
                    Ok(())
                }
            };
            let result = match rejected {
                LayoutOperation::ClonePlanning | LayoutOperation::Clone => {
                    copy(&source, &mut admit).map(|_| ())
                }
                _ => source.try_align_admitted(
                    Some(400.0),
                    Alignment::End,
                    AlignmentOptions::default(),
                    &mut admit,
                ),
            };
            assert!(matches!(
                result,
                Err(LayoutOperationError::Admission(step)) if step == rejected
            ));
            assert_eq!(calls.last(), Some(&rejected));
            assert_eq!(
                calls.len(),
                usize::from(matches!(
                    rejected,
                    LayoutOperation::Clone | LayoutOperation::Alignment
                )) + 1
            );
            assert_eq!(observe(&source), storage);
            assert_eq!(signature(&source), geometry);
        }
    }
}
