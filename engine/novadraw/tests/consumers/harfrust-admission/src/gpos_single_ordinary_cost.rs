//! Original/production ordinary positioning with and without earlier GSUB.
#![allow(clippy::duplicate_mod)]
#[path = "gpos_single_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABC".repeat(1024);
    for (case, substitutions) in [("simple", false), ("staged", true)] {
        let bytes = fixtures::font(2, 2, 3, 2, 15, substitutions);
        for hot in [false, true] {
            let name = format!("{case}/{}", if hot { "hot" } else { "cold" });
            for sample in 0..1100 {
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                let plan = ShapePlan::new(
                    &shaper,
                    Direction::LeftToRight,
                    Some(script::LATIN),
                    None,
                    &[],
                );
                let make_input = || {
                    let mut input = UnicodeBuffer::new();
                    input.push_str(&text);
                    input.set_script(script::LATIN);
                    input.set_direction(Direction::LeftToRight);
                    input
                };
                if hot {
                    shaper.shape_with_plan(&plan, make_input(), &[]);
                }
                let input = make_input();
                let start = Instant::now();
                let result = shaper.shape_with_plan(black_box(&plan), input, &[]);
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.len(), text.len() * if substitutions { 2 } else { 1 });
                black_box(result);
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
