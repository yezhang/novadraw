//! Same ordinary Unicode shaping calls in original and production.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "simple_unicode_fixtures.rs"]
mod fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, script};
use std::{hint::black_box, time::Instant};

fn main() {
    for cmap in 0..3 {
        let text = if cmap == 0 {
            "BCé中中(".repeat(512)
        } else {
            "BCé(".repeat(768)
        };
        for (script_case, script) in [("native", Some(script::HAN)), ("absent", None)] {
            for (case, substitutions) in [("simple", false), ("staged", true)] {
                let bytes = fixtures::font(3, cmap, 0, substitutions);
                for hot in [false, true] {
                    let name = format!(
                        "{cmap}-{script_case}-{case}/{}",
                        if hot { "hot" } else { "cold" }
                    );
                    for sample in 0..1100 {
                        let font = FontRef::new(&bytes).unwrap();
                        let data = ShaperData::new(&font);
                        let shaper = data.shaper(&font).build();
                        let plan =
                            ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                        let make = || {
                            fixtures::input(
                                &text,
                                Direction::RightToLeft,
                                script,
                                0,
                                fixtures::LEVELS[3],
                                0,
                            )
                        };
                        if hot {
                            shaper.shape_with_plan(&plan, make(), &[]);
                        }
                        let buffer = make();
                        let start = Instant::now();
                        let output = shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                        let ns = start.elapsed().as_nanos();
                        assert_eq!(
                            output.len(),
                            if substitutions {
                                if cmap == 0 { 5120 } else { 5376 }
                            } else {
                                3072
                            }
                        );
                        black_box(output);
                        if sample >= 100 {
                            println!(
                                "{{\"font\":\"{name}\",\"sample\":{},\"ns\":{ns}}}",
                                sample - 100
                            );
                        }
                    }
                }
            }
        }
    }
}
