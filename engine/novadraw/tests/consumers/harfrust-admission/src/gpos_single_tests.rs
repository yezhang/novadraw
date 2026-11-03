use harfrust::{
    BufferFlags, Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData,
    UnicodeBuffer, script,
};
use std::{error::Error, fmt};
#[path = "gpos_single_fixtures.rs"]
mod fixtures;

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
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_script(script::LATIN);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

#[test]
fn scalar_single_positioning_matches_complete_output_after_substitution() {
    for format in [1, 2] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                for lookups in [1, 2] {
                    for values in [0, 1, 2, 4, 8, 15] {
                        for substitutions in [false, true] {
                            let bytes = fixtures::font(
                                format,
                                coverage,
                                subtables,
                                lookups,
                                values,
                                substitutions,
                            );
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
                            for text in ["ABCABC", "other", "A", ""] {
                                for spare in [0, 256] {
                                    for clusters in 0..3 {
                                        let expected = shaper.shape_with_plan(
                                            &plan,
                                            input(text, clusters, spare),
                                            &[],
                                        );
                                        let mut buffer = input(text, clusters, spare);
                                        let actual = shaper
                                            .try_shape_with_plan_admitted(
                                                &plan,
                                                &mut buffer,
                                                &[],
                                                |_, _| Ok::<_, ()>(()),
                                            )
                                            .unwrap()
                                            .expect("scalar positioning is supported");
                                        assert_eq!(
                                            format!("{:?}", actual.glyph_infos()),
                                            format!("{:?}", expected.glyph_infos())
                                        );
                                        assert_eq!(
                                            format!("{:?}", actual.glyph_positions()),
                                            format!("{:?}", expected.glyph_positions())
                                        );
                                        assert!(buffer.is_empty());
                                        if text == "ABCABC" && values == 15 {
                                            assert!(
                                                actual
                                                    .glyph_positions()
                                                    .iter()
                                                    .any(|p| p.x_offset != 0 && p.y_offset != 0)
                                            );
                                        }
                                        // Y_ADVANCE is ignored on a horizontal run.
                                        assert!(
                                            actual
                                                .glyph_positions()
                                                .iter()
                                                .all(|p| p.y_advance == 0)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rejected {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_positioning_rejection_retains_source_exact_prefix_and_reusable_input() {
    for substitutions in [false, true] {
        let bytes = fixtures::font(2, 2, 3, 2, 15, substitutions);
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
                shaper.shape_with_plan(&plan, input("ABCABC", 2, 256), &[]);
            }
            let mut buffer = input("ABCABC", 2, 256);
            let mut steps = Vec::new();
            let mut pipeline = false;
            let result =
                shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
                    let index = steps.len();
                    steps.push((op, cost));
                    if reject == Some(index) {
                        return Err(Reject(index));
                    }
                    pipeline |= op == ShapeOperation::Recovery;
                    Ok(())
                });
            if let Some(reject) = reject {
                let error = result.unwrap_err();
                assert!(
                    matches!(&error, ShapeOperationError::Admission(Reject(i)) if *i == reject)
                );
                assert_eq!(
                    error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                    reject
                );
                assert_eq!(steps.len(), reject + 1);
                assert_eq!(buffer.len(), if pipeline { 0 } else { 6 });
                buffer.clear();
                buffer.push_str("next");
                buffer.set_script(script::LATIN);
                buffer.set_direction(Direction::LeftToRight);
                assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
            } else {
                assert!(result.unwrap().is_some());
            }
            steps
        };
        for hot in [false, true] {
            let complete = run(None, hot);
            for reject in 0..complete.len() {
                assert_eq!(run(Some(reject), hot), complete[..=reject]);
            }
            if hot {
                assert!(complete.iter().all(|(_, c)| c.allocation == 0));
            }
        }
    }
}

#[test]
fn unsupported_positioning_declines_before_pipeline_and_preserves_input() {
    for case in 0..6 {
        let mut layout = fixtures::layout(
            1,
            1,
            1,
            1,
            match case {
                0 => 0x10,
                5 => 0x8000,
                _ => 15,
            },
        );
        let list = u16::from_be_bytes(layout[8..10].try_into().unwrap()) as usize;
        let lookup =
            list + u16::from_be_bytes(layout[list + 2..list + 4].try_into().unwrap()) as usize;
        if case == 1 {
            layout[lookup + 2..lookup + 4].copy_from_slice(&8u16.to_be_bytes());
        }
        if case == 3 {
            // Bare GDEF is now supported. Keep a GDEF-related negative
            // through the still-unsupported mark attachment lookup flag.
            layout[lookup + 2..lookup + 4].copy_from_slice(&0x100u16.to_be_bytes());
        }
        if case == 2 {
            layout[lookup..lookup + 2].copy_from_slice(&2u16.to_be_bytes());
            let subtable = lookup
                + u16::from_be_bytes(layout[lookup + 6..lookup + 8].try_into().unwrap()) as usize;
            layout.truncate(subtable);
            // A complete PairPosFormat1 with an unsupported device field.
            for word in [1u16, 18, 0x10, 0, 1, 12, 1, 66, 0, 1, 1, 65] {
                layout.extend(word.to_be_bytes());
            }
        }
        let bytes = fixtures::font_with_layout(&layout, false, case == 3);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let direction = if case == 4 {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };
        let plan = ShapePlan::new(&shaper, direction, Some(script::LATIN), None, &[]);
        // ASCII RTL is supported by the directional admission slice.
        // A missing singleton is supported; marks remain an unknown path.
        let mut buffer = input(if case == 4 { "A\u{301}B" } else { "ABC" }, 0, 0);
        buffer.set_direction(direction);
        let mut pipeline = false;
        assert!(
            shaper
                .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                    pipeline |= op == ShapeOperation::Recovery;
                    Ok::<_, ()>(())
                })
                .unwrap()
                .is_none(),
            "unsupported case {case}"
        );
        assert!(!pipeline);
        assert_eq!(buffer.len(), 3);
    }
}
