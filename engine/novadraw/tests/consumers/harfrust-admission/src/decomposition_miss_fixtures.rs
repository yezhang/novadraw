//! Independent canonical b-miss cases over frozen lookup kernels.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gdef_fixtures.rs"]
#[allow(dead_code)]
mod gdef;
pub use gdef::{LEVELS, input};

pub const TEXTS: [&str; 4] = ["ABéCÅDÅEǻ", "éÅÅ", "ÅéABC", "KΩ\u{f900}éÅÅ\u{2007}\u{2011}"];

/// Bits independently select a, b and the intermediate singleton Å.
pub fn font(kind: usize, case: usize, mapping: usize) -> Vec<u8> {
    let base = gdef::font(kind, case);
    let mut mappings: Vec<(u32, u32)> = (65..=122)
        .filter(|&ch| mapping & 1 != 0 || !matches!(ch, 65 | 101))
        .map(|ch| (ch, ch))
        .collect();
    mappings.extend([(32, 32), (0x2010, 65), ('Ω' as u32, 66), ('豈' as u32, 67)]);
    if mapping & 1 != 0 {
        mappings.push(('å' as u32, 65));
    }
    if mapping & 2 != 0 {
        mappings.extend([(0x0301, 68), (0x030a, 69)]);
    }
    if mapping & 4 != 0 {
        mappings.push(('Å' as u32, 66));
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
