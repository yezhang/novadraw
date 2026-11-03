//! Full ordinary directional output, including unsupported script and mapping.
#[path = "rtl_fixtures.rs"]
mod fixtures;
use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer,
    script,
};

fn main() {
    for kind in 0..4 {
        for missing in [false, true] {
            for substitutions in [false, true] {
                let bytes = fixtures::font(kind, missing, substitutions);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for pass in 0..2 {
                    for script in [
                        Some(script::LATIN),
                        Some(script::COMMON),
                        Some(script::UNKNOWN),
                        None,
                        Some(script::ARABIC),
                    ] {
                        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                            let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                            for text in ["ABC(ABC[<>]", "(", ")", "", "é a\u{301} 中文"] {
                                for clusters in 0..3 {
                                    for level in [
                                        BufferClusterLevel::MonotoneGraphemes,
                                        BufferClusterLevel::MonotoneCharacters,
                                        BufferClusterLevel::Characters,
                                    ] {
                                        for spare in [0, 256] {
                                            let mut input = UnicodeBuffer::new();
                                            assert!(input.reserve(spare));
                                            for (i, ch) in text.chars().enumerate() {
                                                input.add(
                                                    ch,
                                                    match clusters {
                                                        0 => i as u32,
                                                        1 => 0,
                                                        _ => (text.len() - i) as u32,
                                                    },
                                                );
                                            }
                                            input.set_direction(direction);
                                            if let Some(script) = script {
                                                input.set_script(script);
                                            }
                                            input.set_cluster_level(level);
                                            input.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                                            let result = shaper.shape_with_plan(&plan, input, &[]);
                                            println!(
                                                "{kind}/{missing}/{substitutions}/{pass}/{script:?}/{direction:?}/{text:?}/{clusters}/{level:?}/{spare}/{:?}/{:?}",
                                                result.glyph_infos(),
                                                result.glyph_positions()
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
    }
}
