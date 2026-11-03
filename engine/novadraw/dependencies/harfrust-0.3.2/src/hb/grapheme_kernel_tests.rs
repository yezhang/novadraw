// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Internal stored-continuation kernels, without enabling public mark input.
use super::*;
use crate::hb::buffer::admission::GroupStep;
use crate::hb::ot_layout::_hb_grapheme_group_func;
use crate::hb::ot_shape::{ensure_native_direction_with, form_clusters_with};

fn input(n: usize, level: u32, positions: bool, prefix: bool) -> hb_buffer_t {
    let mut b = UnicodeBuffer::new();
    b.try_push_str_admitted(&"A".repeat(n), |_, _| Ok::<_, ()>(()))
        .unwrap();
    b.0.allocate_unicode_vars();
    b.0.direction = Direction::RightToLeft;
    b.0.script = Some(script::LATIN);
    b.0.cluster_level = level;
    b.0.have_positions = positions;
    for i in 0..n {
        b.0.info[i].cluster = [7, 2, 7, 7, 1, 1][i % 6];
        b.0.info[i].mask = 0x1300;
        b.0.info[i].init_unicode_props(&mut b.0.scratch_flags);
        if i % 3 != 0 {
            b.0.info[i].set_continuation(&mut b.0.scratch_flags);
        }
        b.0.pos[i].x_advance = i as i32 + 10;
    }
    if n > 1 {
        b.0.scratch_flags |= crate::hb::buffer::HB_BUFFER_SCRATCH_FLAG_HAS_CONTINUATIONS;
    }
    if prefix && n >= 4 {
        b.0.clear_output();
        // Force separate output so prefix writes do not alias input writes.
        b.0.output_glyph(65);
        b.0.output_glyph(66);
        b.0.out_info_mut()[0].cluster = 7;
        b.0.out_info_mut()[1].cluster = 7;
    }
    b.0
}

fn policy<E, F: FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>>(
    admit: F,
) -> AdmittedShape<F> {
    AdmittedShape {
        admit,
        mapping_work: 0,
        non_ascii: 0,
        special_scalars: 0,
        normalization: SingletonNormalization::default(),
        glyph_properties_work: 0,
        has_glyph_classes: false,
    }
}

fn state(b: &hb_buffer_t) -> (Vec<u8>, Vec<u8>, Vec<u8>, i32, bool, Direction) {
    (
        bytemuck::cast_slice(&b.info[..b.len]).to_vec(),
        bytemuck::cast_slice(&b.pos[..b.len]).to_vec(),
        bytemuck::cast_slice(&b.out_info()[..b.out_len]).to_vec(),
        b.max_ops,
        b.successful,
        b.direction,
    )
}

fn run<P: ShapeAdmission>(b: &mut hb_buffer_t, p: &mut P) -> Result<(), P::Error> {
    form_clusters_with(b, p)?;
    ensure_native_direction_with(b, p)
}

fn recovery_font() -> Vec<u8> {
    // Two Format12 mappings, without outlines or layout tables.
    let mut cmap: Vec<u8> = [0u16, 1, 3, 10]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    cmap.extend(12u32.to_be_bytes());
    cmap.extend([12u16, 0].into_iter().flat_map(u16::to_be_bytes));
    for value in [40u32, 0, 2, 65, 65, 1, 66, 66, 2] {
        cmap.extend(value.to_be_bytes());
    }
    let mut bytes = Vec::from(&b"\0\x01\0\0\0\x01\0\0\0\0\0\0cmap\0\0\0\0\0\0\0\x1c"[..]);
    bytes.extend((cmap.len() as u32).to_be_bytes());
    bytes.extend(cmap);
    bytes
}

#[test]
fn grapheme_internal_all_levels_lengths_positions_and_output_prefix() {
    for n in [0, 1, 2, 4, 6, 255, 256] {
        for level in 0..4 {
            for positions in [false, true] {
                for prefix in [false, true] {
                    let mut ordinary = input(n, level, positions, prefix);
                    let mut admitted = input(n, level, positions, prefix);
                    run(&mut ordinary, &mut OrdinaryShape).unwrap();
                    let mut allocation = 0;
                    run(
                        &mut admitted,
                        &mut policy(|_, cost| {
                            allocation += cost.allocation;
                            Ok::<_, ()>(())
                        }),
                    )
                    .unwrap();
                    assert_eq!(state(&ordinary), state(&admitted));
                    assert_eq!(allocation, 0);
                    assert_eq!(admitted.direction, Direction::LeftToRight);
                }
            }
        }
    }
    // Continuations form [A,A,A] [A,A,A]; preserve each group's order
    // through the two reversals, while reversing group order.
    let mut b = input(6, 2, true, false);
    b.reverse_groups_with(_hb_grapheme_group_func, false, &mut OrdinaryShape)
        .unwrap();
    assert_eq!(
        b.pos[..6].iter().map(|p| p.x_advance).collect::<Vec<_>>(),
        [13, 14, 15, 10, 11, 12]
    );
}

#[test]
fn grapheme_internal_singleton_and_unsafe_short_circuit_costs() {
    for n in [0, 1, 2, 255, 256] {
        let mut b = input(n, 1, false, false);
        let mut ops = Vec::new();
        b.unsafe_to_break_with(
            0,
            n,
            &mut policy(|op, cost| {
                ops.push((op, cost));
                Ok::<_, ()>(())
            }),
        )
        .unwrap();
        assert_eq!(ops.len(), if (2..=255).contains(&n) { 3 } else { 1 });
        if n == 256 {
            assert!(b.info[..n].iter().all(|i| i.mask == 0x1300));
        }
        ops.clear();
        b.merge_clusters_with(
            0,
            n.min(1),
            &mut policy(|op, cost| {
                ops.push((op, cost));
                Ok::<_, ()>(())
            }),
        )
        .unwrap();
        assert_eq!(
            ops,
            [(
                ShapeOperation::Planning,
                ShapeOperationCost {
                    work: 2,
                    allocation: 0
                }
            )]
        );
    }
    let mut b = input(6, 0, false, true);
    b.idx = 0;
    b.max_ops = 0;
    let mut ordinary = input(6, 0, false, true);
    ordinary.idx = 0;
    ordinary.max_ops = 0;
    b.merge_clusters_with(0, 3, &mut policy(|_, _| Ok::<_, ()>(())))
        .unwrap();
    ordinary.merge_clusters(0, 3);
    assert_eq!(state(&ordinary), state(&b));
    assert!(!b.successful);
    assert_eq!(b.max_ops, -3);
    // Prefix cluster 7 is revisited and rewritten to the minimum 2.
    assert!(b.out_info()[..2].iter().all(|i| i.cluster == 2));
}

#[test]
fn grapheme_internal_every_checkpoint_rejection_and_recovery() {
    let bytes = recovery_font();
    let font = crate::FontRef::new(&bytes).unwrap();
    let data = crate::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
    let mut fresh = UnicodeBuffer::new();
    fresh.push_str("AB");
    fresh.set_direction(Direction::LeftToRight);
    let expected = shaper.shape_with_plan(&plan, fresh, &[]);
    for level in 0..4 {
        for positions in [false, true] {
            let mut complete = input(6, level, positions, true);
            let mut checkpoints = Vec::new();
            run(
                &mut complete,
                &mut policy(|op, cost| {
                    checkpoints.push((op, cost));
                    Ok::<_, ()>(())
                }),
            )
            .unwrap();
            for reject in 0..checkpoints.len() {
                let mut b = input(6, level, positions, true);
                let mut seen = 0;
                let failed = run(
                    &mut b,
                    &mut policy(|op, cost| {
                        assert_eq!((op, cost), checkpoints[seen]);
                        let i = seen;
                        seen += 1;
                        if i == reject {
                            Err(i)
                        } else {
                            Ok(())
                        }
                    }),
                );
                assert!(matches!(failed, Err(ShapeOperationError::Admission(i)) if i == reject));
                assert_eq!(seen, reject + 1);
                let capacity = (b.info.capacity(), b.pos.capacity());
                b.allocated_var_bits = 0;
                let mut recovered = UnicodeBuffer(b);
                recovered.clear();
                assert_eq!(
                    capacity,
                    (recovered.0.info.capacity(), recovered.0.pos.capacity())
                );
                recovered.push_str("AB");
                recovered.set_direction(Direction::LeftToRight);
                let output = shaper.shape_with_plan(&plan, recovered, &[]);
                assert_eq!(
                    bytemuck::cast_slice::<crate::GlyphInfo, u8>(output.glyph_infos()),
                    bytemuck::cast_slice::<crate::GlyphInfo, u8>(expected.glyph_infos()),
                );
                assert_eq!(
                    bytemuck::cast_slice::<crate::GlyphPosition, u8>(output.glyph_positions()),
                    bytemuck::cast_slice::<crate::GlyphPosition, u8>(expected.glyph_positions()),
                );
            }
        }
    }
}

#[test]
fn grapheme_internal_merge_checked_bound_rejects_before_records() {
    let mut b = input(2, 0, false, false);
    b.out_len = usize::MAX;
    let result = crate::hb::buffer::admission::admit_group_step(
        GroupStep::Merge { start: 0, end: 2 },
        &b,
        &mut |_, _| Ok::<_, ()>(()),
    );
    assert!(matches!(result, Err(ShapeOperationError::CostOverflow)));
}
