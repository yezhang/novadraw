//! Fresh cache construction only; no shape plan or glyph shaping is timed.
use harfrust::{ColdOperationCost, FontRef, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    for (name, bytes) in [
        (
            "inter",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        ),
        (
            "arabic",
            include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        ),
        (
            "cjk",
            include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
        ),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let mut last = 0;
        assert!(
            ShaperData::try_new_admitted(&font, |_, _| {
                last += 1;
                Ok::<_, usize>(())
            })
            .unwrap()
            .is_some()
        );
        for sample in 0..SAMPLES + WARMUP {
            for offset in 0..3 {
                let mode = (sample + offset) % 3;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = if mode == 0 {
                    Some(ShaperData::new(black_box(&font)))
                } else {
                    ShaperData::try_new_admitted(black_box(&font), |_, cost: ColdOperationCost| {
                        calls += 1;
                        if mode == 2 && calls == last {
                            return Err(calls);
                        }
                        work += cost.work;
                        allocation += cost.allocation;
                        Ok(())
                    })
                    .ok()
                    .flatten()
                };
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.is_some(), mode != 2);
                if mode != 0 {
                    assert_eq!(calls, last);
                }
                black_box(&result);
                if sample >= WARMUP {
                    let mode = ["ordinary", "admitted", "reject_last"][mode];
                    println!(
                        "{{\"font\":\"{name}\",\"mode\":\"{mode}\",\"sample\":{},\
                        \"ns\":{ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls}}}",
                        sample - WARMUP
                    );
                }
                drop(result);
            }
        }
    }
}
