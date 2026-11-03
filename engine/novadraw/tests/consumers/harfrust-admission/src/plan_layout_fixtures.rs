//! Script, feature, required-feature and variation lookup collections.

#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;

fn words(values: &[u16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

fn set16(bytes: &mut [u8], field: usize, value: usize) {
    bytes[field..field + 2].copy_from_slice(&(value as u16).to_be_bytes());
}

pub fn layout(substitution: bool, indexed_language: bool, variation: bool) -> Vec<u8> {
    let mut data = words(&[1, 1, 0, 0, 14]);
    data.extend(0u32.to_be_bytes());
    let lookup = if substitution {
        words(&[1, 0, 1, 8, 1, 6, 1, 1, 1, 1])
    } else {
        words(&[1, 0, 1, 8, 1, 6, 0, 1, 1, 1])
    };
    data.extend(words(&[2, 6, 6 + lookup.len() as u16]));
    data.extend(&lookup);
    data.extend(&lookup);

    let script_offset = data.len();
    set16(&mut data, 4, script_offset);
    let mut scripts = words(&[3]);
    for tag in [b"DFLT", b"dflt", b"latn"] {
        scripts.extend(tag);
        scripts.extend(20u16.to_be_bytes());
    }
    let mut script = if indexed_language {
        let mut bytes = words(&[0, 1]);
        bytes.extend(b"dflt");
        bytes.extend(10u16.to_be_bytes());
        bytes
    } else {
        words(&[4, 0])
    };
    // Required rlig plus repeated and invalid feature indices.
    script.extend(words(&[0, 3, 6, 0, 1, 2, 3, 0xffff, 2]));
    scripts.extend(script);
    data.extend(scripts);

    let feature_offset = data.len();
    set16(&mut data, 6, feature_offset);
    let mut features = words(&[4]);
    for (index, tag) in [b"abcd", b"kern", b"liga", b"rlig"].into_iter().enumerate() {
        features.extend(tag);
        features.extend((26u16 + index as u16 * 12).to_be_bytes());
    }
    for _ in 0..4 {
        features.extend(words(&[0, 4, 0, 1, 0, 0xffff]));
    }
    data.extend(features);
    if variation {
        let offset = data.len();
        data[10..14].copy_from_slice(&(offset as u32).to_be_bytes());
        let mut variations = words(&[1, 0]);
        variations.extend(1u32.to_be_bytes());
        variations.extend(16u32.to_be_bytes()); // empty condition set
        variations.extend(18u32.to_be_bytes()); // substitution table
        variations.extend(0u16.to_be_bytes());
        variations.extend(words(&[1, 0, 2]));
        // Override kern and rlig with a single lookup; offsets are relative
        // to the substitution table and records are sorted by feature index.
        for (index, offset) in [(1u16, 18u32), (3, 24)] {
            variations.extend(index.to_be_bytes());
            variations.extend(offset.to_be_bytes());
        }
        variations.extend(words(&[0, 1, 1, 0, 1, 0]));
        data.extend(variations);
    }
    data
}

pub fn font(substitution: bool, indexed_language: bool, variation: bool) -> Vec<u8> {
    let table = layout(substitution, indexed_language, variation);
    let cmap = cold::cmap(&[(3, 1, 12)]);
    cold::sfnt(
        &[
            (if substitution { *b"GSUB" } else { *b"GPOS" }, &table),
            (*b"cmap", &cmap),
        ],
        true,
    )
}

pub fn fonts() -> Vec<Vec<u8>> {
    let mut fonts = Vec::new();
    for substitution in [false, true] {
        for indexed in [false, true] {
            for variation in [false, true] {
                fonts.push(font(substitution, indexed, variation));
            }
        }
        let table = layout(substitution, false, true);
        // Every truncation covers malformed list, child and offset readers.
        for prefix in 0..table.len() {
            fonts.push(cold::sfnt(
                &[(
                    if substitution { *b"GSUB" } else { *b"GPOS" },
                    &table[..prefix],
                )],
                true,
            ));
        }
    }
    fonts
}
