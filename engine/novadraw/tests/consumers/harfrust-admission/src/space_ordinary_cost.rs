//! Same space/hyphen input for original/production ordinary kernels.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "space_fixtures.rs"]
// Preserve the frozen fixture's original range expression.
#[allow(dead_code, clippy::manual_range_contains)]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "AB\u{2007}\u{2008}\u{202f}\u{2011}\u{2000}\u{2001}\u{a0}CDE".repeat(128);
    assert_eq!(text.chars().count(), 1536);
    for kind in [0, 3, 6] {
        for case in [4, 8] {
            let bytes = fixtures::font(kind, case, 0);
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
                        assert!(!output.is_empty());
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
