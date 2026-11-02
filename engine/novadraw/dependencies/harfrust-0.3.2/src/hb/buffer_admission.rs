// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Input buffer admission only. This does not certify shaping or font caches.

use super::{hb_buffer_t, UnicodeBuffer};
use alloc::collections::TryReserveError;
use core::fmt;

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
    fn ensure_admitted<E>(
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
}
