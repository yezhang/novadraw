//! Ordinary APIs only; rule input bytes are identical for both libraries.
mod general_language_inputs;
#[allow(dead_code)]
mod plan_layout_fixtures;
use harfrust::{
    Direction, FontRef, Language, SerializeFlags, ShapePlan, ShapePlanKey, ShaperData,
    UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let mut fonts = Vec::new();
    for substitution in [false, true] {
        for indexed in [false, true] {
            fonts.push(plan_layout_fixtures::font(substitution, indexed, true));
        }
    }
    fonts.push(include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec());
    fonts.push(include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec());
    for (index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for input in general_language_inputs::languages() {
            let language = Language::from_str(&input).unwrap();
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
                    for text in ["", "office ffi a\u{301}", "中文🙂 السَّلَامُ"] {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.set_script(script);
                        buffer.set_direction(direction);
                        buffer.set_language(language.clone());
                        let result = shaper.shape_with_plan(&plan, buffer, &[]);
                        println!(
                            "{index}/{input}/{script:?}/{direction:?}/{text:?}/{matches}/{:?}/{:?}/{}",
                            result.glyph_infos(),
                            result.glyph_positions(),
                            result.serialize(&shaper, SerializeFlags::empty())
                        );
                    }
                }
            }
        }
    }
}
