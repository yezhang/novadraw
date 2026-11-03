//! Original/production PairPos costs with both table formats.
// Shared frozen fixtures intentionally retain their original nested names.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "gpos_pair_fixtures.rs"]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABC".repeat(1024);
    for format in [1, 2] {
        for (case, substitutions) in [("simple", false), ("staged", true)] {
            let bytes = fixtures::font(format, 2, 3, 2, 15, 15, substitutions);
            for hot in [false, true] {
                let name = format!("pair{format}-{case}/{}", if hot { "hot" } else { "cold" });
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
                    let make = || {
                        let mut input = UnicodeBuffer::new();
                        input.push_str(&text);
                        input.set_script(script::LATIN);
                        input.set_direction(Direction::LeftToRight);
                        input.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                        input
                    };
                    if hot {
                        shaper.shape_with_plan(&plan, make(), &[]);
                    }
                    let input = make();
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
}
