//! Runtime-owned animation clock and timeline primitives.

mod behavior;
mod service;
mod timeline;
mod value;

use std::{
    any::{Any, TypeId},
    fmt,
    marker::PhantomData,
    sync::Arc,
    time::Duration,
};

pub use crate::identity::{AnimationBehaviorId, AnimationId, TemporaryVisualId};
use crate::{
    Affine2D, CapabilityKey, Figure, FigureId, PointList, Rectangle, figure::FigurePresentation,
    identity::AnimationChannelId,
};
pub(crate) use behavior::facts_from_effects;
pub use behavior::{
    AnimationBehavior, AnimationBehaviorContext, AnimationBehaviorFailure, AnimationBehaviorScope,
    AnimationFact, AnimationFactCoalescing, AnimationLifecycle, AnimationPlanFactory,
    AnimationTrigger, PropertySelector, StatePropertySelector,
};
pub use service::AnimationMut;
pub(crate) use service::{AnimationService, FigurePresentationEffect, PresentationSnapshot};
pub use timeline::{AnimationPlan, RepeatBehavior};
pub use value::{
    AnimationValue, Decay, Easing, Keyframe, Keyframes, Motion, Procedural, Spring, Tween,
};

/// Maximum number of registered typed presentation channels in one Runtime.
pub const DEFAULT_MAX_ANIMATION_CHANNELS: usize = 4_096;
/// Maximum number of installed committed-fact behaviors in one Runtime.
pub const DEFAULT_MAX_ANIMATION_BEHAVIORS: usize = 1_024;
/// Maximum number of simultaneously active animations in one Runtime.
pub const DEFAULT_MAX_ACTIVE_ANIMATIONS: usize = 1_024;
/// Maximum total number of tracks owned by active animations.
pub const DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS: usize = 4_096;
/// Maximum number of temporary presentation visuals owned by one Runtime.
pub const DEFAULT_MAX_TEMPORARY_VISUALS: usize = 1_024;
/// Number of terminal animation states retained for diagnostics and queries.
pub const DEFAULT_ANIMATION_TERMINAL_HISTORY: usize = 256;

type CommittedPresentationValue<V> = dyn Fn(&dyn Figure) -> Option<V> + Send + Sync;
type PreparePresentationValue = dyn for<'a> Fn(&dyn Figure, PresentationValues<'a>, Rectangle) -> Option<FigurePresentation>
    + Send
    + Sync;

/// Read-only sampled values for one Figure presentation family.
pub struct PresentationValues<'a> {
    values: &'a [(TypeId, &'a dyn Any)],
}

impl<'a> PresentationValues<'a> {
    pub(crate) const fn new(values: &'a [(TypeId, &'a dyn Any)]) -> Self {
        Self { values }
    }

    /// Returns the sampled or committed value for one channel type in the family.
    pub fn get<V: AnimationValue>(&self) -> Option<&'a V> {
        self.values
            .iter()
            .find_map(|(value_type, value)| (*value_type == TypeId::of::<V>()).then_some(*value))
            .and_then(|value| value.downcast_ref())
    }
}

/// Combines all typed channels in one Figure presentation family.
///
/// Every [`PresentationBinding`] using the same family type shares this implementation, so the
/// resulting content does not depend on channel registration or animation order.
pub trait PresentationFamily<T: Figure> {
    /// Prepares one immutable self-content presentation from the family's effective values.
    fn prepare(figure: &T, values: PresentationValues<'_>, bounds: Rectangle)
    -> FigurePresentation;
}

/// Attach-time adapter from one typed animation value to immutable Figure content.
///
/// The descriptor reads committed truth from its concrete Figure and prepares a complete
/// presentation for a sampled value. Runtime owns channel sampling and publication; the binding
/// never writes the Figure or overrides render traversal.
pub struct PresentationBinding<V> {
    committed: Arc<CommittedPresentationValue<V>>,
    prepare: Arc<PreparePresentationValue>,
    family: TypeId,
}

impl<V> PresentationBinding<V>
where
    V: AnimationValue,
{
    /// Creates a binding backed by one concrete Figure type.
    ///
    /// The concrete downcast remains inside the owned descriptor. Capability consumers never
    /// receive `Any` or need to know `T`.
    pub fn of<T, F>(committed: fn(&T) -> V) -> Self
    where
        T: Figure + 'static,
        F: PresentationFamily<T> + 'static,
    {
        Self {
            committed: Arc::new(move |figure| figure.as_any().downcast_ref::<T>().map(committed)),
            prepare: Arc::new(move |figure, values, bounds| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(|figure| F::prepare(figure, values, bounds))
            }),
            family: TypeId::of::<F>(),
        }
    }

    pub(crate) fn committed(&self, figure: &dyn Figure) -> Option<V> {
        (self.committed)(figure)
    }

    pub(crate) fn prepare(
        &self,
        figure: &dyn Figure,
        values: PresentationValues<'_>,
        bounds: Rectangle,
    ) -> Option<FigurePresentation> {
        (self.prepare)(figure, values, bounds)
    }

    pub(crate) fn family(&self) -> TypeId {
        self.family
    }
}

impl<V> Clone for PresentationBinding<V> {
    fn clone(&self) -> Self {
        Self {
            committed: Arc::clone(&self.committed),
            prepare: Arc::clone(&self.prepare),
            family: self.family,
        }
    }
}

/// Standard presentation binding for Connection route geometry.
pub const CONNECTION_ROUTE_PRESENTATION: CapabilityKey<PresentationBinding<PointList>> =
    CapabilityKey::new("novadraw.animation.connection-route");

/// Standard presentation binding for Connection dash phase.
pub const CONNECTION_DASH_PRESENTATION: CapabilityKey<PresentationBinding<f64>> =
    CapabilityKey::new("novadraw.animation.connection-dash");

/// Geometry used by hit-testing while a presentation override is active.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum InteractionGeometryPolicy {
    /// Input and accessibility continue to use committed Figure geometry.
    #[default]
    Committed,
    /// Input uses the sampled presentation geometry.
    Presentation,
    /// The visual does not participate in input or accessibility.
    NonInteractive,
}

/// Time behavior while a surface or target cannot be presented.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum SuspensionPolicy {
    /// Logical time continues and the next visible frame catches up.
    #[default]
    Advance,
    /// Local timeline time stops until presentation resumes.
    Pause,
    /// The animation terminates and committed state is shown.
    Finish,
}

/// Runtime-wide animation behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnimationMode {
    /// Explicit plans run normally. Core still installs no implicit trigger.
    #[default]
    Enabled,
    /// Plans are validated but no timeline or presentation override is created.
    Disabled,
    /// M01-A uses the static committed fallback; later slices may provide reduced motion plans.
    ReducedMotion,
}

/// Why a valid animation plan did not create an active timeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationSuppression {
    /// The Runtime is in [`AnimationMode::Disabled`].
    Disabled,
    /// Reduced motion selected a static committed fallback.
    ReducedMotionStaticFallback,
    /// The plan has no temporal visual interval.
    NoVisualDelta,
}

/// Result of admitting an animation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationStart {
    /// A new timeline owns its channels.
    Running(AnimationId),
    /// `Ignore` retained an existing channel owner.
    Existing(AnimationId),
    /// No active timeline was created.
    Suppressed(AnimationSuppression),
}

/// Observable lifecycle state for an animation ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationState {
    /// Waiting for the first Runtime monotonic time.
    Scheduled,
    /// Sampling against Runtime monotonic time.
    Running,
    /// Local timeline time is frozen by its suspension policy.
    Paused,
    /// Reached the end of its finite timeline.
    Completed,
    /// Explicitly cancelled, replaced, disabled, or retired.
    Cancelled,
    /// A typed extension produced an invalid sample.
    Failed,
}

/// How a new plan handles an existing owner of the same channel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum InterruptionPolicy {
    /// Atomically cancel existing owners before installing the new plan.
    #[default]
    Replace,
    /// Keep the existing owner and return it from `start`.
    Ignore,
}

/// Resource whose configured animation budget was exhausted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationBudgetKind {
    /// Installed committed-fact behaviors.
    Behaviors,
    /// Registered typed channels.
    Channels,
    /// Active timeline count.
    ActiveAnimations,
    /// Tracks owned by active timelines.
    ActiveTracks,
    /// Runtime-owned temporary visuals.
    TemporaryVisuals,
}

/// Animation construction, admission, identity, or query failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationError {
    /// The owning Runtime crossed its panic boundary.
    RuntimeFaulted,
    /// A duration cannot be represented by the Runtime microsecond clock.
    InvalidDuration,
    /// A value rejected its typed animation invariant.
    InvalidValue,
    /// Spring mass, stiffness, damping, tolerance, or maximum duration was invalid.
    InvalidSpring,
    /// Decay rate or maximum duration was invalid.
    InvalidDecay,
    /// A keyframe offset was not finite or outside `[0, 1]`.
    InvalidKeyframeOffset {
        /// Index of the rejected keyframe.
        index: usize,
    },
    /// Keyframe offsets were not strictly increasing.
    KeyframesOutOfOrder {
        /// Index of the first keyframe that did not increase.
        index: usize,
    },
    /// Keyframes did not include both zero and one endpoints.
    MissingEndpointKeyframe,
    /// A composition contained no tracks.
    EmptyPlan,
    /// A transition capture contained no targets.
    EmptyCapture,
    /// A transition capture contains the same target more than once.
    DuplicateTarget,
    /// A repeat count or repeat duration was invalid.
    InvalidRepeat,
    /// A motion or composition cannot be reversed deterministically.
    InvalidComposition,
    /// Two tracks in one plan overlap on the same channel.
    OverlappingTracks,
    /// A timeline offset or duration overflowed.
    TimeOverflow,
    /// A channel belongs to another Runtime.
    ForeignChannel,
    /// A channel is stale or no longer registered.
    UnknownChannel,
    /// A Runtime-managed presentation channel cannot be committed or removed independently.
    ManagedChannel,
    /// A Figure target belongs to another Runtime.
    ForeignTarget,
    /// A transition capture belongs to another Runtime.
    ForeignCapture,
    /// A Figure target is stale or no longer attached.
    DisposedTarget,
    /// The requested interaction geometry policy is not implemented for this target.
    UnsupportedInteractionGeometry,
    /// The Figure did not register the requested typed presentation binding.
    UnsupportedPresentationBinding {
        /// Stable capability name used for diagnostics.
        capability: &'static str,
    },
    /// The registered presentation binding could not resolve its concrete Figure/value contract.
    InvalidPresentationBinding {
        /// Stable capability name used for diagnostics.
        capability: &'static str,
    },
    /// Old and new bounds cannot form a finite presentation transform.
    IncompatibleBoundsTransition,
    /// A Viewport has no stable contents Figure to animate.
    MissingViewportContents,
    /// A Viewport replaced its contents during one transition transaction.
    ViewportContentsChanged,
    /// Old and new Viewport mappings cannot form a finite presentation transform.
    IncompatibleViewportTransition,
    /// A Connection Figure does not implement presentation-route painting.
    UnsupportedRoutePresentation,
    /// A route cannot be projected into the final Connection-local domain.
    IncompatibleRouteTransition,
    /// A temporary visual is stale or no longer registered.
    UnknownTemporaryVisual,
    /// An animation ID belongs to another Runtime.
    ForeignAnimation,
    /// An animation ID is stale or no longer retained.
    UnknownAnimation,
    /// A behavior ID belongs to another Runtime.
    ForeignBehavior,
    /// A behavior ID is stale or no longer installed.
    UnknownBehavior,
    /// A replacement plan attempted to take a channel owned by another animation.
    ChannelConflict,
    /// A track's terminal value differs from its committed channel value.
    FinalValueMismatch,
    /// A configured Runtime animation limit was reached.
    BudgetExceeded(AnimationBudgetKind),
}

impl fmt::Display for AnimationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeFaulted => formatter.write_str("animation Runtime is faulted"),
            Self::InvalidDuration => formatter.write_str("animation duration is invalid"),
            Self::InvalidValue => formatter.write_str("animation value is invalid"),
            Self::InvalidSpring => formatter.write_str("spring parameters are invalid"),
            Self::InvalidDecay => formatter.write_str("decay parameters are invalid"),
            Self::InvalidKeyframeOffset { index } => {
                write!(formatter, "keyframe {index} has an invalid offset")
            }
            Self::KeyframesOutOfOrder { index } => {
                write!(formatter, "keyframe {index} is not strictly ordered")
            }
            Self::MissingEndpointKeyframe => {
                formatter.write_str("keyframes must start at zero and end at one")
            }
            Self::EmptyPlan => formatter.write_str("animation plan contains no tracks"),
            Self::EmptyCapture => formatter.write_str("animation transition capture is empty"),
            Self::DuplicateTarget => {
                formatter.write_str("animation transition capture contains a duplicate target")
            }
            Self::InvalidRepeat => formatter.write_str("animation repeat is invalid"),
            Self::InvalidComposition => formatter.write_str("animation composition is invalid"),
            Self::OverlappingTracks => {
                formatter.write_str("animation plan overlaps tracks on one channel")
            }
            Self::TimeOverflow => formatter.write_str("animation timeline exceeds time range"),
            Self::ForeignChannel => {
                formatter.write_str("animation channel belongs to another Runtime")
            }
            Self::UnknownChannel => formatter.write_str("animation channel is stale or unknown"),
            Self::ManagedChannel => {
                formatter.write_str("Runtime-managed animation channel cannot be changed directly")
            }
            Self::ForeignTarget => {
                formatter.write_str("animation target belongs to another Runtime")
            }
            Self::ForeignCapture => {
                formatter.write_str("animation transition capture belongs to another Runtime")
            }
            Self::DisposedTarget => formatter.write_str("animation target is disposed"),
            Self::UnsupportedInteractionGeometry => {
                formatter.write_str("animation interaction geometry policy is unsupported")
            }
            Self::UnsupportedPresentationBinding { capability } => {
                write!(
                    formatter,
                    "Figure does not support presentation binding {capability}"
                )
            }
            Self::InvalidPresentationBinding { capability } => {
                write!(
                    formatter,
                    "Figure presentation binding {capability} is invalid"
                )
            }
            Self::IncompatibleBoundsTransition => {
                formatter.write_str("Figure bounds cannot form a finite presentation transition")
            }
            Self::MissingViewportContents => {
                formatter.write_str("Viewport has no contents Figure to animate")
            }
            Self::ViewportContentsChanged => {
                formatter.write_str("Viewport contents changed during animation transaction")
            }
            Self::IncompatibleViewportTransition => {
                formatter.write_str("Viewport mappings cannot form a finite transition")
            }
            Self::UnsupportedRoutePresentation => {
                formatter.write_str("Connection Figure does not support route presentation")
            }
            Self::IncompatibleRouteTransition => {
                formatter.write_str("Connection routes cannot form a finite transition")
            }
            Self::UnknownTemporaryVisual => {
                formatter.write_str("temporary visual is stale or unknown")
            }
            Self::ForeignAnimation => {
                formatter.write_str("animation ID belongs to another Runtime")
            }
            Self::UnknownAnimation => formatter.write_str("animation ID is stale or unknown"),
            Self::ForeignBehavior => {
                formatter.write_str("animation behavior ID belongs to another Runtime")
            }
            Self::UnknownBehavior => {
                formatter.write_str("animation behavior ID is stale or unknown")
            }
            Self::ChannelConflict => formatter.write_str("animation channel is already owned"),
            Self::FinalValueMismatch => {
                formatter.write_str("animation terminal value differs from committed value")
            }
            Self::BudgetExceeded(kind) => write!(formatter, "animation budget exceeded: {kind:?}"),
        }
    }
}

impl std::error::Error for AnimationError {}

/// Failure phase for one Runtime-orchestrated bounds transition transaction.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum AnimationTransactionError<E> {
    /// The committed scene could not stabilize before capture.
    BeforeStabilization(crate::FramePreparationError),
    /// The before-state capture was rejected.
    Capture(AnimationError),
    /// The caller-provided source mutation failed.
    Mutation(E),
    /// The committed scene could not stabilize after source mutation.
    AfterStabilization(crate::FramePreparationError),
    /// The presentation transition was rejected after source mutation committed.
    Transition(AnimationError),
}

impl<E: fmt::Display> fmt::Display for AnimationTransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeStabilization(error) => {
                write!(
                    formatter,
                    "animation pre-capture stabilization failed: {error}"
                )
            }
            Self::Capture(error) => write!(formatter, "animation capture failed: {error}"),
            Self::Mutation(error) => write!(formatter, "animation source mutation failed: {error}"),
            Self::AfterStabilization(error) => {
                write!(
                    formatter,
                    "animation post-mutation stabilization failed: {error}"
                )
            }
            Self::Transition(error) => write!(formatter, "animation transition failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for AnimationTransactionError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BeforeStabilization(error) | Self::AfterStabilization(error) => Some(error),
            Self::Capture(error) | Self::Transition(error) => Some(error),
            Self::Mutation(error) => Some(error),
        }
    }
}

/// Typed handle for one Runtime-owned presentation channel.
#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnimationChannel<V> {
    id: AnimationChannelId,
    value: PhantomData<fn() -> V>,
}

impl<V> AnimationChannel<V> {
    pub(crate) fn new(id: AnimationChannelId) -> Self {
        Self {
            id,
            value: PhantomData,
        }
    }

    pub(crate) fn id(self) -> AnimationChannelId {
        self.id
    }
}

impl<V> Clone for AnimationChannel<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for AnimationChannel<V> {}

/// Validated opacity value used by presentation channels.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Opacity(f64);

impl Opacity {
    /// Fully transparent opacity.
    pub const TRANSPARENT: Self = Self(0.0);
    /// Fully opaque opacity.
    pub const OPAQUE: Self = Self(1.0);

    /// Creates an opacity in the inclusive range `[0, 1]`.
    pub fn try_new(value: f64) -> Result<Self, AnimationError> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AnimationError::InvalidValue)
        }
    }

    /// Returns the scalar opacity.
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl AnimationValue for Opacity {
    fn is_valid(&self) -> bool {
        self.0.is_finite() && (0.0..=1.0).contains(&self.0)
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Self((self.0 + (target.0 - self.0) * progress).clamp(0.0, 1.0))
    }
}

/// One-shot committed Figure bounds captured before a source transaction.
pub struct FigureTransitionCapture {
    pub(crate) namespace: crate::RuntimeNamespace,
    pub(crate) bounds: Vec<(FigureId, Rectangle)>,
}

/// Timing and composition policy for a Figure bounds transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundsTransition {
    pub(crate) duration: Duration,
    pub(crate) easing: Easing,
    pub(crate) stagger: Duration,
    pub(crate) interruption: InterruptionPolicy,
    pub(crate) suspension: SuspensionPolicy,
}

impl BoundsTransition {
    /// Creates a linear bounds transition with no stagger.
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
            stagger: Duration::ZERO,
            interruption: InterruptionPolicy::Replace,
            suspension: SuspensionPolicy::Advance,
        }
    }

    /// Replaces the easing curve.
    pub const fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Delays successive targets by stable capture order.
    pub const fn with_stagger(mut self, stagger: Duration) -> Self {
        self.stagger = stagger;
        self
    }

    /// Replaces the channel interruption policy.
    pub const fn with_interruption(mut self, interruption: InterruptionPolicy) -> Self {
        self.interruption = interruption;
        self
    }

    /// Replaces hidden and surface suspension behavior.
    pub const fn with_suspension(mut self, suspension: SuspensionPolicy) -> Self {
        self.suspension = suspension;
        self
    }
}

/// Timing policy for a Viewport pan, zoom, or fit transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportTransition {
    pub(crate) duration: Duration,
    pub(crate) easing: Easing,
    pub(crate) interruption: InterruptionPolicy,
    pub(crate) suspension: SuspensionPolicy,
}

impl ViewportTransition {
    /// Creates a linear Viewport transition.
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
            interruption: InterruptionPolicy::Replace,
            suspension: SuspensionPolicy::Advance,
        }
    }

    /// Replaces the easing curve.
    pub const fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Replaces the channel interruption policy.
    pub const fn with_interruption(mut self, interruption: InterruptionPolicy) -> Self {
        self.interruption = interruption;
        self
    }

    /// Replaces hidden and surface suspension behavior.
    pub const fn with_suspension(mut self, suspension: SuspensionPolicy) -> Self {
        self.suspension = suspension;
        self
    }
}

/// Timing policy for committed Connection route changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionRouteTransition {
    pub(crate) duration: Duration,
    pub(crate) easing: Easing,
    pub(crate) interruption: InterruptionPolicy,
    pub(crate) suspension: SuspensionPolicy,
}

impl ConnectionRouteTransition {
    /// Creates a linear route transition.
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
            interruption: InterruptionPolicy::Replace,
            suspension: SuspensionPolicy::Advance,
        }
    }

    /// Replaces the easing curve.
    pub const fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Replaces the channel interruption policy.
    pub const fn with_interruption(mut self, interruption: InterruptionPolicy) -> Self {
        self.interruption = interruption;
        self
    }

    /// Replaces hidden and surface suspension behavior.
    pub const fn with_suspension(mut self, suspension: SuspensionPolicy) -> Self {
        self.suspension = suspension;
        self
    }
}

/// Arc-length Connection pulse and optional endpoint-decoration handoff.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionPulse {
    pub(crate) duration: Duration,
    pub(crate) radius: f64,
    pub(crate) color: crate::Color,
    pub(crate) handoff: Option<(FigureId, Duration)>,
}

impl ConnectionPulse {
    /// Creates a pulse that travels from source to target.
    pub const fn new(duration: Duration, radius: f64, color: crate::Color) -> Self {
        Self {
            duration,
            radius,
            color,
            handoff: None,
        }
    }

    /// Adds a terminal scale/fade handoff to an endpoint decoration.
    pub const fn with_endpoint_handoff(mut self, decoration: FigureId, duration: Duration) -> Self {
        self.handoff = Some((decoration, duration));
        self
    }
}

/// Typed channels that affect one attached Figure only during frame presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigurePresentationChannels {
    figure: FigureId,
    opacity: AnimationChannel<Opacity>,
    transform: AnimationChannel<Affine2D>,
}

impl FigurePresentationChannels {
    pub(crate) fn new(
        figure: FigureId,
        opacity: AnimationChannel<Opacity>,
        transform: AnimationChannel<Affine2D>,
    ) -> Self {
        Self {
            figure,
            opacity,
            transform,
        }
    }

    /// Returns the committed Figure subject.
    pub fn figure(self) -> FigureId {
        self.figure
    }

    /// Returns the absolute presentation opacity channel.
    pub fn opacity(self) -> AnimationChannel<Opacity> {
        self.opacity
    }

    /// Returns the node-local presentation transform channel.
    pub fn transform(self) -> AnimationChannel<Affine2D> {
        self.transform
    }
}

/// Runtime-owned, non-interactive visual that exists only while its plan is active.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemporaryVisual {
    id: TemporaryVisualId,
    opacity: AnimationChannel<Opacity>,
    transform: AnimationChannel<Affine2D>,
}

impl TemporaryVisual {
    pub(crate) fn new(
        id: TemporaryVisualId,
        opacity: AnimationChannel<Opacity>,
        transform: AnimationChannel<Affine2D>,
    ) -> Self {
        Self {
            id,
            opacity,
            transform,
        }
    }

    /// Returns the Runtime-scoped visual identity.
    pub fn id(self) -> TemporaryVisualId {
        self.id
    }

    /// Returns the visual's absolute opacity channel.
    pub fn opacity(self) -> AnimationChannel<Opacity> {
        self.opacity
    }

    /// Returns the visual's logical-surface transform channel.
    pub fn transform(self) -> AnimationChannel<Affine2D> {
        self.transform
    }
}

/// Cumulative deterministic work counters for the animation core.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnimationStats {
    /// Successful Runtime clock advances observed by the service.
    pub clock_advances: u64,
    /// Typed tracks sampled, including initial admission samples.
    pub tracks_sampled: u64,
}
