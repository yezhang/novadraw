//! Scalar single-positioning tables, with optional earlier substitutions.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "multiple_fixtures.rs"]
#[allow(dead_code)]
mod multiple;
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn layout(
    format: u16,
    coverage: u16,
    subtables: u16,
    lookups: u16,
    value_format: u16,
) -> Vec<u8> {
    let mut bytes = single::layout(1, 1, 1, lookups);
    bytes[32..36].copy_from_slice(b"kern");
    // Retain ScriptList/FeatureList, then replace LookupList and its payload.
    let list = u16::from_be_bytes(bytes[8..10].try_into().unwrap()) as usize;
    bytes.truncate(list);
    bytes.extend(lookups.to_be_bytes());
    let offsets = bytes.len();
    bytes.resize(offsets + usize::from(lookups) * 2, 0);
    for lookup in 0..lookups {
        let start = bytes.len();
        let field = offsets + usize::from(lookup) * 2;
        bytes[field..field + 2].copy_from_slice(&((start - list) as u16).to_be_bytes());
        bytes.extend(words(&[1, 0, subtables]));
        let fields = bytes.len();
        bytes.resize(fields + usize::from(subtables) * 2, 0);
        for subtable in 0..subtables {
            let start_subtable = bytes.len();
            let field = fields + usize::from(subtable) * 2;
            bytes[field..field + 2]
                .copy_from_slice(&((start_subtable - start) as u16).to_be_bytes());
            bytes.extend(words(&[format, 0, value_format]));
            if format == 2 {
                bytes.extend(3u16.to_be_bytes());
            }
            for record in 0..if format == 1 { 1 } else { 3 } {
                for bit in 0..16 {
                    if value_format & (1 << bit) != 0 {
                        let value = match bit {
                            0 => 11 + record * 3,
                            1 => -7 - record * 2,
                            2 => 23 + record,
                            3 => -29,
                            _ => 0,
                        };
                        bytes.extend((value as i16).to_be_bytes());
                    }
                }
            }
            let offset = (bytes.len() - start_subtable) as u16;
            bytes[start_subtable + 2..start_subtable + 4].copy_from_slice(&offset.to_be_bytes());
            let start_glyph = if subtable + 1 == subtables { 65 } else { 200 };
            bytes.extend(if coverage == 1 {
                words(&[1, 3, start_glyph, start_glyph + 1, start_glyph + 2])
            } else {
                words(&[2, 1, start_glyph, start_glyph + 2, 0])
            });
        }
    }
    bytes
}

pub fn font_with_layout(layout: &[u8], substitutions: bool, gdef: bool) -> Vec<u8> {
    let base = single::font_with_layout(&[]);
    let count = u16::from_be_bytes(base[4..6].try_into().unwrap()) as usize;
    let cmap = base[12..12 + count * 16]
        .chunks_exact(16)
        .find(|record| &record[..4] == b"cmap")
        .unwrap();
    let offset = u32::from_be_bytes(cmap[8..12].try_into().unwrap()) as usize;
    let length = u32::from_be_bytes(cmap[12..16].try_into().unwrap()) as usize;
    let mut hhea = vec![0; 36];
    hhea[34..36].copy_from_slice(&256u16.to_be_bytes());
    let hmtx: Vec<_> = (0..256u16).flat_map(|g| words(&[600 + g, 0])).collect();
    let mut head = vec![0; 54];
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let maxp = words(&[1, 0, 256]);
    let gsub = multiple::layout(&multiple::sequences(3), 2, 3, 1);
    let gdef_data = cold::gdef(0, false);
    let mut tables = vec![
        (*b"cmap", &base[offset..offset + length]),
        (*b"GPOS", layout),
        (*b"hhea", &hhea),
        (*b"hmtx", &hmtx),
        (*b"head", &head),
        (*b"maxp", &maxp),
    ];
    if substitutions {
        tables.push((*b"GSUB", &gsub));
    }
    if gdef {
        tables.push((*b"GDEF", &gdef_data));
    }
    cold::sfnt(&tables, true)
}

pub fn font(
    format: u16,
    coverage: u16,
    subtables: u16,
    lookups: u16,
    values: u16,
    substitutions: bool,
) -> Vec<u8> {
    font_with_layout(
        &layout(format, coverage, subtables, lookups, values),
        substitutions,
        false,
    )
}
