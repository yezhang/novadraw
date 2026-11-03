//! Synthetic Syriac script dispatch over real Arabic lookup data.
//! This is a test fixture, not a claim of a production Syriac typeface.
pub fn syriac_font() -> Vec<u8> {
    let mut bytes = include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").to_vec();
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
    let mut replaced = 0;
    for index in 0..count {
        let directory = 12 + index * 16;
        if !matches!(&bytes[directory..directory + 4], b"GSUB" | b"GPOS") {
            continue;
        }
        let table =
            u32::from_be_bytes(bytes[directory + 8..directory + 12].try_into().unwrap()) as usize;
        let list =
            table + u16::from_be_bytes(bytes[table + 4..table + 6].try_into().unwrap()) as usize;
        let count = u16::from_be_bytes(bytes[list..list + 2].try_into().unwrap()) as usize;
        let mut records: Vec<[u8; 6]> = bytes[list + 2..list + 2 + count * 6]
            .chunks_exact(6)
            .map(|record| record.try_into().unwrap())
            .collect();
        for record in &mut records {
            if &record[..4] == b"arab" {
                record[..4].copy_from_slice(b"syrc");
                replaced += 1;
            }
        }
        records.sort_by_key(|record| <[u8; 4]>::try_from(&record[..4]).unwrap());
        for (index, record) in records.iter().enumerate() {
            bytes[list + 2 + index * 6..list + 2 + (index + 1) * 6].copy_from_slice(record);
        }
    }
    assert!(
        replaced > 0,
        "fixture must select a non-default Syriac script"
    );
    bytes
}
