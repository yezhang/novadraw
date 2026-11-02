//! Fresh borrowed configuration; cached data/instance construction excluded.
use harfrust::{ColdOperationCost, FontRef, NormalizedCoord, ShaperData, ShaperInstance};
use std::{hint::black_box, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    for (name, bytes, coords) in [
        (
            "inter_default",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
            vec![],
        ),
        (
            "inter_variable",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
            vec![NormalizedCoord::from_bits(8192); 16],
        ),
        (
            "arabic_variable",
            include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
            vec![NormalizedCoord::from_bits(8192); 16],
        ),
        (
            "cjk_default",
            include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
            vec![],
        ),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let instance = ShaperInstance::from_coords(&font, coords);
        let mut last = 0;
        data.shaper(&font)
            .instance(Some(&instance))
            .try_build_admitted(|_, _| {
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
                let builder = black_box(&data)
                    .shaper(black_box(&font))
                    .instance(Some(black_box(&instance)));
                let result = match mode {
                    0 => Some(builder.build()),
                    1 => builder
                        .try_build_admitted(|_, _| Ok::<_, usize>(()))
                        .unwrap(),
                    _ => builder
                        .try_build_admitted(|_, cost: ColdOperationCost| {
                            calls += 1;
                            if mode == 3 && calls == last {
                                return Err(calls);
                            }
                            work += cost.work;
                            allocation += cost.allocation;
                            Ok(())
                        })
                        .ok()
                        .flatten(),
                };
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.is_some(), mode != 3);
                if mode >= 2 {
                    assert_eq!(calls, last)
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
