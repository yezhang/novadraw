//! Complete ordinary output across four cluster levels and mapping algorithms.
#[path = "simple_unicode_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};

fn main() {
    for kind in 0..4 {
        for cmap in 0..8 {
            for missing in 0..3 {
                for substitutions in [false, true] {
                    let bytes = fixtures::font(kind, cmap, missing, substitutions);
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    for pass in 0..2 {
                        for script in [
                            Some(script::LATIN),
                            Some(script::HAN),
                            Some(script::COMMON),
                            Some(script::UNKNOWN),
                            None,
                            Some(script::ARABIC),
                        ] {
                            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                                let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                                for text in [
                                    "A(éÉǅ中文𠀀\u{2003}",
                                    "é",
                                    ")",
                                    "",
                                    "a\u{301}\u{200d}\u{1f1e6}\u{2044}",
                                ] {
                                    for clusters in 0..3 {
                                        for level in fixtures::LEVELS {
                                            for spare in [0, 256] {
                                                let input = fixtures::input(
                                                    text, direction, script, clusters, level, spare,
                                                );
                                                let result =
                                                    shaper.shape_with_plan(&plan, input, &[]);
                                                println!(
                                                    "{kind}/{cmap}/{missing}/{substitutions}/{pass}/{script:?}/{direction:?}/{text:?}/{clusters}/{level:?}/{spare}/{:?}/{:?}",
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
}
