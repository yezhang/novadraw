use harfrust::{
    BufferFlags, Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData,
    UnicodeBuffer, script,
};
#[path = "context_fixtures.rs"]
mod fixtures;
#[path = "ligature_fixtures.rs"]
#[allow(dead_code)]
mod ligature;
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn input(text: &str, clusters: usize, spare: usize) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    for (index, character) in text.chars().enumerate() {
        buffer.add(
            character,
            match clusters {
                0 => index as u32,
                1 => 0,
                _ => (text.len() - index) as u32,
            },
        );
    }
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

#[test]
fn context_inline_matching_rewind_and_repeated_records_match_complete_output() {
    for bytes in fixtures::fonts().into_iter().take(16) {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        for text in ["AAAAAAAAAAAAB", "ABCABC other", "A", ""] {
            for clusters in 0..3 {
                for spare in [0, 256] {
                    let expected = shaper.shape_with_plan(&plan, input(text, clusters, spare), &[]);
                    let actual = shaper
                        .try_shape_with_plan_admitted(
                            &plan,
                            &mut input(text, clusters, spare),
                            &[],
                            |_, _| Ok::<_, ()>(()),
                        )
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        format!("{:?}", expected.glyph_infos()),
                        format!("{:?}", actual.glyph_infos())
                    );
                    assert_eq!(
                        format!("{:?}", expected.glyph_positions()),
                        format!("{:?}", actual.glyph_positions())
                    );
                    assert_eq!(actual.len(), text.len());
                    if text == "AAAAAAAAAAAAB" {
                        assert!(actual.glyph_infos()[0].glyph_id >= 67);
                    }
                }
            }
        }
    }
}

#[test]
fn every_context_rejection_preserves_exact_prefix_and_buffer_reuse() {
    let bytes = fixtures::font(8, 2, 3, &fixtures::records(8), 1, 0);
    let run = |reject, hot| {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        if hot {
            shaper.shape_with_plan(&plan, input("AAAAAAAAAAAAB", 0, 0), &[]);
        }
        let mut buffer = input("AAAAAAAAAAAAB", 0, 0);
        let mut steps = Vec::new();
        let mut pipeline = false;
        let result = shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
            let index = steps.len();
            steps.push((op, cost));
            if Some(index) == reject {
                return Err(index);
            }
            pipeline |= op == ShapeOperation::Recovery;
            Ok(())
        });
        if let Some(reject) = reject {
            assert!(
                matches!(result, Err(ShapeOperationError::Admission(index)) if index == reject)
            );
            assert_eq!(steps.len(), reject + 1);
            assert_eq!(buffer.len(), if pipeline { 0 } else { 13 });
            buffer.clear();
            buffer.push_str("next");
            buffer.set_script(script::LATIN);
            buffer.set_direction(Direction::LeftToRight);
            assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
        } else {
            assert_eq!(result.unwrap().unwrap().len(), 13);
        }
        steps
    };
    for hot in [false, true] {
        let complete = run(None, hot);
        for reject in 0..complete.len() {
            assert_eq!(run(Some(reject), hot), complete[..=reject]);
        }
    }
}

#[test]
fn context_unknown_recursion_growth_and_flags_decline_before_pipeline() {
    for bytes in fixtures::fonts().into_iter().skip(16).take(3) {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        let mut buffer = input("AAAAAAAAAAAAB", 0, 0);
        let mut pipeline = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                pipeline |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(result.is_none());
        assert!(!pipeline);
        assert_eq!(buffer.len(), 13);
        buffer.clear();
        buffer.push_str("next");
        buffer.set_script(script::LATIN);
        buffer.set_direction(Direction::LeftToRight);
        assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
    }
}

#[test]
fn context_rejection_retains_source_and_sticky_budget() {
    use std::{error::Error, fmt};
    #[derive(Debug)]
    struct Reject;
    impl fmt::Display for Reject {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("nested work rejected")
        }
    }
    impl Error for Reject {}
    let bytes = fixtures::font(8, 2, 3, &fixtures::records(8), 1, 0);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(
        &shaper,
        Direction::LeftToRight,
        Some(script::LATIN),
        None,
        &[],
    );
    let mut failed = false;
    let mut pipeline = false;
    let mut policy = |op, _| {
        pipeline |= op == ShapeOperation::Recovery;
        if failed || (pipeline && op == ShapeOperation::Matching(harfrust::BufferOperation::Append))
        {
            failed = true;
            Err(Reject)
        } else {
            Ok(())
        }
    };
    let error = shaper
        .try_shape_with_plan_admitted(&plan, &mut input("AAAAAAAAAA", 0, 0), &[], &mut policy)
        .unwrap_err();
    assert!(error.source().unwrap().downcast_ref::<Reject>().is_some());
    let mut next = input("next", 0, 0);
    assert!(
        shaper
            .try_shape_with_plan_admitted(&plan, &mut next, &[], &mut policy)
            .is_err()
    );
    assert_eq!(next.len(), 4);
    assert_eq!(shaper.shape_with_plan(&plan, next, &[]).len(), 4);
}

#[test]
fn context_spill_prepares_two_arrays_and_matches_output_at_kernel_limit() {
    for count in [9, 17, 64] {
        let bytes = fixtures::font(count, 2, 3, &fixtures::records(count), 1, 0);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        let text = "A".repeat(usize::from(count) * 2 + 1);
        for clusters in 0..3 {
            for spare in [0, 256] {
                let expected = shaper.shape_with_plan(&plan, input(&text, clusters, spare), &[]);
                let mut allocation = Vec::new();
                let actual = shaper
                    .try_shape_with_plan_admitted(
                        &plan,
                        &mut input(&text, clusters, spare),
                        &[],
                        |op, cost| {
                            if op == ShapeOperation::Matching(harfrust::BufferOperation::Storage) {
                                allocation.push(cost.allocation);
                            }
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap()
                    .unwrap();
                // Per-shape matching and backup arrays, reused across every
                // nested record, input position and outer subtable.
                assert_eq!(allocation, [usize::from(count); 2]);
                assert_eq!(
                    format!("{:?}", expected.glyph_infos()),
                    format!("{:?}", actual.glyph_infos())
                );
                assert_eq!(
                    format!("{:?}", expected.glyph_positions()),
                    format!("{:?}", actual.glyph_positions())
                );
                assert!(actual.glyph_infos()[0].glyph_id >= 67);
            }
        }
    }
}

#[test]
fn every_spill_rejection_preserves_source_prefix_and_ordinary_reuse() {
    use std::{error::Error, fmt};
    #[derive(Debug)]
    struct Reject(usize);
    impl fmt::Display for Reject {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "spill rejected at {}", self.0)
        }
    }
    impl Error for Reject {}
    for count in [9, 64] {
        let bytes = fixtures::font(count, 2, 3, &fixtures::records(count), 1, 0);
        let text = "A".repeat(usize::from(count) + 2);
        let run = |reject, hot| {
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            let plan = ShapePlan::new(
                &shaper,
                Direction::LeftToRight,
                Some(script::LATIN),
                None,
                &[],
            );
            if hot {
                shaper.shape_with_plan(&plan, input(&text, 0, 0), &[]);
            }
            let mut buffer = input(&text, 0, 0);
            let mut steps = Vec::new();
            let mut pipeline = false;
            let result =
                shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
                    let index = steps.len();
                    steps.push((op, cost));
                    if Some(index) == reject {
                        return Err(Reject(index));
                    }
                    pipeline |= op == ShapeOperation::Recovery;
                    Ok(())
                });
            if let Some(index) = reject {
                let error = result.unwrap_err();
                assert_eq!(
                    error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                    index
                );
                assert_eq!(steps.len(), index + 1);
                assert_eq!(buffer.len(), if pipeline { 0 } else { text.len() });
                buffer.clear();
                buffer.push_str("next");
                buffer.set_script(script::LATIN);
                buffer.set_direction(Direction::LeftToRight);
                assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
            } else {
                assert_eq!(result.unwrap().unwrap().len(), text.len());
            }
            steps
        };
        for hot in [false, true] {
            let complete = run(None, hot);
            for index in 0..complete.len() {
                assert_eq!(run(Some(index), hot), complete[..=index]);
            }
        }
    }
}

#[test]
fn context_beyond_kernel_limit_declines_before_pipeline() {
    let bytes = fixtures::font(65, 1, 1, &fixtures::records(65), 1, 0);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(
        &shaper,
        Direction::LeftToRight,
        Some(script::LATIN),
        None,
        &[],
    );
    let mut buffer = input(&"A".repeat(70), 0, 0);
    let mut pipeline = false;
    let result = shaper
        .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
            pipeline |= op == ShapeOperation::Recovery;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(result.is_none());
    assert!(!pipeline);
    assert_eq!(buffer.len(), 70);
    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 70);
}

#[test]
fn context_component_sum_checks_prior_ligature_before_any_shaping() {
    let words = |values: &[u16]| {
        values
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect::<Vec<_>>()
    };
    for count in [52, 53] {
        // Five-component ligatures before context: 51 * 5 fits u8,
        // whereas 52 * 5 does not. Context match_input discards this sum
        // but the ordinary kernel still computes it.
        let context_font = fixtures::font(count, 1, 1, &[(0, 2)], 1, 0);
        let directory_count = u16::from_be_bytes(context_font[4..6].try_into().unwrap()) as usize;
        let record = (0..directory_count)
            .map(|index| 12 + index * 16)
            .find(|&offset| &context_font[offset..offset + 4] == b"GSUB")
            .unwrap();
        let offset =
            u32::from_be_bytes(context_font[record + 8..record + 12].try_into().unwrap()) as usize;
        let length =
            u32::from_be_bytes(context_font[record + 12..record + 16].try_into().unwrap()) as usize;
        let context = &context_font[offset..offset + length];
        let first = ligature::layout(&[vec![vec![65; 4]]], 1, 1);
        let mut table = first[..40].to_vec();
        table.extend(words(&[2, 0, 1]));
        let list = table.len();
        table[8..10].copy_from_slice(&(list as u16).to_be_bytes());
        table.extend(words(&[3, 0, 0, 0]));
        for (index, (source, lookup_index)) in [(&first[..], 0), (context, 0), (context, 1)]
            .into_iter()
            .enumerate()
        {
            let old_list = u16::from_be_bytes(source[8..10].try_into().unwrap()) as usize;
            let old_start = old_list
                + u16::from_be_bytes(
                    source[old_list + 2 + lookup_index * 2..old_list + 4 + lookup_index * 2]
                        .try_into()
                        .unwrap(),
                ) as usize;
            let old_end = if lookup_index == 0 && source == context {
                old_list
                    + u16::from_be_bytes(source[old_list + 4..old_list + 6].try_into().unwrap())
                        as usize
            } else {
                source.len()
            };
            let field = list + 2 + index * 2;
            let new_offset = (table.len() - list) as u16;
            table[field..field + 2].copy_from_slice(&new_offset.to_be_bytes());
            table.extend_from_slice(&source[old_start..old_end]);
        }
        let bytes = single::font_with_layout(&table);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        let text = "A".repeat(usize::from(count) * 5);
        let mut buffer = input(&text, 0, 0);
        let mut pipeline = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                pipeline |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        if count == 52 {
            let actual = result.unwrap();
            let expected = shaper.shape_with_plan(&plan, input(&text, 0, 0), &[]);
            assert_eq!(actual.len(), usize::from(count));
            assert_eq!(
                format!("{:?}", actual.glyph_infos()),
                format!("{:?}", expected.glyph_infos())
            );
            assert_eq!(
                format!("{:?}", actual.glyph_positions()),
                format!("{:?}", expected.glyph_positions())
            );
        } else {
            assert!(result.is_none());
            assert!(!pipeline);
            assert_eq!(buffer.len(), text.len());
            buffer.clear();
            buffer.push_str("next");
            buffer.set_script(script::LATIN);
            buffer.set_direction(Direction::LeftToRight);
            assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
        }
    }
}
