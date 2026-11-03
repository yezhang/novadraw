//! Ordinary APIs only, compiled against original and production dependencies.
#[allow(dead_code)]
mod plan_layout_fixtures;
use harfrust::{
    Direction, FontRef, Language, SerializeFlags, ShapePlan, ShapePlanKey, ShaperData,
    UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let mut languages: Vec<String> = [
        "en",
        "zh-hant-mo",
        "en-x-hbscdflt",
        "x-hboteng",
        "x-hbotdflt",
        "x-hbotabc-hboteng",
        "x-hboté-hboteng",
        "x-hbot-hboteng",
        "x-hbot1234-hbsclatn",
        "en-a-qwe-b-zxc-x-hboteng",
        "x-hbsclatn-hboteng",
        "x-hbot1",
        "x-hbot",
        "x-hbsc-hboteng",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for len in [0, 1, 7, 8, 9, 64, 1024] {
        languages.push(format!("en-x-{}-hboteng", "a".repeat(len)));
        languages.push(format!("x-{}-hboteng-hbscdflt", "a".repeat(len)));
    }
    let mut fonts = Vec::new();
    for substitution in [false, true] {
        for indexed in [false, true] {
            fonts.push(plan_layout_fixtures::font(substitution, indexed, true));
        }
    }
    fonts.push(include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec());
    fonts.push(include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec());
    for (font_index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for language in &languages {
            let language = Language::from_str(language).unwrap();
            for script in [script::LATIN, script::ARABIC] {
                for direction in [
                    Direction::LeftToRight,
                    Direction::RightToLeft,
                    Direction::TopToBottom,
                    Direction::BottomToTop,
                ] {
                    let plan =
                        ShapePlan::new(&shaper, direction, Some(script), Some(&language), &[]);
                    let matches = ShapePlanKey::new(Some(script), direction)
                        .language(Some(&language))
                        .matches(&plan);
                    for text in ["", "office ffi", "a\u{301}", "中文🙂", "السَّلَامُ"]
                    {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.set_script(script);
                        buffer.set_direction(direction);
                        buffer.set_language(language.clone());
                        let result = shaper.shape_with_plan(&plan, buffer, &[]);
                        println!(
                            "{font_index}/{language:?}/{script:?}/{direction:?}/{text:?}/{matches}/{:?}/{:?}/{}",
                            result.glyph_infos(),
                            result.glyph_positions(),
                            result.serialize(&shaper, SerializeFlags::empty()),
                        );
                    }
                }
            }
        }
    }
}
