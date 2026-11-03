//! Single substitutions with two coverage formats and chained lookups.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "shape_fixtures.rs"]
#[allow(dead_code)]
mod plain;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn layout(format: u16, coverage: u16, subtables: u16, lookups: u16) -> Vec<u8> {
    // GSUB 1.0: ScriptList at 10, FeatureList at 30, LookupList at 44.
    let mut bytes = words(&[1, 0, 10, 30, 44, 1]);
    bytes.extend(b"DFLT");
    bytes.extend(words(&[8, 4, 0, 0, 0xffff, 1, 0, 1]));
    bytes.extend(b"rlig");
    bytes.extend(words(&[8, 0, lookups]));
    for index in 0..lookups {
        bytes.extend(index.to_be_bytes());
    }
    // Feature table length varies with the number of selected lookups.
    let list_offset = bytes.len();
    bytes[8..10].copy_from_slice(&(list_offset as u16).to_be_bytes());
    bytes.extend(lookups.to_be_bytes());
    let offsets = bytes.len();
    bytes.extend(vec![0; usize::from(lookups) * 2]);
    for lookup in 0..lookups {
        let lookup_offset = bytes.len();
        let field = offsets + usize::from(lookup) * 2;
        bytes[field..field + 2]
            .copy_from_slice(&((lookup_offset - list_offset) as u16).to_be_bytes());
        bytes.extend(words(&[1, 0, subtables]));
        let fields = bytes.len();
        bytes.extend(vec![0; usize::from(subtables) * 2]);
        for subtable in 0..subtables {
            let offset = (bytes.len() - lookup_offset) as u16;
            let field = fields + usize::from(subtable) * 2;
            bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
            let header = if format == 1 {
                words(&[1, 6, 1])
            } else {
                words(&[2, 12, 3, 66, 67, 68])
            };
            bytes.extend(header);
            let start = if subtable + 1 == subtables { 65 } else { 200 };
            bytes.extend(if coverage == 1 {
                words(&[1, 3, start, start + 1, start + 2])
            } else {
                words(&[2, 1, start, start + 2, 0])
            });
        }
    }
    bytes
}

pub fn font_with_layout(layout: &[u8]) -> Vec<u8> {
    let base = plain::fonts().remove(0);
    let offset = u32::from_be_bytes(base[20..24].try_into().unwrap()) as usize;
    let length = u32::from_be_bytes(base[24..28].try_into().unwrap()) as usize;
    cold::sfnt(
        &[
            (*b"cmap", &base[offset..offset + length]),
            (*b"GSUB", layout),
        ],
        true,
    )
}

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for format in [1, 2] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                for lookups in [1, 2] {
                    fonts.push(font_with_layout(&layout(
                        format, coverage, subtables, lookups,
                    )));
                }
            }
        }
    }
    fonts
}
