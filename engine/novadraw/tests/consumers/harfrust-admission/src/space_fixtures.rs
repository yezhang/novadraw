//! Space/hyphen fallback over the frozen GSUB/GPOS/GDEF kernels.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gdef_fixtures.rs"]
#[allow(dead_code)]
mod gdef;
pub use gdef::{LEVELS, input};

pub const SPACES: [u32; 17] = [
    0x0020, 0x00a0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008,
    0x2009, 0x200a, 0x202f, 0x205f, 0x3000,
];
pub const TEXTS: [&str; 4] = [
    " \u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{202f}\u{205f}\u{3000}",
    "AB\u{2007}C\u{2008}D\u{202f}EF",
    "A\u{2011}B\u{2000}C\u{2001}D\u{a0}",
    "\u{2011}",
];

pub fn font(kind: usize, case: usize, mapping: usize) -> Vec<u8> {
    let base = gdef::font(kind, case);
    let mut mappings: Vec<(u32, u32)> = (65..=122).map(|ch| (ch, ch)).collect();
    if mapping < 4 || mapping >= 8 {
        mappings.push((32, 32));
    }
    match mapping {
        0 | 4 | 8 | 9 => mappings.push((48, 48)),
        1 | 5 => mappings.push((53, 53)),
        2 | 6 => mappings.push((57, 57)),
        _ => {}
    }
    match mapping {
        0 | 3 | 4 | 8 | 9 => mappings.push((46, 46)),
        1 | 5 => mappings.push((44, 44)),
        _ => {}
    }
    if matches!(mapping, 1 | 4 | 5 | 8 | 9) {
        mappings.push((0x2010, 65));
    }
    if mapping == 8 {
        mappings.extend([(0x2002, 66), (0x2003, 67)]);
    }
    if mapping == 9 {
        mappings.extend(SPACES[1..].iter().map(|&ch| (ch, 68)));
        mappings.push((0x2011, 69));
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
        .map(|r| {
            let tag: [u8; 4] = r[..4].try_into().unwrap();
            let offset = u32::from_be_bytes(r[8..12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize;
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
