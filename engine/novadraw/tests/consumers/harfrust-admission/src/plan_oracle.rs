//! Ordinary plans against original and production Harfrust.
mod cold_fixtures;
mod lookup_fixtures;
use harfrust::{
    Direction, Feature, FontRef, SerializeFlags, ShapePlan, ShapePlanKey, ShaperData, UnicodeBuffer,
};
use std::str::FromStr;

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
        let mut tail = header;
        tail.extend([0xff; 1024]);
        fonts.push(cold_fixtures::sfnt(&[(tag, &tail)], false));
    }
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(bytes.to_vec());
    }
    let features: Vec<_> = [
        "kern=0",
        "trak=0",
        "kern=1",
        "liga[1:4]=0",
        "abcd=4",
        "rand=2",
    ]
    .iter()
    .map(|s| Feature::from_str(s).unwrap())
    .collect();
    for (font_index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for pass in 0..2 {
            for (feature_index, features) in [
                vec![],
                features[..4].to_vec(),
                features.clone(),
                features.repeat(12),
            ]
            .iter()
            .enumerate()
            {
                for text in ["", "office ffi", "é a\u{301}", "中文测试", "السَّلَامُ", "a中🙂"]
                {
                    for direction in [
                        Direction::LeftToRight,
                        Direction::RightToLeft,
                        Direction::TopToBottom,
                        Direction::BottomToTop,
                    ] {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.guess_segment_properties();
                        buffer.set_direction(direction);
                        let script = Some(buffer.script());
                        let language = buffer.language();
                        let plan =
                            ShapePlan::new(&shaper, direction, script, language.as_ref(), features);
                        let matches = ShapePlanKey::new(script, direction)
                            .language(language.as_ref())
                            .features(features)
                            .matches(&plan);
                        let output = shaper.shape_with_plan(&plan, buffer, features);
                        println!(
                            "{font_index}/{pass}/{feature_index}/{text:?}/{direction:?}/{matches}/{} {:?} {:?} {}",
                            shaper.units_per_em(),
                            output.glyph_infos(),
                            output.glyph_positions(),
                            output.serialize(&shaper, SerializeFlags::empty()),
                        );
                    }
                }
            }
        }
    }
}
