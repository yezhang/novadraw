//! Original and production ordinary shaping on heap-backed contexts.
#![allow(clippy::duplicate_mod)]
#[path = "context_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "A".repeat(1040);
    for (case, count) in [("simple", 9), ("staged", 64)] {
        let bytes = fixtures::font(count, 2, 3, &fixtures::records(count), 1, 0);
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
                assert_eq!(result.len(), text.len());
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
