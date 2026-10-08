//! Read-only diagnostics for stable Novadraw Figure scenes.
//!
//! The crate intentionally keeps UI concerns out of the engine. Native, web, and
//! headless hosts consume the same tree snapshot and committed-effect timeline.

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use novadraw::event::{
    ListenerDirective, ListenerId, NotificationEffect, NotificationRecord, ObservationListener,
};
use novadraw::runtime::{StableQueryError, StableSceneQuery};
use novadraw::{FigureId, FigureTree, Rectangle, Runtime};

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
            let Some(node) = tree.node(id) else {
                continue;
            };
            let children = tree.child_order(id).unwrap_or_default();
            nodes.push(FigureSnapshot {
                id,
                parent: tree.parent_id(id),
                child_count: children.len(),
                depth: tree.depth(id).unwrap_or_default(),
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
        self.events.push_back(InspectionEvent::from(record));
        if self.events.len() > self.event_capacity.get() {
            self.events.pop_front();
        }
    }
}

/// Read-only Figure scene inspector with a bounded committed-effect timeline.
#[derive(Clone, Debug)]
pub struct FigureInspector {
    state: Arc<Mutex<InspectorState>>,
}

impl FigureInspector {
    /// Creates an inspector retaining at most `event_capacity` committed effects.
    pub fn new(event_capacity: NonZeroUsize) -> Self {
        Self {
            state: Arc::new(Mutex::new(InspectorState {
                event_capacity,
                events: VecDeque::with_capacity(event_capacity.get()),
            })),
        }
    }

    /// Registers this inspector as a Runtime-scoped committed-effect observer.
    pub fn attach(&self, runtime: &mut Runtime) -> ListenerId {
        runtime.add_observation_listener(Box::new(InspectorObserver {
            state: Arc::clone(&self.state),
        }))
    }

    /// Captures a tree snapshot only after Runtime reports a stable scene.
    pub fn capture_tree(&self, runtime: &Runtime) -> Result<FigureTreeSnapshot, StableQueryError> {
        let stable = runtime.stable_query()?;
        Ok(FigureTreeSnapshot::from_tree(
            stable.epoch(),
            runtime.tree(),
        ))
    }

    /// Returns retained events in FIFO commit order.
    pub fn events(&self) -> Vec<InspectionEvent> {
        self.state
            .lock()
            .map(|state| state.events.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clears the retained diagnostic timeline without modifying Runtime state.
    pub fn clear_events(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.events.clear();
        }
    }
}

struct InspectorObserver {
    state: Arc<Mutex<InspectorState>>,
}

impl ObservationListener for InspectorObserver {
    fn observed(
        &self,
        record: &NotificationRecord,
        _latest: StableSceneQuery<'_>,
    ) -> ListenerDirective {
        if let Ok(mut state) = self.state.lock() {
            state.push(record);
        }
        ListenerDirective::Keep
    }
}
