//! Actual layout-free ASCII shaping overhead, not Core layout or width reuse.
mod shape_fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let fonts = shape_fixtures::fonts();
    let text = "office ffi 123 ".repeat(64);
    for (font_index, bytes) in fonts[fonts.len() - 2..].iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        for spare in [false, true] {
            let prepare = |buffer: &mut UnicodeBuffer| {
                buffer.clear();
                if spare {
                    assert!(buffer.reserve(8192));
                }
                buffer.push_str(&text);
                buffer.set_script(script::LATIN);
                buffer.set_direction(Direction::LeftToRight);
            };
            let mut buffer = UnicodeBuffer::new();
            prepare(&mut buffer);
            let mut complete_calls = 0;
            shaper
                .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |_, _| {
                    complete_calls += 1;
                    Ok::<_, ()>(())
                })
                .unwrap()
                .unwrap();
            let last = complete_calls - 1;
            // Fresh buffers keep input construction outside each timing.
            // Rotation avoids measuring all ordinary calls before admitted calls.
            for sample in 0..SAMPLES + WARMUP {
                for offset in 0..4 {
                    let mode = (sample + offset) % 4;
                    let path = ["ordinary", "noop", "count", "reject-last"][mode];
                    prepare(&mut buffer);
                    let mut calls = 0;
                    let mut work = 0;
                    let mut allocation = 0;
                    let start = Instant::now();
                    let output = if mode == 0 {
                        let input = std::mem::replace(&mut buffer, UnicodeBuffer::new());
                        Ok(Some(shaper.shape_with_plan(black_box(&plan), input, &[])))
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
                                if mode == 3 && index == last {
                                    return Err(());
                                }
                                work += cost.work;
                                allocation += cost.allocation;
                                Ok(())
                            },
                        )
                    };
                    let elapsed = start.elapsed().as_nanos();
                    assert_eq!(output.is_err(), mode == 3);
                    if mode >= 2 {
                        assert_eq!(calls, complete_calls);
                    }
                    if let Ok(Some(output)) = output {
                        assert_eq!(output.len(), text.len());
                        black_box(&output);
                        buffer = output.clear();
                    } else {
                        assert!(buffer.is_empty());
                    }
                    if sample >= WARMUP {
                        println!(
                            "{{\"path\":\"{path}\",\"font\":{font_index},\"spare\":{spare},\
                            \"sample\":{},\"elapsed_ns\":{elapsed},\"characters\":{},\
                            \"calls\":{calls},\"work\":{work},\"allocation\":{allocation},\
                            \"rejected\":{}}}",
                            sample - WARMUP,
                            text.len(),
                            mode == 3,
                        );
                    }
                }
            }
        }
    }
}
