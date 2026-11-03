//! Ordinary public API observations against original and patched Harfrust.
mod shape_fixtures;
use harfrust::{
    BufferFlags, Direction, Feature, FontRef, ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let mut fonts = shape_fixtures::fonts();
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(bytes.to_vec());
    }
    let features: Vec<_> = ["liga=1", "kern=0", "liga[1:4]=0"]
        .map(|value| Feature::from_str(value).unwrap())
        .to_vec();
    for (index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for pass in 0..2 {
            for script in [script::LATIN, script::ARABIC] {
                for direction in [
                    Direction::LeftToRight,
                    Direction::RightToLeft,
                    Direction::TopToBottom,
                    Direction::BottomToTop,
                ] {
                    for features in [&[][..], features.as_slice()] {
                        let plan = ShapePlan::new(&shaper, direction, Some(script), None, features);
                        for text in ["", "aaa", "office ffi", "é a\u{301}", "中文", "السَّلَامُ"]
                        {
                            for reserve in [0, 128] {
                                for (cluster_mode, flags) in [
                                    BufferFlags::empty(),
                                    BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
                                    BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL,
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    let mut input = UnicodeBuffer::new();
                                    assert!(input.reserve(reserve));
                                    if cluster_mode == 0 {
                                        input.push_str(text);
                                    } else {
                                        for (cluster, character) in text.chars().enumerate() {
                                            input.add(
                                                character,
                                                if cluster_mode == 1 {
                                                    0
                                                } else {
                                                    (cluster / 2) as u32
                                                },
                                            );
                                        }
                                    }
                                    input.set_script(script);
                                    input.set_direction(direction);
                                    input.set_flags(flags);
                                    let output = shaper.shape_with_plan(&plan, input, features);
                                    println!(
                                        "{index}/{pass}/{script:?}/{direction:?}/{features:?}/{text:?}/{reserve}/{cluster_mode}/{:?}/{:?}",
                                        output.glyph_infos(),
                                        output.glyph_positions(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
