//! Full ordinary observations; no production admission API is used here.
#[path = "gdef_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};

fn main() {
    for kind in 0..8 {
        for case in 0..12 {
            let bytes = fixtures::font(kind, case);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            for pass in 0..2 {
                for script in [Some(script::LATIN), Some(script::HAN), None] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let plan = ShapePlan::new(&shaper, direction, script, None, &[]);
                        for text in ["AAABCABC", "ABCAAA", "", "é中(ǅ\u{2003}"] {
                            for clusters in 0..3 {
                                for level in fixtures::LEVELS {
                                    for spare in [0, 256] {
                                        let result = shaper.shape_with_plan(
                                            &plan,
                                            fixtures::input(
                                                text, direction, script, clusters, level, spare,
                                            ),
                                            &[],
                                        );
                                        println!(
                                            "{kind}/{case}/{pass}/{script:?}/{direction:?}/{text:?}/{clusters}/{level:?}/{spare}/{:?}/{:?}",
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
