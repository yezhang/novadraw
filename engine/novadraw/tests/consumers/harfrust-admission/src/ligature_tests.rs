use harfrust::{
    BufferFlags, BufferOperation, Direction, FontRef, ShapeOperation, ShapeOperationError,
    ShapePlan, ShaperData, UnicodeBuffer, script,
};
#[path = "ligature_fixtures.rs"]
mod fixtures;

fn input(text: &str, spare: usize, clusters: usize) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    for (index, character) in text.chars().enumerate() {
        buffer.add(
            character,
            if clusters == 0 {
                index as u32
            } else if clusters == 1 {
                0
            } else {
                (text.len() - index) as u32
            },
        );
    }
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

#[test]
fn ligature_matching_spill_alternatives_clusters_and_nested_outputs_match() {
    for count in [1, 2, 8, 9, 17, 64, 65] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                let bytes = fixtures::font(&[fixtures::alternatives(count)], coverage, subtables);
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
                        shaper.shape_with_plan(&plan, input("AB", 0, 0), &[]);
                    }
                    for text in ["A".repeat(count + 1), "ABAZ other".into(), "".into()] {
                        for clusters in 0..3 {
                            for spare in [0, 256] {
                                let expected = shaper.shape_with_plan(
                                    &plan,
                                    input(&text, spare, clusters),
                                    &[],
                                );
                                let actual = shaper
                                    .try_shape_with_plan_admitted(
                                        &plan,
                                        &mut input(&text, spare, clusters),
                                        &[],
                                        |_, _| Ok::<_, ()>(()),
                                    )
                                    .unwrap()
                                    .unwrap();
                                assert_eq!(
                                    format!("{:?}", actual.glyph_infos()),
                                    format!("{:?}", expected.glyph_infos())
                                );
                                assert_eq!(
                                    format!("{:?}", actual.glyph_positions()),
                                    format!("{:?}", expected.glyph_positions())
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_ligature_rejection_has_exact_prefix_and_reusable_buffer() {
    let bytes = fixtures::font(
        &[fixtures::alternatives(9), fixtures::alternatives(3)],
        2,
        3,
    );
    let run = |reject: Option<usize>, hot: bool| {
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
            shaper.shape_with_plan(&plan, input("AAAAAAAAAAAAAB", 0, 0), &[]);
        }
        let mut buffer = input("AAAAAAAAAAAAAB", 0, 0);
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
            assert_eq!(buffer.len(), if pipeline { 0 } else { 14 });
            buffer.clear();
            buffer.push_str("next");
            buffer.set_direction(Direction::LeftToRight);
            buffer.set_script(script::LATIN);
            assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
        } else {
            assert!(result.unwrap().unwrap().len() < 14);
        }
        steps
    };
    for hot in [false, true] {
        let complete = run(None, hot);
        assert!(complete.iter().any(|(op, cost)| *op
            == ShapeOperation::Matching(BufferOperation::Storage)
            && cost.allocation == 9));
        for reject in 0..complete.len() {
            assert_eq!(run(Some(reject), hot), complete[..=reject]);
        }
    }
}

#[test]
fn nested_component_overflow_declines_before_pipeline() {
    // First lookup may create 15-component ligatures; the next can sum 18.
    let bytes = fixtures::font(&[vec![vec![65; 14]], vec![vec![65; 17]]], 1, 1);
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
    let mut buffer = input(&"A".repeat(270), 0, 0);
    let mut pipeline = false;
    let result = shaper
        .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
            pipeline |= op == ShapeOperation::Recovery;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(result.is_none());
    assert!(!pipeline);
    assert_eq!(buffer.len(), 270);
    buffer.clear();
    buffer.push_str("next");
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_script(script::LATIN);
    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
}

#[test]
fn ligature_multiple_ligature_preserves_component_properties() {
    let bytes = fixtures::mixed_font();
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
    for text in ["AAAAAAAB", "AB", "AAAZA other"] {
        let actual =
            shaper
                .try_shape_with_plan_admitted(&plan, &mut input(text, 0, 0), &[], |_, _| {
                    Ok::<_, ()>(())
                })
                .unwrap()
                .unwrap();
        let expected = shaper.shape_with_plan(&plan, input(text, 0, 0), &[]);
        assert_eq!(
            format!("{:?}", actual.glyph_infos()),
            format!("{:?}", expected.glyph_infos())
        );
        assert_eq!(
            format!("{:?}", actual.glyph_positions()),
            format!("{:?}", expected.glyph_positions())
        );
    }
}

#[test]
fn matching_storage_rejection_preserves_source_and_sticky_budget() {
    use std::{error::Error, fmt};
    #[derive(Debug)]
    struct Reject;
    impl fmt::Display for Reject {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("matching storage rejected")
        }
    }
    impl Error for Reject {}
    let bytes = fixtures::font(&[fixtures::alternatives(9)], 1, 1);
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
    let mut policy = |op, _| {
        if failed || op == ShapeOperation::Matching(BufferOperation::Storage) {
            failed = true;
            Err(Reject)
        } else {
            Ok(())
        }
    };
    let mut buffer = input("AAAAAAAAAA", 0, 0);
    let error = shaper
        .try_shape_with_plan_admitted(&plan, &mut buffer, &[], &mut policy)
        .unwrap_err();
    assert!(error.source().unwrap().downcast_ref::<Reject>().is_some());
    assert!(buffer.is_empty());
    buffer = input("next", 0, 0);
    assert!(
        shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], &mut policy)
            .is_err()
    );
    assert_eq!(buffer.len(), 4);
    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
}
