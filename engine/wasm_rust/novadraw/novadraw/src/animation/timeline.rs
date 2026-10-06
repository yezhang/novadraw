use std::{collections::HashMap, time::Duration};

use crate::identity::AnimationChannelId;

use super::{
    AnimationBudgetKind, AnimationChannel, AnimationError, AnimationValue,
    DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS, InterruptionPolicy, SuspensionPolicy,
    service::ChannelState,
    value::{Motion, PreparedMotion, duration_micros},
};

/// Direction behavior for successive finite repeat cycles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepeatBehavior {
    /// Every cycle starts from the original first sample.
    Restart,
    /// Odd cycles reverse both track order and motion direction.
    Reverse,
}

/// A finite, composable set of typed animation tracks.
pub struct AnimationPlan {
    pub(crate) tracks: Vec<PlanTrack>,
    pub(crate) duration_micros: u64,
    pub(crate) interruption: InterruptionPolicy,
    pub(crate) suspension: SuspensionPolicy,
}

impl AnimationPlan {
    /// Creates a one-track plan.
    pub fn track<V: AnimationValue>(
        channel: AnimationChannel<V>,
        motion: Motion<V>,
    ) -> Result<Self, AnimationError> {
        let duration_micros = motion.duration_micros();
        Ok(Self {
            tracks: vec![PlanTrack {
                start_micros: 0,
                duration_micros,
                spec: Box::new(TypedTrackSpec {
                    channel: channel.id(),
                    motion,
                }),
            }],
            duration_micros,
            interruption: InterruptionPolicy::Replace,
            suspension: SuspensionPolicy::Advance,
        })
    }

    /// Runs child plans concurrently.
    pub fn parallel(plans: Vec<Self>) -> Result<Self, AnimationError> {
        compose_with_offsets(plans, |_| Ok(0))
    }

    /// Runs child plans in stable input order.
    pub fn sequence(plans: Vec<Self>) -> Result<Self, AnimationError> {
        if plans.is_empty() {
            return Err(AnimationError::EmptyPlan);
        }
        let mut cursor = 0_u64;
        let mut offsets = Vec::with_capacity(plans.len());
        for plan in &plans {
            offsets.push(cursor);
            cursor = cursor
                .checked_add(plan.duration_micros)
                .ok_or(AnimationError::TimeOverflow)?;
        }
        compose_with_explicit_offsets(plans, offsets)
    }

    /// Starts child plans at stable, evenly spaced offsets.
    pub fn stagger(interval: Duration, plans: Vec<Self>) -> Result<Self, AnimationError> {
        let interval = duration_micros(interval)?;
        compose_with_offsets(plans, |index| {
            u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(interval))
                .ok_or(AnimationError::TimeOverflow)
        })
    }

    /// Delays a complete child plan.
    pub fn delay(duration: Duration, plan: Self) -> Result<Self, AnimationError> {
        let offset = duration_micros(duration)?;
        compose_with_explicit_offsets(vec![plan], vec![offset])
    }

    /// Repeats a finite plan a fixed number of times.
    pub fn repeat(
        count: u32,
        behavior: RepeatBehavior,
        plan: Self,
    ) -> Result<Self, AnimationError> {
        if count == 0 || plan.duration_micros == 0 {
            return Err(AnimationError::InvalidRepeat);
        }
        let count = usize::try_from(count).map_err(|_| AnimationError::InvalidRepeat)?;
        let repeated_tracks = plan
            .tracks
            .len()
            .checked_mul(count)
            .ok_or(AnimationError::TimeOverflow)?;
        if repeated_tracks > DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ));
        }
        let mut cycles = Vec::with_capacity(count);
        for index in 0..count {
            cycles.push(if behavior == RepeatBehavior::Reverse && index % 2 == 1 {
                plan.reversed()?
            } else {
                plan.clone()
            });
        }
        Self::sequence(cycles)
    }

    /// Sets channel ownership behavior for admission.
    pub fn with_interruption(mut self, interruption: InterruptionPolicy) -> Self {
        self.interruption = interruption;
        self
    }

    /// Sets behavior while the target or rendering surface is suspended.
    pub fn with_suspension(mut self, suspension: SuspensionPolicy) -> Self {
        self.suspension = suspension;
        self
    }

    /// Returns the finite timeline duration.
    pub fn duration(&self) -> Duration {
        Duration::from_micros(self.duration_micros)
    }

    fn reversed(&self) -> Result<Self, AnimationError> {
        let mut tracks = Vec::with_capacity(self.tracks.len());
        for track in self.tracks.iter().rev() {
            let end = track
                .start_micros
                .checked_add(track.duration_micros)
                .ok_or(AnimationError::TimeOverflow)?;
            tracks.push(PlanTrack {
                start_micros: self
                    .duration_micros
                    .checked_sub(end)
                    .ok_or(AnimationError::InvalidComposition)?,
                duration_micros: track.duration_micros,
                spec: track.spec.reversed_box()?,
            });
        }
        Ok(Self {
            tracks,
            duration_micros: self.duration_micros,
            interruption: self.interruption,
            suspension: self.suspension,
        })
    }
}

impl Clone for AnimationPlan {
    fn clone(&self) -> Self {
        Self {
            tracks: self.tracks.clone(),
            duration_micros: self.duration_micros,
            interruption: self.interruption,
            suspension: self.suspension,
        }
    }
}

fn compose_with_offsets(
    plans: Vec<AnimationPlan>,
    mut offset: impl FnMut(usize) -> Result<u64, AnimationError>,
) -> Result<AnimationPlan, AnimationError> {
    if plans.is_empty() {
        return Err(AnimationError::EmptyPlan);
    }
    let mut offsets = Vec::with_capacity(plans.len());
    for index in 0..plans.len() {
        offsets.push(offset(index)?);
    }
    compose_with_explicit_offsets(plans, offsets)
}

fn compose_with_explicit_offsets(
    plans: Vec<AnimationPlan>,
    offsets: Vec<u64>,
) -> Result<AnimationPlan, AnimationError> {
    let mut tracks = Vec::new();
    let mut duration_micros = 0_u64;
    for (plan, offset) in plans.into_iter().zip(offsets) {
        duration_micros = duration_micros.max(
            offset
                .checked_add(plan.duration_micros)
                .ok_or(AnimationError::TimeOverflow)?,
        );
        for mut track in plan.tracks {
            track.start_micros = track
                .start_micros
                .checked_add(offset)
                .ok_or(AnimationError::TimeOverflow)?;
            tracks.push(track);
        }
    }
    validate_track_intervals(&tracks)?;
    Ok(AnimationPlan {
        tracks,
        duration_micros,
        interruption: InterruptionPolicy::Replace,
        suspension: SuspensionPolicy::Advance,
    })
}

fn validate_track_intervals(tracks: &[PlanTrack]) -> Result<(), AnimationError> {
    let mut by_channel: HashMap<AnimationChannelId, Vec<(u64, u64)>> = HashMap::new();
    for track in tracks {
        let end = track
            .start_micros
            .checked_add(track.duration_micros)
            .ok_or(AnimationError::TimeOverflow)?;
        by_channel
            .entry(track.spec.channel())
            .or_default()
            .push((track.start_micros, end));
    }
    for intervals in by_channel.values_mut() {
        intervals.sort_unstable();
        for pair in intervals.windows(2) {
            if pair[1].0 < pair[0].1 {
                return Err(AnimationError::OverlappingTracks);
            }
        }
    }
    Ok(())
}

pub(crate) struct PlanTrack {
    pub(crate) start_micros: u64,
    pub(crate) duration_micros: u64,
    pub(crate) spec: Box<dyn ErasedTrackSpec>,
}

impl Clone for PlanTrack {
    fn clone(&self) -> Self {
        Self {
            start_micros: self.start_micros,
            duration_micros: self.duration_micros,
            spec: self.spec.clone_box(),
        }
    }
}

pub(crate) trait ErasedTrackSpec: Send {
    fn channel(&self) -> AnimationChannelId;

    fn clone_box(&self) -> Box<dyn ErasedTrackSpec>;

    fn reversed_box(&self) -> Result<Box<dyn ErasedTrackSpec>, AnimationError>;

    fn terminal_matches(&self, channel: &ChannelState) -> Result<bool, AnimationError>;

    fn prepare(
        self: Box<Self>,
        channel: &ChannelState,
    ) -> Result<Box<dyn PreparedTrack>, AnimationError>;
}

#[derive(Clone)]
struct TypedTrackSpec<V> {
    channel: AnimationChannelId,
    motion: Motion<V>,
}

impl<V: AnimationValue> ErasedTrackSpec for TypedTrackSpec<V> {
    fn channel(&self) -> AnimationChannelId {
        self.channel
    }

    fn clone_box(&self) -> Box<dyn ErasedTrackSpec> {
        Box::new(self.clone())
    }

    fn reversed_box(&self) -> Result<Box<dyn ErasedTrackSpec>, AnimationError> {
        Ok(Box::new(Self {
            channel: self.channel,
            motion: self.motion.reversed()?,
        }))
    }

    fn terminal_matches(&self, channel: &ChannelState) -> Result<bool, AnimationError> {
        Ok(self.motion.terminal_value() == channel.committed::<V>()?)
    }

    fn prepare(
        self: Box<Self>,
        channel: &ChannelState,
    ) -> Result<Box<dyn PreparedTrack>, AnimationError> {
        let current = channel.effective::<V>()?.clone();
        let duration = self.motion.duration_micros();
        let prepared = TypedPreparedTrack {
            motion: self.motion.prepare(current),
        };
        for elapsed in [0, duration / 2, duration] {
            prepared.validate_sample(elapsed, channel)?;
        }
        Ok(Box::new(prepared))
    }
}

pub(crate) trait PreparedTrack: Send {
    fn validate_sample(
        &self,
        elapsed_micros: u64,
        channel: &ChannelState,
    ) -> Result<(), AnimationError>;

    fn sample(
        &self,
        elapsed_micros: u64,
        channel: &mut ChannelState,
    ) -> Result<bool, AnimationError>;
}

struct TypedPreparedTrack<V> {
    motion: PreparedMotion<V>,
}

impl<V: AnimationValue> PreparedTrack for TypedPreparedTrack<V> {
    fn validate_sample(
        &self,
        elapsed_micros: u64,
        channel: &ChannelState,
    ) -> Result<(), AnimationError> {
        channel.validate_value(&self.motion.sample(elapsed_micros))
    }

    fn sample(
        &self,
        elapsed_micros: u64,
        channel: &mut ChannelState,
    ) -> Result<bool, AnimationError> {
        channel.set_override(self.motion.sample(elapsed_micros))
    }
}

pub(crate) struct PreparedPlanTrack {
    pub(crate) channel: AnimationChannelId,
    pub(crate) start_micros: u64,
    pub(crate) duration_micros: u64,
    pub(crate) sampler: Box<dyn PreparedTrack>,
}
