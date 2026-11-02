//! Same ordinary cache/shaping observations against original and patched code.
mod cold_fixtures;
mod lookup_fixtures;
use harfrust::{Direction, FontRef, SerializeFlags, ShaperData, UnicodeBuffer};

fn main() {
    let mut fonts = cold_fixtures::fonts();
    fonts.extend(
        lookup_fixtures::cases()
            .into_iter()
            .map(|(subst, kind, subtable)| lookup_fixtures::font(subst, kind, &subtable, 10)),
    );
    for (tag, header) in [
        (*b"kern", vec![0, 0, 0, 0]),
        (*b"kern", vec![0, 1, 0, 0, 0, 0, 0, 0]),
        (*b"morx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
        (*b"kerx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
    ] {
        for prefix in 0..=header.len() {
            fonts.push(cold_fixtures::sfnt(&[(tag, &header[..prefix])], true));
        }
        let mut table = header;
        table.extend([0xff; 1024]);
        fonts.push(cold_fixtures::sfnt(&[(tag, &table)], false));
    }
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(bytes.to_vec());
    }
    for (font_index, bytes) in fonts.iter().enumerate() {
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
