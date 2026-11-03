//! Ordinary shaping observations for single substitutions and other scripts.
mod single_fixtures;
use harfrust::{
    BufferFlags, Direction, Feature, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let features: Vec<_> = ["rlig=1", "kern=0", "rlig[1:4]=0"]
        .map(|value| Feature::from_str(value).unwrap())
        .to_vec();
    for (font_index, bytes) in single_fixtures::fonts().iter().enumerate() {
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
                        for text in ["", "ABCAB", "other", "é a\u{301}", "中文", "السَّلَامُ"]
                        {
                            for reserve in [0, 128] {
                                for (clusters, flags) in [
                                    BufferFlags::empty(),
                                    BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
                                    BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL,
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    let mut input = UnicodeBuffer::new();
                                    assert!(input.reserve(reserve));
                                    if clusters == 0 {
                                        input.push_str(text);
                                    } else {
                                        for (index, character) in text.chars().enumerate() {
                                            input.add(
                                                character,
                                                if clusters == 1 { 0 } else { (index / 2) as u32 },
                                            );
                                        }
                                    }
                                    input.set_script(script);
                                    input.set_direction(direction);
                                    input.set_flags(flags);
                                    let output = shaper.shape_with_plan(&plan, input, features);
                                    println!(
                                        "{font_index}/{pass}/{script:?}/{direction:?}/{features:?}/{text:?}/{reserve}/{clusters}/{:?}/{:?}",
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
