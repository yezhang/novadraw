//! Configuration admission does not admit any glyph query or shaping.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use harfrust::{
    ColdOperation, ColdOperationError, Direction, FontRef, NormalizedCoord, ShaperData,
    ShaperInstance, UnicodeBuffer,
};
use std::{error::Error, fmt};

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration rejected at {}", self.0)
    }
}
impl Error for Reject {}

fn output(shaper: &harfrust::Shaper<'_>, text: &str, direction: Direction) -> String {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    buffer.set_direction(direction);
    let result = shaper.shape(buffer, &[]);
    format!("{:?}/{:?}", result.glyph_infos(), result.glyph_positions())
}

#[test]
fn configured_tables_and_normalized_instances_match_ordinary_output() {
    let mut fonts = fixtures::fonts();
    fonts.extend([
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    ]);
    let mut supported = 0;
    for bytes in &fonts {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        for coords in [vec![], vec![NormalizedCoord::from_bits(8192); 16]] {
            let instance = ShaperInstance::from_coords(&font, coords);
            for point_size in [None, Some(12.0)] {
                let ordinary = data
                    .shaper(&font)
                    .instance(Some(&instance))
                    .point_size(point_size)
                    .build();
                let mut trace = Vec::new();
                let Some(admitted) = data
                    .shaper(&font)
                    .instance(Some(&instance))
                    .point_size(point_size)
                    .try_build_admitted(|op, cost| {
                        trace.push((op, cost));
                        Ok::<_, Reject>(())
                    })
                    .unwrap()
                else {
                    continue;
                };
                supported += 1;
                assert_eq!(ordinary.units_per_em(), admitted.units_per_em());
                assert_eq!(ordinary.coords(), admitted.coords());
                assert!(trace.iter().all(|(_, cost)| cost.allocation == 0));
                assert!(
                    trace
                        .iter()
                        .any(|(op, cost)| *op == ColdOperation::CmapSubtable && cost.work > 0)
                );
                for direction in [
                    Direction::LeftToRight,
                    Direction::RightToLeft,
                    Direction::TopToBottom,
                    Direction::BottomToTop,
                ] {
                    for text in ["", "office ffi", "a\u{301} é", "中文🙂", "السَّلَامُ"]
                    {
                        assert_eq!(
                            output(&ordinary, text, direction),
                            output(&admitted, text, direction)
                        );
                    }
                }
            }
        }
    }
    assert!(supported >= 120);
}

#[test]
fn each_configuration_rejection_retains_source_and_complete_objects() {
    let bytes = include_bytes!("../../../../assets/fonts/InterVariable.ttf");
    let font = FontRef::new(bytes).unwrap();
    let data = ShaperData::new(&font);
    let instance = ShaperInstance::from_coords(&font, [NormalizedCoord::from_bits(8192); 16]);
    let mut complete = Vec::new();
    data.shaper(&font)
        .instance(Some(&instance))
        .try_build_admitted(|op, cost| {
            complete.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap()
        .unwrap();
    let expected = output(
        &data.shaper(&font).instance(Some(&instance)).build(),
        "office",
        Direction::LeftToRight,
    );
    for reject in 0..complete.len() {
        let mut prefix = Vec::new();
        let error = data
            .shaper(&font)
            .instance(Some(&instance))
            .try_build_admitted(|op, cost| {
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
        assert!(matches!(error, ColdOperationError::Admission(Reject(i)) if i == reject));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
            reject
        );
        assert_eq!(prefix, complete[..=reject]);
        let mut retry = Vec::new();
        let recovered = data
            .shaper(&font)
            .instance(Some(&instance))
            .try_build_admitted(|op, cost| {
                retry.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(retry, complete);
        assert_eq!(
            output(&recovered, "office", Direction::LeftToRight),
            expected
        );
        assert_eq!(
            output(
                &data.shaper(&font).instance(Some(&instance)).build(),
                "office",
                Direction::LeftToRight
            ),
            expected
        );
    }
}

#[test]
fn variable_aat_headers_decline_before_other_configuration_and_rejection_wins() {
    for tag in [*b"morx", *b"kerx", *b"kern"] {
        for header in [vec![0, 1, 0], vec![0, 1, 0, 0, 255, 255, 255, 255]] {
            let bytes = fixtures::sfnt(&[(tag, &header)], true);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let mut trace = Vec::new();
            assert!(
                data.shaper(&font)
                    .try_build_admitted(|op, cost| {
                        trace.push((op, cost));
                        Ok::<_, Reject>(())
                    })
                    .unwrap()
                    .is_none()
            );
            assert!(
                trace.iter().all(
                    |(op, _)| *op == ColdOperation::Planning || *op == ColdOperation::TableRead
                )
            );
            assert!(
                data.shaper(&font)
                    .try_build_admitted(|_, _| Err(Reject(0)))
                    .is_err()
            );
        }
    }
}

#[test]
fn empty_aat_headers_configure_and_sticky_rejection_stops_later_work() {
    for (tag, header) in [
        (*b"morx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
        (*b"kerx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
        (*b"kern", vec![0, 0, 0, 0]),
        (*b"kern", vec![0, 1, 0, 0, 0, 0, 0, 0]),
    ] {
        let bytes = fixtures::sfnt(&[(tag, &header)], true);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        assert!(
            data.shaper(&font)
                .try_build_admitted(|_, _| Ok::<_, Reject>(()))
                .unwrap()
                .is_some()
        );
        let mut failed = false;
        let charged = std::cell::Cell::new(0);
        let mut admit = |_, cost: harfrust::ColdOperationCost| {
            if failed {
                return Err(Reject(1));
            }
            if cost.work > 4 {
                failed = true;
                return Err(Reject(0));
            }
            charged.set(charged.get() + cost.work);
            Ok(())
        };
        assert!(data.shaper(&font).try_build_admitted(&mut admit).is_err());
        let before_retry = charged.get();
        assert!(data.shaper(&font).try_build_admitted(&mut admit).is_err());
        assert!(failed);
        assert_eq!(charged.get(), before_retry);
    }
}
