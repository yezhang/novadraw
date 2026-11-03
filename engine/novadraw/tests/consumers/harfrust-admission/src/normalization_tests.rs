#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "normalization_fixtures.rs"]
mod fixtures;
use harfrust::{
    Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData, script,
};
use std::{error::Error, fmt};

#[test]
fn singleton_normalization_and_notdef_match_all_fields() {
    for kind in 0..8 {
        for case in [0, 4, 8, 10] {
            for mapping in 0..4 {
                let bytes = fixtures::font(kind, case, mapping);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for script in [Some(script::LATIN), Some(script::HAN), None] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                        for text in &fixtures::TEXTS[..if mapping == 3 { 4 } else { 3 }] {
                            for clusters in 0..3 {
                                for level in fixtures::LEVELS {
                                    for spare in [0, 256] {
                                        let make = || {
                                            fixtures::input(
                                                text, direction, script, clusters, level, spare,
                                            )
                                        };
                                        let ordinary = shaper.shape_with_plan(&plan, make(), &[]);
                                        let admitted = shaper
                                            .try_shape_with_plan_admitted(
                                                &plan,
                                                &mut make(),
                                                &[],
                                                |_, _| Ok::<_, ()>(()),
                                            )
                                            .unwrap()
                                            .expect("proven singleton");
                                        assert_eq!(
                                            format!("{:?}", admitted.glyph_infos()),
                                            format!("{:?}", ordinary.glyph_infos())
                                        );
                                        assert_eq!(
                                            format!("{:?}", admitted.glyph_positions()),
                                            format!("{:?}", ordinary.glyph_positions())
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
        write!(f, "reject {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_normalization_rejection_preserves_source_prefix_and_recovery() {
    for kind in [0, 3, 6] {
        for hot in [false, true] {
            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                let bytes = fixtures::font(kind, 8, 0);
                let run = |reject: Option<usize>| {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                    let text = "ABKΩ\u{f900}。C";
                    let make =
                        || fixtures::input(text, direction, None, 2, fixtures::LEVELS[3], 256);
                    if hot {
                        shaper.shape_with_plan(&plan, make(), &[]);
                    }
                    let mut input = make();
                    let mut steps = Vec::new();
                    let mut entered = false;
                    let result =
                        shaper.try_shape_with_plan_admitted(&plan, &mut input, &[], |op, cost| {
                            let index = steps.len();
                            steps.push((op, cost));
                            if reject == Some(index) {
                                return Err(Reject(index));
                            }
                            entered |= op == ShapeOperation::Recovery;
                            Ok(())
                        });
                    if let Some(index) = reject {
                        let error = result.unwrap_err();
                        assert!(
                            matches!(&error, ShapeOperationError::Admission(Reject(i)) if *i == index)
                        );
                        assert_eq!(
                            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                            index
                        );
                        assert_eq!(input.len(), if entered { 0 } else { text.chars().count() });
                        if !entered {
                            let preserved = core::mem::replace(&mut input, make());
                            let actual = shaper.shape_with_plan(&plan, preserved, &[]);
                            let expected = shaper.shape_with_plan(&plan, make(), &[]);
                            assert_eq!(
                                format!("{:?}", actual.glyph_infos()),
                                format!("{:?}", expected.glyph_infos())
                            );
                            assert_eq!(
                                format!("{:?}", actual.glyph_positions()),
                                format!("{:?}", expected.glyph_positions())
                            );
                        }
                        input.clear();
                        for (cluster, ch) in text.chars().enumerate() {
                            input.add(ch, cluster as u32);
                        }
                        input.set_direction(direction);
                        input.set_cluster_level(fixtures::LEVELS[3]);
                        input.set_flags(harfrust::BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                        let actual = shaper.shape_with_plan(&plan, input, &[]);
                        let expected = shaper.shape_with_plan(
                            &plan,
                            fixtures::input(text, direction, None, 0, fixtures::LEVELS[3], 0),
                            &[],
                        );
                        assert_eq!(
                            format!("{:?}", actual.glyph_infos()),
                            format!("{:?}", expected.glyph_infos())
                        );
                        assert_eq!(
                            format!("{:?}", actual.glyph_positions()),
                            format!("{:?}", expected.glyph_positions())
                        );
                    } else {
                        assert!(result.unwrap().is_some());
                    }
                    steps
                };
                let full = run(None);
                for index in 0..full.len() {
                    assert_eq!(run(Some(index)), full[..=index]);
                }
            }
        }
    }
}

#[test]
fn two_character_decomposition_and_unknown_fallback_decline_before_recovery() {
    let bytes = fixtures::font(0, 0, 0);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    for text in ["Å\u{301}", "é\u{fe0f}", "\u{301}", "\u{200d}"] {
        let mut input = fixtures::input(
            text,
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
                .is_none(),
            "{text:?}"
        );
        assert!(!entered);
        assert_eq!(input.len(), text.chars().count());
    }
}

#[test]
fn singleton_output_switch_uses_existing_capacity_and_separate_fees() {
    let bytes = fixtures::font(0, 0, 0);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    // Isolate buffer work from independent lazy lookup cache allocation.
    shaper.shape_with_plan(
        &plan,
        fixtures::input(
            "xyzKuvw",
            Direction::LeftToRight,
            None,
            0,
            fixtures::LEVELS[0],
            256,
        ),
        &[],
    );
    let mut sums = [0usize; 3];
    let output = shaper
        .try_shape_with_plan_admitted(
            &plan,
            &mut fixtures::input(
                "xyzKuvw",
                Direction::LeftToRight,
                None,
                0,
                fixtures::LEVELS[0],
                256,
            ),
            &[],
            |op, cost| {
                sums[0] += cost.allocation;
                if op == ShapeOperation::Unicode {
                    sums[1] += cost.work;
                }
                if op == ShapeOperation::Mapping {
                    sums[2] += cost.work;
                }
                Ok::<_, ()>(())
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(output.len(), 7);
    assert_eq!(sums[0], 0);
    assert!(sums[1] >= 7 * 4 + 4 + 4 + 5);
    assert!(sums[2] > 7 * 2);
}
