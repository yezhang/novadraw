use harfrust::{
    BufferFlags, ColdOperation, Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan,
    ShaperData, UnicodeBuffer, script,
};
#[path = "single_fixtures.rs"]
mod fixtures;

fn input(text: &str, spare: usize) -> UnicodeBuffer {
    let mut input = UnicodeBuffer::new();
    assert!(input.reserve(spare));
    input.push_str(text);
    input.set_script(script::LATIN);
    input.set_direction(Direction::LeftToRight);
    input.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    input
}

#[test]
fn single_substitutions_cover_both_formats_misses_and_lookup_chains() {
    for bytes in fixtures::fonts() {
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
        for pass in 0..2 {
            for text in ["ABCAB", "other", "A", ""] {
                for spare in [0, 128] {
                    let mut buffer = input(text, spare);
                    let result = shaper
                        .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |_, _| {
                            Ok::<_, ()>(())
                        })
                        .unwrap()
                        .expect("all structural single cases are admitted");
                    let expected = shaper.shape_with_plan(&plan, input(text, spare), &[]);
                    assert_eq!(
                        format!("{:?}", result.glyph_infos()),
                        format!("{:?}", expected.glyph_infos())
                    );
                    assert_eq!(
                        format!("{:?}", result.glyph_positions()),
                        format!("{:?}", expected.glyph_positions())
                    );
                    assert!(buffer.is_empty());
                    if pass == 0 && text == "ABCAB" {
                        assert!(
                            result
                                .glyph_infos()
                                .iter()
                                .zip(text.bytes())
                                .any(|(g, c)| g.glyph_id != u32::from(c))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_cold_or_hot_rejection_stops_and_retains_reusable_buffer() {
    let bytes = fixtures::font_with_layout(&fixtures::layout(2, 2, 3, 2));
    let run = |reject: Option<usize>, warm: bool| {
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
        if warm {
            let mut buffer = input("ABCAB", 128);
            shaper
                .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |_, _| Ok::<_, usize>(()))
                .unwrap()
                .unwrap();
        }
        let mut buffer = input("ABCAB", 128);
        let mut steps = Vec::new();
        let result = shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
            let index = steps.len();
            steps.push((op, cost));
            if reject == Some(index) {
                Err(index)
            } else {
                Ok(())
            }
        });
        if let Some(reject) = reject {
            assert!(matches!(result, Err(ShapeOperationError::Admission(i)) if i == reject));
            assert_eq!(steps.len(), reject + 1);
            buffer.clear();
            buffer.push_str("next");
            buffer.set_script(script::LATIN);
            buffer.set_direction(Direction::LeftToRight);
            assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
        } else {
            assert_eq!(result.unwrap().unwrap().len(), 5);
        }
        steps
    };
    for warm in [false, true] {
        let complete = run(None, warm);
        assert!(
            complete
                .iter()
                .any(|(op, _)| *op == ShapeOperation::TableRead)
        );
        for reject in 0..complete.len() {
            assert_eq!(run(Some(reject), warm), complete[..=reject]);
        }
        if warm {
            assert!(complete.iter().all(|(_, cost)| cost.allocation == 0));
        } else if !cfg!(feature = "portable") {
            assert!(complete.iter().any(|(op, cost)| *op
                == ShapeOperation::Cold(ColdOperation::Storage)
                && cost.allocation > 0));
        }
    }
}

#[test]
fn unsupported_lookup_declines_before_any_actual_lookup_operation() {
    for kind in [2, 3, 4, 5, 6, 8] {
        let mut table = fixtures::layout(1, 1, 1, 1);
        let list = u16::from_be_bytes(table[8..10].try_into().unwrap()) as usize;
        let lookup =
            list + u16::from_be_bytes(table[list + 2..list + 4].try_into().unwrap()) as usize;
        table[lookup..lookup + 2].copy_from_slice(&(kind as u16).to_be_bytes());
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
        let mut actual = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                actual |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        // Malformed unsupported tables can be omitted by the upstream cache.
        // If a real cached lookup remains, it must decline before shaping.
        if result.is_none() {
            assert!(!actual);
            assert_eq!(buffer.len(), 3);
        }
    }
}
