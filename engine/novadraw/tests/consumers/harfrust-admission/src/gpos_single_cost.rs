//! Ordinary, accepted and rejected single-positioning costs.
#![allow(clippy::duplicate_mod)]
#[path = "gpos_single_fixtures.rs"]
mod fixtures;
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
    let text = "ABC".repeat(1024);
    for substitutions in [false, true] {
        let bytes = fixtures::font(2, 2, 3, 2, 15, substitutions);
        for hot in [false, true] {
            for spare in [0, 16384] {
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
                for sample in 0..1100 {
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
                        if sample >= 100 {
                            let path = ["ordinary", "noop", "count", "reject-last"][mode];
                            println!(
                                "{{\"substitutions\":{substitutions},\"hot\":{hot},\"spare\":{spare},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"characters\":{},\"glyphs\":{glyphs},\"calls\":{calls},\"work\":{work},\"allocation\":{allocation},\"rejected\":{}}}",
                                sample - 100,
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
