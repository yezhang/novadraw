use harfrust::{
    BufferFlags, Direction, Feature, FontRef, ShapeOperation, ShapeOperationCost,
    ShapeOperationError, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::{error::Error, fmt, str::FromStr};
#[path = "shape_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shape rejection {}", self.0)
    }
}
impl Error for Reject {}

fn buffer(text: &str, reserve: usize) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(reserve));
    buffer.push_str(text);
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer
}

#[test]
fn nonempty_admitted_shaping_matches_glyphs_positions_clusters_and_flags() {
    let mut observations = 0;
    for bytes in fixtures::fonts() {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for text in ["", "a", "abc", "office ffi 123", "aabb"] {
            for reserve in [0, 128] {
                for flags in [
                    BufferFlags::empty(),
                    BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
                    BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL,
                ] {
                    let plan = ShapePlan::new(
                        &shaper,
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &[],
                    );
                    let mut ordinary = buffer(text, reserve);
                    ordinary.set_flags(flags);
                    let mut admitted = buffer(text, reserve);
                    admitted.set_flags(flags);
                    let result = shaper
                        .try_shape_with_plan_admitted(&plan, &mut admitted, &[], |_, cost| {
                            assert_eq!(cost.allocation, 0);
                            Ok::<_, Reject>(())
                        })
                        .unwrap();
                    if let Some(result) = result {
                        let expected = shaper.shape_with_plan(&plan, ordinary, &[]);
                        assert_eq!(
                            format!("{:?}", result.glyph_infos()),
                            format!("{:?}", expected.glyph_infos())
                        );
                        assert_eq!(
                            format!("{:?}", result.glyph_positions()),
                            format!("{:?}", expected.glyph_positions())
                        );
                        assert_eq!(result.len(), text.len());
                        assert!(admitted.is_empty());
                        if !text.is_empty() {
                            observations += 1;
                        }
                    } else {
                        assert_eq!(admitted.len(), text.len());
                    }
                }
            }
        }
    }
    assert!(observations >= 100, "must exercise actual nonempty shaping");
}

#[test]
fn every_rejection_retains_source_and_buffer_can_shape_normally() {
    let bytes = fixtures::fonts().pop().unwrap();
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
    let text = "office ffi 123";
    let mut complete = buffer(text, 256);
    let mut steps = Vec::new();
    let result = shaper
        .try_shape_with_plan_admitted(&plan, &mut complete, &[], |op, cost| {
            steps.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap()
        .unwrap();
    assert!(result.glyph_infos().iter().any(|info| info.glyph_id != 0));
    assert!(
        result
            .glyph_positions()
            .iter()
            .any(|pos| pos.x_advance != 0)
    );
    for reject in 0..steps.len() {
        let mut input = buffer(text, 256);
        let mut calls = 0;
        let error = shaper
            .try_shape_with_plan_admitted(&plan, &mut input, &[], |op, cost| {
                assert_eq!((op, cost), steps[calls]);
                let index = calls;
                calls += 1;
                if index == reject {
                    Err(Reject(index))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(calls, reject + 1);
        assert!(
            matches!(&error, ShapeOperationError::Admission(Reject(index)) if *index == reject)
        );
        assert_eq!(
            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
            reject
        );
        // Reinitialize caller settings and reuse storage after every failure.
        // A leaked variable slot would panic inside ordinary shape().
        input.clear();
        input.push_str("next");
        input.set_script(script::LATIN);
        input.set_direction(Direction::LeftToRight);
        assert_eq!(shaper.shape_with_plan(&plan, input, &[]).len(), 4);
    }
}

#[test]
fn unknown_paths_decline_before_mutating_input_and_mismatch_is_an_error() {
    let plain = fixtures::fonts().pop().unwrap();
    for bytes in [
        plain.as_slice(),
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        // Mapped letters under the explicitly supplied default/Latin shaper
        // are now supported. Marks still require a separate normalization slice.
        for text in ["a\u{301}", "中\u{301}", "ا\u{64e}", "🙂\u{301}"] {
            let mut input = buffer(text, 64);
            let plan = ShapePlan::new(
                &shaper,
                Direction::LeftToRight,
                Some(script::LATIN),
                None,
                &[],
            );
            assert!(
                shaper
                    .try_shape_with_plan_admitted(
                        &plan,
                        &mut input,
                        &[],
                        |_, _| Ok::<_, Reject>(())
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(input.len(), text.chars().count());
            // Decline must leave ordinary shaping usable on the original input.
            assert!(!shaper.shape_with_plan(&plan, input, &[]).is_empty());
        }
    }
    let font = FontRef::new(&plain).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(
        &shaper,
        Direction::RightToLeft,
        Some(script::LATIN),
        None,
        &[],
    );
    let mut input = buffer("abc", 0);
    assert!(matches!(
        shaper.try_shape_with_plan_admitted(&plan, &mut input, &[], |_, _| Ok::<_, Reject>(())),
        Err(ShapeOperationError::PlanMismatch)
    ));
    assert_eq!(input.len(), 3);
    let features = [Feature::from_str("liga[1:3]=0").unwrap()];
    let plan = ShapePlan::new(
        &shaper,
        Direction::LeftToRight,
        Some(script::LATIN),
        None,
        &features,
    );
    assert!(
        shaper
            .try_shape_with_plan_admitted(&plan, &mut input, &features, |_, _| Ok::<_, Reject>(()))
            .unwrap()
            .is_none()
    );
}

#[test]
fn complete_initialized_storage_and_repeated_mapping_are_charged() {
    let bytes = fixtures::fonts().pop().unwrap();
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
    let mut input = buffer("aaa", 256);
    let mut costs = Vec::new();
    shaper
        .try_shape_with_plan_admitted(&plan, &mut input, &[], |op, cost| {
            costs.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap()
        .unwrap();
    assert!(
        costs
            .iter()
            .any(|&(op, cost)| op == ShapeOperation::Positions && cost.work >= 256)
    );
    let mappings: Vec<_> = costs
        .iter()
        .filter(|(op, _)| *op == ShapeOperation::Mapping)
        .collect();
    assert_eq!(mappings.len(), 6); // 3 preflight + 3 actual normalization queries
    assert!(
        mappings
            .iter()
            .all(|(_, cost)| cost.work == mappings[0].1.work)
    );
    assert!(costs.iter().all(|(_, cost)| cost.allocation == 0));
}

#[test]
fn sticky_budget_cannot_be_bypassed_by_retry() {
    let bytes = fixtures::fonts().pop().unwrap();
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
    let mut remaining = 100;
    let mut failed = false;
    let mut policy = |_: ShapeOperation, cost: ShapeOperationCost| {
        if failed {
            return Err(Reject(1));
        }
        if cost.work > remaining {
            failed = true;
            return Err(Reject(0));
        }
        remaining -= cost.work;
        Ok(())
    };
    let mut input = buffer("abc", 0);
    assert!(
        shaper
            .try_shape_with_plan_admitted(&plan, &mut input, &[], &mut policy)
            .is_err()
    );
    let error = shaper
        .try_shape_with_plan_admitted(&plan, &mut input, &[], &mut policy)
        .unwrap_err();
    assert!(matches!(error, ShapeOperationError::Admission(Reject(1))));
}
