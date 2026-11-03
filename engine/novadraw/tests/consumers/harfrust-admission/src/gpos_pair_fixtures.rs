//! Pair positioning: scalar records, both coverage and class formats.
#[path = "gpos_single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn values(bytes: &mut Vec<u8>, format: u16, seed: i16) {
    for bit in 0..16 {
        if format & (1 << bit) != 0 {
            bytes.extend((if bit < 4 { seed + bit } else { 0 }).to_be_bytes());
        }
    }
}

pub fn layout(
    format: u16,
    coverage: u16,
    subtables: u16,
    lookups: u16,
    value1: u16,
    value2: u16,
) -> Vec<u8> {
    let mut bytes = single::layout(1, 1, 1, lookups, 0);
    let list = u16::from_be_bytes(bytes[8..10].try_into().unwrap()) as usize;
    bytes.truncate(list);
    bytes.extend(lookups.to_be_bytes());
    let offsets = bytes.len();
    bytes.resize(offsets + usize::from(lookups) * 2, 0);
    for lookup in 0..lookups {
        let start = bytes.len();
        let field = offsets + usize::from(lookup) * 2;
        bytes[field..field + 2].copy_from_slice(&((start - list) as u16).to_be_bytes());
        bytes.extend(words(&[2, 0, subtables]));
        let fields = bytes.len();
        bytes.resize(fields + usize::from(subtables) * 2, 0);
        for subtable in 0..subtables {
            let base = bytes.len();
            let field = fields + usize::from(subtable) * 2;
            bytes[field..field + 2].copy_from_slice(&((base - start) as u16).to_be_bytes());
            bytes.extend(words(&[format, 0, value1, value2]));
            if format == 1 {
                bytes.extend(words(&[3, 0, 0, 0]));
                for first in 0..3 {
                    let offset = (bytes.len() - base) as u16;
                    let field = base + 10 + first * 2;
                    bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
                    bytes.extend(5u16.to_be_bytes());
                    for second in [65, 66, 67, 68, 90] {
                        bytes.extend((second as u16).to_be_bytes());
                        values(&mut bytes, value1, 11 + first as i16);
                        values(&mut bytes, value2, -17);
                    }
                }
            } else {
                bytes.extend(words(&[0, 0, 4, 4]));
                for first in 0..4 {
                    for second in 0..4 {
                        values(&mut bytes, value1, 11 + first + second);
                        values(&mut bytes, value2, -17 - second);
                    }
                }
                for class in 0..2 {
                    let offset = (bytes.len() - base) as u16;
                    let field = base + 8 + class * 2;
                    bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
                    bytes.extend(if coverage == 1 {
                        words(&[1, 65, 3, 1, 2, 3])
                    } else {
                        words(&[2, 3, 65, 65, 1, 66, 66, 2, 67, 67, 3])
                    });
                }
            }
            let offset = (bytes.len() - base) as u16;
            bytes[base + 2..base + 4].copy_from_slice(&offset.to_be_bytes());
            let glyph = if subtable + 1 == subtables { 65 } else { 200 };
            bytes.extend(if coverage == 1 {
                words(&[1, 3, glyph, glyph + 1, glyph + 2])
            } else {
                words(&[2, 1, glyph, glyph + 2, 0])
            });
        }
    }
    bytes
}

pub fn font(
    format: u16,
    coverage: u16,
    subtables: u16,
    lookups: u16,
    value1: u16,
    value2: u16,
    substitutions: bool,
) -> Vec<u8> {
    single::font_with_layout(
        &layout(format, coverage, subtables, lookups, value1, value2),
        substitutions,
        false,
    )
}
