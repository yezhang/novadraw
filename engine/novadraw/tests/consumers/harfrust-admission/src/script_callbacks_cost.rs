//! Fresh fixed-script plans; successful destruction is outside the timer.
use harfrust::{ColdOperationCost, Direction, FontRef, ShapePlan, ShaperData, script};
use std::{hint::black_box, time::Instant};
mod script_callbacks_fixtures;

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let syriac = script_callbacks_fixtures::syriac_font();
    for (name, bytes, script) in [
        (
            "arabic",
            include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
            script::ARABIC,
        ),
        ("syriac", syriac.as_slice(), script::SYRIAC),
        (
            "hangul",
            include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
            script::HANGUL,
        ),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let direction = Direction::RightToLeft;
        let mut last = 0;
        ShapePlan::try_new_admitted(&shaper, direction, Some(script), None, &[], |_, _| {
            last += 1;
            Ok::<_, usize>(())
        })
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
                        Some(script),
                        None,
                        &[],
                    )),
                    1 => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        direction,
                        Some(script),
                        None,
                        &[],
                        |_, _| Ok::<_, usize>(()),
                    )
                    .unwrap(),
                    _ => ShapePlan::try_new_admitted(
                        black_box(&shaper),
                        direction,
                        Some(script),
                        None,
                        &[],
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
