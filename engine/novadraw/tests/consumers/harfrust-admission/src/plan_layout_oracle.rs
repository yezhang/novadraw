//! Same ordinary API source against original and patched libraries.
mod cold_fixtures;
mod plan_layout_fixtures;
use harfrust::{
    Direction, Feature, FontRef, Language, NormalizedCoord, SerializeFlags, ShapePlan,
    ShapePlanKey, ShaperData, ShaperInstance, UnicodeBuffer, script,
};
use std::str::FromStr;

fn main() {
    let mut fonts = Vec::new();
    for substitution in [false, true] {
        for indexed in [false, true] {
            for variation in [false, true] {
                fonts.push(plan_layout_fixtures::font(substitution, indexed, variation));
            }
        }
        let table = plan_layout_fixtures::layout(substitution, false, true);
        for prefix in [0, 1, 4, 10, 13, 14, 20, 64, 80, 100, table.len() - 1] {
            fonts.push(cold_fixtures::sfnt(
                &[(
                    if substitution { *b"GSUB" } else { *b"GPOS" },
                    &table[..prefix],
                )],
                true,
            ));
        }
    }
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(bytes.to_vec());
    }
    let language = Language::from_str("en-US-x-hbsc-latn").unwrap();
    let features: Vec<_> = ["kern=1", "liga=1", "abcd=2", "rand=1", "liga[1:4]=0"]
        .iter()
        .map(|s| Feature::from_str(s).unwrap())
        .collect();
    for (font_index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let data = ShaperData::new(&font);
        for (coords_index, coords) in [vec![], vec![NormalizedCoord::from_f32(0.5)]]
            .into_iter()
            .enumerate()
        {
            let instance = ShaperInstance::from_coords(&font, coords);
            let shaper = data.shaper(&font).instance(Some(&instance)).build();
            for pass in 0..2 {
                for script in [script::LATIN, script::ARABIC] {
                    for language in [None, Some(&language)] {
                        for (feature_index, features) in [
                            vec![],
                            features[..4].to_vec(),
                            features.clone(),
                            features.repeat(12),
                        ]
                        .iter()
                        .enumerate()
                        {
                            for text in ["", "aaa", "office ffi", "é a\u{301}", "中文", "السَّلَامُ"]
                            {
                                for direction in [
                                    Direction::LeftToRight,
                                    Direction::RightToLeft,
                                    Direction::TopToBottom,
                                    Direction::BottomToTop,
                                ] {
                                    let plan = ShapePlan::new(
                                        &shaper,
                                        direction,
                                        Some(script),
                                        language,
                                        features,
                                    );
                                    let matches = ShapePlanKey::new(Some(script), direction)
                                        .language(language)
                                        .instance(Some(&instance))
                                        .features(features)
                                        .matches(&plan);
                                    let mut buffer = UnicodeBuffer::new();
                                    buffer.push_str(text);
                                    buffer.set_script(script);
                                    buffer.set_direction(direction);
                                    if let Some(language) = language {
                                        buffer.set_language(language.clone());
                                    }
                                    let output = shaper.shape_with_plan(&plan, buffer, features);
                                    println!(
                                        "{font_index}/{coords_index}/{pass}/{script:?}/{language:?}/{feature_index}/{text:?}/{direction:?}/{matches}/{:?}/{:?}/{}",
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
        }
    }
}
