//! Same ordinary single-substitution API for original/production comparison.
#![allow(clippy::duplicate_mod)]
#[allow(dead_code)]
mod single_fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn input(text: &str) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer
}

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let text = "ABCAB other ".repeat(80);
    for (name, config) in [("simple", (1, 1, 1, 1)), ("staged", (2, 2, 3, 2))] {
        let bytes = single_fixtures::font_with_layout(&single_fixtures::layout(
            config.0, config.1, config.2, config.3,
        ));
        for hot in [false, true] {
            for sample in 0..SAMPLES + WARMUP {
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
                if hot {
                    shaper.shape_with_plan(&plan, input(&text), &[]);
                }
                let mut buffer = input(&text);
                let start = Instant::now();
                let buffer = std::mem::replace(&mut buffer, UnicodeBuffer::new());
                let result = shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.len(), text.len());
                black_box(result);
                if sample >= WARMUP {
                    let state = if hot { "hot" } else { "cold" };
                    println!(
                        "{{\"font\":\"{name}/{state}\",\"sample\":{},\"ns\":{ns}}}",
                        sample - WARMUP
                    );
                }
            }
        }
    }
}
