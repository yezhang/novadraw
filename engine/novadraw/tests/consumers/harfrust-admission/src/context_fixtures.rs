//! Format 3 contexts with explicit record order and nested leaf lookups.
#[path = "multiple_fixtures.rs"]
#[allow(dead_code)]
mod multiple;
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn font(
    count: u16,
    coverage: u16,
    subtables: u16,
    records: &[(u16, u16)],
    leaf_kind: u16,
    leaf_flags: u16,
) -> Vec<u8> {
    let mut bytes = words(&[1, 0, 10, 30, 44, 1]);
    bytes.extend(b"DFLT");
    bytes.extend(words(&[8, 4, 0, 0, 0xffff, 1, 0, 1]));
    bytes.extend(b"rlig");
    bytes.extend(words(&[8, 0, 1, 0]));
    let list = bytes.len();
    bytes[8..10].copy_from_slice(&(list as u16).to_be_bytes());
    bytes.extend(words(&[2, 6, 0]));
    let lookup = bytes.len();
    bytes.extend(words(&[5, 0, subtables]));
    let fields = bytes.len();
    bytes.extend(vec![0; usize::from(subtables) * 2]);
    for subtable in 0..subtables {
        let base = bytes.len();
        let field = fields + usize::from(subtable) * 2;
        bytes[field..field + 2].copy_from_slice(&((base - lookup) as u16).to_be_bytes());
        bytes.extend(words(&[3, count, records.len() as u16]));
        let fields = bytes.len();
        bytes.extend(vec![0; usize::from(count) * 2]);
        for &(sequence, nested) in records {
            bytes.extend(words(&[sequence, nested]));
        }
        for index in 0..count {
            let offset = (bytes.len() - base) as u16;
            let field = fields + usize::from(index) * 2;
            bytes[field..field + 2].copy_from_slice(&offset.to_be_bytes());
            let first = if subtable + 1 == subtables { 65 } else { 200 };
            bytes.extend(if coverage == 1 {
                words(&[1, 3, first, first + 1, first + 2])
            } else {
                words(&[2, 1, first, first + 2, 0])
            });
        }
    }
    let offset = (bytes.len() - list) as u16;
    bytes[list + 4..list + 6].copy_from_slice(&offset.to_be_bytes());
    let leaf = if leaf_kind == 2 {
        multiple::layout(&[vec![65, 66]], coverage, 1, 1)
    } else {
        single::layout(leaf_kind, coverage, 3, 1)
    };
    let old_list = u16::from_be_bytes(leaf[8..10].try_into().unwrap()) as usize;
    let old_lookup = old_list
        + u16::from_be_bytes(leaf[old_list + 2..old_list + 4].try_into().unwrap()) as usize;
    let start = bytes.len();
    bytes.extend_from_slice(&leaf[old_lookup..]);
    bytes[start + 2..start + 4].copy_from_slice(&leaf_flags.to_be_bytes());
    single::font_with_layout(&bytes)
}

pub fn records(count: u16) -> Vec<(u16, u16)> {
    // Rewind after the first replacement; repeat positions and include a
    // skipped sequence index and a missing lookup.
    vec![
        (count.saturating_sub(1), 1),
        (0, 1),
        (0, 1),
        (count, 0),
        (0, 99),
    ]
}

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for count in [1, 2, 8, 9] {
        for coverage in [1, 2] {
            for subtables in [1, 3] {
                fonts.push(font(count, coverage, subtables, &records(count), 1, 0));
            }
        }
    }
    fonts.push(font(2, 1, 1, &[(0, 0)], 1, 0)); // recursive context
    fonts.push(font(2, 2, 1, &[(0, 1)], 2, 0)); // growing leaf
    fonts.push(font(2, 1, 1, &records(2), 1, 1)); // nonzero leaf flags
    fonts.push(font(8, 2, 3, &[], 1, 0)); // no nested work
    fonts
}
