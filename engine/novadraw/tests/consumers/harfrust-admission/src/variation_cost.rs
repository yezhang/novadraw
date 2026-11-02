//! Fresh user-space instance construction with rotated ordinary/admitted/reject modes.
use harfrust::{FontRef, InstanceOperationCost, ShaperInstance, Tag, Variation};
use std::{hint::black_box, time::Instant};

mod variation_fixtures;

fn main() {
    const SAMPLES: usize = 1000;
    const WARMUP: usize = 100;
    let mapping = variation_fixtures::avar(
        0x0001_0000,
        0,
        &[&[(-16384, -16384), (0, 8192), (16384, 16384)]],
    );
    let bytes12 = variation_fixtures::font(12, true, &mapping, true);
    let bytes64 = variation_fixtures::font(64, true, &mapping, true);
    let fonts = [
        (
            "inter",
            include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        ),
        ("axes12", bytes12.as_slice()),
        ("axes64", bytes64.as_slice()),
    ];
    let input = [
        Variation {
            tag: Tag::new(b"wght"),
            value: 600.0,
        },
        Variation {
            tag: Tag::new(&0u32.to_be_bytes()),
            value: 0.5,
        },
        Variation {
            tag: Tag::new(&0u32.to_be_bytes()),
            value: -0.25,
        },
    ];
    for (name, bytes) in fonts {
        let font = FontRef::new(bytes).unwrap();
        let mut last = 0;
        ShaperInstance::try_from_variations_admitted(&font, &input, |_, _| {
            last += 1;
            Ok::<_, usize>(())
        })
        .unwrap()
        .unwrap();
        for sample in 0..SAMPLES + WARMUP {
            for offset in 0..3 {
                let mode = (sample + offset) % 3;
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = if mode == 0 {
                    Some(ShaperInstance::from_variations(
                        black_box(&font),
                        black_box(&input).iter().copied(),
                    ))
                } else {
                    ShaperInstance::try_from_variations_admitted(
                        black_box(&font),
                        black_box(&input),
                        |_, cost: InstanceOperationCost| {
                            calls += 1;
                            if mode == 2 && calls == last {
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
                        sample - WARMUP,
                    );
                }
                drop(result);
            }
        }
    }
}
