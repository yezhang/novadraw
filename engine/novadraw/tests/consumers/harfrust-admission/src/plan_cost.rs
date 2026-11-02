//! Fresh plan construction; successful destruction is outside the timer.
#[allow(dead_code)]
mod cold_fixtures;
use harfrust::{ColdOperationCost, Direction, Feature, FontRef, ShapePlan, ShaperData, script};
use std::{hint::black_box, str::FromStr, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let bytes = cold_fixtures::sfnt(&[], true);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let features: Vec<_> = [
        "kern=0",
        "trak=0",
        "kern=1",
        "liga[1:4]=0",
        "abcd=4",
        "rand=2",
    ]
    .iter()
    .map(|s| Feature::from_str(s).unwrap())
    .collect();
    for (name, direction, features) in [
        ("empty_ltr", Direction::LeftToRight, vec![]),
        ("inline_rtl", Direction::RightToLeft, features[..4].to_vec()),
        ("spill_ltr", Direction::LeftToRight, features.clone()),
        ("many_vertical", Direction::TopToBottom, features.repeat(12)),
    ] {
        let mut last = 0;
        ShapePlan::try_new_admitted(
            &shaper,
            direction,
            Some(script::LATIN),
            None,
            &features,
            |_, _| {
                last += 1;
                Ok::<_, usize>(())
            },
        )
        .unwrap()
        .unwrap();
        for sample in 0..SAMPLES + WARMUP {
            for offset in 0..4 {
                let mode = (sample + offset) % 4;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = match mode {
                    0 => Some(ShapePlan::new(
                        black_box(&shaper),
                        direction,
                        Some(script::LATIN),
                        None,
                        &features,
                    )),
                    1 => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        direction,
                        Some(script::LATIN),
                        None,
                        &features,
                        |_, _| Ok::<_, usize>(()),
                    )
                    .unwrap(),
                    _ => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        direction,
                        Some(script::LATIN),
                        None,
                        &features,
                        |_, cost: ColdOperationCost| {
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
                    .flatten(),
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
                        "{{\"font\":\"{name}\",\"mode\":\"{mode}\",\"sample\":{},\
                        \"ns\":{ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls}}}",
                        sample - WARMUP,
                    );
                }
                drop(result);
            }
        }
    }
}
