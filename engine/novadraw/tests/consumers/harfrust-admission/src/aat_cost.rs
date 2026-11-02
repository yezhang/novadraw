//! Empty AAT cache construction; successful drop is outside the timer.
#[allow(dead_code)]
mod cold_fixtures;
use harfrust::{ColdOperationCost, FontRef, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    for (name, tag, header) in [
        ("ot_kern", *b"kern", vec![0, 0, 0, 0]),
        ("aat_kern", *b"kern", vec![0, 1, 0, 0, 0, 0, 0, 0]),
        ("morx", *b"morx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
        ("kerx", *b"kerx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
    ] {
        let bytes = cold_fixtures::sfnt(&[(tag, &header)], true);
        let font = FontRef::new(&bytes).unwrap();
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
            for offset in 0..4 {
                let mode = (sample + offset) % 4;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = match mode {
                    0 => Some(ShaperData::new(black_box(&font))),
                    1 => ShaperData::try_new_admitted(black_box(&font), |_, _| Ok::<_, usize>(()))
                        .unwrap(),
                    _ => ShaperData::try_new_admitted(
                        black_box(&font),
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
                        sample - WARMUP
                    );
                }
                drop(result);
            }
        }
    }
}
