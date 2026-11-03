//! Same ordinary GDEF calls for frozen original and production libraries.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "gdef_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABC".repeat(512);
    for kind in [0, 3, 6] {
        for case in [4, 8] {
            let bytes = fixtures::font(kind, case);
            for (direction_case, direction) in
                [(0, Direction::LeftToRight), (1, Direction::RightToLeft)]
            {
                for hot in [false, true] {
                    let name = format!(
                        "{kind}-{case}-{direction_case}/{}",
                        if hot { "hot" } else { "cold" }
                    );
                    for sample in 0..1100 {
                        let font = FontRef::new(&bytes).unwrap();
                        let data = ShaperData::new(&font);
                        let shaper = data.shaper(&font).build();
                        let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                        let make =
                            || fixtures::input(&text, direction, None, 0, fixtures::LEVELS[3], 0);
                        if hot {
                            shaper.shape_with_plan(&plan, make(), &[]);
                        }
                        let input = make();
                        let start = Instant::now();
                        let output = shaper.shape_with_plan(black_box(&plan), input, &[]);
                        let ns = start.elapsed().as_nanos();
                        assert_eq!(output.len(), if kind == 6 { 1536 } else { 3072 });
                        black_box(output);
                        if sample >= 100 {
                            println!(
                                "{{\"font\":\"{name}\",\"sample\":{},\"ns\":{ns}}}",
                                sample - 100
                            );
                        }
                    }
                }
            }
        }
    }
}
