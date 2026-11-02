//! Read-only diagnostics for stable Novadraw Figure scenes.
//!
//! The crate intentionally keeps UI concerns out of the engine. Native, web, and
//! headless hosts consume the same tree snapshot and committed-effect timeline.

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
};

use novadraw::advanced::RuntimeNamespace;
use novadraw::event::{
    ListenerDirective, ListenerId, ListenerScope, NotificationEffect, NotificationRecord,
    ObservationListener,
};
use novadraw::geometry::Rectangle;
use novadraw::runtime::{Runtime, RuntimeMutationError, StableQueryError, StableSceneQuery};
use novadraw::tree::{FigureId, FigureTree};

/// A stable, UI-independent view of one attached Figure.
#[derive(Clone, Debug)]
pub struct FigureSnapshot {
    /// Runtime-scoped Figure identity.
    pub id: FigureId,
    /// Direct containment parent, or `None` for the synthetic root.
    pub parent: Option<FigureId>,
    /// Direct child count in paint and z-order order.
    pub child_count: usize,
    /// Depth from the FigureTree synthetic root.
    pub depth: usize,
    /// Stable diagnostic name supplied by the Figure implementation.
    pub figure_name: &'static str,
    /// Figure border-box in its parent-local coordinate domain.
    pub bounds: Rectangle,
    /// Whether this Figure is visible before ancestor visibility is considered.
    pub visible: bool,
    /// Whether this Figure accepts input before ancestor enabled state is considered.
    pub enabled: bool,
    /// Whether this Figure satisfies the current validation state.
    pub valid: bool,
    /// Whether this Figure is locally opaque.
    pub opaque: bool,
    /// Whether this Figure may receive keyboard focus.
    pub focusable: bool,
}

/// A pre-order stable snapshot of the attached FigureTree.
#[derive(Clone, Debug)]
pub struct FigureTreeSnapshot {
    epoch: u64,
    nodes: Vec<FigureSnapshot>,
}

impl FigureTreeSnapshot {
    fn from_tree(epoch: u64, tree: &FigureTree) -> Self {
        let mut nodes = Vec::new();
        let mut pending = vec![tree.root_id()];

        while let Some(id) = pending.pop() {
            let node = tree
                .node(id)
                .expect("snapshot traversal uses live tree identities");
            let children = tree.child_order(id).expect("snapshot node is live");
            nodes.push(FigureSnapshot {
                id,
                parent: tree.parent_id(id).expect("snapshot node is live"),
                child_count: children.len(),
                depth: tree.depth(id).expect("snapshot node is live"),
                figure_name: node.figure_name(),
                bounds: node.figure_bounds(),
                visible: node.is_visible(),
                enabled: node.is_enabled(),
                valid: node.is_valid(),
                opaque: node.is_opaque(),
                focusable: node.is_focusable(),
            });
            pending.extend(children.into_iter().rev());
        }

        Self { epoch, nodes }
    }

    /// Returns the Runtime stable epoch represented by this snapshot.
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns nodes in parent-first, child-order traversal order.
    pub fn nodes(&self) -> &[FigureSnapshot] {
        &self.nodes
    }

    /// Looks up one Figure's snapshot.
    pub fn figure(&self, id: FigureId) -> Option<&FigureSnapshot> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

/// One committed notification effect retained for diagnostic presentation.
#[derive(Clone, Debug)]
pub struct InspectionEvent {
    /// Runtime owning this record; sequences are ordered only within this namespace.
    pub namespace: RuntimeNamespace,
    /// Stable epoch that committed this effect.
    pub source_epoch: u64,
    /// Globally ordered sequence inside the Runtime notification stream.
    pub sequence: u64,
    /// Original typed committed effect.
    pub effect: NotificationEffect,
}

impl From<&NotificationRecord> for InspectionEvent {
    fn from(record: &NotificationRecord) -> Self {
        Self {
            namespace: record.namespace,
            source_epoch: record.source_epoch,
            sequence: record.sequence,
            effect: record.effect.clone(),
        }
    }
}

#[derive(Debug)]
struct InspectorState {
    event_capacity: NonZeroUsize,
    events: VecDeque<InspectionEvent>,
}

impl InspectorState {
    fn push(&mut self, record: &NotificationRecord) {
        if self.events.len() == self.event_capacity.get() {
            self.events.pop_front();
        }
        self.events.push_back(InspectionEvent::from(record));
    }
}

/// Operation discovering or reporting an Inspector-local failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorOperation {
    Events,
    ClearEvents,
    Observe,
    Attach,
    AttachScoped,
}

/// Historical record position at which observer enqueue first published a fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionSource {
    pub namespace: RuntimeNamespace,
    pub epoch: u64,
    pub sequence: u64,
}

/// The first completely published event-buffer poison diagnosis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectorFault {
    pub operation: InspectorOperation,
    pub source: Option<InspectionSource>,
}

/// A local operation failure or a preserved Core registration rejection.
#[derive(Debug)]
pub enum InspectorError {
    Faulted {
        operation: InspectorOperation,
        first_fault: InspectorFault,
    },
    Registration(RuntimeMutationError),
}

impl std::fmt::Display for InspectorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Faulted {
                operation,
                first_fault,
            } => write!(
                formatter,
                "Inspector {operation:?} failed; first event-buffer fault: {first_fault:?}"
            ),
            Self::Registration(source) => write!(formatter, "Inspector registration: {source}"),
        }
    }
}

impl std::error::Error for InspectorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registration(source) => Some(source),
            Self::Faulted { .. } => None,
        }
    }
}

/// Explicit, movable subscription ownership. Drop performs no Runtime work.
///
/// Tokens cannot be cloned or copied:
/// ```compile_fail
/// use novadraw_inspector::InspectionSubscription;
/// fn clone_token(token: &InspectionSubscription) -> InspectionSubscription {
///     (*token).clone()
/// }
/// ```
/// ```compile_fail
/// use novadraw_inspector::InspectionSubscription;
/// fn require_copy<T: Copy>() {}
/// require_copy::<InspectionSubscription>();
/// ```
/// Registration must handle both Inspector and Core rejection:
/// ```compile_fail
/// use novadraw::{event::ListenerId, runtime::Runtime};
/// use novadraw_inspector::FigureInspector;
/// let inspector = FigureInspector::new(std::num::NonZeroUsize::new(8).unwrap());
/// let id: ListenerId = inspector.attach(&mut Runtime::empty());
/// ```
#[derive(Debug)]
#[must_use = "retain this token and explicitly detach from its owning Runtime"]
pub struct InspectionSubscription {
    listener: ListenerId,
    scope: ListenerScope,
}

impl InspectionSubscription {
    pub fn owner(&self) -> RuntimeNamespace {
        self.listener.namespace()
    }

    pub fn listener_id(&self) -> ListenerId {
        self.listener
    }

    pub fn scope(&self) -> ListenerScope {
        self.scope
    }

    /// Removes the actual registration; a failed attempt leaves the token reusable.
    pub fn detach(&self, runtime: &mut Runtime) -> Result<bool, RuntimeMutationError> {
        runtime.observations().remove_listener(self.listener)
    }
}

/// Read-only Figure scene inspector with a bounded committed-effect timeline.
#[derive(Clone, Debug)]
pub struct FigureInspector {
    state: Arc<Mutex<InspectorState>>,
    first_fault: Arc<OnceLock<InspectorFault>>,
}

impl FigureInspector {
    /// Creates an inspector retaining at most `event_capacity` committed effects.
    pub fn new(event_capacity: NonZeroUsize) -> Self {
        Self {
            state: Arc::new(Mutex::new(InspectorState {
                event_capacity,
                events: VecDeque::with_capacity(event_capacity.get()),
            })),
            first_fault: Arc::new(OnceLock::new()),
        }
    }

    /// Registers this inspector as a Runtime-scoped committed-effect observer.
    pub fn attach(&self, runtime: &mut Runtime) -> Result<InspectionSubscription, InspectorError> {
        self.attach_inner(runtime, ListenerScope::Runtime, InspectorOperation::Attach)
    }

    /// Registers with Figure disposal ownership or explicit Runtime ownership.
    pub fn attach_scoped(
        &self,
        runtime: &mut Runtime,
        scope: ListenerScope,
    ) -> Result<InspectionSubscription, InspectorError> {
        self.attach_inner(runtime, scope, InspectorOperation::AttachScoped)
    }

    fn attach_inner(
        &self,
        runtime: &mut Runtime,
        scope: ListenerScope,
        operation: InspectorOperation,
    ) -> Result<InspectionSubscription, InspectorError> {
        self.check_health(operation)?;
        let listener = runtime
            .observations()
            .add_observation_listener_scoped(
                scope,
                Box::new(InspectorObserver {
                    state: Arc::clone(&self.state),
                    first_fault: Arc::clone(&self.first_fault),
                }),
            )
            .map_err(InspectorError::Registration)?;
        Ok(InspectionSubscription { listener, scope })
    }

    /// Reads the shared first fault without locking the event buffer.
    pub fn fault(&self) -> Option<InspectorFault> {
        self.first_fault.get().copied()
    }

    fn check_health(&self, operation: InspectorOperation) -> Result<(), InspectorError> {
        match self.fault() {
            Some(first_fault) => Err(InspectorError::Faulted {
                operation,
                first_fault,
            }),
            None => Ok(()),
        }
    }

    fn publish_error(&self, operation: InspectorOperation) -> InspectorError {
        let first_fault = publish_fault(
            &self.first_fault,
            InspectorFault {
                operation,
                source: None,
            },
        );
        InspectorError::Faulted {
            operation,
            first_fault,
        }
    }

    /// Captures a tree snapshot only after Runtime reports a stable scene.
    pub fn capture_tree(
        &self,
        runtime: &mut Runtime,
    ) -> Result<FigureTreeSnapshot, StableQueryError> {
        let epoch = {
            let observations = runtime.observations();
            observations.checkpoint()?.stable_epoch
        };
        Ok(FigureTreeSnapshot::from_tree(epoch, runtime.tree()))
    }

    /// Returns retained events in FIFO commit order.
    pub fn events(&self) -> Result<Vec<InspectionEvent>, InspectorError> {
        self.check_health(InspectorOperation::Events)?;
        self.state
            .lock()
            .map(|state| state.events.iter().cloned().collect())
            .map_err(|_| self.publish_error(InspectorOperation::Events))
    }

    /// Clears the retained diagnostic timeline without modifying Runtime state.
    pub fn clear_events(&self) -> Result<(), InspectorError> {
        self.check_health(InspectorOperation::ClearEvents)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| self.publish_error(InspectorOperation::ClearEvents))?;
        state.events.clear();
        Ok(())
    }
}

struct InspectorObserver {
    state: Arc<Mutex<InspectorState>>,
    first_fault: Arc<OnceLock<InspectorFault>>,
}

fn publish_fault(cell: &OnceLock<InspectorFault>, candidate: InspectorFault) -> InspectorFault {
    // The initializer copies fixed fields only; it cannot invoke a user callback or panic.
    *cell.get_or_init(|| candidate)
}

impl ObservationListener for InspectorObserver {
    fn observed(
        &self,
        record: &NotificationRecord,
        _latest: StableSceneQuery<'_>,
    ) -> ListenerDirective {
        if self.first_fault.get().is_some() {
            return ListenerDirective::Remove;
        }
        match self.state.lock() {
            Ok(mut state) => {
                state.push(record);
                ListenerDirective::Keep
            }
            Err(_) => {
                publish_fault(
                    &self.first_fault,
                    InspectorFault {
                        operation: InspectorOperation::Observe,
                        source: Some(InspectionSource {
                            namespace: record.namespace,
                            epoch: record.source_epoch,
                            sequence: record.sequence,
                        }),
                    },
                );
                ListenerDirective::Remove
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cost;
