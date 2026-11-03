//! Mapped letters/spaces and mapped unsupported scalar categories.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "rtl_fixtures.rs"]
#[allow(dead_code)]
mod rtl;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn font(kind: usize, cmap_case: usize, missing: usize, substitutions: bool) -> Vec<u8> {
    let base = rtl::font(kind, missing == 2, substitutions);
    let count = u16::from_be_bytes(base[4..6].try_into().unwrap()) as usize;
    let records: Vec<_> = base[12..12 + count * 16]
        .chunks_exact(16)
        .map(|r| {
            (
                <[u8; 4]>::try_from(&r[..4]).unwrap(),
                u32::from_be_bytes(r[8..12].try_into().unwrap()) as usize,
                u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize,
            )
        })
        .collect();
    let (platform, encoding, subtable) = match cmap_case {
        1 | 7 => {
            let mut bytes = words(&[0, 262, 0]);
            bytes.extend((0..=255).map(|i| i as u8));
            (
                if cmap_case == 1 { 1 } else { 3 },
                if cmap_case == 1 { 0 } else { 1 },
                bytes,
            )
        }
        3 => {
            // Two direct-delta segments, including the terminal sentinel.
            let bytes = words(&[
                4, 32, 0, 4, 4, 1, 0, 0x00ff, 0xffff, 0, 0, 0xffff, 0, 1, 0, 0,
            ]);
            (3, 1, bytes)
        }
        4 => {
            let mut bytes = words(&[6, 522, 0, 0, 256]);
            bytes.extend(words(&(0..256).collect::<Vec<_>>()));
            (3, 1, bytes)
        }
        5 => {
            let mut bytes = words(&[10, 0]);
            for value in [532u32, 0, 0, 256] {
                bytes.extend(value.to_be_bytes());
            }
            bytes.extend(words(&(0..256).collect::<Vec<_>>()));
            (3, 10, bytes)
        }
        _ => {
            let mut groups: Vec<[u32; 3]> = if cmap_case == 2 {
                vec![[0xf000, 0xf0ff, 0]]
            } else {
                let mut groups = if missing == 2 {
                    vec![[0, 40, 0], [42, 127, 42]]
                } else {
                    vec![[0, 127, 0]]
                };
                for (ch, glyph) in [
                    ('\u{00a0}', 40),
                    ('É', 65),
                    ('é', 66),
                    ('ǅ', 67),
                    ('\u{0301}', 65),
                    ('\u{034f}', 65),
                    ('\u{115f}', 65),
                    ('\u{1160}', 65),
                    ('\u{2003}', 40),
                    ('\u{200d}', 65),
                    ('\u{2044}', 65),
                    ('\u{3000}', 40),
                    ('\u{3164}', 65),
                    ('一', 65),
                    ('中', 65),
                    ('文', 66),
                    ('\u{ffa0}', 65),
                    ('\u{ff9e}', 65),
                    ('\u{1f1e6}', 65),
                    ('\u{1f3fb}', 65),
                    ('\u{20000}', 65),
                    ('\u{200e9}', 66),
                    ('\u{e0020}', 65),
                ] {
                    if missing != 1 || ch != 'é' {
                        groups.push([ch as u32, ch as u32, glyph]);
                    }
                }
                groups
            };
            groups.sort_by_key(|g| g[0]);
            let mut bytes = words(&[if cmap_case == 6 { 13 } else { 12 }, 0]);
            for value in [16 + groups.len() as u32 * 12, 0, groups.len() as u32] {
                bytes.extend(value.to_be_bytes());
            }
            for group in groups {
                for value in group {
                    bytes.extend(value.to_be_bytes());
                }
            }
            (3, if cmap_case == 2 { 0 } else { 10 }, bytes)
        }
    };
    let mut cmap = words(&[0, 1, platform, encoding]);
    cmap.extend(12u32.to_be_bytes());
    cmap.extend(subtable);
    let tables: Vec<_> = records
        .iter()
        .map(|(tag, offset, len)| {
            (
                *tag,
                if *tag == *b"cmap" {
                    cmap.as_slice()
                } else {
                    &base[*offset..*offset + *len]
                },
            )
        })
        .collect();
    cold::sfnt(&tables, true)
}

pub const LEVELS: [harfrust::BufferClusterLevel; 4] = [
    harfrust::BufferClusterLevel::MonotoneGraphemes,
    harfrust::BufferClusterLevel::MonotoneCharacters,
    harfrust::BufferClusterLevel::Characters,
    harfrust::BufferClusterLevel::Graphemes,
];

pub fn input(
    text: &str,
    direction: harfrust::Direction,
    script: Option<harfrust::Script>,
    clusters: usize,
    level: harfrust::BufferClusterLevel,
    spare: usize,
) -> harfrust::UnicodeBuffer {
    let mut buffer = harfrust::UnicodeBuffer::new();
    assert!(buffer.reserve(spare));
    for (i, ch) in text.chars().enumerate() {
        buffer.add(
            ch,
            match clusters {
                0 => i as u32,
                1 => 0,
                _ => (text.len() - i) as u32,
            },
        );
    }
    buffer.set_direction(direction);
    if let Some(script) = script {
        buffer.set_script(script);
    }
    buffer.set_cluster_level(level);
    buffer.set_flags(harfrust::BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    buffer
}
