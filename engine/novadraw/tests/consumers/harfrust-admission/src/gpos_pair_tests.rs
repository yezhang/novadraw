use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, FontRef, ShapeOperation, ShapeOperationError,
    ShapePlan, ShaperData, UnicodeBuffer, script,
};
use std::{error::Error, fmt};
#[path = "gpos_pair_fixtures.rs"]
mod fixtures;

fn input(text: &str, clusters: usize, level: BufferClusterLevel, spare: usize) -> UnicodeBuffer {
    let mut input = UnicodeBuffer::new();
    assert!(input.reserve(spare));
    for (index, ch) in text.chars().enumerate() {
        input.add(
            ch,
            match clusters {
                0 => index as u32,
                1 => 0,
                _ => (text.len() - index) as u32,
            },
        );
    }
    input.set_direction(Direction::LeftToRight);
    input.set_script(script::LATIN);
    input.set_flags(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT);
    input.set_cluster_level(level);
    input
}

#[test]
fn pair_positioning_preserves_complete_output_and_all_cluster_levels() {
    for format in [1, 2] {
        for coverage in [1, 2] {
            for (value1, value2) in [(0, 0), (1, 0), (0, 15), (15, 15), (8, 8)] {
                for substitutions in [false, true] {
                    let bytes =
                        fixtures::font(format, coverage, 3, 2, value1, value2, substitutions);
                    let font = FontRef::new(&bytes).unwrap();
                    let data = ShaperData::new(&font);
                    let shaper = data.shaper(&font).build();
                    let plan = ShapePlan::new(
                        &shaper,
                        Direction::LeftToRight,
                        Some(script::LATIN),
                        None,
                        &[],
                    );
                    for text in ["ABCABC", "AZADAX", "other", "A", ""] {
                        for clusters in 0..3 {
                            for level in [
                                BufferClusterLevel::MonotoneGraphemes,
                                BufferClusterLevel::MonotoneCharacters,
                                BufferClusterLevel::Characters,
                            ] {
                                for spare in [0, 256] {
                                    let expected = shaper.shape_with_plan(
                                        &plan,
                                        input(text, clusters, level, spare),
                                        &[],
                                    );
                                    let mut buffer = input(text, clusters, level, spare);
                                    let actual = shaper
                                        .try_shape_with_plan_admitted(
                                            &plan,
                                            &mut buffer,
                                            &[],
                                            |_, _| Ok::<_, ()>(()),
                                        )
                                        .unwrap()
                                        .expect("scalar PairPos supported");
                                    assert_eq!(
                                        format!("{:?}", actual.glyph_infos()),
                                        format!("{:?}", expected.glyph_infos())
                                    );
                                    assert_eq!(
                                        format!("{:?}", actual.glyph_positions()),
                                        format!("{:?}", expected.glyph_positions())
                                    );
                                    assert!(buffer.is_empty());
                                    assert!(
                                        actual.glyph_positions().iter().all(|p| p.y_advance == 0)
                                    );
                                    if text == "ABCABC" && value1 == 15 {
                                        assert!(
                                            actual
                                                .glyph_positions()
                                                .iter()
                                                .any(|p| p.x_offset != 0)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct Reject(usize);
impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reject {}", self.0)
    }
}
impl Error for Reject {}

#[test]
fn every_pair_rejection_preserves_source_prefix_and_next_ordinary_call() {
    for format in [1, 2] {
        for substitutions in [false, true] {
            let bytes = fixtures::font(format, 2, 3, 2, 15, 15, substitutions);
            let run = |reject, hot| {
                let font = FontRef::new(&bytes).unwrap();
                let data = ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                let plan = ShapePlan::new(
                    &shaper,
                    Direction::LeftToRight,
                    Some(script::LATIN),
                    None,
                    &[],
                );
                let make = || input("ABCABC", 2, BufferClusterLevel::Characters, 256);
                if hot {
                    shaper.shape_with_plan(&plan, make(), &[]);
                }
                let mut buffer = make();
                let mut steps = Vec::new();
                let mut entered = false;
                let result =
                    shaper.try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, cost| {
                        let index = steps.len();
                        steps.push((op, cost));
                        if reject == Some(index) {
                            return Err(Reject(index));
                        }
                        entered |= op == ShapeOperation::Recovery;
                        Ok(())
                    });
                if let Some(index) = reject {
                    let error = result.unwrap_err();
                    assert!(
                        matches!(&error, ShapeOperationError::Admission(Reject(i)) if *i == index)
                    );
                    assert_eq!(
                        error.source().unwrap().downcast_ref::<Reject>().unwrap().0,
                        index
                    );
                    assert_eq!(steps.len(), index + 1);
                    assert_eq!(buffer.len(), if entered { 0 } else { 6 });
                    buffer.clear();
                    buffer.push_str("next");
                    buffer.set_direction(Direction::LeftToRight);
                    buffer.set_script(script::LATIN);
                    assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);
                } else {
                    assert!(result.unwrap().is_some());
                }
                steps
            };
            for hot in [false, true] {
                let complete = run(None, hot);
                for index in 0..complete.len() {
                    assert_eq!(run(Some(index), hot), complete[..=index]);
                }
                if hot {
                    assert!(complete.iter().all(|(_, cost)| cost.allocation == 0));
                }
            }
        }
    }
}

#[test]
fn pair_device_and_reserved_formats_decline_before_shaping() {
    for format in [1, 2] {
        for (value1, value2) in [(0x10, 0), (0, 0x10), (0x8000, 15), (15, 0x8000)] {
            let bytes = fixtures::font(format, 2, 1, 1, value1, value2, false);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            let plan = ShapePlan::new(
                &shaper,
                Direction::LeftToRight,
                Some(script::LATIN),
                None,
                &[],
            );
            let mut buffer = input("ABC", 0, BufferClusterLevel::Characters, 0);
            let mut entered = false;
            assert!(
                shaper
                    .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                        entered |= op == ShapeOperation::Recovery;
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                    .is_none()
            );
            assert!(!entered);
            assert_eq!(buffer.len(), 3);
        }
    }
}

#[test]
fn out_of_range_pair_classes_decline_before_matrix_addressing() {
    #[path = "gpos_single_fixtures.rs"]
    #[allow(dead_code)]
    mod single;
    for coverage in [1, 2] {
        for side in 0..2 {
            let mut layout = fixtures::layout(2, coverage, 1, 1, 15, 15);
            let list = u16::from_be_bytes(layout[8..10].try_into().unwrap()) as usize;
            let lookup =
                list + u16::from_be_bytes(layout[list + 2..list + 4].try_into().unwrap()) as usize;
            let subtable = lookup
                + u16::from_be_bytes(layout[lookup + 6..lookup + 8].try_into().unwrap()) as usize;
            let field = subtable + 8 + side * 2;
            let class = subtable
                + u16::from_be_bytes(layout[field..field + 2].try_into().unwrap()) as usize;
            let value = class + if coverage == 1 { 6 } else { 8 };
            layout[value..value + 2].copy_from_slice(&u16::MAX.to_be_bytes());
            let bytes = single::font_with_layout(&layout, false, false);
            let font = FontRef::new(&bytes).unwrap();
            let data = ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            let plan = ShapePlan::new(
                &shaper,
                Direction::LeftToRight,
                Some(script::LATIN),
                None,
                &[],
            );
            let mut buffer = input("ABC", 0, BufferClusterLevel::Characters, 0);
            let mut entered = false;
            assert!(
                shaper
                    .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                        entered |= op == ShapeOperation::Recovery;
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                    .is_none()
            );
            assert!(!entered);
            assert_eq!(buffer.len(), 3);
        }
    }
}
