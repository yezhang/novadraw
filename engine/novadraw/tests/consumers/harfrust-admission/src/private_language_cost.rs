//! Failed candidate cleanup is inside the plan-construction timer.
use harfrust::{ColdOperationCost, Direction, FontRef, Language, ShapePlan, ShaperData, script};
use std::{hint::black_box, str::FromStr, time::Instant};

fn main() {
    const WARMUP: usize = 100;
    const SAMPLES: usize = 1000;
    let font = FontRef::new(include_bytes!("../../../../assets/fonts/InterVariable.ttf")).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for (name, text) in [
        ("inline", String::from("x-hbot1")),
        ("spill", String::from("en-x-hbsclatn-hboteng")),
        ("long", format!("en-x-{}-hboteng", "a".repeat(1024))),
    ] {
        let language = Language::from_str(&text).unwrap();
        let mut last = 0;
        ShapePlan::try_new_admitted(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            Some(&language),
            &[],
            |_, _| {
                last += 1;
                Ok::<_, usize>(())
            },
        )
        .unwrap()
        .unwrap();
        for sample in 0..WARMUP + SAMPLES {
            for offset in 0..4 {
                let mode = (sample + offset) % 4;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = if mode == 0 {
                    Some(ShapePlan::new(
                        black_box(&shaper),
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        Some(&language),
                        &[],
                    ))
                } else {
                    ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        Some(&language),
                        &[],
                        |_, cost: ColdOperationCost| {
                            if mode == 1 {
                                return Ok(());
                            }
                            calls += 1;
                            if mode == 3 && calls == last {
                                return Err(calls);
                            }
                            work += cost.work;
                            allocation += cost.allocation;
                            Ok(())
                        },
                    )
                    .ok()
                    .flatten()
                };
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.is_some(), mode != 3);
                if mode >= 2 {
                    assert_eq!(calls, last);
                }
                black_box(&result);
                if sample >= WARMUP {
                    let mode = ["ordinary", "noop", "counted", "reject_last"][mode];
                    println!(
                        "{{\"font\":\"{name}\",\"mode\":\"{mode}\",\"sample\":{},\"ns\":{ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls}}}",
                        sample - WARMUP,
                    );
                }
                drop(result);
            }
        }
    }
}
