//! Ordinary original/production ASCII RTL with both reversal locations.
#![allow(clippy::duplicate_mod, clippy::module_inception)]
#[path = "rtl_fixtures.rs"]
mod fixtures;
use harfrust::{BufferFlags, Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    let text = "ABC(".repeat(768);
    for (script_case, script) in [("native", Some(script::LATIN)), ("absent", None)] {
        for (case, substitutions) in [("simple", false), ("staged", true)] {
            let bytes = fixtures::font(3, false, substitutions);
            for hot in [false, true] {
                let name = format!("{script_case}-{case}/{}", if hot { "hot" } else { "cold" });
                for sample in 0..1100 {
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(&shaper, Direction::RightToLeft, script, None, &[]);
                    let make = || {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(&text);
                        if let Some(script) = script {
                            buffer.set_script(script);
                        }
                        buffer.set_direction(Direction::RightToLeft);
                        buffer.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
                        buffer
                    };
                    if hot {
                        shaper.shape_with_plan(&plan, make(), &[]);
                    }
                    let buffer = make();
                    let start = Instant::now();
                    let output = shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                    let ns = start.elapsed().as_nanos();
                    assert_eq!(output.len(), if substitutions { 5376 } else { 3072 });
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
