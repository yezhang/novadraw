//! Direction conversion and final reverse costs with optional GSUB expansion.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "rtl_fixtures.rs"]
mod fixtures;
use harfrust::{
    BufferFlags, Direction, FontRef, Script, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::{hint::black_box, time::Instant};

fn input(text: &str, spare: usize, script: Option<Script>) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    buffer.push_str(text);
    if let Some(script) = script {
        buffer.set_script(script);
    }
    buffer.set_direction(Direction::RightToLeft);
    buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}

fn main() {
    let text = "ABC(".repeat(768);
    for (script_case, script) in [(0, Some(script::LATIN)), (1, None)] {
        for substitutions in [false, true] {
            let bytes = fixtures::font(3, false, substitutions);
            for hot in [false, true] {
                for spare in [0, 16384] {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                    if hot {
                        shaper.shape_with_plan(&plan, input(&text, spare, script), &[]);
                    }
                    let mut complete_calls = 0;
                    let glyphs = shaper
                        .try_shape_with_plan_admitted(
                            &plan,
                            &mut input(&text, spare, script),
                            &[],
                            |_, _| {
                                complete_calls += 1;
                                Ok::<_, ()>(())
                            },
                        )
                        .unwrap()
                        .unwrap()
                        .len();
                    for sample in 0..1100 {
                        for offset in 0..4 {
                            let mode = (sample + offset) % 4;
                            let font = FontRef::new(&bytes).unwrap();
                            let data = ShaperData::new(&font);
                            let shaper = data.shaper(&font).build();
                            let plan =
                                ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                            if hot {
                                shaper.shape_with_plan(&plan, input(&text, spare, script), &[]);
                            }
                            let mut buffer = input(&text, spare, script);
                            let (mut work, mut allocation, mut calls) = (0, 0, 0);
                            let start = Instant::now();
                            let rejected = if mode == 0 {
                                let output = shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                                assert_eq!(output.len(), glyphs);
                                let _ = black_box(output);
                                false
                            } else {
                                let output = shaper.try_shape_with_plan_admitted(
                                    black_box(&plan),
                                    &mut buffer,
                                    &[],
                                    |_, cost| {
                                        if mode == 1 {
                                            return Ok(());
                                        }
                                        calls += 1;
                                        if mode == 3 && calls == complete_calls {
                                            return Err(());
                                        }
                                        work += cost.work;
                                        allocation += cost.allocation;
                                        Ok(())
                                    },
                                );
                                let rejected = output.is_err();
                                if !rejected {
                                    assert_eq!(
                                        output.as_ref().unwrap().as_ref().unwrap().len(),
                                        glyphs
                                    );
                                }
                                let _ = black_box(output);
                                rejected
                            };
                            let elapsed_ns = start.elapsed().as_nanos();
                            if sample >= 100 {
                                let path = ["ordinary", "noop", "count", "reject-last"][mode];
                                println!(
                                    "{{\"script_case\":{script_case},\"substitutions\":{substitutions},\"hot\":{hot},\"spare\":{spare},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls},\"glyphs\":{glyphs},\"rejected\":{rejected}}}",
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
