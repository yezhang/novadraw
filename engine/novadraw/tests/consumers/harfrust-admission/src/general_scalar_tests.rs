#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "general_scalar_fixtures.rs"]
mod fixtures;
use harfrust::{
    Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData, script,
};
use std::{error::Error, fmt};

#[test]
fn mapped_singletons_match_every_field_across_all_existing_kernels() {
    for kind in 0..8 {
        for case in [0, 4, 8, 10] {
            let bytes = fixtures::font(kind, case, false);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            for script in [Some(script::LATIN), Some(script::HAN), None] {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                    for text in &fixtures::TEXTS[..3] {
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

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reject {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_new_rejection_preserves_exact_prefix_source_and_ordinary_recovery() {
    for kind in [0, 3, 6] {
        for hot in [false, true] {
            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                let bytes = fixtures::font(kind, 8, false);
                let run = |reject: Option<usize>| {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(&shaper, direction, Some(script::HAN), None, &[]);
                    let text = "A，🙂≤∉";
                    let make = || {
                        fixtures::input(
                            text,
                            direction,
                            Some(script::HAN),
                            2,
                            fixtures::LEVELS[3],
                            256,
                        )
                    };
                    if hot {
                        shaper.shape_with_plan(&plan, make(), &[]);
                    }
                    let mut buffer = make();
                    let mut steps = Vec::new();
                    let mut entered = false;
                    let result =
                        shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
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
                        assert_eq!(buffer.len(), if entered { 0 } else { text.chars().count() });
                        if !entered {
                            let preserved = core::mem::replace(&mut buffer, make());
                            let output = shaper.shape_with_plan(&plan, preserved, &[]);
                            let expected = shaper.shape_with_plan(&plan, make(), &[]);
                            assert_eq!(
                                format!("{:?}", output.glyph_infos()),
                                format!("{:?}", expected.glyph_infos())
                            );
                            assert_eq!(
                                format!("{:?}", output.glyph_positions()),
                                format!("{:?}", expected.glyph_positions())
                            );
                        }
                        buffer.clear();
                        buffer.push_str(text);
                        buffer.set_direction(direction);
                        buffer.set_script(script::HAN);
                        let output = shaper.shape_with_plan(&plan, buffer, &[]);
                        let expected = shaper.shape_with_plan(
                            &plan,
                            fixtures::input(
                                text,
                                direction,
                                Some(script::HAN),
                                0,
                                fixtures::LEVELS[3],
                                0,
                            ),
                            &[],
                        );
                        assert_eq!(
                            format!("{:?}", output.glyph_positions()),
                            format!("{:?}", expected.glyph_positions())
                        );
                        assert_eq!(output.len(), expected.len());
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
fn special_sequences_decline_before_recovery_and_missing_mirrors_match() {
    let bytes = fixtures::font(0, 0, true);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::RightToLeft, None, None, &[]);
    for text in [
        "\u{301}",
        "\u{200d}",
        "\u{1f1e6}",
        "\u{1f3fb}",
        "\u{ff9e}",
        "\u{2044}",
    ] {
        let mut buffer = fixtures::input(
            text,
            Direction::RightToLeft,
            None,
            0,
            fixtures::LEVELS[0],
            0,
        );
        let mut entered = false;
        let result = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                entered |= op == ShapeOperation::Recovery;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(result.is_none(), "{text:?}");
        assert!(!entered);
        assert_eq!(buffer.len(), text.chars().count());
    }
    // A missing mirror retains the mapped original and uses ordinary rtlm.
    let make = || {
        fixtures::input(
            "（∉",
            Direction::RightToLeft,
            None,
            0,
            fixtures::LEVELS[0],
            0,
        )
    };
    let expected = shaper.shape_with_plan(&plan, make(), &[]);
    let actual = shaper
        .try_shape_with_plan_admitted(&plan, &mut make(), &[], |_, _| Ok::<_, ()>(()))
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

#[test]
fn special_branch_fees_are_additive_without_unicode_table_or_allocation_work() {
    let bytes = fixtures::font(0, 0, false);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    // Prime lazy lookup storage before comparing two property-only deltas.
    shaper.shape_with_plan(
        &plan,
        fixtures::input(
            "ǆ。",
            Direction::LeftToRight,
            None,
            0,
            fixtures::LEVELS[0],
            256,
        ),
        &[],
    );
    let totals = |text| {
        let mut sums = [0; 4];
        shaper
            .try_shape_with_plan_admitted(
                &plan,
                &mut fixtures::input(
                    text,
                    Direction::LeftToRight,
                    None,
                    0,
                    fixtures::LEVELS[0],
                    256,
                ),
                &[],
                |op, cost| {
                    match op {
                        ShapeOperation::Unicode => sums[0] += cost.work,
                        ShapeOperation::Information => sums[1] += cost.work,
                        ShapeOperation::Planning => sums[2] += cost.work,
                        _ => {}
                    }
                    sums[3] += cost.allocation;
                    Ok::<_, ()>(())
                },
            )
            .unwrap()
            .unwrap();
        sums
    };
    let early = totals("ǆ");
    // Both map to glyph 67, isolating properties from lookup output growth.
    let general = totals("。");
    assert_eq!(general[0], early[0]);
    assert_eq!(general[1] - early[1], 5);
    assert_eq!(general[2] - early[2], 10);
    assert_eq!(general[3], early[3]);
}
