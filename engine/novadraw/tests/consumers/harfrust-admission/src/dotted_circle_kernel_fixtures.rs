//! Independent ordinary output fixtures; no public mark admission is implied.
#[path = "cold_fixtures.rs"]
#[allow(dead_code)]
mod cold;
#[path = "normalizer_kernel_fixtures.rs"]
#[allow(dead_code)]
mod normalizer;
pub use normalizer::{LEVELS, input};

pub const TEXTS: [&str; 6] = [
    "\u{301}",
    "\u{301}A\u{301}A",
    "\u{344}a",
    "\u{315}\u{300}\u{316}",
    "A\u{301}",
    "\u{fe0f}A",
];

pub fn font(kind: usize, case: usize, mapping: usize, circle: bool) -> Vec<u8> {
    let base = normalizer::font(kind, case, mapping);
    let count = u16::from_be_bytes(base[4..6].try_into().unwrap()) as usize;
    let records: Vec<_> = base[12..12 + count * 16]
        .chunks_exact(16)
        .map(|record| {
            let tag = <[u8; 4]>::try_from(&record[..4]).unwrap();
            let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
            (tag, &base[offset..offset + len])
        })
        .collect();
    let old = records.iter().find(|(tag, _)| tag == b"cmap").unwrap().1;
    let mut groups: Vec<[u32; 3]> = old[28..]
        .chunks_exact(12)
        .map(|record| {
            core::array::from_fn(|i| {
                u32::from_be_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
            })
        })
        .collect();
    if circle {
        groups.push([0x25cc, 0x25cc, 67]);
    }
    groups.sort_unstable();
    let mut cmap = old[..20].to_vec();
    cmap[16..20].copy_from_slice(&(16 + groups.len() as u32 * 12).to_be_bytes());
    cmap.extend(0u32.to_be_bytes());
    cmap.extend((groups.len() as u32).to_be_bytes());
    for group in groups {
        for value in group {
            cmap.extend(value.to_be_bytes());
        }
    }
    let tables: Vec<_> = records
        .iter()
        .map(|(tag, bytes)| {
            (
                *tag,
                if tag == b"cmap" {
                    cmap.as_slice()
                } else {
                    *bytes
                },
            )
        })
        .collect();
    cold::sfnt(&tables, true)
}
