use harfrust::{ColdLookupTable, ColdOperation, ColdOperationError, FontRef, ShaperData};
use std::{error::Error, fmt};

#[path = "lookup_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lookup reject {}", self.0)
    }
}
impl Error for Reject {}

fn table(subst: bool) -> ColdLookupTable {
    if subst {
        ColdLookupTable::Substitution
    } else {
        ColdLookupTable::Positioning
    }
}

#[test]
fn all_subtable_kinds_full_and_small_caches_prepare_and_warm_without_allocation() {
    for (subst, kind, subtable) in fixtures::cases() {
        let bytes = fixtures::font(subst, kind, &subtable, 10);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(()))
            .unwrap()
            .unwrap();
        assert!(
            data.try_prepare_lookup_admitted(&font, table(subst), 0, |_, _| Ok::<_, Reject>(()))
                .unwrap()
        );
        let mut calls = Vec::new();
        assert!(
            data.try_prepare_lookup_admitted(&font, table(subst), 0, |op, cost| {
                calls.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
        );
        assert!(calls.iter().all(|(_, cost)| cost.allocation == 0));
        assert_eq!(calls.len(), 4); // directory planning/search, reader, slot
        assert!(
            !data
                .try_prepare_lookup_admitted(&font, table(subst), 1, |_, _| Ok::<_, Reject>(()))
                .unwrap()
        );
    }
}

#[test]
fn each_lookup_rejection_preserves_source_and_allows_complete_retry() {
    for (subst, kind, subtable) in fixtures::cases() {
        let bytes = fixtures::font(subst, kind, &subtable, 10);
        let font = FontRef::new(&bytes).unwrap();
        let mut trace = Vec::new();
        #[cfg(not(feature = "portable"))]
        {
            let data = ShaperData::new(&font);
            assert!(
                data.try_prepare_lookup_admitted(&font, table(subst), 0, |op, cost| {
                    trace.push((op, cost));
                    Ok::<_, Reject>(())
                })
                .unwrap()
            );
        }
        #[cfg(feature = "portable")]
        assert!(
            ShaperData::try_new_admitted(&font, |op, cost| {
                trace.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
            .is_some()
        );
        assert!(trace.iter().any(|(op, _)| *op == ColdOperation::Lookup));
        for rejected in 0..trace.len() {
            let mut calls = 0;
            let mut admit = |_, _| {
                let current = calls;
                calls += 1;
                if current == rejected {
                    Err(Reject(current))
                } else {
                    Ok(())
                }
            };
            #[cfg(not(feature = "portable"))]
            let data = ShaperData::new(&font);
            #[cfg(not(feature = "portable"))]
            let result = data
                .try_prepare_lookup_admitted(&font, table(subst), 0, &mut admit)
                .map(|_| ());
            #[cfg(feature = "portable")]
            let result = ShaperData::try_new_admitted(&font, &mut admit).map(|_| ());
            let error = result.err().unwrap();
            assert_eq!(calls, rejected + 1);
            assert!(matches!(&error, ColdOperationError::Admission(Reject(i)) if *i == rejected));
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                rejected
            );
            let mut retry = Vec::new();
            #[cfg(not(feature = "portable"))]
            assert!(
                data.try_prepare_lookup_admitted(&font, table(subst), 0, |op, cost| {
                    retry.push((op, cost));
                    Ok::<_, Reject>(())
                })
                .unwrap()
            );
            #[cfg(feature = "portable")]
            assert!(
                ShaperData::try_new_admitted(&font, |op, cost| {
                    retry.push((op, cost));
                    Ok::<_, Reject>(())
                })
                .unwrap()
                .is_some()
            );
            assert_eq!(retry, trace);
        }
    }
}

#[test]
fn ligature_nested_visits_and_complete_heap_capacity_are_charged() {
    let bytes = fixtures::font(true, 4, &fixtures::ligature(), 10);
    let font = FontRef::new(&bytes).unwrap();
    let mut visits = 0;
    let mut allocations = Vec::new();
    let mut admit = |op, cost: harfrust::ColdOperationCost| {
        if op == ColdOperation::Ligature {
            visits += 1;
        }
        if cost.allocation != 0 {
            allocations.push(cost.allocation);
        }
        Ok::<_, Reject>(())
    };
    #[cfg(not(feature = "portable"))]
    {
        let data = ShaperData::new(&font);
        assert!(
            data.try_prepare_lookup_admitted(&font, ColdLookupTable::Substitution, 0, &mut admit)
                .unwrap()
        );
    }
    #[cfg(feature = "portable")]
    assert!(
        ShaperData::try_new_admitted(&font, &mut admit)
            .unwrap()
            .is_some()
    );
    // Each of ten subtables visits two set offsets and four ligature offsets,
    // including the malformed one before its resolution.
    assert_eq!(visits, 10 * (2 + 4));
    assert_eq!(allocations.iter().filter(|&&n| n == 10).count(), 1);
    #[cfg(not(feature = "portable"))]
    assert_eq!(allocations.iter().filter(|&&n| n == 1).count(), 9); // eight full caches + LookupInfo
    #[cfg(feature = "portable")]
    assert_eq!(allocations.iter().filter(|&&n| n == 1).count(), 9); // eight full caches + lookup vector
}

#[test]
fn extension_chains_are_bounded_by_progress_and_rejection() {
    // 512 forward extensions end in a normal substitution; the constructor
    // uses an iterative resolver, independent of native call-stack depth.
    let mut chain = Vec::new();
    for _ in 0..512 {
        chain.extend([0, 1, 0, 7, 0, 0, 0, 8]);
    }
    chain.extend([0, 1, 0, 6, 0, 1, 0, 1, 0, 1, 0, 1]);
    let bytes = fixtures::font(true, 7, &chain, 1);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    assert!(
        data.try_prepare_lookup_admitted(&font, ColdLookupTable::Substitution, 0, |_, _| Ok::<
            _,
            Reject,
        >(
            ()
        ))
        .unwrap()
    );
    // A non-progressing extension is malformed and terminates. It must not
    // overflow the stack or publish a fabricated subtable.
    let bytes = fixtures::font(true, 7, &[0, 1, 0, 7, 0, 0, 0, 0], 1);
    let font = FontRef::new(&bytes).unwrap();
    let data = ShaperData::new(&font);
    assert!(
        data.try_prepare_lookup_admitted(&font, ColdLookupTable::Substitution, 0, |_, _| Ok::<
            _,
            Reject,
        >(
            ()
        ))
        .unwrap()
    );
}

#[test]
fn prepared_real_font_lookups_preserve_shaping_outputs() {
    use harfrust::{Direction, UnicodeBuffer};
    for bytes in [
        include_bytes!("../../../../assets/fonts/InterVariable.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
    ] {
        let font = FontRef::new(bytes).unwrap();
        let ordinary = ShaperData::new(&font);
        let admitted = ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(()))
            .unwrap()
            .unwrap();
        let mut count = 0;
        for table in [ColdLookupTable::Substitution, ColdLookupTable::Positioning] {
            for index in 0..=u16::MAX {
                if !admitted
                    .try_prepare_lookup_admitted(&font, table, index, |_, _| Ok::<_, Reject>(()))
                    .unwrap()
                {
                    break;
                }
                count += 1;
            }
        }
        assert!(count > 0);
        for _ in 0..2 {
            for text in ["office ffi AV", "é a\u{301}", "中文测试", "السَّلَامُ", "a中🙂"]
            {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let observe = |data: &ShaperData| {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.guess_segment_properties();
                        buffer.set_direction(direction);
                        let shaper = data.shaper(&font).build();
                        let output = shaper.shape(buffer, &[]);
                        format!(
                            "{:?} {:?} {}",
                            output.glyph_infos(),
                            output.glyph_positions(),
                            output.serialize(&shaper, harfrust::SerializeFlags::empty())
                        )
                    };
                    assert_eq!(observe(&ordinary), observe(&admitted));
                }
            }
        }
    }
}
