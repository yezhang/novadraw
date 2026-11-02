//! Structural cases for directory/cmap/mark-set admission, not a font whitelist.

pub fn sfnt(tables: &[([u8; 4], &[u8])], sorted: bool) -> Vec<u8> {
    let mut tables = tables.to_vec();
    if sorted {
        tables.sort_by_key(|(tag, _)| *tag);
    }
    let mut data = Vec::new();
    data.extend(0x0001_0000u32.to_be_bytes());
    data.extend((tables.len() as u16).to_be_bytes());
    data.extend([0; 6]);
    let mut offset = 12 + tables.len() * 16;
    for (tag, table) in &tables {
        data.extend(tag);
        data.extend([0; 4]);
        data.extend((offset as u32).to_be_bytes());
        data.extend((table.len() as u32).to_be_bytes());
        offset += table.len();
    }
    for (_, table) in tables {
        data.extend(table);
    }
    data
}

fn put16(data: &mut Vec<u8>, value: u16) {
    data.extend(value.to_be_bytes());
}

fn put32(data: &mut Vec<u8>, value: u32) {
    data.extend(value.to_be_bytes());
}

fn cmap_subtable(format: u16) -> Vec<u8> {
    let mut data = Vec::new();
    put16(&mut data, format);
    match format {
        0 => {
            put16(&mut data, 262);
            put16(&mut data, 0);
            data.extend([0; 256]);
        }
        2 => {
            put16(&mut data, 518);
            put16(&mut data, 0);
            data.extend([0; 512]);
        }
        4 => {
            for value in [24, 0, 2, 2, 0, 0, 0xffff, 0, 0xffff, 1, 0] {
                put16(&mut data, value);
            }
        }
        6 => {
            for value in [12, 0, 0, 1, 1] {
                put16(&mut data, value);
            }
        }
        8 => {
            put16(&mut data, 0);
            put32(&mut data, 8208);
            put32(&mut data, 0);
            data.extend([0; 8192]);
            put32(&mut data, 0);
        }
        10 => {
            put16(&mut data, 0);
            for value in [22, 0, 0, 1] {
                put32(&mut data, value);
            }
            put16(&mut data, 1);
        }
        12 | 13 => {
            put16(&mut data, 0);
            for value in [28, 0, 1, 0x61, 0x61, 1] {
                put32(&mut data, value);
            }
        }
        14 => {
            put32(&mut data, 10);
            put32(&mut data, 0);
        }
        _ => {}
    }
    data
}

pub fn cmap(records: &[(u16, u16, u16)]) -> Vec<u8> {
    let mut data = vec![0, 0];
    put16(&mut data, records.len() as u16);
    let mut offset = 4 + records.len() * 8;
    let subtables: Vec<_> = records.iter().map(|r| cmap_subtable(r.2)).collect();
    for (record, subtable) in records.iter().zip(&subtables) {
        put16(&mut data, record.0);
        put16(&mut data, record.1);
        put32(&mut data, offset as u32);
        offset += subtable.len();
    }
    for subtable in subtables {
        data.extend(subtable);
    }
    data
}

pub fn gdef(mark_count: u16, malformed: bool) -> Vec<u8> {
    let mut data = Vec::new();
    put32(&mut data, 0x0001_0002);
    data.extend([0; 8]);
    put16(&mut data, 14);
    put16(&mut data, 1); // MarkGlyphSets format
    put16(&mut data, mark_count);
    let mut offset = 4 + usize::from(mark_count) * 4;
    for index in 0..mark_count {
        put32(
            &mut data,
            if malformed && index % 3 == 2 {
                u32::MAX
            } else {
                offset as u32
            },
        );
        offset += if index % 2 == 0 { 10 } else { 16 };
    }
    for index in 0..mark_count {
        if index % 2 == 0 {
            for value in [1, 3, 1, 42, 60000] {
                put16(&mut data, value);
            }
        } else {
            for value in [2, 2, 1, 42, 0, 1024, 65535, 42] {
                put16(&mut data, value);
            }
        }
    }
    data
}

pub fn lookup_layout(count: u16) -> Vec<u8> {
    let mut data = Vec::new();
    put32(&mut data, 0x0001_0000);
    for value in [0, 0, 10, count] {
        put16(&mut data, value);
    }
    // Lookup offsets are deliberately invalid: native cold cache does not
    // resolve them; portable must admit resolution and skip invalid entries.
    for _ in 0..count {
        put16(&mut data, 0xffff);
    }
    data
}

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for sorted in [false, true] {
        fonts.push(sfnt(&[], sorted));
        for format in [0, 2, 4, 6, 8, 10, 12, 13, 14, 99] {
            for (platform, encoding) in [(3, 0), (3, 10), (0, 4), (3, 1), (1, 0), (9, 9)] {
                let cmap = cmap(&[(9, 9, 99), (platform, encoding, format), (0, 5, 14)]);
                let gdef = gdef(3, true);
                fonts.push(sfnt(&[(*b"cmap", &cmap), (*b"GDEF", &gdef)], sorted));
            }
        }
        for count in [0, 1, 32] {
            let layout = lookup_layout(count);
            let gdef = gdef(32, false);
            let cmap = cmap(&[(0, 3, 12), (3, 0, 0), (0, 5, 14)]);
            fonts.push(sfnt(
                &[(*b"cmap", &cmap), (*b"GDEF", &gdef), (*b"GSUB", &layout)],
                sorted,
            ));
        }
        // Minimal AAT headers: admitted construction must decline before
        // reading declared chain/subtable counts or scanning their payloads.
        for tag in [*b"morx", *b"kern", *b"kerx"] {
            fonts.push(sfnt(
                &[(tag, &[0, 2, 0, 0, 0xff, 0xff, 0xff, 0xff])],
                sorted,
            ));
            fonts.push(sfnt(&[(tag, &[])], sorted));
        }
    }
    fonts
}
