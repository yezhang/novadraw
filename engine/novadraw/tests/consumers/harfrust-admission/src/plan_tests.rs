//! Plan admission is separate from shaping admission.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use harfrust::{
    ColdOperation, ColdOperationError, Direction, Feature, FontRef, Language, ShapePlan,
    ShapePlanKey, ShaperData, UnicodeBuffer, script,
};
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "plan rejection {}", self.0)
    }
}
impl Error for Reject {}

fn features() -> Vec<Feature> {
    [
        "kern=0",
        "trak=0",
        "kern=1",
        "liga[1:4]=0",
        "abcd=4",
        "rand=2",
    ]
    .iter()
    .map(|s| Feature::from_str(s).unwrap())
    .collect()
}

#[test]
fn supported_plans_match_keys_and_ordinary_shaping_observations() {
    let bytes = fixtures::sfnt(&[], true);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for direction in [
        Direction::LeftToRight,
        Direction::RightToLeft,
        Direction::TopToBottom,
        Direction::BottomToTop,
    ] {
        for script in [
            None,
            Some(script::LATIN),
            Some(script::HAN),
            Some(script::HEBREW),
        ] {
            for features in [vec![], features(), features().repeat(12)] {
                let ordinary = ShapePlan::new(&shaper, direction, script, None, &features);
                let mut trace = Vec::new();
                let admitted = ShapePlan::try_new_admitted(
                    &shaper,
                    direction,
                    script,
                    None,
                    &features,
                    |op, cost| {
                        trace.push((op, cost));
                        Ok::<_, Reject>(())
                    },
                )
                .unwrap()
                .unwrap();
                assert!(
                    ShapePlanKey::new(script, direction)
                        .features(&features)
                        .matches(&admitted)
                );
                assert_eq!(ordinary.direction(), admitted.direction());
                assert_eq!(ordinary.script(), admitted.script());
                assert_eq!(ordinary.language(), admitted.language());
                assert!(
                    trace
                        .iter()
                        .any(|(op, cost)| { *op == ColdOperation::Storage && cost.allocation > 0 })
                );
                if !features.is_empty() {
                    assert!(trace.iter().any(|(op, _)| *op == ColdOperation::Sort));
                }
                for text in ["", "office ffi", "a\u{301} é", "中文🙂", "אבג"] {
                    let shape = |plan: &ShapePlan| {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.set_direction(direction);
                        if let Some(script) = script {
                            buffer.set_script(script);
                        }
                        let result = shaper.shape_with_plan(plan, buffer, &features);
                        format!("{:?}/{:?}", result.glyph_infos(), result.glyph_positions())
                    };
                    assert_eq!(shape(&ordinary), shape(&admitted));
                }
            }
        }
    }
}

#[test]
fn every_plan_rejection_preserves_source_and_identical_retry_trace() {
    let bytes = fixtures::sfnt(&[], false);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let input = features().repeat(2);
    let mut complete = Vec::new();
    ShapePlan::try_new_admitted(
        &shaper,
        Direction::RightToLeft,
        Some(script::LATIN),
        None,
        &input,
        |op, cost| {
            complete.push((op, cost));
            Ok::<_, Reject>(())
        },
    )
    .unwrap()
    .unwrap();
    for reject in 0..complete.len() {
        let mut prefix = Vec::new();
        let error = ShapePlan::try_new_admitted(
            &shaper,
            Direction::RightToLeft,
            Some(script::LATIN),
            None,
            &input,
            |op, cost| {
                let index = prefix.len();
                prefix.push((op, cost));
                if index == reject {
                    Err(Reject(index))
                } else {
                    Ok(())
                }
            },
        )
        .err()
        .unwrap();
        assert!(matches!(error, ColdOperationError::Admission(Reject(i)) if i == reject));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
            reject
        );
        assert_eq!(prefix, complete[..=reject]);
        let mut retry = Vec::new();
        ShapePlan::try_new_admitted(
            &shaper,
            Direction::RightToLeft,
            Some(script::LATIN),
            None,
            &input,
            |op, cost| {
                retry.push((op, cost));
                Ok::<_, Reject>(())
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(retry, complete);
        // No partially built plan was installed in the shaper.
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str("office");
        buffer.guess_segment_properties();
        assert_eq!(shaper.shape(buffer, &[]).len(), 6);
    }
}

#[test]
fn unknown_paths_decline_before_feature_storage_and_budget_rejection_wins() {
    let language = Language::from_str("en-US-x-hbsc-latn").unwrap();
    for (bytes, direction, script, language) in [
        (fixtures::sfnt(&[], true), Direction::Invalid, None, None),
        (
            fixtures::sfnt(&[], true),
            Direction::LeftToRight,
            None,
            Some(&language),
        ),
        (
            fixtures::sfnt(&[], true),
            Direction::LeftToRight,
            Some(script::ARABIC),
            None,
        ),
        (
            fixtures::sfnt(&[], true),
            Direction::LeftToRight,
            Some(script::DEVANAGARI),
            None,
        ),
        (
            fixtures::sfnt(&[(*b"trak", &[0xff; 100])], false),
            Direction::LeftToRight,
            None,
            None,
        ),
        (
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
        ),
    ] {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let mut trace = Vec::new();
        assert!(
            ShapePlan::try_new_admitted(
                &shaper,
                direction,
                script,
                language,
                &features(),
                |op, cost| {
                    trace.push((op, cost));
                    Ok::<_, Reject>(())
                },
            )
            .unwrap()
            .is_none()
        );
        assert!(trace.iter().all(|(_, cost)| cost.allocation == 0));
        let error =
            ShapePlan::try_new_admitted(&shaper, direction, script, language, &[], |_, _| {
                Err(Reject(10))
            })
            .err()
            .unwrap();
        assert!(matches!(error, ColdOperationError::Admission(Reject(10))));
    }
}

#[test]
fn sticky_budget_cannot_be_bypassed_by_a_second_plan_request() {
    let bytes = fixtures::sfnt(&[], true);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut remaining = 50usize;
    let mut failed = false;
    let mut paid = 0;
    let mut admit = |_, cost: harfrust::ColdOperationCost| {
        if failed {
            return Err(Reject(1));
        }
        let fee = cost.work + cost.allocation;
        if fee > remaining {
            failed = true;
            return Err(Reject(0));
        }
        remaining -= fee;
        paid += fee;
        Ok(())
    };
    assert!(
        ShapePlan::try_new_admitted(&shaper, Direction::LeftToRight, None, None, &[], &mut admit,)
            .is_err()
    );
    assert!(
        ShapePlan::try_new_admitted(&shaper, Direction::LeftToRight, None, None, &[], &mut admit,)
            .is_err()
    );
    assert!(failed);
    assert_eq!(paid + remaining, 50);
}
