use harfrust::{ColdOperation, ColdOperationError, Direction, FontRef, ShaperData, UnicodeBuffer};
use std::{error::Error, fmt};

#[path = "cold_fixtures.rs"]
mod fixtures;

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reject {}", self.0)
    }
}
impl Error for Reject {}

const FONTS: [&[u8]; 3] = [
    include_bytes!("../../../../assets/fonts/InterVariable.ttf"),
    include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf"),
    include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf"),
];

fn shape(data: &ShaperData, font: &FontRef, text: &str, direction: Direction) -> String {
    let shaper = data.shaper(font).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    buffer.set_direction(direction);
    let output = shaper.shape(buffer, &[]);
    format!(
        "{} {:?} {:?}",
        shaper.units_per_em(),
        output.glyph_infos(),
        output.glyph_positions()
    )
}

#[test]
fn supported_cold_caches_preserve_ordinary_observations() {
    let synthetic = fixtures::fonts();
    let mut supported = 0;
    for bytes in synthetic.iter().map(Vec::as_slice).chain(FONTS) {
        let font = FontRef::new(bytes).unwrap();
        let Some(admitted) =
            ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(())).unwrap()
        else {
            continue;
        };
        let ordinary = ShaperData::new(&font);
        supported += 1;
        // Two passes exercise cold and warm lazy native lookups. Shaping is
        // deliberately ordinary here; this tests cold cache equivalence only.
        for _ in 0..2 {
            for text in ["", "office ffi", "é a\u{301}", "中文测试", "السَّلَامُ", "a中🙂"]
            {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    assert_eq!(
                        shape(&ordinary, &font, text, direction),
                        shape(&admitted, &font, text, direction)
                    );
                }
            }
        }
    }
    assert!(supported >= 122);
}

#[test]
fn every_cold_rejection_retains_source_and_stops_callbacks() {
    // Marks exercise partially filled vectors and both coverage formats.
    let gdef = fixtures::gdef(5, true);
    let cmap = fixtures::cmap(&[(9, 9, 99), (1, 0, 12), (0, 5, 14)]);
    let bytes = fixtures::sfnt(&[(*b"cmap", &cmap), (*b"GDEF", &gdef)], false);
    let font = FontRef::new(&bytes).unwrap();
    let mut costs = Vec::new();
    assert!(
        ShaperData::try_new_admitted(&font, |op, cost| {
            costs.push((op, cost));
            Ok::<_, Reject>(())
        })
        .unwrap()
        .is_some()
    );
    assert!(costs.iter().any(|(op, _)| *op == ColdOperation::Coverage));
    assert!(
        costs
            .iter()
            .any(|(op, cost)| *op == ColdOperation::Storage && cost.allocation == 5)
    );
    for rejected in 0..costs.len() {
        let mut calls = 0;
        let result = ShaperData::try_new_admitted(&font, |_, _| {
            let index = calls;
            calls += 1;
            if index == rejected {
                Err(Reject(index))
            } else {
                Ok(())
            }
        });
        let error = result.err().unwrap();
        assert_eq!(calls, rejected + 1);
        assert!(matches!(&error, ColdOperationError::Admission(Reject(i)) if *i == rejected));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
            rejected
        );
        // Rejected candidate never changes immutable input or prior caches.
        let ordinary = ShaperData::new(&font);
        assert!(!shape(&ordinary, &font, "a", Direction::LeftToRight).is_empty());
        assert!(
            ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(()))
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn unsupported_aat_is_detected_before_variable_size_reader() {
    for tag in [*b"morx", *b"kern", *b"kerx"] {
        for payload in [b"".as_slice(), &[0, 2, 0, 0, 0xff, 0xff, 0xff, 0xff]] {
            let bytes = fixtures::sfnt(&[(tag, payload)], true);
            let font = FontRef::new(&bytes).unwrap();
            let mut operations = Vec::new();
            assert!(
                ShaperData::try_new_admitted(&font, |op, _| {
                    operations.push(op);
                    Ok::<_, Reject>(())
                })
                .unwrap()
                .is_none()
            );
            assert_eq!(operations.last(), Some(&ColdOperation::TableRead));
            assert!(!operations.contains(&ColdOperation::CmapRecord));
        }
    }
}

#[test]
fn empty_aat_headers_preserve_cache_and_ordinary_shaping() {
    // Both kern dispatch branches and morx/kerx scalar counts. Version values
    // follow the locked reader's dispatch; admission adds no font whitelist.
    for (tag, header) in [
        (*b"kern", vec![0, 0, 0, 0]),
        (*b"kern", vec![0, 1, 0, 0, 0, 0, 0, 0]),
        (*b"morx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
        (*b"kerx", vec![0, 2, 0, 0, 0, 0, 0, 0]),
    ] {
        for trailing in [0, 1024] {
            let mut table = header.clone();
            table.extend(std::iter::repeat_n(0xff, trailing));
            let bytes = fixtures::sfnt(&[(tag, &table)], false);
            let font = FontRef::new(&bytes).unwrap();
            let mut trace = Vec::new();
            let admitted = ShaperData::try_new_admitted(&font, |op, cost| {
                trace.push((op, cost));
                Ok::<_, Reject>(())
            })
            .unwrap()
            .unwrap();
            assert!(trace.iter().all(|(_, cost)| cost.allocation == 0));
            let ordinary = ShaperData::new(&font);
            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                for text in ["", "ffi a\u{301}", "中文", "السَّلَامُ"] {
                    assert_eq!(
                        shape(&admitted, &font, text, direction),
                        shape(&ordinary, &font, text, direction),
                    );
                }
            }
            for rejected in 0..trace.len() {
                let mut observed = Vec::new();
                let error = ShaperData::try_new_admitted(&font, |op, cost| {
                    let index = observed.len();
                    observed.push((op, cost));
                    if index == rejected {
                        Err(Reject(index))
                    } else {
                        Ok(())
                    }
                })
                .err()
                .unwrap();
                assert_eq!(observed, trace[..=rejected]);
                assert_eq!(
                    error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                    rejected
                );
                let mut retry = Vec::new();
                assert!(
                    ShaperData::try_new_admitted(&font, |op, cost| {
                        retry.push((op, cost));
                        Ok::<_, Reject>(())
                    })
                    .unwrap()
                    .is_some()
                );
                assert_eq!(retry, trace);
                assert_eq!(
                    shape(&ordinary, &font, "a", Direction::LeftToRight),
                    shape(&admitted, &font, "a", Direction::LeftToRight)
                );
            }
        }
        for prefix in 0..header.len() {
            let bytes = fixtures::sfnt(&[(tag, &header[..prefix])], true);
            let font = FontRef::new(&bytes).unwrap();
            assert!(
                ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(()))
                    .unwrap()
                    .is_none()
            );
        }
        let mut nonempty = header;
        *nonempty.last_mut().unwrap() = 1;
        let bytes = fixtures::sfnt(&[(tag, &nonempty)], true);
        let font = FontRef::new(&bytes).unwrap();
        assert!(
            ShaperData::try_new_admitted(&font, |_, _| Ok::<_, Reject>(()))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn portable_admits_malformed_eager_lookups_native_allocates_empty_slots() {
    let layout = fixtures::lookup_layout(32);
    let bytes = fixtures::sfnt(&[(*b"GSUB", &layout)], true);
    let font = FontRef::new(&bytes).unwrap();
    let mut allocations = Vec::new();
    let result = ShaperData::try_new_admitted(&font, |_, cost| {
        if cost.allocation != 0 {
            allocations.push(cost.allocation);
        }
        Ok::<_, Reject>(())
    })
    .unwrap();
    #[cfg(not(feature = "portable"))]
    {
        assert!(result.is_some());
        assert_eq!(allocations, [32]);
    }
    #[cfg(feature = "portable")]
    {
        assert!(result.is_some());
        assert_eq!(allocations, [32]);
    }
}

#[test]
fn repeated_cmap_scans_and_inline_atom_initialization_are_charged() {
    let cmap = fixtures::cmap(&[(9, 9, 99); 7]);
    let bytes = fixtures::sfnt(&[(*b"cmap", &cmap)], true);
    let font = FontRef::new(&bytes).unwrap();
    let mut record_visits = 0;
    let mut subtable_reads = 0;
    let mut initialized = 0;
    assert!(
        ShaperData::try_new_admitted(&font, |op, cost| {
            match op {
                ColdOperation::CmapRecord => record_visits += 1,
                ColdOperation::CmapSubtable => subtable_reads += 1,
                ColdOperation::Initialize => initialized += cost.work,
                _ => {}
            }
            assert_eq!(cost.allocation, 0);
            Ok::<_, Reject>(())
        })
        .unwrap()
        .is_some()
    );
    assert_eq!(record_visits, 7 * 10 + 7);
    assert_eq!(subtable_reads, 7);
    assert_eq!(initialized, 128 + 256);
}

#[test]
fn caller_sticky_budget_survives_cold_candidate_rejection() {
    let gdef = fixtures::gdef(5, false);
    let bytes = fixtures::sfnt(&[(*b"GDEF", &gdef)], true);
    let font = FontRef::new(&bytes).unwrap();
    let mut failed = false;
    let mut remaining = 30usize;
    let mut admitted_work = 0;
    let mut admit = |_, cost: harfrust::ColdOperationCost| {
        if failed {
            return Err(Reject(1));
        }
        let cost = cost.work + cost.allocation;
        if cost > remaining {
            failed = true;
            return Err(Reject(0));
        }
        remaining -= cost;
        admitted_work += cost;
        Ok(())
    };
    assert!(ShaperData::try_new_admitted(&font, &mut admit).is_err());
    assert!(ShaperData::try_new_admitted(&font, &mut admit).is_err());
    assert!(failed);
    assert_eq!(admitted_work + remaining, 30);
    assert!(admitted_work > 0);
}
