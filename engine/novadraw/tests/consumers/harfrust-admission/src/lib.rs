//! External consumers for the input admission contract, not whole shaping admission.

#[cfg(test)]
mod instance_tests;

#[cfg(test)]
mod variation_tests;

#[cfg(test)]
mod cold_tests;
#[cfg(test)]
mod configuration_tests;
#[cfg(test)]
mod lookup_tests;
#[cfg(test)]
mod plan_layout_tests;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod private_language_tests;

#[cfg(test)]
mod script_callback_tests;

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod decomposition_miss_tests;
#[cfg(test)]
mod gdef_tests;
#[cfg(test)]
mod general_scalar_tests;
#[cfg(test)]
mod gpos_pair_tests;
#[cfg(test)]
mod gpos_single_tests;
#[cfg(test)]
mod ligature_tests;
#[cfg(test)]
mod multiple_tests;
#[cfg(test)]
mod normalization_tests;
#[cfg(test)]
mod rtl_tests;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod simple_unicode_tests;
#[cfg(test)]
mod single_tests;
#[cfg(test)]
mod space_tests;

#[cfg(test)]
mod tests {
    use harfrust::{BufferOperationError, Direction, FontRef, ShaperData, UnicodeBuffer};
    use std::{error::Error, fmt};

    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/InterVariable.ttf");

    #[derive(Debug)]
    struct Reject(usize);

    impl fmt::Display for Reject {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "rejected {}", self.0)
        }
    }
    impl Error for Reject {}

    #[test]
    fn every_input_rejection_retains_error_and_can_shape_next_call() {
        let font = FontRef::new(FONT).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let mut complete = UnicodeBuffer::new();
        let mut calls = 0;
        complete
            .try_push_str_admitted("ffi é中🙂", |_, _| {
                calls += 1;
                Ok::<_, Reject>(())
            })
            .unwrap();
        for reject in 0..calls {
            let mut buffer = UnicodeBuffer::new();
            let mut visited = 0;
            let error = buffer
                .try_push_str_admitted("ffi é中🙂", |_, _| {
                    let index = visited;
                    visited += 1;
                    if index == reject {
                        Err(Reject(index))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(visited, reject + 1);
            assert!(matches!(&error, BufferOperationError::Admission(Reject(i)) if *i == reject));
            assert_eq!(
                error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                reject
            );
            assert_eq!(buffer.len(), 0);
            buffer.push_str("office");
            buffer.guess_segment_properties();
            let output = shaper.shape(buffer, &[]);
            assert!(!output.is_empty());
            let mut recovered = output.clear();
            recovered
                .try_add_admitted('x', 7, |_, _| Ok::<_, Reject>(()))
                .unwrap();
            assert_eq!(recovered.len(), 1);
        }
    }

    #[test]
    fn caller_sticky_budget_stops_each_later_operation() {
        let mut buffer = UnicodeBuffer::new();
        let mut remaining = 3usize;
        let mut failed = false;
        let mut charged = 0usize;
        let mut admit = |_, cost: harfrust::BufferOperationCost| {
            if failed {
                return Err(Reject(1));
            }
            let total = cost.work + cost.allocation;
            if total > remaining {
                failed = true;
                return Err(Reject(0));
            }
            charged += total;
            remaining -= total;
            Ok(())
        };
        assert!(buffer.try_push_str_admitted("a", &mut admit).is_err());
        assert!(buffer.try_add_admitted('b', 0, &mut admit).is_err());
        assert_eq!(charged, 3);
        assert!(failed);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn admitted_input_matches_glyphs_positions_flags_and_clusters() {
        for bytes in [
            FONT,
            include_bytes!("../../../../assets/fonts/NotoSansArabic-VF.ttf").as_slice(),
            include_bytes!("../../../../assets/fonts/NotoSansSC-VF.ttf").as_slice(),
        ] {
            let font = FontRef::new(bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            for text in ["", "office ffi", "a\u{301} é", "中文测试", "السَّلَامُ", "a中🙂"]
            {
                for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                    let mut ordinary = UnicodeBuffer::new();
                    ordinary.push_str(text);
                    ordinary.guess_segment_properties();
                    ordinary.set_direction(direction);
                    let mut admitted = UnicodeBuffer::new();
                    admitted
                        .try_push_str_admitted(text, |_, _| Ok::<_, Reject>(()))
                        .unwrap();
                    admitted.guess_segment_properties();
                    admitted.set_direction(direction);
                    let ordinary = shaper.shape(ordinary, &[]);
                    let admitted = shaper.shape(admitted, &[]);
                    assert_eq!(
                        format!("{:?}", ordinary.glyph_infos()),
                        format!("{:?}", admitted.glyph_infos())
                    );
                    assert_eq!(
                        format!("{:?}", ordinary.glyph_positions()),
                        format!("{:?}", admitted.glyph_positions())
                    );
                }
            }
        }
    }
}
