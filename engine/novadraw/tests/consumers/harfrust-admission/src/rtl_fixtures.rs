//! ASCII mirrors, optional missing mirror, substitution and scalar positioning.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "gpos_pair_fixtures.rs"]
#[allow(dead_code)]
mod pair;
#[path = "gpos_single_fixtures.rs"]
#[allow(dead_code)]
mod single;

pub fn font(kind: usize, missing_mirror: bool, substitutions: bool) -> Vec<u8> {
    let bytes = match kind {
        0 => single::font(1, 2, 0, 0, 0, substitutions),
        1 => single::font(2, 2, 3, 2, 15, substitutions),
        2 => pair::font(1, 2, 3, 2, 15, 15, substitutions),
        _ => pair::font(2, 2, 3, 2, 15, 15, substitutions),
    };
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
    let records: Vec<_> = bytes[12..12 + count * 16]
        .chunks_exact(16)
        .map(|r| {
            let tag: [u8; 4] = r[..4].try_into().unwrap();
            let offset = u32::from_be_bytes(r[8..12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize;
            (tag, offset, len)
        })
        .collect();
    // An absent format-12 group produces None. A zero format-0 glyph record
    // still produces Some(GlyphId(0)) and cannot test a missing mirror.
    let groups: &[[u32; 3]] = if missing_mirror {
        &[[0, 40, 0], [42, 127, 42]]
    } else {
        &[[0, 127, 0]]
    };
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12, 0, 12, 0, 0];
    for value in [16 + groups.len() as u32 * 12, 0, groups.len() as u32] {
        cmap.extend(value.to_be_bytes());
    }
    for group in groups {
        for value in group {
            cmap.extend(value.to_be_bytes());
        }
    }
    let tables: Vec<_> = records
        .iter()
        .filter(|(tag, _, _)| kind != 0 || *tag != *b"GPOS")
        .map(|(tag, offset, len)| {
            (
                *tag,
                if *tag == *b"cmap" {
                    cmap.as_slice()
                } else {
                    &bytes[*offset..*offset + *len]
                },
            )
        })
        .collect();
    cold::sfnt(&tables, true)
}
