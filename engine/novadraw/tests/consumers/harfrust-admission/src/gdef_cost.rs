//! Same GDEF inputs in ordinary/noop/count/final-rejection modes.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "gdef_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapeOperation, ShapePlan, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABC".repeat(512);
    for kind in [0, 3, 6] {
        for case in [4, 8] {
            let bytes = fixtures::font(kind, case);
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
                                let (mut work, mut allocation, mut calls, mut property_work) =
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
                                            if op == ShapeOperation::GlyphProperties {
                                                assert_eq!(cost.allocation, 0);
                                                property_work += cost.work;
                                            }
                                            Ok(())
                                        },
                                    );
                                    let rejected = output.is_err();
                                    if !rejected {
                                        assert_eq!(
                                            output.as_ref().unwrap().as_ref().unwrap().len(),
                                            glyphs
                                        );
                                    } else {
                                        assert_eq!(buffer.len(), 0);
                                    }
                                    let _ = black_box(output);
                                    rejected
                                };
                                let elapsed_ns = start.elapsed().as_nanos();
                                if sample >= 100 {
                                    let path = ["ordinary", "noop", "count", "reject-last"][mode];
                                    println!(
                                        "{{\"kind\":{kind},\"case\":{case},\"direction\":{direction_case},\"hot\":{hot},\"spare\":{spare},\"path\":\"{path}\",\"sample\":{},\"elapsed_ns\":{elapsed_ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls},\"property_work\":{property_work},\"glyphs\":{glyphs},\"rejected\":{rejected}}}",
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
