//! Full ordinary outputs, including real circle mappings and flag suppression.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "dotted_circle_kernel_fixtures.rs"]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData, script};

fn main() {
    for kind in [0, 3, 6] {
        for case in [0, 8] {
            for mapping in [0, 1, 2] {
                for circle in [false, true] {
                    let bytes = fixtures::font(kind, case, mapping, circle);
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    for pass in 0..2 {
                        for script in [None, Some(script::LATIN), Some(script::ARABIC)] {
                            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                                let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                                for text in fixtures::TEXTS {
                                    for flags in [
                                        BufferFlags::empty(),
                                        BufferFlags::BEGINNING_OF_TEXT,
                                        BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE,
                                        BufferFlags::BEGINNING_OF_TEXT
                                            | BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE,
                                    ] {
                                        for context in ["", "A"] {
                                            for clusters in 0..3 {
                                                for level in fixtures::LEVELS {
                                                    for spare in [0, 256] {
                                                        let mut input = fixtures::input(
                                                            text, direction, script, clusters,
                                                            level, spare,
                                                        );
                                                        input.set_flags(flags);
                                                        input.set_pre_context(context);
                                                        let output = shaper.shape_with_plan(
                                                            &plan,
                                                            input,
                                                            &[],
                                                        );
                                                        println!(
                                                            "{kind}/{case}/{mapping}/{circle}/{pass}/{script:?}/{direction:?}/{text:?}/{flags:?}/{context:?}/{clusters}/{level:?}/{spare}/{:?}/{:?}",
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
}
