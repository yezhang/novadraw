// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Admission for proven horizontal shaping paths with bounded output growth.

use super::cold_admission::{ColdAdmission, ColdOperation};
use super::hb_font_t;
use crate::hb::buffer::hb_buffer_t;
use crate::hb::ot::lookup::SubtableKind;
use crate::hb::ot_layout::TableIndex;
use crate::hb::ot_layout_gsubgpos::MatchPositions;
use crate::hb::ot_shape::{hb_ot_shape_context_t, shape_internal_with};
use crate::hb::unicode::{CharExt, GeneralCategory};
use crate::{
    script, BufferOperation, BufferOperationError, Direction, Feature, GlyphBuffer, ShapePlan,
    UnicodeBuffer,
};
use alloc::{collections::TryReserveError, vec::Vec};
use core::{convert::Infallible, fmt};
use read_fonts::tables::{
    gpos::{PairPosFormat1, PairPosFormat2, SinglePosFormat1, SinglePosFormat2},
    gsub::{LigatureSubstFormat1, MultipleSubstFormat1, SingleSubstFormat1, SingleSubstFormat2},
    layout::{ClassDef, CoverageTable, SequenceContextFormat3},
};
use read_fonts::{types::Tag, FontData, FontRead};

/// A shaping operation whose work is admitted before execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeOperation {
    /// Prepare or inspect a complete lookup cache with cold-operation admission.
    Cold(ColdOperation),
    /// Plan, reserve and initialize the two shaping arrays before lookup growth.
    Buffer(BufferOperation),
    /// Reserve and initialize lookup matching positions, including inline spill.
    Matching(BufferOperation),
    /// Read substitution/positioning table fields or coverage search records.
    TableRead,
    /// Inspect scalar state or borrowed collection headers.
    Planning,
    /// Check logical input records for a supported algorithm path.
    Input,
    /// Search the font table directory.
    Directory,
    /// Map a character, including cache reads and writes.
    Mapping,
    /// Query GDEF glyph/mark classes and their fixed property cache.
    GlyphProperties,
    /// Read generated Unicode property tables.
    Unicode,
    /// Visit shape-plan features or stages.
    Features,
    /// Visit or mutate glyph information records.
    Information,
    /// Visit or initialize position records.
    Positions,
    /// Read horizontal metrics.
    Metrics,
    /// Read or update fixed cache/digest elements.
    Cache,
    /// Prepay eventual buffer recovery or result ownership transfer.
    Recovery,
}

/// Logical input/element visits and whole new capacity slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShapeOperationCost {
    /// Element visits, including repeated passes.
    pub work: usize,
    /// New collection capacity slots, not bytes.
    pub allocation: usize,
}

/// Failure before a complete glyph result is returned.
#[derive(Debug)]
pub enum ShapeOperationError<E> {
    /// Caller rejection, preserving the original source.
    Admission(E),
    /// Lookup cache or buffer reservation failed before publication.
    Storage(TryReserveError),
    /// An operation cost cannot be represented.
    CostOverflow,
    /// The supplied plan does not match buffer direction or script.
    PlanMismatch,
    /// A supported lookup would exceed the existing shaping buffer limit.
    LengthLimit,
}

impl<E: fmt::Display> fmt::Display for ShapeOperationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "shaping operation rejected: {error}"),
            Self::Storage(error) => write!(f, "shaping storage reservation failed: {error}"),
            Self::CostOverflow => f.write_str("shaping operation cost overflow"),
            Self::PlanMismatch => f.write_str("buffer properties do not match shape plan"),
            Self::LengthLimit => f.write_str("shaping buffer length limit exceeded"),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for ShapeOperationError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::CostOverflow | Self::PlanMismatch | Self::LengthLimit => None,
        }
    }
}

pub(crate) enum Phase {
    Masks,
    Unicode,
    DottedCircle,
    Clusters,
    Direction,
    Substitute,
    Normalized,
    Position,
    Postprocess,
    Flags,
}

pub(crate) trait ShapeAdmission {
    type Error;
    fn before(&mut self, phase: Phase, ctx: &hb_ot_shape_context_t) -> Result<(), Self::Error>;
    fn before_group(
        &mut self,
        step: crate::hb::buffer::admission::GroupStep,
        buffer: &hb_buffer_t,
    ) -> Result<(), Self::Error>;
    fn before_normalization_sort(
        &mut self,
        step: crate::hb::buffer::admission::SortStep,
        buffer: &hb_buffer_t,
    ) -> Result<(), Self::Error>;
    fn before_normalization_buffer(
        &mut self,
        scalar_visits: usize,
        record_visits: usize,
    ) -> Result<(), Self::Error>;
    fn normalization_size(&mut self, base: usize, count: usize) -> Result<usize, Self::Error>;
    fn before_normalization_mapping(&mut self) -> Result<(), Self::Error>;
    fn before_normalization_unicode(&mut self, visits: usize) -> Result<(), Self::Error>;
    fn ensure_normalization_output(
        &mut self,
        buffer: &mut hb_buffer_t,
        size: usize,
    ) -> Result<bool, Self::Error>;
    fn before_lookup(
        &mut self,
        face: &hb_font_t,
        buffer: &mut hb_buffer_t,
        positions: &mut MatchPositions,
        saved_positions: &mut Option<MatchPositions>,
        table: TableIndex,
        index: u16,
    ) -> Result<(), Self::Error>;
}

pub(crate) struct OrdinaryShape;

impl ShapeAdmission for OrdinaryShape {
    type Error = Infallible;
    #[inline]
    fn before_group(
        &mut self,
        _: crate::hb::buffer::admission::GroupStep,
        _: &hb_buffer_t,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn before(&mut self, _: Phase, _: &hb_ot_shape_context_t) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn before_normalization_sort(
        &mut self,
        _: crate::hb::buffer::admission::SortStep,
        _: &hb_buffer_t,
    ) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn before_normalization_buffer(&mut self, _: usize, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn normalization_size(&mut self, base: usize, count: usize) -> Result<usize, Infallible> {
        Ok(base + count)
    }
    #[inline]
    fn before_normalization_mapping(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn before_normalization_unicode(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    #[inline]
    fn ensure_normalization_output(
        &mut self,
        buffer: &mut hb_buffer_t,
        size: usize,
    ) -> Result<bool, Infallible> {
        Ok(buffer.ensure(size))
    }
    #[inline]
    fn before_lookup(
        &mut self,
        _: &hb_font_t,
        _: &mut hb_buffer_t,
        _: &mut MatchPositions,
        _: &mut Option<MatchPositions>,
        _: TableIndex,
        _: u16,
    ) -> Result<(), Infallible> {
        Ok(())
    }
}

struct AdmittedShape<F> {
    admit: F,
    mapping_work: usize,
    non_ascii: usize,
    special_scalars: usize,
    normalization: SingletonNormalization,
    glyph_properties_work: usize,
    has_glyph_classes: bool,
}

#[derive(Default)]
struct SingletonNormalization {
    missing: bool,
    queries: usize,
    outputs: usize,
    terminal_b_misses: usize,
    fallback_mappings: usize,
}

impl<E, F: FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>> AdmittedShape<F> {
    fn charge(&mut self, op: ShapeOperation, work: usize) -> Result<(), ShapeOperationError<E>> {
        (self.admit)(
            op,
            ShapeOperationCost {
                work,
                allocation: 0,
            },
        )
        .map_err(ShapeOperationError::Admission)
    }

    fn pass(
        &mut self,
        op: ShapeOperation,
        count: usize,
        visits: usize,
    ) -> Result<(), ShapeOperationError<E>> {
        let work = count
            .checked_mul(visits)
            .ok_or(ShapeOperationError::CostOverflow)?;
        self.charge(op, work)
    }

    fn simple_scalar(&mut self, codepoint: u32) -> Result<Option<bool>, ShapeOperationError<E>> {
        if codepoint < 0x80 {
            return Ok(Some(false));
        }
        // These five categories take the early return in set_unicode_props.
        // Other categories require the singleton special-branch proof below.
        self.charge(ShapeOperation::Unicode, 4)?;
        let category = codepoint.general_category();
        self.charge(ShapeOperation::Planning, 1)?;
        let early = matches!(
            category,
            GeneralCategory::LOWERCASE_LETTER
                | GeneralCategory::UPPERCASE_LETTER
                | GeneralCategory::TITLECASE_LETTER
                | GeneralCategory::OTHER_LETTER
                | GeneralCategory::SPACE_SEPARATOR
        );
        if !early {
            // Inspect the mark category and the scalar branches that can
            // create continuations or fraction state in set_unicode_props.
            self.charge(ShapeOperation::Planning, 2)?;
            if category.is_mark()
                || matches!(codepoint, 0x1F1E6..=0x1F1FF | 0x1F3FB..=0x1F3FF
                    | 0xFF9E..=0xFF9F | 0x2044)
            {
                return Ok(None);
            }
        }
        self.charge(ShapeOperation::Unicode, 1)?;
        Ok((!codepoint.is_default_ignorable()).then_some(!early))
    }

    fn missing_singleton(
        &mut self,
        face: &hb_font_t,
        original: u32,
    ) -> Result<bool, ShapeOperationError<E>> {
        self.normalization.missing = true;
        let mut current = original;
        loop {
            // Fixed Hangul arithmetic, three generated index reads and at
            // most one decomposition payload. No allocation or callback.
            self.charge(ShapeOperation::Planning, 12)?;
            self.charge(ShapeOperation::Unicode, 4)?;
            self.normalization.queries = self
                .normalization
                .queries
                .checked_add(1)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let Some((a, b)) = crate::hb::unicode::decompose(current) else {
                break;
            };
            self.charge(ShapeOperation::Planning, 2)?;
            if b != 0 {
                // Ordinary decompose maps a, then b, before recursion or
                // output. A missing b terminates this attempt regardless
                // of a; neither scalar receives output Unicode properties.
                self.charge(ShapeOperation::Mapping, self.mapping_work)?;
                let _ = face.get_nominal_glyph(a);
                self.charge(ShapeOperation::Mapping, self.mapping_work)?;
                self.charge(ShapeOperation::Planning, 2)?;
                if face.get_nominal_glyph(b).is_some() {
                    // Actual two-character output still needs the
                    // mark/cluster/growth proof before entering pipeline.
                    return Ok(false);
                }
                self.normalization.terminal_b_misses = self
                    .normalization
                    .terminal_b_misses
                    .checked_add(1)
                    .ok_or(ShapeOperationError::CostOverflow)?;
                break;
            }
            if self.simple_scalar(a)?.is_none() {
                return Ok(false);
            }
            self.charge(ShapeOperation::Mapping, self.mapping_work)?;
            if face.get_nominal_glyph(a).is_some() {
                self.normalization.outputs = self
                    .normalization
                    .outputs
                    .checked_add(1)
                    .ok_or(ShapeOperationError::CostOverflow)?;
                return Ok(true);
            }
            // The immutable canonical decomposition table is acyclic.
            // Follow only a, as the ordinary shortest normalizer does.
            current = a;
        }
        // Failed decomposition falls back using ORIGINAL Unicode properties.
        self.charge(ShapeOperation::Planning, 2)?;
        self.charge(ShapeOperation::Unicode, 4)?;
        let space = original.general_category() == GeneralCategory::SPACE_SEPARATOR;
        // space_fallback is a fixed scalar match, with no generated-table
        // query or allocation. OGHAM SPACE MARK remains ordinary .notdef.
        self.charge(ShapeOperation::Planning, 4)?;
        let fallback = if space {
            (original.space_fallback() != crate::hb::unicode::hb_unicode_funcs_t::NOT_SPACE)
                .then_some(0x0020)
        } else {
            (original == 0x2011).then_some(0x2010)
        };
        if let Some(codepoint) = fallback {
            // A failed canonical chain does not replace this repeated query.
            // The original normalizer may then inspect buffer.invisible.
            self.charge(ShapeOperation::Mapping, self.mapping_work)?;
            self.charge(ShapeOperation::Planning, 2)?;
            let _ = face.get_nominal_glyph(codepoint);
            self.normalization.fallback_mappings = self
                .normalization
                .fallback_mappings
                .checked_add(1)
                .ok_or(ShapeOperationError::CostOverflow)?;
        }
        Ok(true)
    }

    fn supports(
        &mut self,
        face: &hb_font_t,
        plan: &ShapePlan,
        buffer: &UnicodeBuffer,
        features: &[Feature],
    ) -> Result<bool, ShapeOperationError<E>> {
        // Plan/buffer headers, callbacks, algorithm switches and coordinate span.
        self.charge(ShapeOperation::Planning, 32)?;
        if buffer.0.direction != plan.direction
            || buffer.0.script.unwrap_or(script::UNKNOWN) != plan.script.unwrap_or(script::UNKNOWN)
        {
            return Err(ShapeOperationError::PlanMismatch);
        }
        // This is an algorithm restriction, independent of font identity.
        // Simple scalar categories never create grapheme continuations.
        // Missing input additionally proves singleton normalization below.
        if !matches!(
            buffer.0.direction,
            Direction::LeftToRight | Direction::RightToLeft
        ) || !matches!(
            buffer.0.script,
            None | Some(script::LATIN | script::HAN | script::COMMON | script::UNKNOWN)
        ) || !face.coords().is_empty()
            || !buffer.0.successful
            || buffer.0.allocated_var_bits != 0
            || plan.shaper.preprocess_text.is_some()
            || plan.shaper.postprocess_glyphs.is_some()
            || plan.shaper.setup_masks.is_some()
            || plan.shaper.decompose.is_some()
            || plan.shaper.compose.is_some()
            || plan.shaper.reorder_marks.is_some()
            || !matches!(plan.shaper.normalization_preference,
                crate::hb::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_NONE
                | crate::hb::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_AUTO
                | crate::hb::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS)
            || plan.apply_kern
            || plan.apply_kerx
            || plan.apply_morx
            || plan.apply_trak
        {
            return Ok(false);
        }
        // No AAT data or external callback can run on this slice.
        // Directory searching is itself admitted.
        self.charge(ShapeOperation::Planning, 1)?;
        let directory_work = face.font.table_directory().table_records().len() + 3;
        for tag in [b"morx", b"kern", b"kerx", b"trak"] {
            self.charge(ShapeOperation::Directory, directory_work)?;
            if face.font.table_data(Tag::new(tag)).is_some() {
                return Ok(false);
            }
        }
        // Admit borrowed GDEF class headers before deriving query bounds.
        // Missing or invalid readers retain the ordinary class-zero behavior.
        self.charge(ShapeOperation::Planning, 6)?;
        self.glyph_properties_work = face.ot_tables.glyph_properties_work();
        self.has_glyph_classes = face.ot_tables.has_glyph_classes();
        let mut output_bound = buffer.len();
        // Initially lig_props is zero regardless of GDEF classification.
        // Single preserves it; Multiple either preserves a ligature base or
        // writes components with id zero and IS_LIG_BASE unset. Only ligating
        // can increase the component bound, and its storage uses four bits.
        // Zero lookup flags prohibit skipped marks, so ligate_input never
        // assigns nonzero attachment ids to glyphs between matched records.
        let mut component_bound = 1usize;
        for table in TableIndex::iter() {
            self.charge(ShapeOperation::Planning, 1)?;
            let stages = plan.ot_map.stages(table);
            self.charge(ShapeOperation::Features, stages.len())?;
            for (index, stage) in stages.iter().enumerate() {
                if stage.pause_func.is_some() {
                    return Ok(false);
                }
                let lookups = plan.ot_map.stage_lookups(table, index);
                self.charge(ShapeOperation::Features, lookups.len())?;
                if table == TableIndex::GPOS {
                    if let Some(gpos) = face.ot_tables.gpos.as_ref() {
                        for map in lookups {
                            let Some(lookup) =
                                gpos.lookups.get_with(&gpos.table, map.index, self)?
                            else {
                                continue;
                            };
                            self.charge(ShapeOperation::Planning, 4)?;
                            if lookup.is_subst || lookup.is_reverse() || lookup.props != 0 {
                                return Ok(false);
                            }
                            self.charge(ShapeOperation::Features, lookup.subtables.len())?;
                            for subtable in &lookup.subtables {
                                if !matches!(
                                    subtable.kind,
                                    SubtableKind::SinglePos1
                                        | SubtableKind::SinglePos2
                                        | SubtableKind::PairPos1
                                        | SubtableKind::PairPos2
                                ) {
                                    return Ok(false);
                                }
                                if matches!(
                                    subtable.kind,
                                    SubtableKind::PairPos1 | SubtableKind::PairPos2
                                ) {
                                    let Some(pair) = self.pair_position_plan(
                                        gpos.table.offset_data(),
                                        subtable.offset,
                                        subtable.kind,
                                    )?
                                    else {
                                        continue;
                                    };
                                    if pair.formats.iter().any(|format| format & !0x000f != 0)
                                        || !pair.address_valid
                                    {
                                        return Ok(false);
                                    }
                                    continue;
                                }
                                let Some((format, _)) = self.single_position_plan(
                                    gpos.table.offset_data(),
                                    subtable.offset,
                                    subtable.kind,
                                )?
                                else {
                                    continue;
                                };
                                // Scalar fields only: no device/variation lookup or
                                // reserved format bit can enter the shared kernel.
                                if format & !0x000f != 0 {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                    continue;
                }
                if let Some(gsub) = face.ot_tables.gsub.as_ref() {
                    for map in lookups {
                        let lookup = gsub.lookups.get_with(&gsub.table, map.index, self)?;
                        let Some(lookup) = lookup else { continue };
                        self.charge(ShapeOperation::Planning, 4)?;
                        if !lookup.is_subst || lookup.is_reverse() || lookup.props != 0 {
                            return Ok(false);
                        }
                        self.charge(ShapeOperation::Features, lookup.subtables.len())?;
                        let mut factor = 1;
                        let mut ligature_count = 1;
                        let mut contextual = false;
                        let mut context_count = 0;
                        let mut changes_length = false;
                        for subtable in &lookup.subtables {
                            if !matches!(
                                subtable.kind,
                                SubtableKind::SingleSubst1
                                    | SubtableKind::SingleSubst2
                                    | SubtableKind::MultipleSubst1
                                    | SubtableKind::LigatureSubst1
                                    | SubtableKind::ContextFormat3
                            ) {
                                return Ok(false);
                            }
                            if subtable.kind == SubtableKind::MultipleSubst1 {
                                changes_length = true;
                                let Some(maximum) = self
                                    .multiple_factor(gsub.table.offset_data(), subtable.offset)?
                                else {
                                    return Ok(false);
                                };
                                factor = factor.max(maximum);
                            }
                            if subtable.kind == SubtableKind::LigatureSubst1 {
                                changes_length = true;
                                ligature_count = ligature_count.max(
                                    self.ligature_plan(gsub.table.offset_data(), subtable.offset)?
                                        .maximum,
                                );
                            }
                            if subtable.kind == SubtableKind::ContextFormat3 {
                                contextual = true;
                                let Some(context) = self.context_plan(
                                    face,
                                    gsub.table.offset_data(),
                                    subtable.offset,
                                )?
                                else {
                                    return Ok(false);
                                };
                                context_count = context_count.max(context.count);
                            }
                        }
                        // move_to can rewind without shift_forward only while
                        // out_len == idx. Single/context-to-single preserve
                        // that invariant within this lookup; prior lookups
                        // have already synchronized their output.
                        if contextual && changes_length {
                            return Ok(false);
                        }
                        // match_input also sums all but the first contextual
                        // component even when the caller discards that sum.
                        if context_count.saturating_sub(1) * component_bound > usize::from(u8::MAX)
                        {
                            return Ok(false);
                        }
                        // match_input and ligate_input sum in u8. Reject before
                        // execution, rather than allowing a debug overflow or
                        // release wrap. Single/multiple cannot increase this
                        // bound, even when preserving earlier ligature props.
                        let components = component_bound * ligature_count;
                        if components > usize::from(u8::MAX) {
                            return Ok(false);
                        }
                        component_bound = component_bound.max(components.min(15));
                        if factor > 1 {
                            // Mirror enter() without changing buffer settings.
                            // Sequential lookups can expand an earlier result.
                            self.charge(ShapeOperation::Planning, 3)?;
                            let limit = buffer
                                .len()
                                .checked_mul(hb_buffer_t::MAX_LEN_FACTOR)
                                .map_or(hb_buffer_t::MAX_LEN_DEFAULT, |n| {
                                    n.max(hb_buffer_t::MAX_LEN_MIN)
                                });
                            let Some(next) = output_bound.checked_mul(factor) else {
                                return Ok(false);
                            };
                            if next > limit {
                                return Ok(false);
                            }
                            output_bound = next;
                        }
                    }
                }
            }
        }
        self.charge(ShapeOperation::Features, features.len())?;
        if features.iter().any(|feature| !feature.is_global()) {
            return Ok(false);
        }
        // Subtable/header inspection and the two encoding flags used to bound
        // MacRoman conversion and symbol fallback, before reading the plan.
        self.charge(ShapeOperation::Planning, 4)?;
        let Some(work) = face.charmap.general_mapping_work() else {
            return Ok(false);
        };
        self.mapping_work = work;
        self.charge(ShapeOperation::Input, buffer.len())?;
        // Prechecking nominal mapping does real font work, so each query is
        // charged before it runs. Repeated normalization queries are charged
        // again, even if this check has warmed the fixed atomic cache.
        for info in &buffer.0.info[..buffer.len()] {
            let Some(special) = self.simple_scalar(info.glyph_id)? else {
                return Ok(false);
            };
            self.special_scalars += usize::from(special);
            self.non_ascii += usize::from(info.glyph_id >= 0x80);
            self.charge(ShapeOperation::Mapping, self.mapping_work)?;
            if face.get_nominal_glyph(info.glyph_id).is_none()
                && !self.missing_singleton(face, info.glyph_id)?
            {
                return Ok(false);
            }
            if buffer.0.direction.is_backward() {
                // mirrored() reads at most four generated-table elements.
                self.charge(ShapeOperation::Unicode, 4)?;
                if let Some(mirror) = info.as_codepoint().mirrored() {
                    if self.simple_scalar(mirror)?.is_none() {
                        return Ok(false);
                    }
                    // A missing mirror is supported: rotate_chars retains
                    // the already-mapped original and applies the rtlm mask.
                    self.charge(ShapeOperation::Mapping, self.mapping_work)?;
                    let _ = face.has_glyph(mirror);
                }
            }
        }
        Ok(true)
    }

    fn multiple_factor(
        &mut self,
        data: FontData<'_>,
        offset: u32,
    ) -> Result<Option<usize>, ShapeOperationError<E>> {
        // Borrowed subtable/offset headers; generated readers only verify
        // ranges and counts here, without visiting the glyph payload.
        self.charge(ShapeOperation::Planning, 1)?;
        self.charge(ShapeOperation::TableRead, 10)?;
        let Some(table) = data
            .split_off(offset as usize)
            .and_then(|data| MultipleSubstFormat1::read(data).ok())
        else {
            return Ok(Some(1));
        };
        let sequences = table.sequences();
        let mut maximum = 1;
        for index in 0..sequences.len() {
            // One Offset16 and the Sequence glyph-count field (two bytes
            // each). Sequence::read verifies its borrowed payload range.
            self.charge(ShapeOperation::TableRead, 4)?;
            let Ok(sequence) = sequences.get(index) else {
                continue;
            };
            let count = sequence.substitute_glyph_ids().len();
            if count == 0 {
                return Ok(None);
            }
            maximum = maximum.max(count);
        }
        Ok(Some(maximum))
    }

    fn single_position_plan<'a>(
        &mut self,
        data: FontData<'a>,
        offset: u32,
        kind: SubtableKind,
    ) -> Result<Option<(u16, Option<CoverageTable<'a>>)>, ShapeOperationError<E>> {
        self.charge(ShapeOperation::Planning, 1)?;
        // Generated shape, scalar format/count, offset and coverage headers.
        // These readers borrow the value array; no value payload is scanned.
        self.charge(ShapeOperation::TableRead, 12)?;
        let Some(data) = data.split_off(offset as usize) else {
            return Ok(None);
        };
        // ValueFormat::from_raw drops reserved bits; admission must inspect
        // the original scalar before allowing the generated reader's kernel.
        let format = data.read_at::<u16>(4).unwrap_or(0);
        Ok(match kind {
            SubtableKind::SinglePos1 => SinglePosFormat1::read(data)
                .ok()
                .map(|t| (format, t.coverage().ok())),
            SubtableKind::SinglePos2 => SinglePosFormat2::read(data)
                .ok()
                .map(|t| (format, t.coverage().ok())),
            _ => None,
        })
    }

    fn before_position_lookup(
        &mut self,
        face: &hb_font_t,
        buffer: &hb_buffer_t,
        index: u16,
    ) -> Result<(), ShapeOperationError<E>> {
        let Some(gpos) = face.ot_tables.gpos.as_ref() else {
            return Ok(());
        };
        let Some(lookup) = gpos.lookups.get_with(&gpos.table, index, self)? else {
            return Ok(());
        };
        let n = buffer.len;
        self.charge(ShapeOperation::Planning, 40)?;
        self.charge(ShapeOperation::Cache, 6)?; // lookup/buffer digest intersection
        self.charge(ShapeOperation::Information, n)?; // forward skip/mask/property scan
        self.pass(ShapeOperation::Cache, n, 3)?; // lookup digest probe
        self.charge(ShapeOperation::Information, n)?; // check_glyph_property
        self.charge(ShapeOperation::Features, lookup.subtables.len())?;
        for subtable in &lookup.subtables {
            if matches!(
                subtable.kind,
                SubtableKind::PairPos1 | SubtableKind::PairPos2
            ) {
                let Some(pair) = self.pair_position_plan(
                    gpos.table.offset_data(),
                    subtable.offset,
                    subtable.kind,
                )?
                else {
                    continue;
                };
                self.charge(ShapeOperation::Features, n)?; // attempted subtables
                self.pass(ShapeOperation::Information, n, 2)?; // subtable/apply cur(0)
                self.pass(ShapeOperation::Cache, n, 3)?; // subtable digest
                let search = pair.coverage.as_ref().map_or(0, coverage_search_work);
                // Header, coverage, pair-set binary search or both class
                // searches, matrix address and each possible scalar read.
                let values = pair
                    .formats
                    .iter()
                    .map(|f| f.count_ones() as usize)
                    .sum::<usize>();
                self.pass(
                    ShapeOperation::TableRead,
                    n,
                    24 + search + pair.search_work + values,
                )?;
                self.pass(ShapeOperation::Positions, n, 2 + values * 2)?;
                // Full mode: one coverage and up to two class mapping caches,
                // each with a read and a write. Small/None do not use them.
                self.pass(ShapeOperation::Cache, n, 6)?;
                // Iterator new/reset/match_at visits its initial glyph and
                // next glyph's property, Unicode state, mask and match data.
                // ASCII + zero lookup flags + no GDEF guarantee no skips.
                self.pass(ShapeOperation::Information, n, 8)?;
                // At most two flag calls per success: the pair [idx,idx+2)
                // and, with record2, [idx,idx+3). Failure uses one concat.
                // Even nonmonotone/equal clusters stay inside these ranges.
                // Per call: minimum scan (3+2 endpoints), flag endpoints (2),
                // and at most three records' two comparisons plus mask RMW
                // (3*4), including the final monotone-loop comparison.
                self.pass(ShapeOperation::Information, n, 2 * (3 + 2 + 2 + 3 * 4 + 1))?;
                continue;
            }
            let Some((format, coverage)) = self.single_position_plan(
                gpos.table.offset_data(),
                subtable.offset,
                subtable.kind,
            )?
            else {
                continue;
            };
            self.charge(ShapeOperation::Features, n)?; // attempted subtables
            self.pass(ShapeOperation::Information, n, 2)?; // subtable/apply cur(0)
            self.pass(ShapeOperation::Cache, n, 3)?; // subtable digest
            let search = coverage.as_ref().map_or(0, coverage_search_work);
            let values = format.count_ones() as usize;
            // Header/offset/value format plus every possible scalar read.
            // Horizontal Y_ADVANCE skips its value; counting it is conservative.
            self.pass(ShapeOperation::TableRead, n, 12 + search + values)?;
            // Borrowing the record, then read-modify-write of each scalar.
            self.pass(ShapeOperation::Positions, n, 1 + values * 2)?;
        }
        // IN_PLACE keeps arrays and digest unchanged. SinglePos advances once;
        // PairPos consumes one or two records; neither grows or attaches.
        Ok(())
    }

    fn pair_position_plan<'a>(
        &mut self,
        data: FontData<'a>,
        offset: u32,
        kind: SubtableKind,
    ) -> Result<Option<PairPositionPlan<'a>>, ShapeOperationError<E>> {
        self.charge(ShapeOperation::Planning, 1)?;
        // Generated shape, formats, coverage and borrowed array headers.
        self.charge(ShapeOperation::TableRead, 24)?;
        let Some(data) = data.split_off(offset as usize) else {
            return Ok(None);
        };
        let formats = [
            data.read_at::<u16>(4).unwrap_or(0),
            data.read_at::<u16>(6).unwrap_or(0),
        ];
        Ok(match kind {
            SubtableKind::PairPos1 => {
                let Ok(table) = PairPosFormat1::read(data) else {
                    return Ok(None);
                };
                let mut maximum = 0usize;
                // Pair application searches the selected set with a manual
                // binary search. Read every set's count under admission to
                // bound any glyph's search, without scanning record payloads.
                for offset in table.pair_set_offsets() {
                    self.charge(ShapeOperation::TableRead, 2)?;
                    let count = data
                        .split_off(offset.get().to_u32() as usize)
                        .and_then(|d| d.read_at::<u16>(0).ok())
                        .unwrap_or(0) as usize;
                    maximum = maximum.max(count);
                }
                Some(PairPositionPlan {
                    formats,
                    coverage: table.coverage().ok(),
                    search_work: (usize::BITS - maximum.leading_zeros()) as usize,
                    address_valid: true,
                })
            }
            SubtableKind::PairPos2 => {
                let Ok(table) = PairPosFormat2::read(data) else {
                    return Ok(None);
                };
                // Both generated ClassDef headers are borrowed. Full/None
                // use ClassDef::get; Small uses ClassDefInfo::class.
                self.charge(ShapeOperation::TableRead, 12)?;
                let first = self.position_class_plan(table.class_def1().ok().as_ref())?;
                let second = self.position_class_plan(table.class_def2().ok().as_ref())?;
                let record_size = formats
                    .iter()
                    .map(|format| (format & 0xff).count_ones() as usize * 2)
                    .sum::<usize>();
                // The shared PairPos2 kernel computes the address using usize.
                // Reject invalid classes before that arithmetic, including on
                // 32-bit targets. Every class payload scan is admitted above.
                let address_valid = first.1 < usize::from(table.class1_count())
                    && second.1 < usize::from(table.class2_count())
                    && first
                        .1
                        .checked_mul(record_size)
                        .and_then(|v| v.checked_mul(usize::from(table.class2_count())))
                        .and_then(|v| {
                            second
                                .1
                                .checked_mul(record_size)
                                .and_then(|w| v.checked_add(w))
                        })
                        .and_then(|v| v.checked_add(16))
                        .and_then(|v| v.checked_add(record_size))
                        .is_some();
                Some(PairPositionPlan {
                    formats,
                    coverage: table.coverage().ok(),
                    search_work: first.0 + second.0,
                    address_valid,
                })
            }
            _ => None,
        })
    }

    fn position_class_plan(
        &mut self,
        class: Option<&ClassDef>,
    ) -> Result<(usize, usize), ShapeOperationError<E>> {
        self.charge(ShapeOperation::Planning, 2)?;
        let Some(class) = class else {
            return Ok((0, 0));
        };
        let maximum = match class {
            ClassDef::Format1(table) => {
                let values = table.class_value_array();
                self.charge(ShapeOperation::TableRead, values.len())?;
                values
                    .iter()
                    .map(|v| usize::from(v.get()))
                    .max()
                    .unwrap_or(0)
            }
            ClassDef::Format2(table) => {
                let records = table.class_range_records();
                self.charge(ShapeOperation::TableRead, records.len())?;
                records
                    .iter()
                    .map(|v| usize::from(v.class()))
                    .max()
                    .unwrap_or(0)
            }
        };
        Ok((class_search_work(class), maximum))
    }

    fn ligature_plan(
        &mut self,
        data: FontData<'_>,
        offset: u32,
    ) -> Result<LigaturePlan, ShapeOperationError<E>> {
        self.charge(ShapeOperation::Planning, 1)?;
        self.charge(ShapeOperation::TableRead, 10)?;
        let mut plan = LigaturePlan::default();
        let Some(table) = data
            .split_off(offset as usize)
            .and_then(|data| LigatureSubstFormat1::read(data).ok())
        else {
            return Ok(plan);
        };
        let sets = table.ligature_sets();
        for index in 0..sets.len() {
            self.charge(ShapeOperation::TableRead, 4)?; // set offset/count
            let Ok(set) = sets.get(index) else { continue };
            let ligatures = set.ligatures();
            for index in 0..ligatures.len() {
                self.charge(ShapeOperation::TableRead, 6)?; // offset/glyph/count
                plan.attempts = plan
                    .attempts
                    .checked_add(1)
                    .ok_or(ShapeOperationError::CostOverflow)?;
                let Ok(ligature) = ligatures.get(index) else {
                    continue;
                };
                let count = ligature.component_glyph_ids().len() + 1;
                // Larger patterns fail match_input before storage or iteration.
                if count <= crate::hb::ot_layout::MAX_CONTEXT_LENGTH {
                    plan.maximum = plan.maximum.max(count);
                    plan.components = plan
                        .components
                        .checked_add(count - 1)
                        .ok_or(ShapeOperationError::CostOverflow)?;
                }
            }
        }
        Ok(plan)
    }

    fn context_plan(
        &mut self,
        face: &hb_font_t,
        data: FontData<'_>,
        offset: u32,
    ) -> Result<Option<ContextPlan>, ShapeOperationError<E>> {
        self.charge(ShapeOperation::Planning, 1)?;
        self.charge(ShapeOperation::TableRead, 10)?;
        let Some(table) = data
            .split_off(offset as usize)
            .and_then(|data| SequenceContextFormat3::read(data).ok())
        else {
            return Ok(Some(ContextPlan::default()));
        };
        let count = table.coverages().len();
        // Larger contexts use a pre-reserved saved matching array. Growing or
        // recursive leaves still require their own storage/error boundary.
        if count == 0 || count > crate::hb::ot_layout::MAX_CONTEXT_LENGTH {
            return Ok(None);
        }
        let mut plan = ContextPlan {
            count,
            ..ContextPlan::default()
        };
        for index in 0..count {
            self.charge(ShapeOperation::TableRead, 6)?; // offset/format/count
            let search = table
                .coverages()
                .get(index)
                .ok()
                .as_ref()
                .map_or(0, coverage_search_work);
            plan.table_work += 6 + search;
        }
        let Some(gsub) = face.ot_tables.gsub.as_ref() else {
            return Ok(Some(plan));
        };
        for record in table.seq_lookup_records() {
            self.charge(ShapeOperation::TableRead, 4)?;
            plan.records += 1;
            // An invalid sequence index is skipped before lookup access.
            if usize::from(record.sequence_index.get()) >= count {
                continue;
            }
            let leaf = gsub
                .lookups
                .get_with(&gsub.table, record.lookup_list_index.get(), self)?;
            let Some(leaf) = leaf else { continue };
            self.charge(ShapeOperation::Planning, 4)?;
            if !leaf.is_subst || leaf.is_reverse() || leaf.props != 0 {
                return Ok(None);
            }
            self.charge(ShapeOperation::Features, leaf.subtables.len())?;
            for subtable in &leaf.subtables {
                if !matches!(
                    subtable.kind,
                    SubtableKind::SingleSubst1 | SubtableKind::SingleSubst2
                ) {
                    return Ok(None);
                }
                self.charge(ShapeOperation::TableRead, 10)?;
                let coverage = data.split_off(subtable.offset as usize).and_then(|data| {
                    if subtable.kind == SubtableKind::SingleSubst1 {
                        SingleSubstFormat1::read(data)
                            .ok()
                            .and_then(|table| table.coverage().ok())
                    } else {
                        SingleSubstFormat2::read(data)
                            .ok()
                            .and_then(|table| table.coverage().ok())
                    }
                });
                plan.leaf_subtables = plan
                    .leaf_subtables
                    .checked_add(1)
                    .ok_or(ShapeOperationError::CostOverflow)?;
                plan.leaf_tables = plan
                    .leaf_tables
                    .checked_add(11 + coverage.as_ref().map_or(0, coverage_search_work))
                    .ok_or(ShapeOperationError::CostOverflow)?;
            }
        }
        Ok(Some(plan))
    }

    fn context_work(
        &mut self,
        n: usize,
        plan: &ContextPlan,
        positions: &mut MatchPositions,
        saved_positions: &mut Option<MatchPositions>,
    ) -> Result<(), ShapeOperationError<E>> {
        self.matching_storage(positions, plan.count)?;
        if plan.count > positions.inline_size() || saved_positions.is_some() {
            self.matching_storage(
                saved_positions.get_or_insert_with(MatchPositions::new),
                plan.count,
            )?;
        }
        // Matching visits adjacent records: zero flags and mapped ASCII
        // prohibit glyph skipping. Preflight bounds the component sum.
        self.pass(ShapeOperation::Information, n, plan.count * 6 + 2)?;
        self.pass(ShapeOperation::TableRead, n, plan.table_work)?;
        self.pass(ShapeOperation::TableRead, n, plan.records * 4)?;
        self.pass(ShapeOperation::TableRead, n, plan.leaf_tables)?;
        self.pass(ShapeOperation::Features, n, plan.leaf_subtables)?;
        let cache_work = plan
            .leaf_subtables
            .checked_mul(3)
            .and_then(|work| work.checked_add(plan.records * 5))
            .ok_or(ShapeOperationError::CostOverflow)?;
        self.pass(ShapeOperation::Cache, n, cache_work)?;
        let information_work = plan
            .leaf_subtables
            .checked_add(plan.records * 6)
            .ok_or(ShapeOperationError::CostOverflow)?;
        self.pass(ShapeOperation::Information, n, information_work)?;
        // Initialize match_input indices, convert to output indexing, clone
        // and restore the matching slice for each nested leaf. Heap-backed
        // contexts copy into pre-reserved storage and swap it back.
        self.pass(
            ShapeOperation::Matching(BufferOperation::Append),
            n,
            plan.count * (4 + plan.records * 2),
        )?;
        self.pass(ShapeOperation::Planning, n, plan.records * 40)?;
        // Every record and the final move traverse at most count glyphs.
        // No length-changing subtable runs inside this lookup, hence no
        // shift_forward, separate-output switch or glyph allocation.
        self.pass(
            ShapeOperation::Information,
            n,
            plan.count * 2 * (plan.records + 1),
        )?;
        // Success/failure unsafe flags: bounded range minimum, compare/write.
        self.pass(ShapeOperation::Information, n, plan.count * 3)?;
        Ok(())
    }

    fn matching_storage(
        &mut self,
        positions: &mut MatchPositions,
        count: usize,
    ) -> Result<(), ShapeOperationError<E>> {
        (self.admit)(
            ShapeOperation::Matching(BufferOperation::Planning),
            ShapeOperationCost {
                work: 2,
                allocation: 0,
            },
        )
        .map_err(ShapeOperationError::Admission)?;
        if count > positions.capacity() {
            // Use Vec's fallible exact allocation to retain its original
            // TryReserveError. count exceeds inline capacity, so from_vec
            // transfers the allocation without an additional allocation.
            (self.admit)(
                ShapeOperation::Matching(BufferOperation::Storage),
                ShapeOperationCost {
                    work: positions.len() * 2,
                    allocation: count,
                },
            )
            .map_err(ShapeOperationError::Admission)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(ShapeOperationError::Storage)?;
            (self.admit)(
                ShapeOperation::Matching(BufferOperation::Initialize),
                ShapeOperationCost {
                    work: count - positions.len(),
                    allocation: 0,
                },
            )
            .map_err(ShapeOperationError::Admission)?;
            values.extend_from_slice(positions);
            values.resize(count, 0);
            *positions = MatchPositions::from_vec(values);
        } else if count > positions.len() {
            (self.admit)(
                ShapeOperation::Matching(BufferOperation::Initialize),
                ShapeOperationCost {
                    work: count - positions.len(),
                    allocation: 0,
                },
            )
            .map_err(ShapeOperationError::Admission)?;
            positions.resize(count, 0);
        }
        Ok(())
    }
}

#[derive(Default)]
struct LigaturePlan {
    maximum: usize,
    attempts: usize,
    components: usize,
}

impl<E, F: FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>> ColdAdmission
    for AdmittedShape<F>
{
    type Error = ShapeOperationError<E>;
    const ADMITTED: bool = true;

    fn charge(
        &mut self,
        operation: ColdOperation,
        work: usize,
        allocation: usize,
    ) -> Result<(), Self::Error> {
        (self.admit)(
            ShapeOperation::Cold(operation),
            ShapeOperationCost { work, allocation },
        )
        .map_err(ShapeOperationError::Admission)
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Error> {
        ColdAdmission::charge(self, ColdOperation::Planning, 2, 0)?;
        let required = values
            .len()
            .checked_add(count)
            .ok_or(ShapeOperationError::CostOverflow)?;
        if required > values.capacity() {
            ColdAdmission::charge(self, ColdOperation::Storage, values.len(), required)?;
            values
                .try_reserve_exact(count)
                .map_err(ShapeOperationError::Storage)?;
        }
        Ok(())
    }
}

fn coverage_search_work(coverage: &CoverageTable) -> usize {
    let (count, record_reads) = match coverage {
        CoverageTable::Format1(table) => (table.glyph_array().len(), 1),
        CoverageTable::Format2(table) => (table.range_records().len(), 3),
    };
    // Rust binary_search[_by] compares at most bit_length(n)+1 times.
    (usize::BITS - count.leading_zeros()) as usize * record_reads + record_reads
}

struct PairPositionPlan<'a> {
    formats: [u16; 2],
    coverage: Option<CoverageTable<'a>>,
    search_work: usize,
    address_valid: bool,
}

fn class_search_work(class: &ClassDef) -> usize {
    match class {
        ClassDef::Format1(_) => 4, // start/count and indexed class read
        ClassDef::Format2(table) => {
            let count = table.class_range_records().len();
            // Small compares both range endpoints; Full/None compare start
            // then read start/end/class of the selected range.
            (usize::BITS - count.leading_zeros()) as usize * 2 + 2 + 3
        }
    }
}

impl<E, F: FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>> ShapeAdmission
    for AdmittedShape<F>
{
    type Error = ShapeOperationError<E>;
    fn before_group(
        &mut self,
        step: crate::hb::buffer::admission::GroupStep,
        buffer: &hb_buffer_t,
    ) -> Result<(), Self::Error> {
        crate::hb::buffer::admission::admit_group_step(step, buffer, &mut self.admit)
    }
    fn before_normalization_sort(
        &mut self,
        step: crate::hb::buffer::admission::SortStep,
        buffer: &hb_buffer_t,
    ) -> Result<(), Self::Error> {
        crate::hb::buffer::admission::admit_sort_step(step, buffer, &mut self.admit)
    }
    fn before_normalization_buffer(
        &mut self,
        scalar_visits: usize,
        record_visits: usize,
    ) -> Result<(), Self::Error> {
        self.charge(ShapeOperation::Planning, scalar_visits)?;
        self.charge(ShapeOperation::Information, record_visits)
    }
    fn normalization_size(&mut self, base: usize, count: usize) -> Result<usize, Self::Error> {
        base.checked_add(count)
            .ok_or(ShapeOperationError::CostOverflow)
    }
    fn before_normalization_mapping(&mut self) -> Result<(), Self::Error> {
        self.charge(ShapeOperation::Mapping, self.mapping_work)
    }
    fn before_normalization_unicode(&mut self, visits: usize) -> Result<(), Self::Error> {
        self.charge(ShapeOperation::Unicode, visits)
    }
    fn ensure_normalization_output(
        &mut self,
        buffer: &mut hb_buffer_t,
        size: usize,
    ) -> Result<bool, Self::Error> {
        buffer
            .ensure_admitted(size, &mut |op, cost| {
                (self.admit)(
                    ShapeOperation::Buffer(op),
                    ShapeOperationCost {
                        work: cost.work,
                        allocation: cost.allocation,
                    },
                )
            })
            .map_err(|error| match error {
                BufferOperationError::Admission(source) => ShapeOperationError::Admission(source),
                BufferOperationError::Storage { source, .. } => {
                    ShapeOperationError::Storage(source)
                }
                BufferOperationError::CostOverflow => ShapeOperationError::CostOverflow,
                BufferOperationError::LengthLimit => ShapeOperationError::LengthLimit,
            })?;
        Ok(true)
    }

    fn before_lookup(
        &mut self,
        face: &hb_font_t,
        buffer: &mut hb_buffer_t,
        positions: &mut MatchPositions,
        saved_positions: &mut Option<MatchPositions>,
        table: TableIndex,
        index: u16,
    ) -> Result<(), Self::Error> {
        if table == TableIndex::GPOS {
            return self.before_position_lookup(face, buffer, index);
        }
        let Some(gsub) = face.ot_tables.gsub.as_ref() else {
            return Ok(());
        };
        // Preflight has already admitted construction; this is still a charged
        // cache access, independent of whether the previous access was cold.
        let Some(lookup) = gsub.lookups.get_with(&gsub.table, index, self)? else {
            return Ok(());
        };
        self.charge(ShapeOperation::Planning, 40)?;
        self.charge(ShapeOperation::Cache, 6)?; // three masks from each digest
        let n = buffer.len;
        // Single substitution advances exactly once and never switches output
        // arrays. Admit each possible pass, including failed coverage attempts.
        self.charge(ShapeOperation::Information, n)?; // forward skip scan
        self.pass(ShapeOperation::Cache, n, 3)?; // lookup digest
        self.charge(ShapeOperation::Information, n)?; // current glyph read
        self.charge(ShapeOperation::Information, n)?; // property read
        self.charge(ShapeOperation::Information, n)?; // property write
        self.pass(ShapeOperation::Cache, n, 3)?; // new glyph digest
        self.charge(ShapeOperation::Information, n)?; // output glyph write
        self.charge(ShapeOperation::Features, lookup.subtables.len())?;
        let mut maximum = 1;
        let mut multiple = false;
        let mut matching = 0;
        let mut ligature = false;
        for subtable in &lookup.subtables {
            self.charge(ShapeOperation::Planning, 1)?;
            self.charge(ShapeOperation::TableRead, 10)?;
            let data = FontData::new(gsub.table.offset_data().as_bytes())
                .split_off(subtable.offset as usize);
            let Some(data) = data else { continue };
            // Fixed generated headers, coverage offset/format/count. Invalid
            // readers terminate the same attempted subtable in the kernel.
            let coverage = match subtable.kind {
                SubtableKind::SingleSubst1 => SingleSubstFormat1::read(data)
                    .ok()
                    .and_then(|t| t.coverage().ok()),
                SubtableKind::SingleSubst2 => SingleSubstFormat2::read(data)
                    .ok()
                    .and_then(|t| t.coverage().ok()),
                SubtableKind::MultipleSubst1 => {
                    multiple = true;
                    let factor = self
                        .multiple_factor(gsub.table.offset_data(), subtable.offset)?
                        .expect("preflight rejected deletion in immutable font data");
                    maximum = maximum.max(factor);
                    MultipleSubstFormat1::read(data)
                        .ok()
                        .and_then(|t| t.coverage().ok())
                }
                SubtableKind::LigatureSubst1 => {
                    ligature = true;
                    let plan = self.ligature_plan(gsub.table.offset_data(), subtable.offset)?;
                    matching = matching.max(plan.maximum);
                    // All sets may be attempted at every input position.
                    // Generated set offset/count and second-glyph fast-path
                    // probe; three seconds-digest masks are read per probe.
                    self.pass(ShapeOperation::TableRead, n, 4)?;
                    self.pass(ShapeOperation::Information, n, 4)?;
                    self.pass(ShapeOperation::Cache, n, 3)?;
                    let attempts_work = plan
                        .attempts
                        .checked_mul(6)
                        .ok_or(ShapeOperationError::CostOverflow)?;
                    let component_work = plan
                        .components
                        .checked_mul(2)
                        .ok_or(ShapeOperationError::CostOverflow)?;
                    self.pass(ShapeOperation::TableRead, n, attempts_work)?;
                    self.pass(ShapeOperation::TableRead, n, component_work)?;
                    // Per attempt: reset/first record, matcher property/mask/
                    // glyph checks, component properties and matching writes.
                    self.pass(
                        ShapeOperation::Information,
                        n,
                        plan.attempts
                            .checked_mul(2)
                            .ok_or(ShapeOperationError::CostOverflow)?,
                    )?;
                    self.pass(
                        ShapeOperation::Information,
                        n,
                        plan.components
                            .checked_mul(6)
                            .ok_or(ShapeOperationError::CostOverflow)?,
                    )?;
                    self.pass(
                        ShapeOperation::Matching(BufferOperation::Append),
                        n,
                        plan.components
                            .checked_add(plan.attempts)
                            .ok_or(ShapeOperationError::CostOverflow)?,
                    )?;
                    // Failed attempts flag a contiguous range; fast-path
                    // mismatches can flag it once more. No ASCII glyph skips.
                    self.pass(
                        ShapeOperation::Information,
                        n,
                        component_work
                            .checked_add(2)
                            .ok_or(ShapeOperationError::CostOverflow)?,
                    )?;
                    // Full mapping cache can read and write its atomic slot.
                    self.pass(ShapeOperation::Cache, n, 2)?;
                    LigatureSubstFormat1::read(data)
                        .ok()
                        .and_then(|t| t.coverage().ok())
                }
                SubtableKind::ContextFormat3 => {
                    let plan = self
                        .context_plan(face, gsub.table.offset_data(), subtable.offset)?
                        .expect("preflight checked immutable contextual lookup");
                    self.context_work(n, &plan, positions, saved_positions)?;
                    if self.has_glyph_classes {
                        // Every allowed leaf record can replace at most once
                        // at each outer position, even when positions repeat.
                        let queries = n
                            .checked_mul(plan.records)
                            .ok_or(ShapeOperationError::CostOverflow)?;
                        self.pass(
                            ShapeOperation::GlyphProperties,
                            queries,
                            self.glyph_properties_work,
                        )?;
                    }
                    // All input coverage searches are included in context_work.
                    None
                }
                _ => unreachable!("preflight rejected unsupported substitution"),
            };
            self.charge(ShapeOperation::Features, n)?; // subtable attempts
            self.charge(ShapeOperation::Information, n)?; // each subtable's cur(0)
            self.pass(ShapeOperation::Cache, n, 3)?; // subtable digest
                                                     // Each attempt repeats its header/coverage reads and search. The
                                                     // delta or substitute-array access contributes one final read.
            let search = coverage.as_ref().map_or(0, coverage_search_work);
            self.pass(ShapeOperation::TableRead, n, 10 + search + 1)?;
            if subtable.kind == SubtableKind::MultipleSubst1 {
                // Repeated sequence-offset/count reads on each attempt,
                // followed by one glyph payload read per output component.
                self.pass(ShapeOperation::TableRead, n, 4)?;
                self.pass(ShapeOperation::TableRead, n, maximum)?;
            }
        }
        if self.has_glyph_classes {
            // A direct lookup publishes at most maximum outputs per input.
            // Alternatives stop on first success; unsuccessful attempts do
            // not call set_glyph_class. Context leaves were prepaid above.
            self.pass(
                ShapeOperation::GlyphProperties,
                n,
                maximum
                    .checked_mul(self.glyph_properties_work)
                    .ok_or(ShapeOperationError::CostOverflow)?,
            )?;
        }
        if ligature {
            self.matching_storage(positions, matching)?;
            // Successful matches can merge equal-cluster suffixes and an
            // already-emitted prefix. A mixed lookup's multiple substitution
            // can expand that prefix before a later ligature candidate.
            // Upstream does not promise disjoint ranges across successes.
            let span = n
                .checked_mul(maximum)
                .ok_or(ShapeOperationError::CostOverflow)?;
            let scans = n
                .checked_mul(span)
                .ok_or(ShapeOperationError::CostOverflow)?;
            self.charge(ShapeOperation::Information, scans)?; // minimum cluster
            self.pass(ShapeOperation::Information, scans, 2)?; // suffix comparisons
            self.pass(ShapeOperation::Information, scans, 4)?; // prefix read/set_cluster
            self.pass(ShapeOperation::Information, scans, 2)?; // input set_cluster
            self.pass(ShapeOperation::Information, scans, 3)?; // nonmonotone unsafe flags
            self.pass(ShapeOperation::Information, scans, 3)?; // trailing ligature components
            self.pass(
                ShapeOperation::Matching(BufferOperation::Append),
                n,
                matching * 2,
            )?;
            self.pass(ShapeOperation::Information, n, matching * 4 + 12)?;
            // A ligature only consumes records, but replacing it after an
            // earlier contraction can copy the emitted prefix into pos.
            self.pass(ShapeOperation::Information, n, 2)?;
            self.pass(ShapeOperation::Information, n, 2)?;
        }
        if multiple {
            let output = n
                .checked_mul(maximum)
                .ok_or(ShapeOperationError::CostOverflow)?;
            // Switching arrays copies one already-emitted prefix of at most n.
            self.pass(ShapeOperation::Information, n, 2)?;
            // Unmatched records (and mixed single subtables) can copy to output.
            self.pass(ShapeOperation::Information, n, 2)?;
            self.charge(ShapeOperation::Information, n)?; // class guess
            self.charge(ShapeOperation::Information, n)?; // ligature identity
            self.charge(ShapeOperation::Information, output)?; // component property read
            self.charge(ShapeOperation::Information, output)?; // component property write
            self.charge(ShapeOperation::Information, output)?; // glyph class read
            self.charge(ShapeOperation::Information, output)?; // glyph class write
            self.pass(ShapeOperation::Cache, output, 3)?; // output digest
            self.charge(ShapeOperation::Information, output)?; // output source read
            self.charge(ShapeOperation::Information, output)?; // output record copy
            self.charge(ShapeOperation::Information, output)?; // output glyph write
            buffer
                .ensure_admitted(output, &mut |op, cost| {
                    (self.admit)(
                        ShapeOperation::Buffer(op),
                        ShapeOperationCost {
                            work: cost.work,
                            allocation: cost.allocation,
                        },
                    )
                })
                .map_err(|error| match error {
                    BufferOperationError::Admission(source) => {
                        ShapeOperationError::Admission(source)
                    }
                    BufferOperationError::Storage { source, .. } => {
                        ShapeOperationError::Storage(source)
                    }
                    BufferOperationError::CostOverflow => ShapeOperationError::CostOverflow,
                    BufferOperationError::LengthLimit => ShapeOperationError::LengthLimit,
                })?;
        }
        Ok(())
    }

    fn before(&mut self, phase: Phase, ctx: &hb_ot_shape_context_t) -> Result<(), Self::Error> {
        let n = ctx.buffer.len;
        // Each pass has its own admission. Costs below describe the shared
        // upstream kernel under supports(), rather than a per-character
        // estimate for arbitrary shaping. Lookup growth is separately admitted.
        match phase {
            Phase::Masks => self.charge(ShapeOperation::Information, n)?,
            Phase::Unicode => {
                self.charge(ShapeOperation::Information, n)?;
                // _hb_ucd_gc has four fixed generated-table accesses.
                self.pass(ShapeOperation::Unicode, n, 4)?;
                // Non-ASCII additionally checks default-ignorable and reads
                // the stored category before the simple-category early return.
                if self.non_ascii != 0 {
                    self.charge(ShapeOperation::Unicode, self.non_ascii)?;
                    self.charge(ShapeOperation::Information, self.non_ascii)?;
                    self.charge(ShapeOperation::Planning, self.non_ascii)?;
                }
                // Non-early-return scalars inspect glyph_id in the modifier,
                // RI, halfwidth/tag and fraction branches, plus the stored
                // ZWJ bit. No continuation branch can run under supports().
                // Prepay the split headers and scalar branch checks too.
                self.pass(ShapeOperation::Information, self.special_scalars, 5)?;
                self.pass(ShapeOperation::Planning, self.special_scalars, 8)?;
            }
            Phase::DottedCircle | Phase::Clusters => {
                self.charge(ShapeOperation::Planning, 8)?;
            }
            Phase::Direction => {
                self.charge(ShapeOperation::Planning, 8)?;
                // Stored continuation comparisons, repeated merges and
                // reversals are admitted by the shared grouping kernel.
            }
            Phase::Substitute => {
                self.charge(ShapeOperation::Planning, 16)?;
                if ctx.target_direction.is_backward() {
                    self.pass(ShapeOperation::Information, n, 2)?;
                    self.pass(ShapeOperation::Unicode, n, 4)?;
                    // Prepay has_glyph for every possible mirrored character.
                    // This is separate from the nominal normalizer query.
                    self.pass(ShapeOperation::Mapping, n, self.mapping_work)?;
                }
                // Normalization now admits its actual operations in the
                // shared kernel. Do not also prepay singleton estimates.
            }
            Phase::Normalized => {
                // These passes execute AFTER normalization; use its live
                // length and the current fully initialized array lengths.
                self.charge(ShapeOperation::Planning, 16)?;
                self.charge(ShapeOperation::Features, ctx.features.len())?;
                if ctx.plan.fallback_mark_positioning {
                    self.charge(ShapeOperation::Information, n)?;
                }
                // map_glyphs_fast revisits both info and its output alias.
                self.charge(ShapeOperation::Information, n)?;
                self.charge(ShapeOperation::Information, n)?;
                // Initial glyph_props runs even without a valid ClassDef.
                self.charge(ShapeOperation::Information, n)?;
                self.pass(
                    ShapeOperation::GlyphProperties,
                    n,
                    self.glyph_properties_work,
                )?;
                if ctx.plan.fallback_glyph_classes {
                    self.charge(ShapeOperation::Information, n)?;
                }
                // update_digest visits the entire initialized info array,
                // including spare records, and three inline digest masks.
                self.charge(ShapeOperation::Information, ctx.buffer.info.len())?;
                self.pass(ShapeOperation::Cache, ctx.buffer.info.len(), 3)?;
                self.charge(
                    ShapeOperation::Features,
                    ctx.plan.ot_map.stages(TableIndex::GSUB).len(),
                )?;
            }
            Phase::Position => {
                self.charge(ShapeOperation::Planning, 16)?;
                self.charge(
                    ShapeOperation::Features,
                    ctx.plan.ot_map.stages(TableIndex::GPOS).len(),
                )?;
                // clear_positions initializes the whole array, not just n.
                self.charge(ShapeOperation::Positions, ctx.buffer.pos.len())?;
                // populate_advance_widths also visits both complete arrays.
                let count = ctx.buffer.info.len().min(ctx.buffer.pos.len());
                self.charge(ShapeOperation::Information, count)?;
                self.charge(ShapeOperation::Positions, count)?;
                // get(index) or last(): at most one metric record is read.
                self.charge(ShapeOperation::Metrics, count)?;
                if ctx.buffer.scratch_flags
                    & crate::hb::buffer::HB_BUFFER_SCRATCH_FLAG_HAS_SPACE_FALLBACK
                    != 0
                {
                    // GSUB may have expanded the output: use the live n.
                    // Each zip reads category, ligated and fallback type.
                    self.pass(ShapeOperation::Information, n, 4)?;
                    self.pass(ShapeOperation::Positions, n, 3)?;
                    self.pass(ShapeOperation::Planning, n, 32)?;
                    // FIGURE searches at most ten digits; PUNCTUATION at
                    // most two characters. These are mutually exclusive
                    // branches; their sum is a conservative per-record bound.
                    let figure_work = self
                        .mapping_work
                        .checked_mul(10)
                        .ok_or(ShapeOperationError::CostOverflow)?;
                    let punctuation_work = self
                        .mapping_work
                        .checked_mul(2)
                        .ok_or(ShapeOperationError::CostOverflow)?;
                    self.pass(ShapeOperation::Mapping, n, figure_work)?;
                    self.pass(ShapeOperation::Mapping, n, punctuation_work)?;
                    self.pass(ShapeOperation::Metrics, n, 2)?;
                }
                self.charge(ShapeOperation::Positions, n)?; // GPOS::position_start
                if ctx.plan.zero_marks {
                    self.charge(ShapeOperation::Information, n)?;
                    self.charge(ShapeOperation::Positions, n)?;
                    if self.has_glyph_classes {
                        // GDEF can classify a Unicode letter as mark. Two
                        // advance writes plus, when enabled, four offset/
                        // advance reads and two offset writes per glyph.
                        let writes = if ctx.plan.adjust_mark_positioning_when_zeroing
                            && ctx.buffer.direction.is_forward()
                        {
                            8
                        } else {
                            2
                        };
                        self.pass(ShapeOperation::Positions, n, writes)?;
                    }
                }
                // No GPOS attachments or ignorables exist on the ASCII path.
                if ctx.plan.fallback_mark_positioning {
                    // Copied ASCII Unicode properties remain non-mark and
                    // non-hidden. Fallback groups remain singleton even when
                    // MultipleSubst repeats their source cluster identity.
                    self.charge(ShapeOperation::Information, n.saturating_sub(1))?;
                }
                if ctx.buffer.direction.is_backward() && n > 1 {
                    self.pass(ShapeOperation::Information, n, 2)?;
                    self.pass(ShapeOperation::Positions, n, 2)?;
                }
            }
            Phase::Postprocess => self.charge(ShapeOperation::Planning, 8)?,
            Phase::Flags => {
                // group_end compares adjacent records. Each record belongs
                // to one group; propagation reads and writes each group once.
                self.pass(ShapeOperation::Information, n, 2)?;
                self.charge(ShapeOperation::Information, n)?;
                self.charge(ShapeOperation::Information, n)?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct ContextPlan {
    count: usize,
    records: usize,
    table_work: usize,
    leaf_tables: usize,
    leaf_subtables: usize,
}

impl hb_font_t<'_> {
    /// Shapes a proven horizontal path after operation admission.
    ///
    /// Supports non-mark, non-ignorable singleton scalars without special
    /// continuation/fraction state. Missing mappings support canonical
    /// one-to-one replacement, .notdef, space or nonbreaking-hyphen fallback.
    /// LTR/RTL Latin/Han/Common/unknown or absent script, default
    /// coordinates, global features, and layout-free fonts or selected GSUB
    /// single, positive-length multiple or bounded-component ligature
    /// substitutions, or bounded Format 3 contexts calling only single
    /// substitutions, and GPOS scalar single/pair positioning, with zero lookup flags.
    /// Pair class values must fit their matrix dimensions and target addresses;
    /// raw value formats may contain only the four placement/advance bits.
    /// Multiple lookup growth is checked against the existing buffer limit
    /// before shaping, and both arrays are reserved before each lookup.
    /// GDEF glyph/mark classification uses bounded borrowed ClassDef readers;
    /// mark filtering remains unsupported through the zero lookup flag rule.
    /// Other GPOS algorithms, device/variation positioning and AAT tables
    /// remain unsupported. Lookup caches are separately admitted.
    /// These structural restrictions apply to any font; there is no identity
    /// whitelist. Other scalar categories, complex scripts, absent nominal
    /// mappings requiring nonsingleton normalization and unknown lookups
    /// return `Ok(None)` before shaping, preserving the logical input.
    ///
    /// Takes a borrowed buffer so callers never lose its recovery handle.
    /// Failure during shaping clears it and retains its allocated capacity.
    /// Preflight rejection preserves input. Success transfers glyph storage
    /// into the returned result and leaves a new empty input buffer. Completed
    /// charges and complete cache entries remain spent/available. No ordinary
    /// fallback runs. This does not admit font/plan construction or glyph export.
    pub fn try_shape_with_plan_admitted<E>(
        &self,
        plan: &ShapePlan,
        buffer: &mut UnicodeBuffer,
        features: &[Feature],
        admit: impl FnMut(ShapeOperation, ShapeOperationCost) -> Result<(), E>,
    ) -> Result<Option<GlyphBuffer>, ShapeOperationError<E>> {
        let mut policy = AdmittedShape {
            admit,
            mapping_work: 0,
            non_ascii: 0,
            special_scalars: 0,
            normalization: SingletonNormalization::default(),
            glyph_properties_work: 0,
            has_glyph_classes: false,
        };
        if !policy.supports(self, plan, buffer, features)? {
            return Ok(None);
        }
        // POD arrays have no element destructors. clear drops a possible
        // language allocation and resets scalar state; prepay that cleanup
        // before entering the pipeline. UnicodeBuffer::new uses only inline
        // state (including the native/portable empty glyph set).
        let recovery_work = 32usize
            .checked_add(
                buffer
                    .0
                    .language
                    .as_ref()
                    .map_or(0, |language| language.as_str().len()),
            )
            .ok_or(ShapeOperationError::CostOverflow)?;
        policy.charge(ShapeOperation::Recovery, recovery_work)?;
        let target_direction = buffer.0.direction;
        buffer.0.enter();
        let result = if buffer.len() > 0 {
            shape_internal_with(
                &mut hb_ot_shape_context_t {
                    plan,
                    face: self,
                    buffer: &mut buffer.0,
                    target_direction,
                    features,
                },
                &mut policy,
            )
        } else {
            Ok(())
        };
        buffer.0.leave();
        if let Err(error) = result {
            // Interrupted shaping may still own Unicode/normalizer/GSUB
            // variable slots. Upstream clear() assumes complete shaping and
            // does not reset them; release the slots before ordinary reuse.
            buffer.0.allocated_var_bits = 0;
            buffer.clear();
            return Err(error);
        }
        let result = core::mem::replace(buffer, UnicodeBuffer::new());
        Ok(Some(GlyphBuffer(result.0)))
    }
}

#[cfg(test)]
#[path = "normalization_kernel_tests.rs"]
mod normalization_kernel_tests;

#[cfg(test)]
#[path = "dotted_circle_kernel_tests.rs"]
mod dotted_circle_kernel_tests;

#[cfg(test)]
#[path = "grapheme_kernel_tests.rs"]
mod grapheme_kernel_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn missing_spaces_use_original_properties_and_invisible_without_growth() {
        for space_present in [false, true] {
            // Format 6 omits SPACE when first==33. A format-0 glyph-zero
            // entry is Some(0), and would defeat the invisible fallback.
            let first: u16 = if space_present { 32 } else { 33 };
            let count = 123 - first;
            let mut cmap: Vec<u8> = [0u16, 1, 3, 1]
                .into_iter()
                .flat_map(u16::to_be_bytes)
                .collect();
            cmap.extend(12u32.to_be_bytes());
            cmap.extend(
                [6u16, 10 + 2 * count, 0, first, count]
                    .into_iter()
                    .flat_map(u16::to_be_bytes),
            );
            cmap.extend((first..123).flat_map(u16::to_be_bytes));
            let mut bytes = Vec::from(&b"\0\x01\0\0\0\x01\0\0\0\0\0\0cmap\0\0\0\0\0\0\0\x1c"[..]);
            bytes.extend((cmap.len() as u32).to_be_bytes());
            bytes.extend(cmap);
            let font = crate::FontRef::new(&bytes).unwrap();
            let data = crate::ShaperData::new(&font);
            let shaper = data.shaper(&font).build();
            for invisible in [None, Some(read_fonts::types::GlyphId::new(66))] {
                for text in [
                    "\u{2007}",
                    "\u{2008}",
                    "\u{202f}",
                    "\u{1680}",
                    "\u{2000}\u{2001}\u{2011}",
                ] {
                    let plan = ShapePlan::new(&shaper, Direction::LeftToRight, None, None, &[]);
                    let make = || {
                        let mut buffer = UnicodeBuffer::new();
                        buffer.push_str(text);
                        buffer.set_direction(Direction::LeftToRight);
                        buffer.0.invisible = invisible;
                        buffer
                    };
                    let ordinary = shaper.shape_with_plan(&plan, make(), &[]);
                    let admitted = shaper
                        .try_shape_with_plan_admitted(&plan, &mut make(), &[], |_, _| {
                            Ok::<_, ()>(())
                        })
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        alloc::format!("{:?}", ordinary.glyph_infos()),
                        alloc::format!("{:?}", admitted.glyph_infos())
                    );
                    assert_eq!(
                        alloc::format!("{:?}", ordinary.glyph_positions()),
                        alloc::format!("{:?}", admitted.glyph_positions())
                    );
                    assert_eq!(admitted.len(), text.chars().count());
                    if text == "\u{2007}" {
                        assert_eq!(
                            admitted.glyph_infos()[0].glyph_id,
                            if space_present {
                                32
                            } else {
                                invisible.map_or(0, |g| g.to_u32())
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn frozen_singleton_decomposition_chains_are_acyclic() {
        // Supports follows only b==0 edges. Exhaust the locked generated
        // Unicode data, rather than imposing a guessed recursion limit.
        for original in 0..=0x10ffff {
            let mut current = original;
            let mut seen = Vec::new();
            while let Some((a, b)) = crate::hb::unicode::decompose(current) {
                if b != 0 {
                    break;
                }
                assert!(!seen.contains(&current), "canonical cycle at {original:x}");
                seen.push(current);
                current = a;
            }
        }
    }

    #[test]
    fn matching_storage_inline_spill_reuse_and_rejection() {
        let mut positions = MatchPositions::from_elem(7, 1);
        let mut costs = Vec::new();
        let mut policy = AdmittedShape {
            admit: |op, cost| {
                costs.push((op, cost));
                Ok::<_, ()>(())
            },
            mapping_work: 0,
            non_ascii: 0,
            special_scalars: 0,
            normalization: SingletonNormalization::default(),
            glyph_properties_work: 0,
            has_glyph_classes: false,
        };
        policy.matching_storage(&mut positions, 8).unwrap();
        assert!(!positions.spilled());
        assert_eq!(positions[0], 7);
        policy.matching_storage(&mut positions, 9).unwrap();
        assert!(positions.spilled());
        assert_eq!(positions.len(), 9);
        assert_eq!(positions[0], 7);
        policy.matching_storage(&mut positions, 9).unwrap();
        assert_eq!(costs.iter().map(|(_, c)| c.allocation).sum::<usize>(), 9);
        assert_eq!(
            costs
                .iter()
                .find(|(op, _)| *op == ShapeOperation::Matching(BufferOperation::Storage))
                .unwrap()
                .1
                .work,
            16
        );
        for reject in 0..3 {
            let mut positions = MatchPositions::from_elem(7, 1);
            let mut calls = 0;
            let mut policy = AdmittedShape {
                admit: |_, _| {
                    let index = calls;
                    calls += 1;
                    if index == reject {
                        Err(index)
                    } else {
                        Ok(())
                    }
                },
                mapping_work: 0,
                non_ascii: 0,
                special_scalars: 0,
                normalization: SingletonNormalization::default(),
                glyph_properties_work: 0,
                has_glyph_classes: false,
            };
            assert!(matches!(policy.matching_storage(&mut positions, 9),
                Err(ShapeOperationError::Admission(i)) if i == reject));
            assert_eq!(positions.as_slice(), &[7]);
            assert!(!positions.spilled());
            assert_eq!(calls, reject + 1);
        }
    }

    #[test]
    fn cost_overflow_does_not_invoke_admission() {
        let mut calls = 0;
        let mut policy = AdmittedShape {
            admit: |_, _| {
                calls += 1;
                Ok::<_, Infallible>(())
            },
            mapping_work: 0,
            non_ascii: 0,
            special_scalars: 0,
            normalization: SingletonNormalization::default(),
            glyph_properties_work: 0,
            has_glyph_classes: false,
        };
        assert!(matches!(
            policy.pass(ShapeOperation::Mapping, usize::MAX, 2),
            Err(ShapeOperationError::CostOverflow)
        ));
        assert_eq!(calls, 0);
    }

    #[test]
    fn logical_preflight_and_pipeline_rejection_have_distinct_recovery() {
        // One format-0 mapping covering ASCII, in a layout-free sfnt.
        let mut bytes =
            Vec::from(&b"\0\x01\0\0\0\x01\0\0\0\0\0\0cmap\0\0\0\0\0\0\0\x1c\0\0\x01\x12"[..]);
        bytes.extend_from_slice(b"\0\0\0\x01\0\x03\0\x01\0\0\0\x0c\0\0\x01\x06\0\0");
        bytes.extend(0..=255u8);
        let font = crate::FontRef::new(&bytes).unwrap();
        let data = crate::ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let plan = ShapePlan::new(
            &shaper,
            Direction::LeftToRight,
            Some(script::LATIN),
            None,
            &[],
        );
        let input = || {
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str("abc");
            buffer.set_direction(Direction::LeftToRight);
            buffer.set_script(script::LATIN);
            buffer
        };
        let mut buffer = input();
        let capacity = (buffer.0.info.capacity(), buffer.0.pos.capacity());
        let error = shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |op, _| {
                if op == ShapeOperation::Unicode {
                    Err("reject")
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(matches!(error, ShapeOperationError::Admission("reject")));
        assert_eq!(buffer.0.allocated_var_bits, 0);
        assert_eq!(
            buffer.0.max_len,
            super::super::super::buffer::hb_buffer_t::MAX_LEN_DEFAULT
        );
        assert_eq!(
            buffer.0.max_ops,
            super::super::super::buffer::hb_buffer_t::MAX_OPS_DEFAULT
        );
        assert_eq!(
            capacity,
            (buffer.0.info.capacity(), buffer.0.pos.capacity())
        );
        assert!(buffer.is_empty());
        buffer.push_str("next");
        buffer.set_direction(Direction::LeftToRight);
        buffer.set_script(script::LATIN);
        assert_eq!(shaper.shape_with_plan(&plan, buffer, &[]).len(), 4);

        let mut buffer = input();
        assert!(shaper
            .try_shape_with_plan_admitted(&plan, &mut buffer, &[], |_, _| Err("preflight"))
            .is_err());
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.0.allocated_var_bits, 0);
    }
}
