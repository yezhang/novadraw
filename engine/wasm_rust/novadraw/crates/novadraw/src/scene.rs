//! Immutable scene capture and bounded producer-to-host handoff.
//!
//! A producer owns all live Figures. A handoff owns only frozen results and remains
//! readable after that producer is dropped. It has no frame acknowledgement authority.

pub use crate::runtime::runtime::scene_capture::RuntimeScenes;

pub(crate) mod group;
pub(crate) mod import;
pub(crate) mod view;
pub use group::{SceneAdmissionError, SceneBindingGroup, SceneDependencyEvidence};
pub use view::{
    ASPECT_FIT_VIEW, AspectFitView, InstalledScene, SCENE_VIEW, SceneViewCapability,
    SceneViewConsumer, SceneViewFigure, SceneViewHandle, SceneViewPlacement, SceneViewPolicy,
    SceneViewPolicyKey,
};

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Mutex},
    task::Waker,
};

use crate::{
    advanced::RuntimeNamespace,
    figure::ResolvedStyle,
    geometry::Rectangle,
    graphics::GraphicsError,
    render::{InvalidGraphicsInput, RecordedDrawing},
    runtime::{FramePreparationError, ResourceError},
    tree::{FigureId, TreeQueryError},
};

/// A never-reused source identity in its producer's namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SceneSourceId {
    pub(crate) namespace: RuntimeNamespace,
    pub(crate) sequence: u64,
}

/// A never-reused subscription identity, separate from its source.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SceneSubscriptionId {
    pub(crate) namespace: RuntimeNamespace,
    pub(crate) sequence: u64,
}

/// Never-reused destination identity. Rebinding retires the preceding identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SceneViewBindingId {
    pub(crate) namespace: RuntimeNamespace,
    pub(crate) sequence: u64,
}

/// Typed identity for external view dependency descriptors.
pub struct SceneViewDependencyKey<P>(SceneCapturePolicyKey<P>);

impl<P> SceneViewDependencyKey<P> {
    /// Creates a namespaced descriptor identity.
    pub const fn new(namespace: &'static str, name: &'static str) -> Self {
        Self(SceneCapturePolicyKey::new(namespace, name))
    }
}
impl<P> Copy for SceneViewDependencyKey<P> {}
impl<P> Clone for SceneViewDependencyKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Open structural source declaration. Drawing, visibility and clipping cannot
/// override Core's subtree membership rule.
pub trait SceneViewDependencyPolicy<P>: 'static {
    /// Resolves the view's source from its typed immutable configuration.
    fn source(&self, payload: &P) -> Result<SceneSourceId, SceneError>;
}

/// Default descriptor uses the same registry and admission as external views.
pub const SCENE_SOURCE_VIEW: SceneViewDependencyKey<SceneSourceId> =
    SceneViewDependencyKey::new("novadraw.scene", "source-view");

struct SourceViewDependency;
impl SceneViewDependencyPolicy<SceneSourceId> for SourceViewDependency {
    fn source(&self, payload: &SceneSourceId) -> Result<SceneSourceId, SceneError> {
        Ok(*payload)
    }
}

/// Monotonically increasing configuration version within one subscription.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CaptureConfigRevision(pub(crate) u64);

impl CaptureConfigRevision {
    /// Returns the diagnostic revision number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Checked capture or handoff failure. Failures never publish partial drawings.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneError {
    /// Shared structural dependencies rejected this candidate before commit.
    Admission(SceneAdmissionError),
    /// The producer is faulted.
    Faulted,
    /// Source identity is unknown, foreign or retired.
    UnknownSource,
    /// Subscription identity is unknown, foreign or closed.
    UnknownSubscription,
    /// Delivery targets a retired binding or an unexpected source/configuration.
    StaleSceneDelivery,
    /// Equal complete content identity carries different immutable data.
    SnapshotConflict,
    /// Policy identity was already registered.
    DuplicatePolicy,
    /// Policy identity and payload type were not registered together.
    UnknownPolicy,
    /// Identity or version counter cannot advance without wrapping.
    Exhausted,
    /// Capture geometry is negative, non-finite or overflows an edge.
    InvalidCaptureRectangle,
    /// An external policy rejected its configuration.
    Policy(String),
    /// Source tree query failed.
    Tree(TreeQueryError),
    /// Source derived state did not stabilize.
    Stabilization(FramePreparationError),
    /// Recording failed.
    Graphics(GraphicsError),
    /// A referenced immutable resource is unavailable.
    Resource(ResourceError),
    /// Resource kind, dimensions or revision does not match its command.
    ResourceMismatch,
    /// A command contains invalid numeric input.
    InvalidGraphicsInput(InvalidGraphicsInput),
    /// A host handoff mutex was poisoned.
    HandoffPoisoned,
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scene capture failed: {self:?}")
    }
}

impl std::error::Error for SceneError {}

/// A typed, namespaced identity for a capture payload.
pub struct SceneCapturePolicyKey<P> {
    namespace: &'static str,
    name: &'static str,
    marker: PhantomData<fn() -> P>,
}

impl<P> SceneCapturePolicyKey<P> {
    /// Creates a stable policy identity. Empty names are invalid.
    pub const fn new(namespace: &'static str, name: &'static str) -> Self {
        assert!(!namespace.is_empty() && !name.is_empty());
        Self {
            namespace,
            name,
            marker: PhantomData,
        }
    }

    pub(crate) fn erased(self) -> PolicyIdentity
    where
        P: 'static,
    {
        (self.namespace, self.name, TypeId::of::<P>())
    }
}

impl<P> Copy for SceneCapturePolicyKey<P> {}
impl<P> Clone for SceneCapturePolicyKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Stable source-local input for capture policy selection.
pub struct SceneCaptureInput<'a> {
    /// Root's border-box extent, with external placement removed.
    pub border_box: Rectangle,
    /// Resolved ancestor and root inheritance from the same stable scene.
    pub style: &'a ResolvedStyle,
}

/// Open capture selection protocol. Implementations cannot access mutable Figures.
pub trait SceneCapturePolicy<P>: 'static {
    /// Selects a root-local rectangle. Core retains coordinate and lifecycle authority.
    fn source_rectangle(
        &self,
        payload: &P,
        input: SceneCaptureInput<'_>,
    ) -> Result<Rectangle, SceneError>;
}

/// Default source-local capture configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceLocalCapture {
    /// Explicit source-local rectangle, or the complete border box.
    pub rectangle: Option<Rectangle>,
}

/// Default capture identity, registered through the same path as external policies.
pub const SOURCE_LOCAL_CAPTURE: SceneCapturePolicyKey<SourceLocalCapture> =
    SceneCapturePolicyKey::new("novadraw.scene", "source-local");

struct SourceLocalPolicy;

impl SceneCapturePolicy<SourceLocalCapture> for SourceLocalPolicy {
    fn source_rectangle(
        &self,
        payload: &SourceLocalCapture,
        input: SceneCaptureInput<'_>,
    ) -> Result<Rectangle, SceneError> {
        Ok(payload.rectangle.unwrap_or(input.border_box))
    }
}

type PolicyIdentity = (&'static str, &'static str, TypeId);
type CaptureCallback = dyn Fn(&dyn Any, SceneCaptureInput<'_>) -> Result<Rectangle, SceneError>;

/// Owned, immutable source-local drawing with an exact resource closure.
#[derive(Clone, Debug)]
pub struct SceneSnapshot {
    pub(crate) dependencies: Option<SceneDependencyEvidence>,
    pub(crate) source: SceneSourceId,
    pub(crate) subscription: SceneSubscriptionId,
    pub(crate) config: CaptureConfigRevision,
    pub(crate) stable_epoch: u64,
    pub(crate) rectangle: Rectangle,
    pub(crate) style: ResolvedStyle,
    pub(crate) drawing: RecordedDrawing,
}

impl PartialEq for SceneSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.subscription == other.subscription
            && self.config == other.config
            && self.stable_epoch == other.stable_epoch
            && self.rectangle == other.rectangle
            && self.style == other.style
            && self.drawing == other.drawing
    }
}

impl SceneSnapshot {
    /// Frozen structural evidence. Current admission always rechecks the group.
    pub fn dependencies(&self) -> Option<&SceneDependencyEvidence> {
        self.dependencies.as_ref()
    }
    /// Source identity; never compare epochs across source namespaces.
    pub const fn source(&self) -> SceneSourceId {
        self.source
    }
    /// Independent subscription identity.
    pub const fn subscription(&self) -> SceneSubscriptionId {
        self.subscription
    }
    /// Capture configuration frozen with this drawing.
    pub const fn config_revision(&self) -> CaptureConfigRevision {
        self.config
    }
    /// Stable producer epoch, independent of surface submission.
    pub const fn stable_source_epoch(&self) -> u64 {
        self.stable_epoch
    }
    /// Root-local capture domain; drawing origin is this rectangle's origin.
    pub const fn source_rectangle(&self) -> Rectangle {
        self.rectangle
    }
    /// Frozen inherited style at capture entry.
    pub fn resolved_style(&self) -> &ResolvedStyle {
        &self.style
    }
    /// Validated immutable commands and resource leases.
    pub fn drawing(&self) -> &RecordedDrawing {
        &self.drawing
    }
}

/// Reason a terminal subscription event was published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCloseReason {
    /// Host explicitly cancelled the subscription.
    Cancelled,
    /// Source registration was removed.
    SourceRemoved,
    /// Source Figure was disposed.
    SourceDisposed,
    /// Producer Runtime was dropped.
    ProducerDisposed,
}

/// Terminal event carrying both independently owned identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneClosed {
    /// Retired source.
    pub source: SceneSourceId,
    /// Retired subscription.
    pub subscription: SceneSubscriptionId,
    /// Lifecycle cause.
    pub reason: SceneCloseReason,
}

/// One host-owned event; taking it acknowledges handoff, never presentation.
#[derive(Clone, Debug)]
pub enum SceneEvent {
    /// Latest complete candidate. Unconsumed older candidates may be overwritten.
    Ready(Arc<SceneSnapshot>),
    /// Latest failed attempt; last accepted destination view is retained.
    Error(SceneError),
    /// Terminal state, delivered before all ordinary updates.
    Closed(SceneClosed),
}

struct Mailbox {
    latest: Option<Arc<SceneSnapshot>>,
    error: Option<SceneError>,
    terminal: Option<SceneClosed>,
    closed: bool,
    waker: Option<Waker>,
    capture_requested: bool,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self {
            latest: None,
            error: None,
            terminal: None,
            closed: false,
            waker: None,
            capture_requested: true,
        }
    }
}

/// Bounded handoff that remains valid after source disposal.
#[derive(Clone)]
pub struct SceneHandoff(Arc<Mutex<Mailbox>>);

impl SceneHandoff {
    /// Takes ownership of one event. Closed is always handed over first.
    pub fn take(&self) -> Result<Option<SceneEvent>, SceneError> {
        let mut mailbox = self.0.lock().map_err(|_| SceneError::HandoffPoisoned)?;
        Ok(if let Some(closed) = mailbox.terminal.take() {
            Some(SceneEvent::Closed(closed))
        } else if let Some(error) = mailbox.error.take() {
            Some(SceneEvent::Error(error))
        } else {
            mailbox.latest.take().map(SceneEvent::Ready)
        })
    }

    /// Registers a host wakeup. Already pending events trigger it immediately.
    pub fn set_waker(&self, waker: Waker) -> Result<(), SceneError> {
        let ready = {
            let mut mailbox = self.0.lock().map_err(|_| SceneError::HandoffPoisoned)?;
            let ready = mailbox.latest.is_some()
                || mailbox.error.is_some()
                || mailbox.terminal.is_some()
                || mailbox.capture_requested;
            mailbox.waker = Some(waker.clone());
            ready
        };
        if ready {
            waker.wake();
        }
        Ok(())
    }

    fn update(&self, event: SceneEvent) -> Result<(), SceneError> {
        let waker = {
            let mut mailbox = self.0.lock().map_err(|_| SceneError::HandoffPoisoned)?;
            if mailbox.closed {
                return Err(SceneError::UnknownSubscription);
            }
            mailbox.capture_requested = false;
            match event {
                SceneEvent::Ready(snapshot) => {
                    mailbox.latest = Some(snapshot);
                    mailbox.error = None;
                }
                SceneEvent::Error(error) => mailbox.error = Some(error),
                SceneEvent::Closed(closed) => {
                    mailbox.latest = None;
                    mailbox.error = None;
                    mailbox.closed = true;
                    mailbox.terminal = Some(closed);
                }
            }
            mailbox.waker.clone()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }

    fn clear_candidate(&self) -> Result<Option<Waker>, SceneError> {
        let waker = {
            let mut mailbox = self.0.lock().map_err(|_| SceneError::HandoffPoisoned)?;
            mailbox.latest = None;
            mailbox.error = None;
            mailbox.capture_requested = true;
            mailbox.waker.clone()
        };
        Ok(waker)
    }
}

pub(crate) type CaptureInputs = (u64, u64, crate::event::MonotonicTime, u64);

pub(crate) struct CaptureSubscription {
    pub(crate) source: SceneSourceId,
    pub(crate) config: CaptureConfigRevision,
    pub(crate) policy: PolicyIdentity,
    pub(crate) payload: Box<dyn Any>,
    pub(crate) handoff: SceneHandoff,
    pub(crate) attempted_inputs: Option<CaptureInputs>,
    pub(crate) attempted_member_generation: Option<u64>,
}

pub(crate) struct SceneProducer {
    namespace: RuntimeNamespace,
    next_identity: u64,
    pub(crate) sources: HashMap<SceneSourceId, FigureId>,
    pub(crate) subscriptions: HashMap<SceneSubscriptionId, CaptureSubscription>,
    policies: HashMap<PolicyIdentity, Rc<CaptureCallback>>,
    dependency_policies: HashMap<PolicyIdentity, Rc<DependencyCallback>>,
    pub(crate) bindings: HashMap<SceneViewBindingId, (FigureId, SceneSourceId)>,
    pub(crate) imports: import::ResourceImports,
    pub(crate) view_policies: HashMap<PolicyIdentity, view::ViewPolicy>,
    pub(crate) view_targets: HashMap<SceneViewBindingId, view::ViewTarget>,
    pub(crate) stable_inputs: Option<CaptureInputs>,
}

type DependencyCallback = dyn Fn(&dyn Any) -> Result<SceneSourceId, SceneError>;

impl SceneProducer {
    pub(crate) fn new(namespace: RuntimeNamespace) -> Self {
        let mut producer = Self {
            namespace,
            next_identity: 0,
            sources: HashMap::new(),
            subscriptions: HashMap::new(),
            policies: HashMap::new(),
            dependency_policies: HashMap::new(),
            bindings: HashMap::new(),
            imports: import::ResourceImports::default(),
            view_policies: HashMap::new(),
            view_targets: HashMap::new(),
            stable_inputs: None,
        };
        producer
            .register_policy(SOURCE_LOCAL_CAPTURE, SourceLocalPolicy)
            .expect("empty registry accepts the default capture policy");
        producer
            .register_dependency_policy(SCENE_SOURCE_VIEW, SourceViewDependency)
            .expect("empty registry accepts the default dependency policy");
        producer
            .register_view_policy(ASPECT_FIT_VIEW, view::AspectFitPolicy)
            .expect("empty registry accepts the default display policy");
        producer
    }

    pub(crate) fn register_view_policy<P: Send + Sync + 'static>(
        &mut self,
        key: SceneViewPolicyKey<P>,
        policy: impl SceneViewPolicy<P>,
    ) -> Result<(), SceneError> {
        let identity = key.0.erased();
        if self.view_policies.contains_key(&identity) {
            return Err(SceneError::DuplicatePolicy);
        }
        let policy = Arc::new(policy);
        let measure = policy.clone();
        self.view_policies.insert(
            identity,
            view::ViewPolicy {
                measure: Arc::new(move |payload, natural, constraints| {
                    measure.measure(
                        payload
                            .downcast_ref::<P>()
                            .ok_or(SceneError::UnknownPolicy)?,
                        natural,
                        constraints,
                    )
                }),
                place: Arc::new(move |payload, natural, client| {
                    policy.place(
                        payload
                            .downcast_ref::<P>()
                            .ok_or(SceneError::UnknownPolicy)?,
                        natural,
                        client,
                    )
                }),
            },
        );
        Ok(())
    }

    pub(crate) fn register_policy<P: 'static>(
        &mut self,
        key: SceneCapturePolicyKey<P>,
        policy: impl SceneCapturePolicy<P>,
    ) -> Result<(), SceneError> {
        let identity = key.erased();
        if self.policies.contains_key(&identity) {
            return Err(SceneError::DuplicatePolicy);
        }
        self.policies.insert(
            identity,
            Rc::new(move |payload, input| {
                policy.source_rectangle(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(SceneError::UnknownPolicy)?,
                    input,
                )
            }),
        );
        Ok(())
    }

    pub(crate) fn allocate_identity(&mut self) -> Result<u64, SceneError> {
        let next = self
            .next_identity
            .checked_add(1)
            .ok_or(SceneError::Exhausted)?;
        self.next_identity = next;
        Ok(next)
    }

    pub(crate) fn register_dependency_policy<P: 'static>(
        &mut self,
        key: SceneViewDependencyKey<P>,
        policy: impl SceneViewDependencyPolicy<P>,
    ) -> Result<(), SceneError> {
        let identity = key.0.erased();
        if self.dependency_policies.contains_key(&identity) {
            return Err(SceneError::DuplicatePolicy);
        }
        self.dependency_policies.insert(
            identity,
            Rc::new(move |payload| {
                policy.source(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(SceneError::UnknownPolicy)?,
                )
            }),
        );
        Ok(())
    }

    pub(crate) fn dependency_source<P: 'static>(
        &self,
        key: SceneViewDependencyKey<P>,
        payload: &P,
    ) -> Result<SceneSourceId, SceneError> {
        self.dependency_policies
            .get(&key.0.erased())
            .ok_or(SceneError::UnknownPolicy)?(payload)
    }

    #[cfg(test)]
    pub(crate) fn register_source(
        &mut self,
        figure: FigureId,
    ) -> Result<SceneSourceId, SceneError> {
        let source = SceneSourceId {
            namespace: self.namespace,
            sequence: self.allocate_identity()?,
        };
        self.sources.insert(source, figure);
        Ok(source)
    }

    pub(crate) fn subscribe<P: 'static>(
        &mut self,
        source: SceneSourceId,
        key: SceneCapturePolicyKey<P>,
        payload: P,
    ) -> Result<(SceneSubscriptionId, CaptureConfigRevision, SceneHandoff), SceneError> {
        if !self.sources.contains_key(&source) {
            return Err(SceneError::UnknownSource);
        }
        let policy = key.erased();
        if !self.policies.contains_key(&policy) {
            return Err(SceneError::UnknownPolicy);
        }
        let id = SceneSubscriptionId {
            namespace: self.namespace,
            sequence: self.allocate_identity()?,
        };
        let handoff = SceneHandoff(Arc::new(Mutex::new(Mailbox::default())));
        let config = CaptureConfigRevision(1);
        self.subscriptions.insert(
            id,
            CaptureSubscription {
                source,
                config,
                policy,
                payload: Box::new(payload),
                handoff: handoff.clone(),
                attempted_inputs: None,
                attempted_member_generation: None,
            },
        );
        Ok((id, config, handoff))
    }

    pub(crate) fn configure<P: 'static>(
        &mut self,
        id: SceneSubscriptionId,
        key: SceneCapturePolicyKey<P>,
        payload: P,
    ) -> Result<CaptureConfigRevision, SceneError> {
        let policy = key.erased();
        if !self.policies.contains_key(&policy) {
            return Err(SceneError::UnknownPolicy);
        }
        let subscription = self
            .subscriptions
            .get_mut(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        let next = CaptureConfigRevision(
            subscription
                .config
                .0
                .checked_add(1)
                .ok_or(SceneError::Exhausted)?,
        );
        let waker = subscription.handoff.clear_candidate()?;
        subscription.config = next;
        subscription.policy = policy;
        subscription.attempted_inputs = None;
        subscription.attempted_member_generation = None;
        let previous = std::mem::replace(&mut subscription.payload, Box::new(payload));
        drop(previous);
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(next)
    }

    pub(crate) fn rectangle(
        &self,
        subscription: &CaptureSubscription,
        input: SceneCaptureInput<'_>,
    ) -> Result<Rectangle, SceneError> {
        self.policies
            .get(&subscription.policy)
            .ok_or(SceneError::UnknownPolicy)?(subscription.payload.as_ref(), input)
    }

    pub(crate) fn publish(
        &self,
        id: SceneSubscriptionId,
        result: Result<Arc<SceneSnapshot>, SceneError>,
    ) -> Result<(), SceneError> {
        let subscription = self
            .subscriptions
            .get(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        subscription.handoff.update(match result {
            Ok(snapshot) => SceneEvent::Ready(snapshot),
            Err(error) => SceneEvent::Error(error),
        })
    }

    pub(crate) fn cancel(
        &mut self,
        id: SceneSubscriptionId,
        reason: SceneCloseReason,
    ) -> Result<(), SceneError> {
        let subscription = self
            .subscriptions
            .get(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        subscription
            .handoff
            .update(SceneEvent::Closed(SceneClosed {
                source: subscription.source,
                subscription: id,
                reason,
            }))?;
        self.subscriptions.remove(&id);
        Ok(())
    }

    pub(crate) fn remove_source(
        &mut self,
        id: SceneSourceId,
        reason: SceneCloseReason,
    ) -> Result<(), SceneError> {
        if !self.sources.contains_key(&id) {
            return Err(SceneError::UnknownSource);
        }
        let subscriptions: Vec<_> = self
            .subscriptions
            .iter()
            .filter_map(|(subscription, capture)| (capture.source == id).then_some(*subscription))
            .collect();
        for subscription in subscriptions {
            self.cancel(subscription, reason)?;
        }
        self.sources.remove(&id);
        Ok(())
    }

    pub(crate) fn retire_figures(&mut self, removed: &std::collections::HashSet<FigureId>) {
        self.view_targets.retain(|binding, _| {
            self.bindings
                .get(binding)
                .is_some_and(|(figure, _)| !removed.contains(figure))
        });
        self.bindings
            .retain(|_, (figure, _)| !removed.contains(figure));
        let sources: Vec<_> = self
            .sources
            .iter()
            .filter_map(|(source, figure)| removed.contains(figure).then_some(*source))
            .collect();
        for source in sources {
            // Poison is confined to a host-owned handoff; tree disposal still completes.
            let _ = self.remove_source(source, SceneCloseReason::SourceDisposed);
        }
    }
}

impl Drop for SceneProducer {
    fn drop(&mut self) {
        for (&id, subscription) in &self.subscriptions {
            let _ = subscription.handoff.update(SceneEvent::Closed(SceneClosed {
                source: subscription.source,
                subscription: id,
                reason: SceneCloseReason::ProducerDisposed,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_exhaustion_never_wraps_or_registers_a_source() {
        let mut producer = SceneProducer::new(RuntimeNamespace::new());
        producer.next_identity = u64::MAX;
        let tree = crate::tree::FigureTree::new();
        assert_eq!(
            producer.register_source(tree.root_id()),
            Err(SceneError::Exhausted)
        );
        assert!(producer.sources.is_empty());
        assert_eq!(producer.next_identity, u64::MAX);
    }

    #[test]
    fn configuration_exhaustion_preserves_previous_configuration() {
        let tree = crate::tree::FigureTree::new();
        let mut producer = SceneProducer::new(tree.namespace());
        let source = producer.register_source(tree.root_id()).unwrap();
        let (id, _, _) = producer
            .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
            .unwrap();
        producer.subscriptions.get_mut(&id).unwrap().config = CaptureConfigRevision(u64::MAX);
        assert_eq!(
            producer.configure(id, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default()),
            Err(SceneError::Exhausted)
        );
        assert_eq!(
            producer.subscriptions[&id].config,
            CaptureConfigRevision(u64::MAX)
        );
    }
}
