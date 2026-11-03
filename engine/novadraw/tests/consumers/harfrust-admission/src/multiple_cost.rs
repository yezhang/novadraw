//! Multiple-substitution overhead, including fresh and spare buffer storage.
#![allow(clippy::duplicate_mod)]
mod multiple_fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn input(text: &str, spare: usize) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    buffer.push_str(text);
    buffer.set_script(script::LATIN);
    buffer.set_direction(Direction::LeftToRight);
    buffer
}

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let text = "ABCAB other ".repeat(80);
    for (case, (maximum, coverage, subtables, lookups)) in
        [(2, 1, 1, 1), (3, 2, 3, 2)].into_iter().enumerate()
    {
        let bytes = multiple_fixtures::font_with_layout(&multiple_fixtures::layout(
            &multiple_fixtures::sequences(maximum),
            coverage,
            subtables,
            lookups,
        ));
        for hot in [false, true] {
            for spare in [0, 4096] {
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
                    shaper.shape_with_plan(&plan, input(&text, spare), &[]);
                }
                let mut complete_calls = 0;
                let glyphs = shaper
                    .try_shape_with_plan_admitted(&plan, &mut input(&text, spare), &[], |_, _| {
                        complete_calls += 1;
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                    .unwrap()
                    .len();
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
                            shaper.shape_with_plan(&plan, input(&text, spare), &[]);
                        }
                        let mut buffer = input(&text, spare);
                        let (mut calls, mut work, mut allocation) = (0, 0, 0);
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
                            assert_eq!(result.len(), glyphs);
                            black_box(result);
                        } else {
                            assert!(buffer.is_empty());
                        }
                        if sample >= WARMUP {
                            let path = ["ordinary", "noop", "count", "reject-last"][mode];
                            println!(
                                "{{\"case\":{case},\"hot\":{hot},\"spare\":{spare},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"characters\":{},\"glyphs\":{glyphs},\"calls\":{calls},\"work\":{work},\"allocation\":{allocation},\"rejected\":{}}}",
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
}
