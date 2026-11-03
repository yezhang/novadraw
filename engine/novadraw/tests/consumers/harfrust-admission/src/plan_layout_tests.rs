//! OT plan admission does not admit execution of a shaping plan.

#[path = "plan_layout_fixtures.rs"]
mod fixtures;

use harfrust::{
    ColdOperation, ColdOperationCost, ColdOperationError, Direction, Feature, FontRef,
    NormalizedCoord, ShapePlan, ShapePlanKey, ShaperData, ShaperInstance, UnicodeBuffer, script,
};
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OT plan rejected {}", self.0)
    }
}
impl Error for Reject {}

fn features() -> Vec<Feature> {
    ["kern=1", "liga=1", "abcd=2", "rand=1", "liga[1:4]=0"]
        .iter()
        .map(|text| Feature::from_str(text).unwrap())
        .collect()
}

fn observation(
    shaper: &harfrust::Shaper<'_>,
    plan: &ShapePlan,
    text: &str,
    direction: Direction,
    features: &[Feature],
) -> String {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(direction);
    buffer.set_script(script::LATIN);
    let result = shaper.shape_with_plan(plan, buffer, features);
    format!("{:?}/{:?}", result.glyph_infos(), result.glyph_positions())
}

#[test]
fn ot_plans_match_full_outputs_with_required_features_variations_and_malformed_tables() {
    let mut fonts = fixtures::fonts();
    fonts.extend([
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    ]);
    for bytes in fonts {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        for coords in [&[][..], &[0.5][..]] {
            let instance = ShaperInstance::from_coords(
                &font,
                coords.iter().copied().map(NormalizedCoord::from_f32),
            );
            let shaper = data.shaper(&font).instance(Some(&instance)).build();
            for direction in [
                Direction::LeftToRight,
                Direction::RightToLeft,
                Direction::TopToBottom,
                Direction::BottomToTop,
            ] {
                for features in [vec![], features(), features().repeat(12)] {
                    let ordinary =
                        ShapePlan::new(&shaper, direction, Some(script::LATIN), None, &features);
                    let admitted = ShapePlan::try_new_admitted(
                        &shaper,
                        direction,
                        Some(script::LATIN),
                        None,
                        &features,
                        |_, _| Ok::<_, Reject>(()),
                    )
                    .unwrap()
                    .unwrap();
                    assert!(
                        ShapePlanKey::new(Some(script::LATIN), direction)
                            .instance(Some(&instance))
                            .features(&features)
                            .matches(&admitted)
                    );
                    for text in ["", "aaa", "office ffi", "a\u{301} é", "中文🙂", "אבג"] {
                        assert_eq!(
                            observation(&shaper, &ordinary, text, direction, &features),
                            observation(&shaper, &admitted, text, direction, &features),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_ot_plan_rejection_stops_at_its_prefix_and_leaves_ordinary_calls_available() {
    for substitution in [false, true] {
        for indexed in [false, true] {
            for variation in [false, true] {
                let bytes = fixtures::font(substitution, indexed, variation);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let instance = ShaperInstance::from_coords(&font, [NormalizedCoord::from_f32(0.5)]);
                let shaper = data.shaper(&font).instance(Some(&instance)).build();
                let input = features();
                let run = |admit: &mut dyn FnMut(
                    ColdOperation,
                    ColdOperationCost,
                ) -> Result<(), Reject>| {
                    ShapePlan::try_new_admitted(
                        &shaper,
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &input,
                        admit,
                    )
                };
                let mut complete = Vec::new();
                run(&mut |op, cost| {
                    complete.push((op, cost));
                    Ok::<_, Reject>(())
                })
                .unwrap()
                .unwrap();
                assert!(complete.iter().any(|(op, _)| *op == ColdOperation::Lookup));
                assert!(complete.iter().any(|(op, _)| *op == ColdOperation::Sort));
                assert!(
                    complete
                        .iter()
                        .any(|(op, cost)| *op == ColdOperation::Storage && cost.allocation > 0)
                );
                let ordinary = ShapePlan::new(
                    &shaper,
                    Direction::LeftToRight,
                    Some(script::LATIN),
                    None,
                    &input,
                );
                let expected =
                    observation(&shaper, &ordinary, "aaa", Direction::LeftToRight, &input);
                for reject in 0..complete.len() {
                    let mut prefix = Vec::new();
                    let error = run(&mut |op, cost| {
                        let index = prefix.len();
                        prefix.push((op, cost));
                        if index == reject {
                            Err(Reject(index))
                        } else {
                            Ok(())
                        }
                    })
                    .err()
                    .unwrap();
                    assert!(
                        matches!(&error, ColdOperationError::Admission(Reject(i)) if *i == reject)
                    );
                    assert_eq!(
                        error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                        reject
                    );
                    assert_eq!(prefix, complete[..=reject]);
                    let mut retry = Vec::new();
                    run(&mut |op, cost| {
                        retry.push((op, cost));
                        Ok::<_, Reject>(())
                    })
                    .unwrap()
                    .unwrap();
                    assert_eq!(retry, complete);
                    let next = ShapePlan::new(
                        &shaper,
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &input,
                    );
                    assert_eq!(
                        observation(&shaper, &next, "aaa", Direction::LeftToRight, &input),
                        expected
                    );
                }
            }
        }
    }
}
