use harfrust::{
    Direction, FontRef, ShapeOperation, ShapeOperationError, ShapePlan, ShaperData, script,
};
use std::{error::Error, fmt};
#[path = "simple_unicode_fixtures.rs"]
#[allow(clippy::duplicate_mod, clippy::module_inception)]
mod fixtures;

#[test]
fn mapped_simple_unicode_matches_all_fields_and_four_cluster_levels() {
    for kind in 0..4 {
        for substitutions in [false, true] {
            for missing in [0, 2] {
                let bytes = fixtures::font(kind, 0, missing, substitutions);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for script in [
                    Some(script::LATIN),
                    Some(script::HAN),
                    Some(script::COMMON),
                    Some(script::UNKNOWN),
                    None,
                ] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                        for text in [
                            "A(éÉǅ中文𠀀\u{2003}",
                            "é",
                            "\u{00a0}\u{3000}\u{115f}\u{1160}\u{3164}\u{ffa0}",
                            "",
                        ] {
                            for clusters in 0..3 {
                                for level in fixtures::LEVELS {
                                    for spare in [0, 256] {
                                        let make = || {
                                            fixtures::input(
                                                text, direction, script, clusters, level, spare,
                                            )
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
                                            .expect("mapped simple scalar");
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
fn every_simple_unicode_rejection_preserves_source_exact_prefix_and_reuse() {
    for cmap in [0, 1, 2] {
        for script in [Some(script::HAN), None] {
            for level in fixtures::LEVELS {
                let bytes = fixtures::font(3, cmap, 0, true);
                let text = if cmap == 0 { "A(é中𠀀" } else { "A(é" };
                let run = |reject, hot| {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                    let make =
                        || fixtures::input(text, Direction::RightToLeft, script, 1, level, 256);
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
                        assert_eq!(buffer.len(), if entered { 0 } else { text.chars().count() });
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
                        buffer.push_str("nexté");
                        buffer.set_direction(Direction::RightToLeft);
                        if let Some(script) = script {
                            buffer.set_script(script);
                        }
                        let next = shaper.shape_with_plan(&plan, buffer, &[]);
                        let expected = shaper.shape_with_plan(
                            &plan,
                            fixtures::input("nexté", Direction::RightToLeft, script, 0, level, 0),
                            &[],
                        );
                        assert_eq!(
                            format!("{:?}", next.glyph_positions()),
                            format!("{:?}", expected.glyph_positions())
                        );
                        assert_eq!(next.len(), expected.len());
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
}

#[test]
fn mapped_complex_categories_and_missing_letters_decline_before_pipeline() {
    for (script, text, missing) in [
        (Some(script::LATIN), "a\u{301}", 0),
        (None, "\u{034f}", 0),
        (None, "\u{200d}", 0),
        (None, "\u{1f1e6}", 0),
        (None, "\u{1f3fb}", 0),
        (None, "\u{ff9e}", 0),
        (None, "\u{e0020}", 0),
        (None, "\u{2044}", 0),
        (Some(script::ARABIC), "中", 0),
        (Some(script::HANGUL), "中", 0),
        (None, "é\u{301}", 1),
    ] {
        let bytes = fixtures::font(0, 0, missing, false);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
        let mut buffer = fixtures::input(
            text,
            Direction::RightToLeft,
            script,
            0,
            fixtures::LEVELS[3],
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
fn each_bounded_charmap_matches_ordinary_non_ascii_mapping() {
    for cmap in 0..8 {
        let bytes = fixtures::font(0, cmap, 0, false);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
            let plan = ShapePlan::new(&shaper, direction, Some(script::LATIN), None, &[]);
            for text in ["éA", "É(", "\u{200e9}"] {
                let make = || {
                    fixtures::input(
                        text,
                        direction,
                        Some(script::LATIN),
                        0,
                        fixtures::LEVELS[3],
                        0,
                    )
                };
                let expected = shaper.shape_with_plan(&plan, make(), &[]);
                let actual = shaper
                    .try_shape_with_plan_admitted(&plan, &mut make(), &[], |_, _| Ok::<_, ()>(()))
                    .unwrap();
                let actual = actual.expect("mapped or missing singleton cmap path");
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

#[test]
fn scalar_property_and_macroman_scan_fees_are_separate_from_mapping() {
    let run = |cmap, text: &str| {
        let bytes = fixtures::font(0, cmap, 0, false);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
        let mut totals = [0; 4];
        let result = shaper
            .try_shape_with_plan_admitted(
                &plan,
                &mut fixtures::input(
                    text,
                    Direction::LeftToRight,
                    None,
                    0,
                    fixtures::LEVELS[0],
                    0,
                ),
                &[],
                |op, cost| {
                    let index = match op {
                        ShapeOperation::Unicode => Some(0),
                        ShapeOperation::Planning => Some(1),
                        ShapeOperation::Information => Some(2),
                        ShapeOperation::Mapping => Some(3),
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
        assert_eq!(result.len(), 1);
        totals
    };
    let ascii = run(0, "A");
    let unicode = run(0, "é");
    assert_eq!(unicode[0] - ascii[0], 6); // preflight GC+DI, pipeline DI
    assert_eq!(unicode[1] - ascii[1], 2); // preflight/pipeline category
    assert_eq!(unicode[2] - ascii[2], 1); // stored category
    assert_eq!(unicode[3], ascii[3]);
    let unicode0 = run(7, "é");
    let mac = run(1, "é");
    assert_eq!(mac[3] - unicode0[3], 2 * 128); // two independent map queries
    assert_eq!(run(2, "é")[3], 2 * 2 * 8); // two queries, each symbol fallback bounded twice
}

#[test]
fn unicode_rejection_wins_over_decline_and_stays_sticky_across_requests() {
    let bytes = fixtures::font(0, 0, 0, false);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    let mut failed = false;
    let mut spent = 0;
    let mut entered = false;
    let mut admit = |op, cost: harfrust::ShapeOperationCost| {
        entered |= op == ShapeOperation::Recovery;
        if failed || op == ShapeOperation::Unicode {
            failed = true;
            return Err(Reject(spent));
        }
        spent += cost.work + cost.allocation;
        Ok(())
    };
    let mut mark = fixtures::input(
        "\u{301}",
        Direction::LeftToRight,
        None,
        0,
        fixtures::LEVELS[3],
        0,
    );
    assert!(matches!(
        shaper.try_shape_with_plan_admitted(&plan, &mut mark, &[], &mut admit),
        Err(ShapeOperationError::Admission(_))
    ));
    // The second request cannot bypass a caller-owned failed budget.
    let mut letter = fixtures::input(
        "中",
        Direction::LeftToRight,
        None,
        0,
        fixtures::LEVELS[3],
        0,
    );
    assert!(matches!(
        shaper.try_shape_with_plan_admitted(&plan, &mut letter, &[], &mut admit),
        Err(ShapeOperationError::Admission(_))
    ));
    assert!(!entered);
    assert_eq!(mark.len(), 1);
    assert_eq!(letter.len(), 1);
    assert!(failed);
    assert!(spent > 0);
}
