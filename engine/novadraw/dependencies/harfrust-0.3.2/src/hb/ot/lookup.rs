use crate::hb::{
    hb_font_t,
    ot_layout::TableIndex,
    ot_layout_gsubgpos::{
        Apply, SubtableExternalCache, SubtableExternalCacheMode, WouldApply, WouldApplyContext,
        OT::hb_ot_apply_context_t,
    },
    set_digest::hb_set_digest_t,
    GlyphInfo,
};
use alloc::vec::Vec;
use crate::hb::face::cold_admission::{ColdAdmission, ColdOperation};
use read_fonts::{
    tables::{
        gpos::{
            CursivePosFormat1, Gpos, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1,
            PairPosFormat1, PairPosFormat2, SinglePosFormat1, SinglePosFormat2,
        },
        gsub::{
            AlternateSubstFormat1, ExtensionSubstFormat1, Gsub, LigatureSubstFormat1,
            MultipleSubstFormat1, ReverseChainSingleSubstFormat1, SingleSubstFormat1,
            SingleSubstFormat2,
        },
        layout::{
            ChainedSequenceContextFormat1, ChainedSequenceContextFormat2,
            ChainedSequenceContextFormat3, CoverageTable, Lookup, LookupFlag,
            SequenceContextFormat1, SequenceContextFormat2, SequenceContextFormat3,
        },
    },
    FontData, FontRead, Offset, ReadError,
};

pub struct LookupData<'a> {
    /// Offset of the lookup from the base of the layout table.
    offset: usize,
    /// True if the lookup comes from GSUB.
    is_subst: bool,
    /// Data of the layout table.
    table_data: FontData<'a>,
}

pub trait LookupHost<'a> {
    fn lookup_count(&self) -> u16;
    fn lookup_data(&self, index: u16) -> Result<LookupData<'a>, ReadError>;
}

impl<'a> LookupHost<'a> for Gsub<'a> {
    fn lookup_count(&self) -> u16 {
        self.lookup_list()
            .map(|list| list.lookup_count())
            .unwrap_or_default()
    }

    fn lookup_data(&self, index: u16) -> Result<LookupData<'a>, ReadError> {
        let list = self.lookup_list()?;
        let offset = list
            .lookup_offsets()
            .get(index as usize)
            .ok_or(ReadError::OutOfBounds)?
            .get()
            .to_usize()
            + self.lookup_list_offset().to_usize();
        Ok(LookupData {
            offset,
            is_subst: true,
            table_data: self.offset_data(),
        })
    }
}

impl<'a> LookupHost<'a> for Gpos<'a> {
    fn lookup_count(&self) -> u16 {
        self.lookup_list()
            .map(|list| list.lookup_count())
            .unwrap_or_default()
    }

    fn lookup_data(&self, index: u16) -> Result<LookupData<'a>, ReadError> {
        let list = self.lookup_list()?;
        let offset = list
            .lookup_offsets()
            .get(index as usize)
            .ok_or(ReadError::OutOfBounds)?
            .get()
            .to_usize()
            + self.lookup_list_offset().to_usize();
        Ok(LookupData {
            offset,
            is_subst: false,
            table_data: self.offset_data(),
        })
    }
}

#[cfg(feature = "std")]
mod cache {
    use super::{LookupHost, LookupInfo};
    use crate::hb::face::cold_admission::{ColdAdmission, ColdOperation, HeapSlot, OrdinaryCold};
    use std::sync::OnceLock;

    #[derive(Default)]
    pub(crate) struct LookupCache {
        lookups: Vec<OnceLock<Option<HeapSlot<LookupInfo>>>>,
    }

    impl LookupCache {
        pub(crate) fn new_with<'a, P: ColdAdmission>(
            host: &impl LookupHost<'a>,
            policy: &mut P,
        ) -> Result<Option<Self>, P::Error> {
            // LookupList::read and lookup_count each read a u16 count; the
            // table accessor reads an Offset16. It creates borrowed arrays.
            policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
            let count = host.lookup_count() as usize;
            let mut lookups = Vec::new();
            policy.reserve(&mut lookups, count)?;
            // Initialize one empty OnceLock per slot and prepay its cleanup.
            // No LookupInfo or Box is constructed on this std cold path.
            policy.charge(ColdOperation::Initialize, count * 2, 0)?;
            lookups.resize_with(count, Default::default);
            Ok(Some(Self { lookups }))
        }

        pub fn get<'a>(&self, host: &impl LookupHost<'a>, index: u16) -> Option<&LookupInfo> {
            self.lookups.get(index as usize)?
                .get_or_init(|| match Self::load_with(host, index, &mut OrdinaryCold) {
                    Ok(value) => value,
                    Err(never) => match never {},
                })
                .as_ref().map(|v| &**v)
        }

        fn load_with<'a, P: ColdAdmission>(
            host: &impl LookupHost<'a>,
            index: u16,
            policy: &mut P,
        ) -> Result<Option<HeapSlot<LookupInfo>>, P::Error> {
            // LookupList count, list offset and selected lookup offset.
            policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
            match host.lookup_data(index) {
                Ok(data) => match LookupInfo::new_with(&data, policy)? {
                    Some(info) => Ok(Some(HeapSlot::new_with(info, policy)?)),
                    None => Ok(None),
                },
                Err(_) => Ok(None),
            }
        }

        pub(crate) fn get_with<'a, P: ColdAdmission>(
            &self,
            host: &impl LookupHost<'a>,
            index: u16,
            policy: &mut P,
        ) -> Result<Option<&LookupInfo>, P::Error> {
            policy.charge(ColdOperation::Planning, 1, 0)?;
            let Some(slot) = self.lookups.get(index as usize) else { return Ok(None) };
            if slot.get().is_none() {
                let value = Self::load_with(host, index, policy)?;
                // Only complete candidates are published. A racing complete
                // candidate may win; the losing candidate's cleanup is prepaid.
                let _ = slot.set(value);
            }
            Ok(slot.get().and_then(Option::as_ref).map(|v| &**v))
        }
    }
}

#[cfg(not(feature = "std"))]
mod cache {
    use super::{LookupHost, LookupInfo, Vec};
    use crate::hb::face::cold_admission::{ColdAdmission, ColdOperation, OrdinaryCold};

    #[derive(Default)]
    pub(crate) struct LookupCache {
        lookups: Vec<Option<LookupInfo>>,
    }

    impl LookupCache {
        pub(crate) fn new_with<'a, P: ColdAdmission>(
            host: &impl LookupHost<'a>,
            policy: &mut P,
        ) -> Result<Option<Self>, P::Error> {
            policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
            let count = host.lookup_count();
            let mut lookups = Vec::new();
            policy.reserve(&mut lookups, count as usize)?;
            for i in 0..count {
                policy.charge(ColdOperation::Lookup, 2 + 2 + 2 + 2, 0)?;
                let info = match host.lookup_data(i) {
                    Ok(data) => LookupInfo::new_with(&data, policy)?,
                    Err(_) => None,
                };
                lookups.push(info);
            }
            Ok(Some(Self { lookups }))
        }

        pub fn get<'a>(&self, _host: &impl LookupHost<'a>, index: u16) -> Option<&LookupInfo> {
            match self.get_with(_host, index, &mut OrdinaryCold) {
                Ok(value) => value,
                Err(never) => match never {},
            }
        }

        pub(crate) fn get_with<'a, P: ColdAdmission>(
            &self,
            _host: &impl LookupHost<'a>,
            index: u16,
            policy: &mut P,
        ) -> Result<Option<&LookupInfo>, P::Error> {
            policy.charge(ColdOperation::Planning, 1, 0)?;
            Ok(self.lookups.get(index as usize).and_then(Option::as_ref))
        }
    }
}

pub(crate) use cache::LookupCache;

fn is_reversed(table_data: FontData, lookup: &Lookup<()>, lookup_offset: usize) -> Option<bool> {
    match lookup.lookup_type() {
        // Reverse chain context
        8 => Some(true),
        // Extension table
        7 => {
            let offset = lookup_offset + lookup.subtable_offsets().first()?.get().to_usize();
            let data = table_data.split_off(offset)?;
            let ext = ExtensionSubstFormat1::<()>::read(data).ok()?;
            Some(ext.extension_lookup_type() == 8)
        }
        _ => Some(false),
    }
}

/// Cached information about a lookup.
#[derive(Default)]
pub struct LookupInfo {
    pub props: u32,
    pub is_subst: bool,
    pub is_reversed: bool,
    pub digest: hb_set_digest_t,
    pub subtable_cache_user_idx: Option<usize>,
    pub subtables: Vec<SubtableInfo>,
}

impl LookupInfo {
    pub(crate) fn new_with<P: ColdAdmission>(
        data: &LookupData,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        let mut info = Self {
            is_subst: data.is_subst,
            ..Default::default()
        };
        let Some(lookup_data) = data.table_data.split_off(data.offset) else { return Ok(None) };
        // Reader flag/count, then type, flags twice and optional mark set.
        policy.charge(ColdOperation::TableRead, 2 * 6, 0)?;
        let Ok(lookup): Result<Lookup<()>, _> = Lookup::read(lookup_data) else { return Ok(None) };
        let lookup_type = lookup.lookup_type();
        let lookup_flag = lookup.lookup_flag();
        info.props = u32::from(lookup.lookup_flag().to_bits());
        if lookup_flag.to_bits() & LookupFlag::USE_MARK_FILTERING_SET.to_bits() != 0 {
            info.props |= (lookup.mark_filtering_set().unwrap_or_default() as u32) << 16;
        }
        if data.is_subst {
            // Reverse test reads lookup type, first offset and extension type.
            policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
            info.is_reversed =
                is_reversed(data.table_data, &lookup, data.offset).unwrap_or_default();
        }
        let mut subtable_cache_user_cost = 0;
        policy.charge(ColdOperation::Planning, 2, 0)?;
        policy.reserve(&mut info.subtables, lookup.sub_table_count() as usize)?;
        for (idx, subtable_offset) in lookup.subtable_offsets().iter().enumerate() {
            // Offset read, digest union and element write/cleanup.
            policy.charge(ColdOperation::Lookup, 2 + 3 + 2, 0)?;
            let cache_mode = if idx < 8 {
                SubtableExternalCacheMode::Full
            } else {
                SubtableExternalCacheMode::Small
            };
            let subtable_offset = subtable_offset.get().to_usize() + data.offset;
            if let Some((subtable_info, cache_cost)) = SubtableInfo::new_with(
                data.table_data,
                subtable_offset as u32,
                data.is_subst,
                lookup_type as u8,
                cache_mode,
                policy,
            )? {
                info.digest.union(&subtable_info.digest);
                if cache_cost > subtable_cache_user_cost {
                    info.subtable_cache_user_idx = Some(info.subtables.len());
                    subtable_cache_user_cost = cache_cost;
                }
                info.subtables.push(subtable_info);
            }
        }
        // Admitted capacity is retained: shrinking could allocate again.
        // The ordinary constructor retains its original shrink behavior.
        if !P::ADMITTED {
            info.subtables.shrink_to_fit();
        }
        Ok(Some(info))
    }

    pub fn props(&self) -> u32 {
        self.props
    }

    pub fn is_reverse(&self) -> bool {
        self.is_reversed
    }

    pub fn digest(&self) -> &hb_set_digest_t {
        &self.digest
    }
}

impl LookupInfo {
    #[inline]
    pub(crate) fn apply(
        &self,
        ctx: &mut hb_ot_apply_context_t,
        table_data: &[u8],
        use_hot_subtable_cache: bool,
    ) -> Option<()> {
        let glyph = ctx.buffer.cur(0).glyph_id;
        for (subtable_idx, subtable_info) in self.subtables.iter().enumerate() {
            if !subtable_info.digest.may_have(glyph) {
                continue;
            }
            let is_cached =
                use_hot_subtable_cache & (self.subtable_cache_user_idx == Some(subtable_idx));
            if subtable_info.apply(ctx, table_data, is_cached).is_some() {
                return Some(());
            }
        }
        None
    }

    pub(crate) fn cache_enter(&self, ctx: &mut hb_ot_apply_context_t) -> bool {
        let Some(idx) = self.subtable_cache_user_idx else {
            return false;
        };
        let Some(subtable_info) = self.subtables.get(idx) else {
            return false;
        };
        if matches!(
            subtable_info.kind,
            SubtableKind::ContextFormat2 | SubtableKind::ChainedContextFormat2
        ) {
            cache_enter(ctx)
        } else {
            false
        }
    }
    pub(crate) fn cache_leave(&self, ctx: &mut hb_ot_apply_context_t) {
        let Some(idx) = self.subtable_cache_user_idx else {
            return;
        };
        let Some(subtable_info) = self.subtables.get(idx) else {
            return;
        };
        if matches!(
            subtable_info.kind,
            SubtableKind::ContextFormat2 | SubtableKind::ChainedContextFormat2
        ) {
            cache_leave(ctx);
        }
    }
}

impl LookupInfo {
    pub fn would_apply(&self, face: &hb_font_t, ctx: &WouldApplyContext) -> Option<bool> {
        let glyph = ctx.glyphs[0];
        if !self.digest.may_have(glyph.into()) {
            return Some(false);
        }
        let table_index = if self.is_subst {
            TableIndex::GSUB
        } else {
            TableIndex::GPOS
        };
        let table_data = face.ot_tables.table_data(table_index)?;
        for subtable_info in &self.subtables {
            if !subtable_info.digest.may_have(glyph.into()) {
                continue;
            }
            let Some(data) = table_data.get(subtable_info.offset as usize..) else {
                continue;
            };
            let data = FontData::new(data);
            let result = match subtable_info.kind {
                SubtableKind::SingleSubst1 => {
                    SingleSubstFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::SingleSubst2 => {
                    SingleSubstFormat2::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::MultipleSubst1 => {
                    MultipleSubstFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::AlternateSubst1 => {
                    AlternateSubstFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::LigatureSubst1 => {
                    LigatureSubstFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ReverseChainContext => {
                    ReverseChainSingleSubstFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ContextFormat1 => {
                    SequenceContextFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ContextFormat2 => {
                    SequenceContextFormat2::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ContextFormat3 => {
                    SequenceContextFormat3::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ChainedContextFormat1 => {
                    ChainedSequenceContextFormat1::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ChainedContextFormat2 => {
                    ChainedSequenceContextFormat2::read(data).map(|t| t.would_apply(ctx))
                }
                SubtableKind::ChainedContextFormat3 => {
                    ChainedSequenceContextFormat3::read(data).map(|t| t.would_apply(ctx))
                }
                _ => continue,
            };
            if result == Ok(true) {
                return Some(true);
            }
        }
        None
    }
}

/// Cached information about a subtable.
pub struct SubtableInfo {
    /// The fully resolved type of the subtable.
    pub kind: SubtableKind,
    /// Byte offset to the subtable from the base of the GSUB or GPOS
    /// table.
    pub offset: u32,
    pub digest: hb_set_digest_t,
    pub apply_fns: [SubtableApplyFn; 2],
    pub external_cache: SubtableExternalCache,
}

pub type SubtableApplyFn =
    fn(&mut hb_ot_apply_context_t, &SubtableExternalCache, &[u8]) -> Option<()>;

impl SubtableInfo {
    #[inline]
    pub(crate) fn apply(
        &self,
        ctx: &mut hb_ot_apply_context_t,
        table_data: &[u8],
        is_cached: bool,
    ) -> Option<()> {
        let subtable_data = table_data.get(self.offset as usize..)?;
        self.apply_fns[is_cached as usize](ctx, &self.external_cache, subtable_data)
    }
}

macro_rules! apply_fns {
    ($apply:ident, $apply_cached:ident, $ty:ident) => {
        fn $apply(
            ctx: &mut hb_ot_apply_context_t,
            external_cache: &SubtableExternalCache,
            table_data: &[u8],
        ) -> Option<()> {
            let t = $ty::read(FontData::new(table_data)).ok()?;
            t.apply_with_external_cache(ctx, external_cache)
        }

        fn $apply_cached(
            ctx: &mut hb_ot_apply_context_t,
            external_cache: &SubtableExternalCache,
            table_data: &[u8],
        ) -> Option<()> {
            let t = $ty::read(FontData::new(table_data)).ok()?;
            t.apply_cached(ctx, external_cache)
        }
    };
}

apply_fns!(single_subst1, single_subst1_cached, SingleSubstFormat1);
apply_fns!(single_subst2, single_subst2_cached, SingleSubstFormat2);
apply_fns!(
    multiple_subst1,
    multiple_subst1_cached,
    MultipleSubstFormat1
);
apply_fns!(
    alternate_subst1,
    alternate_subst1_cached,
    AlternateSubstFormat1
);
apply_fns!(
    ligature_subst1,
    ligature_subst1_cached,
    LigatureSubstFormat1
);
apply_fns!(single_pos1, single_pos1_cached, SinglePosFormat1);
apply_fns!(single_pos2, single_pos2_cached, SinglePosFormat2);
apply_fns!(pair_pos1, pair_pos1_cached, PairPosFormat1);
apply_fns!(pair_pos2, pair_pos2_cached, PairPosFormat2);
apply_fns!(cursive_pos1, cursive_pos1_cached, CursivePosFormat1);
apply_fns!(mark_base_pos1, mark_base_pos1_cached, MarkBasePosFormat1);
apply_fns!(mark_mark_pos1, mark_mark_pos1_cached, MarkMarkPosFormat1);
apply_fns!(mark_lig_pos1, mark_lig_pos1_cached, MarkLigPosFormat1);
apply_fns!(context1, context1_cached, SequenceContextFormat1);
apply_fns!(context2, context2_cached, SequenceContextFormat2);
apply_fns!(context3, context3_cached, SequenceContextFormat3);
apply_fns!(
    chained_context1,
    chained_context1_cached,
    ChainedSequenceContextFormat1
);
apply_fns!(
    chained_context2,
    chained_context2_cached,
    ChainedSequenceContextFormat2
);
apply_fns!(
    chained_context3,
    chained_context3_cached,
    ChainedSequenceContextFormat3
);
apply_fns!(
    rev_chain_single_subst1,
    rev_chain_single_subst1_cached,
    ReverseChainSingleSubstFormat1
);

/// All possible subtables in a lookup.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubtableKind {
    SingleSubst1,
    SingleSubst2,
    MultipleSubst1,
    AlternateSubst1,
    LigatureSubst1,
    SinglePos1,
    SinglePos2,
    PairPos1,
    PairPos2,
    CursivePos1,
    MarkBasePos1,
    MarkMarkPos1,
    MarkLigPos1,
    ContextFormat1,
    ContextFormat2,
    ContextFormat3,
    ChainedContextFormat1,
    ChainedContextFormat2,
    ChainedContextFormat3,
    ReverseChainContext,
}

impl SubtableInfo {
    fn new_with<P: ColdAdmission>(
        table_data: FontData,
        mut subtable_offset: u32,
        is_subst: bool,
        mut lookup_type: u8,
        cache_mode: SubtableExternalCacheMode,
        policy: &mut P,
    ) -> Result<Option<(Self, u32)>, P::Error> {
        macro_rules! valid {
            ($value:expr) => {
                match $value { Some(value) => value, None => return Ok(None) }
            };
        }
        // Resolve extensions iteratively. Positive offsets strictly progress;
        // a self extension with zero offset cannot produce a finite subtable.
        let data = loop {
            let data = valid!(table_data.split_off(subtable_offset as usize));
            if !matches!((is_subst, lookup_type), (true, 7) | (false, 9)) {
                break data;
            }
            policy.charge(ColdOperation::TableRead, 2 + 4, 0)?;
            let ext = valid!(ExtensionSubstFormat1::<'_, ()>::read(data).ok());
            let ext_type = ext.extension_lookup_type() as u8;
            let offset = ext.extension_offset().to_u32();
            if offset == 0 && matches!((is_subst, ext_type), (true, 7) | (false, 9)) {
                return Ok(None);
            }
            subtable_offset = valid!(subtable_offset.checked_add(offset));
            lookup_type = ext_type;
        };
        // Format dispatch is one u16. Typed readers below borrow arrays and
        // only read their count/value-format fields before advancing spans.
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let format = valid!(data.read_at::<u16>(0).ok());
        macro_rules! build {
            ($ty:ident, $kind:ident, $coverage:ident, $header:expr, $apply:ident, $cached:ident) => {{
                policy.charge(ColdOperation::TableRead, $header, 0)?;
                let table = valid!($ty::read(data).ok());
                let external = table.external_cache_create_with(cache_mode, policy)?;
                // Context cache_cost parses at most two class definitions:
                // each offset + format dispatch + up to two header/count
                // fields and one cost getter. Other table costs are zero.
                let cost_headers = match SubtableKind::$kind {
                    SubtableKind::ContextFormat2 => 2 + 2 + 4 + 2,
                    SubtableKind::ChainedContextFormat2 => (2 + 2 + 4 + 2) * 2,
                    _ => 0,
                };
                policy.charge(ColdOperation::TableRead, cost_headers, 0)?;
                let cost = table.cache_cost();
                // Offset16, coverage format and count; no payload traversal.
                policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
                let coverage = valid!(table.$coverage().ok());
                (SubtableKind::$kind, (external, cost, coverage), [$apply, $cached as _])
            }};
        }
        let (kind, (external_cache, cache_cost, coverage), apply_fns): (
            SubtableKind,
            (SubtableExternalCache, u32, CoverageTable),
            [SubtableApplyFn; 2],
        ) = match (is_subst, lookup_type, format) {
            (true, 1, 1) => build!(SingleSubstFormat1, SingleSubst1, coverage, 0, single_subst1, single_subst1_cached),
            (true, 1, 2) => build!(SingleSubstFormat2, SingleSubst2, coverage, 2, single_subst2, single_subst2_cached),
            (false, 1, 1) => build!(SinglePosFormat1, SinglePos1, coverage, 2, single_pos1, single_pos1_cached),
            (false, 1, 2) => build!(SinglePosFormat2, SinglePos2, coverage, 2 + 2, single_pos2, single_pos2_cached),
            (true, 2, _) => build!(MultipleSubstFormat1, MultipleSubst1, coverage, 2, multiple_subst1, multiple_subst1_cached),
            (false, 2, 1) => build!(PairPosFormat1, PairPos1, coverage, 2, pair_pos1, pair_pos1_cached),
            (false, 2, 2) => build!(PairPosFormat2, PairPos2, coverage, 2 * 4, pair_pos2, pair_pos2_cached),
            (true, 3, _) => build!(AlternateSubstFormat1, AlternateSubst1, coverage, 2, alternate_subst1, alternate_subst1_cached),
            (false, 3, _) => build!(CursivePosFormat1, CursivePos1, coverage, 2, cursive_pos1, cursive_pos1_cached),
            (true, 4, _) => build!(LigatureSubstFormat1, LigatureSubst1, coverage, 2, ligature_subst1, ligature_subst1_cached),
            (false, 4, _) => build!(MarkBasePosFormat1, MarkBasePos1, mark_coverage, 0, mark_base_pos1, mark_base_pos1_cached),
            (true, 5, 1) | (false, 7, 1) => build!(SequenceContextFormat1, ContextFormat1, coverage, 2, context1, context1_cached),
            (true, 5, 2) | (false, 7, 2) => build!(SequenceContextFormat2, ContextFormat2, coverage, 2, context2, context2_cached),
            (false, 5, _) => build!(MarkLigPosFormat1, MarkLigPos1, mark_coverage, 0, mark_lig_pos1, mark_lig_pos1_cached),
            (true, 6, 1) | (false, 8, 1) => build!(ChainedSequenceContextFormat1, ChainedContextFormat1, coverage, 2, chained_context1, chained_context1_cached),
            (true, 6, 2) | (false, 8, 2) => build!(ChainedSequenceContextFormat2, ChainedContextFormat2, coverage, 2, chained_context2, chained_context2_cached),
            (true, 5, 3) | (false, 7, 3) => {
                policy.charge(ColdOperation::TableRead, 2 * 2 + 2 + 2 + 2, 0)?;
                let table = valid!(SequenceContextFormat3::read(data).ok());
                (SubtableKind::ContextFormat3,
                 (table.external_cache_create_with(cache_mode, policy)?, table.cache_cost(), valid!(table.coverages().get(0).ok())),
                 [context3, context3_cached as _])
            }
            (true, 6, 3) | (false, 8, 3) => {
                // Four counts, then input offset + coverage format/count.
                policy.charge(ColdOperation::TableRead, 2 * 4 + 2 + 2 + 2, 0)?;
                let table = valid!(ChainedSequenceContextFormat3::read(data).ok());
                (SubtableKind::ChainedContextFormat3,
                 (table.external_cache_create_with(cache_mode, policy)?, table.cache_cost(), valid!(table.input_coverages().get(0).ok())),
                 [chained_context3, chained_context3_cached as _])
            }
            (false, 6, _) => build!(MarkMarkPosFormat1, MarkMarkPos1, mark1_coverage, 0, mark_mark_pos1, mark_mark_pos1_cached),
            (true, 8, _) => build!(ReverseChainSingleSubstFormat1, ReverseChainContext, coverage, 2 * 3, rev_chain_single_subst1, rev_chain_single_subst1_cached),
            _ => return Ok(None),
        };
        let mut digest = hb_set_digest_t::new();
        digest.add_coverage_with(&coverage, policy)?;
        Ok(Some((
            SubtableInfo {
                kind,
                offset: subtable_offset,
                digest,
                apply_fns,
                external_cache,
            },
            cache_cost,
        )))
    }
}

fn cache_enter(ctx: &mut hb_ot_apply_context_t) -> bool {
    if !ctx.buffer.try_allocate_var(GlyphInfo::SYLLABLE_VAR) {
        return false;
    }
    for info in &mut ctx.buffer.info {
        info.set_syllable(255);
    }
    ctx.new_syllables = Some(255);
    true
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    use crate::hb::face::cold_admission::{AdmittedCold, OrdinaryCold};
    use core::convert::Infallible;

    fn words(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_be_bytes()).collect()
    }

    #[test]
    fn full_and_small_ligature_candidates_preserve_internal_metadata() {
        let mut bytes = words(&[4, 0, 10]);
        bytes.extend(words(&[26; 10]));
        bytes.extend(words(&[
            1, 10, 2, 16, 32, 1, 1, 1,
            2, 6, 12, 42, 2, 3, 43, 1,
            2, 0xffff, 6, 44, 2, 4,
        ]));
        let data = LookupData { offset: 0, is_subst: true, table_data: FontData::new(&bytes) };
        let ordinary = LookupInfo::new_with(&data, &mut OrdinaryCold).unwrap().unwrap();
        let admitted = LookupInfo::new_with(&data, &mut AdmittedCold(|_, _| Ok::<_, Infallible>(())))
            .unwrap().unwrap();
        assert_eq!(ordinary.props, admitted.props);
        assert_eq!(ordinary.is_reversed, admitted.is_reversed);
        assert_eq!(ordinary.subtable_cache_user_idx, admitted.subtable_cache_user_idx);
        assert_eq!(alloc::format!("{:?}", ordinary.digest), alloc::format!("{:?}", admitted.digest));
        assert_eq!(ordinary.subtables.len(), 10);
        assert_eq!(admitted.subtables.len(), 10);
        for (index, (a, b)) in ordinary.subtables.iter().zip(&admitted.subtables).enumerate() {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.offset, b.offset);
            assert_eq!(alloc::format!("{:?}", a.digest), alloc::format!("{:?}", b.digest));
            match (&a.external_cache, &b.external_cache) {
                (SubtableExternalCache::LigatureSubstFormat1Cache(a),
                 SubtableExternalCache::LigatureSubstFormat1Cache(b)) => {
                    assert!(index < 8);
                    assert_eq!(alloc::format!("{:?}", a.seconds), alloc::format!("{:?}", b.seconds));
                }
                (SubtableExternalCache::LigatureSubstFormat1SmallCache(a),
                 SubtableExternalCache::LigatureSubstFormat1SmallCache(b)) => {
                    assert!(index >= 8);
                    assert_eq!(alloc::format!("{:?}", a.seconds), alloc::format!("{:?}", b.seconds));
                    assert_eq!(alloc::format!("{:?}", a.coverage), alloc::format!("{:?}", b.coverage));
                }
                _ => panic!("cache mode changed"),
            }
        }
    }

    #[test]
    fn nonprogressing_extension_is_skipped_without_a_fabricated_subtable() {
        let bytes = words(&[7, 0, 1, 8, 1, 7, 0, 0]);
        let data = LookupData { offset: 0, is_subst: true, table_data: FontData::new(&bytes) };
        let candidate = LookupInfo::new_with(&data, &mut AdmittedCold(|_, _| Ok::<_, Infallible>(())))
            .unwrap().unwrap();
        assert!(candidate.subtables.is_empty());
        assert!(candidate.subtable_cache_user_idx.is_none());
    }
}

fn cache_leave(ctx: &mut hb_ot_apply_context_t) {
    ctx.new_syllables = None;
    ctx.buffer.deallocate_var(GlyphInfo::SYLLABLE_VAR);
}
