use harfrust::{
    FontRef, InstanceOperation, ShaperInstance, Variation, VariationOperation,
    VariationOperationError,
};
use std::{error::Error, fmt};

#[path = "variation_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rejected {}", self.0)
    }
}
impl Error for Reject {}

fn settings() -> Vec<Variation> {
    (0u32..65)
        .map(|tag| variation(&tag.to_be_bytes(), 0.5))
        .chain([variation(&0u32.to_be_bytes(), -0.25)])
        .collect()
}

fn variation(tag: &[u8; 4], value: f32) -> Variation {
    Variation {
        tag: harfrust::Tag::new(tag),
        value,
    }
}

#[test]
fn every_user_variation_rejection_stops_and_preserves_error_and_previous_instance() {
    let variations = settings();
    let mut observed = Vec::new();
    for bytes in fixtures::fonts() {
        let font = FontRef::new(&bytes).unwrap();
        let expected = ShaperInstance::from_variations(&font, variations.iter().copied());
        let mut steps = Vec::new();
        let actual =
            ShaperInstance::try_from_variations_admitted(&font, &variations, |op, cost| {
                steps.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        for (op, _) in &steps {
            if !observed.contains(op) {
                observed.push(*op);
            }
        }
        for rejected in 0..steps.len() {
            let mut visited = 0;
            let error =
                ShaperInstance::try_from_variations_admitted(&font, &variations, |op, cost| {
                    assert_eq!((op, cost), steps[visited]);
                    let index = visited;
                    visited += 1;
                    if index == rejected {
                        Err(Reject(index))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(visited, rejected + 1);
            assert!(
                matches!(&error, VariationOperationError::Admission(Reject(i)) if *i == rejected)
            );
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                rejected
            );
            assert_eq!(
                format!(
                    "{:?}",
                    ShaperInstance::from_variations(&font, variations.iter().copied())
                ),
                format!("{expected:?}"),
            );
        }
    }
    for operation in [
        VariationOperation::MappingPlan,
        VariationOperation::MappingRead,
        VariationOperation::Normalization,
        VariationOperation::Instance(InstanceOperation::Storage),
        VariationOperation::Instance(InstanceOperation::DefaultCheck),
    ] {
        assert!(observed.contains(&operation), "unexercised {operation:?}");
    }
}

#[test]
fn unknown_avar_version_is_missing_before_coordinate_storage_or_normalization() {
    for version in [0x0002_0000, 0x0001_0001, 0x0000_0000, 0xffff_ffff] {
        let mapping = fixtures::avar(version, 12, &[]);
        let bytes = fixtures::font(12, false, &mapping, true);
        let font = FontRef::new(&bytes).unwrap();
        let mut steps = Vec::new();
        let actual = ShaperInstance::try_from_variations_admitted(&font, &settings(), |op, _| {
            steps.push(op);
            Ok::<_, Reject>(())
        })
        .unwrap();
        assert!(actual.is_none());
        assert_eq!(steps.last(), Some(&VariationOperation::MappingPlan));
        assert!(!steps.contains(&VariationOperation::Normalization));
        assert!(!steps.contains(&VariationOperation::Instance(InstanceOperation::Storage)));
    }
}

#[test]
fn normalization_charges_quadratic_scan_and_trailing_map_prefix() {
    let mapping = fixtures::avar(0x0001_0000, 0, &[&[(0, 8192)], &[(0, 4096), (8192, 12288)]]);
    let bytes = fixtures::font(12, true, &mapping, true);
    let font = FontRef::new(&bytes).unwrap();
    let mut costs = Vec::new();
    for input in [
        vec![variation(&0u32.to_be_bytes(), 0.5)],
        vec![variation(&0u32.to_be_bytes(), 0.5); 2],
    ] {
        ShaperInstance::try_from_variations_admitted(&font, &input, |op, cost| {
            if op == VariationOperation::Normalization {
                costs.push(cost.work);
            }
            if op == VariationOperation::Instance(InstanceOperation::Storage) {
                assert_eq!(cost.allocation, 12);
            }
            Ok::<_, Reject>(())
        })
        .unwrap()
        .unwrap();
    }
    // Formula tracks each input × all axes, independently of declared avar axes.
    let fixed = 12 + 8 + 2 + 4;
    assert_eq!(costs[1] - fixed, (costs[0] - fixed) * 2);
    assert!(costs[0] > 12 * (4 + 12 + 1));
}

#[test]
fn failed_user_variation_budget_is_sticky_and_no_candidate_is_returned() {
    let bytes = fixtures::font(12, false, &[], true);
    let font = FontRef::new(&bytes).unwrap();
    let mut remaining = 1;
    let mut failed = false;
    let mut calls = 0;
    let mut callback = |_, cost: harfrust::InstanceOperationCost| {
        calls += 1;
        if failed || cost.work + cost.allocation > remaining {
            failed = true;
            Err(Reject(calls))
        } else {
            remaining -= cost.work + cost.allocation;
            Ok(())
        }
    };
    for _ in 0..2 {
        assert!(
            ShaperInstance::try_from_variations_admitted(&font, &settings(), &mut callback)
                .is_err()
        );
    }
    assert!(failed);
    assert_eq!(remaining, 0);
    assert_eq!(calls, 3);
}

#[test]
fn user_instances_match_real_font_shaping_with_duplicate_unknown_and_clamped_values() {
    use harfrust::{Direction, ShaperData, UnicodeBuffer};
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        for value in [-1000.0, 100.0, 400.0, 650.0, 2000.0] {
            let input = [
                variation(b"wght", 900.0),
                variation(b"wght", value),
                variation(b"????", 1.0),
                variation(b"opsz", 20.0),
            ];
            let expected = ShaperInstance::from_variations(&font, input);
            let actual = ShaperInstance::try_from_variations_admitted(&font, &input, |_, _| {
                Ok::<_, Reject>(())
            })
            .unwrap()
            .unwrap();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            for text in ["", "office ffi", "a\u{301} é", "中文测试", "السَّلَامُ", "a中🙂"]
            {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let shape = |instance: &ShaperInstance| {
                        let shaper = data.shaper(&font).instance(Some(instance)).build();
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.guess_segment_properties();
                        buffer.set_direction(direction);
                        let output = shaper.shape(buffer, &[]);
                        (
                            format!("{:?}", output.glyph_infos()),
                            format!("{:?}", output.glyph_positions()),
                        )
                    };
                    assert_eq!(shape(&actual), shape(&expected));
                }
            }
        }
    }
}
