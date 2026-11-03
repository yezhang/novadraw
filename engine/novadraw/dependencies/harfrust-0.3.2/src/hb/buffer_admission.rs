// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Input buffer admission only. This does not certify shaping or font caches.

use super::{hb_buffer_t, UnicodeBuffer};
use alloc::collections::TryReserveError;
use core::fmt;

/// Private insertion-sort checkpoints. Comparators read only stored fields.
pub(crate) enum SortStep {
    Begin,
    Item,
    Compare,
    Destination,
    Move { distance: usize },
}

/// Checkpoints for fixed stored-property grouping. These fees do not certify
/// arbitrary user callbacks passed to the ordinary grouping helpers.
pub(crate) enum GroupStep {
    Begin,
    Compare,
    Advance,
    Merge { start: usize, end: usize },
    Unsafe { start: usize, end: usize },
    Reverse { start: usize, end: usize },
}

pub(crate) fn admit_group_step<E>(
    step: GroupStep,
    buffer: &hb_buffer_t,
    admit: &mut impl FnMut(
        crate::hb::face::shape_admission::ShapeOperation,
        crate::hb::face::shape_admission::ShapeOperationCost,
    ) -> Result<(), E>,
) -> Result<(), crate::hb::face::shape_admission::ShapeOperationError<E>> {
    use crate::hb::face::shape_admission::{
        ShapeOperation, ShapeOperationCost, ShapeOperationError,
    };
    let mut charge = |op, work| {
        admit(
            op,
            ShapeOperationCost {
                work,
                allocation: 0,
            },
        )
        .map_err(ShapeOperationError::Admission)
    };
    let product = |n: usize, factor| {
        n.checked_mul(factor)
            .ok_or(ShapeOperationError::CostOverflow)
    };
    match step {
        GroupStep::Begin | GroupStep::Advance => charge(ShapeOperation::Planning, 4),
        GroupStep::Compare => {
            charge(ShapeOperation::Planning, 4)?;
            // Two record references and one stored continuation field.
            charge(ShapeOperation::Information, 3)
        }
        GroupStep::Merge { start, end } => {
            charge(ShapeOperation::Planning, 2)?;
            if end - start < 2 {
                return Ok(());
            }
            // Same repeated minimum/extension/cluster-write bound as sort,
            // without its insertion shifts. Output prefixes may be revisited.
            let n = buffer
                .len
                .checked_add(buffer.out_len)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let planning = product(n, 16)?
                .checked_add(16)
                .ok_or(ShapeOperationError::CostOverflow)?;
            charge(ShapeOperation::Planning, planning)?;
            charge(ShapeOperation::Information, product(n, 12)?)
        }
        GroupStep::Unsafe { start, end } => {
            charge(ShapeOperation::Planning, 16)?;
            let span = end.wrapping_sub(start);
            // Preserve _set_glyph_flags' 255-element short circuit.
            if !(2..=255).contains(&span) {
                return Ok(());
            }
            // Local minimum scan, endpoint reads, then a repeated cluster
            // predicate and mask read/write pass. Include terminal reads and
            // both monotone traversal directions, independently per call.
            let planning = product(span, 16)?
                .checked_add(32)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let records = product(span, 12)?
                .checked_add(12)
                .ok_or(ShapeOperationError::CostOverflow)?;
            charge(ShapeOperation::Planning, planning)?;
            charge(ShapeOperation::Information, records)
        }
        GroupStep::Reverse { start, end } => {
            charge(ShapeOperation::Planning, 4)?;
            let span = end - start;
            if span < 2 {
                return Ok(());
            }
            charge(ShapeOperation::Information, product(span, 2)?)?;
            if buffer.have_positions {
                charge(ShapeOperation::Positions, product(span, 2)?)?;
            }
            Ok(())
        }
    }
}

/// Admission for stored-property sorting, including every repeated cluster merge.
/// This does not admit an arbitrary comparator, callback or shaping pipeline.
pub(crate) fn admit_sort_step<E>(
    step: SortStep,
    buffer: &hb_buffer_t,
    admit: &mut impl FnMut(
        crate::hb::face::shape_admission::ShapeOperation,
        crate::hb::face::shape_admission::ShapeOperationCost,
    ) -> Result<(), E>,
) -> Result<(), crate::hb::face::shape_admission::ShapeOperationError<E>> {
    use crate::hb::face::shape_admission::{
        ShapeOperation, ShapeOperationCost, ShapeOperationError,
    };
    let mut charge = |op, work| {
        admit(
            op,
            ShapeOperationCost {
                work,
                allocation: 0,
            },
        )
        .map_err(ShapeOperationError::Admission)
    };
    match step {
        SortStep::Begin => charge(ShapeOperation::Planning, 2),
        SortStep::Item => charge(ShapeOperation::Planning, 2),
        SortStep::Compare => {
            // Two records, two stored CCC fields, comparison and loop control.
            charge(ShapeOperation::Planning, 4)?;
            charge(ShapeOperation::Information, 4)
        }
        SortStep::Destination => charge(ShapeOperation::Planning, 2),
        SortStep::Move { distance } => {
            // A merge may extend over the whole live input and output. Its
            // minimum/extension/unsafe/cluster-write passes revisit records.
            // Pay this bound anew for EACH move, before merge_clusters.
            const MERGE_RECORD_VISITS: usize = 12;
            const MERGE_SCALAR_VISITS: usize = 16;
            let records = buffer
                .len
                .checked_add(buffer.out_len)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let merge = records
                .checked_mul(MERGE_RECORD_VISITS)
                .ok_or(ShapeOperationError::CostOverflow)?;
            // Read temporary, read/write each shifted record, write temporary.
            let shifts = distance
                .checked_mul(2)
                .and_then(|v| v.checked_add(2))
                .ok_or(ShapeOperationError::CostOverflow)?;
            let work = merge
                .checked_add(shifts)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let planning = records
                .checked_mul(MERGE_SCALAR_VISITS)
                .and_then(|v| v.checked_add(distance))
                .and_then(|v| v.checked_add(MERGE_SCALAR_VISITS))
                .ok_or(ShapeOperationError::CostOverflow)?;
            charge(ShapeOperation::Planning, planning)?;
            charge(ShapeOperation::Information, work)
        }
    }
}

/// An input operation about to execute.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferOperation {
    /// Read the two buffer array headers to plan growth.
    Planning,
    /// Scan UTF-8 bytes to count input characters.
    Scan,
    /// Reserve new array capacity and potentially move existing elements.
    Storage,
    /// Initialize newly exposed info and position elements.
    Initialize,
    /// Decode input and write character and cluster records.
    Append,
}

/// Logical costs, excluding physical allocator rounding and bookkeeping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferOperationCost {
    /// Input byte or initialized array element visits, including repeated passes.
    pub work: usize,
    /// New collection capacity slots; this is not a byte count.
    pub allocation: usize,
}

/// The array whose exact reservation failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferStorageField {
    /// Unicode/glyph information.
    Info,
    /// Glyph positions and the alternate output buffer.
    Positions,
}

/// Failure before publishing additional input characters.
#[derive(Debug)]
pub enum BufferOperationError<E> {
    /// Caller rejection, preserving the original error.
    Admission(E),
    /// Exact capacity reservation failed.
    Storage {
        /// The destination array.
        field: BufferStorageField,
        /// The original reservation error.
        source: TryReserveError,
    },
    /// A cost or target length overflowed.
    CostOverflow,
    /// The requested length exceeds Harfrust's existing buffer limit.
    LengthLimit,
}

impl<E: fmt::Display> fmt::Display for BufferOperationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "buffer operation rejected: {error}"),
            Self::Storage { field, source } => {
                write!(f, "buffer {field:?} reservation failed: {source}")
            }
            Self::CostOverflow => f.write_str("buffer operation cost overflow"),
            Self::LengthLimit => f.write_str("buffer length limit exceeded"),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for BufferOperationError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Storage { source, .. } => Some(source),
            Self::CostOverflow | Self::LengthLimit => None,
        }
    }
}

fn charge<E>(
    admit: &mut impl FnMut(BufferOperation, BufferOperationCost) -> Result<(), E>,
    operation: BufferOperation,
    work: usize,
    allocation: usize,
) -> Result<(), BufferOperationError<E>> {
    admit(operation, BufferOperationCost { work, allocation })
        .map_err(BufferOperationError::Admission)
}

impl hb_buffer_t {
    pub(crate) fn ensure_admitted<E>(
        &mut self,
        size: usize,
        admit: &mut impl FnMut(BufferOperation, BufferOperationCost) -> Result<(), E>,
    ) -> Result<(), BufferOperationError<E>> {
        // Two arrays, matching the two growth owners below.
        const ARRAY_HEADERS: usize = 2;
        charge(admit, BufferOperation::Planning, ARRAY_HEADERS, 0)?;
        if size > self.max_len {
            return Err(BufferOperationError::LengthLimit);
        }
        if size <= self.info.len() {
            debug_assert!(size <= self.pos.len());
            return Ok(());
        }
        let info_grows = size > self.info.capacity();
        let pos_grows = size > self.pos.capacity();
        // A fresh allocation can migrate the entire old initialized array.
        // Charge the entire new capacity, not merely the delta.
        let moves = usize::from(info_grows)
            .checked_mul(self.info.len())
            .and_then(|v| {
                usize::from(pos_grows)
                    .checked_mul(self.pos.len())
                    .and_then(|p| v.checked_add(p))
            })
            .ok_or(BufferOperationError::CostOverflow)?;
        let slots = (usize::from(info_grows) + usize::from(pos_grows))
            .checked_mul(size)
            .ok_or(BufferOperationError::CostOverflow)?;
        if info_grows || pos_grows {
            charge(admit, BufferOperation::Storage, moves, slots)?;
            self.info
                .try_reserve_exact(size - self.info.len())
                .map_err(|source| BufferOperationError::Storage {
                    field: BufferStorageField::Info,
                    source,
                })?;
            self.pos
                .try_reserve_exact(size - self.pos.len())
                .map_err(|source| BufferOperationError::Storage {
                    field: BufferStorageField::Positions,
                    source,
                })?;
        }
        let initialize = (size - self.info.len())
            .checked_add(size - self.pos.len())
            .ok_or(BufferOperationError::CostOverflow)?;
        charge(admit, BufferOperation::Initialize, initialize, 0)?;
        // Both reservations and admission precede either resize. No allocating
        // resize remains, and rejection cannot break the equal-length invariant.
        self.resize_storage(size);
        Ok(())
    }
}

impl UnicodeBuffer {
    /// Ensures space for `size` input characters after operation admission.
    ///
    /// Charges header planning, old initialized elements potentially moved,
    /// whole new capacity slots, and both array initialization passes. Global
    /// Vec exact-reservation slot behavior is verified on Rust 1.96; physical
    /// allocator rounding is outside these logical units.
    ///
    /// On failure, existing characters and settings remain available. Fully
    /// reserved spare capacity may remain; costs are not refunded. This method
    /// does not provide admission for subsequent ordinary shaping.
    pub fn try_reserve_admitted<E>(
        &mut self,
        size: usize,
        mut admit: impl FnMut(BufferOperation, BufferOperationCost) -> Result<(), E>,
    ) -> Result<(), BufferOperationError<E>> {
        self.0.ensure_admitted(size, &mut admit)
    }

    /// Appends UTF-8 input after admitting counting, growth, and writing.
    ///
    /// Each decoding pass is bounded by input bytes; writing also visits each
    /// character record once. Cluster indices, including truncation to u32,
    /// match `push_str`. Failure publishes no additional characters and leaves
    /// the buffer reusable, though admitted spare storage may remain.
    pub fn try_push_str_admitted<E>(
        &mut self,
        text: &str,
        mut admit: impl FnMut(BufferOperation, BufferOperationCost) -> Result<(), E>,
    ) -> Result<(), BufferOperationError<E>> {
        charge(&mut admit, BufferOperation::Scan, text.len(), 0)?;
        let count = text.chars().count();
        let size = self
            .0
            .len
            .checked_add(count)
            .ok_or(BufferOperationError::CostOverflow)?;
        self.0.ensure_admitted(size, &mut admit)?;
        let work = text
            .len()
            .checked_add(count)
            .ok_or(BufferOperationError::CostOverflow)?;
        charge(&mut admit, BufferOperation::Append, work, 0)?;
        self.0.push_reserved_str(text);
        Ok(())
    }

    /// Appends one character and cluster after growth and write admission.
    ///
    /// Failure preserves existing characters and post-context. Successful
    /// append clears the post-context exactly as `add` does.
    pub fn try_add_admitted<E>(
        &mut self,
        codepoint: char,
        cluster: u32,
        mut admit: impl FnMut(BufferOperation, BufferOperationCost) -> Result<(), E>,
    ) -> Result<(), BufferOperationError<E>> {
        let size = self
            .0
            .len
            .checked_add(1)
            .ok_or(BufferOperationError::CostOverflow)?;
        self.0.ensure_admitted(size, &mut admit)?;
        charge(&mut admit, BufferOperation::Append, 1, 0)?;
        self.0.add_reserved(codepoint as u32, cluster);
        self.0.context_len[1] = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::GlyphInfo;
    use super::*;
    use alloc::vec::Vec;
    use core::convert::Infallible;

    fn allow(_: BufferOperation, _: BufferOperationCost) -> Result<(), Infallible> {
        Ok(())
    }

    fn info_bytes(values: &[GlyphInfo]) -> &[u8] {
        bytemuck::cast_slice(values)
    }

    #[test]
    fn every_rejection_preserves_logical_input_and_array_invariant() {
        for text in ["", "ASCII", "é中🙂", "a\u{301} العربية"] {
            let mut complete = UnicodeBuffer::new();
            complete.add('x', 91);
            let mut steps = Vec::new();
            complete
                .try_push_str_admitted(text, |op, cost| {
                    steps.push((op, cost));
                    Ok::<_, Infallible>(())
                })
                .unwrap();
            for reject in 0..steps.len() {
                let mut buffer = UnicodeBuffer::new();
                buffer.add('x', 91);
                buffer.set_post_context("abc");
                let original = buffer.0.info[..buffer.len()].to_vec();
                let context = buffer.0.context;
                let context_len = buffer.0.context_len;
                let mut seen = 0;
                let result = buffer.try_push_str_admitted(text, |op, cost| {
                    assert_eq!((op, cost), steps[seen]);
                    let index = seen;
                    seen += 1;
                    if index == reject {
                        Err(index)
                    } else {
                        Ok(())
                    }
                });
                assert!(matches!(result, Err(BufferOperationError::Admission(i)) if i == reject));
                assert_eq!(seen, reject + 1);
                assert_eq!(buffer.len(), 1);
                assert_eq!(
                    info_bytes(&buffer.0.info[..buffer.len()]),
                    info_bytes(&original)
                );
                assert_eq!(buffer.0.context, context);
                assert_eq!(buffer.0.context_len, context_len);
                assert_eq!(buffer.0.info.len(), buffer.0.pos.len());
                // Ordinary input can still be appended after every failure.
                buffer.add('y', 92);
                assert_eq!(buffer.len(), 2);
                assert_eq!(buffer.0.info[1].cluster, 92);
            }
        }
    }

    #[test]
    fn cold_and_warm_growth_charge_capacity_migration_and_initialization() {
        let mut buffer = UnicodeBuffer::new();
        let mut steps = Vec::new();
        buffer
            .try_reserve_admitted(7, |op, cost| {
                steps.push((op, cost));
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(
            steps,
            [
                (
                    BufferOperation::Planning,
                    BufferOperationCost {
                        work: 2,
                        allocation: 0
                    }
                ),
                (
                    BufferOperation::Storage,
                    BufferOperationCost {
                        work: 0,
                        allocation: 14
                    }
                ),
                (
                    BufferOperation::Initialize,
                    BufferOperationCost {
                        work: 14,
                        allocation: 0
                    }
                ),
            ]
        );
        assert_eq!(buffer.0.info.capacity(), 7);
        assert_eq!(buffer.0.pos.capacity(), 7);
        steps.clear();
        buffer
            .try_reserve_admitted(8, |op, cost| {
                steps.push((op, cost));
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(
            steps[1].1,
            BufferOperationCost {
                work: 14,
                allocation: 16
            }
        );
        assert_eq!(
            steps[2].1,
            BufferOperationCost {
                work: 2,
                allocation: 0
            }
        );
        buffer.clear();
        steps.clear();
        buffer
            .try_reserve_admitted(5, |op, cost| {
                steps.push((op, cost));
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[1],
            (
                BufferOperation::Initialize,
                BufferOperationCost {
                    work: 10,
                    allocation: 0
                }
            )
        );
    }

    #[test]
    fn add_rejection_keeps_post_context_and_commit_clears_it() {
        let mut buffer = UnicodeBuffer::new();
        buffer.set_post_context("hello");
        for reject in 0..4 {
            let before = buffer.0.context_len;
            let mut seen = 0;
            let result = buffer.try_add_admitted('中', u32::MAX, |_, _| {
                let index = seen;
                seen += 1;
                if index == reject {
                    Err(())
                } else {
                    Ok(())
                }
            });
            // Fresh capacity is retained on rejection; reset to exercise each step.
            assert!(result.is_err());
            assert_eq!(buffer.len(), 0);
            assert_eq!(buffer.0.context_len, before);
            buffer = UnicodeBuffer::new();
            buffer.set_post_context("hello");
        }
        buffer.try_add_admitted('中', u32::MAX, allow).unwrap();
        assert_eq!(buffer.0.context_len[1], 0);
        assert_eq!(buffer.0.info[0].cluster, u32::MAX);
    }

    #[test]
    fn shared_input_kernel_matches_original_clusters() {
        let mut ordinary = UnicodeBuffer::new();
        let mut admitted = UnicodeBuffer::new();
        for text in ["a中", "", "é🙂", "العربية"] {
            ordinary.push_str(text);
            admitted.try_push_str_admitted(text, allow).unwrap();
            assert_eq!(ordinary.len(), admitted.len());
            assert_eq!(
                info_bytes(&ordinary.0.info[..ordinary.len()]),
                info_bytes(&admitted.0.info[..admitted.len()])
            );
        }
    }

    #[test]
    fn limit_and_overflow_do_not_mark_existing_buffer_unsuccessful() {
        let mut buffer = UnicodeBuffer::new();
        buffer.add('x', 0);
        let before = buffer.0.info[..buffer.len()].to_vec();
        assert!(matches!(
            buffer.try_reserve_admitted(usize::MAX, allow),
            Err(BufferOperationError::LengthLimit)
        ));
        buffer.0.max_len = usize::MAX;
        assert!(matches!(
            buffer.try_reserve_admitted(usize::MAX, allow),
            Err(BufferOperationError::CostOverflow)
        ));
        assert!(buffer.0.successful);
        assert_eq!(
            info_bytes(&buffer.0.info[..buffer.len()]),
            info_bytes(&before)
        );
        buffer.add('y', 1);
        assert_eq!(buffer.len(), 2);
    }

    fn sorting_input(pattern: usize, level: u32, output: bool) -> UnicodeBuffer {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str("a\u{315}\u{300}\u{316}\u{315}\u{300}\u{316}b");
        buffer.0.cluster_level = level;
        for (i, info) in buffer.0.info[..buffer.0.len].iter_mut().enumerate() {
            info.cluster = match pattern {
                0 => i as u32,
                1 => 7 - i as u32,
                2 => 4,
                _ => [4, 7, 7, 2, 2, 7, 7, 1][i],
            };
            let mut flags = 0;
            info.init_unicode_props(&mut flags);
        }
        if output {
            buffer.0.clear_output();
            buffer.0.next_glyph();
            // Create a separate output prefix. Its clusters can participate
            // in repeated merges, even though the sorted range is input.
            buffer.0.output_glyph('a' as u32);
        }
        buffer
    }

    fn sorting_state(buffer: &hb_buffer_t) -> (Vec<u8>, Vec<u8>, i32, bool) {
        (
            bytemuck::cast_slice(&buffer.info).to_vec(),
            bytemuck::cast_slice(&buffer.pos).to_vec(),
            buffer.max_ops,
            buffer.successful,
        )
    }

    // Frozen upstream insertion-sort logic: a test oracle, not an execution
    // path. Preserve its merge ordering and comparator's original i index.
    fn original_sort(buffer: &mut hb_buffer_t, start: usize, end: usize) {
        for i in start + 1..end {
            let mut j = i;
            while j > start
                && buffer.info[j - 1].modified_combining_class()
                    > buffer.info[i].modified_combining_class()
            {
                j -= 1;
            }
            if i == j {
                continue;
            }
            buffer.merge_clusters(j, i + 1);
            let t = buffer.info[i];
            for idx in (0..i - j).rev() {
                buffer.info[idx + j + 1] = buffer.info[idx + j];
            }
            buffer.info[j] = t;
        }
    }

    #[test]
    fn sorting_shared_kernel_matches_original_with_repeated_merges() {
        for pattern in 0..4 {
            for level in 0..4 {
                for output in [false, true] {
                    let mut original = sorting_input(pattern, level, output);
                    let mut admitted = sorting_input(pattern, level, output);
                    original_sort(&mut original.0, 1, 7);
                    let mut moves = 0;
                    admitted
                        .0
                        .sort_with(
                            1,
                            7,
                            |a, b| a.modified_combining_class() > b.modified_combining_class(),
                            |step, buffer| {
                                moves += usize::from(matches!(step, SortStep::Move { .. }));
                                admit_sort_step(step, buffer, &mut |_, cost| {
                                    assert_eq!(cost.allocation, 0);
                                    Ok::<_, Infallible>(())
                                })
                            },
                        )
                        .unwrap();
                    assert!(moves > 1);
                    assert_eq!(sorting_state(&original.0), sorting_state(&admitted.0));
                }
            }
        }
    }

    #[test]
    fn sorting_rejects_before_each_comparison_merge_and_shift_and_can_clear() {
        use crate::hb::face::shape_admission::ShapeOperationError;
        for pattern in 0..4 {
            for level in 0..4 {
                for output in [false, true] {
                    let mut complete = sorting_input(pattern, level, output);
                    let mut checkpoints = Vec::new();
                    complete
                        .0
                        .sort_with(
                            1,
                            7,
                            |a, b| a.modified_combining_class() > b.modified_combining_class(),
                            |step, buffer| {
                                admit_sort_step(step, buffer, &mut |op, cost| {
                                    checkpoints.push((op, cost, sorting_state(buffer)));
                                    Ok::<_, Infallible>(())
                                })
                            },
                        )
                        .unwrap();
                    for reject in 0..checkpoints.len() {
                        let mut buffer = sorting_input(pattern, level, output);
                        let mut seen = 0;
                        let result = buffer.0.sort_with(
                            1,
                            7,
                            |a, b| a.modified_combining_class() > b.modified_combining_class(),
                            |step, buffer| {
                                admit_sort_step(step, buffer, &mut |op, cost| {
                                    assert_eq!(
                                        (op, cost),
                                        (checkpoints[seen].0, checkpoints[seen].1)
                                    );
                                    assert_eq!(sorting_state(buffer), checkpoints[seen].2);
                                    let current = seen;
                                    seen += 1;
                                    if current == reject {
                                        Err(current)
                                    } else {
                                        Ok(())
                                    }
                                })
                            },
                        );
                        assert!(
                            matches!(result, Err(ShapeOperationError::Admission(i)) if i == reject)
                        );
                        assert_eq!(seen, reject + 1);
                        assert_eq!(sorting_state(&buffer.0), checkpoints[reject].2);
                        assert_eq!(buffer.0.info.len(), buffer.0.pos.len());
                        buffer.clear();
                        buffer.push_str("again");
                        assert_eq!(buffer.len(), 5);
                    }
                }
            }
        }
    }

    #[test]
    fn sorting_move_cost_counts_full_live_input_and_output_on_every_call() {
        use crate::hb::face::shape_admission::{ShapeOperation, ShapeOperationError};
        let mut buffer = sorting_input(3, 0, true);
        let mut costs = Vec::new();
        for _ in 0..3 {
            admit_sort_step(
                SortStep::Move { distance: 5 },
                &buffer.0,
                &mut |op, cost| {
                    costs.push((op, cost));
                    Ok::<_, Infallible>(())
                },
            )
            .unwrap();
        }
        assert_eq!(costs.len(), 6);
        for pair in costs.chunks_exact(2) {
            assert_eq!(pair[0].0, ShapeOperation::Planning);
            assert_eq!(pair[0].1.work, (8 + 2) * 16 + 5 + 16);
            assert_eq!(pair[1].0, ShapeOperation::Information);
            assert_eq!(pair[1].1.work, (8 + 2) * 12 + 5 * 2 + 2);
        }
        buffer.0.len = usize::MAX;
        let mut called = false;
        let error = admit_sort_step(SortStep::Move { distance: 1 }, &buffer.0, &mut |_, _| {
            called = true;
            Ok::<_, Infallible>(())
        });
        assert!(matches!(error, Err(ShapeOperationError::CostOverflow)));
        assert!(!called);
    }
}
