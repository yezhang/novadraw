//! Native lookup cold fill, warm hit and rejected cold tail; no shaping.
use harfrust::{ColdLookupTable, ColdOperationCost, FontRef, ShaperData};
use std::{hint::black_box, time::Instant};

fn main() {
    for (name, bytes) in [
        ("inter", include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice()),
        ("arabic", include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice()),
        ("cjk", include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice()),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let (table, index) = (ColdLookupTable::Substitution, 0);
        let data = ShaperData::new(&font);
        let mut last = 0;
        assert!(data.try_prepare_lookup_admitted(&font, table, index, |_, _| {
            last += 1; Ok::<_, usize>(())
        }).unwrap());
        for sample in 0..1100 {
            for offset in 0..5 {
                let mode = (sample + offset) % 5;
                let data = ShaperData::new(&font);
                if matches!(mode, 2 | 3) {
                    assert!(data.try_prepare_lookup_admitted(&font, table, index,
                        |_, _| Ok::<_, usize>(())).unwrap());
                }
                let mut calls = 0;
                let mut work = 0;
                let mut allocation = 0;
                let start = Instant::now();
                let result = data.try_prepare_lookup_admitted(black_box(&font), table, index,
                    |_, cost: ColdOperationCost| {
                        // Same shared kernel with a no-op callback is the
                        // local baseline; it is not the ordinary shaping API.
                        if matches!(mode, 0 | 2) { return Ok(()); }
                        calls += 1;
                        if mode == 4 && calls == last { return Err(calls); }
                        work += cost.work;
                        allocation += cost.allocation;
                        Ok(())
                    });
                let ns = start.elapsed().as_nanos();
                assert_eq!(result.is_ok(), mode != 4);
                if mode != 4 { assert!(result.unwrap()); }
                if matches!(mode, 1 | 4) { assert_eq!(calls, last); }
                if sample >= 100 {
                    let mode = ["cold_noop", "cold_counted", "warm_noop", "warm_counted", "reject_last"][mode];
                    println!("{{\"font\":\"{name}\",\"mode\":\"{mode}\",\"sample\":{},\"ns\":{ns},\"work\":{work},\"allocation\":{allocation},\"calls\":{calls}}}", sample - 100);
                }
                black_box(data);
            }
        }
    }
}
