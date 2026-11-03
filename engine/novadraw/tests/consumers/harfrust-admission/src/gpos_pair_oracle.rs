//! Full ordinary PairPos output across supported and unsupported algorithms.
#[path = "gpos_pair_fixtures.rs"]
mod fixtures;
use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer,
    script,
};

fn main() {
    for format in [1, 2] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                for lookups in [1, 2] {
                    for (v1, v2) in [(0, 0), (1, 0), (0, 15), (15, 15)] {
                        for substitutions in [false, true] {
                            let bytes = fixtures::font(
                                format,
                                coverage,
                                subtables,
                                lookups,
                                v1,
                                v2,
                                substitutions,
                            );
                            let font = FontRef::new(&bytes).unwrap();
                            let data = ShaperData::new(&font);
                            let shaper = data.shaper(&font).build();
                            for pass in 0..2 {
                                for script in [script::LATIN, script::ARABIC] {
                                    for direction in
                                        [Direction::LeftToRight, Direction::RightToLeft]
                                    {
                                        let plan = ShapePlan::new(
                                            &shaper,
                                            direction,
                                            Some(script),
                                            None,
                                            &[],
                                        );
                                        for text in ["ABCABC", "AZADAX", "", "é a\u{301} 中文"] {
                                            for clusters in 0..3 {
                                                for level in [
                                                    BufferClusterLevel::MonotoneGraphemes,
                                                    BufferClusterLevel::MonotoneCharacters,
                                                    BufferClusterLevel::Characters,
                                                ] {
                                                    let mut input = UnicodeBuffer::new();
                                                    for (index, ch) in text.chars().enumerate() {
                                                        input.add(
                                                            ch,
                                                            match clusters {
                                                                0 => index as u32,
                                                                1 => 0,
                                                                _ => (text.len() - index) as u32,
                                                            },
                                                        );
                                                    }
                                                    input.set_script(script);
                                                    input.set_direction(direction);
                                                    input.set_cluster_level(level);
                                                    input.set_flags(
                                                        BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
                                                    );
                                                    let output =
                                                        shaper.shape_with_plan(&plan, input, &[]);
                                                    println!(
                                                        "{format}/{coverage}/{subtables}/{lookups}/{v1}/{v2}/{substitutions}/{pass}/{script:?}/{direction:?}/{text:?}/{clusters}/{level:?}/{:?}/{:?}",
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
            }
        }
    }
}
