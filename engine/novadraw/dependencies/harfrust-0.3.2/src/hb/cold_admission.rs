// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Operation policy shared by cold shaper construction kernels.

use alloc::{collections::TryReserveError, vec::Vec};
use core::{convert::Infallible, fmt};
use read_fonts::FontRef;

/// OpenType table whose lookup cache is being prepared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColdLookupTable {
    /// GSUB substitution lookups.
    Substitution,
    /// GPOS positioning lookups.
    Positioning,
}

/// A cold shaper operation about to execute.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColdOperation {
    /// Inspect a collection length or a fixed-size planning header.
    Planning,
    /// Search font directory records.
    Directory,
    /// Parse a fixed table header or read scalar fields.
    TableRead,
    /// Visit a cmap encoding record, including repeated priority scans.
    CmapRecord,
    /// Resolve and parse a cmap subtable.
    CmapSubtable,
    /// Reserve heap capacity before initializing cache elements.
    Storage,
    /// Initialize cache elements, including their eventual cleanup.
    Initialize,
    /// Visit a mark coverage offset.
    MarkSet,
    /// Visit coverage elements and update a fixed digest.
    Coverage,
    /// Visit a lookup or one of its subtable offsets.
    Lookup,
    /// Visit a ligature-set or ligature record.
    Ligature,
    /// Visit shape-plan feature or stage elements.
    Feature,
    /// Compare or move elements during admitted sorting.
    Sort,
}

/// Logical costs of a cold shaper operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColdOperationCost {
    /// Input bytes or element visits, including repeated passes.
    pub work: usize,
    /// Whole new heap capacity slots, not bytes.
    pub allocation: usize,
}

/// Failure before a complete cold cache is returned.
#[derive(Debug)]
pub enum ColdOperationError<E> {
    /// Caller rejection with the original source.
    Admission(E),
    /// Heap storage could not be reserved.
    Storage(TryReserveError),
    /// A logical cost cannot be represented.
    CostOverflow,
}

impl<E: fmt::Display> fmt::Display for ColdOperationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "cold shaper operation rejected: {error}"),
            Self::Storage(error) => write!(f, "cold shaper storage failed: {error}"),
            Self::CostOverflow => f.write_str("cold shaper cost overflow"),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for ColdOperationError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::CostOverflow => None,
        }
    }
}

pub(crate) trait ColdAdmission {
    type Error;

    /// Ordinary kernels permit all algorithm paths; admitted kernels can
    /// decline a path before its currently unproved work starts.
    const ADMITTED: bool;

    fn charge(
        &mut self,
        operation: ColdOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), Self::Error>;

    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Error>;
}

pub(crate) struct OrdinaryCold;

/// One heap element with fallible reservation on the admitted path.
/// Keeps external caches out of the large inline enum without Box::new.
pub(crate) struct HeapSlot<T>(Vec<T>);

impl<T> HeapSlot<T> {
    pub(crate) fn new_with<P: ColdAdmission>(value: T, policy: &mut P) -> Result<Self, P::Error> {
        let mut values = Vec::new();
        policy.reserve(&mut values, 1)?;
        // Moving the value into storage and its eventual destruction.
        policy.charge(ColdOperation::Initialize, 2, 0)?;
        values.push(value);
        Ok(Self(values))
    }
}

impl<T> core::ops::Deref for HeapSlot<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0[0]
    }
}

impl ColdAdmission for OrdinaryCold {
    type Error = Infallible;
    const ADMITTED: bool = false;

    #[inline]
    fn charge(&mut self, _: ColdOperation, _: usize, _: usize) -> Result<(), Infallible> {
        Ok(())
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Infallible> {
        values.reserve(count);
        Ok(())
    }
}

pub(crate) struct AdmittedCold<F>(pub F);

impl<E, F> ColdAdmission for AdmittedCold<F>
where
    F: FnMut(ColdOperation, ColdOperationCost) -> Result<(), E>,
{
    type Error = ColdOperationError<E>;
    const ADMITTED: bool = true;

    fn charge(
        &mut self,
        operation: ColdOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), Self::Error> {
        (self.0)(operation, ColdOperationCost { work, allocation })
            .map_err(ColdOperationError::Admission)
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Error> {
        self.charge(ColdOperation::Planning, 2, 0)?;
        let required = values
            .len()
            .checked_add(count)
            .ok_or(ColdOperationError::CostOverflow)?;
        if required > values.capacity() {
            self.charge(ColdOperation::Storage, values.len(), required)?;
            values
                .try_reserve_exact(count)
                .map_err(ColdOperationError::Storage)?;
        }
        Ok(())
    }
}

pub(crate) fn directory<P: ColdAdmission>(font: &FontRef, policy: &mut P) -> Result<(), P::Error> {
    policy.charge(ColdOperation::Planning, 1, 0)?;
    let records = font.table_directory().table_records();
    // TableProvider can use a linear search on an unsorted directory.
    // Binary search uses at most n+1 tag comparisons. A matched record's
    // offset and length contribute two additional field reads.
    policy.charge(ColdOperation::Directory, records.len() + 3, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn reservation_overflow_precedes_allocation_admission() {
        let mut calls = Vec::new();
        let mut policy = AdmittedCold(|op, cost| {
            calls.push((op, cost));
            Ok::<_, Infallible>(())
        });
        let mut values = vec![1u8];
        assert!(matches!(
            policy.reserve(&mut values, usize::MAX),
            Err(ColdOperationError::CostOverflow)
        ));
        assert_eq!(values, [1]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, ColdOperation::Planning);
        assert_eq!(calls[0].1.allocation, 0);
    }
}
