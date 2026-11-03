use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, FontRef, ShapeOperation, ShapeOperationError,
    ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::{error::Error, fmt};
#[path = "rtl_fixtures.rs"]
#[allow(clippy::duplicate_mod, clippy::module_inception)]
mod fixtures;

fn input(
    text: &str,
    direction: Direction,
    script: Option<harfrust::Script>,
    clusters: usize,
    level: BufferClusterLevel,
    spare: usize,
) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    for (i, ch) in text.chars().enumerate() {
        buffer.add(
            ch,
            match clusters {
                0 => i as u32,
                1 => 0,
                _ => (text.len() - i) as u32,
            },
        );
    }
    buffer.set_direction(direction);
    if let Some(script) = script {
        buffer.set_script(script);
    }
    buffer.set_cluster_level(level);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

#[test]
fn rtl_mirrors_and_reversals_match_full_ordinary_output() {
    for kind in 0..4 {
        for missing in [false, true] {
            for substitutions in [false, true] {
                let bytes = fixtures::font(kind, missing, substitutions);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for script in [
                    Some(script::LATIN),
                    Some(script::COMMON),
                    Some(script::UNKNOWN),
                    None,
                ] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                        for text in ["ABC(ABC[<>]", "(", "ABC", ""] {
                            for clusters in 0..3 {
                                for level in [
                                    BufferClusterLevel::MonotoneGraphemes,
                                    BufferClusterLevel::MonotoneCharacters,
                                    BufferClusterLevel::Characters,
                                ] {
                                    for spare in [0, 256] {
                                        let make = || {
                                            input(text, direction, script, clusters, level, spare)
                                        };
                                        let expected = shaper.shape_with_plan(&plan, make(), &[]);
                                        let actual = shaper
                                            .try_shape_with_plan_admitted(
                                                &plan,
                                                &mut make(),
                                                &[],
                                                |_, _| Ok::<_, ()>(()),
                                            )
                                            .unwrap()
                                            .expect("mapped ASCII horizontal");
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
fn every_rtl_rejection_keeps_source_prefix_and_reusable_buffer() {
    for script in [Some(script::LATIN), None] {
        for missing in [false, true] {
            let bytes = fixtures::font(3, missing, true);
            let run = |reject, hot| {
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                let make = || {
                    input(
                        "ABC(ABC",
                        Direction::RightToLeft,
                        script,
                        2,
                        BufferClusterLevel::MonotoneCharacters,
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
                    assert_eq!(steps.len(), index + 1);
                    assert_eq!(buffer.len(), if entered { 0 } else { 7 });
                    buffer.clear();
                    buffer.push_str("next(");
                    buffer.set_direction(Direction::RightToLeft);
                    if let Some(script) = script {
                        buffer.set_script(script);
                    }
                    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 5);
                } else {
                    assert!(result.unwrap().is_some());
                }
                steps
            };
            for hot in [false, true] {
                let complete = run(None, hot);
                for index in 0..complete.len() {
                    assert_eq!(run(Some(index), hot), complete[..=index]);
                }
                if hot {
                    assert!(complete.iter().all(|(_, cost)| cost.allocation == 0));
                }
            }
        }
    }
}

#[test]
fn rtl_complex_script_and_mark_input_decline_before_recovery() {
    let bytes = fixtures::font(0, true, false);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for (script, text) in [
        (Some(script::ARABIC), "ABC"),
        (None, ")\u{301}"),
        (None, "中文\u{301}"),
    ] {
        let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
        let mut buffer = input(
            text,
            Direction::RightToLeft,
            script,
            0,
            BufferClusterLevel::Characters,
            0,
        );
        let mut entered = false;
        assert!(
            shaper
                .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                    entered |= op == ShapeOperation::Recovery;
                    Ok::<_, ()>(())
                })
                .unwrap()
                .is_none()
        );
        assert!(!entered);
        assert_eq!(buffer.len(), text.chars().count());
    }
}

#[test]
fn mirror_presence_and_direction_passes_have_separate_charges() {
    for missing in [false, true] {
        let bytes = fixtures::font(0, missing, false);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for script in [Some(script::LATIN), None] {
            let run = |direction, text| {
                let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                let mut totals = [0usize; 4];
                let output = shaper
                    .try_shape_with_plan_admitted(
                        &plan,
                        &mut input(
                            text,
                            direction,
                            script,
                            0,
                            BufferClusterLevel::Characters,
                            0,
                        ),
                        &[],
                        |op, cost| {
                            let index = match op {
                                ShapeOperation::Unicode => Some(0),
                                ShapeOperation::Mapping => Some(1),
                                ShapeOperation::Information => Some(2),
                                ShapeOperation::Positions => Some(3),
                                _ => None,
                            };
                            if let Some(index) = index {
                                totals[index] += cost.work;
                            }
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap()
                    .unwrap();
                (output, totals)
            };
            assert_eq!(
                run(Direction::RightToLeft, "(").0.glyph_infos()[0].glyph_id,
                if missing { 40 } else { 41 }
            );
            let (_, ltr) = run(Direction::LeftToRight, "A(B");
            let (_, rtl) = run(Direction::RightToLeft, "A(B");
            assert_eq!(rtl[0] - ltr[0], 8 * 3);
            // Format12: one/two groups => one/two binary search comparisons,
            // plus two result and four cache/header visits per map call.
            assert_eq!(rtl[1] - ltr[1], 4 * if missing { 10 } else { 8 });
            assert_eq!(
                rtl[2] - ltr[2],
                if script.is_some() {
                    // Each adjacent continuation comparison pays two record
                    // references and its stored property read independently.
                    4 * 3 + 3 * 2
                } else {
                    4 * 3
                }
            );
            assert_eq!(rtl[3] - ltr[3], if script.is_some() { 0 } else { 2 * 3 });
        }
    }
}
