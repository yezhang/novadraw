//! Missing glyphs and canonical singleton chains over frozen lookup kernels.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gdef_fixtures.rs"]
#[allow(dead_code)]
mod gdef;
pub use gdef::{LEVELS, input};

pub const TEXTS: [&str; 6] = [
    "ABKΩ\u{f900}C",
    "KAΩB\u{f900}",
    "ABC。🙂\u{0378}",
    "ÅA",
    "éA\u{301}",
    "\u{2007}\u{2011}\u{200d}",
];

pub fn font(kind: usize, case: usize, mapping: usize) -> Vec<u8> {
    let base = gdef::font(kind, case);
    let mut mappings: Vec<(u32, u32)> = (0..128).map(|v| (v, v)).collect();
    if mapping != 1 {
        mappings.extend([('Ω' as u32, 66), ('豈' as u32, 67)]);
    }
    if mapping == 2 {
        mappings.extend([
            ('K' as u32, 65),
            ('Ω' as u32, 66),
            (0xf900, 67),
            ('。' as u32, 67),
            ('🙂' as u32, 66),
            (0x0378, 65),
        ]);
    }
    if mapping == 3 {
        mappings.push(('Å' as u32, 65));
    }
    mappings.sort_by_key(|&(ch, _)| ch);
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
