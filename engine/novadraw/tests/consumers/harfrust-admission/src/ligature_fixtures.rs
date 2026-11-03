//! Ligature alternatives, matching-position spill and nested lookup patterns.
#[path = "multiple_fixtures.rs"]
#[allow(dead_code)]
mod multiple;
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// Each lookup contains ordered alternatives for A, with explicit tail glyphs.
pub fn layout(stages: &[Vec<Vec<u16>>], coverage: u16, subtables: u16) -> Vec<u8> {
    let mut bytes = words(&[1, 0, 10, 30, 44, 1]);
    bytes.extend(b"DFLT");
    bytes.extend(words(&[8, 4, 0, 0, 0xffff, 1, 0, 1]));
    bytes.extend(b"rlig");
    bytes.extend(words(&[8, 0, stages.len() as u16]));
    for index in 0..stages.len() {
        bytes.extend((index as u16).to_be_bytes());
    }
    let list = bytes.len();
    bytes[8..10].copy_from_slice(&(list as u16).to_be_bytes());
    bytes.extend((stages.len() as u16).to_be_bytes());
    let offsets = bytes.len();
    bytes.extend(vec![0; stages.len() * 2]);
    for (index, alternatives) in stages.iter().enumerate() {
        let start = bytes.len();
        let field = offsets + index * 2;
        bytes[field..field + 2].copy_from_slice(&((start - list) as u16).to_be_bytes());
        bytes.extend(words(&[4, 0, subtables]));
        let fields = bytes.len();
        bytes.extend(vec![0; usize::from(subtables) * 2]);
        for subtable in 0..subtables {
            let base = bytes.len();
            let field = fields + usize::from(subtable) * 2;
            bytes[field..field + 2].copy_from_slice(&((base - start) as u16).to_be_bytes());
            bytes.extend(words(&[1, 0, 1, 8]));
            let set = bytes.len();
            bytes.extend((alternatives.len() as u16).to_be_bytes());
            let ligature_fields = bytes.len();
            bytes.extend(vec![0; alternatives.len() * 2]);
            for (index, tail) in alternatives.iter().enumerate() {
                let field = ligature_fields + index * 2;
                let offset = (bytes.len() - set) as u16;
                bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
                bytes.extend(words(&[65, tail.len() as u16 + 1]));
                bytes.extend(words(tail));
            }
            let offset = (bytes.len() - base) as u16;
            bytes[base + 2..base + 4].copy_from_slice(&offset.to_be_bytes());
            let first = if subtable + 1 == subtables { 65 } else { 200 };
            bytes.extend(if coverage == 1 {
                words(&[1, 1, first])
            } else {
                words(&[2, 1, first, first, 0])
            });
        }
    }
    bytes
}

pub fn font(stages: &[Vec<Vec<u16>>], coverage: u16, subtables: u16) -> Vec<u8> {
    single::font_with_layout(&layout(stages, coverage, subtables))
}

pub fn alternatives(count: usize) -> Vec<Vec<u16>> {
    vec![
        vec![90; count.saturating_sub(1)],
        vec![65; count.saturating_sub(1)],
        vec![66],
    ]
}

pub fn mixed_font() -> Vec<u8> {
    let tables = [
        layout(&[alternatives(3)], 1, 1),
        multiple::layout(&[vec![65, 65]], 2, 1, 1),
        layout(&[alternatives(2)], 2, 1),
    ];
    let mut bytes = tables[0][..40].to_vec();
    bytes.extend(words(&[3, 0, 1, 2]));
    let list = bytes.len();
    bytes[8..10].copy_from_slice(&(list as u16).to_be_bytes());
    bytes.extend(words(&[3, 0, 0, 0]));
    for (index, table) in tables.iter().enumerate() {
        let old_list = u16::from_be_bytes(table[8..10].try_into().unwrap()) as usize;
        let lookup = old_list
            + u16::from_be_bytes(table[old_list + 2..old_list + 4].try_into().unwrap()) as usize;
        let offset = (bytes.len() - list) as u16;
        let field = list + 2 + index * 2;
        bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
        bytes.extend_from_slice(&table[lookup..]);
    }
    single::font_with_layout(&bytes)
}
