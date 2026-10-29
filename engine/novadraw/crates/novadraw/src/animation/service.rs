use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use crate::{
    Affine2D, CapabilityKey, Color, Dimension, Figure, FigureId, FigureMeasurement, FigureTree,
    MonotonicTime, NotificationEffect, PointList, Rectangle, StableSceneQuery, TimeError,
    UpdateManager,
    figure::{FigureDrawing, FigurePresentation},
    graphics::{GraphicsError, PaintContext},
    identity::{
        AnimationBehaviorId, AnimationChannelId, AnimationId, RuntimeArena, RuntimeNamespace,
        TemporaryVisualId,
    },
};

use super::{
    AnimationBehavior, AnimationBehaviorFailure, AnimationBehaviorScope, AnimationBudgetKind,
    AnimationChannel, AnimationError, AnimationLimits, AnimationMode, AnimationReservations,
    AnimationSampleVersion, AnimationStart, AnimationState, AnimationStats, AnimationSuppression,
    AnimationValue, BoundsTransition, CONNECTION_DASH_PRESENTATION, CONNECTION_ROUTE_PRESENTATION,
    ConnectionPulse, DEFAULT_ANIMATION_TERMINAL_HISTORY, DEFAULT_MAX_ANIMATION_BEHAVIORS,
    DEFAULT_MAX_ANIMATION_CHANNELS, DEFAULT_MAX_TEMPORARY_VISUALS, FigurePresentationChannels,
    FigureTransitionCapture, InteractionGeometryPolicy, InterruptionPolicy, Motion, Opacity,
    PresentationBinding, PresentationValues, Procedural, SuspensionPolicy, TemporaryVisual, Tween,
    facts_from_effects,
    timeline::{AnimationPlan, PreparedPlanTrack},
};

enum ChannelBinding {
    Detached,
    FigureGeometry {
        figure: FigureId,
        value_type: TypeId,
        binding: Arc<dyn super::geometry::ErasedGeometryBinding>,
        fields: super::geometry::GeometryFields,
    },
    FigureOpacity(FigureId),
    FigureTransform(FigureId),
    FigurePresentation {
        figure: FigureId,
        binding: Arc<dyn ErasedPresentationBinding>,
    },
    TemporaryOpacity(TemporaryVisualId),
    TemporaryTransform(TemporaryVisualId),
}

trait ErasedPresentationBinding: Send + Sync {
    fn value_type_id(&self) -> TypeId;
    fn family(&self) -> TypeId;

    fn prepare(
        &self,
        figure: &dyn Figure,
        bounds: Rectangle,
        values: PresentationValues<'_>,
    ) -> Option<FigurePresentation>;
}

struct TypedPresentationBinding<V> {
    binding: PresentationBinding<V>,
}

impl<V> ErasedPresentationBinding for TypedPresentationBinding<V>
where
    V: AnimationValue,
{
    fn value_type_id(&self) -> TypeId {
        TypeId::of::<V>()
    }

    fn family(&self) -> TypeId {
        self.binding.family()
    }

    fn prepare(
        &self,
        figure: &dyn Figure,
        bounds: Rectangle,
        values: PresentationValues<'_>,
    ) -> Option<FigurePresentation> {
        self.binding.prepare(figure, values, bounds)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct FigurePresentationChannelKey {
    figure: FigureId,
    value_type: TypeId,
}

pub(crate) struct ChannelState {
    committed: Box<dyn Any + Send + Sync>,
    presentation: Option<Box<dyn Any + Send + Sync>>,
    binding: ChannelBinding,
    presentation_revision: u64,
    presentation_patch: super::GeometryPatch,
}

/// A validated value awaiting the atomic publication of its complete group.
pub(crate) struct PreparedChannelValue {
    value: Box<dyn Any + Send + Sync>,
    changed: bool,
    patch: super::GeometryPatch,
}

impl ChannelState {
    pub(crate) fn supports_geometry_endpoint(&self) -> bool {
        matches!(self.binding, ChannelBinding::FigureGeometry { .. })
    }

    fn new<V: AnimationValue>(value: V) -> Self {
        Self::new_bound(value, ChannelBinding::Detached)
    }

    fn new_bound<V: AnimationValue>(value: V, binding: ChannelBinding) -> Self {
        Self {
            committed: Box::new(value),
            presentation: None,
            binding,
            presentation_revision: 0,
            presentation_patch: super::GeometryPatch::default(),
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

    pub(crate) fn prepare_override<V: AnimationValue>(
        &self,
        value: V,
    ) -> Result<PreparedChannelValue, AnimationError> {
        let patch = self.validated_patch(&value)?;
        let changed = self.effective::<V>()? != &value;
        Ok(PreparedChannelValue {
            value: Box::new(value),
            changed,
            patch,
        })
    }

    fn publish_override(&mut self, value: PreparedChannelValue) -> bool {
        self.presentation = Some(value.value);
        self.presentation_patch = value.patch;
        if value.changed {
            self.presentation_revision = self.presentation_revision.saturating_add(1);
        }
        value.changed
    }

    pub(crate) fn validate_value<V: AnimationValue>(
        &self,
        value: &V,
    ) -> Result<(), AnimationError> {
        self.validated_patch(value).map(|_| ())
    }

    fn validated_patch<V: AnimationValue>(
        &self,
        value: &V,
    ) -> Result<super::GeometryPatch, AnimationError> {
        let mut patch = super::GeometryPatch::default();
        if let ChannelBinding::FigureGeometry {
            binding, fields, ..
        } = &self.binding
        {
            patch = binding.patch(value)?;
            if patch.fields() != *fields {
                return Err(AnimationError::InvalidValue);
            }
        }
        if value.is_valid() && self.binding.accepts(value) {
            Ok(patch)
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
        self.presentation_patch = super::GeometryPatch::default();
        let changed = self.presentation.take().is_some();
        if changed {
            self.presentation_revision = self.presentation_revision.saturating_add(1);
        }
        changed
    }

    fn has_override(&self) -> bool {
        self.presentation.is_some()
    }

    fn effective_any(&self) -> &dyn Any {
        self.presentation
            .as_deref()
            .unwrap_or(self.committed.as_ref())
    }

    fn into_committed<V: AnimationValue>(self) -> Result<V, AnimationError> {
        self.committed
            .downcast::<V>()
            .map(|value| *value)
            .map_err(|_| AnimationError::InvalidValue)
    }
}

impl ChannelBinding {
    fn accepts<V: AnimationValue>(&self, value: &V) -> bool {
        match self {
            Self::FigureOpacity(_) | Self::TemporaryOpacity(_) => (value as &dyn Any)
                .downcast_ref::<Opacity>()
                .is_some_and(AnimationValue::is_valid),
            Self::FigureTransform(_) | Self::TemporaryTransform(_) => {
                (value as &dyn Any).downcast_ref::<Affine2D>().is_some()
            }
            Self::FigurePresentation { binding, .. } => {
                binding.value_type_id() == TypeId::of::<V>()
            }
            Self::FigureGeometry { value_type, .. } => *value_type == TypeId::of::<V>(),
            Self::Detached => true,
        }
    }

    fn figure(&self) -> Option<FigureId> {
        match self {
            Self::FigureOpacity(figure)
            | Self::FigureTransform(figure)
            | Self::FigureGeometry { figure, .. }
            | Self::FigurePresentation { figure, .. } => Some(*figure),
            _ => None,
        }
    }

    fn temporary(&self) -> Option<TemporaryVisualId> {
        match self {
            Self::TemporaryOpacity(visual) | Self::TemporaryTransform(visual) => Some(*visual),
            _ => None,
        }
    }

    fn is_detached(&self) -> bool {
        matches!(self, Self::Detached)
    }

    fn figure_presentation(&self) -> Option<(FigureId, &Arc<dyn ErasedPresentationBinding>)> {
        match self {
            Self::FigurePresentation { figure, binding } => Some((*figure, binding)),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct FigurePresentationEffect {
    pub(crate) opacity: Option<Opacity>,
    pub(crate) transform: Option<Affine2D>,
    pub(crate) content: Option<FigurePresentation>,
    pub(crate) geometry: super::GeometryPatch,
}

#[derive(Clone)]
pub(crate) struct TemporaryVisualSnapshot {
    pub(crate) id: TemporaryVisualId,
    pub(crate) presentation: FigurePresentation,
    pub(crate) opacity: Opacity,
    pub(crate) transform: Affine2D,
}

#[derive(Clone, Default)]
pub(crate) struct PresentationSnapshot {
    figures: HashMap<FigureId, FigurePresentationEffect>,
    temporaries: Vec<TemporaryVisualSnapshot>,
    pub(crate) geometry: Option<Arc<super::PresentationGeometry>>,
}

impl PresentationSnapshot {
    pub(crate) fn figure(&self, id: FigureId) -> Option<FigurePresentationEffect> {
        self.figures.get(&id).cloned()
    }

    pub(crate) fn paint_temporaries(&self, canvas: &mut crate::NdCanvas) {
        for visual in &self.temporaries {
            canvas.push_state();
            let [a, b, c, d, e, f] = visual.transform.coeffs();
            canvas.transform(a, b, c, d, e, f);
            canvas.global_alpha(visual.opacity);
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

struct PulseDrawing {
    radius: f64,
    color: Color,
}

impl FigureDrawing for PulseDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.canvas.ellipse_with_style(
            0.0,
            0.0,
            self.radius,
            self.radius,
            Some(self.color),
            None,
            crate::graphics::StrokeStyle::default(),
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum DamageSubject {
    Figure(FigureId),
    Temporary(TemporaryVisualId),
}

#[derive(Clone, Debug, PartialEq)]
enum DamageSubjectState {
    Figure {
        opacity: f64,
        transform: Affine2D,
        content_revision: Option<u64>,
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
    geometry_source: Option<Arc<super::frozen::FrozenGeometrySource>>,
    work: super::AnimationWork,
    providers: super::provider::ProviderGraph,
    derived_fields: HashMap<FigureId, super::geometry::GeometryFields>,
}

struct PreparedAnimation {
    tracks: Vec<PreparedPlanTrack>,
    initial_values: Vec<(AnimationChannelId, PreparedChannelValue)>,
    duration_micros: u64,
    interruption: InterruptionPolicy,
    suspension: SuspensionPolicy,
    geometry_source: Option<Arc<super::frozen::FrozenGeometrySource>>,
    work: super::AnimationWork,
    providers: super::provider::ProviderGraph,
    derived_fields: HashMap<FigureId, super::geometry::GeometryFields>,
}

struct RecordedGeometry {
    version: super::PresentationVersion,
    envelopes: Vec<(FigureId, Option<crate::Rectangle>)>,
}

pub(crate) struct AnimationService {
    namespace: RuntimeNamespace,
    mode: AnimationMode,
    now: MonotonicTime,
    time_initialized: bool,
    surface_suspended: bool,
    channels: RuntimeArena<AnimationChannelId, ChannelState>,
    active: RuntimeArena<AnimationId, ActiveAnimation>,
    behaviors: RuntimeArena<AnimationBehaviorId, AnimationBehavior>,
    behavior_failures: VecDeque<AnimationBehaviorFailure>,
    figure_channels: HashMap<FigureId, FigureChannels>,
    presentation_channels: HashMap<FigurePresentationChannelKey, AnimationChannelId>,
    geometry_channels: HashMap<FigurePresentationChannelKey, AnimationChannelId>,
    temporary_visuals: RuntimeArena<TemporaryVisualId, TemporaryVisualState>,
    channel_owners: HashMap<AnimationChannelId, AnimationId>,
    terminal: VecDeque<(AnimationId, AnimationState)>,
    failures: HashMap<AnimationId, AnimationError>,
    active_track_count: usize,
    limits: AnimationLimits,
    capture_storage: super::storage::CaptureStorage,
    submission_storage: super::storage::CaptureStorage,
    active_revision: u64,
    binding_revision: u64,
    geometry_cache: Option<Arc<super::PresentationGeometry>>,
    snapshot_cache: Option<(super::PresentationVersion, Option<PresentationSnapshot>)>,
    recorded_geometry: Option<RecordedGeometry>,
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
            behaviors: RuntimeArena::new(namespace),
            behavior_failures: VecDeque::new(),
            figure_channels: HashMap::new(),
            presentation_channels: HashMap::new(),
            geometry_channels: HashMap::new(),
            temporary_visuals: RuntimeArena::new(namespace),
            channel_owners: HashMap::new(),
            terminal: VecDeque::new(),
            failures: HashMap::new(),
            active_track_count: 0,
            limits: AnimationLimits::default(),
            capture_storage: super::storage::CaptureStorage::new(
                AnimationLimits::default().capture_bytes,
            ),
            submission_storage: super::storage::CaptureStorage::new(
                AnimationLimits::default().submission_bytes,
            ),
            active_revision: 0,
            binding_revision: 0,
            geometry_cache: None,
            snapshot_cache: None,
            recorded_geometry: None,
            stats: AnimationStats::default(),
        }
    }

    pub(crate) fn access_mut<'a>(
        &'a mut self,
        tree: &'a FigureTree,
        resources: &'a crate::runtime::ResourceRegistry,
        updates: &'a mut UpdateManager,
        full_redraw_pending: &'a mut bool,
        faulted: &'a mut bool,
    ) -> AnimationMut<'a> {
        AnimationMut {
            service: self,
            tree,
            resources,
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
        // Admission and retarget already initialize their samples at the current time.
        // Repeating that time must not invoke extension Motion code again.
        if self.time_initialized && now == self.now {
            return Ok(false);
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
                Err(error) => {
                    failed.push((id, error));
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
        for (id, error) in failed {
            changed |= self.finish_failed(id, error);
        }
        Ok(changed)
    }

    pub(crate) fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        self.active
            .iter()
            .any(|(_, animation)| animation.paused_at.is_none())
            .then_some(self.now)
    }

    pub(crate) fn sample_version(&self) -> AnimationSampleVersion {
        AnimationSampleVersion {
            time: self.time_initialized.then_some(self.now),
            active_revision: self.active_revision,
        }
    }

    pub(crate) fn geometry(
        &self,
        source_epoch: u64,
    ) -> Result<&super::PresentationGeometry, AnimationError> {
        let geometry = self
            .geometry_cache
            .as_deref()
            .ok_or(AnimationError::GeometryNotPrepared)?;
        if geometry.version() != self.geometry_version(source_epoch) {
            return Err(AnimationError::GeometryNotPrepared);
        }
        Ok(geometry)
    }

    pub(crate) fn capture_geometry(
        &mut self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
        source_epoch: u64,
        dependencies: Vec<FigureId>,
    ) -> Result<super::GeometryCapture, AnimationError> {
        if dependencies.is_empty() {
            return Err(AnimationError::EmptyCapture);
        }
        let mut unique = HashSet::new();
        for figure in &dependencies {
            if figure.namespace() != self.namespace {
                return Err(AnimationError::ForeignTarget);
            }
            tree.node(*figure)?;
            if !unique.insert(*figure) {
                return Err(AnimationError::DuplicateTarget);
            }
        }
        let snapshot = self.snapshot(tree, resources, source_epoch)?;
        if let Ok(geometry) = self.geometry(source_epoch)
            && geometry.sources_valid(tree, resources)
            && dependencies
                .iter()
                .all(|figure| geometry.data.covered.contains(figure))
        {
            let geometry = self
                .geometry_cache
                .as_ref()
                .expect("validated cache")
                .clone();
            let retention = self.capture_storage.acquire(geometry.collection_bytes()?)?;
            return Ok(super::GeometryCapture {
                geometry,
                _retention: Arc::new(retention),
            });
        }
        // Establish ownership before source/content allocations. The frozen recorder receives
        // the same finite bound; unused capacity is returned only after the candidate is complete.
        let reserved_bytes = self
            .capture_storage
            .available_bytes()
            .min(self.limits.staging_work.bytes);
        let mut retention = self.capture_storage.acquire(reserved_bytes)?;
        let preflight = super::frozen::FrozenGeometrySource::capture(
            tree,
            resources,
            dependencies.clone(),
            super::AnimationWork {
                bytes: reserved_bytes,
                ..self.limits.staging_work
            },
        )?;
        let preflight_work = super::AnimationWork {
            nodes: preflight.nodes.len(),
            bytes: preflight.collection_bytes()?,
            operations: preflight
                .nodes
                .len()
                .checked_mul(super::capacity::GEOMETRY_OPERATIONS_PER_NODE)
                .ok_or(AnimationError::BudgetExceeded(
                    AnimationBudgetKind::GeometryOperations,
                ))?,
            ..super::AnimationWork::default()
        };
        preflight_work.validate(self.limits.staging_work)?;
        let empty = HashMap::new();
        let effects = snapshot
            .as_ref()
            .map(|snapshot| &snapshot.figures)
            .unwrap_or(&empty);
        let mut sources = self.geometry_sources();
        sources.push(preflight);
        // The frozen source and candidate geometry coexist. Check the complete combined
        // closure before allocating candidate maps or materializing bounded self-paint.
        let mut peak_bytes = 0usize;
        let mut seen = HashSet::new();
        let mut combined_nodes = HashSet::new();
        let mut combined_edges = 0usize;
        for source in &sources {
            if seen.insert(Arc::as_ptr(source)) {
                for (id, node) in &source.nodes {
                    if combined_nodes.insert(*id) {
                        if combined_nodes.len() > self.limits.staging_work.nodes {
                            return Err(AnimationError::BudgetExceeded(
                                AnimationBudgetKind::GeometryNodes,
                            ));
                        }
                        combined_edges = combined_edges.checked_add(node.children.len()).ok_or(
                            AnimationError::BudgetExceeded(AnimationBudgetKind::DependencyEdges),
                        )?;
                        if combined_edges > self.limits.staging_work.edges {
                            return Err(AnimationError::BudgetExceeded(
                                AnimationBudgetKind::DependencyEdges,
                            ));
                        }
                    }
                }
                let geometry_bytes = source.materialization_bytes()?;
                peak_bytes = peak_bytes
                    .checked_add(source.collection_bytes()?)
                    .and_then(|bytes| bytes.checked_add(geometry_bytes))
                    .ok_or(AnimationError::BudgetExceeded(
                        AnimationBudgetKind::SnapshotBytes,
                    ))?;
            }
        }
        let combined_operations = combined_nodes
            .len()
            .checked_mul(super::capacity::GEOMETRY_OPERATIONS_PER_NODE)
            .and_then(|n| n.checked_add(combined_edges))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::GeometryOperations,
            ))?;
        if combined_operations > self.limits.staging_work.operations {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::GeometryOperations,
            ));
        }
        let value_slots = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.geometry.as_ref())
            .map_or(self.geometry_channels.len(), |geometry| {
                geometry.data.values.len()
            });
        let mut add_value = |value: &dyn Any| -> Result<(), AnimationError> {
            peak_bytes = peak_bytes
                .checked_add(std::mem::size_of_val(value) + 2 * std::mem::size_of::<usize>())
                .ok_or(AnimationError::BudgetExceeded(
                    AnimationBudgetKind::SnapshotBytes,
                ))?;
            Ok(())
        };
        if let Some(geometry) = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.geometry.as_ref())
        {
            for value in geometry.data.values.values() {
                add_value(value.as_ref())?;
            }
        } else {
            for channel in self.geometry_channels.values() {
                if let Some(channel) = self.channels.get(*channel) {
                    add_value(channel.effective_any())?;
                }
            }
        }
        let value_bytes = value_slots
            .checked_mul(
                4 * (std::mem::size_of::<((FigureId, TypeId), Arc<dyn Any + Send + Sync>)>() + 1),
            )
            .and_then(|bytes| peak_bytes.checked_add(bytes))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ))?;
        if value_bytes > reserved_bytes {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ));
        }
        let values = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.geometry.as_ref())
            .map(|geometry| geometry.data.values.clone())
            .map_or_else(|| self.geometry_values(), Ok)?;
        // Active sources keep their sampled content. Newly captured static dependencies
        // use the immutable inputs already checked above, rather than live Figure paint.
        let dependencies = dependencies
            .into_iter()
            .chain(effects.keys().copied())
            .chain(
                sources
                    .iter()
                    .flat_map(|source| source.roots.iter().copied()),
            );
        let mut geometry = super::PresentationGeometry::freeze_sources(
            tree,
            effects,
            dependencies,
            self.geometry_version(source_epoch),
            values,
            &sources,
        )?;
        retention.shrink_to(geometry.collection_bytes()?)?;
        let retention = Arc::new(retention);
        geometry.retain_capture(retention.clone());
        let geometry = Arc::new(geometry);
        self.geometry_cache = Some(geometry.clone());
        Ok(super::GeometryCapture {
            geometry,
            _retention: retention,
        })
    }

    fn geometry_values(&self) -> Result<super::geometry_view::GeometryValues, AnimationError> {
        let mut values = HashMap::new();
        for (key, channel) in &self.geometry_channels {
            let Some(channel) = self.channels.get(*channel) else {
                continue;
            };
            if let ChannelBinding::FigureGeometry { binding, .. } = &channel.binding {
                values.insert(
                    (key.figure, key.value_type),
                    binding.clone_value(channel.effective_any())?,
                );
            }
        }
        Ok(values)
    }

    fn geometry_sources(&self) -> Vec<Arc<super::frozen::FrozenGeometrySource>> {
        self.active
            .iter()
            .filter_map(|(_, group)| group.geometry_source.clone())
            .collect()
    }

    /// Compares against the last recorded geometry, never against a capture-only materialization.
    pub(crate) fn geometry_damage(
        &self,
        tree: &FigureTree,
        source_epoch: u64,
        snapshot: Option<&PresentationSnapshot>,
        updates: &mut UpdateManager,
    ) -> Result<(), AnimationError> {
        let old = self.recorded_geometry.as_ref();
        let new = snapshot.and_then(|snapshot| snapshot.geometry.as_deref());
        if old.is_none() && new.is_none() {
            return Ok(());
        }
        if let (Some(old), Some(new)) = (old, new)
            && old.version == new.version()
        {
            return Ok(());
        }
        let fallback;
        let new = if new.is_none() {
            let roots = old
                .expect("old geometry exists")
                .envelopes
                .iter()
                .map(|(figure, _)| *figure)
                .filter(|figure| tree.node(*figure).is_ok());
            fallback = super::PresentationGeometry::freeze(
                tree,
                &HashMap::new(),
                roots,
                self.geometry_version(source_epoch),
                HashMap::new(),
            )?;
            Some(&fallback)
        } else {
            new
        };
        if let Some(old) = old {
            for (_, envelope) in &old.envelopes {
                if let Some(envelope) = envelope {
                    updates.add_frozen_surface_region(*envelope);
                }
            }
        }
        if let Some(view) = new {
            for root in &view.data.roots {
                if let Some(envelope) = view
                    .data
                    .nodes
                    .get(root)
                    .and_then(|geometry| geometry.visual_envelope)
                {
                    updates.add_frozen_surface_region(envelope);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn record_geometry(&mut self, snapshot: Option<&PresentationSnapshot>) {
        self.recorded_geometry = snapshot
            .and_then(|snapshot| snapshot.geometry.as_ref())
            .map(|geometry| RecordedGeometry {
                version: geometry.version(),
                envelopes: geometry
                    .data
                    .roots
                    .iter()
                    .map(|root| {
                        (
                            *root,
                            geometry
                                .data
                                .nodes
                                .get(root)
                                .and_then(|node| node.visual_envelope),
                        )
                    })
                    .collect(),
            });
    }

    pub(crate) fn retain_submission(
        &self,
        snapshot: Option<&PresentationSnapshot>,
    ) -> Result<Option<crate::render::submission::SubmissionRetention>, AnimationError> {
        let Some(geometry) = snapshot.and_then(|snapshot| snapshot.geometry.as_ref()) else {
            return Ok(None);
        };
        let bytes = geometry.collection_bytes()?;
        let lease = self.submission_storage.acquire(bytes)?;
        Ok(Some(crate::render::submission::SubmissionRetention::new(
            bytes,
            geometry.clone(),
            lease,
        )))
    }

    fn geometry_version(&self, source_epoch: u64) -> super::PresentationVersion {
        super::PresentationVersion {
            sample: self.sample_version(),
            source_epoch,
            binding_revision: self.binding_revision,
        }
    }

    pub(crate) fn snapshot(
        &mut self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
        source_epoch: u64,
    ) -> Result<Option<PresentationSnapshot>, AnimationError> {
        let mut successful = HashMap::new();
        loop {
            let result =
                self.snapshot_with_provider_cache(tree, resources, source_epoch, &mut successful);
            let Err(
                error @ AnimationError::ProviderOutputExceeded {
                    figure, value_type, ..
                },
            ) = &result
            else {
                return result;
            };
            if *value_type != TypeId::of::<crate::figure::BoundsDrawing>() {
                return result;
            }
            let failed: Vec<_> = self
                .active
                .iter()
                .filter_map(|(id, group)| {
                    group
                        .geometry_source
                        .as_ref()
                        .is_some_and(|source| source.nodes.contains_key(figure))
                        .then_some(id)
                })
                .collect();
            if failed.is_empty() {
                return result;
            }
            for id in failed {
                self.finish_failed(id, error.clone());
                successful.remove(&id);
            }
        }
    }

    fn snapshot_with_provider_cache(
        &mut self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
        source_epoch: u64,
        successful: &mut HashMap<AnimationId, super::provider::ProviderEvaluation>,
    ) -> Result<Option<PresentationSnapshot>, AnimationError> {
        let invalid: Vec<_> = self
            .active
            .iter()
            .filter_map(|(id, animation)| {
                animation
                    .geometry_source
                    .as_ref()
                    .is_some_and(|source| !source.is_valid(tree, resources))
                    .then_some(id)
            })
            .collect();
        for id in invalid {
            self.finish(id, AnimationState::Cancelled);
        }
        let version = self.geometry_version(source_epoch);
        if let Some((cached_version, snapshot)) = &self.snapshot_cache
            && *cached_version == version
        {
            return Ok(snapshot.clone());
        }
        if let Some((cached_version, snapshot)) = &self.snapshot_cache
            && cached_version.sample == version.sample
            && cached_version.binding_revision == version.binding_revision
            && self.active.len() != 0
            && self
                .active
                .iter()
                .all(|(_, group)| group.geometry_source.is_some())
        {
            let mut snapshot = snapshot.clone();
            if let Some(snapshot) = &mut snapshot {
                snapshot.geometry = snapshot
                    .geometry
                    .as_ref()
                    .map(|geometry| Arc::new(geometry.with_version(version)));
                self.geometry_cache = snapshot.geometry.clone();
            }
            self.snapshot_cache = Some((version, snapshot.clone()));
            return Ok(snapshot);
        }
        if self.active.len() == 0 {
            if self.geometry_cache.as_ref().is_some_and(|geometry| {
                geometry.version() != version || !geometry.sources_valid(tree, resources)
            }) {
                self.geometry_cache = None;
            }
            return Ok(None);
        }
        let (active_figures, active_temporaries) = self.active_subjects();
        let active_presentations = self.active_presentation_channels_by_figure();
        let mut figures = HashMap::new();
        let mut patches: HashMap<FigureId, super::GeometryPatch> = HashMap::new();
        for (key, channel) in &self.geometry_channels {
            let Some(channel) = self.channels.get(*channel) else {
                continue;
            };
            if channel.has_override()
                && let ChannelBinding::FigureGeometry { .. } = &channel.binding
            {
                patches
                    .entry(key.figure)
                    .or_default()
                    .merge(channel.presentation_patch)?;
            }
        }
        for figure in active_figures {
            let geometry = patches.remove(&figure).unwrap_or_default();
            let opacity = self.figure_channels.get(&figure).and_then(|channels| {
                self.channels.get(channels.opacity).and_then(|channel| {
                    channel
                        .has_override()
                        .then(|| channel.effective::<Opacity>().ok().copied())
                        .flatten()
                })
            });
            let transform = self.figure_channels.get(&figure).and_then(|channels| {
                self.channels.get(channels.transform).and_then(|channel| {
                    channel
                        .has_override()
                        .then(|| channel.effective::<Affine2D>().ok().copied())
                        .flatten()
                })
            });
            let figure_presentations = active_presentations
                .get(&figure)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let content = figure_presentations.first().and_then(|channel| {
                let (bound_figure, binding) = channel.binding.figure_presentation()?;
                debug_assert_eq!(bound_figure, figure);
                let node = tree.node(figure).ok()?;
                let values: Vec<_> = figure_presentations
                    .iter()
                    .filter_map(|channel| {
                        let (_, binding) = channel.binding.figure_presentation()?;
                        Some((binding.value_type_id(), channel.effective_any()))
                    })
                    .collect();
                binding.prepare(
                    node.figure.as_ref(),
                    node.figure_bounds(),
                    PresentationValues::new(&values),
                )
            });
            if opacity.is_some()
                || transform.is_some()
                || content.is_some()
                || geometry != super::GeometryPatch::default()
            {
                figures.insert(
                    figure,
                    FigurePresentationEffect {
                        opacity,
                        transform,
                        content,
                        geometry,
                    },
                );
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
                    opacity: *self
                        .channels
                        .get(opacity_channel)?
                        .effective::<Opacity>()
                        .ok()?,
                    transform: *self
                        .channels
                        .get(transform_channel)?
                        .effective::<Affine2D>()
                        .ok()?,
                })
            })
            .collect();
        temporaries.sort_by_key(|visual| visual.id);
        let geometry = if figures
            .values()
            .any(|effect| effect.geometry != super::GeometryPatch::default())
            || self
                .active
                .iter()
                .any(|(_, group)| group.providers.len() != 0)
        {
            let sources = self.geometry_sources();
            let roots: Vec<_> = figures
                .keys()
                .copied()
                .chain(
                    sources
                        .iter()
                        .flat_map(|source| source.roots.iter().copied()),
                )
                .collect();
            let base = super::PresentationGeometry::freeze_sources(
                tree,
                &figures,
                roots.clone(),
                version,
                self.geometry_values()?,
                &sources,
            )?;
            let mut failed = Vec::new();
            let mut values = self.geometry_values()?;
            for (id, group) in self.active.iter() {
                if group.providers.len() == 0 {
                    continue;
                }
                let result = if let Some(cached) = successful.get(&id) {
                    Ok(cached.clone())
                } else {
                    group
                        .providers
                        .evaluate(&base, |patches| {
                            let source = group
                                .geometry_source
                                .as_ref()
                                .ok_or(AnimationError::GeometryNotPrepared)?;
                            let mut staged: HashMap<_, _> = source
                                .nodes
                                .keys()
                                .filter_map(|id| {
                                    figures.get(id).map(|effect| (*id, effect.clone()))
                                })
                                .collect();
                            for (figure, patch) in patches {
                                staged.entry(*figure).or_default().geometry.merge(*patch)?;
                            }
                            super::PresentationGeometry::freeze_sources(
                                tree,
                                &staged,
                                source.roots.iter().copied(),
                                version,
                                HashMap::new(),
                                std::slice::from_ref(source),
                            )
                        })
                        .and_then(|result| {
                            if result.patches.iter().any(|(figure, patch)| {
                                group.derived_fields.get(figure) != Some(&patch.fields())
                            }) {
                                return Err(AnimationError::InvalidValue);
                            }
                            Ok(result)
                        })
                };
                match result {
                    Ok(result) => {
                        successful.insert(id, result.clone());
                        for (figure, patch) in result.patches {
                            figures.entry(figure).or_default().geometry.merge(patch)?;
                        }
                        for (figure, content) in result.content {
                            if figures
                                .entry(figure)
                                .or_default()
                                .content
                                .replace(content)
                                .is_some()
                            {
                                return Err(AnimationError::ChannelConflict);
                            }
                        }
                        for (key, value) in result.values {
                            if values.insert(key, value).is_some() {
                                return Err(AnimationError::ChannelConflict);
                            }
                        }
                    }
                    // Shared allocation failure never terminates a group within its declaration.
                    Err(error @ AnimationError::BudgetExceeded(_)) => return Err(error),
                    Err(error) => failed.push((id, error)),
                }
            }
            if !failed.is_empty() {
                for (id, error) in failed {
                    self.finish_failed(id, error);
                }
                return self.snapshot_with_provider_cache(
                    tree,
                    resources,
                    source_epoch,
                    successful,
                );
            }
            Some(Arc::new(super::PresentationGeometry::freeze_sources(
                tree, &figures, roots, version, values, &sources,
            )?))
        } else {
            None
        };
        self.geometry_cache = geometry.clone();
        let snapshot =
            (!figures.is_empty() || !temporaries.is_empty()).then_some(PresentationSnapshot {
                figures,
                temporaries,
                geometry,
            });
        self.snapshot_cache = Some((version, snapshot.clone()));
        Ok(snapshot)
    }

    fn active_presentation_channels_by_figure(&self) -> HashMap<FigureId, Vec<&ChannelState>> {
        let mut seen = HashSet::new();
        let mut by_figure: HashMap<_, Vec<_>> = HashMap::new();
        for track in self
            .active
            .iter()
            .flat_map(|(_, animation)| animation.tracks.iter())
        {
            if !seen.insert(track.channel) {
                continue;
            }
            let Some(channel) = self.channels.get(track.channel) else {
                continue;
            };
            let Some((figure, _)) = channel.binding.figure_presentation() else {
                continue;
            };
            if channel.has_override() {
                by_figure.entry(figure).or_default().push(channel);
            }
        }
        by_figure
    }

    fn damage_state(&self, tree: &FigureTree) -> HashMap<DamageSubject, DamageSubjectState> {
        let (active_figures, active_temporaries) = self.active_subjects();
        let active_presentations = self.active_presentation_channels_by_figure();
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
            let content_revision = active_presentations
                .get(&figure)
                .into_iter()
                .flatten()
                .map(|channel| channel.presentation_revision)
                .reduce(u64::wrapping_add);
            state.insert(
                DamageSubject::Figure(figure),
                DamageSubjectState::Figure {
                    opacity: opacity.get(),
                    transform,
                    content_revision,
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
                    opacity: opacity.get(),
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

    pub(crate) fn reconcile_visibility(
        &mut self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
    ) -> bool {
        let ids: Vec<_> = self.active.iter().map(|(id, _)| id).collect();
        let mut changed = false;
        for id in ids {
            if self.active.get(id).is_some_and(|animation| {
                animation
                    .geometry_source
                    .as_ref()
                    .is_some_and(|source| !source.is_valid(tree, resources))
            }) {
                changed |= self.finish(id, AnimationState::Cancelled);
                continue;
            }
            let target_hidden = self.active.get(id).is_some_and(|animation| {
                // A locally retained visible node must not suspend its own collapse group.
                // A later visibility revision revokes retention, without invalidating content.
                let visibility: HashMap<_, _> = animation
                    .tracks
                    .iter()
                    .filter_map(|track| {
                        let channel = self.channels.get(track.channel)?;
                        Some((
                            channel.binding.figure()?,
                            channel.presentation_patch.visible?,
                        ))
                    })
                    .collect();
                animation.tracks.iter().any(|track| {
                    self.channels
                        .get(track.channel)
                        .and_then(|channel| channel.binding.figure())
                        .is_some_and(|figure| {
                            let mut current = Some(figure);
                            while let Some(figure) = current {
                                let Ok(node) = tree.node(figure) else {
                                    return true;
                                };
                                let external_visibility = animation
                                    .geometry_source
                                    .as_ref()
                                    .and_then(|source| source.nodes.get(&figure))
                                    .is_some_and(|frozen| {
                                        frozen.visibility_revision != node.visibility_revision
                                    });
                                let visible = if external_visibility {
                                    node.is_visible
                                } else {
                                    visibility.get(&figure).copied().unwrap_or(node.is_visible)
                                };
                                if !visible {
                                    return true;
                                }
                                current = node.parent;
                            }
                            false
                        })
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
        let mut owners: HashSet<_> = self
            .figure_channels
            .iter()
            .filter(|(figure, _)| figures.contains(figure))
            .flat_map(|(_, channels)| [channels.opacity, channels.transform])
            .filter_map(|channel| self.channel_owners.get(&channel).copied())
            .collect();
        owners.extend(
            self.presentation_channels
                .iter()
                .filter(|(key, _)| figures.contains(&key.figure))
                .filter_map(|(_, channel)| self.channel_owners.get(channel).copied()),
        );
        owners.extend(
            self.geometry_channels
                .iter()
                .filter(|(key, _)| figures.contains(&key.figure))
                .filter_map(|(_, channel)| self.channel_owners.get(channel).copied()),
        );
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
        let retired_presentations: Vec<_> = self
            .presentation_channels
            .keys()
            .copied()
            .filter(|key| figures.contains(&key.figure))
            .collect();
        for key in retired_presentations {
            if let Some(channel) = self.presentation_channels.remove(&key) {
                self.channels.remove(channel);
            }
        }
        let retired_geometry: Vec<_> = self
            .geometry_channels
            .keys()
            .copied()
            .filter(|key| figures.contains(&key.figure))
            .collect();
        for key in retired_geometry {
            if let Some(channel) = self.geometry_channels.remove(&key) {
                self.channels.remove(channel);
            }
        }
        let retired_behaviors: Vec<_> = self
            .behaviors
            .iter()
            .filter_map(|(id, behavior)| match behavior.scope {
                AnimationBehaviorScope::Figure(figure) if figures.contains(&figure) => Some(id),
                AnimationBehaviorScope::Runtime | AnimationBehaviorScope::Figure(_) => None,
            })
            .collect();
        for behavior in retired_behaviors {
            self.behaviors.remove(behavior);
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

    fn process_behaviors(
        &mut self,
        effects: &[NotificationEffect],
        epoch: u64,
        scene: StableSceneQuery<'_>,
        tree: &FigureTree,
    ) -> bool {
        let facts = facts_from_effects(effects);
        if facts.is_empty() || self.mode == AnimationMode::Disabled {
            return false;
        }

        let ids: Vec<_> = self.behaviors.iter().map(|(id, _)| id).collect();
        let mut changed = false;
        for id in ids {
            let result = {
                let Some(behavior) = self.behaviors.get_mut(id) else {
                    continue;
                };
                let matching = behavior.matching_facts(&facts);
                if matching.is_empty() {
                    continue;
                }
                let context = super::AnimationBehaviorContext::new(epoch, &matching, scene);
                match self.mode {
                    AnimationMode::Enabled => behavior.factory.create_plan(context),
                    AnimationMode::ReducedMotion => behavior
                        .reduced_motion_factory
                        .as_mut()
                        .map_or(Ok(None), |factory| factory.create_plan(context)),
                    AnimationMode::Disabled => unreachable!("disabled behavior processing exits"),
                }
            };
            match result {
                Ok(Some(plan)) => match self.start_from_behavior(plan, tree) {
                    Ok(AnimationStart::Running(_)) => changed = true,
                    Ok(AnimationStart::Existing(_) | AnimationStart::Suppressed(_)) => {}
                    Err(error) => self.record_behavior_failure(id, epoch, error),
                },
                Ok(None) => {}
                Err(error) => self.record_behavior_failure(id, epoch, error),
            }
        }
        changed
    }

    fn record_behavior_failure(
        &mut self,
        behavior: AnimationBehaviorId,
        epoch: u64,
        error: AnimationError,
    ) {
        self.behavior_failures
            .push_back(AnimationBehaviorFailure::new(behavior, epoch, error));
        while self.behavior_failures.len() > DEFAULT_ANIMATION_TERMINAL_HISTORY {
            self.behavior_failures.pop_front();
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

    fn prepare(
        &self,
        mut plan: AnimationPlan,
        tree: &FigureTree,
    ) -> Result<PreparedAnimation, AnimationError> {
        if plan.tracks.is_empty() {
            return Err(AnimationError::EmptyPlan);
        }
        self.limits.validate_staging(plan.tracks.len())?;
        // Reject oversized owned configuration and scheduling scratch before building the DAG.
        plan.providers.work()?.validate(self.limits.staging_work)?;
        plan.providers.validate()?;
        if plan.geometry_source.is_none() {
            let roots: Vec<_> = plan
                .tracks
                .iter()
                .filter_map(
                    |track| match &self.channels.get(track.spec.channel())?.binding {
                        ChannelBinding::FigureGeometry { figure, .. } => Some(*figure),
                        _ => None,
                    },
                )
                .chain(plan.providers.figures())
                .collect();
            if !roots.is_empty() {
                // Public admission freezes through AnimationMut, where resource versions
                // are available. Internal preparation cannot silently borrow live content.
                return Err(AnimationError::GeometryNotPrepared);
            }
        }
        let mut work = super::AnimationWork::for_tracks(plan.tracks.len());
        for track in &plan.tracks {
            if track.spec.channel().namespace() != self.namespace {
                return Err(AnimationError::ForeignChannel);
            }
            let channel = self
                .channels
                .get(track.spec.channel())
                .ok_or(AnimationError::UnknownChannel)?;
            work = work.combine(
                super::AnimationWork::default(),
                track.spec.storage_work(channel)?,
            )?;
            if let ChannelBinding::FigureGeometry { binding, .. } = &channel.binding {
                work.bytes = work.bytes.checked_add(binding.storage_bytes()).ok_or(
                    AnimationError::BudgetExceeded(AnimationBudgetKind::SnapshotBytes),
                )?;
            }
        }
        work.calls =
            work.calls
                .checked_add(plan.providers.len())
                .ok_or(AnimationError::BudgetExceeded(
                    AnimationBudgetKind::ProviderCalls,
                ))?;
        work = work.combine(super::AnimationWork::default(), plan.providers.work()?)?;
        work.edges = plan.providers.edges()?;
        if let Some(source) = &plan.geometry_source {
            work.nodes = source.nodes.len();
            work.edges = source.nodes.values().try_fold(work.edges, |count, node| {
                count
                    .checked_add(node.children.len())
                    .ok_or(AnimationError::BudgetExceeded(
                        AnimationBudgetKind::DependencyEdges,
                    ))
            })?;
            // Current and next presentation coexist with the owned source. Checked arithmetic
            // preserves the peak reservation even if address-space arithmetic would overflow.
            let source_bytes = source.collection_bytes()?;
            let geometry_bytes = source.materialization_bytes()?;
            work.bytes = work
                .bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(source_bytes))
                .and_then(|bytes| {
                    geometry_bytes
                        .checked_mul(4)
                        .and_then(|recordings| bytes.checked_add(recordings))
                })
                .ok_or(AnimationError::BudgetExceeded(
                    AnimationBudgetKind::SnapshotBytes,
                ))?;
            work.operations = work
                .nodes
                .checked_mul(super::capacity::GEOMETRY_OPERATIONS_PER_NODE)
                .and_then(|operations| operations.checked_add(work.edges))
                .and_then(|operations| operations.checked_add(work.operations))
                .ok_or(AnimationError::BudgetExceeded(
                    AnimationBudgetKind::GeometryOperations,
                ))?;
        }
        work.validate(self.limits.staging_work)?;
        let mut final_tracks: HashMap<AnimationChannelId, (u64, usize)> = HashMap::new();
        let mut presentation_families = HashMap::new();
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
            if let Some((figure, binding)) = self
                .channels
                .get(track.spec.channel())
                .and_then(|channel| channel.binding.figure_presentation())
            {
                let entry = presentation_families
                    .entry(figure)
                    .or_insert_with(|| binding.family());
                if *entry != binding.family() {
                    return Err(AnimationError::OverlappingTracks);
                }
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
        // Resolve the complete initial sample before changing any live owner or value.
        let initial_values = self.prepare_samples(&tracks, 0)?;
        let mut fields: HashMap<FigureId, Vec<super::geometry::GeometryFields>> = HashMap::new();
        let mut seen = HashSet::new();
        for track in &tracks {
            if !seen.insert(track.channel) {
                continue;
            }
            if let Some(ChannelState {
                binding:
                    ChannelBinding::FigureGeometry {
                        figure,
                        fields: output,
                        ..
                    },
                ..
            }) = self.channels.get(track.channel)
            {
                let previous = fields.entry(*figure).or_default();
                if previous.iter().any(|fields| fields.overlaps(*output)) {
                    return Err(AnimationError::ChannelConflict);
                }
                previous.push(*output);
            }
        }
        let mut derived_fields = HashMap::new();
        if plan.geometry_source.is_some() {
            let mut effects = HashMap::new();
            for (id, value) in &initial_values {
                if let ChannelBinding::FigureGeometry { figure, .. } =
                    &self.channels.get(*id).expect("prepared channel").binding
                {
                    let effect = effects
                        .entry(*figure)
                        .or_insert_with(FigurePresentationEffect::default);
                    effect.geometry.merge(value.patch)?;
                }
            }
            let sources: Vec<_> = plan.geometry_source.iter().cloned().collect();
            let geometry = super::PresentationGeometry::freeze_sources(
                tree,
                &effects,
                plan.providers.figures().chain(effects.keys().copied()),
                self.geometry_version(0),
                HashMap::new(),
                &sources,
            )?;
            let result = if plan.providers.len() == 0 {
                super::provider::ProviderEvaluation::default()
            } else {
                plan.providers.evaluate(&geometry, |patches| {
                    let mut staged = effects.clone();
                    for (figure, patch) in patches {
                        staged.entry(*figure).or_default().geometry.merge(*patch)?;
                    }
                    super::PresentationGeometry::freeze_sources(
                        tree,
                        &staged,
                        plan.providers.figures().chain(effects.keys().copied()),
                        self.geometry_version(0),
                        HashMap::new(),
                        &sources,
                    )
                })?
            };
            for figure in result.content.keys() {
                if presentation_families.contains_key(figure) {
                    return Err(AnimationError::ChannelConflict);
                }
            }
            for (figure, patch) in result.patches {
                if fields.get(&figure).is_some_and(|outputs| {
                    outputs.iter().any(|output| output.overlaps(patch.fields()))
                }) {
                    return Err(AnimationError::ChannelConflict);
                }
                derived_fields.insert(figure, patch.fields());
            }
            if result.values.keys().any(|(figure, value_type)| {
                self.geometry_channels
                    .contains_key(&FigurePresentationChannelKey {
                        figure: *figure,
                        value_type: *value_type,
                    })
            }) {
                return Err(AnimationError::ChannelConflict);
            }
        }
        Ok(PreparedAnimation {
            tracks,
            initial_values,
            duration_micros: plan.duration_micros,
            interruption: plan.interruption,
            suspension: plan.suspension,
            geometry_source: plan.geometry_source,
            work,
            providers: plan.providers,
            derived_fields,
        })
    }

    fn start(
        &mut self,
        plan: AnimationPlan,
        tree: &FigureTree,
    ) -> Result<AnimationStart, AnimationError> {
        self.start_with_mode(plan, false, tree)
    }

    fn start_from_behavior(
        &mut self,
        plan: AnimationPlan,
        tree: &FigureTree,
    ) -> Result<AnimationStart, AnimationError> {
        self.start_with_mode(plan, true, tree)
    }

    fn start_with_mode(
        &mut self,
        plan: AnimationPlan,
        reduced_motion_fallback: bool,
        tree: &FigureTree,
    ) -> Result<AnimationStart, AnimationError> {
        if let Some(suppressed) = self.suppressed_start(&plan, reduced_motion_fallback)? {
            return Ok(suppressed);
        }
        let prepared = self.prepare(plan, tree)?;
        if prepared.duration_micros == 0 {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }
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
        existing.extend(self.conflicting_presentation_owners(&prepared.tracks, None));
        existing.extend(self.conflicting_geometry_owners(&prepared.tracks, None));
        existing.extend(self.conflicting_derived_owners(&prepared, None)?);
        if let (InterruptionPolicy::Ignore, Some(owner)) =
            (prepared.interruption, existing.iter().next().copied())
        {
            return Ok(AnimationStart::Existing(owner));
        }
        let replaced_tracks = existing
            .iter()
            .filter_map(|owner| self.active.get(*owner))
            .map(|animation| animation.tracks.len())
            .sum();
        self.validate_active_budget(prepared.tracks.len(), existing.len(), replaced_tracks)?;
        self.validate_work_budget(&prepared, existing.iter().copied())?;
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
            geometry_source: prepared.geometry_source,
            work: prepared.work,
            providers: prepared.providers,
            derived_fields: prepared.derived_fields,
        });
        self.active_revision = self.active_revision.saturating_add(1);
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
        self.publish_samples(prepared.initial_values);
        self.set_suspension_reason(id, None, None);
        Ok(AnimationStart::Running(id))
    }

    fn suppressed_start(
        &self,
        plan: &AnimationPlan,
        reduced_motion_fallback: bool,
    ) -> Result<Option<AnimationStart>, AnimationError> {
        let suppression = match self.mode {
            AnimationMode::Disabled => AnimationSuppression::Disabled,
            AnimationMode::ReducedMotion if !reduced_motion_fallback => {
                AnimationSuppression::ReducedMotionStaticFallback
            }
            AnimationMode::Enabled | AnimationMode::ReducedMotion => return Ok(None),
        };
        if plan.tracks.is_empty() {
            return Err(AnimationError::EmptyPlan);
        }
        let mut terminal = HashMap::new();
        for (index, track) in plan.tracks.iter().enumerate() {
            let channel_id = track.spec.channel();
            if channel_id.namespace() != self.namespace {
                return Err(AnimationError::ForeignChannel);
            }
            if !self.channels.contains_key(channel_id) {
                return Err(AnimationError::UnknownChannel);
            }
            let end = track
                .start_micros
                .checked_add(track.duration_micros)
                .ok_or(AnimationError::TimeOverflow)?;
            let last = terminal.entry(channel_id).or_insert((end, index));
            if (end, index) > *last {
                *last = (end, index);
            }
        }
        for (channel, (_, index)) in terminal {
            if !plan.tracks[index]
                .spec
                .terminal_matches(self.channels.get(channel).expect("validated channel"))?
            {
                return Err(AnimationError::FinalValueMismatch);
            }
        }
        Ok(Some(AnimationStart::Suppressed(suppression)))
    }

    fn retarget(
        &mut self,
        id: AnimationId,
        plan: AnimationPlan,
        tree: &FigureTree,
    ) -> Result<bool, AnimationError> {
        self.validate_animation_id(id)?;
        let prepared = self.prepare(plan, tree)?;
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
        self.validate_active_budget(prepared.tracks.len(), 1, old_track_count)?;
        self.validate_work_budget(&prepared, [id])?;
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
        if !self
            .conflicting_presentation_owners(&prepared.tracks, Some(id))
            .is_empty()
            || !self
                .conflicting_geometry_owners(&prepared.tracks, Some(id))
                .is_empty()
        {
            return Err(AnimationError::ChannelConflict);
        }
        if !self
            .conflicting_derived_owners(&prepared, Some(id))?
            .is_empty()
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
            geometry_source: prepared.geometry_source,
            work: prepared.work,
            providers: prepared.providers,
            derived_fields: prepared.derived_fields,
        };
        self.active_revision = self.active_revision.saturating_add(1);
        for visual in old_temporaries.difference(&new_temporaries) {
            changed |= self.remove_temporary_visual_state(*visual);
        }
        self.assign_temporary_visuals(id);
        changed |= self.publish_samples(prepared.initial_values);
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
        let animation = self
            .active
            .get(id)
            .expect("sampled animation must remain active");
        let values = self.prepare_samples(&animation.tracks, elapsed_micros)?;
        Ok(self.publish_samples(values))
    }

    fn prepare_samples(
        &self,
        tracks: &[PreparedPlanTrack],
        elapsed_micros: u64,
    ) -> Result<Vec<(AnimationChannelId, PreparedChannelValue)>, AnimationError> {
        let mut selected: HashMap<AnimationChannelId, usize> = HashMap::new();
        for (index, track) in tracks.iter().enumerate() {
            let Some(current) = selected.get(&track.channel).copied() else {
                selected.insert(track.channel, index);
                continue;
            };
            let current_track = &tracks[current];
            let current_started = current_track.start_micros <= elapsed_micros;
            let candidate_started = track.start_micros <= elapsed_micros;
            if (candidate_started && track.start_micros >= current_track.start_micros)
                || (!current_started && track.start_micros < current_track.start_micros)
            {
                selected.insert(track.channel, index);
            }
        }

        let mut selected: Vec<_> = selected.into_values().collect();
        selected.sort_unstable();
        let mut values = Vec::with_capacity(selected.len());
        for index in selected {
            let track = &tracks[index];
            let local = elapsed_micros
                .saturating_sub(track.start_micros)
                .min(track.duration_micros);
            let channel = self
                .channels
                .get(track.channel)
                .expect("prepared channel must remain registered");
            values.push((track.channel, track.sampler.sample(local, channel)?));
        }
        Ok(values)
    }

    fn publish_samples(&mut self, values: Vec<(AnimationChannelId, PreparedChannelValue)>) -> bool {
        let mut changed = false;
        for (id, value) in values {
            changed |= self
                .channels
                .get_mut(id)
                .expect("prepared channel must remain registered")
                .publish_override(value);
            self.stats.tracks_sampled = self.stats.tracks_sampled.saturating_add(1);
        }
        changed
    }

    fn finish(&mut self, id: AnimationId, state: AnimationState) -> bool {
        self.finish_preserving_temporaries(id, state, &HashSet::new())
    }

    fn finish_failed(&mut self, id: AnimationId, error: AnimationError) -> bool {
        let changed = self.finish(id, AnimationState::Failed);
        self.failures.insert(id, error);
        changed
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
        self.active_revision = self.active_revision.saturating_add(1);
        // A stale cache has no query authority after the owner revision changes. Its resources
        // may survive only through independent capture/submission retention responsibilities.
        self.geometry_cache = None;
        self.snapshot_cache = None;
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
            if let Some((id, _)) = self.terminal.pop_front() {
                self.failures.remove(&id);
            }
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
        replaced_animations: usize,
        replaced_tracks: usize,
    ) -> Result<(), AnimationError> {
        self.limits.validate_replacement(
            self.reservations(),
            AnimationReservations {
                active_animations: replaced_animations,
                active_tracks: replaced_tracks,
                work: super::AnimationWork::default(),
                capture_bytes: 0,
                submission_bytes: 0,
            },
            new_tracks,
            true,
        )
    }

    fn reservations(&self) -> AnimationReservations {
        AnimationReservations {
            active_animations: self.active.len(),
            active_tracks: self.active_track_count,
            work: self
                .active
                .iter()
                .fold(super::AnimationWork::default(), |sum, (_, group)| {
                    sum.combine(super::AnimationWork::default(), group.work)
                        .expect("admitted work reservations cannot overflow")
                }),
            capture_bytes: self.capture_storage.live_bytes(),
            submission_bytes: self.submission_storage.live_bytes(),
        }
    }

    fn validate_work_budget(
        &self,
        prepared: &PreparedAnimation,
        retired: impl IntoIterator<Item = AnimationId>,
    ) -> Result<(), AnimationError> {
        let mut removed = super::AnimationWork::default();
        for id in retired {
            if let Some(group) = self.active.get(id) {
                removed = removed.combine(super::AnimationWork::default(), group.work)?;
            }
        }
        self.reservations()
            .work
            .combine(removed, prepared.work)?
            .validate(self.limits.active_work)
    }

    fn conflicting_derived_owners(
        &self,
        prepared: &PreparedAnimation,
        except: Option<AnimationId>,
    ) -> Result<HashSet<AnimationId>, AnimationError> {
        let mut requested = prepared.derived_fields.clone();
        for track in &prepared.tracks {
            if let Some(ChannelState {
                binding: ChannelBinding::FigureGeometry { figure, fields, .. },
                ..
            }) = self.channels.get(track.channel)
            {
                // Track/derived overlap in a candidate was already rejected during prepare.
                requested
                    .entry(*figure)
                    .and_modify(|output| *output = output.union(*fields))
                    .or_insert(*fields);
            }
        }
        let mut dependencies: HashSet<_> = prepared.providers.figures().collect();
        if (prepared.providers.len() != 0
            || requested.values().any(|fields| fields.child_transform))
            && let Some(source) = &prepared.geometry_source
        {
            // World/client queries depend on each captured ancestor mapping too.
            dependencies.extend(source.nodes.keys().copied());
        }
        let requested_channels: HashSet<_> =
            prepared.tracks.iter().map(|track| track.channel).collect();
        let requested_drawings: HashSet<_> = prepared.providers.drawing_figures().collect();
        let mut requested_content = requested_drawings.clone();
        requested_content.extend(prepared.tracks.iter().filter_map(|track| {
            self.channels
                .get(track.channel)?
                .binding
                .figure_presentation()
                .map(|(figure, _)| figure)
        }));
        let mut conflicts = HashSet::new();
        for (id, group) in self.active.iter() {
            if Some(id) == except {
                continue;
            }
            let replaces_group = group
                .providers
                .drawing_figures()
                .any(|figure| requested_content.contains(&figure))
                || group.derived_fields.iter().any(|(figure, fields)| {
                    requested
                        .get(figure)
                        .is_some_and(|output| output.overlaps(*fields))
                });
            if replaces_group {
                conflicts.insert(id);
                continue;
            }
            for track in &group.tracks {
                if let Some(ChannelState {
                    binding: ChannelBinding::FigureTransform(figure),
                    ..
                }) = self.channels.get(track.channel)
                    && dependencies.contains(figure)
                    && !requested_channels.contains(&track.channel)
                {
                    return Err(AnimationError::ChannelConflict);
                }
                if let Some((figure, _)) = self
                    .channels
                    .get(track.channel)
                    .and_then(|channel| channel.binding.figure_presentation())
                    && requested_drawings.contains(&figure)
                {
                    conflicts.insert(id);
                }
                if let Some(ChannelState {
                    binding: ChannelBinding::FigureGeometry { figure, fields, .. },
                    ..
                }) = self.channels.get(track.channel)
                {
                    if dependencies.contains(figure) && !requested_channels.contains(&track.channel)
                    {
                        return Err(AnimationError::ChannelConflict);
                    }
                    if requested
                        .get(figure)
                        .is_some_and(|output| output.overlaps(*fields))
                    {
                        conflicts.insert(id);
                    }
                }
            }
            if group
                .providers
                .drawing_figures()
                .any(|figure| requested_content.contains(&figure))
            {
                conflicts.insert(id);
            }
            for (figure, fields) in &group.derived_fields {
                if dependencies.contains(figure) {
                    return Err(AnimationError::ChannelConflict);
                }
                if requested
                    .get(figure)
                    .is_some_and(|output| output.overlaps(*fields))
                {
                    conflicts.insert(id);
                }
            }
        }
        Ok(conflicts)
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

    fn presentation_families(&self, tracks: &[PreparedPlanTrack]) -> HashMap<FigureId, TypeId> {
        tracks
            .iter()
            .filter_map(|track| {
                self.channels
                    .get(track.channel)
                    .and_then(|channel| channel.binding.figure_presentation())
                    .map(|(figure, binding)| (figure, binding.family()))
            })
            .collect()
    }

    fn conflicting_presentation_owners(
        &self,
        tracks: &[PreparedPlanTrack],
        except: Option<AnimationId>,
    ) -> HashSet<AnimationId> {
        let requested = self.presentation_families(tracks);
        if requested.is_empty() {
            return HashSet::new();
        }
        self.active
            .iter()
            .filter(|(owner, _)| Some(*owner) != except)
            .filter_map(|(owner, animation)| {
                animation
                    .tracks
                    .iter()
                    .any(|track| {
                        self.channels
                            .get(track.channel)
                            .and_then(|channel| channel.binding.figure_presentation())
                            .is_some_and(|(figure, binding)| {
                                requested
                                    .get(&figure)
                                    .is_some_and(|family| *family != binding.family())
                            })
                    })
                    .then_some(owner)
            })
            .collect()
    }

    fn conflicting_geometry_owners(
        &self,
        tracks: &[PreparedPlanTrack],
        except: Option<AnimationId>,
    ) -> HashSet<AnimationId> {
        let requested: Vec<_> = tracks
            .iter()
            .filter_map(|track| {
                if let ChannelBinding::FigureGeometry { figure, fields, .. } =
                    &self.channels.get(track.channel)?.binding
                {
                    Some((*figure, *fields))
                } else {
                    None
                }
            })
            .collect();
        self.active
            .iter()
            .filter(|(owner, _)| Some(*owner) != except)
            .filter_map(|(owner, animation)| {
                animation
                    .tracks
                    .iter()
                    .any(|track| {
                        let Some(channel) = self.channels.get(track.channel) else {
                            return false;
                        };
                        if let ChannelBinding::FigureGeometry { figure, fields, .. } =
                            &channel.binding
                        {
                            requested.iter().any(|(target, output)| {
                                target == figure && fields.overlaps(*output)
                            })
                        } else {
                            false
                        }
                    })
                    .then_some(owner)
            })
            .collect()
    }

    /// Both typed geometry and legacy contents transforms participate in origin arbitration.
    pub(crate) fn viewport_owners(
        &self,
        viewport: FigureId,
        contents: Option<FigureId>,
    ) -> Vec<AnimationId> {
        self.active
            .iter()
            .filter_map(|(id, group)| {
                let owns = group
                    .derived_fields
                    .get(&viewport)
                    .is_some_and(|fields| fields.child_transform)
                    || group.tracks.iter().any(|track| {
                        self.channels.get(track.channel).is_some_and(|channel| {
                            match &channel.binding {
                                ChannelBinding::FigureGeometry { figure, fields, .. } => {
                                    *figure == viewport && fields.child_transform
                                        || Some(*figure) == contents && fields.local_transform
                                }
                                ChannelBinding::FigureTransform(figure) => {
                                    Some(*figure) == contents
                                }
                                _ => false,
                            }
                        })
                    });
                owns.then_some(id)
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

fn map_connection_presentation_error(error: AnimationError) -> AnimationError {
    match error {
        AnimationError::UnsupportedPresentationBinding { .. }
        | AnimationError::InvalidPresentationBinding { .. } => {
            AnimationError::UnsupportedRoutePresentation
        }
        error => error,
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
    resources: &'a crate::runtime::ResourceRegistry,
    updates: &'a mut UpdateManager,
    full_redraw_pending: &'a mut bool,
    faulted: &'a mut bool,
}

impl AnimationMut<'_> {
    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if self.service.time_initialized && now <= self.service.now {
            return self.service.advance_time(now);
        }
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
        let changed = self.service.reconcile_visibility(self.tree, self.resources);
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

    pub(crate) fn process_behaviors(
        &mut self,
        effects: &[NotificationEffect],
        stable_epoch: u64,
    ) -> bool {
        if effects.is_empty() {
            return false;
        }
        let before = self.service.damage_state(self.tree);
        let scene = StableSceneQuery::new(stable_epoch, self.tree);
        let tree = self.tree;
        let changed = self.guarded_service(|service| {
            service.process_behaviors(effects, stable_epoch, scene, tree)
        });
        self.reconcile_damage(before);
        changed
    }

    /// Returns the Runtime-wide mode.
    pub fn mode(&self) -> AnimationMode {
        self.service.mode
    }

    /// Returns the configured ownership and candidate preparation limits.
    pub fn limits(&self) -> AnimationLimits {
        self.service.limits
    }

    /// Atomically updates limits, rejecting capacity below existing reservations.
    ///
    /// No healthy owner, cached value, or wake deadline is evicted by this operation.
    pub fn set_limits(&mut self, limits: AnimationLimits) -> Result<(), AnimationError> {
        self.ensure_not_faulted()?;
        limits.validate_live(self.service.reservations())?;
        self.service
            .capture_storage
            .set_limit(limits.capture_bytes)?;
        self.service
            .submission_storage
            .set_limit(limits.submission_bytes)?;
        self.service.limits = limits;
        Ok(())
    }

    /// Returns live reservations without advancing time or executing extension code.
    pub fn reservations(&self) -> AnimationReservations {
        self.service.reservations()
    }

    /// Returns the current base-sample identity without preparing geometry or recording a frame.
    pub fn sample_version(&self) -> AnimationSampleVersion {
        AnimationSampleVersion {
            time: self.service.time_initialized.then_some(self.service.now),
            active_revision: self.service.active_revision,
        }
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

    /// Installs a committed-fact behavior. It observes only future stable transactions.
    pub fn install_behavior(
        &mut self,
        behavior: AnimationBehavior,
    ) -> Result<AnimationBehaviorId, AnimationError> {
        self.ensure_not_faulted()?;
        if let AnimationBehaviorScope::Figure(figure) = behavior.scope {
            if figure.namespace() != self.service.namespace {
                return Err(AnimationError::ForeignTarget);
            }
            if !self.tree.is_attached(figure)? {
                return Err(AnimationError::DisposedTarget);
            }
        }
        if self.service.behaviors.len() >= DEFAULT_MAX_ANIMATION_BEHAVIORS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Behaviors,
            ));
        }
        Ok(self.service.behaviors.insert(behavior))
    }

    /// Removes an installed behavior.
    pub fn remove_behavior(
        &mut self,
        behavior: AnimationBehaviorId,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        if behavior.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignBehavior);
        }
        self.service
            .behaviors
            .remove(behavior)
            .map(|_| true)
            .ok_or(AnimationError::UnknownBehavior)
    }

    /// Returns the number of installed behaviors.
    pub fn behavior_count(&self) -> usize {
        self.service.behaviors.len()
    }

    /// Drains bounded recoverable behavior failures in occurrence order.
    pub fn take_behavior_failures(&mut self) -> Vec<AnimationBehaviorFailure> {
        self.service.behavior_failures.drain(..).collect()
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
        if !state.binding.is_detached() {
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
            .is_some_and(|channel| !channel.binding.is_detached())
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
        if !self.tree.is_attached(figure)? {
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
        let opacity = self.tree.resolved_style(figure)?.alpha;
        let opacity = self.service.channels.insert(ChannelState::new_bound(
            opacity,
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

    /// Creates or returns a Runtime-managed channel identified by Figure and semantic value type.
    ///
    /// The descriptor freezes its output fields at registration. Input and accessibility keep
    /// reading committed geometry; external descriptors use the same registration path.
    pub fn bind_geometry<V: AnimationValue + Copy>(
        &mut self,
        figure: FigureId,
        binding: super::GeometryBinding<V>,
    ) -> Result<AnimationChannel<V>, AnimationError> {
        self.ensure_not_faulted()?;
        if figure.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignTarget);
        }
        let tree = self.tree;
        let node = tree.node(figure)?;
        let key = FigurePresentationChannelKey {
            figure,
            value_type: TypeId::of::<V>(),
        };
        if let Some(channel) = self.service.geometry_channels.get(&key) {
            return Ok(AnimationChannel::new(*channel));
        }
        if self.service.channels.len() >= DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        let committed = self.guarded_service(|_| {
            (binding.committed)(node.figure.as_ref(), super::geometry::inputs(node))
        })?;
        let erased = binding.erased();
        let fields = self.guarded_service(|_| erased.patch(&committed))?.fields();
        let state = ChannelState::new_bound(
            committed,
            ChannelBinding::FigureGeometry {
                figure,
                value_type: TypeId::of::<V>(),
                binding: erased,
                fields,
            },
        );
        self.guarded_service(|_| state.validate_value(state.committed::<V>()?))?;
        let channel = self.service.channels.insert(state);
        self.service.geometry_channels.insert(key, channel);
        self.service.binding_revision = self.service.binding_revision.saturating_add(1);
        Ok(AnimationChannel::new(channel))
    }

    /// Updates the terminal source value before admitting a geometry transition.
    ///
    /// This only updates the channel's source input; it never writes Figure state.
    pub fn refresh_geometry<V: AnimationValue + Copy>(
        &mut self,
        figure: FigureId,
        binding: super::GeometryBinding<V>,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        let tree = self.tree;
        let node = tree.node(figure)?;
        let committed = self.guarded_service(|_| {
            (binding.committed)(node.figure.as_ref(), super::geometry::inputs(node))
        })?;
        let channel = self.bind_geometry(figure, binding)?;
        self.guarded_service(|service| {
            let state = service
                .channels
                .get_mut(channel.id())
                .ok_or(AnimationError::UnknownChannel)?;
            state.validate_value(&committed)?;
            let source_changed = state.committed::<V>()? != &committed;
            let changed = state.set_committed(committed)?;
            if source_changed {
                service.binding_revision = service.binding_revision.saturating_add(1);
            }
            Ok(changed)
        })
    }

    /// Creates or returns a Runtime-managed typed content-presentation channel.
    ///
    /// The Figure must register the same key at attach time.
    pub fn bind_presentation<V: AnimationValue>(
        &mut self,
        figure: FigureId,
        key: CapabilityKey<PresentationBinding<V>>,
        interaction: InteractionGeometryPolicy,
    ) -> Result<AnimationChannel<V>, AnimationError> {
        self.ensure_not_faulted()?;
        if interaction != InteractionGeometryPolicy::Committed {
            return Err(AnimationError::UnsupportedInteractionGeometry);
        }
        if figure.namespace() != self.service.namespace {
            return Err(AnimationError::ForeignTarget);
        }
        let node = self.tree.node(figure)?;
        let binding = self
            .tree
            .capability(figure, key)
            .map_err(|_| AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            })?
            .cloned()
            .ok_or(AnimationError::UnsupportedPresentationBinding {
                capability: key.name(),
            })?;
        let channel_key = FigurePresentationChannelKey {
            figure,
            value_type: TypeId::of::<V>(),
        };
        if let Some(channel) = self
            .service
            .presentation_channels
            .get(&channel_key)
            .copied()
        {
            return Ok(AnimationChannel::new(channel));
        }
        let required = usize::from(!self.service.figure_channels.contains_key(&figure)) * 2 + 1;
        if self.service.channels.len().saturating_add(required) > DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        let committed = binding.committed(node.figure.as_ref()).ok_or(
            AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            },
        )?;
        if !committed.is_valid() {
            return Err(AnimationError::InvalidValue);
        }
        self.bind_figure(figure, interaction)?;
        let channel = self.service.channels.insert(ChannelState::new_bound(
            committed,
            ChannelBinding::FigurePresentation {
                figure,
                binding: Arc::new(TypedPresentationBinding { binding }),
            },
        ));
        self.service
            .presentation_channels
            .insert(channel_key, channel);
        Ok(AnimationChannel::new(channel))
    }

    /// Refreshes a managed presentation channel from the Figure's committed source state.
    pub fn refresh_presentation_binding<V: AnimationValue>(
        &mut self,
        figure: FigureId,
        key: CapabilityKey<PresentationBinding<V>>,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        let node = self.tree.node(figure)?;
        let binding = self
            .tree
            .capability(figure, key)
            .map_err(|_| AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            })?
            .ok_or(AnimationError::UnsupportedPresentationBinding {
                capability: key.name(),
            })?;
        let committed = binding.committed(node.figure.as_ref()).ok_or(
            AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            },
        )?;
        let channel = self.bind_presentation(figure, key, InteractionGeometryPolicy::Committed)?;
        self.set_presentation_committed(channel, committed)
    }

    pub(crate) fn prepare_presentation<V: AnimationValue>(
        &self,
        figure: FigureId,
        key: CapabilityKey<PresentationBinding<V>>,
        value: &V,
    ) -> Result<FigurePresentation, AnimationError> {
        let node = self.tree.node(figure)?;
        let binding = self
            .tree
            .capability(figure, key)
            .map_err(|_| AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            })?
            .ok_or(AnimationError::UnsupportedPresentationBinding {
                capability: key.name(),
            })?;
        let supplied = value as &dyn Any;
        let values: Vec<_> = self
            .service
            .presentation_channels
            .iter()
            .filter(|(channel_key, _)| channel_key.figure == figure)
            .filter_map(|(_, channel)| {
                let channel = self.service.channels.get(*channel)?;
                let (_, channel_binding) = channel.binding.figure_presentation()?;
                (channel_binding.family() == binding.family()).then(|| {
                    let value = if channel_binding.value_type_id() == TypeId::of::<V>() {
                        supplied
                    } else {
                        channel.effective_any()
                    };
                    (channel_binding.value_type_id(), value)
                })
            })
            .collect();
        binding
            .prepare(
                node.figure.as_ref(),
                PresentationValues::new(&values),
                node.figure_bounds(),
            )
            .ok_or(AnimationError::InvalidPresentationBinding {
                capability: key.name(),
            })
    }

    fn set_presentation_committed<V: AnimationValue>(
        &mut self,
        channel: AnimationChannel<V>,
        committed: V,
    ) -> Result<bool, AnimationError> {
        let state = self.channel_mut(channel)?;
        if state.binding.figure_presentation().is_none() {
            return Err(AnimationError::ManagedChannel);
        }
        state.set_committed(committed)
    }

    /// Creates or returns the Runtime-managed route presentation channel for a Connection.
    pub fn bind_connection_route(
        &mut self,
        figure: FigureId,
    ) -> Result<AnimationChannel<PointList>, AnimationError> {
        self.bind_presentation(
            figure,
            CONNECTION_ROUTE_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .map_err(map_connection_presentation_error)
    }

    pub(crate) fn set_connection_route_committed(
        &mut self,
        channel: AnimationChannel<PointList>,
        committed: PointList,
    ) -> Result<bool, AnimationError> {
        self.ensure_not_faulted()?;
        self.set_presentation_committed(channel, committed)
    }

    /// Creates or returns the Runtime-managed dash-offset channel for a Connection.
    pub fn bind_connection_dash_offset(
        &mut self,
        figure: FigureId,
    ) -> Result<AnimationChannel<f64>, AnimationError> {
        self.bind_presentation(
            figure,
            CONNECTION_DASH_PRESENTATION,
            InteractionGeometryPolicy::Committed,
        )
        .map_err(map_connection_presentation_error)
    }

    /// Starts a continuous dash flow in logical units per second.
    ///
    /// Positive speed moves the visible pattern from source to target; negative speed reverses it.
    pub fn start_connection_dash_flow(
        &mut self,
        figure: FigureId,
        speed: f64,
    ) -> Result<AnimationStart, AnimationError> {
        self.ensure_not_faulted()?;
        if !speed.is_finite() || speed == 0.0 {
            return Err(AnimationError::InvalidValue);
        }
        let connection = self
            .tree
            .node(figure)?
            .connection()
            .ok_or(AnimationError::UnsupportedRoutePresentation)?;
        let period = connection.connection_dash_period();
        if !period.is_finite() || period <= 0.0 {
            return Err(AnimationError::InvalidValue);
        }
        let channel = self.bind_connection_dash_offset(figure)?;
        let start = self.value(channel)?;
        let duration = Duration::try_from_secs_f64(period / speed.abs())
            .map_err(|_| AnimationError::InvalidDuration)?;
        // Increasing a stroke dash offset moves the visible pattern against route order.
        let end = start - period.copysign(speed);
        let plan = AnimationPlan::track(
            channel,
            Motion::Procedural(Procedural::continuous(start, end, duration)?),
        )?
        .with_suspension(SuspensionPolicy::Pause);
        self.start(plan)
    }

    /// Starts a non-interactive pulse that follows committed route arc length.
    pub fn start_connection_pulse(
        &mut self,
        figure: FigureId,
        pulse: ConnectionPulse,
    ) -> Result<AnimationStart, AnimationError> {
        self.ensure_not_faulted()?;
        if !pulse.radius.is_finite()
            || pulse.radius <= 0.0
            || !pulse.color.is_valid()
            || pulse.duration.is_zero()
        {
            return Err(AnimationError::InvalidValue);
        }
        let local_to_surface = self
            .tree
            .local_to_surface_transform(figure)
            .map_err(AnimationError::Coordinate)?;
        let route = self
            .tree
            .connection_route_points(figure)?
            .ok_or(AnimationError::UnsupportedRoutePresentation)?;
        let points: Vec<_> = route
            .iter()
            .map(|point| local_to_surface.transform_point(*point))
            .collect();
        let segments: Vec<_> = points
            .windows(2)
            .filter_map(|segment| {
                let length = (segment[1] - segment[0]).length();
                (length > f64::EPSILON).then_some((segment[0], segment[1], length))
            })
            .collect();
        let total_length: f64 = segments.iter().map(|(_, _, length)| length).sum();
        let duration_micros = u64::try_from(pulse.duration.as_micros())
            .map_err(|_| AnimationError::InvalidDuration)?;
        if segments.is_empty()
            || !total_length.is_finite()
            || duration_micros < segments.len() as u64
        {
            return Err(AnimationError::InvalidValue);
        }
        if let Some((decoration, _)) = pulse.handoff
            && !self.tree.is_attached(decoration)?
        {
            return Err(AnimationError::DisposedTarget);
        }
        if pulse
            .handoff
            .is_some_and(|(_, handoff_duration)| handoff_duration.is_zero())
        {
            return Err(AnimationError::InvalidDuration);
        }

        let final_point = segments.last().expect("non-empty segments").1;
        let visual = self.create_temporary_visual(
            FigurePresentation::new(
                FigureMeasurement::new(pulse.radius * 2.0, pulse.radius * 2.0, None),
                Dimension::new(pulse.radius * 2.0, pulse.radius * 2.0),
                Rectangle::new(
                    -pulse.radius,
                    -pulse.radius,
                    pulse.radius * 2.0,
                    pulse.radius * 2.0,
                ),
                std::sync::Arc::new(PulseDrawing {
                    radius: pulse.radius,
                    color: pulse.color,
                }),
            )
            .map_err(|_| AnimationError::InvalidValue)?,
            Affine2D::from_translation(final_point.x(), final_point.y()),
            if pulse.handoff.is_some() { 0.0 } else { 1.0 },
        )?;

        let build_result = (|| {
            let mut travel = Vec::with_capacity(segments.len());
            let mut elapsed = 0_u64;
            let mut traversed = 0.0;
            for (index, (start, end, length)) in segments.iter().copied().enumerate() {
                traversed += length;
                let segment_end = if index + 1 == segments.len() {
                    duration_micros
                } else {
                    ((traversed / total_length) * duration_micros as f64).round() as u64
                };
                let segment_duration = segment_end.saturating_sub(elapsed);
                elapsed = segment_end;
                travel.push(AnimationPlan::track(
                    visual.transform(),
                    Motion::Tween(Tween::between(
                        Affine2D::from_translation(start.x(), start.y()),
                        Affine2D::from_translation(end.x(), end.y()),
                        Duration::from_micros(segment_duration),
                    )?),
                )?);
            }
            let travel = AnimationPlan::sequence(travel)?;
            let plan = if let Some((decoration, handoff_duration)) = pulse.handoff {
                let decoration = self
                    .bind_figure(decoration, InteractionGeometryPolicy::Committed)?
                    .transform();
                let visible_travel = AnimationPlan::parallel(vec![
                    travel,
                    AnimationPlan::track(
                        visual.opacity(),
                        Motion::Tween(Tween::between(
                            Opacity::OPAQUE,
                            Opacity::OPAQUE,
                            pulse.duration,
                        )?),
                    )?,
                ])?;
                let handoff = AnimationPlan::parallel(vec![
                    AnimationPlan::track(
                        visual.opacity(),
                        Motion::Tween(Tween::between(
                            Opacity::OPAQUE,
                            Opacity::TRANSPARENT,
                            handoff_duration,
                        )?),
                    )?,
                    AnimationPlan::track(
                        decoration,
                        Motion::Tween(Tween::between(
                            Affine2D::from_uniform_scale(1.35),
                            Affine2D::IDENTITY,
                            handoff_duration,
                        )?),
                    )?,
                ])?;
                AnimationPlan::sequence(vec![visible_travel, handoff])?
            } else {
                travel
            };
            self.start(plan.with_suspension(SuspensionPolicy::Pause))
        })();
        match build_result {
            Ok(start @ AnimationStart::Running(_)) => Ok(start),
            Ok(start) => {
                let _ = self.remove_temporary_visual(visual.id());
                Ok(start)
            }
            Err(error) => {
                let _ = self.remove_temporary_visual(visual.id());
                Err(error)
            }
        }
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
            let bounds_value = self.tree.figure_bounds(figure)?;
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
    ///
    /// This legacy entry preserves affine size scaling.
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

        let mut changed = Vec::new();
        for (figure, before) in capture.bounds {
            let after = self.tree.figure_bounds(figure)?;
            if before == after {
                continue;
            }
            changed.push((figure, before, after));
        }
        if changed.is_empty() {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }
        // Static modes validate target identity, but do not require an
        // interpolable transform or allocate presentation channels.
        match self.service.mode {
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
        let targets = changed
            .into_iter()
            .map(|(figure, before, after)| {
                bounds_transition_transform(before, after).map(|transform| (figure, transform))
            })
            .collect::<Result<Vec<_>, _>>()?;
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
        self.service.validate_active_budget(targets.len(), 0, 0)?;

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

    /// Consumes complete sampled geometry and animates real bounds toward stable source bounds.
    ///
    /// All captured dependency identities and child order are checked before channel preparation.
    /// Unlike the affine transition, finite zero-sized endpoints are supported.
    pub fn transition_layout_bounds(
        &mut self,
        capture: super::GeometryCapture,
        transition: BoundsTransition,
    ) -> Result<AnimationStart, AnimationError> {
        self.ensure_not_faulted()?;
        if capture
            .geometry
            .data
            .roots
            .iter()
            .any(|figure| figure.namespace() != self.service.namespace)
        {
            return Err(AnimationError::ForeignCapture);
        }
        let mut changed = Vec::new();
        for id in &capture.geometry.data.order {
            let before = capture.geometry.figure(*id)?;
            let node = self.tree.node(*id)?;
            if node.parent != before.parent || node.children != before.children {
                return Err(AnimationError::GeometryTopologyChanged { figure: *id });
            }
            if capture.geometry.data.covered.contains(id) && before.bounds != node.figure_bounds() {
                changed.push((
                    *id,
                    super::LayoutBounds::try_new(before.bounds)?,
                    super::LayoutBounds::try_new(node.figure_bounds())?,
                ));
            }
        }
        if changed.is_empty() || transition.duration.is_zero() {
            return Ok(AnimationStart::Suppressed(
                AnimationSuppression::NoVisualDelta,
            ));
        }
        match self.service.mode {
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
        // Validate all Motion parameters before creating or refreshing any channels.
        let motions = changed
            .iter()
            .map(|(_, before, after)| {
                Tween::between(*before, *after, transition.duration)
                    .map(|tween| Motion::Tween(tween.with_easing(transition.easing)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let required = changed
            .iter()
            .filter(|(figure, _, _)| {
                !self
                    .service
                    .geometry_channels
                    .contains_key(&FigurePresentationChannelKey {
                        figure: *figure,
                        value_type: TypeId::of::<super::LayoutBounds>(),
                    })
            })
            .count();
        if self.service.channels.len().saturating_add(required) > DEFAULT_MAX_ANIMATION_CHANNELS {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::Channels,
            ));
        }
        self.service.limits.validate_staging(changed.len())?;
        let tree = self.tree;
        let resources = self.resources;
        let source = self.guarded_service(|service| {
            super::frozen::FrozenGeometrySource::capture(
                tree,
                resources,
                capture.geometry.data.roots.clone(),
                service.limits.staging_work,
            )
        })?;
        let revision = self.service.binding_revision;
        let mut restored = Vec::new();
        let mut created = Vec::new();
        let mut plans = Vec::with_capacity(changed.len());
        for ((figure, _, after), motion) in changed.into_iter().zip(motions) {
            let key = FigurePresentationChannelKey {
                figure,
                value_type: TypeId::of::<super::LayoutBounds>(),
            };
            let existed = self.service.geometry_channels.contains_key(&key);
            let channel = self.bind_geometry(figure, super::GeometryBinding::layout_bounds())?;
            if existed {
                let state = self
                    .service
                    .channels
                    .get_mut(channel.id())
                    .expect("bound channel");
                restored.push((
                    channel.id(),
                    std::mem::replace(&mut state.committed, Box::new(after)),
                ));
            } else {
                created.push((key, channel.id()));
            }
            plans.push(AnimationPlan::track(channel, motion)?);
        }
        let result = (|| {
            let mut plan = if transition.stagger.is_zero() {
                AnimationPlan::parallel(plans)?
            } else {
                AnimationPlan::stagger(transition.stagger, plans)?
            };
            plan.geometry_source = Some(source);
            self.start(
                plan.with_interruption(transition.interruption)
                    .with_suspension(transition.suspension),
            )
        })();
        if !matches!(result, Ok(AnimationStart::Running(_))) {
            for (id, committed) in restored {
                self.service
                    .channels
                    .get_mut(id)
                    .expect("existing channel")
                    .committed = committed;
            }
            for (key, id) in created {
                self.service.geometry_channels.remove(&key);
                self.service.channels.remove(id);
            }
            self.service.binding_revision = revision;
        } else {
            self.service.binding_revision = self.service.binding_revision.saturating_add(1);
        }
        result
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

    /// Admits and starts a finite or continuous plan.
    pub fn start(&mut self, mut plan: AnimationPlan) -> Result<AnimationStart, AnimationError> {
        if *self.faulted {
            return Err(AnimationError::RuntimeFaulted);
        }
        if let Some(suppressed) = self.service.suppressed_start(&plan, false)? {
            return Ok(suppressed);
        }
        self.freeze_plan_source(&mut plan)?;
        let before = self.service.damage_state(self.tree);
        let tree = self.tree;
        let result = self.guarded_service(|service| service.start(plan, tree));
        if result.is_ok() {
            self.reconcile_damage(before);
        }
        result
    }

    /// Atomically replaces one active plan while preserving current presentation values.
    pub fn retarget(
        &mut self,
        animation: AnimationId,
        mut replacement: AnimationPlan,
    ) -> Result<bool, AnimationError> {
        if *self.faulted {
            return Err(AnimationError::RuntimeFaulted);
        }
        self.freeze_plan_source(&mut replacement)?;
        let before = self.service.damage_state(self.tree);
        let tree = self.tree;
        let result = self.guarded_service(|service| service.retarget(animation, replacement, tree));
        if result.is_ok() {
            self.reconcile_damage(before);
        }
        result
    }

    fn freeze_plan_source(&mut self, plan: &mut AnimationPlan) -> Result<(), AnimationError> {
        if plan.geometry_source.is_some() {
            return Ok(());
        }
        let roots: Vec<_> = plan
            .tracks
            .iter()
            .filter_map(
                |track| match &self.service.channels.get(track.spec.channel())?.binding {
                    ChannelBinding::FigureGeometry { figure, .. } => Some(*figure),
                    _ => None,
                },
            )
            .chain(plan.providers.figures())
            .collect();
        if !roots.is_empty() {
            let tree = self.tree;
            let resources = self.resources;
            plan.geometry_source = Some(self.guarded_service(|service| {
                super::frozen::FrozenGeometrySource::capture(
                    tree,
                    resources,
                    roots,
                    service.limits.staging_work,
                )
            })?);
        }
        Ok(())
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

    /// Reads the checked failure of an active or retained terminal group.
    ///
    /// Provider limit errors include the provider's typed identity and declared/actual counts.
    /// Diagnostics share the bounded terminal-history lifetime; this query runs no callbacks.
    pub fn failure(
        &self,
        animation: AnimationId,
    ) -> Result<Option<&AnimationError>, AnimationError> {
        self.state(animation)?;
        Ok(self.service.failures.get(&animation))
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
            if old.is_some_and(DamageSubjectState::has_content_presentation)
                || new.is_some_and(DamageSubjectState::has_content_presentation)
            {
                *self.full_redraw_pending = true;
            }
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
        match self {
            Self::Figure { envelope, .. } => *envelope,
            Self::Temporary {
                envelope,
                active: true,
                ..
            } => *envelope,
            Self::Temporary { active: false, .. } => None,
        }
    }

    fn has_content_presentation(&self) -> bool {
        matches!(
            self,
            Self::Figure {
                content_revision: Some(_),
                ..
            }
        )
    }
}
