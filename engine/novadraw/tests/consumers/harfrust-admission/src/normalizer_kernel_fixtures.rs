//! Ordinary normalization, including marks and callbacks not publicly admitted.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gdef_fixtures.rs"]
#[allow(dead_code)]
mod gdef;
pub use gdef::{LEVELS, input};

pub const TEXTS: [&str; 10] = [
    "ÅÅéǻ",
    "A\u{30a}a\u{301}",
    "a\u{315}\u{300}\u{316}\u{315}\u{301}\u{316}",
    "a\u{301}\u{301}",
    "\u{344}a",
    "\u{315}\u{300}\u{316}",
    "a\u{315}\u{34f}\u{300}",
    "a\u{fe0f}\u{fe0e}b",
    "\u{ac01}\u{1100}\u{1161}\u{11a8}",
    "a\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}\u{315}\u{316}",
];

pub fn font(kind: usize, case: usize, mapping: usize) -> Vec<u8> {
    let base = gdef::font(kind, case);
    let mut mappings: Vec<(u32, u32)> = (0..128).map(|ch| (ch, ch)).collect();
    for (group, chars) in [
        [
            '\u{300}', '\u{301}', '\u{30a}', '\u{315}', '\u{316}', '\u{308}',
        ]
        .as_slice(),
        ['Å', 'é', 'ǻ', '\u{344}', '\u{34f}', '\u{fe0f}'].as_slice(),
        [
            '\u{ac01}', '\u{1100}', '\u{1161}', '\u{11a8}', '\u{fe0e}', 'Ä',
        ]
        .as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        if mapping & (1 << group) == 0 {
            for (i, ch) in chars.iter().enumerate() {
                mappings.push((*ch as u32, 65 + i as u32 % 3));
            }
        }
    }
    mappings.sort_unstable();
    let mut cmap: Vec<u8> = [0u16, 1, 3, 10]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    cmap.extend(12u32.to_be_bytes());
    cmap.extend([12u16, 0].into_iter().flat_map(u16::to_be_bytes));
    for value in [16 + mappings.len() as u32 * 12, 0, mappings.len() as u32] {
        cmap.extend(value.to_be_bytes());
    }
    for (ch, glyph) in mappings {
        for value in [ch, ch, glyph] {
            cmap.extend(value.to_be_bytes());
        }
    }
    let count = u16::from_be_bytes(base[4..6].try_into().unwrap()) as usize;
    let tables: Vec<_> = base[12..12 + count * 16]
        .chunks_exact(16)
        .map(|record| {
            let tag = <[u8; 4]>::try_from(&record[..4]).unwrap();
            let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
            (
                tag,
                if tag == *b"cmap" {
                    cmap.as_slice()
                } else {
                    &base[offset..offset + len]
                },
            )
        })
        .collect();
    cold::sfnt(&tables, true)
}
