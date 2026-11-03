//! Original/production timings for decomposition, sorting and composition.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "normalizer_kernel_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ÅÅéǻA\u{30a}a\u{315}\u{300}\u{316}".repeat(128);
    assert_eq!(text.chars().count(), 1280);
    for kind in [0, 3, 6] {
        for case in [0, 8] {
            for mapping in [0, 1] {
                let bytes = fixtures::font(kind, case, mapping);
                for (direction_case, direction) in
                    [(0, Direction::LeftToRight), (1, Direction::RightToLeft)]
                {
                    for hot in [false, true] {
                        let name = format!(
                            "{kind}-{case}-{mapping}-{direction_case}/{}",
                            if hot { "hot" } else { "cold" }
                        );
                        for sample in 0..600 {
                            let font = FontRef::new(&bytes).unwrap();
                            let data = ShaperData::new(&font);
                            let shaper = data.shaper(&font).build();
                            let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                            let make = || {
                                fixtures::input(&text, direction, None, 0, fixtures::LEVELS[3], 0)
                            };
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
}
