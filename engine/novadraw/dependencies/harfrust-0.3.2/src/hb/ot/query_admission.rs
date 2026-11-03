// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Prepaid borrowed layout queries. Header costs include skipped fields;
//! payload arrays are borrowed without traversal by the locked readers.

use super::{Feature, FeatureList, LangSys, LayoutTable, Script, ScriptList};
use crate::hb::{
    common::TagExt,
    face::cold_admission::{ColdAdmission, ColdOperation},
    hb_tag_t,
};

impl<'a> LayoutTable<'a> {
    fn script_list_with<P: ColdAdmission>(
        &self,
        policy: &mut P,
    ) -> Result<Option<ScriptList<'a>>, P::Error> {
        // Parent Offset16 and ScriptList count.
        policy.charge(ColdOperation::TableRead, 2 + 2, 0)?;
        Ok(self.script_list())
    }

    fn feature_list_with<P: ColdAdmission>(
        &self,
        policy: &mut P,
    ) -> Result<Option<FeatureList<'a>>, P::Error> {
        // Parent Offset16 and FeatureList count.
        policy.charge(ColdOperation::TableRead, 2 + 2, 0)?;
        Ok(self.feature_list())
    }

    fn script_with<P: ColdAdmission>(
        &self,
        index: u16,
        policy: &mut P,
    ) -> Result<Option<Script<'a>>, P::Error> {
        let Some(list) = self.script_list_with(policy)? else {
            return Ok(None);
        };
        // TaggedElement reads the tag and offset, then the Script header.
        policy.charge(ColdOperation::TableRead, 4 + 2 + 4, 0)?;
        Ok(list.get(index).ok().map(|value| value.element))
    }

    fn langsys_with<P: ColdAdmission>(
        &self,
        script_index: u16,
        lang_index: Option<u16>,
        policy: &mut P,
    ) -> Result<Option<LangSys<'a>>, P::Error> {
        let Some(script) = self.script_with(script_index, policy)? else {
            return Ok(None);
        };
        // Nullable offset test + resolution (default), or one record offset,
        // and the six-byte LangSys header. No feature-index scan here.
        policy.charge(ColdOperation::TableRead, 2 + 2 + 6, 0)?;
        Ok(if let Some(index) = lang_index {
            script
                .lang_sys_records()
                .get(index as usize)
                .and_then(|record| record.lang_sys(script.offset_data()).ok())
        } else {
            script.default_lang_sys().transpose().ok().flatten()
        })
    }

    pub(crate) fn select_script_with<P: ColdAdmission>(
        &self,
        tags: &[hb_tag_t],
        policy: &mut P,
    ) -> Result<Option<(bool, u16, hb_tag_t)>, P::Error> {
        let Some(list) = self.script_list_with(policy)? else {
            return Ok(None);
        };
        policy.charge(ColdOperation::Planning, 1, 0)?;
        // read-fonts select tries every candidate and three fallback tags.
        // Each binary search performs at most n+1 record visits. Charging
        // separately avoids multiplication overflow and covers repeated passes.
        for _ in 0..tags.len() {
            policy.charge(ColdOperation::Feature, 1, 0)?;
            policy.charge(ColdOperation::Feature, list.script_records().len() + 1, 0)?;
        }
        for _ in 0..3 {
            policy.charge(ColdOperation::Feature, list.script_records().len() + 1, 0)?;
        }
        Ok(list
            .select(tags)
            .map(|selected| (!selected.is_fallback, selected.index, selected.tag)))
    }

    pub(crate) fn select_script_language_with<P: ColdAdmission>(
        &self,
        script_index: u16,
        tags: &[hb_tag_t],
        policy: &mut P,
    ) -> Result<Option<u16>, P::Error> {
        // Preserve the original repeated Script reads and final dflt query.
        for tag in tags.iter().copied().chain([hb_tag_t::default_language()]) {
            policy.charge(ColdOperation::Feature, 1, 0)?;
            let Some(script) = self.script_with(script_index, policy)? else {
                continue;
            };
            policy.charge(ColdOperation::Planning, 1, 0)?;
            policy.charge(
                ColdOperation::Feature,
                script.lang_sys_records().len() + 1,
                0,
            )?;
            if let Some(index) = script.lang_sys_index_for_tag(tag) {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    pub(crate) fn feature_with<P: ColdAdmission>(
        &self,
        index: u16,
        policy: &mut P,
    ) -> Result<Option<Feature<'a>>, P::Error> {
        let Some(list) = self.feature_list_with(policy)? else {
            return Ok(None);
        };
        // TaggedElement and FeatureRecord::feature each read the tag.
        // Then Offset16 and the four-byte Feature header are read.
        // FeatureParams is not resolved by this constructor.
        policy.charge(ColdOperation::TableRead, 4 + 4 + 2 + 4, 0)?;
        Ok(list.get(index).ok().map(|value| value.element))
    }

    pub(crate) fn feature_index_with<P: ColdAdmission>(
        &self,
        tag: hb_tag_t,
        policy: &mut P,
    ) -> Result<Option<u16>, P::Error> {
        let Some(list) = self.feature_list_with(policy)? else {
            return Ok(None);
        };
        for (index, record) in list.feature_records().iter().enumerate() {
            policy.charge(ColdOperation::Feature, 1, 0)?;
            if record.feature_tag() == tag {
                return Ok(Some(index as u16));
            }
        }
        Ok(None)
    }

    pub(crate) fn get_required_language_feature_with<P: ColdAdmission>(
        &self,
        script_index: u16,
        lang_index: Option<u16>,
        policy: &mut P,
    ) -> Result<Option<(u16, hb_tag_t)>, P::Error> {
        let Some(sys) = self.langsys_with(script_index, lang_index, policy)? else {
            return Ok(None);
        };
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        let index = sys.required_feature_index();
        if index == 0xffff {
            return Ok(None);
        }
        let Some(list) = self.feature_list_with(policy)? else {
            return Ok(None);
        };
        // Preserve feature_tag's full Feature parse, including malformed
        // child-table behavior; do not replace it with a record-only query.
        policy.charge(ColdOperation::TableRead, 4 + 4 + 2 + 4, 0)?;
        Ok(list.get(index).ok().map(|value| (index, value.tag)))
    }

    pub(crate) fn find_language_feature_with<P: ColdAdmission>(
        &self,
        script_index: u16,
        lang_index: Option<u16>,
        tag: hb_tag_t,
        policy: &mut P,
    ) -> Result<Option<u16>, P::Error> {
        let Some(sys) = self.langsys_with(script_index, lang_index, policy)? else {
            return Ok(None);
        };
        let Some(list) = self.feature_list_with(policy)? else {
            return Ok(None);
        };
        // At most one feature-index and one record visit per index. Counts
        // are u16 in the locked reader, so doubling is representable.
        policy.charge(ColdOperation::Planning, 1, 0)?;
        policy.charge(ColdOperation::Feature, 2 * sys.feature_indices().len(), 0)?;
        Ok(sys.feature_index_for_tag(&list, tag))
    }

    pub(crate) fn feature_substitution_with<P: ColdAdmission>(
        &self,
        variation_index: u32,
        feature_index: u16,
        policy: &mut P,
    ) -> Result<Option<Feature<'a>>, P::Error> {
        // Nullable Offset32 test + resolve and eight-byte variation header.
        policy.charge(ColdOperation::TableRead, 4 + 4 + 8, 0)?;
        let Some(variations) = self.feature_variations() else {
            return Ok(None);
        };
        policy.charge(ColdOperation::Feature, 1, 0)?;
        let Some(record) = variations
            .feature_variation_records()
            .get(variation_index as usize)
        else {
            return Ok(None);
        };
        // Nullable Offset32 test + resolve and six-byte substitution header.
        policy.charge(ColdOperation::TableRead, 4 + 4 + 6, 0)?;
        let Some(table) = record
            .feature_table_substitution(variations.offset_data())
            .and_then(Result::ok)
        else {
            return Ok(None);
        };
        policy.charge(ColdOperation::Planning, 1, 0)?;
        let records = table.substitutions();
        policy.charge(ColdOperation::Feature, records.len() + 1, 0)?;
        let Ok(index) =
            records.binary_search_by_key(&feature_index, |record| record.feature_index())
        else {
            return Ok(None);
        };
        // Offset32 and borrowed alternate Feature header.
        policy.charge(ColdOperation::TableRead, 4 + 4, 0)?;
        Ok(records
            .get(index)
            .and_then(|record| record.alternate_feature(table.offset_data()).ok()))
    }

    pub(crate) fn lookup_count_with<P: ColdAdmission>(
        &self,
        policy: &mut P,
    ) -> Result<u16, P::Error> {
        // Parent Offset16, LookupList header and accessor's count read.
        policy.charge(ColdOperation::TableRead, 2 + 2 + 2, 0)?;
        Ok(self.lookup_count())
    }
}
