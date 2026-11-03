//! Identical ordinary public API source for original and production plans.
mod script_callbacks_fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};
use std::{hint::black_box, time::Instant};

fn main() {
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
        for sample in 0..1100 {
            let start = Instant::now();
            let plan = ShapePlan::new(
                black_box(&shaper),
                Direction::RightToLeft,
                Some(script),
                None,
                &[],
            );
            let ns = start.elapsed().as_nanos();
            black_box(&plan);
            if sample >= 100 {
                println!(
                    "{{\"font\":\"{name}\",\"sample\":{},\"ns\":{ns}}}",
                    sample - 100,
                );
            }
            drop(plan);
        }
    }
}
