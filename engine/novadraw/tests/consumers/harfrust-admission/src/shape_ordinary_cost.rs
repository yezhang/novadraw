//! Same ordinary shaping API for original/production overhead comparison.
mod shape_fixtures;
use harfrust::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script};
use std::{hint::black_box, time::Instant};

fn main() {
    let fonts = shape_fixtures::fonts();
    let text = "office ffi 123 ".repeat(64);
    for (font_index, bytes) in fonts[fonts.len() - 2..].iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        for spare in [false, true] {
            let name = ["inter", "cjk"][font_index];
            let name = format!("{name}/{}", if spare { "spare" } else { "default" });
            let mut input = UnicodeBuffer::new();
            for sample in 0..1100 {
                input.clear();
                if spare {
                    assert!(input.reserve(8192));
                }
                input.push_str(&text);
                input.set_script(script::LATIN);
                input.set_direction(Direction::LeftToRight);
                let start = Instant::now();
                let buffer = std::mem::replace(&mut input, UnicodeBuffer::new());
                let result = shaper.shape_with_plan(black_box(&plan), buffer, &[]);
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.len(), text.len());
                black_box(&result);
                input = result.clear();
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
