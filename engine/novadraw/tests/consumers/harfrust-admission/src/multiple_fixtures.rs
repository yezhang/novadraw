//! Multiple substitutions with variable sequence lengths and lookup chains.
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn layout(sequences: &[Vec<u16>], coverage: u16, subtables: u16, lookups: u16) -> Vec<u8> {
    let mut bytes = words(&[1, 0, 10, 30, 44, 1]);
    bytes.extend(b"DFLT");
    bytes.extend(words(&[8, 4, 0, 0, 0xffff, 1, 0, 1]));
    bytes.extend(b"rlig");
    bytes.extend(words(&[8, 0, lookups]));
    for index in 0..lookups {
        bytes.extend(index.to_be_bytes());
    }
    let list = bytes.len();
    bytes[8..10].copy_from_slice(&(list as u16).to_be_bytes());
    bytes.extend(lookups.to_be_bytes());
    let offsets = bytes.len();
    bytes.extend(vec![0; usize::from(lookups) * 2]);
    for lookup in 0..lookups {
        let start = bytes.len();
        let field = offsets + usize::from(lookup) * 2;
        bytes[field..field + 2].copy_from_slice(&((start - list) as u16).to_be_bytes());
        bytes.extend(words(&[2, 0, subtables]));
        let fields = bytes.len();
        bytes.extend(vec![0; usize::from(subtables) * 2]);
        for subtable in 0..subtables {
            let base = bytes.len();
            let field = fields + usize::from(subtable) * 2;
            bytes[field..field + 2].copy_from_slice(&((base - start) as u16).to_be_bytes());
            bytes.extend(words(&[1, 0, sequences.len() as u16]));
            let sequence_fields = bytes.len();
            bytes.extend(vec![0; sequences.len() * 2]);
            for (index, sequence) in sequences.iter().enumerate() {
                let field = sequence_fields + index * 2;
                let offset = (bytes.len() - base) as u16;
                bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
                bytes.extend((sequence.len() as u16).to_be_bytes());
                bytes.extend(words(sequence));
            }
            let offset = (bytes.len() - base) as u16;
            bytes[base + 2..base + 4].copy_from_slice(&offset.to_be_bytes());
            let first = if subtable + 1 == subtables { 65 } else { 200 };
            let count = sequences.len() as u16;
            if coverage == 1 {
                bytes.extend(words(&[1, count]));
                for glyph in first..first + count {
                    bytes.extend(glyph.to_be_bytes());
                }
            } else {
                bytes.extend(words(&[2, 1, first, first + count - 1, 0]));
            }
        }
    }
    bytes
}

pub fn font_with_layout(layout: &[u8]) -> Vec<u8> {
    single::font_with_layout(layout)
}

pub fn sequences(max: u16) -> Vec<Vec<u16>> {
    vec![
        vec![65],
        vec![65, 66],
        (0..max).map(|i| 65 + i % 3).collect(),
    ]
}
