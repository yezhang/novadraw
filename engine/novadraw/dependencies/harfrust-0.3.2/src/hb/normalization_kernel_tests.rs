// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Internal normalization only; this does not admit marks through public shaping.
use super::*;
use crate::hb::ot_shape_normalize::_hb_ot_shape_normalize_with;

fn font(case: usize) -> Vec<u8> {
    let mut mappings = Vec::new();
    for (i, ch) in [
        'A', 'a', 'b', '\u{300}', '\u{301}', '\u{30a}', '\u{315}', '\u{316}', 'Å', 'é', 'ǻ',
        '\u{344}', 'Ä',
    ]
    .into_iter()
    .enumerate()
    {
        if case == 2 && i >= 3 || case == 0 && i >= 8 {
            continue;
        }
        mappings.push((ch as u32, (i + 1) as u32));
    }
    mappings.sort_unstable();
    let mut cmap: Vec<u8> = [0u16, 1, 3, 10]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    cmap.extend(12u32.to_be_bytes());
    cmap.extend([12u16, 0].into_iter().flat_map(u16::to_be_bytes));
    cmap.extend((16 + 12 * mappings.len() as u32).to_be_bytes());
    cmap.extend(0u32.to_be_bytes());
    cmap.extend((mappings.len() as u32).to_be_bytes());
    for (ch, glyph) in mappings {
        for value in [ch, ch, glyph] {
            cmap.extend(value.to_be_bytes());
        }
    }
    let mut bytes = Vec::from(&b"\0\x01\0\0\0\x01\0\0\0\0\0\0cmap\0\0\0\0\0\0\0\x1c"[..]);
    bytes.extend((cmap.len() as u32).to_be_bytes());
    bytes.extend(cmap);
    bytes
}

fn input(text: &str, level: u32, spare: bool) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    if spare {
        buffer
            .try_reserve_admitted(128, |_, _| Ok::<_, ()>(()))
            .unwrap();
    }
    buffer
        .try_push_str_admitted(text, |_, _| Ok::<_, ()>(()))
        .unwrap();
    buffer.set_direction(Direction::LeftToRight);
    buffer.0.allocate_unicode_vars();
    buffer
        .0
        .allocate_var(crate::GlyphInfo::NORMALIZER_GLYPH_INDEX_VAR);
    buffer.0.cluster_level = level;
    for (i, info) in buffer.0.info[..buffer.0.len].iter_mut().enumerate() {
        info.cluster = [7, 2, 2, 7][i % 4];
        info.init_unicode_props(&mut buffer.0.scratch_flags);
    }
    buffer
}

fn policy<E, F: FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>>(
    admit: F,
    shaper: &hb_font_t,
) -> AdmittedShape<F> {
    AdmittedShape {
        admit,
        mapping_work: shaper.charmap.general_mapping_work().unwrap(),
        non_ascii: 0,
        special_scalars: 0,
        normalization: SingletonNormalization::default(),
        glyph_properties_work: 0,
        has_glyph_classes: false,
    }
}

fn result(buffer: &hb_buffer_t) -> (Vec<u8>, usize, u32, i32, bool) {
    (
        bytemuck::cast_slice(&buffer.info[..buffer.len]).to_vec(),
        buffer.len,
        buffer.scratch_flags,
        buffer.max_ops,
        buffer.successful,
    )
}

#[test]
fn normalization_internal_growth_sort_and_compose_match_ordinary() {
    for case in 0..3 {
        let bytes = font(case);
        let font = crate::FontRef::new(&bytes).unwrap();
        let data = crate::ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
        for text in [
            "ÅÅ",
            "éaǻ",
            "A\u{30a}",
            "a\u{301}",
            "a\u{315}\u{300}\u{316}\u{315}\u{300}\u{316}",
            "\u{344}a",
            "a\u{301}\u{301}",
            "a\u{300}b",
        ] {
            for level in 0..4 {
                for spare in [false, true] {
                    let mut ordinary = input(text, level, spare);
                    let mut admitted = input(text, level, spare);
                    _hb_ot_shape_normalize_with(
                        &plan,
                        &mut ordinary.0,
                        &shaper,
                        &mut OrdinaryShape,
                    )
                    .unwrap();
                    let mut allocations = 0;
                    let mut budget = policy(
                        |_, cost| {
                            allocations += cost.allocation;
                            Ok::<_, ()>(())
                        },
                        &shaper,
                    );
                    _hb_ot_shape_normalize_with(&plan, &mut admitted.0, &shaper, &mut budget)
                        .unwrap();
                    assert_eq!(
                        result(&ordinary.0),
                        result(&admitted.0),
                        "{case}/{text}/{level}/{spare}"
                    );
                    assert_eq!(admitted.0.info.len(), admitted.0.pos.len());
                    if spare {
                        assert_eq!(allocations, 0);
                    } else if case == 0 && text == "ÅÅ" {
                        assert_eq!(admitted.len(), 4);
                        assert!(allocations >= 2 * 4);
                    }
                }
            }
        }
    }
}

#[test]
fn normalization_internal_each_rejection_stops_before_next_operation_and_retains_storage() {
    for case in 0..2 {
        let bytes = font(case);
        let font = crate::FontRef::new(&bytes).unwrap();
        let data = crate::ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
        for text in ["ÅÅ", "A\u{30a}", "a\u{315}\u{300}\u{316}\u{301}"] {
            for level in 0..4 {
                let mut completed = input(text, level, false);
                let mut checkpoints = Vec::new();
                _hb_ot_shape_normalize_with(
                    &plan,
                    &mut completed.0,
                    &shaper,
                    &mut policy(
                        |op, cost| {
                            checkpoints.push((op, cost));
                            Ok::<_, ()>(())
                        },
                        &shaper,
                    ),
                )
                .unwrap();
                for reject in 0..checkpoints.len() {
                    let mut buffer = input(text, level, false);
                    let mut seen = 0;
                    let failed = _hb_ot_shape_normalize_with(
                        &plan,
                        &mut buffer.0,
                        &shaper,
                        &mut policy(
                            |op, cost| {
                                assert_eq!((op, cost), checkpoints[seen]);
                                let current = seen;
                                seen += 1;
                                if current == reject {
                                    Err(current)
                                } else {
                                    Ok(())
                                }
                            },
                            &shaper,
                        ),
                    );
                    assert!(
                        matches!(failed, Err(ShapeOperationError::Admission(i)) if i == reject)
                    );
                    assert_eq!(seen, reject + 1);
                    assert_eq!(buffer.0.info.len(), buffer.0.pos.len());
                    let capacities = (buffer.0.info.capacity(), buffer.0.pos.capacity());
                    // Mirror the owning shaping API's error recovery.
                    buffer.0.allocated_var_bits = 0;
                    buffer.clear();
                    assert_eq!(buffer.0.allocated_var_bits, 0);
                    assert_eq!(
                        capacities,
                        (buffer.0.info.capacity(), buffer.0.pos.capacity())
                    );
                    buffer.push_str("Ab");
                    buffer.set_direction(Direction::LeftToRight);
                    let glyphs = shaper.shape_with_plan(&plan, buffer, &[]);
                    assert_eq!(glyphs.len(), 2);
                }
            }
        }
    }
}

#[test]
fn normalization_internal_growth_limit_rejects_before_array_publication() {
    let bytes = font(0);
    let font = crate::FontRef::new(&bytes).unwrap();
    let data = crate::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    let mut buffer = input("ÅÅ", 0, false);
    buffer.0.max_len = 2;
    let failed = _hb_ot_shape_normalize_with(
        &plan,
        &mut buffer.0,
        &shaper,
        &mut policy(|_, _| Ok::<_, ()>(()), &shaper),
    );
    assert!(matches!(failed, Err(ShapeOperationError::LengthLimit)));
    assert_eq!(buffer.0.info.len(), 2);
    assert_eq!(buffer.0.pos.len(), 2);
}
