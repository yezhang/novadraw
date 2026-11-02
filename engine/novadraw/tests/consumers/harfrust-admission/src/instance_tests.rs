use harfrust::{
    FontRef, InstanceOperation, InstanceOperationCost, InstanceOperationError, NormalizedCoord,
    ShaperInstance,
};
use std::{error::Error, fmt};

#[path = "instance_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rejected {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_instance_rejection_stops_and_retains_source_without_publishing() {
    let coords = [NormalizedCoord::from_f32(0.5); 65];
    let mut observed = Vec::new();
    for bytes in fixtures::fonts() {
        let font = FontRef::new(&bytes).unwrap();
        let expected = ShaperInstance::from_coords(&font, coords);
        let mut steps = Vec::new();
        let completed = ShaperInstance::try_from_coords_admitted(&font, &coords, |op, cost| {
            steps.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap();
        assert_eq!(format!("{completed:?}"), format!("{expected:?}"));
        for (op, _) in &steps {
            if !observed.contains(op) {
                observed.push(*op);
            }
        }
        for reject in 0..steps.len() {
            let mut visited = 0;
            let error = ShaperInstance::try_from_coords_admitted(&font, &coords, |op, cost| {
                assert_eq!((op, cost), steps[visited]);
                let index = visited;
                visited += 1;
                if index == reject {
                    Err(Reject(index))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(visited, reject + 1);
            assert!(matches!(&error, InstanceOperationError::Admission(Reject(i)) if *i == reject));
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                reject
            );
            // A complete previous instance survives, and ordinary retry is usable.
            assert_eq!(
                format!("{:?}", ShaperInstance::from_coords(&font, coords)),
                format!("{expected:?}")
            );
        }
    }
    for op in [
        InstanceOperation::Planning,
        InstanceOperation::Directory,
        InstanceOperation::TableRead,
        InstanceOperation::Storage,
        InstanceOperation::Coordinates,
        InstanceOperation::DefaultCheck,
        InstanceOperation::VariationRecord,
        InstanceOperation::ConditionSet,
        InstanceOperation::Condition,
        InstanceOperation::Predicate,
    ] {
        assert!(observed.contains(&op), "unexercised {op:?}");
    }
}

#[test]
fn coordinate_spill_charges_whole_axis_capacity_even_for_short_input() {
    for bytes in fixtures::fonts() {
        let font = FontRef::new(&bytes).unwrap();
        for length in [0, 1, 11, 12, 64, 65] {
            let coords = vec![NormalizedCoord::ZERO; length];
            let mut storage = Vec::new();
            let actual = ShaperInstance::try_from_coords_admitted(&font, &coords, |op, cost| {
                if op == InstanceOperation::Storage {
                    storage.push(cost);
                }
                Ok::<_, Reject>(())
            })
            .unwrap();
            assert_eq!(
                format!("{actual:?}"),
                format!("{:?}", ShaperInstance::from_coords(&font, coords))
            );
            for cost in storage {
                assert_eq!(cost.work, 0);
                assert!(matches!(cost.allocation, 12 | 64));
            }
        }
    }
}

#[test]
fn instance_sticky_budget_rejects_subsequent_candidates_before_lookup() {
    let bytes = &fixtures::fonts()[38];
    let font = FontRef::new(&bytes).unwrap();
    let mut failed = false;
    let mut calls = 0;
    let mut remaining = 3usize;
    let mut charged = 0;
    let mut admit = |_, cost: InstanceOperationCost| {
        calls += 1;
        if failed {
            return Err(Reject(calls));
        }
        let total = cost.work + cost.allocation;
        if total > remaining {
            failed = true;
            return Err(Reject(calls));
        }
        remaining -= total;
        charged += total;
        Ok(())
    };
    for _ in 0..2 {
        assert!(ShaperInstance::try_from_coords_admitted(&font, &[], &mut admit).is_err());
    }
    assert!(failed);
    assert_eq!(calls, 3);
    assert_eq!(charged, 1);
    assert_eq!(remaining, 2);
}

#[test]
fn admitted_instances_match_real_font_glyphs_positions_and_clusters() {
    use harfrust::{Direction, ShaperData, UnicodeBuffer};
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        for value in [-0.75, 0.0, 0.5] {
            let coords = [NormalizedCoord::from_f32(value); 65];
            let ordinary = ShaperInstance::from_coords(&font, coords);
            let admitted =
                ShaperInstance::try_from_coords_admitted(
                    &font,
                    &coords,
                    |_, _| Ok::<_, Reject>(()),
                )
                .unwrap();
            assert_eq!(format!("{ordinary:?}"), format!("{admitted:?}"));
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
                    assert_eq!(shape(&ordinary), shape(&admitted));
                }
            }
        }
    }
}
