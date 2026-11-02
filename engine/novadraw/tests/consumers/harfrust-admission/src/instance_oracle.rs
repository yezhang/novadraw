//! Identical ordinary normalized-instance oracle for original/production builds.
use harfrust::{FontRef, NormalizedCoord, ShaperData, ShaperInstance, UnicodeBuffer};

mod instance_fixtures;

fn main() {
    let mut fonts = instance_fixtures::fonts();
    let table_fonts = fonts.len();
    fonts.extend([
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    ]);
    for (index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let mut reused = ShaperInstance::from_coords(&font, [NormalizedCoord::from_f32(0.5)]);
        for length in [0, 1, 11, 12, 64, 65] {
            for value in [-0.75, 0.0, 0.5] {
                let coords = vec![NormalizedCoord::from_f32(value); length];
                let instance = ShaperInstance::from_coords(&font, coords.iter().copied());
                reused.set_coords(&font, coords.iter().copied());
                assert_eq!(format!("{reused:?}"), format!("{instance:?}"));
                println!("{index}|{length}|{value}|{instance:?}");
                if index < table_fonts {
                    continue;
                }
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).instance(Some(&instance)).build();
                for text in ["office a\u{301}", "中文测试", "السَّلَامُ"] {
                    let mut buffer = UnicodeBuffer::new();
                    buffer.push_str(text);
                    buffer.guess_segment_properties();
                    let output = shaper.shape(buffer, &[]);
                    println!(
                        "{text}|{:?}|{:?}",
                        output.glyph_infos(),
                        output.glyph_positions()
                    );
                }
            }
        }
    }
}
