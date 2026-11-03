//! Structural layout-free fonts for actual shaping, not an identity whitelist.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;

pub fn without_layout(bytes: &[u8]) -> Vec<u8> {
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
    let mut tables = Vec::new();
    for record in bytes[12..12 + count * 16].chunks_exact(16) {
        let tag: [u8; 4] = record[..4].try_into().unwrap();
        if matches!(
            &tag,
            b"GDEF" | b"GSUB" | b"GPOS" | b"morx" | b"kern" | b"kerx" | b"trak"
        ) {
            continue;
        }
        let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
        tables.push((tag, &bytes[offset..offset + length]));
    }
    cold::sfnt(&tables, true)
}

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for format in [0, 4, 6, 10, 12, 13] {
        let cmap = mapping(format);
        fonts.push(cold::sfnt(&[(*b"cmap", &cmap)], true));
    }
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        fonts.push(without_layout(bytes));
    }
    fonts
}

fn mapping(format: u16) -> Vec<u8> {
    let mut subtable = Vec::new();
    subtable.extend(format.to_be_bytes());
    match format {
        0 => {
            subtable.extend(262u16.to_be_bytes());
            subtable.extend(0u16.to_be_bytes());
            subtable.extend(0..=255u8);
        }
        4 => {
            // Two segments: ASCII and the required sentinel.
            for value in [32u16, 0, 4, 4, 1, 0, 127, 65535, 0, 0, 65535, 1, 1, 0, 0] {
                subtable.extend(value.to_be_bytes());
            }
        }
        6 => {
            for value in [266u16, 0, 0, 128] {
                subtable.extend(value.to_be_bytes());
            }
            for glyph in 1..=128u16 {
                subtable.extend(glyph.to_be_bytes());
            }
        }
        10 => {
            subtable.extend(0u16.to_be_bytes());
            for value in [276u32, 0, 0, 128] {
                subtable.extend(value.to_be_bytes());
            }
            for glyph in 1..=128u16 {
                subtable.extend(glyph.to_be_bytes());
            }
        }
        12 | 13 => {
            subtable.extend(0u16.to_be_bytes());
            for value in [28u32, 0, 1, 0, 127, 1] {
                subtable.extend(value.to_be_bytes());
            }
        }
        _ => unreachable!(),
    }
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12];
    cmap.extend(subtable);
    cmap
}
