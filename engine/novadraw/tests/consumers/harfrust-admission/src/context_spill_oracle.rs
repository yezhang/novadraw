//! Ordinary output for heap-backed and component-limit contexts.
#[path = "context_fixtures.rs"]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};

fn main() {
    for count in [9, 17, 64, 65] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                let bytes =
                    fixtures::font(count, coverage, subtables, &fixtures::records(count), 1, 0);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for pass in 0..2 {
                    for script in [script::LATIN, script::ARABIC] {
                        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                            let plan = ShapePlan::new(&shaper, direction, Some(script), None, &[]);
                            for text in [
                                String::new(),
                                "A".repeat(usize::from(count) - 1),
                                "A".repeat(usize::from(count) * 2 + 1),
                                "ABC".repeat(usize::from(count)),
                                "é a\u{301} 中文".into(),
                            ] {
                                for spare in [0, 256] {
                                    for clusters in 0..3 {
                                        let mut input = UnicodeBuffer::new();
                                        assert!(input.reserve(spare));
                                        for (index, character) in text.chars().enumerate() {
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
                                        input.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                                        let output = shaper.shape_with_plan(&plan, input, &[]);
                                        println!(
                                            "{count}/{coverage}/{subtables}/{pass}/{script:?}/{direction:?}/{text:?}/{spare}/{clusters}/{:?}/{:?}",
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
