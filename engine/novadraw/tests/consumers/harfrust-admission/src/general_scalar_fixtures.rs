//! Singleton Unicode scalars combined with frozen GSUB/GPOS/GDEF fixtures.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gdef_fixtures.rs"]
#[allow(dead_code)]
mod gdef;
pub use gdef::{LEVELS, input};

pub const TEXTS: [&str; 4] = [
    "A，。！？「」１２３٢Ⅷ±×÷∑∞€🙂",
    "（A）〈B〉≤≥∈∉",
    "ǆ\u{02b0}\u{0085}\u{2028}\u{2029}\u{0378}",
    "a\u{301}\u{200d}\u{1f1e6}\u{1f3fb}\u{ff9e}\u{2044}",
];

pub fn font(kind: usize, case: usize, missing_mirror: bool) -> Vec<u8> {
    let base = gdef::font(kind, case);
    let mut mappings: Vec<(u32, u32)> = (0..128).map(|v| (v, v)).collect();
    for (i, ch) in TEXTS.iter().flat_map(|text| text.chars()).enumerate() {
        mappings.push((ch as u32, 65 + (i % 3) as u32));
    }
    // Include the mirrored targets even when they are absent from the input.
    for ch in ['∋', '∌'] {
        mappings.push((ch as u32, 66));
    }
    if missing_mirror {
        mappings.retain(|&(ch, _)| ch != '）' as u32 && ch != '∌' as u32);
    }
    mappings.sort_by_key(|&(ch, _)| ch);
    mappings.dedup_by_key(|entry| entry.0);
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
