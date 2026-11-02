use read_fonts::{
    tables::{
        ankr::Ankr,
        cmap::{Cmap, CmapSubtable, PlatformId},
        feat::Feat,
        gdef::Gdef,
        glyf::Glyf,
        gpos::Gpos,
        gsub::Gsub,
        gvar::Gvar,
        hmtx::Hmtx,
        hvar::Hvar,
        kern::Kern,
        kerx::Kerx,
        loca::Loca,
        morx::Morx,
        mvar::Mvar,
        trak::Trak,
        vmtx::Vmtx,
        vorg::Vorg,
        vvar::Vvar,
    },
    types::Tag,
    FontData, FontRead, FontRef, TableProvider, TopLevelTable,
};
use super::face::cold_admission::{directory, ColdAdmission, ColdOperation, OrdinaryCold};

// https://docs.microsoft.com/en-us/typography/opentype/spec/cmap#windows-platform-platform-id--3
const WINDOWS_SYMBOL_ENCODING: u16 = 0;
const WINDOWS_UNICODE_BMP_ENCODING: u16 = 1;
const WINDOWS_UNICODE_FULL_ENCODING: u16 = 10;

// https://docs.microsoft.com/en-us/typography/opentype/spec/name#platform-specific-encoding-and-language-ids-unicode-platform-platform-id--0
const UNICODE_1_0_ENCODING: u16 = 0;
const UNICODE_1_1_ENCODING: u16 = 1;
const UNICODE_ISO_ENCODING: u16 = 2;
const UNICODE_2_0_BMP_ENCODING: u16 = 3;
const UNICODE_2_0_FULL_ENCODING: u16 = 4;

//const UNICODE_VARIATION_ENCODING: u16 = 5;
const UNICODE_FULL_ENCODING: u16 = 6;

#[derive(Clone)]
pub struct TableOffsets {
    pub num_glyphs: u32,
    pub units_per_em: u16,
    pub loca_long: bool,
    pub num_v_metrics: u16,
    pub num_h_metrics: u16,
    pub ascent: i16,
    pub descent: i16,
    pub loca: TableOffset,
    pub glyf: TableOffset,
    pub gvar: TableOffset,
    pub hmtx: TableOffset,
    pub hvar: TableOffset,
    pub vmtx: TableOffset,
    pub vvar: TableOffset,
    pub vorg: TableOffset,
    pub mvar: TableOffset,
    pub cmap: TableOffset,
    pub cmap_subtable: Option<SelectedCmapSubtable>,
    pub cmap_vs_subtable: Option<u16>,
    pub gdef: TableOffset,
    pub gsub: TableOffset,
    pub gpos: TableOffset,
    pub morx: TableOffset,
    pub kerx: TableOffset,
    pub ankr: TableOffset,
    pub kern: TableOffset,
    pub feat: TableOffset,
    pub trak: TableOffset,
}

#[derive(Copy, Clone)]
pub struct SelectedCmapSubtable {
    pub index: u16,
    pub is_mac_roman: bool,
    pub is_symbol: bool,
}

impl TableOffsets {
    pub fn new(font: &FontRef) -> Self {
        match Self::new_with(font, &mut OrdinaryCold) {
            Ok(value) => value,
            Err(never) => match never {},
        }
    }

    pub(crate) fn new_with<P: ColdAdmission>(
        font: &FontRef,
        policy: &mut P,
    ) -> Result<Self, P::Error> {
        directory(font, policy)?;
        // Maxp::read reads its four-byte version; num_glyphs reads two bytes.
        policy.charge(ColdOperation::TableRead, 4 + 2, 0)?;
        let num_glyphs = font
            .maxp()
            .map(|maxp| maxp.num_glyphs() as u32)
            .unwrap_or_default();
        directory(font, policy)?;
        // Head::read only advances fields. Read units_per_em and loca format.
        policy.charge(ColdOperation::TableRead, 2 + 2, 0)?;
        let (units_per_em, loca_long) = font
            .head()
            .map(|head| (head.units_per_em(), head.index_to_loc_format() == 1))
            .unwrap_or((1000, false));
        directory(font, policy)?;
        // Os2::read reads the version; hhea only advances fixed fields.
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let os2 = font.os2().ok();
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 1, 0)?;
        let hhea = font.hhea().ok();
        policy.charge(ColdOperation::TableRead, 2 + 2, 0)?;
        let (ascent, descent) = if let Some(os2) = &os2 {
            (os2.s_typo_ascender(), os2.s_typo_descender())
        } else if let Some(hhea) = &hhea {
            (hhea.ascender().to_i16(), hhea.descender().to_i16())
        } else {
            (0, 0) // TODO
        };
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let num_h_metrics = hhea
            .map(|hhea| hhea.number_of_h_metrics())
            .unwrap_or_default();
        directory(font, policy)?;
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let num_v_metrics = font
            .vhea()
            .map(|vhea| vhea.number_of_long_ver_metrics())
            .unwrap_or_default();
        let mut offset = |tag| {
            directory(font, policy)?;
            Ok(TableOffset::new(font, tag).unwrap_or_default())
        };
        let loca = offset(Loca::TAG)?;
        let glyf = offset(Glyf::TAG)?;
        let gvar = offset(Gvar::TAG)?;
        let hmtx = offset(Hmtx::TAG)?;
        let hvar = offset(Hvar::TAG)?;
        let vmtx = offset(Vmtx::TAG)?;
        let vvar = offset(Vvar::TAG)?;
        let vorg = offset(Vorg::TAG)?;
        let mvar = offset(Mvar::TAG)?;
        let cmap = offset(Cmap::TAG)?;
        // Cmap::read reads a u16 count and creates a borrowed record array.
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let cmap_table: Option<Cmap> = cmap.resolve_table(font);
        let cmap_subtable = cmap_table
            .as_ref()
            .map(|cmap| find_best_cmap_subtable_with(cmap, policy))
            .transpose()?
            .flatten()
            .map(|(index, platform, encoding, _)| SelectedCmapSubtable {
                index,
                is_mac_roman: platform == PlatformId::Macintosh,
                is_symbol: platform == PlatformId::Windows && encoding == WINDOWS_SYMBOL_ENCODING,
            });
        let cmap_vs_subtable = if let Some(cmap) = cmap_table {
            let data = cmap.offset_data();
            let mut found = None;
            for (index, record) in cmap.encoding_records().iter().enumerate() {
                policy.charge(ColdOperation::CmapRecord, 1, 0)?;
                admit_cmap_subtable(policy)?;
                if matches!(record.subtable(data), Ok(CmapSubtable::Format14(_))) {
                    found = Some(index as u16);
                    break;
                }
            }
            found
        } else {
            None
        };
        let mut offset = |tag| {
            directory(font, policy)?;
            Ok(TableOffset::new(font, tag).unwrap_or_default())
        };
        let gdef = offset(Gdef::TAG)?;
        let gsub = offset(Gsub::TAG)?;
        let gpos = offset(Gpos::TAG)?;
        let morx = offset(Morx::TAG)?;
        let kerx = offset(Kerx::TAG)?;
        let ankr = offset(Ankr::TAG)?;
        let kern = offset(Kern::TAG)?;
        let feat = offset(Feat::TAG)?;
        let trak = offset(Trak::TAG)?;
        Ok(Self {
            num_glyphs,
            units_per_em,
            loca_long,
            num_h_metrics,
            num_v_metrics,
            ascent,
            descent,
            loca,
            glyf,
            gvar,
            hmtx,
            hvar,
            vmtx,
            vvar,
            vorg,
            mvar,
            cmap,
            cmap_subtable,
            cmap_vs_subtable,
            gdef,
            gsub,
            gpos,
            morx,
            kerx,
            ankr,
            kern,
            feat,
            trak,
        })
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct TableOffset(u32);

impl TableOffset {
    fn new(font: &FontRef, tag: Tag) -> Option<Self> {
        let records = font.table_directory().table_records();
        records
            .binary_search_by_key(&tag, |rec| rec.tag())
            .ok()
            .and_then(|ix| records.get(ix))
            .map(|rec| Self(rec.offset()))
    }

    pub fn resolve(self) -> Option<usize> {
        (self.0 != 0).then_some(self.0 as usize)
    }

    pub fn resolve_data<'a>(self, font: &FontRef<'a>) -> Option<FontData<'a>> {
        font.data().slice(self.resolve()?..)
    }

    pub fn resolve_table<'a, T: FontRead<'a>>(self, font: &FontRef<'a>) -> Option<T> {
        T::read(self.resolve_data(font)?).ok()
    }
}

fn find_best_cmap_subtable_with<'a, P: ColdAdmission>(
    cmap: &Cmap<'a>,
    policy: &mut P,
) -> Result<Option<(u16, PlatformId, u16, CmapSubtable<'a>)>, P::Error> {
    // Symbol subtable.
    // Prefer symbol if available.
    // https://github.com/harfbuzz/harfbuzz/issues/1918
    for (platform, encoding) in [
        (PlatformId::Windows, WINDOWS_SYMBOL_ENCODING),
        (PlatformId::Windows, WINDOWS_UNICODE_FULL_ENCODING),
        (PlatformId::Unicode, UNICODE_FULL_ENCODING),
        (PlatformId::Unicode, UNICODE_2_0_FULL_ENCODING),
        (PlatformId::Windows, WINDOWS_UNICODE_BMP_ENCODING),
        (PlatformId::Unicode, UNICODE_2_0_BMP_ENCODING),
        (PlatformId::Unicode, UNICODE_ISO_ENCODING),
        (PlatformId::Unicode, UNICODE_1_1_ENCODING),
        (PlatformId::Unicode, UNICODE_1_0_ENCODING),
        (PlatformId::Macintosh, 0),
    ] {
        if let Some(found) = find_cmap_subtable_with(cmap, platform, encoding, policy)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn find_cmap_subtable_with<'a, P: ColdAdmission>(
    cmap: &Cmap<'a>,
    platform_id: PlatformId,
    encoding_id: u16,
    policy: &mut P,
) -> Result<Option<(u16, PlatformId, u16, CmapSubtable<'a>)>, P::Error> {
    let offset_data = cmap.offset_data();
    for (index, record) in cmap.encoding_records().iter().enumerate() {
        // Record visit plus platform and encoding u16 fields.
        policy.charge(ColdOperation::CmapRecord, 1 + 2 + 2, 0)?;
        if record.platform_id() != platform_id || record.encoding_id() != encoding_id {
            continue;
        }
        admit_cmap_subtable(policy)?;
        if let Ok(subtable) = record.subtable(offset_data) {
            match subtable {
                CmapSubtable::Format0(_)
                | CmapSubtable::Format4(_)
                | CmapSubtable::Format6(_)
                | CmapSubtable::Format10(_)
                | CmapSubtable::Format12(_)
                | CmapSubtable::Format13(_) => {
                    return Ok(Some((index as u16, platform_id, encoding_id, subtable)))
                }
                _ => {}
            }
        }
    }
    Ok(None)
}

fn admit_cmap_subtable<P: ColdAdmission>(policy: &mut P) -> Result<(), P::Error> {
    // EncodingRecord::subtable reads Offset32; CmapSubtable::read reads u16
    // format. Every generated format reader only advances fixed/array spans
    // and reads at most one u32 count. Even format 8 skips its 8192-byte is32
    // array without visiting elements. No parser allocates or scans payloads.
    policy.charge(ColdOperation::CmapSubtable, 4 + 2 + 4, 0)
}
