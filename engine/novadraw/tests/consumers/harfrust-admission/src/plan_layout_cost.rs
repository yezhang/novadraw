//! Plan-only sampling. Failed candidate cleanup remains inside the timer.
#[allow(dead_code)]
mod plan_layout_fixtures;
use harfrust::{
    ColdOperationCost, Direction, Feature, FontRef, NormalizedCoord, ShapePlan, ShaperData,
    ShaperInstance, script,
};
use std::{hint::black_box, str::FromStr, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let inputs = [
        ("struct_gsub", plan_layout_fixtures::font(true, false, true)),
        ("struct_gpos", plan_layout_fixtures::font(false, true, true)),
        (
            "inter_variable",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
        ),
        (
            "cjk_default",
            include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        ),
    ];
    let features: Vec<_> = ["kern=1", "liga=1", "abcd=2", "rand=1", "liga[1:4]=0"]
        .iter()
        .map(|text| Feature::from_str(text).unwrap())
        .collect();
    for (name, bytes) in inputs {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let coords = if name == "cjk_default" {
            vec![]
        } else {
            vec![NormalizedCoord::from_f32(0.5)]
        };
        let instance = ShaperInstance::from_coords(&font, coords);
        let shaper = data.shaper(&font).instance(Some(&instance)).build();
        let mut last = 0;
        ShapePlan::try_new_admitted(
            &shaper,
            Direction::LeftToRight,
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
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &features,
                    )),
                    1 => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &features,
                        |_, _| Ok::<_, usize>(()),
                    )
                    .unwrap(),
                    _ => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        Direction::LeftToRight,
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
