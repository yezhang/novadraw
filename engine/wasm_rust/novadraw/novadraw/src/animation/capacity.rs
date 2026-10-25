use super::{
    AnimationBudgetKind, AnimationError, DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
    DEFAULT_MAX_ACTIVE_ANIMATIONS,
};

/// Runtime limits for timeline ownership and candidate sampling.
///
/// Active tracks stay reserved until their complete owner terminates. Staging is independent:
/// preparing a replacement never borrows the old owner's reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationLimits {
    /// Maximum number of active timeline owners.
    pub active_animations: usize,
    /// Maximum number of tracks reserved by active owners.
    pub active_tracks: usize,
    /// Maximum number of candidate tracks prepared by one admission or retarget.
    pub staging_tracks: usize,
}

impl Default for AnimationLimits {
    fn default() -> Self {
        Self {
            active_animations: DEFAULT_MAX_ACTIVE_ANIMATIONS,
            active_tracks: DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
            staging_tracks: DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
        }
    }
}

/// Pull diagnostics for live timeline reservations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnimationReservations {
    /// Number of owners holding active capacity.
    pub active_animations: usize,
    /// Sum of the track bounds reserved at admission.
    pub active_tracks: usize,
}

impl AnimationLimits {
    pub(crate) fn validate_live(self, live: AnimationReservations) -> Result<(), AnimationError> {
        self.validate_replacement(live, AnimationReservations::default(), 0, false)
    }

    pub(crate) fn validate_staging(self, tracks: usize) -> Result<(), AnimationError> {
        if tracks > self.staging_tracks {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::StagingTracks,
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_replacement(
        self,
        live: AnimationReservations,
        retired: AnimationReservations,
        new_tracks: usize,
        new_owner: bool,
    ) -> Result<(), AnimationError> {
        let owners = live
            .active_animations
            .checked_sub(retired.active_animations)
            .and_then(|value| value.checked_add(usize::from(new_owner)))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveAnimations,
            ))?;
        let tracks = live
            .active_tracks
            .checked_sub(retired.active_tracks)
            .and_then(|value| value.checked_add(new_tracks))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ))?;
        if owners > self.active_animations {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveAnimations,
            ));
        }
        if tracks > self.active_tracks {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ));
        }
        Ok(())
    }
}
