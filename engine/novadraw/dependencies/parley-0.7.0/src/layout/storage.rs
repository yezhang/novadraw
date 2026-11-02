// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Read-only storage observations, not allocation admission or operation bounds.

use super::Layout;
use crate::{style::Brush, FontData};
use alloc::vec::Vec;
use core::mem::size_of;

/// A directly owned layout array.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStorageField {
    /// Font handles, excluding their shared backing allocations.
    Fonts,
    /// Normalized variation coordinates.
    Coordinates,
    /// Styles, excluding any allocations owned or referenced by brushes.
    Styles,
    /// Inline boxes.
    InlineBoxes,
    /// Shaped runs.
    Runs,
    /// Logical layout items.
    Items,
    /// Shaped clusters, including glyphs stored inline in cluster data.
    Clusters,
    /// Out-of-line glyphs.
    Glyphs,
    /// Lines produced by line breaking.
    Lines,
    /// Visual items produced by line breaking.
    LineItems,
}

/// Length, capacity and element size of one directly owned array.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutArrayStorage {
    /// The array described by this observation.
    pub field: LayoutStorageField,
    /// Number of initialized elements.
    pub len: usize,
    /// Capacity reported by the underlying `Vec`, including unused slots.
    pub capacity: usize,
    /// Size of one element, including inline fields and padding.
    pub element_bytes: usize,
}

impl LayoutArrayStorage {
    fn of<T>(field: LayoutStorageField, values: &Vec<T>) -> Self {
        Self {
            field,
            len: values.len(),
            capacity: values.capacity(),
            element_bytes: size_of::<T>(),
        }
    }

    /// Logical storage occupied by the vector allocation.
    ///
    /// Returns `None` on arithmetic overflow. Allocator bookkeeping and physical
    /// allocation granularity are outside this observation.
    pub fn capacity_bytes(self) -> Option<usize> {
        self.capacity.checked_mul(self.element_bytes)
    }
}

/// Allocation-free observation of the ten arrays directly owned by a layout.
///
/// This is a shallow snapshot. Font blob allocations and their reference-count
/// control blocks, brush-owned or brush-referenced allocations, and allocations
/// in external contexts are excluded. Callers must resolve those separately.
/// A snapshot can become stale when the layout changes. It describes neither
/// a future clone nor a future line-breaking operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutOwnedStorage {
    /// Inline size of `Layout<B>`, including its vector headers.
    pub object_bytes: usize,
    /// All directly owned arrays, in `LayoutStorageField` declaration order.
    pub arrays: [LayoutArrayStorage; 10],
}

impl LayoutOwnedStorage {
    /// Inline object and directly owned vector allocations.
    ///
    /// Do not add vector headers separately: they are already in `object_bytes`.
    /// Shared font and brush backing must be added by the caller.
    pub fn owned_bytes(self) -> Option<usize> {
        self.arrays
            .iter()
            .try_fold(self.object_bytes, |total, array| {
                total.checked_add(array.capacity_bytes()?)
            })
    }

    /// Retrieves an array observation without allocating.
    pub fn array(&self, field: LayoutStorageField) -> Option<&LayoutArrayStorage> {
        self.arrays.iter().find(|array| array.field == field)
    }
}

impl<B: Brush> Layout<B> {
    /// Observes actual capacities without scanning, cloning or mutating content.
    ///
    /// This is not an operation cost plan or a complete retained-memory
    /// certificate. See [`LayoutOwnedStorage`] for exclusions.
    pub fn owned_storage(&self) -> LayoutOwnedStorage {
        let data = &self.data;
        use LayoutStorageField as F;
        LayoutOwnedStorage {
            object_bytes: size_of::<Self>(),
            arrays: [
                LayoutArrayStorage::of(F::Fonts, &data.fonts),
                LayoutArrayStorage::of(F::Coordinates, &data.coords),
                LayoutArrayStorage::of(F::Styles, &data.styles),
                LayoutArrayStorage::of(F::InlineBoxes, &data.inline_boxes),
                LayoutArrayStorage::of(F::Runs, &data.runs),
                LayoutArrayStorage::of(F::Items, &data.items),
                LayoutArrayStorage::of(F::Clusters, &data.clusters),
                LayoutArrayStorage::of(F::Glyphs, &data.glyphs),
                LayoutArrayStorage::of(F::Lines, &data.lines),
                LayoutArrayStorage::of(F::LineItems, &data.line_items),
            ],
        }
    }

    /// Borrows the font handles retained by this layout without cloning them.
    ///
    /// Blob IDs identify shared backing. Blob data length does not certify its
    /// allocation capacity; the owner must supply that information. Repeated
    /// blob IDs can occur with different face indices in a font collection.
    pub fn retained_fonts(&self) -> impl ExactSizeIterator<Item = &FontData> + '_ {
        self.data.fonts.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_arrays_report_capacity_after_logical_clear() {
        let mut layout = Layout::<usize>::new();
        let data = &mut layout.data;
        data.fonts.reserve_exact(2);
        data.coords.reserve_exact(3);
        data.styles.reserve_exact(4);
        data.inline_boxes.reserve_exact(5);
        data.runs.reserve_exact(6);
        data.items.reserve_exact(7);
        data.clusters.reserve_exact(8);
        data.glyphs.reserve_exact(9);
        data.lines.reserve_exact(10);
        data.line_items.reserve_exact(11);
        data.coords.push(0);
        let before = layout.owned_storage();
        layout.data.clear();
        let after = layout.owned_storage();
        for (a, b) in before.arrays.iter().zip(after.arrays) {
            assert!(a.capacity > 0, "each reserved array must be represented");
            assert_eq!(a.capacity, b.capacity);
            assert_eq!(a.element_bytes, b.element_bytes);
            assert_eq!(b.len, 0);
        }
        assert_eq!(before.owned_bytes(), after.owned_bytes());
    }

    #[test]
    fn snapshot_uses_each_private_element_size_and_capacity() {
        let layout = Layout::<usize>::new();
        let data = &layout.data;
        let expected = [
            LayoutArrayStorage::of(LayoutStorageField::Fonts, &data.fonts),
            LayoutArrayStorage::of(LayoutStorageField::Coordinates, &data.coords),
            LayoutArrayStorage::of(LayoutStorageField::Styles, &data.styles),
            LayoutArrayStorage::of(LayoutStorageField::InlineBoxes, &data.inline_boxes),
            LayoutArrayStorage::of(LayoutStorageField::Runs, &data.runs),
            LayoutArrayStorage::of(LayoutStorageField::Items, &data.items),
            LayoutArrayStorage::of(LayoutStorageField::Clusters, &data.clusters),
            LayoutArrayStorage::of(LayoutStorageField::Glyphs, &data.glyphs),
            LayoutArrayStorage::of(LayoutStorageField::Lines, &data.lines),
            LayoutArrayStorage::of(LayoutStorageField::LineItems, &data.line_items),
        ];
        let snapshot = layout.owned_storage();
        assert_eq!(snapshot.arrays, expected);
        assert_eq!(snapshot.object_bytes, size_of::<Layout<usize>>());
        assert_eq!(snapshot.owned_bytes(), Some(snapshot.object_bytes));
    }

    #[test]
    fn zero_sized_capacity_does_not_overflow() {
        let zero = LayoutArrayStorage::of(LayoutStorageField::Styles, &Vec::<()>::new());
        assert_eq!(zero.capacity_bytes(), Some(0));
    }

    #[test]
    fn arithmetic_overflow_is_explicit() {
        let overflowing = LayoutArrayStorage {
            field: LayoutStorageField::Glyphs,
            len: 0,
            capacity: usize::MAX,
            element_bytes: 2,
        };
        assert_eq!(overflowing.capacity_bytes(), None);
        let snapshot = LayoutOwnedStorage {
            object_bytes: 1,
            arrays: [LayoutArrayStorage {
                element_bytes: 1,
                ..overflowing
            }; 10],
        };
        assert_eq!(snapshot.owned_bytes(), None);
    }

    #[test]
    fn caller_constructed_snapshot_with_missing_field_is_safe() {
        let empty = LayoutArrayStorage {
            field: LayoutStorageField::Fonts,
            len: 0,
            capacity: 0,
            element_bytes: 0,
        };
        let snapshot = LayoutOwnedStorage {
            object_bytes: 0,
            arrays: [empty; 10],
        };
        assert_eq!(snapshot.array(LayoutStorageField::Glyphs), None);
    }
}
