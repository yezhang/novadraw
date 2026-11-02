//! Identical ordinary public calls compiled against original and production.

mod cold_fixtures;
use harfrust::{Direction, FontRef, SerializeFlags, ShaperData, UnicodeBuffer};

fn main() {
    let fonts = cold_fixtures::fonts();
    let real = [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ];
    for (font_index, bytes) in fonts.iter().map(Vec::as_slice).chain(real).enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for pass in 0..2 {
            for text in ["", "office ffi", "é a\u{301}", "中文测试", "السَّلَامُ", "a中🙂"]
            {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let mut buffer = UnicodeBuffer::new();
                    buffer.push_str(text);
                    buffer.guess_segment_properties();
                    buffer.set_direction(direction);
                    let output = shaper.shape(buffer, &[]);
                    println!(
                        "{font_index}/{pass}/{text:?}/{direction:?}/{} {:?} {:?} {}",
                        shaper.units_per_em(),
                        output.glyph_infos(),
                        output.glyph_positions(),
                        output.serialize(&shaper, SerializeFlags::empty())
                    );
                }
            }
        }
    }
}
