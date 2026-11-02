// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private policy shared by ordinary and admitted line breaking.

use super::{LayoutOperation, LayoutOperationCost, LayoutOperationError, LayoutStorageField};
use alloc::vec::Vec;
use core::convert::Infallible;

pub(super) trait Meter {
    type Error;
    const ENABLED: bool;

    fn charge(
        &mut self,
        step: LayoutOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), LayoutOperationError<Self::Error>>;
}

pub(super) struct Ordinary;

impl Meter for Ordinary {
    type Error = Infallible;
    const ENABLED: bool = false;

    #[inline]
    fn charge(
        &mut self,
        _: LayoutOperation,
        _: usize,
        _: usize,
    ) -> Result<(), LayoutOperationError<Self::Error>> {
        Ok(())
    }
}

pub(super) struct Admitted<F>(pub F);

impl<E, F: FnMut(LayoutOperation, LayoutOperationCost) -> Result<(), E>> Meter for Admitted<F> {
    type Error = E;
    const ENABLED: bool = true;

    fn charge(
        &mut self,
        step: LayoutOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), LayoutOperationError<E>> {
        self.0(step, LayoutOperationCost { work, allocation })
            .map_err(LayoutOperationError::Admission)
    }
}

pub(super) fn scan<M: Meter>(
    meter: &mut M,
    step: LayoutOperation,
    count: usize,
    passes: usize,
) -> Result<(), LayoutOperationError<M::Error>> {
    if M::ENABLED {
        let work = count
            .checked_mul(passes)
            .ok_or(LayoutOperationError::CostOverflow)?;
        meter.charge(step, work, 0)?;
    }
    Ok(())
}

pub(super) fn push<T, M: Meter>(
    values: &mut Vec<T>,
    value: T,
    field: LayoutStorageField,
    meter: &mut M,
) -> Result<(), LayoutOperationError<M::Error>> {
    if M::ENABLED {
        if values.len() == values.capacity() {
            // Geometric growth avoids one reallocation for every output item.
            // Charge the entire new reservation, conservatively including the
            // old slots, and all initialized elements potentially relocated.
            let capacity = values
                .capacity()
                .checked_mul(2)
                .and_then(|grown| grown.checked_add(usize::from(grown == 0)))
                .ok_or(LayoutOperationError::CostOverflow)?;
            meter.charge(LayoutOperation::LineStorage, values.len(), capacity)?;
            values
                .try_reserve_exact(capacity - values.len())
                .map_err(|source| LayoutOperationError::Storage { field, source })?;
        }
        meter.charge(LayoutOperation::LineStorage, 1, 0)?;
    }
    values.push(value);
    Ok(())
}
