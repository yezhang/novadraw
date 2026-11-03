use super::buffer::GlyphPropsFlags;
use super::ot_layout::TableIndex;
use super::set_digest::hb_set_digest_t;
use crate::hb::ot_layout_gsubgpos::MappingCache;
use crate::hb::tables::TableOffsets;
use alloc::vec::Vec;
use lookup::{LookupCache, LookupInfo};
use super::face::cold_admission::{directory, ColdAdmission, ColdOperation, OrdinaryCold};
use read_fonts::tables::layout::{ClassRangeRecord, RangeRecord};
use read_fonts::types::GlyphId16;
use read_fonts::{
    tables::{
        gdef::Gdef,
        gpos::{AnchorTable, DeviceOrVariationIndex, Gpos},
        gsub::{ClassDef, FeatureList, FeatureVariations, Gsub, ScriptList},
        layout::{Feature, LangSys, Script},
        varc::CoverageTable,
        variations::{DeltaSetIndex, ItemVariationStore},
    },
    types::{BigEndian, F2Dot14, GlyphId, Offset32},
    FontData, FontRef, ReadError, ResolveOffset, TableProvider,
};

pub mod contextual;
pub mod gpos;
pub mod gsub;
pub mod lookup;
mod query_admission;

pub struct OtCache {
    pub gsub: LookupCache,
    pub gpos: LookupCache,
    pub gdef_glyph_props_cache: MappingCache,
    pub gdef_mark_set_digests: Vec<hb_set_digest_t>,
}

impl OtCache {
    pub fn new(font: &FontRef) -> Self {
        match Self::new_with(font, &mut OrdinaryCold) {
            Ok(Some(value)) => value,
            Ok(None) => unreachable!("ordinary cold policy supports every OT cache"),
            Err(never) => match never {},
        }
    }

    pub(crate) fn new_with<P: ColdAdmission>(
        font: &FontRef,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        directory(font, policy)?;
        // GSUB/GPOS readers read a four-byte version, then advance fields.
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        let gsub = if let Ok(table) = font.gsub() {
            let Some(cache) = LookupCache::new_with(&table, policy)? else {
                return Ok(None);
            };
            cache
        } else {
            LookupCache::default()
        };
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        let gpos = if let Ok(table) = font.gpos() {
            let Some(cache) = LookupCache::new_with(&table, policy)? else {
                return Ok(None);
            };
            cache
        } else {
            LookupCache::default()
        };
        let mut gdef_mark_set_digests = Vec::new();
        directory(font, policy)?;
        // GDEF reader reads Version16Dot16 and advances optional fields.
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        if let Ok(gdef) = font.gdef() {
            // Nullable Offset16 tests and resolution, MarkGlyphSets reader's
            // u16 count. coverages() only wraps the borrowed offset array.
            policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
            if let Some(Ok(mark_sets)) = gdef.mark_glyph_sets_def() {
                let coverages = mark_sets.coverages();
                policy.charge(ColdOperation::Planning, 1, 0)?;
                policy.reserve(&mut gdef_mark_set_digests, coverages.len())?;
                for index in 0..coverages.len() {
                    // Offset32, Coverage format u16, format reader count u16.
                    policy.charge(ColdOperation::MarkSet, 4 + 2 + 2, 0)?;
                    let mut digest = hb_set_digest_t::new();
                    if let Ok(coverage) = coverages.get(index) {
                        digest.add_coverage_with(&coverage, policy)?;
                    }
                    // One write and one eventual candidate/cache drop visit.
                    policy.charge(ColdOperation::Initialize, 2, 0)?;
                    gdef_mark_set_digests.push(digest);
                }
            }
        }
        // MappingCache has 128 inline atomics, constructed with from_fn.
        // No heap allocation and no glyph mapping is queried here.
        policy.charge(ColdOperation::Initialize, MappingCache::INLINE_SLOTS, 0)?;
        Ok(Some(Self {
            gsub,
            gpos,
            gdef_glyph_props_cache: MappingCache::new(),
            gdef_mark_set_digests,
        }))
    }
}

#[derive(Clone)]
pub struct GsubTable<'a> {
    pub table: Gsub<'a>,
    pub lookups: &'a LookupCache,
}

impl crate::hb::ot_layout::LayoutTable for GsubTable<'_> {
    const INDEX: TableIndex = TableIndex::GSUB;
    const IN_PLACE: bool = false;

    fn get_lookup(&self, index: u16) -> Option<&LookupInfo> {
        self.lookups.get(&self.table, index)
    }
}

#[derive(Clone)]
pub struct GposTable<'a> {
    pub table: Gpos<'a>,
    pub lookups: &'a LookupCache,
}

impl crate::hb::ot_layout::LayoutTable for GposTable<'_> {
    const INDEX: TableIndex = TableIndex::GPOS;
    const IN_PLACE: bool = true;

    fn get_lookup(&self, index: u16) -> Option<&LookupInfo> {
        self.lookups.get(&self.table, index)
    }
}

#[derive(Clone, Default)]
pub struct GdefTable<'a> {
    table: Option<Gdef<'a>>,
    classes: Option<ClassDef<'a>>,
    mark_classes: Option<ClassDef<'a>>,
    mark_sets: Option<(FontData<'a>, &'a [BigEndian<Offset32>])>,
}

impl<'a> GdefTable<'a> {
    fn new(font: &FontRef<'a>, table_offsets: &TableOffsets) -> Self {
        if let Some(gdef) = table_offsets.gdef.resolve_table::<Gdef>(font) {
            let classes = gdef.glyph_class_def().transpose().ok().flatten();
            let mark_classes = gdef.mark_attach_class_def().transpose().ok().flatten();
            let mark_sets = gdef
                .mark_glyph_sets_def()
                .transpose()
                .ok()
                .flatten()
                .map(|sets| (sets.offset_data(), sets.coverage_offsets()));
            Self {
                table: Some(gdef),
                classes,
                mark_classes,
                mark_sets,
            }
        } else {
            Self::default()
        }
    }
}

#[derive(Clone)]
pub struct OtTables<'a> {
    pub gsub: Option<GsubTable<'a>>,
    pub gpos: Option<GposTable<'a>>,
    pub gdef: GdefTable<'a>,
    pub gdef_glyph_props_cache: &'a MappingCache,
    pub gdef_mark_set_digests: &'a [hb_set_digest_t],
    pub coords: &'a [F2Dot14],
    pub var_store: Option<ItemVariationStore<'a>>,
    pub feature_variations: [Option<u32>; 2],
}

impl<'a> OtTables<'a> {
    pub(crate) fn new_with<P: ColdAdmission>(
        font: &FontRef<'a>, cache: &'a OtCache, table_offsets: &TableOffsets,
        coords: &'a [F2Dot14], feature_variations: [Option<u32>; 2],
        policy: &mut P,
    ) -> Result<Self, P::Error> {
        policy.charge(ColdOperation::Planning, 1, 0)?;
        policy.charge(ColdOperation::TableRead, coords.len(), 0)?;
        // GSUB/GPOS fixed headers; GDEF fixed header, two nullable
        // class offsets (test+resolve), ClassDef enum/format/count reads,
        // mark-set offset (test+resolve) and count. All payloads borrow.
        const LAYOUT_HEADERS: usize = 14 * 2;
        const GDEF_AND_CHILD_HEADERS: usize = 18 + 2 * (2 + 2 + 2 + 6) + 2 + 2 + 4;
        // Optional ItemVariationStore: Offset32 test+resolve, its
        // fixed 8-byte header and a borrowed offset array; no delta read.
        const VAR_STORE_HEADER: usize = 4 + 4 + 8;
        policy.charge(ColdOperation::TableRead,
            LAYOUT_HEADERS + GDEF_AND_CHILD_HEADERS + VAR_STORE_HEADER, 0)?;
        Ok(Self::new(font, cache, table_offsets, coords, feature_variations))
    }

    pub fn new(
        font: &FontRef<'a>,
        cache: &'a OtCache,
        table_offsets: &TableOffsets,
        coords: &'a [F2Dot14],
        feature_variations: [Option<u32>; 2],
    ) -> Self {
        let gsub = table_offsets
            .gsub
            .resolve_table(font)
            .map(|table| GsubTable {
                table,
                lookups: &cache.gsub,
            });
        let gpos = table_offsets
            .gpos
            .resolve_table(font)
            .map(|table| GposTable {
                table,
                lookups: &cache.gpos,
            });
        let coords = if coords.iter().any(|coord| *coord != F2Dot14::ZERO) {
            coords
        } else {
            &[]
        };
        let gdef = GdefTable::new(font, table_offsets);
        let var_store = if !coords.is_empty() {
            gdef.table
                .as_ref()
                .and_then(|gdef| gdef.item_var_store().transpose().ok().flatten())
        } else {
            None
        };
        Self {
            gsub,
            gpos,
            gdef,
            gdef_glyph_props_cache: &cache.gdef_glyph_props_cache,
            gdef_mark_set_digests: &cache.gdef_mark_set_digests,
            var_store,
            coords,
            feature_variations,
        }
    }

    pub fn has_glyph_classes(&self) -> bool {
        self.gdef.classes.is_some()
    }

    /// Bound a cache miss without querying or warming a glyph property.
    /// Readers borrow existing ClassDef arrays and allocate no storage.
    /// The caller must admit the borrowed headers before calling this helper.
    pub(crate) fn glyph_properties_work(&self) -> usize {
        fn class_work(class: &ClassDef) -> usize {
            match class {
                ClassDef::Format1(_) => 4, // start, array header and indexed value
                ClassDef::Format2(table) => {
                    let count = table.class_range_records().len();
                    // Rust 1.96 binary_search_by: at most bit_length(n)+1
                    // start reads, then selected start/end/class. Empty arrays
                    // perform no comparisons. No ordering assumption is needed.
                    let comparisons = if count == 0 {
                        0
                    } else {
                        (usize::BITS - count.leading_zeros()) as usize + 1
                    };
                    comparisons + 4 // array header plus selected fields
                }
            }
        }
        // Cache get/set and scalar optional-class/branch accesses. A hit is
        // also charged; no preflight query changes this miss upper bound.
        let mut work = 2 + 6;
        if let Some(classes) = self.gdef.classes.as_ref() {
            work += class_work(classes);
            // A class-3 miss may additionally query mark attachment classes.
            // Absent glyph classes cannot enter that branch.
            if let Some(mark_classes) = self.gdef.mark_classes.as_ref() {
                work += class_work(mark_classes);
            }
        }
        work
    }

    pub fn glyph_class(&self, glyph_id: u32) -> u16 {
        self.gdef
            .classes
            .as_ref()
            .map_or(0, |class_def| class_def.get(glyph_id))
    }

    pub fn glyph_mark_attachment_class(&self, glyph_id: u32) -> u16 {
        self.gdef
            .mark_classes
            .as_ref()
            .map_or(0, |class_def| class_def.get(glyph_id))
    }

    pub(crate) fn glyph_props(&self, glyph: GlyphId) -> u16 {
        let glyph = glyph.to_u32();

        if let Some(props) = self.gdef_glyph_props_cache.get(glyph) {
            return props as u16;
        }

        let props = match self.glyph_class(glyph) {
            1 => GlyphPropsFlags::BASE_GLYPH.bits(),
            2 => GlyphPropsFlags::LIGATURE.bits(),
            3 => {
                let class = self.glyph_mark_attachment_class(glyph);
                (class << 8) | GlyphPropsFlags::MARK.bits()
            }
            _ => 0,
        };

        self.gdef_glyph_props_cache.set(glyph, props as u32);

        props
    }

    pub fn is_mark_glyph(&self, glyph_id: u32, set_index: u16) -> bool {
        if self
            .gdef_mark_set_digests
            .get(set_index as usize)
            .is_some_and(|digest| digest.may_have(glyph_id))
        {
            self.gdef
                .mark_sets
                .as_ref()
                .and_then(|(data, offsets)| Some((data, offsets.get(set_index as usize)?.get())))
                .and_then(|(data, offset)| offset.resolve::<CoverageTable>(*data).ok())
                .is_some_and(|coverage| coverage.get(glyph_id).is_some())
        } else {
            false
        }
    }

    pub fn table_data(&self, table_index: TableIndex) -> Option<&'a [u8]> {
        if table_index == TableIndex::GSUB {
            self.gsub.as_ref().map(|t| t.table.offset_data().as_bytes())
        } else {
            self.gpos.as_ref().map(|t| t.table.offset_data().as_bytes())
        }
    }

    pub fn table_data_and_lookup(
        &self,
        table_index: TableIndex,
        lookup_index: u16,
    ) -> Option<(&'a [u8], &'a LookupInfo)> {
        if table_index == TableIndex::GSUB {
            let table = self.gsub.as_ref()?;
            Some((
                table.table.offset_data().as_bytes(),
                table.lookups.get(&table.table, lookup_index)?,
            ))
        } else {
            let table = self.gpos.as_ref()?;
            Some((
                table.table.offset_data().as_bytes(),
                table.lookups.get(&table.table, lookup_index)?,
            ))
        }
    }

    pub(super) fn resolve_anchor(&self, anchor: &AnchorTable) -> (i32, i32) {
        let mut x = anchor.x_coordinate() as i32;
        let mut y = anchor.y_coordinate() as i32;
        if let Some(vs) = self.var_store.as_ref() {
            let delta = |val: Option<Result<DeviceOrVariationIndex<'_>, ReadError>>| match val {
                Some(Ok(DeviceOrVariationIndex::VariationIndex(varix))) => vs
                    .compute_delta(
                        DeltaSetIndex {
                            outer: varix.delta_set_outer_index(),
                            inner: varix.delta_set_inner_index(),
                        },
                        self.coords,
                    )
                    .unwrap_or_default(),
                _ => 0,
            };
            x += delta(anchor.x_device());
            y += delta(anchor.y_device());
        }
        (x, y)
    }
}

pub enum LayoutTable<'a> {
    Gsub(Gsub<'a>),
    Gpos(Gpos<'a>),
}

impl<'a> LayoutTable<'a> {
    fn script_list(&self) -> Option<ScriptList<'a>> {
        match self {
            Self::Gsub(gsub) => gsub.script_list().ok(),
            Self::Gpos(gpos) => gpos.script_list().ok(),
        }
    }

    fn feature_list(&self) -> Option<FeatureList<'a>> {
        match self {
            Self::Gsub(gsub) => gsub.feature_list().ok(),
            Self::Gpos(gpos) => gpos.feature_list().ok(),
        }
    }

    pub(super) fn feature_variations(&self) -> Option<FeatureVariations<'a>> {
        match self {
            Self::Gsub(gsub) => gsub.feature_variations(),
            Self::Gpos(gpos) => gpos.feature_variations(),
        }
        .transpose()
        .ok()
        .flatten()
    }

    pub(crate) fn lookup_count(&self) -> u16 {
        match self {
            Self::Gsub(gsub) => gsub
                .lookup_list()
                .map(|list| list.lookup_count())
                .unwrap_or_default(),
            Self::Gpos(gpos) => gpos
                .lookup_list()
                .map(|list| list.lookup_count())
                .unwrap_or_default(),
        }
    }

}

fn coverage_index(coverage: Result<CoverageTable, ReadError>, gid: GlyphId) -> Option<u16> {
    coverage.ok().and_then(|coverage| coverage.get(gid))
}

fn coverage_index_cached(
    coverage: impl Fn(GlyphId) -> Option<u16>,
    gid: GlyphId,
    cache: &MappingCache,
) -> Option<u16> {
    if let Some(index) = cache.get(gid.into()) {
        if index == MappingCache::MAX_VALUE {
            None
        } else {
            Some(index as u16)
        }
    } else {
        let index = coverage(gid);
        if let Some(index) = index {
            if (index as u32) < MappingCache::MAX_VALUE {
                cache.set_unchecked(gid.into(), index as u32);
            }
            Some(index)
        } else {
            cache.set_unchecked(gid.into(), MappingCache::MAX_VALUE);
            None
        }
    }
}

fn covered(coverage: Result<CoverageTable, ReadError>, gid: GlyphId) -> bool {
    coverage_index(coverage, gid).is_some()
}

fn glyph_class(class_def: Result<ClassDef, ReadError>, gid: GlyphId) -> u16 {
    class_def
        .map(|class_def| class_def.get(gid))
        .unwrap_or_default()
}

fn glyph_class_cached(
    class_def: impl Fn(GlyphId) -> u16,
    gid: GlyphId,
    cache: &MappingCache,
) -> u16 {
    if let Some(index) = cache.get(gid.into()) {
        index as u16
    } else {
        let index = class_def(gid);
        cache.set(gid.into(), index as u32);
        index
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct CoverageInfo {
    pub offset: u16,
    pub format: u16,
    pub count: u16,
}

impl CoverageInfo {
    pub(crate) fn new_with<P: ColdAdmission>(
        parent_data: &FontData,
        offset: u16,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        // At most format + count; the payload is borrowed, not scanned.
        policy.charge(ColdOperation::TableRead, 2 + 2, 0)?;
        Ok(Self::new(parent_data, offset))
    }
    pub fn new(parent_data: &FontData, offset: u16) -> Option<Self> {
        if offset == 0 {
            return None;
        }
        let format = parent_data.read_at::<u16>(offset as usize).ok()?;
        if format != 1 && format != 2 {
            return None;
        }
        let count = parent_data.read_at::<u16>(offset as usize + 2).ok()?;
        Some(Self {
            offset,
            format,
            count,
        })
    }

    pub fn index(&self, parent_data: &FontData, gid: GlyphId) -> Option<u16> {
        if self.offset == 0 {
            return None;
        }
        let gid = gid.to_u32();
        let data_offset = self.offset as usize + 4;
        let len = self.count as usize;
        if self.format == 1 {
            let glyphs = parent_data
                .read_array::<BigEndian<GlyphId16>>(data_offset..data_offset + len * 2)
                .ok()?;
            glyphs
                .binary_search_by_key(&gid, |g| g.get().to_u32())
                .ok()
                .map(|idx| idx as _)
        } else {
            use core::cmp::Ordering;
            let records = parent_data
                .read_array::<RangeRecord>(
                    data_offset..data_offset + len * size_of::<RangeRecord>(),
                )
                .ok()?;
            records
                .binary_search_by(|rec| {
                    if rec.end_glyph_id().to_u32() < gid {
                        Ordering::Less
                    } else if rec.start_glyph_id().to_u32() > gid {
                        Ordering::Greater
                    } else {
                        Ordering::Equal
                    }
                })
                .ok()
                .map(|idx| {
                    let rec = &records[idx];
                    (rec.start_coverage_index() as u32 + gid - rec.start_glyph_id().to_u32()) as u16
                })
        }
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct ClassDefInfo {
    pub offset: u16,
    pub format: u16,
    // For format 1 only
    pub start_glyph_id: u16,
    pub count: u16,
}

impl ClassDefInfo {
    pub(crate) fn new_with<P: ColdAdmission>(
        parent_data: &FontData,
        offset: u16,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        // Format 1 reads format, start and count; format 2 reads two fields.
        policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
        Ok(Self::new(parent_data, offset))
    }
    pub fn new(parent_data: &FontData, offset: u16) -> Option<Self> {
        if offset == 0 {
            return None;
        }
        let format = parent_data.read_at::<u16>(offset as usize).ok()?;
        if format != 1 && format != 2 {
            return None;
        }
        let (start_glyph_id, count) = if format == 1 {
            let start_glyph_id = parent_data.read_at::<u16>(offset as usize + 2).ok()?;
            let count = parent_data.read_at::<u16>(offset as usize + 4).ok()?;
            (start_glyph_id, count)
        } else if format == 2 {
            let count = parent_data.read_at::<u16>(offset as usize + 2).ok()?;
            (0, count)
        } else {
            return None;
        };
        Some(Self {
            offset,
            format,
            start_glyph_id,
            count,
        })
    }

    pub fn class(&self, parent_data: &FontData, gid: GlyphId) -> u16 {
        let offset = self.offset as usize;
        if offset == 0 {
            return 0;
        }
        let gid = gid.to_u32();
        if self.format == 1 {
            let Some(idx) = gid.checked_sub(self.start_glyph_id as u32) else {
                return 0;
            };
            if idx >= self.count as u32 {
                return 0;
            }
            parent_data
                .read_at::<u16>(offset + 6 + idx as usize * 2)
                .unwrap_or(0)
        } else {
            use core::cmp::Ordering;
            let start = offset + 4;
            let end = start + self.count as usize * size_of::<ClassRangeRecord>();
            let Ok(records) = parent_data.read_array::<ClassRangeRecord>(start..end) else {
                return 0;
            };
            records
                .binary_search_by(|rec| {
                    if rec.end_glyph_id().to_u32() < gid {
                        Ordering::Less
                    } else if rec.start_glyph_id().to_u32() > gid {
                        Ordering::Greater
                    } else {
                        Ordering::Equal
                    }
                })
                .ok()
                .map_or(0, |idx| records[idx].class())
        }
    }
}
