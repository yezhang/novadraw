//! Structural lookup paths; no names or font whitelist in production.

#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub fn layout(kind: u16, subtable: &[u8], repeats: u16) -> Vec<u8> {
    let mut data = words(&[1, 0, 0, 0, 10, 1, 4, kind, 0, repeats]);
    data.extend(words(&vec![6 + repeats * 2; repeats as usize]));
    data.extend(subtable);
    data
}

fn simple(mut header: Vec<u16>, coverage_field: usize, class_fields: &[usize]) -> Vec<u8> {
    header[coverage_field] = (header.len() * 2) as u16;
    let class_offset = (header.len() * 2 + 6) as u16;
    for &field in class_fields {
        header[field] = class_offset;
    }
    let mut bytes = words(&header);
    bytes.extend(words(&[1, 1, 1])); // coverage format 1
    if !class_fields.is_empty() {
        bytes.extend(words(&[1, 1, 1, 1])); // class format 1
    }
    bytes
}

pub fn ligature() -> Vec<u8> {
    // Two sets: first has a normal and zero-component ligature; second has
    // one malformed offset and one normal ligature.
    words(&[
        1, 10, 2, 16, 32, 1, 1, 1, 2, 6, 12, 42, 2, 3, 43, 1, 2, 0xffff, 6, 44, 2, 4,
    ])
}

pub fn cases() -> Vec<(bool, u16, Vec<u8>)> {
    let mut cases = vec![
        (true, 1, simple(vec![1, 0, 1], 1, &[])),
        (true, 1, simple(vec![2, 0, 1, 2], 1, &[])),
        (true, 2, simple(vec![1, 0, 0], 1, &[])),
        (true, 3, simple(vec![1, 0, 0], 1, &[])),
        (true, 4, ligature()),
        (true, 5, simple(vec![1, 0, 0], 1, &[])),
        (true, 5, simple(vec![2, 0, 0, 0], 1, &[2])),
        (true, 5, simple(vec![3, 1, 0, 0], 3, &[])),
        (true, 6, simple(vec![1, 0, 0], 1, &[])),
        (true, 6, simple(vec![2, 0, 0, 0, 0, 0], 1, &[2, 3, 4])),
        (true, 6, simple(vec![3, 0, 1, 0, 0, 0], 3, &[])),
        (true, 8, simple(vec![1, 0, 0, 0, 0], 1, &[])),
        (false, 1, simple(vec![1, 0, 0], 1, &[])),
        (false, 1, simple(vec![2, 0, 0, 0], 1, &[])),
        (false, 2, simple(vec![1, 0, 0, 0, 0], 1, &[])),
        (false, 2, simple(vec![2, 0, 0, 0, 0, 0, 0, 0], 1, &[4, 5])),
        (false, 3, simple(vec![1, 0, 0], 1, &[])),
        (false, 4, simple(vec![1, 0, 0, 0, 0, 0], 1, &[])),
        (false, 5, simple(vec![1, 0, 0, 0, 0, 0], 1, &[])),
        (false, 6, simple(vec![1, 0, 0, 0, 0, 0], 1, &[])),
    ];
    // GPOS contextual variants use exactly the GSUB shared kernels.
    for (kind, subtable) in cases
        .iter()
        .filter(|c| c.0 && matches!(c.1, 5 | 6))
        .map(|c| (c.1 + 2, c.2.clone()))
        .collect::<Vec<_>>()
    {
        cases.push((false, kind, subtable));
    }
    for subst in [true, false] {
        let kind = if subst { 1 } else { 2 };
        let original = cases
            .iter()
            .find(|c| c.0 == subst && c.1 == kind)
            .unwrap()
            .2
            .clone();
        let mut extension = words(&[1, kind, 0, 8]);
        extension.extend(original);
        cases.push((subst, if subst { 7 } else { 9 }, extension));
    }
    cases
}

pub fn font(subst: bool, kind: u16, subtable: &[u8], repeats: u16) -> Vec<u8> {
    let table = layout(kind, subtable, repeats);
    let cmap = cold::cmap(&[(3, 1, 12)]);
    cold::sfnt(
        &[
            (if subst { *b"GSUB" } else { *b"GPOS" }, &table),
            (*b"cmap", &cmap),
        ],
        true,
    )
}
