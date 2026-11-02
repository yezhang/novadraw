//! Table-level fixtures. These are not a font whitelist for production admission.

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for axes in [0, 1, 11, 12, 64] {
        for scenario in 0..6 {
            let fvar = fvar(axes);
            let layout = layout(scenario);
            let tables = [
                (*b"fvar", fvar.as_slice()),
                (*b"GSUB", layout.as_slice()),
                (*b"GPOS", layout.as_slice()),
            ];
            // Test both sorted and unsorted FontRef directory lookup paths.
            for sorted in [false, true] {
                fonts.push(sfnt(&tables, sorted));
            }
        }
    }
    fonts.push(sfnt(&[], true));
    fonts.push(sfnt(&[(*b"fvar", &[0, 1])], true));
    fonts
}

fn put16(data: &mut Vec<u8>, value: u16) {
    data.extend(value.to_be_bytes());
}

fn put32(data: &mut Vec<u8>, value: u32) {
    data.extend(value.to_be_bytes());
}

fn fvar(axes: u16) -> Vec<u8> {
    let mut data = Vec::new();
    put32(&mut data, 0x0001_0000);
    for value in [16, 2, axes, 20, 0, 4] {
        put16(&mut data, value);
    }
    for axis in 0..axes {
        put32(&mut data, u32::from(axis));
        for fixed in [0, 0, 0x0001_0000] {
            put32(&mut data, fixed);
        }
        put16(&mut data, 0);
        put16(&mut data, 256 + axis);
    }
    data
}

fn range(axis: u16, min: i16, max: i16) -> Vec<u8> {
    [1, axis, min as u16, max as u16]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect()
}

fn layout(scenario: usize) -> Vec<u8> {
    let mut data = Vec::new();
    put32(&mut data, 0x0001_0001);
    for _ in 0..3 {
        put16(&mut data, 0);
    }
    put32(&mut data, 14);
    let variation_base = data.len();
    put32(&mut data, 0x0001_0000);
    put32(&mut data, 2);
    let record = data.len();
    // The second record is universal, including after false/invalid first sets.
    for _ in 0..4 {
        put32(&mut data, 0);
    }
    if scenario == 0 {
        return data;
    }
    let set_offset = (data.len() - variation_base) as u32;
    data[record..record + 4].copy_from_slice(&set_offset.to_be_bytes());
    if scenario == 5 {
        // Entirely malformed condition set; upstream skips it.
        data.push(0);
        return data;
    }
    let conditions = match scenario {
        1 => vec![range(0, -16384, 16384), range(60, 0, 0)],
        2 => vec![range(0, -16384, -8192), range(0, 0, 0)],
        3 => vec![vec![0, 9], vec![0, 1]],
        // A generated AND reader only reads its header/count. Harfrust's
        // current predicate returns false without resolving the nested offset.
        4 => vec![vec![0, 3, 1, 0xff, 0xff, 0xff]],
        _ => unreachable!(),
    };
    let set_base = data.len();
    put16(&mut data, conditions.len() as u16);
    let offset_base = data.len();
    for _ in &conditions {
        put32(&mut data, 0);
    }
    for (index, condition) in conditions.iter().enumerate() {
        let offset = (data.len() - set_base) as u32;
        data[offset_base + index * 4..offset_base + index * 4 + 4]
            .copy_from_slice(&offset.to_be_bytes());
        data.extend(condition);
    }
    data
}

fn sfnt(tables: &[([u8; 4], &[u8])], sorted: bool) -> Vec<u8> {
    let mut tables = tables.to_vec();
    if sorted {
        tables.sort_by_key(|(tag, _)| *tag);
    }
    let mut data = Vec::new();
    put32(&mut data, 0x0001_0000);
    put16(&mut data, tables.len() as u16);
    for _ in 0..3 {
        put16(&mut data, 0);
    }
    let mut offset = 12 + tables.len() * 16;
    for (tag, table) in &tables {
        data.extend(tag);
        put32(&mut data, 0); // checksum is not consulted by FontRef
        put32(&mut data, offset as u32);
        put32(&mut data, table.len() as u32);
        offset += table.len();
    }
    for (_, table) in tables {
        data.extend(table);
    }
    data
}
