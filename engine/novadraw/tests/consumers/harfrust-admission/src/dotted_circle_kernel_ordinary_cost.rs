//! Ordinary circle insertion, suffix copying and normalization growth timings.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "dotted_circle_kernel_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "\u{301}ÅÅéǻA\u{30a}a\u{315}\u{300}\u{316}".repeat(128);
    assert_eq!(text.chars().count(), 1408);
    for kind in [0, 3, 6] {
        for case in [0, 8] {
            for mapping in [0, 2] {
                for circle in [false, true] {
                    let bytes = fixtures::font(kind, case, mapping, circle);
                    for (direction_case, direction) in
                        [(0, Direction::LeftToRight), (1, Direction::RightToLeft)]
                    {
                        for hot in [false, true] {
                            let name = format!(
                                "{kind}-{case}-{mapping}-{circle}-{direction_case}/{}",
                                if hot { "hot" } else { "cold" }
                            );
                            for sample in 0..600 {
                                let font = FontRef::new(&bytes).unwrap();
                                let data = ShaperData::new(&font);
                                let shaper = data.shaper(&font).build();
                                let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                                let make = || {
                                    let mut input = fixtures::input(
                                        &text,
                                        direction,
                                        None,
                                        0,
                                        fixtures::LEVELS[3],
                                        0,
                                    );
                                    input.set_flags(BufferFlags::BEGINNING_OF_TEXT);
                                    input
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
}
