use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
};

use crate::{
    Affine2D, FigureId, FigureTree, MonotonicTime, Rectangle, TimeError, UpdateManager,
    figure::FigurePresentation,
    identity::{
        AnimationChannelId, AnimationId, RuntimeArena, RuntimeNamespace, TemporaryVisualId,
    },
};

use super::{
    AnimationBudgetKind, AnimationChannel, AnimationError, AnimationMode, AnimationStart,
    AnimationState, AnimationStats, AnimationSuppression, AnimationValue, BoundsTransition,
    DEFAULT_ANIMATION_TERMINAL_HISTORY, DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
    DEFAULT_MAX_ACTIVE_ANIMATIONS, DEFAULT_MAX_ANIMATION_CHANNELS, DEFAULT_MAX_TEMPORARY_VISUALS,
    FigurePresentationChannels, FigureTransitionCapture, InteractionGeometryPolicy,
    InterruptionPolicy, Motion, Opacity, SuspensionPolicy, TemporaryVisual, Tween,
    timeline::{AnimationPlan, PreparedPlanTrack},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChannelBinding {
    Detached,
    FigureOpacity(FigureId),
    FigureTransform(FigureId),
    TemporaryOpacity(TemporaryVisualId),
    TemporaryTransform(TemporaryVisualId),
}

pub(crate) struct ChannelState {
    committed: Box<dyn Any + Send + Sync>,
    presentation: Option<Box<dyn Any + Send + Sync>>,
    binding: ChannelBinding,
}

impl ChannelState {
    fn new<V: AnimationValue>(value: V) -> Self {
        Self::new_bound(value, ChannelBinding::Detached)
    }

    fn new_bound<V: AnimationValue>(value: V, binding: ChannelBinding) -> Self {
        Self {
            committed: Box::new(value),
            presentation: None,
            binding,
        }
    }

    pub(crate) fn committed<V: AnimationValue>(&self) -> Result<&V, AnimationError> {
        self.committed
            .downcast_ref::<V>()
            .ok_or(AnimationError::InvalidValue)
    }

    pub(crate) fn effective<V: AnimationValue>(&self) -> Result<&V, AnimationError> {
        self.presentation.as_ref().map_or_else(
            || self.committed::<V>(),
            |value| {
                value
                    .downcast_ref::<V>()
                    .ok_or(AnimationError::InvalidValue)
            },
        )
    }

    pub(crate) fn set_override<V: AnimationValue>(
        &mut self,
        value: V,
    ) -> Result<bool, AnimationError> {
        self.validate_value(&value)?;
        let changed = self.effective::<V>()? != &value;
        self.presentation = Some(Box::new(value));
        Ok(changed)
    }

    pub(crate) fn validate_value<V: AnimationValue>(
        &self,
        value: &V,
    ) -> Result<(), AnimationError> {
        if value.is_valid() && self.binding.accepts(value) {
            Ok(())
        } else {
            Err(AnimationError::InvalidValue)
        }
    }

    fn set_committed<V: AnimationValue>(&mut self, value: V) -> Result<bool, AnimationError> {
        if !value.is_valid() || !self.binding.accepts(&value) {
            return Err(AnimationError::InvalidValue);
        }
        let changed = self.presentation.is_none() && self.committed::<V>()? != &value;
        self.committed = Box::new(value);
        Ok(changed)
    }

    fn clear_override(&mut self) -> bool {
        self.presentation.take().is_some()
    }

    fn has_override(&self) -> bool {
        self.presentation.is_some()
    }

    fn into_committed<V: AnimationValue>(self) -> Result<V, AnimationError> {
        self.committed
            .downcast::<V>()
            .map(|value| *value)
            .map_err(|_| AnimationError::InvalidValue)
    }
}

impl ChannelBinding {
    fn accepts<V: AnimationValue>(self, value: &V) -> bool {
        match self {
            Self::FigureOpacity(_) | Self::TemporaryOpacity(_) => (value as &dyn Any)
                .downcast_ref::<Opacity>()
                .is_some_and(AnimationValue::is_valid),
            Self::FigureTransform(_) | Self::TemporaryTransform(_) => {
                (value as &dyn Any).downcast_ref::<Affine2D>().is_some()
            }
            Self::Detached => true,
        }
    }

    fn figure(self) -> Option<FigureId> {
        match self {
            Self::FigureOpacity(figure) | Self::FigureTransform(figure) => Some(figure),
            _ => None,
        }
    }

    fn temporary(self) -> Option<TemporaryVisualId> {
        match self {
            Self::TemporaryOpacity(visual) | Self::TemporaryTransform(visual) => Some(visual),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigurePresentationEffect {
    pub(crate) opacity: Option<f64>,
    pub(crate) transform: Option<Affine2D>,
}

#[derive(Clone)]
pub(crate) struct TemporaryVisualSnapshot {
    pub(crate) id: TemporaryVisualId,
    pub(crate) presentation: FigurePresentation,
    pub(crate) opacity: f64,
    pub(crate) transform: Affine2D,
}

#[derive(Clone, Default)]
pub(crate) struct PresentationSnapshot {
    figures: HashMap<FigureId, FigurePresentationEffect>,
    temporaries: Vec<TemporaryVisualSnapshot>,
}

impl PresentationSnapshot {
    pub(crate) fn figure(&self, id: FigureId) -> Option<FigurePresentationEffect> {
        self.figures.get(&id).copied()
    }

    pub(crate) fn paint_temporaries(&self, canvas: &mut crate::NdCanvas) {
        for visual in &self.temporaries {
            canvas.push_state();
            let [a, b, c, d, e, f] = visual.transform.coeffs();
            canvas.transform(a, b, c, d, e, f);
            canvas.set_alpha(visual.opacity);
            let mut context = crate::graphics::PaintContext::for_figure(canvas);
            if let Err(error) = visual.presentation.paint(&mut context) {
                context.canvas.reject_recording(error);
            }
            drop(context);
            canvas.pop_state();
        }
    }
}

struct FigureChannels {
    opacity: AnimationChannelId,
    transform: AnimationChannelId,
}

struct TemporaryVisualState {
    presentation: FigurePresentation,
    opacity: Option<AnimationChannelId>,
    transform: Option<AnimationChannelId>,
    owner: Option<AnimationId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum DamageSubject {
    Figure(FigureId),
    Temporary(TemporaryVisualId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DamageSubjectState {
    Figure {
        opacity: f64,
        transform: Affine2D,
        envelope: Option<Rectangle>,
    },
    Temporary {
        opacity: f64,
        transform: Affine2D,
        envelope: Option<Rectangle>,
        active: bool,
    },
}

struct ActiveAnimation {
    tracks: Vec<PreparedPlanTrack>,
    duration_micros: u64,
    start_time: Option<MonotonicTime>,
    suspension: SuspensionPolicy,
    paused_at: Option<MonotonicTime>,
    surface_suspended: bool,
    target_hidden: bool,
}

struct PreparedAnimation {
    tracks: Vec<PreparedPlanTrack>,
    duration_micros: u64,
    interruption: InterruptionPolicy,
    suspension: SuspensionPolicy,
}

pub(crate) struct AnimationService {
    namespace: RuntimeNamespace,
    mode: AnimationMode,
    now: MonotonicTime,
    time_initialized: bool,
    surface_suspended: bool,
    channels: RuntimeArena<AnimationChannelId, ChannelState>,
    active: RuntimeArena<AnimationId, ActiveAnimation>,
    figure_channels: HashMap<FigureId, FigureChannels>,
    temporary_visuals: RuntimeArena<TemporaryVisualId, TemporaryVisualState>,
    channel_owners: HashMap<AnimationChannelId, AnimationId>,
    terminal: VecDeque<(AnimationId, AnimationState)>,
    active_track_count: usize,
    stats: AnimationStats,
}

impl AnimationService {
    pub(crate) fn new(namespace: RuntimeNamespace) -> Self {
        Self {
            namespace,
            mode: AnimationMode::Enabled,
            now: MonotonicTime::ZERO,
            time_initialized: false,
            surface_suspended: false,
            channels: RuntimeArena::new(namespace),
            active: RuntimeArena::new(namespace),
            figure_channels: HashMap::new(),
            temporary_visuals: RuntimeArena::new(namespace),
            channel_owners: HashMap::new(),
            terminal: VecDeque::new(),
            active_track_count: 0,
            stats: AnimationStats::default(),
        }
    }

    pub(crate) fn editor<'a>(
        &'a mut self,
        tree: &'a FigureTree,
        updates: &'a mut UpdateManager,
        full_redraw_pending: &'a mut bool,
        faulted: &'a mut bool,
    ) -> AnimationMut<'a> {
        AnimationMut {
            service: self,
            tree,
            updates,
            full_redraw_pending,
            faulted,
        }
    }

    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if self.time_initialized && now < self.now {
            return Err(TimeError::NonMonotonic {
                previous: self.now,
                next: now,
            });
        }
        self.now = now;
        self.time_initialized = true;
        self.stats.clock_advances = self.stats.clock_advances.saturating_add(1);

        let ids: Vec<_> = self.active.iter().map(|(id, _)| id).collect();
        let mut changed = false;
        let mut completed = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if self
                .active
                .get(id)
                .is_some_and(|animation| animation.paused_at.is_some())
            {
                continue;
            }
            let start = {
                let animation = self
                    .active
                    .get_mut(id)
                    .expect("active animation ID must remain valid");
                *animation.start_time.get_or_insert(now)
            };
            let elapsed = now.as_micros().saturating_sub(start.as_micros());
            match self.sample(id, elapsed) {
                Ok(sampled) => changed |= sampled,
                Err(_) => {
                    failed.push(id);
                    continue;
                }
            }
            if self
                .active
                .get(id)
                .is_some_and(|animation| elapsed >= animation.duration_micros)
            {
                completed.push(id);
            }
        }
        for id in completed {
            changed |= self.finish(id, AnimationState::Completed);
        }
        for id in failed {
            changed |= self.finish(id, AnimationState::Failed);
        }
        Ok(changed)
    }

    pub(crate) fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        self.active
            .iter()
            .any(|(_, animation)| animation.paused_at.is_none())
            .then_some(self.now)
    }

    pub(crate) fn snapshot(&self) -> Option<PresentationSnapshot> {
        if self.active.len() == 0 {
            return None;
        }
        let (active_figures, active_temporaries) = self.active_subjects();
        let mut figures = HashMap::new();
        for figure in active_figures {
            let Some(channels) = self.figure_channels.get(&figure) else {
                continue;
            };
            let opacity = self.channels.get(channels.opacity).and_then(|channel| {
                channel
                    .has_override()
                    .then(|| channel.effective::<Opacity>().ok().copied())
                    .flatten()
                    .map(Opacity::get)
            });
            let transform = self.channels.get(channels.transform).and_then(|channel| {
                channel
                    .has_override()
                    .then(|| channel.effective::<Affine2D>().ok().copied())
                    .flatten()
            });
            if opacity.is_some() || transform.is_some() {
                figures.insert(figure, FigurePresentationEffect { opacity, transform });
            }
        }

        let mut temporaries: Vec<_> = active_temporaries
            .into_iter()
            .filter_map(|id| {
                let visual = self.temporary_visuals.get(id)?;
                let opacity_channel = visual.opacity?;
                let transform_channel = visual.transform?;
                Some(TemporaryVisualSnapshot {
                    id,
                    presentation: visual.presentation.clone(),
                    opacity: self
                        .channels
                        .get(opacity_channel)?
                        .effective::<Opacity>()
                        .ok()?
                        .get(),
                    transform: *self
                        .channels
                        .get(transform_channel)?
                        .effective::<Affine2D>()
                        .ok()?,
                })
            })
            .collect();
        temporaries.sort_by_key(|visual| visual.id);
        (!figures.is_empty() || !temporaries.is_empty()).then_some(PresentationSnapshot {
            figures,
            temporaries,
        })
    }

    fn damage_state(&self, tree: &FigureTree) -> HashMap<DamageSubject, DamageSubjectState> {
        let (active_figures, active_temporaries) = self.active_subjects();
        let mut state = HashMap::new();
        for figure in active_figures {
            let Some(channels) = self.figure_channels.get(&figure) else {
                continue;
            };
            let Some(opacity) = self
                .channels
                .get(channels.opacity)
                .and_then(|channel| channel.effective::<Opacity>().ok())
                .copied()
                .map(Opacity::get)
            else {
                continue;
            };
            let Some(transform) = self
                .channels
                .get(channels.transform)
                .and_then(|channel| channel.effective::<Affine2D>().ok())
                .copied()
            else {
                continue;
            };
            state.insert(
                DamageSubject::Figure(figure),
                DamageSubjectState::Figure {
                    opacity,
                    transform,
                    envelope: tree.presentation_envelope(figure, transform),
                },
            );
        }
        for id in active_temporaries {
            let Some(visual) = self.temporary_visuals.get(id) else {
                continue;
            };
            let (Some(opacity_channel), Some(transform_channel)) =
                (visual.opacity, visual.transform)
            else {
                continue;
            };
            let Some(opacity) = self
                .channels
                .get(opacity_channel)
                .and_then(|channel| channel.effective::<Opacity>().ok())
                .copied()
                .map(Opacity::get)
            else {
                continue;
            };
            let Some(transform) = self
                .channels
                .get(transform_channel)
                .and_then(|channel| channel.effective::<Affine2D>().ok())
                .copied()
            else {
                continue;
            };
            state.insert(
                DamageSubject::Temporary(id),
                DamageSubjectState::Temporary {
                    opacity,
                    transform,
                    envelope: transformed_envelope(transform, visual.presentation.visual_bounds()),
                    active: visual.owner.is_some(),
                },
            );
        }
        state
    }

    fn active_subjects(&self) -> (HashSet<FigureId>, HashSet<TemporaryVisualId>) {
        let mut figures = HashSet::new();
        let mut temporaries = HashSet::new();
        for (_, animation) in self.active.iter() {
            for track in &animation.tracks {
                let Some(channel) = self.channels.get(track.channel) else {
                    continue;
                };
                if let Some(figure) = channel.binding.figure() {
                    figures.insert(figure);
                }
                if let Some(visual) = channel.binding.temporary() {
                    temporaries.insert(visual);
                }
            }
        }
        (figures, temporaries)
    }

    pub(crate) fn reconcile_visibility(&mut self, tree: &FigureTree) -> bool {
        let ids: Vec<_> = self.active.iter().map(|(id, _)| id).collect();
        let mut changed = false;
        for id in ids {
            let target_hidden = self.active.get(id).is_some_and(|animation| {
                animation.tracks.iter().any(|track| {
                    self.channels
                        .get(track.channel)
                        .and_then(|channel| channel.binding.figure())
                        .is_some_and(|figure| !tree.is_effectively_visible(figure))
                })
            });
            changed |= self.set_suspension_reason(id, None, Some(target_hidden));
        }
        changed
    }

    pub(crate) fn set_surface_suspended(&mut self, suspended: bool) -> bool {
        self.surface_suspended = suspended;
        let ids: Vec<_> = self.active.iter().map(|(id, _)| id).collect();
        let mut changed = false;
        for id in ids {
            changed |= self.set_suspension_reason(id, Some(suspended), None);
        }
        changed
    }

    pub(crate) fn retire_figures(&mut self, figures: &HashSet<FigureId>) {
        let owners: HashSet<_> = self
            .figure_channels
            .iter()
            .filter(|(figure, _)| figures.contains(figure))
            .flat_map(|(_, channels)| [channels.opacity, channels.transform])
            .filter_map(|channel| self.channel_owners.get(&channel).copied())
            .collect();
        for owner in owners {
            self.finish(owner, AnimationState::Cancelled);
        }
        let retired: Vec<_> = self
            .figure_channels
            .keys()
            .copied()
            .filter(|figure| figures.contains(figure))
            .collect();
        for figure in retired {
            if let Some(channels) = self.figure_channels.remove(&figure) {
                self.channels.remove(channels.opacity);
                self.channels.remove(channels.transform);
            }
        }
    }

    pub(crate) fn clear_presentation(&mut self) {
        let ids: Vec<_> = self.active.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.finish(id, AnimationState::Cancelled);
        }
        let dormant: Vec<_> = self.temporary_visuals.iter().map(|(id, _)| id).collect();
        for visual in dormant {
            self.remove_temporary_visual_state(visual);
        }
    }

    fn set_suspension_reason(
        &mut self,
        id: AnimationId,
        surface_suspended: Option<bool>,
        target_hidden: Option<bool>,
    ) -> bool {
        let Some(animation) = self.active.get_mut(id) else {
            return false;
        };
        if let Some(suspended) = surface_suspended {
            animation.surface_suspended = suspended;
        }
        if let Some(hidden) = target_hidden {
            animation.target_hidden = hidden;
        }
        let suspended = animation.surface_suspended || animation.target_hidden;
        let finish = match animation.suspension {
            SuspensionPolicy::Advance => false,
            SuspensionPolicy::Finish if suspended => true,
            SuspensionPolicy::Finish => false,
            SuspensionPolicy::Pause if suspended && animation.paused_at.is_none() => {
                animation.paused_at = Some(self.now);
                false
            }
            SuspensionPolicy::Pause if !suspended => {
                let Some(paused_at) = animation.paused_at.take() else {
                    return false;
                };
                if let Some(start) = animation.start_time.as_mut() {
                    let paused = self.now.as_micros().saturating_sub(paused_at.as_micros());
                    *start = MonotonicTime::from_micros(start.as_micros().saturating_add(paused));
                }
                false
            }
            SuspensionPolicy::Pause => false,
        };
        if finish {
            self.finish(id, AnimationState::Completed)
        } else {
            false
        }
    }

    fn prepare(&self, plan: AnimationPlan) -> Result<PreparedAnimation, AnimationError> {
        if plan.tracks.is_empty() {
            return Err(AnimationError::EmptyPlan);
        }
        let mut final_tracks: HashMap<AnimationChannelId, (u64, usize)> = HashMap::new();
        for (index, track) in plan.tracks.iter().enumerate() {
            let end = track
                .start_micros
                .checked_add(track.duration_micros)
                .ok_or(AnimationError::TimeOverflow)?;
            let entry = final_tracks
                .entry(track.spec.channel())
                .or_insert((end, index));
            if (end, index) > *entry {
                *entry = (end, index);
            }
        }
        for (channel_id, (_, index)) in &final_tracks {
            if channel_id.namespace() != self.namespace {
                return Err(AnimationError::ForeignChannel);
            }
            let channel = self
                .channels
                .get(*channel_id)
                .ok_or(AnimationError::UnknownChannel)?;
            if !plan.tracks[*index].spec.terminal_matches(channel)? {
                return Err(AnimationError::FinalValueMismatch);
            }
        }

        let mut tracks = Vec::with_capacity(plan.tracks.len());
        for track in plan.tracks {
            let channel_id = track.spec.channel();
            if channel_id.namespace() != self.namespace {
                return Err(AnimationError::ForeignChannel);
            }
            let channel = self
                .channels
                .get(channel_id)
                .ok_or(AnimationError::UnknownChannel)?;
            let sampler = track.spec.prepare(channel)?;
            tracks.push(PreparedPlanTrack {
                channel: channel_id,
                start_micros: track.start_micros,
                duration_micros: track.duration_micros,
                sampler,
            });
        }
        Ok(PreparedAnimation {
            tracks,
            duration_micros: plan.duration_micros,
            interruption: plan.interruption,
            suspension: plan.suspension,
        })
    }

    fn start(&mut self, plan: AnimationPlan) -> Result<AnimationStart, AnimationError> {
        let prepared = self.prepare(plan)?;
        match self.mode {
            AnimationMode::Disabled => {
                return Ok(AnimationStart::Suppressed(AnimationSuppression::Disabled));
            }
            AnimationMode::ReducedMotion => {
                return Ok(AnimationStart::Suppressed(
                    AnimationSuppression::ReducedMotionStaticFallback,
                ));
            }
            AnimationMode::Enabled => {}
        }
        if prepared.duration_micros == 0 {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }
        self.validate_active_budget(prepared.tracks.len(), 0)?;

        let mut existing: HashSet<_> = prepared
            .tracks
            .iter()
            .filter_map(|track| self.channel_owners.get(&track.channel).copied())
            .collect();
        for visual in self.temporary_targets(&prepared.tracks) {
            if let Some(owner) = self
                .temporary_visuals
                .get(visual)
                .and_then(|visual| visual.owner)
            {
                existing.insert(owner);
            }
        }
        if let (InterruptionPolicy::Ignore, Some(owner)) =
            (prepared.interruption, existing.iter().next().copied())
        {
            return Ok(AnimationStart::Existing(owner));
        }
        let temporary_targets = self.temporary_targets(&prepared.tracks);
        for owner in existing {
            self.finish_preserving_temporaries(
                owner,
                AnimationState::Cancelled,
                &temporary_targets,
            );
        }

        let start_time = self.time_initialized.then_some(self.now);
        let id = self.active.insert(ActiveAnimation {
            tracks: prepared.tracks,
            duration_micros: prepared.duration_micros,
            start_time,
            suspension: prepared.suspension,
            paused_at: None,
            surface_suspended: self.surface_suspended,
            target_hidden: false,
        });
        let track_count = self
            .active
            .get(id)
            .expect("new animation must exist")
            .tracks
            .len();
        self.active_track_count += track_count;
        for channel in self
            .active
            .get(id)
            .expect("new animation must exist")
            .tracks
            .iter()
            .map(|track| track.channel)
        {
            self.channel_owners.insert(channel, id);
        }
        self.assign_temporary_visuals(id);
        if let Err(error) = self.sample(id, 0) {
            self.finish_preserving_temporaries(id, AnimationState::Failed, &temporary_targets);
            self.terminal.retain(|(candidate, _)| *candidate != id);
            return Err(error);
        }
        self.set_suspension_reason(id, None, None);
        Ok(AnimationStart::Running(id))
    }

    fn retarget(&mut self, id: AnimationId, plan: AnimationPlan) -> Result<bool, AnimationError> {
        self.validate_animation_id(id)?;
        let prepared = self.prepare(plan)?;
        if prepared.duration_micros == 0 {
            return Ok(self.finish(id, AnimationState::Cancelled));
        }
        let old_track_count = self
            .active
            .get(id)
            .ok_or(AnimationError::UnknownAnimation)?
            .tracks
            .len();
        let (surface_suspended, target_hidden) = self
            .active
            .get(id)
            .map(|animation| (animation.surface_suspended, animation.target_hidden))
            .expect("validated animation must exist");
        self.validate_active_budget(prepared.tracks.len(), old_track_count)?;
        if prepared.tracks.iter().any(|track| {
            self.channel_owners
                .get(&track.channel)
                .is_some_and(|owner| *owner != id)
        }) {
            return Err(AnimationError::ChannelConflict);
        }
        if self
            .temporary_targets(&prepared.tracks)
            .into_iter()
            .any(|visual| {
                self.temporary_visuals
                    .get(visual)
                    .and_then(|visual| visual.owner)
                    .is_some_and(|owner| owner != id)
            })
        {
            return Err(AnimationError::ChannelConflict);
        }

        let old_channels: HashSet<_> = self
            .active
            .get(id)
            .expect("validated animation must exist")
            .tracks
            .iter()
            .map(|track| track.channel)
            .collect();
        let new_channels: HashSet<_> = prepared.tracks.iter().map(|track| track.channel).collect();
        let old_temporaries = self.temporary_targets(
            &self
                .active
                .get(id)
                .expect("validated animation must exist")
                .tracks,
        );
        let new_temporaries = self.temporary_targets(&prepared.tracks);
        let mut changed = false;
        for channel in old_channels.difference(&new_channels) {
            self.channel_owners.remove(channel);
            if let Some(state) = self.channels.get_mut(*channel) {
                changed |= state.clear_override();
            }
        }
        for channel in &new_channels {
            self.channel_owners.insert(*channel, id);
        }

        self.active_track_count = self
            .active_track_count
            .saturating_sub(old_track_count)
            .saturating_add(prepared.tracks.len());
        *self
            .active
            .get_mut(id)
            .expect("validated animation must exist") = ActiveAnimation {
            tracks: prepared.tracks,
            duration_micros: prepared.duration_micros,
            start_time: self.time_initialized.then_some(self.now),
            suspension: prepared.suspension,
            paused_at: None,
            surface_suspended,
            target_hidden,
        };
        for visual in old_temporaries.difference(&new_temporaries) {
            changed |= self.remove_temporary_visual_state(*visual);
        }
        self.assign_temporary_visuals(id);
        changed |= self.sample(id, 0)?;
        changed |= self.set_suspension_reason(id, None, None);
        Ok(changed)
    }

    fn cancel(&mut self, id: AnimationId) -> Result<bool, AnimationError> {
        self.validate_animation_id(id)?;
        if self.active.contains_key(id) {
            return Ok(self.finish(id, AnimationState::Cancelled));
        }
        if self.terminal.iter().any(|(candidate, _)| *candidate == id) {
            return Ok(false);
        }
        Err(AnimationError::UnknownAnimation)
    }

    fn sample(&mut self, id: AnimationId, elapsed_micros: u64) -> Result<bool, AnimationError> {
        let (active, channels) = (&self.active, &mut self.channels);
        let animation = active
            .get(id)
            .expect("sampled animation must remain active");
        let mut selected: HashMap<AnimationChannelId, usize> = HashMap::new();
        for (index, track) in animation.tracks.iter().enumerate() {
            let Some(current) = selected.get(&track.channel).copied() else {
                selected.insert(track.channel, index);
                continue;
            };
            let current_track = &animation.tracks[current];
            let current_started = current_track.start_micros <= elapsed_micros;
            let candidate_started = track.start_micros <= elapsed_micros;
            if (candidate_started && track.start_micros >= current_track.start_micros)
                || (!current_started && track.start_micros < current_track.start_micros)
            {
                selected.insert(track.channel, index);
            }
        }

        let mut changed = false;
        let mut selected: Vec<_> = selected.into_values().collect();
        selected.sort_unstable();
        for index in selected {
            let track = &animation.tracks[index];
            let local = elapsed_micros
                .saturating_sub(track.start_micros)
                .min(track.duration_micros);
            let channel = channels
                .get_mut(track.channel)
                .expect("prepared channel must remain registered");
            changed |= track.sampler.sample(local, channel)?;
            self.stats.tracks_sampled = self.stats.tracks_sampled.saturating_add(1);
        }
        Ok(changed)
    }

    fn finish(&mut self, id: AnimationId, state: AnimationState) -> bool {
        self.finish_preserving_temporaries(id, state, &HashSet::new())
    }

    fn finish_preserving_temporaries(
        &mut self,
        id: AnimationId,
        state: AnimationState,
        preserved: &HashSet<TemporaryVisualId>,
    ) -> bool {
        let Some(animation) = self.active.remove(id) else {
            return false;
        };
        let temporary_visuals = self.temporary_targets(&animation.tracks);
        self.active_track_count = self
            .active_track_count
            .saturating_sub(animation.tracks.len());
        let mut changed = false;
        for channel in animation.tracks.into_iter().map(|track| track.channel) {
            if self.channel_owners.get(&channel) == Some(&id) {
                self.channel_owners.remove(&channel);
                if let Some(channel) = self.channels.get_mut(channel) {
                    changed |= channel.clear_override();
                }
            }
        }
        for visual in temporary_visuals {
            if preserved.contains(&visual) {
                if let Some(visual) = self.temporary_visuals.get_mut(visual) {
                    visual.owner = None;
                }
            } else {
                changed |= self.remove_temporary_visual_state(visual);
            }
        }
        self.record_terminal(id, state);
        changed
    }

    fn record_terminal(&mut self, id: AnimationId, state: AnimationState) {
        self.terminal.push_back((id, state));
        while self.terminal.len() > DEFAULT_ANIMATION_TERMINAL_HISTORY {
            self.terminal.pop_front();
        }
    }

    fn validate_animation_id(&self, id: AnimationId) -> Result<(), AnimationError> {
        if id.namespace() != self.namespace {
            Err(AnimationError::ForeignAnimation)
        } else {
            Ok(())
        }
    }

    fn validate_active_budget(
        &self,
        new_tracks: usize,
        replaced_tracks: usize,
    ) -> Result<(), AnimationError> {
        if replaced_tracks == 0 && self.active.len() >= DEFAULT_MAX_ACTIVE_ANIMATIONS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveAnimations,
            ));
        }
        if self
            .active_track_count
            .saturating_sub(replaced_tracks)
            .saturating_add(new_tracks)
            > DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS
        {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ));
        }
        Ok(())
    }

    fn temporary_targets(&self, tracks: &[PreparedPlanTrack]) -> HashSet<TemporaryVisualId> {
        tracks
            .iter()
            .filter_map(|track| {
                self.channels
                    .get(track.channel)
                    .and_then(|channel| channel.binding.temporary())
            })
            .collect()
    }

    fn assign_temporary_visuals(&mut self, owner: AnimationId) {
        let targets = self
            .active
            .get(owner)
            .map(|animation| self.temporary_targets(&animation.tracks))
            .unwrap_or_default();
        for visual in targets {
            if let Some(visual) = self.temporary_visuals.get_mut(visual) {
                visual.owner = Some(owner);
            }
        }
    }

    fn remove_temporary_visual_state(&mut self, id: TemporaryVisualId) -> bool {
        let Some(visual) = self.temporary_visuals.remove(id) else {
            return false;
        };
        if let Some(opacity) = visual.opacity {
            self.channel_owners.remove(&opacity);
            self.channels.remove(opacity);
        }
        if let Some(transform) = visual.transform {
            self.channel_owners.remove(&transform);
            self.channels.remove(transform);
        }
        true
    }
}

fn bounds_transition_transform(
    before: Rectangle,
    after: Rectangle,
) -> Result<Affine2D, AnimationError> {
    let scale_x = transition_scale(before.width, after.width)?;
    let scale_y = transition_scale(before.height, after.height)?;
    let transform = Affine2D::from_translation(before.x - after.x, before.y - after.y)
        * Affine2D::from_scale(scale_x, scale_y);
    transform
        .is_valid()
        .then_some(transform)
        .ok_or(AnimationError::IncompatibleBoundsTransition)
}

fn transition_scale(before: f64, after: f64) -> Result<f64, AnimationError> {
    if before == after {
        return Ok(1.0);
    }
    if after == 0.0 {
        return Err(AnimationError::IncompatibleBoundsTransition);
    }
    let scale = before / after;
    scale
        .is_finite()
        .then_some(scale)
        .ok_or(AnimationError::IncompatibleBoundsTransition)
}

fn transformed_envelope(transform: Affine2D, rectangle: Rectangle) -> Option<Rectangle> {
    let corners = [
        transform.transform_point(crate::Point::new(rectangle.x, rectangle.y)),
        transform.transform_point(crate::Point::new(
            rectangle.x + rectangle.width,
            rectangle.y,
        )),
        transform.transform_point(crate::Point::new(
            rectangle.x,
            rectangle.y + rectangle.height,
        )),
        transform.transform_point(crate::Point::new(
            rectangle.x + rectangle.width,
            rectangle.y + rectangle.height,
        )),
    ];
    if corners
        .iter()
        .flat_map(|point| [point.x(), point.y()])
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let left = corners
        .iter()
        .map(|point| point.x())
        .fold(f64::INFINITY, f64::min);
    let top = corners
        .iter()
        .map(|point| point.y())
        .fold(f64::INFINITY, f64::min);
    let right = corners
        .iter()
        .map(|point| point.x())
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = corners
        .iter()
        .map(|point| point.y())
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Rectangle::new(left, top, right - left, bottom - top))
}

/// Scoped mutable access to one Runtime's animation service.
pub struct AnimationMut<'a> {
    service: &'a mut AnimationService,
    tree: &'a FigureTree,
    updates: &'a mut UpdateManager,
    full_redraw_pending: &'a mut bool,
    faulted: &'a mut bool,
}

impl AnimationMut<'_> {
    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if self.service.active.len() == 0 {
            return self.service.advance_time(now);
        }
        let before = self.service.damage_state(self.tree);
        let changed = self.guarded_service(|service| service.advance_time(now))?;
        self.reconcile_damage(before);
        Ok(changed)
    }

    pub(crate) fn reconcile_visibility(&mut self) -> bool {
        if self.service.active.len() == 0 {
            return false;
        }
        let before = self.service.damage_state(self.tree);
        let changed = self.service.reconcile_visibility(self.tree);
        self.reconcile_damage(before);
        changed
    }

    pub(crate) fn set_surface_suspended(&mut self, suspended: bool) -> bool {
        if self.service.active.len() == 0 {
            return self.service.set_surface_suspended(suspended);
        }
        let before = self.service.damage_state(self.tree);
        let changed = self.service.set_surface_suspended(suspended);
        self.reconcile_damage(before);
        changed
    }

    /// Returns the Runtime-wide mode.
    pub fn mode(&self) -> AnimationMode {
        self.service.mode
    }

    /// Changes the Runtime-wide mode and clears active presentation when disabling motion.
    pub fn set_mode(&mut self, mode: AnimationMode) -> bool {
        if *self.faulted || self.service.mode == mode {
            return false;
        }
        if self.service.active.len() == 0 {
            self.service.mode = mode;
            return true;
        }
        let before = self.service.damage_state(self.tree);
        self.service.mode = mode;
        if mode != AnimationMode::Enabled {
            let active: Vec<_> = self.service.active.iter().map(|(id, _)| id).collect();
            for id in active {
                self.service.finish(id, AnimationState::Cancelled);
            }
        }
        self.reconcile_damage(before);
        true
    }

    /// Registers a typed committed value as an animation presentation channel.
    pub fn create_channel<V: AnimationValue>(
        &mut self,
        committed: V,
    ) -> Result<AnimationChannel<V>, AnimationError> {
        self.ensure_not_faulted()?;
        if !committed.is_valid() {
            return Err(AnimationError::InvalidValue);
        }
        if self.service.channels.len() >= DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        Ok(AnimationChannel::new(
            self.service.channels.insert(ChannelState::new(committed)),
        ))
    }

    /// Replaces the committed final value without mutating an active presentation override.
    pub fn set_committed<V: AnimationValue>(
        &mut self,
        channel: AnimationChannel<V>,
        committed: V,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        let state = self.channel_mut(channel)?;
        if state.binding != ChannelBinding::Detached {
            return Err(AnimationError::ManagedChannel);
        }
        state.set_committed(committed)
    }

    /// Removes a typed channel, cancelling the complete animation that owns it.
    pub fn remove_channel<V: AnimationValue>(
        &mut self,
        channel: AnimationChannel<V>,
    ) -> Result<V, AnimationError> {
        self.ensure_not_faulted()?;
        let channel_id = channel.id();
        if channel_id.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignChannel);
        }
        if !self.service.channels.contains_key(channel_id) {
            return Err(AnimationError::UnknownChannel);
        }
        if self
            .service
            .channels
            .get(channel_id)
            .is_some_and(|channel| channel.binding != ChannelBinding::Detached)
        {
            return Err(AnimationError::ManagedChannel);
        }
        if let Some(owner) = self.service.channel_owners.get(&channel_id).copied() {
            self.service.finish(owner, AnimationState::Cancelled);
        }
        self.service
            .channels
            .remove(channel_id)
            .expect("validated channel must remain registered")
            .into_committed()
    }

    /// Creates or returns the Runtime-managed opacity and transform channels for a Figure.
    ///
    /// M01-B keeps input and accessibility on committed geometry. Presentation hit-testing is
    /// admitted by a later capability slice.
    pub fn bind_figure(
        &mut self,
        figure: FigureId,
        interaction: InteractionGeometryPolicy,
    ) -> Result<FigurePresentationChannels, AnimationError> {
        self.ensure_not_faulted()?;
        if interaction != InteractionGeometryPolicy::Committed {
            return Err(AnimationError::UnsupportedInteractionGeometry);
        }
        if figure.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignTarget);
        }
        if !self.tree.is_attached(figure) {
            return Err(AnimationError::DisposedTarget);
        }
        if let Some(channels) = self.service.figure_channels.get(&figure) {
            return Ok(FigurePresentationChannels::new(
                figure,
                AnimationChannel::new(channels.opacity),
                AnimationChannel::new(channels.transform),
            ));
        }
        if self.service.channels.len().saturating_add(2) > DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        let opacity = self
            .tree
            .resolved_style(figure)
            .ok_or(AnimationError::DisposedTarget)?
            .alpha;
        let opacity = self.service.channels.insert(ChannelState::new_bound(
            Opacity::try_new(opacity)?,
            ChannelBinding::FigureOpacity(figure),
        ));
        let transform = self.service.channels.insert(ChannelState::new_bound(
            Affine2D::IDENTITY,
            ChannelBinding::FigureTransform(figure),
        ));
        self.service
            .figure_channels
            .insert(figure, FigureChannels { opacity, transform });
        Ok(FigurePresentationChannels::new(
            figure,
            AnimationChannel::new(opacity),
            AnimationChannel::new(transform),
        ))
    }

    /// Captures committed Figure bounds in stable input order.
    pub fn capture_figures(
        &self,
        figures: impl IntoIterator<Item = FigureId>,
    ) -> Result<FigureTransitionCapture, AnimationError> {
        self.ensure_not_faulted()?;
        let mut seen = HashSet::new();
        let mut bounds = Vec::new();
        for figure in figures {
            if figure.namespace() != self.service.namespace {
                return Err(AnimationError::ForeignTarget);
            }
            if !seen.insert(figure) {
                return Err(AnimationError::DuplicateTarget);
            }
            let bounds_value = self
                .tree
                .figure_bounds(figure)
                .ok_or(AnimationError::DisposedTarget)?;
            bounds.push((figure, bounds_value));
        }
        if bounds.is_empty() {
            return Err(AnimationError::EmptyCapture);
        }
        Ok(FigureTransitionCapture {
            namespace: self.service.namespace,
            bounds,
        })
    }

    /// Animates captured Figure bounds toward their already committed current bounds.
    pub fn transition_bounds(
        &mut self,
        capture: FigureTransitionCapture,
        transition: BoundsTransition,
    ) -> Result<AnimationStart, AnimationError> {
        self.ensure_not_faulted()?;
        if capture.namespace != self.service.namespace {
            return Err(AnimationError::ForeignCapture);
        }
        let duration_micros = u64::try_from(transition.duration.as_micros())
            .map_err(|_| AnimationError::InvalidDuration)?;
        let stagger_micros = u64::try_from(transition.stagger.as_micros())
            .map_err(|_| AnimationError::TimeOverflow)?;
        if duration_micros == 0 {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }

        let mut targets = Vec::new();
        for (figure, before) in capture.bounds {
            let after = self
                .tree
                .figure_bounds(figure)
                .ok_or(AnimationError::DisposedTarget)?;
            if before == after {
                continue;
            }
            targets.push((figure, bounds_transition_transform(before, after)?));
        }
        if targets.is_empty() {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }
        let last_index =
            u64::try_from(targets.len() - 1).map_err(|_| AnimationError::TimeOverflow)?;
        stagger_micros
            .checked_mul(last_index)
            .and_then(|offset| offset.checked_add(duration_micros))
            .ok_or(AnimationError::TimeOverflow)?;

        let new_channel_count = targets
            .iter()
            .filter(|(figure, _)| !self.service.figure_channels.contains_key(figure))
            .count()
            .saturating_mul(2);
        if self
            .service
            .channels
            .len()
            .saturating_add(new_channel_count)
            > DEFAULT_MAX_ANIMATION_CHANNELS
        {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        self.service.validate_active_budget(targets.len(), 0)?;

        let mut plans = Vec::with_capacity(targets.len());
        for (figure, start) in targets {
            let channel = self
                .bind_figure(figure, InteractionGeometryPolicy::Committed)?
                .transform();
            plans.push(AnimationPlan::track(
                channel,
                Motion::Tween(
                    Tween::between(start, Affine2D::IDENTITY, transition.duration)?
                        .with_easing(transition.easing),
                ),
            )?);
        }
        let plan = if transition.stagger.is_zero() {
            AnimationPlan::parallel(plans)?
        } else {
            AnimationPlan::stagger(transition.stagger, plans)?
        }
        .with_interruption(transition.interruption)
        .with_suspension(transition.suspension);
        self.start(plan)
    }

    /// Creates a dormant non-interactive visual that activates atomically with its first plan.
    pub fn create_temporary_visual(
        &mut self,
        presentation: FigurePresentation,
        committed_transform: Affine2D,
        committed_opacity: f64,
    ) -> Result<TemporaryVisual, AnimationError> {
        self.ensure_not_faulted()?;
        if !committed_transform.is_valid()
            || !committed_opacity.is_finite()
            || !(0.0..=1.0).contains(&committed_opacity)
            || transformed_envelope(committed_transform, presentation.visual_bounds()).is_none()
        {
            return Err(AnimationError::InvalidValue);
        }
        if self.service.temporary_visuals.len() >= DEFAULT_MAX_TEMPORARY_VISUALS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::TemporaryVisuals,
            ));
        }
        if self.service.channels.len().saturating_add(2) > DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        let id = self.service.temporary_visuals.insert(TemporaryVisualState {
            presentation,
            opacity: None,
            transform: None,
            owner: None,
        });
        let opacity = self.service.channels.insert(ChannelState::new_bound(
            Opacity::try_new(committed_opacity)?,
            ChannelBinding::TemporaryOpacity(id),
        ));
        let transform = self.service.channels.insert(ChannelState::new_bound(
            committed_transform,
            ChannelBinding::TemporaryTransform(id),
        ));
        let visual = self
            .service
            .temporary_visuals
            .get_mut(id)
            .expect("new temporary visual must remain registered");
        visual.opacity = Some(opacity);
        visual.transform = Some(transform);
        Ok(TemporaryVisual::new(
            id,
            AnimationChannel::new(opacity),
            AnimationChannel::new(transform),
        ))
    }

    /// Removes a dormant visual or cancels the complete active plan that owns it.
    pub fn remove_temporary_visual(
        &mut self,
        visual: TemporaryVisualId,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        if visual.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignTarget);
        }
        let owner = self
            .service
            .temporary_visuals
            .get(visual)
            .ok_or(AnimationError::UnknownTemporaryVisual)?
            .owner;
        let before = self.service.damage_state(self.tree);
        let changed = if let Some(owner) = owner {
            self.service.finish(owner, AnimationState::Cancelled)
        } else {
            self.service.remove_temporary_visual_state(visual)
        };
        self.reconcile_damage(before);
        Ok(changed)
    }

    /// Returns the current presentation value, or the committed value when no override exists.
    pub fn value<V: AnimationValue>(
        &self,
        channel: AnimationChannel<V>,
    ) -> Result<V, AnimationError> {
        Ok(self.channel(channel)?.effective::<V>()?.clone())
    }

    /// Returns whether the channel currently has a presentation override.
    pub fn has_override<V>(&self, channel: AnimationChannel<V>) -> Result<bool, AnimationError> {
        Ok(self.channel(channel)?.has_override())
    }

    /// Admits and starts a finite plan.
    pub fn start(&mut self, plan: AnimationPlan) -> Result<AnimationStart, AnimationError> {
        if *self.faulted {
            return Err(AnimationError::RuntimeFaulted);
        }
        let before = self.service.damage_state(self.tree);
        let result = self.guarded_service(|service| service.start(plan));
        if result.is_ok() {
            self.reconcile_damage(before);
        }
        result
    }

    /// Atomically replaces one active plan while preserving current presentation values.
    pub fn retarget(
        &mut self,
        animation: AnimationId,
        replacement: AnimationPlan,
    ) -> Result<bool, AnimationError> {
        if *self.faulted {
            return Err(AnimationError::RuntimeFaulted);
        }
        let before = self.service.damage_state(self.tree);
        let result = self.guarded_service(|service| service.retarget(animation, replacement));
        if result.is_ok() {
            self.reconcile_damage(before);
        }
        result
    }

    /// Cancels an active plan and clears its presentation overrides.
    pub fn cancel(&mut self, animation: AnimationId) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        let before = self.service.damage_state(self.tree);
        let result = self.service.cancel(animation);
        if result.is_ok() {
            self.reconcile_damage(before);
        }
        result
    }

    /// Queries active or bounded terminal state without advancing time.
    pub fn state(&self, animation: AnimationId) -> Result<AnimationState, AnimationError> {
        self.service.validate_animation_id(animation)?;
        if let Some(active) = self.service.active.get(animation) {
            return Ok(if active.paused_at.is_some() {
                AnimationState::Paused
            } else if active.start_time.is_some() {
                AnimationState::Running
            } else {
                AnimationState::Scheduled
            });
        }
        self.service
            .terminal
            .iter()
            .rev()
            .find_map(|(candidate, state)| (*candidate == animation).then_some(*state))
            .ok_or(AnimationError::UnknownAnimation)
    }

    /// Returns the earliest animation wake deadline.
    pub fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        self.service.next_wake_deadline()
    }

    /// Returns the number of active timeline owners.
    pub fn active_animation_count(&self) -> usize {
        self.service.active.len()
    }

    /// Returns the number of dormant or active temporary visuals.
    pub fn temporary_visual_count(&self) -> usize {
        self.service.temporary_visuals.len()
    }

    /// Returns cumulative animation work counters.
    pub fn stats(&self) -> AnimationStats {
        self.service.stats
    }

    fn channel<V>(&self, channel: AnimationChannel<V>) -> Result<&ChannelState, AnimationError> {
        if channel.id().namespace() != self.service.namespace {
            return Err(AnimationError::ForeignChannel);
        }
        self.service
            .channels
            .get(channel.id())
            .ok_or(AnimationError::UnknownChannel)
    }

    fn channel_mut<V>(
        &mut self,
        channel: AnimationChannel<V>,
    ) -> Result<&mut ChannelState, AnimationError> {
        if channel.id().namespace() != self.service.namespace {
            return Err(AnimationError::ForeignChannel);
        }
        self.service
            .channels
            .get_mut(channel.id())
            .ok_or(AnimationError::UnknownChannel)
    }

    fn ensure_not_faulted(&self) -> Result<(), AnimationError> {
        if *self.faulted {
            Err(AnimationError::RuntimeFaulted)
        } else {
            Ok(())
        }
    }

    fn reconcile_damage(&mut self, before: HashMap<DamageSubject, DamageSubjectState>) -> bool {
        let after = self.service.damage_state(self.tree);
        let subjects: HashSet<_> = before.keys().chain(after.keys()).copied().collect();
        let mut changed = false;
        for subject in subjects {
            let old = before.get(&subject);
            let new = after.get(&subject);
            if old == new {
                continue;
            }
            changed = true;
            let fallback = match subject {
                DamageSubject::Figure(figure) => {
                    self.tree.presentation_envelope(figure, Affine2D::IDENTITY)
                }
                DamageSubject::Temporary(_) => None,
            };
            let old_envelope = old
                .and_then(DamageSubjectState::visible_envelope)
                .or(fallback);
            let new_envelope = new
                .and_then(DamageSubjectState::visible_envelope)
                .or(fallback);
            if old_envelope.is_none() && new_envelope.is_none() {
                *self.full_redraw_pending = true;
            }
            if let Some(envelope) = old_envelope {
                self.updates.add_frozen_surface_region(envelope);
            }
            if let Some(envelope) = new_envelope {
                self.updates.add_frozen_surface_region(envelope);
            }
        }
        changed
    }

    fn guarded_service<T>(&mut self, operation: impl FnOnce(&mut AnimationService) -> T) -> T {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self.service))) {
            Ok(value) => value,
            Err(payload) => {
                self.service.clear_presentation();
                *self.faulted = true;
                std::panic::resume_unwind(payload)
            }
        }
    }
}

impl DamageSubjectState {
    fn visible_envelope(&self) -> Option<Rectangle> {
        match *self {
            Self::Figure { envelope, .. } => envelope,
            Self::Temporary {
                envelope,
                active: true,
                ..
            } => envelope,
            Self::Temporary { active: false, .. } => None,
        }
    }
}
