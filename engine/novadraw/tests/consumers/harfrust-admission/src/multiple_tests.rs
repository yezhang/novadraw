use harfrust::{
    BufferFlags, Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData,
    UnicodeBuffer, script,
};
#[path = "multiple_fixtures.rs"]
mod fixtures;

fn input(text: &str, spare: usize) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    buffer.push_str(text);
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

#[test]
fn multiple_sequences_share_original_kernel_with_growth_misses_and_chains() {
    for max in [2, 3, 17] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                for lookups in [1, 2] {
                    let bytes = fixtures::font_with_layout(&fixtures::layout(
                        &fixtures::sequences(max),
                        coverage,
                        subtables,
                        lookups,
                    ));
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
                    for hot in [false, true] {
                        if hot {
                            shaper.shape_with_plan(&plan, input("ABC", 0), &[]);
                        }
                        for text in ["ABCAB other", "B", "other", ""] {
                            for spare in [0, 4096] {
                                let mut buffer = input(text, spare);
                                let actual = shaper
                                    .try_shape_with_plan_admitted(
                                        &plan,
                                        &mut buffer,
                                        &[],
                                        |_, _| Ok::<_, ()>(()),
                                    )
                                    .unwrap()
                                    .expect("positive sequence lengths are admitted");
                                let expected =
                                    shaper.shape_with_plan(&plan, input(text, spare), &[]);
                                assert_eq!(
                                    format!("{:?}", actual.glyph_infos()),
                                    format!("{:?}", expected.glyph_infos())
                                );
                                assert_eq!(
                                    format!("{:?}", actual.glyph_positions()),
                                    format!("{:?}", expected.glyph_positions())
                                );
                                if text == "B" {
                                    assert!(actual.len() > text.len());
                                }
                                assert!(buffer.is_empty());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_multiple_rejection_stops_at_exact_prefix_and_allows_ordinary_reuse() {
    let bytes = fixtures::font_with_layout(&fixtures::layout(&fixtures::sequences(3), 2, 3, 2));
    let run = |reject: Option<usize>, hot: bool, spare: usize| {
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
            shaper.shape_with_plan(&plan, input("ABC", 0), &[]);
        }
        let mut buffer = input("ABC other", spare);
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
            assert!(matches!(result, Err(ShapeOperationError::Admission(i)) if i == reject));
            assert_eq!(steps.len(), reject + 1);
            assert_eq!(buffer.len(), if pipeline { 0 } else { 9 });
            buffer.clear();
            buffer.push_str("next");
            buffer.set_script(script::LATIN);
            buffer.set_direction(Direction::LeftToRight);
            assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
        } else {
            assert!(result.unwrap().unwrap().len() > 9);
        }
        steps
    };
    for hot in [false, true] {
        for spare in [0, 128] {
            let complete = run(None, hot, spare);
            assert!(
                complete
                    .iter()
                    .any(|(op, _)| *op == ShapeOperation::TableRead)
            );
            if spare == 0 {
                assert!(complete.iter().any(|(op, cost)| *op
                    == ShapeOperation::Buffer(harfrust::BufferOperation::Storage)
                    && cost.allocation > 0));
            } else {
                assert!(
                    complete
                        .iter()
                        .filter(|(op, _)| matches!(op, ShapeOperation::Buffer(_)))
                        .all(|(_, cost)| cost.allocation == 0)
                );
            }
            for reject in 0..complete.len() {
                assert_eq!(run(Some(reject), hot, spare), complete[..=reject]);
            }
        }
    }
}

#[test]
fn deletion_and_excessive_growth_decline_before_pipeline_preserving_input() {
    for (max, lookups) in [(0, 1), (17, 4)] {
        let bytes =
            fixtures::font_with_layout(&fixtures::layout(&fixtures::sequences(max), 1, 1, lookups));
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
        let mut buffer = input("ABC", 0);
        let mut pipeline = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                pipeline |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(result.is_none());
        assert!(!pipeline);
        assert_eq!(buffer.len(), 3);
        assert_eq!(
            shaper.shape_with_plan(&plan, input("other", 0), &[]).len(),
            5
        );
    }
}

#[test]
fn buffer_growth_rejection_retains_source_and_sticky_policy_stops_retry() {
    use std::{error::Error, fmt};
    #[derive(Debug)]
    struct Reject;
    impl fmt::Display for Reject {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("growth rejected")
        }
    }
    impl Error for Reject {}
    let bytes = fixtures::font_with_layout(&fixtures::layout(&fixtures::sequences(3), 1, 1, 1));
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
    let mut admitted = 0;
    let mut policy = |op, _| {
        if failed {
            return Err(Reject);
        }
        if op == ShapeOperation::Buffer(harfrust::BufferOperation::Storage) {
            failed = true;
            return Err(Reject);
        }
        admitted += 1;
        Ok(())
    };
    let mut buffer = input("ABC", 0);
    let error = shaper
        .try_shape_with_plan_admitted(&plan, &mut buffer, &[], &mut policy)
        .unwrap_err();
    assert!(matches!(error, ShapeOperationError::Admission(Reject)));
    assert!(error.source().unwrap().downcast_ref::<Reject>().is_some());
    assert!(buffer.is_empty());
    buffer.push_str("next");
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    assert!(
        shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], &mut policy)
            .is_err()
    );
    assert!(failed);
    assert!(admitted > 0);
    assert_eq!(buffer.len(), 4);
    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
}

#[test]
fn malformed_sequence_is_skipped_and_lookup_flags_decline_before_pipeline() {
    for flags in [0u16, 1, 2, 4, 8] {
        let mut table = fixtures::layout(&fixtures::sequences(3), 1, 1, 1);
        let list = u16::from_be_bytes(table[8..10].try_into().unwrap()) as usize;
        let lookup =
            list + u16::from_be_bytes(table[list + 2..list + 4].try_into().unwrap()) as usize;
        table[lookup + 2..lookup + 4].copy_from_slice(&flags.to_be_bytes());
        let subtable =
            lookup + u16::from_be_bytes(table[lookup + 6..lookup + 8].try_into().unwrap()) as usize;
        // Last sequence offset exceeds the borrowed subtable; upstream skips it.
        table[subtable + 10..subtable + 12].copy_from_slice(&u16::MAX.to_be_bytes());
        let bytes = fixtures::font_with_layout(&table);
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
        let mut buffer = input("ABC", 0);
        let mut pipeline = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                pipeline |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        if flags == 0 {
            assert!(pipeline);
            let actual = result.unwrap();
            let expected = shaper.shape_with_plan(&plan, input("ABC", 0), &[]);
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
            assert_eq!(buffer.len(), 3);
        }
    }
}
