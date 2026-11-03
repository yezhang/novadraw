//! Ordinary/noop/count/final rejection for missing singleton normalization.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "normalization_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use harfrust::{Direction, FontRef, ShapeOperation, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABKΩ\u{f900}。C🙂\u{0378}DEF".repeat(128);
    assert_eq!(text.chars().count(), 1536);
    for kind in [0, 3, 6] {
        for case in [4, 8] {
            let bytes = fixtures::font(kind, case, 0);
            for (direction_case, direction) in
                [(0, Direction::LeftToRight), (1, Direction::RightToLeft)]
            {
                for hot in [false, true] {
                    for spare in [0, 16384] {
                        let make = || {
                            fixtures::input(&text, direction, None, 0, fixtures::LEVELS[3], spare)
                        };
                        let font = FontRef::new(&bytes).unwrap();
                        let data = ShaperData::new(&font);
                        let shaper = data.shaper(&font).build();
                        let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                        if hot {
                            shaper.shape_with_plan(&plan, make(), &[]);
                        }
                        let mut complete_calls = 0;
                        let glyphs = shaper
                            .try_shape_with_plan_admitted(&plan, &mut make(), &[], |_, _| {
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
                                let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                                if hot {
                                    shaper.shape_with_plan(&plan, make(), &[]);
                                }
                                let mut buffer = make();
                                let (mut work, mut allocation, mut calls, mut unicode_work) =
                                    (0, 0, 0, 0);
                                let start = Instant::now();
                                let rejected = if mode == 0 {
                                    let output =
                                        shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                                    assert_eq!(output.len(), glyphs);
                                    let _ = black_box(output);
                                    false
                                } else {
                                    let output = shaper.try_shape_with_plan_admitted(
                                        black_box(&plan),
                                        &mut buffer,
                                        &[],
                                        |op, cost| {
                                            if mode == 1 {
                                                return Ok(());
                                            }
                                            calls += 1;
                                            if mode == 3 && calls == complete_calls {
                                                return Err(());
                                            }
                                            work += cost.work;
                                            allocation += cost.allocation;
                                            if op == ShapeOperation::Unicode {
                                                assert_eq!(cost.allocation, 0);
                                                unicode_work += cost.work;
                                            }
                                            Ok(())
                                        },
                                    );
                                    let rejected = output.is_err();
                                    if rejected {
                                        assert_eq!(buffer.len(), 0);
                                    } else {
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
                                        "{{\"kind\":{kind},\"case\":{case},\"direction\":{direction_case},\"hot\":{hot},\"spare\":{spare},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls},\"unicode_work\":{unicode_work},\"glyphs\":{glyphs},\"rejected\":{rejected}}}",
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
