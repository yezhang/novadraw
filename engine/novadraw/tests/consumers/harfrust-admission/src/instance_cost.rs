//! Repeated fresh normalized-instance construction; no Core or GPU timing.
use harfrust::{FontRef, InstanceOperationCost, NormalizedCoord, ShaperInstance};
use std::{hint::black_box, time::Instant};

mod instance_fixtures;

fn main() {
    const SAMPLES_PER_MODE: usize = 1000;
    const WARMUP_PER_MODE: usize = 100;
    let fixtures = instance_fixtures::fonts();
    let fonts = [
        (
            "inter",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        ),
        ("axes12", fixtures[38].as_slice()),
        ("axes64", fixtures[50].as_slice()),
    ];
    let coords = [NormalizedCoord::from_f32(0.5); 65];
    for (font_name, bytes) in fonts {
        let font = FontRef::new(bytes).unwrap();
        let mut last_call = 0;
        let _ = ShaperInstance::try_from_coords_admitted(&font, &coords, |_, _| {
            last_call += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
        for sample in 0..SAMPLES_PER_MODE + WARMUP_PER_MODE {
            // Rotate modes deterministically to reduce fixed ordering bias.
            for offset in 0..3 {
                let mode = (sample + offset) % 3;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = if mode == 0 {
                    Some(ShaperInstance::from_coords(
                        black_box(&font),
                        black_box(&coords).iter().copied(),
                    ))
                } else {
                    ShaperInstance::try_from_coords_admitted(
                        black_box(&font),
                        black_box(&coords),
                        |_, cost: InstanceOperationCost| {
                            calls += 1;
                            if mode == 2 && calls == last_call {
                                return Err(calls);
                            }
                            work += cost.work;
                            allocation += cost.allocation;
                            Ok(())
                        },
                    )
                    .ok()
                };
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.is_some(), mode != 2);
                if mode != 0 {
                    assert_eq!(calls, last_call);
                }
                black_box(&result);
                if sample >= WARMUP_PER_MODE {
                    let mode = ["ordinary", "admitted", "reject_last"][mode];
                    println!(
                        "{{\"font\":\"{font_name}\",\"sample\":{},\"mode\":\"{mode}\",\
                         \"ns\":{ns},\"work\":{work},\"allocation\":{allocation},\
                         \"calls\":{calls}}}",
                        sample - WARMUP_PER_MODE
                    );
                }
                // Success drops its returned instance outside the timer;
                // rejection includes candidate destruction inside the call.
                drop(result);
            }
        }
    }
}
