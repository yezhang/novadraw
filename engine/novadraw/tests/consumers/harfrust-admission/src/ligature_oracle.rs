//! Original/production ligature output matrix, including rejected input paths.
mod ligature_fixtures;
use harfrust::{
    BufferFlags, Direction, Feature, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let features: Vec<_> = ["rlig=1", "kern=0", "rlig[1:4]=0"]
        .map(|value| Feature::from_str(value).unwrap())
        .to_vec();
    let mut fonts = Vec::new();
    for count in [1, 2, 8, 9, 17, 64, 65] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                fonts.push(ligature_fixtures::font(
                    &[ligature_fixtures::alternatives(count)],
                    coverage,
                    subtables,
                ));
            }
        }
    }
    fonts.push(ligature_fixtures::mixed_font());
    fonts.push(ligature_fixtures::font(
        &[
            ligature_fixtures::alternatives(9),
            ligature_fixtures::alternatives(3),
        ],
        2,
        3,
    ));
    for (font_index, bytes) in fonts.iter().enumerate() {
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
                            "AAAAAAAAAAAAAAAAAAB",
                            "ABAZA other",
                            "é a\u{301}",
                            "中文",
                            "السَّلَامُ",
                        ] {
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
                                    for (index, character) in text.chars().enumerate() {
                                        input.add(
                                            character,
                                            if clusters == 0 {
                                                index as u32
                                            } else if clusters == 1 {
                                                0
                                            } else {
                                                (index / 2) as u32
                                            },
                                        );
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
