// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Admission for user-space normalization with absent or version 1 avar.

use super::admission::{directory, InstanceAdmission, OrdinaryInstance};
use super::ShaperInstance;
use crate::{FontRef, InstanceOperation, InstanceOperationCost, InstanceStorageError, Variation};
use core::{convert::Infallible, fmt};
use read_fonts::{
    tables::{avar::Avar, fvar::Fvar},
    types::{Fixed, Tag},
    FontRead, TableProvider,
};

/// Operation about to execute while constructing a user-space instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariationOperation {
    /// An existing normalized-instance operation.
    Instance(InstanceOperation),
    /// Read the avar version/count or one segment-map count for planning.
    MappingPlan,
    /// Parse the avar header and its variable-length map headers.
    MappingRead,
    /// Normalize all supplied values, including repeated axis and map scans.
    Normalization,
}

/// Failure before returning a complete user-space instance.
#[derive(Debug)]
pub enum VariationOperationError<E> {
    /// Caller rejection with the original error object.
    Admission(E),
    /// The original SmallVec storage failure.
    Storage(InstanceStorageError),
    /// The source-derived logical cost cannot be represented by usize.
    Arithmetic,
}

impl<E: fmt::Display> fmt::Display for VariationOperationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "variation operation rejected: {error}"),
            Self::Storage(error) => write!(f, "variation storage failed: {error}"),
            Self::Arithmetic => f.write_str("variation operation cost overflow"),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for VariationOperationError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::Arithmetic => None,
        }
    }
}

pub(crate) trait VariationAdmission: InstanceAdmission {
    /// None rejects an unsupported path before normalization or storage.
    fn mapping<'a>(
        &mut self,
        font: &FontRef<'a>,
        axes: usize,
    ) -> Result<Option<Option<Avar<'a>>>, Self::Error>;

    fn normalization(&mut self, axes: usize, input_len: Option<usize>) -> Result<(), Self::Error>;
}

impl VariationAdmission for OrdinaryInstance {
    fn mapping<'a>(
        &mut self,
        font: &FontRef<'a>,
        _: usize,
    ) -> Result<Option<Option<Avar<'a>>>, Infallible> {
        Ok(Some(font.avar().ok()))
    }

    #[inline]
    fn normalization(&mut self, _: usize, _: Option<usize>) -> Result<(), Infallible> {
        Ok(())
    }
}

struct AdmittedVariation<F> {
    callback: F,
    // Upper bound for a borrowed SegmentMaps::apply + VarLenArray::get call.
    mapping_work: usize,
}

impl<E, F> AdmittedVariation<F>
where
    F: FnMut(VariationOperation, InstanceOperationCost) -> Result<(), E>,
{
    fn charge_variation(
        &mut self,
        operation: VariationOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), VariationOperationError<E>> {
        (self.callback)(operation, InstanceOperationCost { work, allocation })
            .map_err(VariationOperationError::Admission)
    }
}

impl<E, F> InstanceAdmission for AdmittedVariation<F>
where
    F: FnMut(VariationOperation, InstanceOperationCost) -> Result<(), E>,
{
    type Error = VariationOperationError<E>;

    fn charge(
        &mut self,
        operation: InstanceOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), Self::Error> {
        self.charge_variation(VariationOperation::Instance(operation), work, allocation)
    }

    fn reserve(&mut self, instance: &mut ShaperInstance, count: usize) -> Result<(), Self::Error> {
        self.charge(InstanceOperation::Planning, 1, 0)?;
        if count > instance.coords.capacity() {
            // Candidate is empty: no old initialized elements can migrate.
            self.charge(InstanceOperation::Storage, 0, count)?;
            instance
                .coords
                .try_reserve_exact(count)
                .map_err(|error| VariationOperationError::Storage(InstanceStorageError(error)))?;
        }
        Ok(())
    }
}

impl<E, F> VariationAdmission for AdmittedVariation<F>
where
    F: FnMut(VariationOperation, InstanceOperationCost) -> Result<(), E>,
{
    fn mapping<'a>(
        &mut self,
        font: &FontRef<'a>,
        axes: usize,
    ) -> Result<Option<Option<Avar<'a>>>, Self::Error> {
        directory(font, self)?;
        let Some(data) = font.table_data(Tag::new(b"avar")) else {
            return Ok(Some(None));
        };
        // Read only the fixed version/count. No generated parser has run yet.
        self.charge_variation(VariationOperation::MappingPlan, 8, 0)?;
        let Ok(version) = data.read_at::<u32>(0) else {
            return Ok(Some(None));
        };
        if version != 0x0001_0000 {
            // Including avar2: do not enter delta-store evaluation.
            return Ok(None);
        }
        let Ok(count) = data.read_at::<u16>(6) else {
            return Ok(Some(None));
        };
        // generated_avar::read walks count headers via total_len_for_count.
        // Each step reads u16 and visits a map; payload arrays are borrowed.
        // No allocation or traversal of mapping pairs happens in the parser.
        self.charge_variation(
            VariationOperation::MappingRead,
            8 + usize::from(count) * 3,
            0,
        )?;
        let Ok(avar) = Avar::read(data) else {
            return Ok(Some(None));
        };
        // VarLenArray::get does not enforce avar.axis_count. It may read
        // trailing records when fvar has more axes. Plan exactly the reachable
        // prefix, including incomplete records; do not assume equal counts.
        let mut position = 8;
        let mut max_pairs = 0;
        for _ in 0..axes {
            self.charge_variation(VariationOperation::MappingPlan, 3, 0)?;
            let Ok(pairs) = data.read_at::<u16>(position) else {
                break;
            };
            max_pairs = max_pairs.max(usize::from(pairs));
            // pairs is u16; axes is u16; these offsets fit wasm32 usize.
            position += 2 + usize::from(pairs) * 4;
            if position > data.len() {
                break;
            }
        }
        // VarLenArray::get: at most axes-1 prefix u16 reads, then
        // SegmentMaps::read's final u16 count and one borrowed-array visit.
        // SegmentMaps::apply: six cap reads; at most three scans of n from
        // fields (exact, duplicate, interpolation); four final from/to reads.
        // Each from/to field is 2 bytes. Extra 2 visits cover array access.
        self.mapping_work = axes * 2 + 2 + 6 * 2 + max_pairs * 3 * 2 + 4 * 2;
        Ok(Some(Some(avar)))
    }

    fn normalization(&mut self, axes: usize, input_len: Option<usize>) -> Result<(), Self::Error> {
        // Only the public slice entry instantiates this policy. Its exact
        // length is supplied by Core-independent code, never size_hint().
        let inputs = input_len.ok_or(VariationOperationError::Arithmetic)?;
        // user_to_normalized: input plus two field visits; every axis tag
        // (4 bytes) is inspected. A matching axis reads 3 Fixed fields
        // (12 bytes), normalizes, maps and writes one coordinate.
        let per_axis = 5usize
            .checked_add(12 + 1)
            .and_then(|cost| cost.checked_add(self.mapping_work))
            .ok_or(VariationOperationError::Arithmetic)?;
        let per_input = axes
            .checked_mul(per_axis)
            .and_then(|cost| cost.checked_add(3))
            .ok_or(VariationOperationError::Arithmetic)?;
        let work = inputs
            .checked_mul(per_input)
            // A second fill of all coordinates; fvar.axes reads 4 u16 fields,
            // and visits its borrowed arrays. avar.version reads 4 bytes.
            .and_then(|cost| cost.checked_add(axes + 8 + 2 + 4))
            .ok_or(VariationOperationError::Arithmetic)?;
        self.charge_variation(VariationOperation::Normalization, work, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_never_runs_normalization_callback() {
        let mut calls = 0;
        let mut policy = AdmittedVariation {
            callback: |_, _| {
                calls += 1;
                Ok::<_, Infallible>(())
            },
            mapping_work: 0,
        };
        assert!(matches!(
            policy.normalization(64, Some(usize::MAX)),
            Err(VariationOperationError::Arithmetic)
        ));
        assert_eq!(calls, 0);
    }
}

impl ShaperInstance {
    /// Constructs a user-space instance with operation admission.
    ///
    /// Supports missing/malformed avar and avar 1.0. Other versions return
    /// `Ok(None)` before coordinate allocation or normalization. Successful
    /// candidates exactly preserve ordinary duplicate-tag, clamping, default
    /// detection and feature-variation behavior. Rejection returns the original
    /// error, drops the unpublished candidate and never invokes a fallback.
    /// This does not cover cold shaper data, shape plans or shaping.
    pub fn try_from_variations_admitted<E>(
        font: &FontRef,
        variations: &[Variation],
        admit: impl FnMut(VariationOperation, InstanceOperationCost) -> Result<(), E>,
    ) -> Result<Option<Self>, VariationOperationError<E>> {
        let mut candidate = Self::default();
        let mut policy = AdmittedVariation {
            callback: admit,
            mapping_work: 0,
        };
        if !candidate.set_variations_with(
            font,
            variations.iter().copied(),
            Some(variations.len()),
            &mut policy,
        )? {
            return Ok(None);
        }
        Ok(Some(candidate))
    }

    pub(crate) fn set_variations_with<V, P>(
        &mut self,
        font: &FontRef,
        variations: V,
        input_len: Option<usize>,
        policy: &mut P,
    ) -> Result<bool, P::Error>
    where
        V: IntoIterator,
        V::Item: Into<Variation>,
        P: VariationAdmission,
    {
        directory(font, policy)?;
        policy.charge(InstanceOperation::TableRead, 18, 0)?;
        let Ok(fvar) = font.fvar() else {
            return Ok(true);
        };
        let count = usize::from(fvar.axis_count());
        let Some(avar) = policy.mapping(font, count)? else {
            return Ok(false);
        };
        policy.reserve(self, count)?;
        // Initialization and future cleanup are prepaid. The normalization
        // helper's additional fill is charged separately.
        policy.charge(InstanceOperation::Coordinates, count * 2, 0)?;
        self.coords.resize(count, crate::NormalizedCoord::ZERO);
        policy.normalization(count, input_len)?;
        normalize(&fvar, avar.as_ref(), variations, self.coords.as_mut_slice());
        self.check_default_with(policy)?;
        self.set_feature_variations_with(font, policy)?;
        Ok(true)
    }
}

fn normalize<V>(
    fvar: &Fvar,
    avar: Option<&Avar>,
    variations: V,
    coords: &mut [crate::NormalizedCoord],
) where
    V: IntoIterator,
    V::Item: Into<Variation>,
{
    fvar.user_to_normalized(
        avar,
        variations
            .into_iter()
            .map(Into::into)
            .map(|var| (var.tag, Fixed::from_f64(var.value as _))),
        coords,
    );
}
