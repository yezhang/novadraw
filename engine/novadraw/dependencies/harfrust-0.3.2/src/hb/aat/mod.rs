pub mod layout;
pub mod layout_common;
pub mod layout_kerx_table;
pub mod layout_morx_table;
pub mod layout_trak_table;
pub mod map;
mod cold_headers;

use crate::hb::aat::layout_kerx_table::KerxSubtableCache;
use crate::hb::aat::layout_morx_table::MorxSubtableCache;
use crate::hb::kerning::KernSubtableCache;
use crate::hb::tables::TableOffsets;
use alloc::vec::Vec;
use read_fonts::{
    tables::{ankr::Ankr, feat::Feat, kern::Kern, kerx::Kerx, morx::Morx, trak::Trak},
    FontRef, TableProvider,
};
use crate::hb::face::cold_admission::{directory, ColdAdmission, ColdOperation, OrdinaryCold};

#[derive(Default)]
pub struct AatCache {
    pub morx: Vec<MorxSubtableCache>,
    pub kern: Vec<KernSubtableCache>,
    pub kerx: Vec<KerxSubtableCache>,
}

impl AatCache {
    #[allow(unused)]
    pub fn new(font: &FontRef) -> Self {
        match Self::new_with(font, &mut OrdinaryCold) {
            Ok(Some(value)) => value,
            Ok(None) => unreachable!("ordinary cold policy supports every AAT cache"),
            Err(never) => match never {},
        }
    }

    pub(crate) fn new_with<P: ColdAdmission>(
        font: &FontRef,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        if P::ADMITTED {
            return Ok(cold_headers::empty_caches(font, policy)?.then(Self::default));
        }
        let mut cache = Self::default();
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 4 + 2, 0)?;
        let num_glyphs = font
            .maxp()
            .map(|maxp| maxp.num_glyphs() as u32)
            .unwrap_or_default();
        directory(font, policy)?;
        // Admitted paths already returned above. Ordinary policy is a no-op;
        // its existing variable-size readers remain unchanged.
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        if let Ok(morx) = font.morx() {
            for chain in morx.chains().iter() {
                let Ok(chain) = chain else {
                    continue;
                };
                for subtable in chain.subtables().iter() {
                    let Ok(subtable) = subtable else {
                        continue;
                    };
                    cache
                        .morx
                        .push(MorxSubtableCache::new(&subtable, num_glyphs));
                }
            }
        }
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 2 + 4 + 4, 0)?;
        if let Ok(kern) = font.kern() {
            for subtable in kern.subtables() {
                let Ok(subtable) = subtable else {
                    continue;
                };
                cache
                    .kern
                    .push(KernSubtableCache::new(&subtable, num_glyphs));
            }
        }
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        if let Ok(kerx) = font.kerx() {
            for subtable in kerx.subtables().iter() {
                let Ok(subtable) = subtable else {
                    continue;
                };
                cache
                    .kerx
                    .push(KerxSubtableCache::new(&subtable, num_glyphs));
            }
        }
        Ok(Some(cache))
    }
}

#[derive(Clone, Default)]
pub struct AatTables<'a> {
    pub morx: Option<(Morx<'a>, &'a [MorxSubtableCache])>,
    pub ankr: Option<Ankr<'a>>,
    pub kern: Option<(Kern<'a>, &'a [KernSubtableCache])>,
    pub kerx: Option<(Kerx<'a>, &'a [KerxSubtableCache])>,
    pub trak: Option<Trak<'a>>,
    pub feat: Option<Feat<'a>>,
}

impl<'a> AatTables<'a> {
    pub(crate) fn new_with<P: ColdAdmission>(
        font: &FontRef<'a>, cache: &'a AatCache, table_offsets: &TableOffsets,
        policy: &mut P,
    ) -> Result<Option<Self>, P::Error> {
        if P::ADMITTED {
            // Inspect the exact stored offsets consumed by new(), not a
            // second directory lookup (which can differ on unsorted fonts).
            for (offset, tag) in [
                (table_offsets.morx, read_fonts::types::Tag::new(b"morx")),
                (table_offsets.kern, read_fonts::types::Tag::new(b"kern")),
                (table_offsets.kerx, read_fonts::types::Tag::new(b"kerx")),
            ] {
                policy.charge(ColdOperation::Planning, 1, 0)?;
                if let Some(data) = offset.resolve_data(font) {
                    if !cold_headers::empty_table(data, tag, policy)? {
                        return Ok(None);
                    }
                }
            }
        }
        // Empty morx/kerx only resolve fixed headers and zero-length
        // VarSize arrays. Kern's top-level reader resolves its header only.
        // ankr/trak/feat advance fixed headers and borrow array spans;
        // no nested anchors, tracks, settings or cache elements are read.
        const HEADER_BYTES: usize = 8 + 8 + 8 + 12 + 12 + 12;
        policy.charge(ColdOperation::TableRead, HEADER_BYTES, 0)?;
        Ok(Some(Self::new(font, cache, table_offsets)))
    }

    pub fn new(font: &FontRef<'a>, cache: &'a AatCache, table_offsets: &TableOffsets) -> Self {
        let morx = table_offsets
            .morx
            .resolve_table(font)
            .map(|table| (table, cache.morx.as_slice()));
        let ankr = table_offsets.ankr.resolve_table(font);
        let kern = table_offsets
            .kern
            .resolve_table(font)
            .map(|table| (table, cache.kern.as_slice()));
        let kerx = table_offsets
            .kerx
            .resolve_table(font)
            .map(|table| (table, cache.kerx.as_slice()));
        let trak = table_offsets.trak.resolve_table(font);
        let feat = table_offsets.feat.resolve_table(font);
        Self {
            morx,
            ankr,
            kern,
            kerx,
            trak,
            feat,
        }
    }
}
