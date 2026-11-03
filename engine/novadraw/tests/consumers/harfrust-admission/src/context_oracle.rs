//! Original/production contexts, including inputs unsupported by admission.
mod context_fixtures;
use harfrust::{
    BufferFlags, Direction, Feature, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let features: Vec<_> = ["rlig=1", "kern=0", "rlig[1:4]=0"]
        .map(|value| Feature::from_str(value).unwrap())
        .to_vec();
    for (font_index, bytes) in context_fixtures::fonts().iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for pass in 0..2 {
            for script in [script::LATIN, script::ARABIC] {
                for direction in [
                    Direction::LeftToRight,
                    Direction::RightToLeft,
                    Direction::TopToBottom,
                    Direction::BottomToTop,
                ] {
                    for features in [&[][..], features.as_slice()] {
                        let plan = ShapePlan::new(&shaper, direction, Some(script), None, features);
                        for text in [
                            "",
                            "AAAAAAAAAAAAAB",
                            "ABCABC other",
                            "é a\u{301}",
                            "中文",
                            "السَّلَامُ",
                        ] {
                            for spare in [0, 128] {
                                for clusters in 0..3 {
                                    let mut input = UnicodeBuffer::new();
                                    assert!(input.reserve(spare));
                                    for (index, character) in text.chars().enumerate() {
                                        input.add(
                                            character,
                                            match clusters {
                                                0 => index as u32,
                                                1 => 0,
                                                _ => (index / 2) as u32,
                                            },
                                        );
                                    }
                                    input.set_script(script);
                                    input.set_direction(direction);
                                    input.set_flags(if clusters == 0 {
                                        BufferFlags::empty()
                                    } else {
                                        BufferFlags::PRODUCE_UNSAFE_TO_CONCAT
                                    });
                                    let output = shaper.shape_with_plan(&plan, input, features);
                                    println!(
                                        "{font_index}/{pass}/{script:?}/{direction:?}/{features:?}/{text:?}/{spare}/{clusters}/{:?}/{:?}",
                                        output.glyph_infos(),
                                        output.glyph_positions()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
