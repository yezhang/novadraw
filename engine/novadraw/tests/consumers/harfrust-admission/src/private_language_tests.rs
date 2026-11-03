//! Private-use language admission is independent of shaping admission.

#[path = "plan_layout_fixtures.rs"]
mod fixtures;
#[path = "general_language_inputs.rs"]
mod general_language_inputs;

use harfrust::{
    ColdOperation, ColdOperationCost, ColdOperationError, Direction, FontRef, Language, ShapePlan,
    ShapePlanKey, ShaperData, UnicodeBuffer, script,
};
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "private language rejected {}", self.0)
    }
}
impl Error for Reject {}

fn languages() -> Vec<String> {
    let mut result: Vec<String> = [
        "x-hbotdflt",
        "x-hboteng",
        "en-x-hboteng",
        "x-hbot1234-hbsclatn",
        "x-hbsclatn-hboteng",
        "en-a-qwe-b-zxc-x-hboteng",
        "zh-hant-mo-x-hbotzhtm",
        "x-hbotabc-hboteng",
        "x-hbot1",
        "x-hboté-hboteng",
        "x-hbot-hboteng",
        "x-hbsc-hboteng",
        "en",
        "zh-hant-hk",
        "en-x-hbscdflt",
        "x-hbot",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for length in [0, 1, 7, 8, 9, 64, 1024] {
        result.push(format!("en-x-{}-hboteng", "a".repeat(length)));
        result.push(format!("x-{}-hboteng-hbscdflt", "a".repeat(length)));
    }
    result.extend(general_language_inputs::languages());
    result
}

fn observation(
    shaper: &harfrust::Shaper<'_>,
    plan: &ShapePlan,
    language: &Language,
    direction: Direction,
) -> String {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str("office ffi a\u{301} 中文🙂 אבג");
    buffer.set_script(script::LATIN);
    buffer.set_direction(direction);
    buffer.set_language(language.clone());
    let result = shaper.shape_with_plan(plan, buffer, &[]);
    format!("{:?}/{:?}", result.glyph_infos(), result.glyph_positions())
}

#[test]
fn private_and_general_languages_match_ordinary_plans() {
    let mut fonts = fixtures::fonts();
    fonts.push(include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec());
    for bytes in fonts {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for text in languages() {
            let language = Language::from_str(&text).unwrap();
            for direction in [
                Direction::LeftToRight,
                Direction::RightToLeft,
                Direction::TopToBottom,
                Direction::BottomToTop,
            ] {
                let ordinary = ShapePlan::new(
                    &shaper,
                    direction,
                    Some(script::LATIN),
                    Some(&language),
                    &[],
                );
                let result = ShapePlan::try_new_admitted(
                    &shaper,
                    direction,
                    Some(script::LATIN),
                    Some(&language),
                    &[],
                    |_, _| Ok::<_, Reject>(()),
                )
                .unwrap();
                let admitted = result.expect("language mapping is fully admitted");
                assert_eq!(admitted.language(), Some(&language));
                assert!(
                    ShapePlanKey::new(Some(script::LATIN), direction)
                        .language(Some(&language))
                        .matches(&admitted)
                );
                assert_eq!(
                    observation(&shaper, &ordinary, &language, direction),
                    observation(&shaper, &admitted, &language, direction)
                );
            }
        }
    }
}

#[test]
fn every_private_language_rejection_keeps_source_prefix_and_next_call_available() {
    let bytes = fixtures::font(true, true, true);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for text in [
        "x-hbot1",
        "x-hboteng",
        "en-x-hbsclatn-hboteng",
        "en",
        "zh-hant-mo",
        "ro-abc-md",
        "und-fonipaa-fonipa",
        "und-abc",
        "zzzz",
        "x-hbot",
        "en-x-hbscdflt",
    ] {
        let language = Language::from_str(text).unwrap();
        let run =
            |admit: &mut dyn FnMut(ColdOperation, ColdOperationCost) -> Result<(), Reject>| {
                ShapePlan::try_new_admitted(
                    &shaper,
                    Direction::LeftToRight,
                    Some(script::LATIN),
                    Some(&language),
                    &[],
                    admit,
                )
            };
        let mut complete = Vec::new();
        run(&mut |op, cost| {
            complete.push((op, cost));
            Ok(())
        })
        .unwrap()
        .unwrap();
        if text.len() > 8 {
            assert!(complete.iter().any(|(op, cost)| {
                *op == ColdOperation::Storage && cost.allocation == text.len()
            }));
        }
        let ordinary = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            Some(&language),
            &[],
        );
        let expected = observation(&shaper, &ordinary, &language, Direction::LeftToRight);
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
            assert!(matches!(&error, ColdOperationError::Admission(Reject(i)) if *i == reject));
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                reject
            );
            assert_eq!(prefix, complete[..=reject]);
            let mut retry = Vec::new();
            run(&mut |op, cost| {
                retry.push((op, cost));
                Ok(())
            })
            .unwrap()
            .unwrap();
            assert_eq!(retry, complete);
            let next = ShapePlan::new(
                &shaper,
                Direction::LeftToRight,
                Some(script::LATIN),
                Some(&language),
                &[],
            );
            assert_eq!(
                observation(&shaper, &next, &language, Direction::LeftToRight),
                expected
            );
        }
    }
}
