#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "gdef_fixtures.rs"]
mod fixtures;
use harfrust::{
    Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData, script,
};
use std::{error::Error, fmt};

fn same(actual: &harfrust::GlyphBuffer, expected: &harfrust::GlyphBuffer) {
    assert_eq!(
        format!("{:?}", actual.glyph_infos()),
        format!("{:?}", expected.glyph_infos())
    );
    assert_eq!(
        format!("{:?}", actual.glyph_positions()),
        format!("{:?}", expected.glyph_positions())
    );
}

#[test]
fn gdef_classes_match_full_results_across_lookup_combinations() {
    for kind in 0..8 {
        for case in 0..12 {
            let bytes = fixtures::font(kind, case);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            for script in [Some(script::LATIN), Some(script::HAN), None] {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                    for text in ["AAABCABC", "ABCAAA", "", "é中(ǅ\u{2003}"] {
                        for clusters in 0..3 {
                            for level in fixtures::LEVELS {
                                let make = || {
                                    fixtures::input(text, direction, script, clusters, level, 256)
                                };
                                let expected = shaper.shape_with_plan(&plan, make(), &[]);
                                let result = shaper
                                    .try_shape_with_plan_admitted(
                                        &plan,
                                        &mut make(),
                                        &[],
                                        |_, _| Ok::<_, ()>(()),
                                    )
                                    .unwrap();
                                // Missing singleton letters and spaces now
                                // use the same ordinary normalization result.
                                same(&result.expect("bounded GDEF classes"), &expected);
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
        write!(f, "reject {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_gdef_rejection_preserves_source_prefix_and_ordinary_recovery() {
    for kind in 0..8 {
        for case in [4, 6, 8, 10, 11] {
            for level in fixtures::LEVELS {
                let bytes = fixtures::font(kind, case);
                let run = |reject, hot| {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let direction = Direction::RightToLeft;
                    let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                    let make = || fixtures::input("AAABCABC", direction, None, 1, level, 256);
                    if hot {
                        shaper.shape_with_plan(&plan, make(), &[]);
                    }
                    let mut input = make();
                    let mut entered = false;
                    let mut steps = Vec::new();
                    let result =
                        shaper.try_shape_with_plan_admitted(&plan, &mut input, &[], |op, cost| {
                            let i = steps.len();
                            steps.push((op, cost));
                            if reject == Some(i) {
                                return Err(Reject(i));
                            }
                            entered |= op == ShapeOperation::Recovery;
                            Ok(())
                        });
                    if let Some(i) = reject {
                        let error = result.unwrap_err();
                        assert!(
                            matches!(&error, ShapeOperationError::Admission(Reject(j)) if *j == i)
                        );
                        assert_eq!(
                            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                            i
                        );
                        assert_eq!(input.len(), if entered { 0 } else { 8 });
                        if !entered {
                            same(
                                &shaper.shape_with_plan(
                                    &plan,
                                    core::mem::replace(&mut input, make()),
                                    &[],
                                ),
                                &shaper.shape_with_plan(&plan, make(), &[]),
                            );
                        }
                        input.clear();
                        input.push_str("ABC");
                        input.set_direction(direction);
                        input.set_cluster_level(level);
                        same(
                            &shaper.shape_with_plan(&plan, input, &[]),
                            &shaper.shape_with_plan(
                                &plan,
                                fixtures::input("ABC", direction, None, 0, level, 0),
                                &[],
                            ),
                        );
                    } else {
                        same(
                            &result.unwrap().unwrap(),
                            &shaper.shape_with_plan(&plan, make(), &[]),
                        );
                        assert!(
                            steps
                                .iter()
                                .any(|(op, _)| *op == ShapeOperation::GlyphProperties)
                        );
                    }
                    steps
                };
                for hot in [false, true] {
                    let complete = run(None, hot);
                    for i in 0..complete.len() {
                        assert_eq!(run(Some(i), hot), complete[..=i]);
                    }
                    // Format-3 matching arrays are local to each call and may
                    // spill even with hot lookup/GDEF caches. Property queries
                    // themselves always borrow storage.
                    assert!(
                        complete
                            .iter()
                            .all(|(op, cost)| *op != ShapeOperation::GlyphProperties
                                || cost.allocation == 0)
                    );
                }
            }
        }
    }
}

#[test]
fn actual_gdef_mark_classification_zeroes_widths_and_adjusts_offsets() {
    let run = |case, direction| {
        let bytes = fixtures::font(0, case);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
        let mut position_work = 0;
        let output = shaper
            .try_shape_with_plan_admitted(
                &plan,
                &mut fixtures::input("A", direction, None, 0, fixtures::LEVELS[0], 0),
                &[],
                |op, cost| {
                    if op == ShapeOperation::Positions {
                        position_work += cost.work;
                    }
                    Ok::<_, ()>(())
                },
            )
            .unwrap()
            .unwrap();
        (output, position_work)
    };
    let (base, _) = run(3, Direction::LeftToRight);
    assert_ne!(base.glyph_positions()[0].x_advance, 0);
    let (mark, forward) = run(4, Direction::LeftToRight);
    let (backward_mark, backward) = run(4, Direction::RightToLeft);
    assert_eq!(mark.glyph_positions()[0].x_advance, 0);
    assert_eq!(
        mark.glyph_positions()[0].x_offset,
        -base.glyph_positions()[0].x_advance
    );
    assert_eq!(backward_mark.glyph_positions()[0].x_advance, 0);
    assert_eq!(backward_mark.glyph_positions()[0].x_offset, 0);
    assert_eq!(forward - backward, 6);
}

#[test]
fn gdef_query_work_covers_cache_hits_initial_and_substituted_glyphs() {
    for kind in [0, 5, 6, 7] {
        let run = |case, hot| {
            let bytes = fixtures::font(kind, case);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
            let make = || {
                fixtures::input(
                    "AAABCABC",
                    Direction::LeftToRight,
                    None,
                    0,
                    fixtures::LEVELS[0],
                    256,
                )
            };
            if hot {
                shaper.shape_with_plan(&plan, make(), &[]);
            }
            let mut queries = Vec::new();
            shaper
                .try_shape_with_plan_admitted(&plan, &mut make(), &[], |op, cost| {
                    if op == ShapeOperation::GlyphProperties {
                        assert_eq!(cost.allocation, 0);
                        queries.push(cost.work);
                    }
                    Ok::<_, ()>(())
                })
                .unwrap()
                .unwrap();
            queries
        };
        assert_eq!(run(4, false), run(4, true));
        assert_eq!(run(8, false), run(8, true));
        let no_classes = run(0, true);
        assert_eq!(no_classes, vec![8 * 8]); // mark-attachment reader cannot run
        let indexed = run(4, true);
        assert_eq!(indexed[0], 8 * (8 + 4 + 4));
        assert!(indexed.len() > 1); // substitutions must query again
        let ranged = run(8, true);
        assert_eq!(ranged[0], 8 * (8 + (5 + 1 + 4) * 2));
    }
}

#[test]
fn nonzero_gdef_lookup_flags_decline_and_rejection_never_falls_back() {
    // Modify only the actual lookup flag field in a deterministic fixture.
    for flags in [1u16, 2, 4, 8, 0x100, 0x10] {
        let mut bytes = fixtures::font(7, 4);
        let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let record = bytes[12..12 + count * 16]
            .chunks_exact(16)
            .find(|r| &r[..4] == b"GSUB")
            .unwrap();
        let start = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
        let list =
            start + u16::from_be_bytes(bytes[start + 8..start + 10].try_into().unwrap()) as usize;
        let lookup =
            list + u16::from_be_bytes(bytes[list + 2..list + 4].try_into().unwrap()) as usize;
        bytes[lookup + 2..lookup + 4].copy_from_slice(&flags.to_be_bytes());
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
        let mut input = fixtures::input(
            "ABC",
            Direction::LeftToRight,
            None,
            0,
            fixtures::LEVELS[0],
            0,
        );
        let mut entered = false;
        assert!(
            shaper
                .try_shape_with_plan_admitted(&plan, &mut input, &[], |op, _| {
                    entered |= op == ShapeOperation::Recovery;
                    Ok::<_, ()>(())
                })
                .unwrap()
                .is_none()
        );
        assert!(!entered);
        assert_eq!(input.len(), 3);
    }
}
