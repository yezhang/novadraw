//! Isolated single-substitution admission overhead with cold and hot caches.
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
    for (case, config) in [(1, 1, 1, 1), (2, 2, 3, 2)].into_iter().enumerate() {
        let bytes = single_fixtures::font_with_layout(&single_fixtures::layout(
            config.0, config.1, config.2, config.3,
        ));
        for hot in [false, true] {
            let dry = || {
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
                let mut calls = 0;
                shaper
                    .try_shape_with_plan_admitted(&plan, &mut input(&text), &[], |_, _| {
                        calls += 1;
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                    .unwrap();
                calls
            };
            let complete_calls = dry();
            for sample in 0..SAMPLES + WARMUP {
                for offset in 0..4 {
                    let mode = (sample + offset) % 4;
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
                    let mut calls = 0;
                    let mut work = 0;
                    let mut allocation = 0;
                    let start = Instant::now();
                    let result = if mode == 0 {
                        let buffer = std::mem::replace(&mut buffer, UnicodeBuffer::new());
                        Ok(Some(shaper.shape_with_plan(black_box(&plan), buffer, &[])))
                    } else {
                        shaper.try_shape_with_plan_admitted(
                            black_box(&plan),
                            &mut buffer,
                            &[],
                            |_, cost| {
                                if mode == 1 {
                                    return Ok(());
                                }
                                let index = calls;
                                calls += 1;
                                if mode == 3 && index + 1 == complete_calls {
                                    return Err(());
                                }
                                work += cost.work;
                                allocation += cost.allocation;
                                Ok(())
                            },
                        )
                    };
                    let elapsed_ns = start.elapsed().as_nanos();
                    assert_eq!(result.is_err(), mode == 3);
                    if mode >= 2 {
                        assert_eq!(calls, complete_calls);
                    }
                    if let Ok(Some(result)) = result {
                        assert_eq!(result.len(), text.len());
                        black_box(result);
                    } else {
                        assert!(buffer.is_empty());
                    }
                    if sample >= WARMUP {
                        let path = ["ordinary", "noop", "count", "reject-last"][mode];
                        println!(
                            "{{\"case\":{case},\"hot\":{hot},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"characters\":{},\"calls\":{calls},\"work\":{work},\"allocation\":{allocation},\"rejected\":{}}}",
                            sample - WARMUP,
                            text.len(),
                            mode == 3
                        );
                    }
                }
            }
        }
    }
}
