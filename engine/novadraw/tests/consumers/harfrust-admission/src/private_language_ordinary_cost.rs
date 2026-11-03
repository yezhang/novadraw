//! Ordinary API only, compiled unchanged against original and production.
use harfrust::{Direction, FontRef, Language, ShapePlan, ShaperData, script};
use std::{hint::black_box, str::FromStr, time::Instant};

fn main() {
    let font = FontRef::new(include_bytes!("../../../../assets/fonts/InterVariable.ttf")).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for (name, text) in [
        ("inline", String::from("x-hbot1")),
        ("spill", String::from("en-x-hbsclatn-hboteng")),
        ("long", format!("en-x-{}-hboteng", "a".repeat(1024))),
    ] {
        let language = Language::from_str(&text).unwrap();
        for sample in 0..1100 {
            let start = Instant::now();
            let plan = ShapePlan::new(
                black_box(&shaper),
                Direction::LeftToRight,
                Some(script::LATIN),
                Some(black_box(&language)),
                &[],
            );
            let ns = start.elapsed().as_nanos();
            black_box(&plan);
            if sample >= 100 {
                println!(
                    "{{\"font\":\"{name}\",\"sample\":{},\"ns\":{ns}}}",
                    sample - 100
                );
            }
            drop(plan);
        }
    }
}
