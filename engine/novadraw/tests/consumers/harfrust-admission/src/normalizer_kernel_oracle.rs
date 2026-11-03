//! Full ordinary output from the original and shared normalization kernels.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "normalizer_kernel_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};

fn main() {
    for kind in 0..8 {
        for case in [0, 8] {
            for mapping in 0..8 {
                let bytes = fixtures::font(kind, case, mapping);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for pass in 0..2 {
                    for script in [
                        None,
                        Some(script::LATIN),
                        Some(script::HAN),
                        Some(script::HEBREW),
                        Some(script::ARABIC),
                    ] {
                        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                            let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                            for text in fixtures::TEXTS {
                                for clusters in 0..3 {
                                    for level in fixtures::LEVELS {
                                        for spare in [0, 256] {
                                            let output = shaper.shape_with_plan(
                                                &plan,
                                                fixtures::input(
                                                    text, direction, script, clusters, level, spare,
                                                ),
                                                &[],
                                            );
                                            println!(
                                                "{kind}/{case}/{mapping}/{pass}/{script:?}/{direction:?}/{text:?}/{clusters}/{level:?}/{spare}/{:?}/{:?}",
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
