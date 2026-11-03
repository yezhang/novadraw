//! Ordinary API only, compiled unchanged against original and production.
#[allow(dead_code)]
mod plan_layout_fixtures;
use harfrust::{
    Direction, Feature, FontRef, NormalizedCoord, ShapePlan, ShaperData, ShaperInstance, script,
};
use std::{hint::black_box, str::FromStr, time::Instant};

fn main() {
    let features: Vec<_> = ["kern=1", "liga=1", "abcd=2", "rand=1", "liga[1:4]=0"]
        .iter()
        .map(|text| Feature::from_str(text).unwrap())
        .collect();
    for (name, bytes) in [
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
    ] {
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let coords = if name == "cjk_default" {
            vec![]
        } else {
            vec![NormalizedCoord::from_f32(0.5)]
        };
        let instance = ShaperInstance::from_coords(&font, coords);
        let shaper = data.shaper(&font).instance(Some(&instance)).build();
        for sample in 0..1100 {
            let start = Instant::now();
            let plan = ShapePlan::new(
                black_box(&shaper),
                Direction::LeftToRight,
                Some(script::LATIN),
                None,
                black_box(&features),
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
