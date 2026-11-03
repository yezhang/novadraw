#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "decomposition_miss_fixtures.rs"]
mod fixtures;
use harfrust::{
    Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData, script,
};
use std::{error::Error, fmt};

#[test]
fn terminal_b_miss_matches_all_fields_and_mapped_b_stays_unknown() {
    for kind in 0..8 {
        for case in [4, 8] {
            for mapping in 0..8 {
                let bytes = fixtures::font(kind, case, mapping);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for script in [Some(script::LATIN), Some(script::HAN), None] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                        for pass in 0..2 {
                            for text in fixtures::TEXTS {
                                for clusters in 0..3 {
                                    for level in fixtures::LEVELS {
                                        for spare in [0, 256] {
                                            let make = || {
                                                fixtures::input(
                                                    text, direction, script, clusters, level, spare,
                                                )
                                            };
                                            let ordinary =
                                                shaper.shape_with_plan(&plan, make(), &[]);
                                            let mut buffer = make();
                                            let mut entered = false;
                                            let actual = shaper
                                                .try_shape_with_plan_admitted(
                                                    &plan,
                                                    &mut buffer,
                                                    &[],
                                                    |op, _| {
                                                        entered |= op == ShapeOperation::Recovery;
                                                        Ok::<_, ()>(())
                                                    },
                                                )
                                                .unwrap();
                                            if mapping & 2 != 0 {
                                                assert!(actual.is_none());
                                                assert!(!entered);
                                                assert_eq!(buffer.len(), text.chars().count());
                                            } else {
                                                let actual = actual.expect("terminal b is missing");
                                                assert_eq!(
                                                    format!("{:?}", ordinary.glyph_infos()),
                                                    format!("{:?}", actual.glyph_infos()),
                                                    "{kind}/{case}/{mapping}/{script:?}/{direction:?}/{pass}/{text:?}"
                                                );
                                                assert_eq!(
                                                    format!("{:?}", ordinary.glyph_positions()),
                                                    format!("{:?}", actual.glyph_positions())
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
fn every_terminal_b_miss_rejection_preserves_source_prefix_and_recovery() {
    for kind in [0, 3, 6] {
        for mapping in [0, 1, 4, 5] {
            for hot in [false, true] {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let bytes = fixtures::font(kind, 8, mapping);
                    let run = |reject: Option<usize>| {
                        let font = FontRef::new(&bytes).unwrap();
                        let data = ShaperData::new(&font);
                        let shaper = data.shaper(&font).build();
                        let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                        let text = "ABéÅÅǻ\u{2007}\u{2011}CDEF";
                        let make =
                            || fixtures::input(text, direction, None, 2, fixtures::LEVELS[3], 256);
                        if hot {
                            shaper.shape_with_plan(&plan, make(), &[]);
                        }
                        let mut buffer = make();
                        let mut steps = Vec::new();
                        let mut entered = false;
                        let result = shaper.try_shape_with_plan_admitted(
                            &plan,
                            &mut buffer,
                            &[],
                            |op, cost| {
                                let index = steps.len();
                                steps.push((op, cost));
                                if reject == Some(index) {
                                    return Err(Reject(index));
                                }
                                entered |= op == ShapeOperation::Recovery;
                                Ok(())
                            },
                        );
                        if let Some(index) = reject {
                            let error = result.unwrap_err();
                            assert!(matches!(
                                &error, ShapeOperationError::Admission(Reject(i)) if *i == index
                            ));
                            assert_eq!(
                                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                                index
                            );
                            assert_eq!(
                                buffer.len(),
                                if entered { 0 } else { text.chars().count() }
                            );
                            if !entered {
                                let preserved = core::mem::replace(&mut buffer, make());
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
                            buffer.clear();
                            for (cluster, ch) in text.chars().enumerate() {
                                buffer.add(ch, cluster as u32);
                            }
                            buffer.set_direction(direction);
                            buffer.set_cluster_level(fixtures::LEVELS[3]);
                            buffer.set_flags(harfrust::BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                            let actual = shaper.shape_with_plan(&plan, buffer, &[]);
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
}

#[test]
fn terminal_maps_a_and_b_without_initializing_unemitted_unicode() {
    let bytes = fixtures::font(0, 0, 0);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    let run = |text| {
        let make = || {
            fixtures::input(
                text,
                Direction::LeftToRight,
                None,
                0,
                fixtures::LEVELS[0],
                256,
            )
        };
        shaper.shape_with_plan(&plan, make(), &[]);
        let (mut mapping, mut unicode, mut allocation, mut unit) = (0, 0, 0, None);
        let output = shaper
            .try_shape_with_plan_admitted(&plan, &mut make(), &[], |op, cost| {
                allocation += cost.allocation;
                if op == ShapeOperation::Mapping {
                    unit.get_or_insert(cost.work);
                    mapping += cost.work;
                }
                if op == ShapeOperation::Unicode {
                    unicode += cost.work;
                }
                Ok::<_, ()>(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(output.len(), text.chars().count());
        assert_eq!(allocation, 0);
        (mapping, unicode, unit.unwrap())
    };
    let absent = run("x\u{0378}y");
    for text in ["xéy", "xǻy"] {
        let terminal = run(text);
        assert_eq!(terminal.2, absent.2);
        // Independently map a and b in preflight and in the shared normalizer.
        assert_eq!(terminal.0 - absent.0, 4 * absent.2);
        assert_eq!(terminal.1, absent.1);
    }
}

#[test]
fn mapped_intermediate_stops_before_b_and_explicit_marks_stay_unknown() {
    for mapping in 0..8 {
        let bytes = fixtures::font(0, 0, mapping);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
        for text in ["Å", "é", "ǻ", "é\u{301}", "Å\u{fe0f}"] {
            let make = || {
                fixtures::input(
                    text,
                    Direction::LeftToRight,
                    None,
                    0,
                    fixtures::LEVELS[0],
                    0,
                )
            };
            let mut buffer = make();
            let mut entered = false;
            let result = shaper
                .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                    entered |= op == ShapeOperation::Recovery;
                    Ok::<_, ()>(())
                })
                .unwrap();
            let supported = match text {
                "Å" => mapping & 4 != 0 || mapping & 2 == 0,
                "é" | "ǻ" => mapping & 2 == 0,
                _ => false,
            };
            assert_eq!(result.is_some(), supported, "{mapping}/{text:?}");
            if let Some(result) = result {
                let ordinary = shaper.shape_with_plan(&plan, make(), &[]);
                assert_eq!(
                    format!("{:?}", result.glyph_infos()),
                    format!("{:?}", ordinary.glyph_infos())
                );
                assert_eq!(
                    format!("{:?}", result.glyph_positions()),
                    format!("{:?}", ordinary.glyph_positions())
                );
            } else {
                assert!(!entered);
                assert_eq!(buffer.len(), text.chars().count());
            }
        }
    }
}
