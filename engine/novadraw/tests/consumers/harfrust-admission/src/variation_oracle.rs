//! Same ordinary user/named-instance oracle for original and production.
use harfrust::{FontRef, ShaperData, ShaperInstance, Tag, UnicodeBuffer, Variation};

mod variation_fixtures;

fn main() {
    let mut fonts = variation_fixtures::fonts();
    // Ordinary paths must also preserve avar2/unknown-version behavior even
    // though the admitted constructor refuses those versions before execution.
    for version in [0x0002_0000, 0x0001_0001, 0, 0xffff_ffff] {
        fonts.push(variation_fixtures::font(
            12,
            true,
            &variation_fixtures::avar(version, 0, &[]),
            true,
        ));
    }
    let synthetic = fonts.len();
    fonts.extend([
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").to_vec(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    ]);
    for (index, bytes) in fonts.iter().enumerate() {
        let font = FontRef::new(bytes).unwrap();
        let mut reused = ShaperInstance::default();
        for length in [0, 1, 11, 12, 64, 65] {
            for value in [-1000.0, -0.5, 0.0, 0.5, 400.0, 650.0, 2000.0] {
                let mut input: Vec<_> = (0u32..length)
                    .map(|axis| Variation {
                        tag: Tag::new(&axis.to_be_bytes()),
                        value,
                    })
                    .collect();
                if length > 0 {
                    input.extend([
                        Variation {
                            tag: Tag::new(&0u32.to_be_bytes()),
                            value: -0.25,
                        },
                        Variation {
                            tag: Tag::new(b"wght"),
                            value: 900.0,
                        },
                        Variation {
                            tag: Tag::new(b"wght"),
                            value,
                        },
                        Variation {
                            tag: Tag::new(b"opsz"),
                            value: 20.0,
                        },
                    ]);
                }
                let fresh = ShaperInstance::from_variations(&font, input.iter().copied());
                reused.set_variations(&font, input.iter().copied());
                assert_eq!(format!("{fresh:?}"), format!("{reused:?}"));
                println!("user|{index}|{length}|{value}|{fresh:?}");
                if index < synthetic {
                    continue;
                }
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).instance(Some(&fresh)).build();
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
        // Named-instance normalization also uses the shared user kernel.
        for named in [0, 1, 8, 64, usize::MAX] {
            let fresh = ShaperInstance::from_named_instance(&font, named);
            reused.set_named_instance(&font, named);
            assert_eq!(format!("{fresh:?}"), format!("{reused:?}"));
            println!("named|{index}|{named}|{fresh:?}");
        }
    }
}
