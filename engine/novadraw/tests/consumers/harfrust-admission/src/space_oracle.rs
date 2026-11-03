//! Complete ordinary space/hyphen output, using only published APIs.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "space_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};

fn main() {
    for kind in 0..8 {
        for case in [4, 8] {
            for mapping in 0..10 {
                let bytes = fixtures::font(kind, case, mapping);
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                for pass in 0..2 {
                    for script in [Some(script::LATIN), None] {
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
