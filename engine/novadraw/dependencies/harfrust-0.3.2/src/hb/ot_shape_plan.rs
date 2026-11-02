use alloc::boxed::Box;
use core::any::Any;
use smallvec::SmallVec;

use crate::hb::common::HB_FEATURE_GLOBAL_END;
use crate::hb::common::HB_FEATURE_GLOBAL_START;
use crate::ShaperInstance;
use super::face::cold_admission::{self, AdmittedCold, ColdAdmission, ColdOperation, ColdOperationCost, ColdOperationError};
use alloc::vec::Vec;
use read_fonts::types::Tag;

use super::aat::map::*;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shaper::*;
use super::{hb_font_t, hb_mask_t, Direction, Feature, Language, Script};

/// A reusable plan for shaping a text buffer.
pub struct hb_ot_shape_plan_t {
    pub(crate) direction: Direction,
    pub(crate) script: Option<Script>,
    pub(crate) language: Option<Language>,
    pub(crate) shaper: &'static hb_ot_shaper_t,
    pub(crate) ot_map: hb_ot_map_t,
    pub(crate) aat_map: AatMap,
    pub(crate) data: Option<Box<dyn Any + Send + Sync>>,

    pub(crate) frac_mask: hb_mask_t,
    pub(crate) numr_mask: hb_mask_t,
    pub(crate) dnom_mask: hb_mask_t,
    pub(crate) rtlm_mask: hb_mask_t,
    pub(crate) kern_mask: hb_mask_t,

    pub(crate) requested_kerning: bool,
    pub(crate) has_frac: bool,
    pub(crate) has_vert: bool,
    pub(crate) has_gpos_mark: bool,
    pub(crate) zero_marks: bool,
    pub(crate) fallback_glyph_classes: bool,
    pub(crate) fallback_mark_positioning: bool,
    pub(crate) adjust_mark_positioning_when_zeroing: bool,

    pub(crate) apply_gpos: bool,
    pub(crate) apply_fallback_kern: bool,
    pub(crate) apply_kern: bool,
    pub(crate) apply_kerx: bool,
    pub(crate) apply_morx: bool,
    pub(crate) apply_trak: bool,

    pub(crate) user_features: SmallVec<[Feature; 4]>,
}

impl hb_ot_shape_plan_t {
    /// Builds a plan with operation admission before scanning or growing storage.
    ///
    /// Currently supports plans with no GSUB/GPOS tables, no language, no
    /// tracking table, and no AAT or script-specific plan callbacks.
    /// Unknown paths and invalid directions return `Ok(None)` before those
    /// operations execute. This does not admit shaping or shaper configuration.
    pub fn try_new_admitted<E>(
        face: &hb_font_t, direction: Direction, script: Option<Script>,
        language: Option<&Language>, user_features: &[Feature],
        admit: impl FnMut(ColdOperation, ColdOperationCost) -> Result<(), E>,
    ) -> Result<Option<Self>, ColdOperationError<E>> {
        let policy = &mut AdmittedCold(admit);
        policy.charge(ColdOperation::Planning, 1, 0)?;
        if direction == Direction::Invalid {
            return Ok(None);
        }
        let Some(mut planner) = hb_ot_shape_planner_t::new_with(
            face, direction, script, language, policy,
        )? else {
            return Ok(None);
        };
        // trak parsing is not yet proved; do not invoke its FontRead reader.
        cold_admission::directory(&face.font, policy)?;
        if face.font.table_data(Tag::new(b"trak")).is_some() {
            return Ok(None);
        }
        planner.collect_features_with(user_features, policy)?;
        planner.compile_with(user_features, policy).map(Some)
    }
    /// Returns a plan that can be used for shaping any buffer with the
    /// provided properties.
    pub fn new(
        face: &hb_font_t,
        direction: Direction,
        script: Option<Script>,
        language: Option<&Language>,
        user_features: &[Feature],
    ) -> Self {
        assert_ne!(
            direction,
            Direction::Invalid,
            "Direction must not be Invalid"
        );
        let mut planner = hb_ot_shape_planner_t::new(face, direction, script, language);
        planner.collect_features(user_features);
        planner.compile(user_features)
    }

    pub(crate) fn data<T: 'static>(&self) -> &T {
        self.data.as_ref().unwrap().downcast_ref().unwrap()
    }

    /// The direction of the text.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// The script of the text.
    pub fn script(&self) -> Option<Script> {
        self.script
    }

    /// The language of the text.
    pub fn language(&self) -> Option<&Language> {
        self.language.as_ref()
    }
}

pub(crate) fn features_with<P: ColdAdmission>(
    features: &[Feature], policy: &mut P,
) -> Result<SmallVec<[Feature; 4]>, P::Error> {
    if !P::ADMITTED {
        return Ok(features.into());
    }
    policy.charge(ColdOperation::Planning, 1, 0)?;
    if features.len() <= 4 {
        // Inline copy and SmallVec's eventual element-drop visits.
        policy.charge(ColdOperation::Initialize, features.len(), 0)?;
        policy.charge(ColdOperation::Initialize, features.len(), 0)?;
        return Ok(features.into());
    }
    let mut values = Vec::new();
    policy.reserve(&mut values, features.len())?;
    policy.charge(ColdOperation::Initialize, features.len(), 0)?;
    policy.charge(ColdOperation::Initialize, features.len(), 0)?;
    values.extend_from_slice(features);
    // For capacity > inline size, SmallVec transfers the Vec allocation.
    Ok(SmallVec::from_vec(values))
}

/// A key used for selecting a shape plan.
pub struct ShapePlanKey<'a> {
    script: Option<Script>,
    direction: Direction,
    language: Option<&'a Language>,
    feature_variations: [Option<u32>; 2],
    features: &'a [Feature],
}

impl<'a> ShapePlanKey<'a> {
    /// Creates a new shape plan key with the given script and direction.
    pub fn new(script: Option<Script>, direction: Direction) -> Self {
        Self {
            script,
            direction,
            language: None,
            feature_variations: [None; 2],
            features: &[],
        }
    }

    /// Sets the language to use for this shape plan key.
    pub fn language(mut self, language: Option<&'a Language>) -> Self {
        self.language = language;
        self
    }

    /// Sets the instance to use for this shape plan key.
    pub fn instance(mut self, instance: Option<&ShaperInstance>) -> Self {
        self.feature_variations = instance
            .map(|instance| instance.feature_variations)
            .unwrap_or_default();
        self
    }

    /// Sets the features to use for this shape plan key.
    pub fn features(mut self, features: &'a [Feature]) -> Self {
        self.features = features;
        self
    }

    /// Returns true if this key is a match for the given shape plan.
    pub fn matches(&self, plan: &hb_ot_shape_plan_t) -> bool {
        self.script == plan.script
            && self.direction == plan.direction
            && self.language == plan.language.as_ref()
            && self.feature_variations == *plan.ot_map.feature_variations()
            && features_equivalent(self.features, &plan.user_features)
    }
}

fn features_equivalent(features_a: &[Feature], features_b: &[Feature]) -> bool {
    if features_a.len() != features_b.len() {
        return false;
    }
    for (a, b) in features_a.iter().zip(features_b) {
        if a.tag != b.tag
            || a.value != b.value
            || (a.start == HB_FEATURE_GLOBAL_START && a.end == HB_FEATURE_GLOBAL_END)
                != (b.start == HB_FEATURE_GLOBAL_START && b.end == HB_FEATURE_GLOBAL_END)
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::hb_ot_shape_plan_t;

    #[test]
    fn test_shape_plan_is_send_and_sync() {
        fn ensure_send_and_sync<T: Send + Sync>() {}
        ensure_send_and_sync::<hb_ot_shape_plan_t>();
    }
}
