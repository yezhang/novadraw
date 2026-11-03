//! GDEF readers combined with the proven substitution/positioning kernels.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "context_fixtures.rs"]
#[allow(dead_code)]
mod context;
#[path = "ligature_fixtures.rs"]
#[allow(dead_code)]
mod ligature;
#[path = "single_fixtures.rs"]
#[allow(dead_code)]
mod single;
#[path = "simple_unicode_fixtures.rs"]
#[allow(dead_code)]
mod unicode;

fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

pub use unicode::{LEVELS, input};

pub fn class_def(case: usize, attachment: bool) -> Vec<u8> {
    let value = |glyph: u16| {
        if attachment {
            glyph.wrapping_mul(1021)
        } else {
            match case {
                3 | 7 => 1,
                4 | 8 => 3,
                5 | 9 => 2,
                _ => glyph % 5, // base/ligature/mark/component/class-zero
            }
        }
    };
    match case {
        0 => vec![],            // nullable offset
        1 => words(&[1, 0, 0]), // empty indexed array
        2 => words(&[99, 0]),   // invalid ClassDef reader
        3..=6 => {
            let mut bytes = words(&[1, 0, 256]);
            bytes.extend(words(&(0..256).map(value).collect::<Vec<_>>()));
            bytes
        }
        11 => words(&[2, 0]), // empty binary search
        _ => {
            let mut records: Vec<_> = (0..19)
                .map(|i| {
                    let start = i * 7;
                    [start, start + 6, value(start)]
                })
                .collect();
            // Test borrowed malformed ordering without assuming sorted data.
            if case == 10 {
                records.swap(0, 18);
            }
            let mut bytes = words(&[2, records.len() as u16]);
            for record in records {
                bytes.extend(words(&record));
            }
            bytes
        }
    }
}

pub fn gdef(case: usize) -> Vec<u8> {
    let mut bytes = words(&[1, 0, 0, 0, 0, 0]);
    let classes = class_def(case, false);
    if !classes.is_empty() {
        let offset = bytes.len() as u16;
        bytes[4..6].copy_from_slice(&offset.to_be_bytes());
        bytes.extend(classes);
    }
    // Exercise an attachment reader even when glyph classes are absent.
    let marks = class_def(if case == 0 { 8 } else { case }, true);
    if !marks.is_empty() {
        let offset = bytes.len() as u16;
        bytes[10..12].copy_from_slice(&offset.to_be_bytes());
        bytes.extend(marks);
    }
    bytes
}

pub fn with_gdef(base: &[u8], case: usize) -> Vec<u8> {
    let count = u16::from_be_bytes(base[4..6].try_into().unwrap()) as usize;
    let mut tables: Vec<_> = base[12..12 + count * 16]
        .chunks_exact(16)
        .filter(|r| &r[..4] != b"GDEF")
        .map(|r| {
            let offset = u32::from_be_bytes(r[8..12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize;
            (r[..4].try_into().unwrap(), &base[offset..offset + len])
        })
        .collect();
    let gdef = gdef(case);
    tables.push((*b"GDEF", &gdef));
    cold::sfnt(&tables, true)
}

pub fn font(kind: usize, case: usize) -> Vec<u8> {
    let base = match kind {
        0..=3 => unicode::font(kind, 0, 0, true),
        4 => ligature::font(
            &[ligature::alternatives(3), ligature::alternatives(2)],
            2,
            3,
        ),
        5 => ligature::mixed_font(),
        6 => context::font(9, 2, 3, &context::records(9), 1, 0),
        _ => single::font_with_layout(&single::layout(1, 2, 3, 2)),
    };
    with_gdef(&base, case)
}
