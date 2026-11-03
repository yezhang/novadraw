//! Independent original-version observations of fixed-script plan callbacks.
mod cold_fixtures;
mod script_callbacks_fixtures;
use harfrust::{
    Direction, Feature, FontRef, SerializeFlags, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let mut fonts = cold_fixtures::fonts();
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(bytes.to_vec());
    }
    fonts.push(script_callbacks_fixtures::syriac_font());
    let features: Vec<_> = ["calt=0", "stch=1", "ljmo=0", "rlig=0", "liga[1:4]=0"]
        .map(|value| Feature::from_str(value).unwrap())
        .to_vec();
    for (font_index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for pass in 0..2 {
            for script in [script::ARABIC, script::SYRIAC, script::HANGUL] {
                for direction in [
                    Direction::LeftToRight,
                    Direction::RightToLeft,
                    Direction::TopToBottom,
                    Direction::BottomToTop,
                ] {
                    for (feature_index, features) in [vec![], features.clone(), features.repeat(8)]
                        .iter()
                        .enumerate()
                    {
                        let plan = ShapePlan::new(&shaper, direction, Some(script), None, features);
                        for text in [
                            "",
                            "السَّلَامُ لا",
                            "ܐܒܓ",
                            "한글 \u{1100}\u{1161}\u{11a8}\u{302e}",
                            "ffi a\u{301}中文",
                        ] {
                            let mut buffer = UnicodeBuffer::new();
                            buffer.push_str(text);
                            buffer.set_script(script);
                            buffer.set_direction(direction);
                            let output = shaper.shape_with_plan(&plan, buffer, features);
                            println!(
                                "{font_index}/{pass}/{script:?}/{direction:?}/{feature_index}/{text:?}/{} {:?} {:?} {}",
                                shaper.units_per_em(),
                                output.glyph_infos(),
                                output.glyph_positions(),
                                output.serialize(&shaper, SerializeFlags::empty()),
                            );
                        }
                    }
                }
            }
        }
    }
}
