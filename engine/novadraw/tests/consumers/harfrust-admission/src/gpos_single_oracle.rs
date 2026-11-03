//! Full ordinary positioning output, including unsupported scripts and flags.
#[path = "gpos_single_fixtures.rs"]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};

fn main() {
    for format in [1, 2] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                for lookups in [1, 2] {
                    for values in [0, 1, 2, 4, 8, 15] {
                        for substitutions in [false, true] {
                            let bytes = fixtures::font(
                                format,
                                coverage,
                                subtables,
                                lookups,
                                values,
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
                                        for text in ["ABCABC", "other", "", "é a\u{301} 中文"] {
                                            for spare in [0, 256] {
                                                for clusters in 0..3 {
                                                    let mut input = UnicodeBuffer::new();
                                                    assert!(input.reserve(spare));
                                                    for (index, character) in
                                                        text.chars().enumerate()
                                                    {
                                                        input.add(
                                                            character,
                                                            match clusters {
                                                                0 => index as u32,
                                                                1 => 0,
                                                                _ => (text.len() - index) as u32,
                                                            },
                                                        );
                                                    }
                                                    input.set_script(script);
                                                    input.set_direction(direction);
                                                    input.set_flags(
                                                        BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
                                                    );
                                                    let output =
                                                        shaper.shape_with_plan(&plan, input, &[]);
                                                    println!(
                                                        "{format}/{coverage}/{subtables}/{lookups}/{values}/{substitutions}/{pass}/{script:?}/{direction:?}/{text:?}/{spare}/{clusters}/{:?}/{:?}",
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
