use read_fonts::types::{F2Dot14, GlyphId};
use read_fonts::{FontRef, TableProvider};
use smallvec::SmallVec;

use super::aat::AatTables;
use super::charmap::{cache_t as cmap_cache_t, Charmap};
use super::glyph_metrics::GlyphMetrics;
use super::glyph_names::GlyphNames;
use super::ot::{LayoutTable, OtCache, OtTables};
use super::ot_layout::TableIndex;
use super::ot_shape::{hb_ot_shape_context_t, shape_internal};
use crate::hb::aat::AatCache;
use crate::hb::buffer::hb_buffer_t;
use crate::hb::tables::TableOffsets;
use crate::{script, Feature, GlyphBuffer, NormalizedCoord, ShapePlan, UnicodeBuffer, Variation};

#[path = "instance_admission.rs"]
pub(crate) mod admission;
pub use admission::{
    InstanceOperation, InstanceOperationCost, InstanceOperationError, InstanceStorageError,
};

#[path = "variation_admission.rs"]
pub(crate) mod variation_admission;
pub use variation_admission::{VariationOperation, VariationOperationError};

#[path = "cold_admission.rs"]
pub(crate) mod cold_admission;
pub use cold_admission::{ColdLookupTable, ColdOperation, ColdOperationCost, ColdOperationError};
use cold_admission::ColdAdmission;

/// Data required for shaping with a single font.
pub struct ShaperData {
    table_offsets: TableOffsets,
    ot_cache: OtCache,
    aat_cache: AatCache,
    cmap_cache: cmap_cache_t,
}

impl ShaperData {
    /// Creates new cached shaper data for the given font.
    pub fn new(font: &FontRef) -> Self {
        let ot_cache = OtCache::new(font);
        let aat_cache = AatCache::new(font);
        let table_offsets = TableOffsets::new(font);
        let cmap_cache = cmap_cache_t::new();
        Self {
            table_offsets,
            ot_cache,
            aat_cache,
            cmap_cache,
        }
    }

    /// Constructs cold shaper caches with operation admission.
    ///
    /// This covers table-directory searches, cmap selection, native lazy
    /// lookup slots, portable eager lookups, mark-set digests and inline
    /// mapping caches. It does not cover native lazy lookup initialization
    /// (use `try_prepare_lookup_admitted`), shaper configuration, shape
    /// plans, normalized instances or shaping.
    ///
    /// Empty AAT tables are supported by scalar-header proof. Nonempty or
    /// truncated AAT cache headers return `Ok(None)` before parsing subtables,
    /// whose cache construction is not yet admitted.
    /// Earlier completed charges remain spent. Rejection returns its original
    /// source and drops the unpublished candidate without ordinary fallback.
    pub fn try_new_admitted<E>(
        font: &FontRef,
        admit: impl FnMut(ColdOperation, ColdOperationCost) -> Result<(), E>,
    ) -> Result<Option<Self>, ColdOperationError<E>> {
        let policy = &mut cold_admission::AdmittedCold(admit);
        let Some(ot_cache) = OtCache::new_with(font, policy)? else {
            return Ok(None);
        };
        let Some(aat_cache) = AatCache::new_with(font, policy)? else {
            return Ok(None);
        };
        let table_offsets = TableOffsets::new_with(font, policy)?;
        // cmap_cache_t is an inline hb_cache_t of 256 atomics.
        policy.charge(ColdOperation::Initialize, cmap_cache_t::INLINE_SLOTS, 0)?;
        let cmap_cache = cmap_cache_t::new();
        Ok(Some(Self {
            table_offsets,
            ot_cache,
            aat_cache,
            cmap_cache,
        }))
    }

    /// Prepares one OpenType lookup without an unadmitted lazy allocation.
    ///
    /// Use the same font that constructed this data, as for `shaper`.
    /// Returns true for a valid lookup, false for a missing or malformed
    /// lookup. Native commits only complete candidates; rejection leaves an
    /// uninitialized slot available for retry. Portable reads its complete
    /// eager cache. A prior complete cache remains available after rejection.
    /// Does not admit ShapePlan construction or shaping itself.
    pub fn try_prepare_lookup_admitted<E>(
        &self,
        font: &FontRef,
        table: ColdLookupTable,
        index: u16,
        admit: impl FnMut(ColdOperation, ColdOperationCost) -> Result<(), E>,
    ) -> Result<bool, ColdOperationError<E>> {
        let policy = &mut cold_admission::AdmittedCold(admit);
        cold_admission::directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        let info = match table {
            ColdLookupTable::Substitution => match font.gsub() {
                Ok(table) => self.ot_cache.gsub.get_with(&table, index, policy)?,
                Err(_) => None,
            },
            ColdLookupTable::Positioning => match font.gpos() {
                Ok(table) => self.ot_cache.gpos.get_with(&table, index, policy)?,
                Err(_) => None,
            },
        };
        Ok(info.is_some())
    }

    /// Returns a builder for constructing a new shaper with the given
    /// font.
    pub fn shaper<'a>(&'a self, font: &FontRef<'a>) -> ShaperBuilder<'a> {
        ShaperBuilder {
            data: self,
            font: font.clone(),
            instance: None,
            point_size: None,
        }
    }
}

// Maximum number of coordinates to store inline before spilling to the
// heap.
//
// Any value between 5 and 11 yields a SmallVec footprint of 32 bytes.
const MAX_INLINE_COORDS: usize = 11;

/// An instance of a variable font.
#[derive(Clone, Default, Debug)]
pub struct ShaperInstance {
    coords: SmallVec<[F2Dot14; MAX_INLINE_COORDS]>,
    pub(crate) feature_variations: [Option<u32>; 2],
    // TODO: this is a good place to hang variation specific caches
}

impl ShaperInstance {
    /// Creates a new shaper instance for the given font from the specified
    /// list of variation settings.
    ///
    /// The setting values are in user space and the order is insignificant.
    pub fn from_variations<V>(font: &FontRef, variations: V) -> Self
    where
        V: IntoIterator,
        V::Item: Into<Variation>,
    {
        let mut this = Self::default();
        this.set_variations(font, variations);
        this
    }

    /// Creates a new shaper instance for the given font from the specified
    /// set of normalized coordinates.
    ///
    /// The sequence of coordinates is expected to be in axis order.
    pub fn from_coords(font: &FontRef, coords: impl IntoIterator<Item = NormalizedCoord>) -> Self {
        let mut this = Self::default();
        this.set_coords(font, coords);
        this
    }

    /// Creates a new shaper instance for the given font using the variation
    /// position from the named instance at the specified index.
    pub fn from_named_instance(font: &FontRef, index: usize) -> Self {
        let mut this = Self::default();
        this.set_named_instance(font, index);
        this
    }

    /// Returns the underlying set of normalized coordinates.
    pub fn coords(&self) -> &[F2Dot14] {
        &self.coords
    }

    /// Resets the instance for the given font and variation settings.
    pub fn set_variations<V>(&mut self, font: &FontRef, variations: V)
    where
        V: IntoIterator,
        V::Item: Into<Variation>,
    {
        self.coords.clear();
        match self.set_variations_with(font, variations, None, &mut admission::OrdinaryInstance) {
            Ok(_) => {}
            Err(never) => match never {},
        }
    }

    /// Resets the instance for the given font and normalized coordinates.
    pub fn set_coords(&mut self, font: &FontRef, coords: impl IntoIterator<Item = F2Dot14>) {
        self.coords.clear();
        match self.set_coords_with(font, coords, &mut admission::OrdinaryInstance) {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    /// Resets the instance for the given font using the variation
    /// position from the named instance at the specified index.
    pub fn set_named_instance(&mut self, font: &FontRef, index: usize) {
        self.coords.clear();
        if let Ok(fvar) = font.fvar() {
            if let Ok((axes, instance)) = fvar
                .axis_instance_arrays()
                .and_then(|arrays| Ok((arrays.axes(), arrays.instances().get(index)?)))
            {
                self.set_variations(
                    font,
                    axes.iter()
                        .zip(instance.coordinates)
                        .map(|(axis, coord)| (axis.axis_tag(), coord.get().to_f32())),
                );
            }
        }
    }

}

/// Builder type for constructing a [`Shaper`](crate::Shaper).
pub struct ShaperBuilder<'a> {
    data: &'a ShaperData,
    font: FontRef<'a>,
    instance: Option<&'a ShaperInstance>,
    point_size: Option<f32>,
}

impl<'a> ShaperBuilder<'a> {
    /// Sets an optional instance for the shaper.
    ///
    /// This defines the variable font configuration.
    pub fn instance(mut self, instance: Option<&'a ShaperInstance>) -> Self {
        self.instance = instance;
        self
    }

    /// Sets the point size for the shaper.
    ///
    /// This controls adjustments provided by the tracking table.
    pub fn point_size(mut self, size: Option<f32>) -> Self {
        self.point_size = size;
        self
    }

    /// Builds the shaper with the current configuration.
    pub fn build(self) -> crate::Shaper<'a> {
        match self.build_with(&mut cold_admission::OrdinaryCold) {
            Ok(Some(value)) => value,
            Ok(None) => unreachable!("ordinary configuration supports every table"),
            Err(error) => match error {},
        }
    }

    /// Configures borrowed font tables with operation admission.
    ///
    /// Covers mapping/metric headers, coordinate scans and OT/AAT table
    /// configuration. Nonempty or truncated morx/kern/kerx headers decline
    /// before their variable-size readers run. Previously completed data and
    /// instances remain usable after rejection; no ordinary fallback runs.
    /// Does not admit data construction, plans, glyph queries or shaping.
    pub fn try_build_admitted<E>(
        self,
        admit: impl FnMut(ColdOperation, ColdOperationCost) -> Result<(), E>,
    ) -> Result<Option<crate::Shaper<'a>>, ColdOperationError<E>> {
        self.build_with(&mut cold_admission::AdmittedCold(admit))
    }

    fn build_with<P: ColdAdmission>(
        self, policy: &mut P,
    ) -> Result<Option<crate::Shaper<'a>>, P::Error> {
        policy.charge(ColdOperation::Planning, 4, 0)?;
        let font = self.font;
        let Some(aat_tables) = AatTables::new_with(
            &font, &self.data.aat_cache, &self.data.table_offsets, policy,
        )? else {
            return Ok(None);
        };
        let units_per_em = self.data.table_offsets.units_per_em;
        let charmap = Charmap::new_with(&font, &self.data.table_offsets, &self.data.cmap_cache, policy)?;
        let glyph_metrics = GlyphMetrics::new_with(&font, &self.data.table_offsets, policy)?;
        let (coords, feature_variations) = self
            .instance
            .map(|instance| (instance.coords(), instance.feature_variations))
            .unwrap_or_default();
        let ot_tables = OtTables::new_with(
            &font,
            &self.data.ot_cache,
            &self.data.table_offsets,
            coords,
            feature_variations,
            policy,
        )?;
        Ok(Some(hb_font_t {
            font,
            units_per_em,
            points_per_em: self.point_size,
            charmap,
            glyph_metrics,
            ot_tables,
            aat_tables,
        }))
    }
}

/// A configured shaper.
#[derive(Clone)]
pub struct hb_font_t<'a> {
    pub(crate) font: FontRef<'a>,
    pub(crate) units_per_em: u16,
    pub(crate) points_per_em: Option<f32>,
    charmap: Charmap<'a>,
    glyph_metrics: GlyphMetrics<'a>,
    pub(crate) ot_tables: OtTables<'a>,
    pub(crate) aat_tables: AatTables<'a>,
}

impl<'a> crate::Shaper<'a> {
    /// Returns font's units per EM.
    #[inline]
    pub fn units_per_em(&self) -> i32 {
        self.units_per_em as i32
    }

    /// Returns the currently active normalized coordinates.
    pub fn coords(&self) -> &'a [NormalizedCoord] {
        self.ot_tables.coords
    }

    /// Shapes the buffer content using provided font and features.
    ///
    /// Consumes the buffer. You can then run [`GlyphBuffer::clear`] to get the [`UnicodeBuffer`] back
    /// without allocating a new one.
    ///
    /// If you plan to shape multiple strings, prefer [`shape_with_plan`](Self::shape_with_plan).
    /// This is because [`ShapePlan`](crate::ShapePlan) initialization is pretty slow and should preferably
    /// be called once for each shaping configuration.
    pub fn shape(&self, buffer: UnicodeBuffer, features: &[Feature]) -> GlyphBuffer {
        let plan = ShapePlan::new(
            self,
            buffer.0.direction,
            buffer.0.script,
            buffer.0.language.as_ref(),
            features,
        );
        self.shape_with_plan(&plan, buffer, features)
    }

    /// Shapes the buffer content using the provided font and plan.
    ///
    /// Consumes the buffer. You can then run [`GlyphBuffer::clear`] to get the [`UnicodeBuffer`] back
    /// without allocating a new one.
    ///
    /// It is up to the caller to ensure that the shape plan matches the properties of the provided
    /// buffer, otherwise the shaping result will likely be incorrect.
    ///
    /// # Panics
    ///
    /// Will panic when debugging assertions are enabled if the buffer and plan have mismatched
    /// properties.
    pub fn shape_with_plan(
        &self,
        plan: &ShapePlan,
        buffer: UnicodeBuffer,
        features: &[Feature],
    ) -> GlyphBuffer {
        let mut buffer = buffer.0;
        buffer.enter();

        assert_eq!(
            buffer.direction, plan.direction,
            "Buffer direction does not match plan direction: {:?} != {:?}",
            buffer.direction, plan.direction
        );
        assert_eq!(
            buffer.script.unwrap_or(script::UNKNOWN),
            plan.script.unwrap_or(script::UNKNOWN),
            "Buffer script does not match plan script: {:?} != {:?}",
            buffer.script.unwrap_or(script::UNKNOWN),
            plan.script.unwrap_or(script::UNKNOWN)
        );

        if buffer.len > 0 {
            // Save the original direction, we use it later.
            let target_direction = buffer.direction;
            shape_internal(&mut hb_ot_shape_context_t {
                plan,
                face: self,
                buffer: &mut buffer,
                target_direction,
                features,
            });
        }

        buffer.leave();

        GlyphBuffer(buffer)
    }

    pub(crate) fn has_glyph(&self, c: u32) -> bool {
        self.get_nominal_glyph(c).is_some()
    }

    pub(crate) fn get_nominal_glyph(&self, c: u32) -> Option<GlyphId> {
        self.charmap.map(c)
    }

    pub(crate) fn get_nominal_variant_glyph(&self, c: u32, vs: u32) -> Option<GlyphId> {
        self.charmap.map_variant(c, vs)
    }

    pub(crate) fn glyph_h_advance(&self, glyph: GlyphId) -> i32 {
        self.glyph_metrics
            .advance_width(glyph, self.ot_tables.coords)
            .unwrap_or_default()
    }
    pub(crate) fn glyph_h_advances(&self, buffer: &mut hb_buffer_t) {
        self.glyph_metrics
            .populate_advance_widths(buffer, self.ot_tables.coords);
    }

    pub(crate) fn glyph_v_advance(&self, glyph: GlyphId) -> i32 {
        -self
            .glyph_metrics
            .advance_height(glyph, self.ot_tables.coords)
            .unwrap_or(self.units_per_em as i32)
    }

    pub(crate) fn glyph_h_origin(&self, glyph: GlyphId) -> i32 {
        self.glyph_h_advance(glyph) / 2
    }

    pub(crate) fn glyph_v_origin(&self, glyph: GlyphId) -> i32 {
        self.glyph_metrics
            .v_origin(glyph, self.ot_tables.coords)
            .unwrap_or_default()
    }

    pub(crate) fn glyph_extents(
        &self,
        glyph: GlyphId,
        glyph_extents: &mut hb_glyph_extents_t,
    ) -> bool {
        if let Some(extents) = self.glyph_metrics.extents(glyph, self.ot_tables.coords) {
            glyph_extents.x_bearing = extents.x_min;
            glyph_extents.y_bearing = extents.y_max;
            glyph_extents.width = extents.x_max - extents.x_min;
            glyph_extents.height = extents.y_min - extents.y_max;
            true
        } else {
            false
        }
    }

    pub(crate) fn glyph_names(&self) -> GlyphNames<'a> {
        GlyphNames::new(&self.font)
    }

    pub(crate) fn layout_table(&self, table_index: TableIndex) -> Option<LayoutTable<'a>> {
        match table_index {
            TableIndex::GSUB => self
                .ot_tables
                .gsub
                .as_ref()
                .map(|table| LayoutTable::Gsub(table.table.clone())),
            TableIndex::GPOS => self
                .ot_tables
                .gpos
                .as_ref()
                .map(|table| LayoutTable::Gpos(table.table.clone())),
        }
    }

    pub(crate) fn layout_tables(&self) -> impl Iterator<Item = (TableIndex, LayoutTable<'a>)> + '_ {
        TableIndex::iter().filter_map(move |idx| self.layout_table(idx).map(|table| (idx, table)))
    }
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct hb_glyph_extents_t {
    pub x_bearing: i32,
    pub y_bearing: i32,
    pub width: i32,
    pub height: i32,
}
