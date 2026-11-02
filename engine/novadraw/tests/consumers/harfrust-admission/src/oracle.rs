//! Shared ordinary-path oracle compiled against original and production sources.
use harfrust::{Direction, FontRef, ShaperData, ShaperInstance, UnicodeBuffer};

fn main() {
    let fonts: [&[u8]; 3] = [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf"),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf"),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf"),
    ];
    let texts = ["", "office ffi", "a\u{301} é", "中文测试", "السَّلَامُ", "a中🙂"];
    for (font_index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        for coords in [&[][..], &[harfrust::NormalizedCoord::from_f32(0.5)][..]] {
            let instance = ShaperInstance::from_coords(&font, coords.iter().copied());
            let shaper = data.shaper(&font).instance(Some(&instance)).build();
            for (text_index, text) in texts.iter().enumerate() {
                for repeats in [1, 8, 64] {
                    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                        let mut buffer = UnicodeBuffer::new();
                        // Reserve and repeated input test both ordinary shared kernels.
                        assert!(buffer.reserve(text.len() * repeats));
                        for _ in 0..repeats {
                            buffer.push_str(text);
                        }
                        buffer.add('x', u32::MAX);
                        buffer.set_pre_context("a中");
                        buffer.set_post_context("é🙂");
                        buffer.guess_segment_properties();
                        buffer.set_direction(direction);
                        let output = shaper.shape(buffer, &[]);
                        println!(
                            "{font_index}|{text_index}|{coords:?}|{repeats}|{direction:?}|{:?}|{:?}",
                            output.glyph_infos(),
                            output.glyph_positions()
                        );
                        // Exercise recovered storage on another ordinary call.
                        let mut recovered = output.clear();
                        recovered.push_str("office a\u{301}");
                        recovered.guess_segment_properties();
                        let output = shaper.shape(recovered, &[]);
                        println!(
                            "recovered|{:?}|{:?}",
                            output.glyph_infos(),
                            output.glyph_positions()
                        );
                    }
                }
            }
        }
    }
}
