// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Prove empty AAT cache inputs before any variable-size reader runs.

use crate::hb::face::cold_admission::{directory, ColdAdmission, ColdOperation};
use read_fonts::{types::Tag, FontData, FontRef};

pub(super) fn empty_caches<P: ColdAdmission>(
    font: &FontRef,
    policy: &mut P,
) -> Result<bool, P::Error> {
    for tag in [Tag::new(b"morx"), Tag::new(b"kern"), Tag::new(b"kerx")] {
        directory(font, policy)?;
        let Some(data) = font.table_data(tag) else {
            continue;
        };
        if !empty_table(data, tag, policy)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn empty_table<P: ColdAdmission>(
    data: FontData<'_>,
    tag: Tag,
    policy: &mut P,
) -> Result<bool, P::Error> {
        // FontData is a borrowed slice. Reads below inspect scalar headers;
        // they never invoke FontRead or VarSize::total_len_for_count.
        let count = if tag == Tag::new(b"kern") {
            policy.charge(ColdOperation::TableRead, 2, 0)?;
            let Ok(version) = data.read_at::<u16>(0) else {
                return Ok(false);
            };
            if version == 0 {
                policy.charge(ColdOperation::TableRead, 2, 0)?;
                data.read_at::<u16>(2).map(u32::from)
            } else {
                policy.charge(ColdOperation::TableRead, 4, 0)?;
                data.read_at::<u32>(4)
            }
        } else {
            policy.charge(ColdOperation::TableRead, 4, 0)?;
            data.read_at::<u32>(4)
        };
        // Nonempty and truncated headers remain unsupported. No
        // subtable reader, glyph collection or candidate is published.
        Ok(count == Ok(0))
}
