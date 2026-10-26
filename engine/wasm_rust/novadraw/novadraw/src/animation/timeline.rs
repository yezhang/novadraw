use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::identity::AnimationChannelId;

use super::{
    AnimationBudgetKind, AnimationChannel, AnimationError, AnimationValue,
    DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS, InterruptionPolicy, SuspensionPolicy,
    service::{ChannelState, PreparedChannelValue},
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
    pub(crate) geometry_source: Option<Arc<super::frozen::FrozenGeometrySource>>,
    pub(crate) providers: super::provider::ProviderGraph,
}

impl AnimationPlan {
    /// Creates a one-track plan.
    pub fn track<V: AnimationValue>(
        channel: AnimationChannel<V>,
        motion: Motion<V>,
    ) -> Result<Self, AnimationError> {
        Self::track_with_endpoint(channel, motion, false)
    }

    /// Runs a geometry track with an explicit presentation endpoint distinct from committed
    /// source geometry. Completion/cancellation clears the override and reveals committed
    /// state. Only a channel registered with `GeometryBinding` can use this contract.
    pub fn geometry_track<V: AnimationValue + Copy>(
        channel: AnimationChannel<V>,
        motion: Motion<V>,
    ) -> Result<Self, AnimationError> {
        Self::track_with_endpoint(channel, motion, true)
    }

    fn track_with_endpoint<V: AnimationValue>(
        channel: AnimationChannel<V>,
        motion: Motion<V>,
        presentation_endpoint: bool,
    ) -> Result<Self, AnimationError> {
        let duration_micros = motion.duration_micros();
        Ok(Self {
            tracks: vec![PlanTrack {
                start_micros: 0,
                duration_micros,
                spec: Box::new(TypedTrackSpec {
                    channel: channel.id(),
                    motion,
                    presentation_endpoint,
                }),
            }],
            duration_micros,
            interruption: InterruptionPolicy::Replace,
            suspension: SuspensionPolicy::Advance,
            geometry_source: None,
            providers: super::provider::ProviderGraph::default(),
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

    /// Adds a pure derived provider to this plan's atomic group.
    pub fn with_geometry_provider<V: AnimationValue + Copy>(
        mut self,
        provider: super::GeometryProvider<V>,
    ) -> Self {
        self.providers.push(provider);
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
            geometry_source: self.geometry_source.clone(),
            providers: self.providers.clone(),
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
            geometry_source: self.geometry_source.clone(),
            providers: self.providers.clone(),
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
    let mut geometry_source = None;
    let mut providers = super::provider::ProviderGraph::default();
    for (plan, offset) in plans.into_iter().zip(offsets) {
        providers.append(plan.providers);
        if plan.geometry_source.is_some() {
            if geometry_source.is_some() {
                return Err(AnimationError::InvalidComposition);
            }
            geometry_source = plan.geometry_source.clone();
        }
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
        geometry_source,
        providers,
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

    fn storage_work(&self, channel: &ChannelState) -> Result<super::AnimationWork, AnimationError>;

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
    presentation_endpoint: bool,
}

impl<V: AnimationValue> ErasedTrackSpec for TypedTrackSpec<V> {
    fn channel(&self) -> AnimationChannelId {
        self.channel
    }

    fn storage_work(&self, channel: &ChannelState) -> Result<super::AnimationWork, AnimationError> {
        let (bytes, _) = self.motion.storage_bound(channel.effective::<V>()?)?;
        let bytes = bytes
            .checked_add(
                std::mem::size_of::<PlanTrack>()
                    + std::mem::size_of::<PreparedPlanTrack>()
                    + std::mem::size_of::<TypedPreparedTrack<V>>()
                    + 8 * std::mem::size_of::<usize>(),
            )
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ))?;
        Ok(super::AnimationWork {
            bytes,
            ..Default::default()
        })
    }

    fn clone_box(&self) -> Box<dyn ErasedTrackSpec> {
        Box::new(self.clone())
    }

    fn reversed_box(&self) -> Result<Box<dyn ErasedTrackSpec>, AnimationError> {
        Ok(Box::new(Self {
            channel: self.channel,
            motion: self.motion.reversed()?,
            presentation_endpoint: self.presentation_endpoint,
        }))
    }

    fn terminal_matches(&self, channel: &ChannelState) -> Result<bool, AnimationError> {
        if self.presentation_endpoint {
            return if channel.supports_geometry_endpoint() {
                Ok(true)
            } else {
                Err(AnimationError::InvalidComposition)
            };
        }
        match self.motion.terminal_value() {
            Some(terminal) => Ok(terminal == channel.committed::<V>()?),
            None => Ok(true),
        }
    }

    fn prepare(
        self: Box<Self>,
        channel: &ChannelState,
    ) -> Result<Box<dyn PreparedTrack>, AnimationError> {
        if self.presentation_endpoint && !channel.supports_geometry_endpoint() {
            return Err(AnimationError::InvalidComposition);
        }
        let current = channel.effective::<V>()?.clone();
        let (_, storage_bound) = self.motion.storage_bound(&current)?;
        let duration = self.motion.duration_micros();
        let prepared = TypedPreparedTrack {
            motion: self.motion.prepare(current),
            channel: self.channel,
            storage_bound,
        };
        for elapsed in [0, duration / 2, duration] {
            prepared.sample(elapsed, channel)?;
        }
        Ok(Box::new(prepared))
    }
}

pub(crate) trait PreparedTrack: Send {
    fn sample(
        &self,
        elapsed_micros: u64,
        channel: &ChannelState,
    ) -> Result<PreparedChannelValue, AnimationError>;
}

struct TypedPreparedTrack<V> {
    motion: PreparedMotion<V>,
    channel: AnimationChannelId,
    storage_bound: usize,
}

impl<V: AnimationValue> PreparedTrack for TypedPreparedTrack<V> {
    fn sample(
        &self,
        elapsed_micros: u64,
        channel: &ChannelState,
    ) -> Result<PreparedChannelValue, AnimationError> {
        let value = self.motion.sample(elapsed_micros);
        let actual = value.storage_bytes();
        if actual > self.storage_bound {
            return Err(AnimationError::TrackStorageExceeded {
                channel: self.channel,
                declared: self.storage_bound,
                actual,
            });
        }
        channel.prepare_override(value)
    }
}

pub(crate) struct PreparedPlanTrack {
    pub(crate) channel: AnimationChannelId,
    pub(crate) start_micros: u64,
    pub(crate) duration_micros: u64,
    pub(crate) sampler: Box<dyn PreparedTrack>,
}
