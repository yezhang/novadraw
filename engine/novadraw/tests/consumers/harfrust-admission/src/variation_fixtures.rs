//! Small table fixtures for user-coordinate, duplicate-tag and avar semantics.

pub fn font(axes: u16, duplicate_tags: bool, mapping: &[u8], sorted: bool) -> Vec<u8> {
    let mut fvar = 0x0001_0000u32.to_be_bytes().to_vec();
    for value in [16, 2, axes, 20, 0, 4] {
        fvar.extend(u16::to_be_bytes(value));
    }
    for axis in 0..axes {
        fvar.extend(
            if duplicate_tags {
                0u32
            } else {
                u32::from(axis)
            }
            .to_be_bytes(),
        );
        for value in [-0x0001_0000i32, 0, 0x0001_0000] {
            fvar.extend(value.to_be_bytes());
        }
        fvar.extend([0u8; 4]);
    }
    sfnt(&[(*b"fvar", &fvar), (*b"avar", mapping)], sorted)
}

pub fn avar(version: u32, declared_axes: u16, records: &[&[(i16, i16)]]) -> Vec<u8> {
    let mut data = version.to_be_bytes().to_vec();
    data.extend(0u16.to_be_bytes());
    data.extend(declared_axes.to_be_bytes());
    for record in records {
        data.extend((record.len() as u16).to_be_bytes());
        for (from, to) in *record {
            data.extend(from.to_be_bytes());
            data.extend(to.to_be_bytes());
        }
    }
    data
}

// Shared with test/oracle targets; the cost binary only needs individual fonts.
#[allow(dead_code)]
pub fn fonts() -> Vec<Vec<u8>> {
    let maps = [
        // Missing header: ordinary normalization ignores malformed avar.
        vec![],
        avar(0x0001_0000, 0, &[]),
        avar(0x0001_0000, 1, &[&[]]),
        avar(0x0001_0000, 1, &[&[(0, 4096)]]),
        avar(
            0x0001_0000,
            1,
            &[&[(-16384, -16384), (0, 8192), (16384, 16384)]],
        ),
        // Duplicated caps and exactly three exact-match from values.
        avar(
            0x0001_0000,
            1,
            &[&[
                (-16384, -16384),
                (-16384, -16384),
                (0, -4096),
                (0, 8192),
                (0, 4096),
                (16384, 16384),
                (16384, 16384),
            ]],
        ),
        // Both duplicate and interpolation branches, including outside range.
        avar(
            0x0001_0000,
            1,
            &[&[(-4096, -8192), (4096, 0), (4096, 4096)]],
        ),
        // VarLenArray::get sees trailing records beyond declared axis_count.
        avar(0x0001_0000, 0, &[&[(0, 4096)], &[(0, 8192), (8192, 12288)]]),
        // Declared records are incomplete. The original parser rejects avar.
        avar(0x0001_0000, 12, &[&[(0, 0)]]),
    ];
    let mut fonts = Vec::new();
    for axes in [0, 1, 11, 12, 64] {
        for duplicate in [false, true] {
            for mapping in &maps {
                for sorted in [false, true] {
                    fonts.push(font(axes, duplicate, mapping, sorted));
                }
            }
        }
    }
    fonts
}

fn sfnt(tables: &[([u8; 4], &[u8])], sorted: bool) -> Vec<u8> {
    let mut tables = tables.to_vec();
    if sorted {
        tables.sort_by_key(|(tag, _)| *tag);
    }
    let mut data = 0x0001_0000u32.to_be_bytes().to_vec();
    data.extend((tables.len() as u16).to_be_bytes());
    data.extend([0u8; 6]);
    let mut offset = 12 + tables.len() * 16;
    for (tag, bytes) in &tables {
        data.extend(tag);
        data.extend(0u32.to_be_bytes());
        data.extend((offset as u32).to_be_bytes());
        data.extend((bytes.len() as u32).to_be_bytes());
        offset += bytes.len();
    }
    for (_, bytes) in &tables {
        data.extend(*bytes);
    }
    data
}
