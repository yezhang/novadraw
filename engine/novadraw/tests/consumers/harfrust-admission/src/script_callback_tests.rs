//! Planning callback admission does not imply shaping admission.
use harfrust::{
    ColdOperation, ColdOperationError, Direction, Feature, FontRef, ShapePlan, ShaperData,
    UnicodeBuffer, script,
};
use std::{error::Error, fmt, str::FromStr};
#[path = "script_callbacks_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "script callback rejection {}", self.0)
    }
}
impl Error for Reject {}

const ARABIC: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf");
const CJK: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf");

#[test]
fn script_plans_preserve_complete_shaping_observations() {
    let syriac = fixtures::syriac_font();
    for bytes in [ARABIC, CJK, syriac.as_slice()] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for script in [script::ARABIC, script::SYRIAC, script::HANGUL] {
            for direction in [
                Direction::LeftToRight,
                Direction::RightToLeft,
                Direction::TopToBottom,
                Direction::BottomToTop,
            ] {
                for features in [
                    vec![],
                    ["calt=0", "stch=1", "ljmo=0", "rlig=0", "liga=1"]
                        .map(|value| Feature::from_str(value).unwrap())
                        .to_vec(),
                ] {
                    let ordinary =
                        ShapePlan::new(&shaper, direction, Some(script), None, &features);
                    let admitted = ShapePlan::try_new_admitted(
                        &shaper,
                        direction,
                        Some(script),
                        None,
                        &features,
                        |_, _| Ok::<_, Reject>(()),
                    )
                    .unwrap()
                    .unwrap();
                    for text in [
                        "",
                        "السَّلَامُ لا",
                        "ܐܒܓ",
                        "한글 \u{1100}\u{1161}\u{11a8}\u{302e}",
                        "ffi a\u{301}中文",
                    ] {
                        let shape = |plan: &ShapePlan| {
                            let mut buffer = UnicodeBuffer::new();
                            buffer.push_str(text);
                            buffer.set_script(script);
                            buffer.set_direction(direction);
                            let output = shaper.shape_with_plan(plan, buffer, &features);
                            format!("{:?}/{:?}", output.glyph_infos(), output.glyph_positions())
                        };
                        assert_eq!(shape(&ordinary), shape(&admitted));
                    }
                }
            }
        }
    }
}

#[test]
fn every_script_callback_rejection_stops_and_preserves_retry() {
    let syriac = fixtures::syriac_font();
    for (bytes, script) in [
        (ARABIC, script::ARABIC),
        (syriac.as_slice(), script::SYRIAC),
        (CJK, script::HANGUL),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let direction = Direction::RightToLeft;
        let mut complete = vec![];
        ShapePlan::try_new_admitted(&shaper, direction, Some(script), None, &[], |op, cost| {
            complete.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap()
        .unwrap();
        assert!(
            complete
                .iter()
                .any(|(op, cost)| { *op == ColdOperation::Storage && cost.allocation == 1 })
        );
        let (operation, cost) = complete.last().unwrap();
        assert_eq!(*operation, ColdOperation::Initialize);
        assert_eq!(cost.work, 2, "must store script-specific plan data");
        for reject in 0..complete.len() {
            let mut prefix = vec![];
            let error = ShapePlan::try_new_admitted(
                &shaper,
                direction,
                Some(script),
                None,
                &[],
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
            assert!(matches!(&error, ColdOperationError::Admission(Reject(i)) if *i == reject));
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                reject
            );
            assert_eq!(prefix, complete[..=reject]);
        }
        let mut retry = vec![];
        let plan =
            ShapePlan::try_new_admitted(&shaper, direction, Some(script), None, &[], |op, cost| {
                retry.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(retry, complete);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str("abc");
        buffer.set_script(script);
        buffer.set_direction(direction);
        assert!(!shaper.shape_with_plan(&plan, buffer, &[]).is_empty());
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str("abc");
        buffer.guess_segment_properties();
        assert!(!shaper.shape(buffer, &[]).is_empty());
    }
}
