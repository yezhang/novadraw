use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

type SubtreeRetirement = (FigureId, crate::graph::RetiredSubtree, Vec<Rectangle>);
type SubtreePublication = (Vec<(FigureId, FigureId)>, Option<SubtreeRetirement>);

use crate::render::{
    BackendCapabilities, BackendSessionId, BuiltinFont, CaretGeometry, DamageMode, FontDescriptor,
    FrameId, NdCanvas, RenderDisposition, RenderFailure, RenderFailureDiagnostic, RenderResult,
    RenderSubmission, ResourceSync, SelectionQuad, SurfaceInfo, TextConstraints, TextError,
    TextInteractionError, TextLayout, TextLayoutEngine, TextMovement, UnsupportedRenderCapability,
};

use crate::animation::{AnimationMut, AnimationService};
use crate::connection::{
    ConnectionRoutingStats, ConnectionRuntime, DependencySubject, FigureTreeRouteSceneRead,
    RouteMetadata, RouteOutput, RouteRequest, TrackedRouteSceneQuery, UnresolvedConnection,
};
use crate::container::layer::LayeredPaneState;
use crate::figure::border::BorderSnapshot;
use crate::geometry::{Dimension, Point, Translatable};
use crate::mutation::{
    ComponentUpdateError, ComponentUpdateReceipt, FigureComponentUpdate, PendingMutation,
    PendingMutationKind, RuntimeMutationError, SizeOverrideKind,
};
use crate::runtime::accessibility::AccessibilityManager;
use crate::runtime::resource::ResourceRegistry;
use crate::runtime::tooltip::TooltipController;
use crate::{
    AccessibilityAction, AccessibilityError, AccessibilityNodeId, AccessibilitySnapshot,
    AccessibilityUpdate, ActionListener, AncestorListener, AnchorGeometry, AnchorGeometryKey,
    AnchorId, Border, CapabilityUpdateError, CapabilityUpdateReceipt, ChildClippingStrategy,
    ClickableSnapshot, ClickableVisualState, ConnectionAnchor, ConnectionId,
    ConnectionLocatorStrategy, ConnectionRouter, ConnectionRuntimeError, ConnectionStateSnapshot,
    CoordinateListener, CoordinateSpace, CursorIcon, DirectRouter, EventDispatcher, Figure,
    FigureCapabilityUpdate, FigureId, FigureListener, FigureStyle, FigureTree, FlowTextPosition,
    FlowTextRange, FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome,
    FocusTraversalPolicy, FontId, InteractionState, Key, KeyModifiers, LabelFigure, LayerError,
    LayerKey, LayerPlacement, LayeredPane, LayoutConstraint, LayoutListener, LayoutManager,
    ListenerId, ListenerScope, MonotonicTime, MouseButton, ObservationListener, PendingMutations,
    PropertyChangeListener, Rectangle, ResourceError, RouteError, RouterBinding, RouterId,
    RoutingConstraint, RoutingGroupScope, SceneDispatchContext, ShapeMutationError,
    StableQueryError, StableSceneQuery, StackLayout, TimeError, TooltipSnapshot, TooltipTiming,
    TooltipUpdate, TreeOrderFocusTraversal, UpdateEvent, UpdateListener, UpdateManager,
    ValidationError, ViewportHandle, WheelEvent, WidgetError, ZoomEvent, ZoomManager,
};

mod background_submission;
mod capability_update;
mod component_update;
mod connection_service;
mod frame_resource;
mod frame_submission;
pub(crate) mod scene_capture;
mod service_facades;
mod viewport_facades;

pub use service_facades::{
    RuntimeAccessibility, RuntimeAnimation, RuntimeConnections, RuntimeFocus, RuntimeFrames,
    RuntimeInput, RuntimeObservations, RuntimeResources, RuntimeText, RuntimeTooltip,
};
pub use viewport_facades::{RuntimeLayers, RuntimeViewports};

const DERIVED_STATE_FEEDBACK_LIMIT: usize = 16;
const DERIVED_WORK_KIND_COUNT: usize = 6;

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum DerivedWorkKind {
    IntrinsicMetrics,
    Layout,
    DependencyInvalidation,
    Routing,
    PostRouteGeometry,
    Presentation,
}

#[derive(Default)]
struct DerivedWorkSet {
    queued: [bool; DERIVED_WORK_KIND_COUNT],
}

impl DerivedWorkSet {
    fn insert(&mut self, kind: DerivedWorkKind) {
        self.queued[kind as usize] = true;
    }

    fn pop_next(&mut self) -> Option<DerivedWorkKind> {
        const ORDER: [DerivedWorkKind; DERIVED_WORK_KIND_COUNT] = [
            DerivedWorkKind::IntrinsicMetrics,
            DerivedWorkKind::Layout,
            DerivedWorkKind::DependencyInvalidation,
            DerivedWorkKind::Routing,
            DerivedWorkKind::PostRouteGeometry,
            DerivedWorkKind::Presentation,
        ];
        ORDER.into_iter().find(|kind| {
            let queued = &mut self.queued[*kind as usize];
            std::mem::take(queued)
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FramePreparationError {
    Faulted,
    EpochExhausted,
    Text(TextError),
    Graphics(crate::graphics::GraphicsError),
    Validation(ValidationError),
    Animation(crate::animation::AnimationError),
    Accessibility(AccessibilityError),
    UnsupportedRenderCapability(UnsupportedRenderCapability),
    InvalidGraphicsInput(crate::render::InvalidGraphicsInput),
    HostBackground(crate::host::HostBackgroundError),
    DidNotConverge,
}

impl fmt::Display for FramePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::EpochExhausted => formatter.write_str("stable scene epoch exhausted"),
            Self::Text(error) => error.fmt(formatter),
            Self::Graphics(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::Animation(error) => error.fmt(formatter),
            Self::Accessibility(error) => error.fmt(formatter),
            Self::UnsupportedRenderCapability(error) => error.fmt(formatter),
            Self::InvalidGraphicsInput(error) => error.fmt(formatter),
            Self::HostBackground(error) => error.fmt(formatter),
            Self::DidNotConverge => formatter.write_str("derived state did not converge"),
        }
    }
}

impl std::error::Error for FramePreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Text(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FramePreparation {
    Ready(RenderSubmission),
    Idle,
    Suspended,
    AwaitingCompletion,
    Error(FramePreparationError),
}

#[derive(Clone, Debug)]
pub enum FrameNotReady {
    Idle,
    Suspended,
    AwaitingCompletion,
    Error(FramePreparationError),
}

impl FramePreparation {
    /// Returns the prepared submission or the non-ready preparation state.
    pub fn into_ready(self) -> Result<RenderSubmission, FrameNotReady> {
        match self {
            Self::Ready(submission) => Ok(submission),
            Self::Idle => Err(FrameNotReady::Idle),
            Self::Suspended => Err(FrameNotReady::Suspended),
            Self::AwaitingCompletion => Err(FrameNotReady::AwaitingCompletion),
            Self::Error(error) => Err(FrameNotReady::Error(error)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendSessionError {
    Faulted,
    Exhausted,
}

/// Runtime recovery decision after one backend submission result.
#[must_use = "the host must apply the Runtime's render recovery decision"]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionAcknowledgement {
    /// The frame and its resource cursor were committed.
    Presented,
    /// The frame was intentionally not published and remains pending.
    Skipped,
    /// The frame remains pending on the current backend session.
    Retry,
    /// The current surface must be recovered before retrying the frame.
    SurfaceInvalidated { diagnostic: RenderFailureDiagnostic },
    /// The old backend session is invalid and the replacement must consume a new snapshot.
    BackendSessionInvalidated {
        previous_session: BackendSessionId,
        next_session: BackendSessionId,
        diagnostic: RenderFailureDiagnostic,
    },
    /// The backend deterministically rejected this submission.
    Rejected(RenderFailure),
    /// Automatic submission must stop until the application replaces or repairs the backend.
    Terminated(RenderFailure),
}

/// Failure to match a backend result to the Runtime's in-flight frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmissionCompletionError {
    Faulted,
    NoInFlightFrame,
    MismatchedSession {
        expected: BackendSessionId,
        actual: BackendSessionId,
    },
    MismatchedFrame {
        expected: FrameId,
        actual: FrameId,
    },
    BackendSessionExhausted,
}

/// Cumulative work counters for Runtime-owned text layout refreshes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextLayoutStats {
    /// Style-propagation nodes inspected while refreshing Label intrinsic layouts.
    pub label_style_nodes_visited: u64,
    /// Label figures considered for intrinsic layout refresh.
    pub label_figures_refreshed: u64,
    /// Compatibility counter; TextFlow now uses the shared preparation pipeline.
    pub text_flow_style_nodes_visited: u64,
    /// Compatibility counter; no separate TextFlow refresh is performed.
    pub text_flow_figures_refreshed: u64,
}

impl fmt::Display for BackendSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::Exhausted => formatter.write_str("backend session id space is exhausted"),
        }
    }
}

impl std::error::Error for BackendSessionError {}

impl fmt::Display for SubmissionCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::NoInFlightFrame => formatter.write_str("no render submission is in flight"),
            Self::MismatchedSession { expected, actual } => write!(
                formatter,
                "submission session mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::MismatchedFrame { expected, actual } => write!(
                formatter,
                "submission frame mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::BackendSessionExhausted => {
                formatter.write_str("backend session id space is exhausted")
            }
        }
    }
}

impl std::error::Error for SubmissionCompletionError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogicalViewportResizeError {
    Faulted,
    InvalidSize { width: f64, height: f64 },
}

impl fmt::Display for LogicalViewportResizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::InvalidSize { width, height } => {
                write!(
                    formatter,
                    "invalid logical viewport size: {width} x {height}"
                )
            }
        }
    }
}

impl std::error::Error for LogicalViewportResizeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextFlowQueryError {
    Coordinate(crate::tree::CoordinateQueryError),
    UnknownFigure(FigureId),
    WrongCapability(FigureId),
    NonInvertibleTransform(FigureId),
    Interaction(TextInteractionError),
}

impl fmt::Display for TextFlowQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure: {id:?}"),
            Self::WrongCapability(id) => {
                write!(
                    formatter,
                    "Figure does not support TextFlow queries: {id:?}"
                )
            }
            Self::NonInvertibleTransform(id) => {
                write!(
                    formatter,
                    "Figure has no invertible surface transform: {id:?}"
                )
            }
            Self::Interaction(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TextFlowQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Interaction(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TextInteractionError> for TextFlowQueryError {
    fn from(error: TextInteractionError) -> Self {
        Self::Interaction(error)
    }
}

/// Owns one scene and enforces its input, mutation, and update transaction boundaries.
pub struct Runtime {
    tree: FigureTree,
    interaction: InteractionState,
    interaction_dispatcher: EventDispatcher,
    tooltip_controller: TooltipController,
    activity_now: MonotonicTime,
    animations: AnimationService,
    accessibility: AccessibilityManager,
    focus_traversal_policy: Box<dyn FocusTraversalPolicy>,
    updates: UpdateManager,
    mutations: PendingMutations,
    full_redraw_pending: bool,
    backend_session_id: BackendSessionId,
    session_sync_pending: bool,
    next_frame_id: FrameId,
    in_flight: Option<InFlightFrame>,
    submission_recovery: Option<crate::render::submission::SubmissionRetention>,
    last_surface: Option<SurfaceInfo>,
    logical_viewport: Option<Rectangle>,
    resources: ResourceRegistry,
    text: Box<dyn TextLayoutEngine>,
    text_layout_stats: TextLayoutStats,
    builtin_fonts: HashMap<BuiltinFont, FontId>,
    layered_panes: HashMap<FigureId, LayeredPaneState>,
    connections: ConnectionRuntime,
    anchor_geometries: HashMap<(FigureId, AnchorGeometryKey), AnchorGeometry>,
    connection_error: Option<ConnectionRuntimeError>,
    deferred_mutation_errors: VecDeque<RuntimeMutationError>,
    derivation_epoch: u64,
    stable_epoch: u64,
    scenes: crate::scene::SceneProducer,
    background: crate::host::BackgroundState,
    last_stabilization_error: Option<FramePreparationError>,
    preparation_budget: crate::figure::PreparationBudget,
    timing: crate::runtime::timing::TimingRecorder,
    faulted: bool,
}

/// Short-lived, Runtime-backed mutation facade for one attached Figure.
pub struct FigureMut<'a> {
    figure: FigureId,
    runtime: &'a mut Runtime,
}

struct InFlightFrame {
    session_id: BackendSessionId,
    id: FrameId,
    resources: ResourceSync,
    retention: Option<crate::render::submission::SubmissionRetention>,
    import_leases: Vec<std::sync::Arc<crate::scene::import::ImportLease>>,
}

impl FigureMut<'_> {
    pub fn figure_id(&self) -> FigureId {
        self.figure
    }

    pub fn reparent(&mut self, new_parent: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.reparent(self.figure, new_parent)
    }

    pub fn set_layout_constraint<C>(&mut self, constraint: C) -> Result<bool, RuntimeMutationError>
    where
        C: LayoutConstraint,
    {
        self.runtime.set_layout_constraint(self.figure, constraint)
    }

    pub fn remove_layout_constraint(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.remove_layout_constraint(self.figure)
    }

    pub fn set_preferred_size(&mut self, size: Dimension) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_preferred_size(self.figure, size)
    }

    pub fn clear_preferred_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_preferred_size(self.figure)
    }

    pub fn set_minimum_size(&mut self, size: Dimension) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_minimum_size(self.figure, size)
    }

    pub fn clear_minimum_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_minimum_size(self.figure)
    }

    pub fn set_maximum_size(&mut self, size: Dimension) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_maximum_size(self.figure, size)
    }

    pub fn clear_maximum_size(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.clear_maximum_size(self.figure)
    }

    pub fn set_bounds(&mut self, bounds: Rectangle) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_bounds(self.figure, bounds)
    }

    pub fn translate(&mut self, dx: f64, dy: f64) -> Result<bool, RuntimeMutationError> {
        self.runtime.translate(self.figure, dx, dy)
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_visible(self.figure, visible)
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_enabled(self.figure, enabled)
    }

    pub fn set_focusable(&mut self, focusable: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_focusable(self.figure, focusable)
    }

    pub fn set_focus_traversable(
        &mut self,
        traversable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_focus_traversable(self.figure, traversable)
    }

    pub fn set_style(&mut self, style: FigureStyle) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_figure_style(self.figure, style)
    }

    pub fn set_opaque(&mut self, opaque: bool) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_opaque(self.figure, opaque)
    }

    pub fn revalidate(&mut self) -> Result<(), RuntimeMutationError> {
        self.runtime.revalidate(self.figure)
    }

    pub fn repaint(&mut self, rect: Option<Rectangle>) -> Result<(), RuntimeMutationError> {
        self.runtime.repaint(self.figure, rect)
    }

    pub fn update_component<U>(
        &mut self,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate,
    {
        self.runtime.update_component(self.figure, update)
    }

    pub fn update_capability<U>(
        &mut self,
        update: U,
    ) -> Result<CapabilityUpdateReceipt, CapabilityUpdateError<U::Error>>
    where
        U: FigureCapabilityUpdate,
    {
        self.runtime.update_capability(self.figure, update)
    }
}

impl FigureMut<'_> {
    /// Publishes an owned complete branch after whole-subtree admission.
    pub fn add_subtree(
        &mut self,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.add_subtree(self.figure, subtree)
    }

    /// Admits a complete replacement before retiring the addressed direct child.
    /// Admission rejection preserves the child and all existing bindings.
    pub fn replace_child_subtree(
        &mut self,
        child: FigureId,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        self.runtime
            .replace_or_add_subtree(self.figure, Some(child), subtree)
    }

    pub fn add(&mut self, figure: Box<dyn Figure>) -> Result<FigureId, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.add_figure(self.figure, figure)
    }

    /// Inserts a new child at `0..=children.len()` as one Runtime operation.
    pub fn insert(
        &mut self,
        index: usize,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.insert_figure(self.figure, index, figure, None)
    }

    /// Atomically inserts a child and its parent-owned layout constraint.
    ///
    /// A rejected index, admission or constraint publishes no partial state.
    pub fn insert_with_constraint<C: LayoutConstraint>(
        &mut self,
        index: usize,
        figure: Box<dyn Figure>,
        constraint: C,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime
            .insert_figure(self.figure, index, figure, Some(Box::new(constraint)))
    }

    pub fn remove(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.remove_figure(self.figure, child)
    }

    pub fn set_layout_manager(
        &mut self,
        manager: Box<dyn LayoutManager>,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.set_layout_manager(self.figure, manager)
    }

    pub fn clear_layout_manager(&mut self) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.clear_layout_manager(self.figure)
    }

    pub fn move_child_to_index(
        &mut self,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.move_child_to_index(self.figure, child, index)
    }

    pub fn set_child_order(&mut self, order: &[FigureId]) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.set_child_order(self.figure, order)
    }

    pub fn bring_child_to_front(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.bring_child_to_front(self.figure, child)
    }

    pub fn send_child_to_back(&mut self, child: FigureId) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime.send_child_to_back(self.figure, child)
    }

    pub fn set_child_clipping_strategy(
        &mut self,
        strategy: ChildClippingStrategy,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_container_figure(self.figure)?;
        self.runtime
            .set_child_clipping_strategy(self.figure, strategy)
    }
}

impl Runtime {
    /// Constructs a Runtime, panicking if the tree declares unavailable resources.
    /// Use [`Self::try_new`] to handle admission errors.
    pub fn new(tree: FigureTree) -> Self {
        Self::with_text_layout_engine(tree, Box::new(crate::render::ParleyTextEngine::new()))
    }

    /// Validates every attached Figure before activating any lifecycle callback.
    ///
    /// The construction registry starts empty. Runtime-owned resource IDs must be
    /// registered after construction and consumed through Runtime mutation APIs.
    pub fn try_new(tree: FigureTree) -> Result<Self, RuntimeMutationError> {
        Self::try_with_text_layout_engine(tree, Box::new(crate::render::ParleyTextEngine::new()))
    }

    /// Constructs a Runtime, panicking if the tree declares unavailable resources.
    /// Use [`Self::try_with_text_layout_engine`] to handle admission errors.
    pub fn with_text_layout_engine(tree: FigureTree, text: Box<dyn TextLayoutEngine>) -> Self {
        Self::try_with_text_layout_engine(tree, text)
            .expect("FigureTree resources must pass Runtime admission")
    }

    /// Uses the same whole-tree admission as [`Self::try_new`] with an external text engine.
    pub fn try_with_text_layout_engine(
        tree: FigureTree,
        text: Box<dyn TextLayoutEngine>,
    ) -> Result<Self, RuntimeMutationError> {
        let namespace = tree.namespace();
        let resources = ResourceRegistry::with_namespace(namespace.as_uuid());
        let mut dependencies = Vec::new();
        for (index, (id, _)) in tree.attached_ids_parent_first().into_iter().enumerate() {
            let node = tree.node(id).expect("attached construction node");
            let admitted =
                Self::admit_resources(&resources, node.figure.as_ref(), &node.capabilities, &[])
                    .map_err(|source| RuntimeMutationError::FigureBatch {
                        index,
                        source: Box::new(source),
                    })?;
            dependencies.push((id, admitted));
        }
        let backend_session_id = BackendSessionId::initial(resources.namespace());
        let mut updates = UpdateManager::with_namespace(namespace);
        for root in tree.invalid_validation_roots() {
            updates.add_invalid_figure(root);
        }
        let mut runtime = Self {
            tree,
            interaction: InteractionState::default(),
            interaction_dispatcher: EventDispatcher,
            tooltip_controller: TooltipController::default(),
            activity_now: MonotonicTime::ZERO,
            animations: AnimationService::new(namespace),
            accessibility: AccessibilityManager::default(),
            focus_traversal_policy: Box::new(TreeOrderFocusTraversal),
            updates,
            mutations: PendingMutations::new(),
            full_redraw_pending: true,
            backend_session_id,
            session_sync_pending: true,
            next_frame_id: FrameId::INITIAL,
            in_flight: None,
            submission_recovery: None,
            last_surface: None,
            logical_viewport: None,
            resources,
            text,
            text_layout_stats: TextLayoutStats::default(),
            builtin_fonts: HashMap::new(),
            layered_panes: HashMap::new(),
            connections: ConnectionRuntime::with_namespace(namespace),
            anchor_geometries: HashMap::new(),
            connection_error: None,
            deferred_mutation_errors: VecDeque::new(),
            derivation_epoch: 0,
            stable_epoch: 0,
            scenes: crate::scene::SceneProducer::new(namespace),
            background: crate::host::BackgroundState::default(),
            last_stabilization_error: None,
            preparation_budget: crate::figure::PreparationBudget::default(),
            timing: crate::runtime::timing::TimingRecorder::default(),
            faulted: false,
        };
        for (id, resources) in dependencies {
            runtime.publish_figure_resources(id, resources);
        }
        runtime.initialize_layered_panes();
        runtime.activate_attached_figures();
        Ok(runtime)
    }

    pub fn empty() -> Self {
        Self::new(FigureTree::new())
    }

    pub fn tree(&self) -> &FigureTree {
        &self.tree
    }

    pub fn figure(&mut self, figure: FigureId) -> Result<FigureMut<'_>, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        Ok(FigureMut {
            figure,
            runtime: self,
        })
    }

    pub fn frames(&mut self) -> RuntimeFrames<'_> {
        RuntimeFrames::new(self)
    }

    pub fn resources(&mut self) -> RuntimeResources<'_> {
        RuntimeResources::new(self)
    }

    pub fn connections(&mut self) -> RuntimeConnections<'_> {
        RuntimeConnections::new(self)
    }

    pub fn input(&mut self) -> RuntimeInput<'_> {
        RuntimeInput::new(self)
    }

    pub fn focus(&mut self) -> RuntimeFocus<'_> {
        RuntimeFocus::new(self)
    }

    pub fn tooltip(&mut self) -> RuntimeTooltip<'_> {
        RuntimeTooltip::new(self)
    }

    pub fn accessibility(&mut self) -> RuntimeAccessibility<'_> {
        RuntimeAccessibility::new(self)
    }

    pub fn animation(&mut self) -> RuntimeAnimation<'_> {
        RuntimeAnimation::new(self)
    }

    pub fn observations(&mut self) -> RuntimeObservations<'_> {
        RuntimeObservations::new(self)
    }

    pub fn text(&mut self) -> RuntimeText<'_> {
        RuntimeText::new(self)
    }

    pub fn viewports(&mut self) -> RuntimeViewports<'_> {
        RuntimeViewports::new(self)
    }

    pub fn layers(&mut self) -> RuntimeLayers<'_> {
        RuntimeLayers::new(self)
    }

    pub(crate) fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub(crate) fn cursor_icon(&self) -> CursorIcon {
        self.interaction
            .cursor_target()
            .and_then(|id| self.tree.resolved_style(id).ok())
            .map_or(CursorIcon::Default, |style| style.cursor)
    }

    pub(crate) fn tooltip_text(&self) -> Option<String> {
        self.interaction
            .hover_source()
            .and_then(|id| self.tree.resolved_style(id).ok())
            .and_then(|style| style.tooltip)
    }

    pub(crate) fn set_tooltip_timing(&mut self, timing: TooltipTiming) -> Result<bool, TimeError> {
        self.tooltip_controller.set_timing(timing)
    }

    pub(crate) fn animations(&mut self) -> AnimationMut<'_> {
        self.animations.access_mut(
            &self.tree,
            &self.resources,
            &mut self.updates,
            &mut self.full_redraw_pending,
            &mut self.faulted,
        )
    }

    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if self.faulted {
            return Ok(false);
        }
        let tooltip_changed = self.tooltip_controller.advance_time(now)?;
        let animation_changed = self
            .animations
            .access_mut(
                &self.tree,
                &self.resources,
                &mut self.updates,
                &mut self.full_redraw_pending,
                &mut self.faulted,
            )
            .advance_time(now)?;
        self.activity_now = now;
        let activity_changed = self.guarded(|runtime| runtime.poll_activities());
        Ok(tooltip_changed || animation_changed || activity_changed)
    }

    pub(crate) fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        if self.faulted {
            return None;
        }
        self.tree
            .activity_ids()
            .into_iter()
            .filter(|id| {
                self.tree.is_attached(*id) == Ok(true)
                    && self.tree.is_effectively_visible(*id) == Ok(true)
                    && self.tree.is_effectively_enabled(*id) == Ok(true)
            })
            .filter_map(|id| self.tree.node(id).ok()?.activity()?.next_wake_deadline())
            .chain(self.tooltip_controller.next_wake_deadline())
            .chain(self.animations.next_wake_deadline())
            .min()
    }

    pub(crate) fn visible_tooltip(&self) -> Option<&TooltipSnapshot> {
        self.tooltip_controller.visible_snapshot()
    }

    pub(crate) fn take_tooltip_updates(&mut self) -> Vec<TooltipUpdate> {
        self.tooltip_controller.take_updates()
    }

    pub(crate) fn accessibility_snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.accessibility.snapshot()
    }

    pub(crate) fn take_accessibility_updates(&mut self) -> Vec<AccessibilityUpdate> {
        self.accessibility.take_updates()
    }

    pub(crate) fn request_accessibility_snapshot(&mut self) {
        self.accessibility.reset();
    }

    pub(crate) fn perform_accessibility_action(
        &mut self,
        node: AccessibilityNodeId,
        action: AccessibilityAction,
    ) -> Result<bool, AccessibilityError> {
        if self.faulted {
            return Err(AccessibilityError::RuntimeFaulted);
        }
        let figure = match node {
            AccessibilityNodeId::Root(namespace) => {
                if namespace != self.tree.namespace() {
                    return Err(AccessibilityError::ForeignRuntime(node));
                }
                return Err(AccessibilityError::UnsupportedAction { node, action });
            }
            AccessibilityNodeId::Figure(figure) => {
                if figure.namespace() != self.tree.namespace() {
                    return Err(AccessibilityError::ForeignRuntime(node));
                }
                figure
            }
        };
        let snapshot = self
            .accessibility
            .snapshot()
            .ok_or(AccessibilityError::Unavailable(node))?;
        let semantic_node = snapshot
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .ok_or(AccessibilityError::UnknownNode(node))?;
        let focusable = semantic_node.state.focusable;
        let default_action = semantic_node.default_action;
        match action {
            AccessibilityAction::Focus if focusable => self
                .request_focus(figure)
                .map(|change| matches!(change, FocusChange::Changed { .. }))
                .map_err(|_| AccessibilityError::Unavailable(node)),
            AccessibilityAction::Default
                if default_action == Some(AccessibilityAction::Default) =>
            {
                self.do_click(figure)
                    .map_err(|_| AccessibilityError::Unavailable(node))
            }
            _ => Err(AccessibilityError::UnsupportedAction { node, action }),
        }
    }

    pub(crate) fn resource_registry(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub(crate) fn builtin_font(&self, font: BuiltinFont) -> Option<FontId> {
        self.builtin_fonts.get(&font).copied()
    }

    pub(crate) fn text_revision(&self) -> u64 {
        self.text.revision()
    }

    pub(crate) fn layout_styled_text(
        &mut self,
        request: &crate::text::StyledTextRequest,
    ) -> Result<crate::text::StyledTextLayout, TextError> {
        if self.faulted {
            return Err(TextError::RuntimeFaulted);
        }
        self.guarded(|runtime| runtime.text.layout_styled(request))
    }

    pub(crate) fn layout_text(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        if self.faulted {
            return Err(TextError::RuntimeFaulted);
        }
        self.guarded(|runtime| runtime.text.layout(text, font, constraints))
    }

    /// Refreshes immutable Label text snapshots from the current Runtime-owned text engine.
    pub(crate) fn refresh_label_layouts(&mut self) -> Result<(), TextError> {
        if self.faulted {
            return Err(TextError::RuntimeFaulted);
        }
        self.guarded(|runtime| {
            runtime.refresh_label_intrinsic_metrics()?;
            runtime.refresh_label_presentations()?;
            Ok(())
        })
    }

    fn refresh_label_intrinsic_metrics(&mut self) -> Result<bool, TextError> {
        let refresh = self
            .tree
            .refresh_label_intrinsic_layouts(self.text.as_mut(), &self.resources)?;
        self.text_layout_stats.label_style_nodes_visited = self
            .text_layout_stats
            .label_style_nodes_visited
            .saturating_add(refresh.style_nodes_visited);
        self.text_layout_stats.label_figures_refreshed = self
            .text_layout_stats
            .label_figures_refreshed
            .saturating_add(refresh.figures_refreshed);
        let changed = refresh.changed;
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(any_changed)
    }

    fn refresh_label_presentations(&mut self) -> Result<bool, TextError> {
        let changed = self
            .tree
            .refresh_label_presentations(self.text.as_mut(), &mut self.updates)?;
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.refresh_label_icon_geometry(figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        Ok(any_changed)
    }

    fn refresh_label_icon_geometry(&mut self, figure: FigureId) {
        let key = AnchorGeometryKey::icon();
        let next = self
            .tree
            .label(figure)
            .and_then(LabelFigure::icon_bounds)
            .filter(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
            .map(AnchorGeometry::Rectangle);
        let map_key = (figure, key.clone());
        let changed = match next {
            Some(geometry) if self.anchor_geometries.get(&map_key) != Some(&geometry) => {
                self.anchor_geometries.insert(map_key, geometry);
                true
            }
            Some(_) => false,
            None => self.anchor_geometries.remove(&map_key).is_some(),
        };
        if changed
            && let Err(error) = self
                .connections
                .invalidate_dependency(&DependencySubject::NamedAnchorRegion(figure, key))
        {
            self.connection_error = Some(error);
        }
    }

    fn refresh_image_figures(&mut self) -> bool {
        let changed = self.tree.refresh_image_figures(&self.resources);
        let any_changed = !changed.is_empty();
        for figure in changed {
            self.tree.mark_invalid(&mut self.updates, figure);
            self.tree.repaint(&mut self.updates, figure, None);
        }
        any_changed
    }

    fn refresh_owner_scoped_borders(&mut self) -> Result<bool, TextError> {
        let Some(contents) = self.tree.contents() else {
            return Ok(false);
        };
        let mut figures = vec![contents];
        figures.extend(self.tree.descendant_ids(contents).expect("live contents"));
        let mut changed = false;
        for figure in figures {
            let has_owner_snapshot = self
                .tree
                .node(figure)
                .expect("live descendant")
                .figure
                .get_border()
                .is_some_and(Border::has_owner_snapshot);
            if !has_owner_snapshot {
                continue;
            }
            let style = self
                .tree
                .resolved_style(figure)
                .expect("attached Figure style");
            let font = style.font.clone();
            let previous = self.tree.border_snapshot(figure).cloned();
            let Some(snapshot) = self
                .tree
                .node(figure)
                .expect("live descendant")
                .figure
                .get_border()
                .map(|border| {
                    border.resolve_owner_snapshot(previous.as_ref(), &font, self.text.as_mut())
                })
                .transpose()
                .map_err(|source| TextError::BorderOwner {
                    figure,
                    source: Box::new(source),
                })?
                .flatten()
            else {
                continue;
            };
            crate::figure::border::BorderMetrics::new(snapshot.insets(), snapshot.preferred_size())
                .map_err(|source| TextError::BorderOwner {
                    figure,
                    source: Box::new(TextError::BorderInput(Box::new(source))),
                })?;
            if self.tree.set_border_snapshot(figure, snapshot) {
                changed = true;
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
        Ok(changed)
    }

    /// Returns cumulative text layout refresh work counters.
    pub(crate) fn text_layout_stats(&self) -> TextLayoutStats {
        self.text_layout_stats
    }

    pub fn set_contents(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.set_contents_subtree(crate::tree::FigureSubtree::new(figure))
            .map(|receipt| receipt.root())
            .map_err(|error| match error {
                RuntimeMutationError::FigureBatch { index: 0, source } => *source,
                error => error,
            })
    }

    /// Replaces contents with a complete description after whole-branch admission.
    ///
    /// Rejection preserves old contents. Lifecycle panic faults Runtime and retains
    /// the actual published scene; the receipt is returned only after callbacks finish.
    pub fn set_contents_subtree(
        &mut self,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| runtime.set_contents_subtree_inner(subtree))
    }

    fn set_contents_subtree_inner(
        &mut self,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        if subtree
            .nodes
            .iter()
            .skip(1)
            .any(|node| node.parent.is_none())
        {
            return Err(RuntimeMutationError::Rejected);
        }
        self.connections
            .validate_disposal()
            .map_err(|_| RuntimeMutationError::Rejected)?;
        let previous = self.tree.contents();
        let owner = subtree.owner;
        let (inserted, retired) =
            self.publish_subtree(self.tree.synthetic_root(), subtree, previous)?;
        let id = inserted[0].0;
        self.tree.designate_contents(id);
        self.accessibility.reset();
        self.full_redraw_pending = true;
        let validation_root = if self.logical_viewport.is_some() {
            self.tree.synthetic_root()
        } else {
            id
        };
        self.tree.mark_invalid(&mut self.updates, validation_root);
        self.tree.repaint(&mut self.updates, id, None);
        if let Some((parent, retired, damage)) = retired {
            self.complete_subtree_retirement(parent, retired, damage);
        }
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        self.retain_interactive_figures();
        self.complete_subtree_attachment(&inserted);
        Ok(crate::tree::SubtreeReceipt::new(
            owner,
            inserted.into_iter().map(|(id, _)| id).collect(),
        ))
    }

    pub(crate) fn add_figure(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| runtime.add_figure_checked(parent, figure))
    }

    /// Appends Figures after validating every input without allocating identities.
    ///
    /// Each tuple supplies an attached parent, a detached Figure and an optional local style.
    /// Capability descriptors are registered once and retained privately for this call.
    /// The complete topology and resource dependencies publish before installation callbacks.
    /// A callback panic faults Runtime and retains this committed batch.
    pub fn add_figures(
        &mut self,
        figures: Vec<(FigureId, Box<dyn Figure>, Option<FigureStyle>)>,
    ) -> Result<Vec<FigureId>, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            let mut pending = std::collections::HashMap::<FigureId, usize>::new();
            let mut prepared = Vec::with_capacity(figures.len());
            for (index, (parent, figure, style)) in figures.into_iter().enumerate() {
                let result = (|| {
                    runtime.validate_attached_figure(parent)?;
                    if runtime.tree.is_layered_pane(parent) {
                        return Err(RuntimeMutationError::LayeredParent(parent));
                    }
                    let count = pending.entry(parent).or_default();
                    let child = runtime
                        .tree
                        .prepare_child(figure, parent, false, *count, style)?;
                    let resources = runtime.admit_figure_resources(
                        child.figure.as_ref(),
                        &child.capabilities,
                        &[],
                    )?;
                    *count += 1;
                    Ok((child, resources))
                })();
                let child = result.map_err(|source| RuntimeMutationError::FigureBatch {
                    index,
                    source: Box::new(source),
                })?;
                prepared.push((parent, child.0, child.1));
            }
            let mut children = Vec::with_capacity(prepared.len());
            let mut dependencies = Vec::with_capacity(prepared.len());
            for (parent, child, resources) in prepared {
                children.push((parent, child));
                dependencies.push((parent, resources));
            }
            let ids = runtime
                .tree
                .attach_prepared_children(&mut runtime.updates, children)?;
            let mut attachments = Vec::with_capacity(ids.len());
            for (&id, (parent, resources)) in ids.iter().zip(dependencies) {
                runtime.publish_figure_resources(id, resources);
                runtime.register_layered_pane(id);
                attachments.push((id, parent));
            }
            runtime.complete_subtree_attachment(&attachments);
            Ok(ids)
        })
    }

    fn insert_figure(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn Figure>,
        constraint: Option<Box<dyn LayoutConstraint>>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(parent)?;
            if runtime.tree.is_layered_pane(parent) {
                return Err(RuntimeMutationError::LayeredParent(parent));
            }
            let child = runtime.tree.prepare_child(figure, parent, false, 0, None)?;
            let resources =
                runtime.admit_figure_resources(child.figure.as_ref(), &child.capabilities, &[])?;
            let id =
                runtime
                    .tree
                    .insert_prepared_child(child, parent, Some(index), |tree, child| {
                        if let Some(constraint) = constraint.as_deref()
                            && let Some(manager) = tree
                                .layout_manager(parent)
                                .map_err(crate::LayoutError::from)?
                        {
                            manager.validate_constraint(parent, child, constraint)?;
                        }
                        Ok::<_, RuntimeMutationError>(constraint)
                    })?;
            runtime
                .tree
                .invalidate_child_insertion(&mut runtime.updates, parent, id);
            runtime.complete_figure_insertion(parent, id, resources);
            Ok(id)
        })
    }

    /// Adds a Viewport Figure and returns its transactional handle.
    pub(crate) fn add_viewport(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ViewportHandle, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(parent)?;
            if runtime.tree.is_layered_pane(parent) {
                return Err(RuntimeMutationError::LayeredParent(parent));
            }
            let viewport = runtime.tree.add_viewport_to(parent, bounds)?;
            runtime.tree.mark_invalid(&mut runtime.updates, parent);
            runtime
                .tree
                .mark_invalid(&mut runtime.updates, viewport.figure_id());
            runtime
                .tree
                .repaint(&mut runtime.updates, viewport.figure_id(), None);
            runtime
                .tree
                .complete_attachment(viewport.figure_id(), parent);
            Ok(viewport)
        })
    }

    fn add_figure_checked(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        self.add_figure_inner(parent, figure)
    }

    fn add_figure_inner(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        let child = self.tree.prepare_child(figure, parent, false, 0, None)?;
        let resources =
            self.admit_figure_resources(child.figure.as_ref(), &child.capabilities, &[])?;
        let id = self
            .tree
            .attach_prepared_child(&mut self.updates, parent, child)?;
        self.complete_figure_insertion(parent, id, resources);
        Ok(id)
    }

    fn complete_figure_insertion(
        &mut self,
        parent: FigureId,
        id: FigureId,
        resources: Vec<crate::render::ResourceId>,
    ) {
        self.publish_figure_resources(id, resources);
        self.register_layered_pane(id);
        self.tree.complete_attachment(id, parent);
    }

    fn admit_figure_resources(
        &self,
        figure: &dyn Figure,
        capabilities: &crate::figure::capability::FigureCapabilitySet,
        explicit: &[crate::render::ResourceId],
    ) -> Result<Vec<crate::render::ResourceId>, RuntimeMutationError> {
        Self::admit_resources(&self.resources, figure, capabilities, explicit)
    }

    fn admit_resources(
        registry: &ResourceRegistry,
        figure: &dyn Figure,
        capabilities: &crate::figure::capability::FigureCapabilitySet,
        explicit: &[crate::render::ResourceId],
    ) -> Result<Vec<crate::render::ResourceId>, RuntimeMutationError> {
        let mut resources = explicit.to_vec();
        if let Some(capability) = capabilities
            .get(crate::figure::RESOURCE_DEPENDENCIES)
            .expect("typed resource descriptor")
        {
            let provider = capability
                .resolve(figure)
                .ok_or(RuntimeMutationError::Rejected)?;
            resources.extend(provider.resource_dependencies());
        }
        resources.sort_unstable();
        resources.dedup();
        for resource in &resources {
            registry.status(*resource)?;
        }
        Ok(resources)
    }

    fn publish_figure_resources(
        &mut self,
        id: FigureId,
        resources: Vec<crate::render::ResourceId>,
    ) {
        for resource in resources {
            self.resources
                .add_dependency(resource, id)
                .expect("resource dependency passed admission");
            self.record_resource_input(resource, id);
        }
    }

    pub(crate) fn add_subtree(
        &mut self,
        parent: FigureId,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_container_figure(parent)?;
            let owner = subtree.owner;
            let (inserted, _) = runtime.publish_subtree(parent, subtree, None)?;
            runtime.complete_subtree_attachment(&inserted);
            Ok(crate::tree::SubtreeReceipt::new(
                owner,
                inserted.into_iter().map(|(id, _)| id).collect(),
            ))
        })
    }

    fn publish_subtree(
        &mut self,
        parent: FigureId,
        subtree: crate::tree::FigureSubtree,
        replacing: Option<FigureId>,
    ) -> Result<SubtreePublication, RuntimeMutationError> {
        let runtime = self;
        let owner = subtree.owner;
        let bindings = subtree.connections;
        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        let subsequent_invalidations =
            u64::from(replacing.is_some()) + u64::from(parent == runtime.tree.synthetic_root());
        let mut dependencies = Vec::with_capacity(subtree.nodes.len());
        let damage = replacing
            .map(|root| {
                let ids = runtime
                    .tree
                    .disposal_ids(root)
                    .expect("validated replacement root");
                runtime.updates.freeze_removed_damage(&runtime.tree, &ids)
            })
            .unwrap_or_default();
        let (inserted, retired) = runtime.tree.insert_subtree(
            parent,
            subtree.nodes,
            replacing,
            |figure, capabilities, explicit| {
                let resources =
                    Self::admit_resources(&runtime.resources, figure, capabilities, explicit)?;
                dependencies.push(resources);
                Ok::<_, RuntimeMutationError>(())
            },
            |index, source| RuntimeMutationError::FigureBatch {
                index,
                source: Box::new(source),
            },
            |tree, ids| {
                let indices: std::collections::HashMap<_, _> = ids
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, id)| (id, index))
                    .collect();
                let nodes = crate::tree::SubtreeBindingNodes(crate::tree::SubtreeReceipt::new(
                    owner,
                    ids.to_vec(),
                ));
                for (node, binding) in bindings {
                    let figure = nodes.figure(node).expect("validated construction identity");
                    let index = indices[&figure];
                    let result = (|| {
                        if !tree
                            .is_connection_figure(figure)
                            .map_err(ConnectionRuntimeError::from)?
                        {
                            return Err(ConnectionRuntimeError::NotConnectionFigure(figure).into());
                        }
                        let payload = binding.prepare(&nodes)?;
                        for anchor in [&payload.source, &payload.target].into_iter().flatten() {
                            if let Some(owner) = anchor.owner()
                                && !indices.contains_key(&owner)
                            {
                                return Err(ConnectionRuntimeError::UnknownFigure(owner).into());
                            }
                        }
                        if let RouterBinding::Inherited { layer } = payload.router
                            && !indices.contains_key(&layer)
                        {
                            return Err(ConnectionRuntimeError::UnknownFigure(layer).into());
                        }
                        let prepared = crate::connection::PreparedSubtreeConnection {
                            connection: ConnectionId::from_figure(figure),
                            payload,
                        };
                        runtime.connections.prepare_subtree_connection(&prepared)?;
                        prepared_bindings.push(prepared);
                        Ok::<_, RuntimeMutationError>(())
                    })();
                    result.map_err(|source| RuntimeMutationError::FigureBatch {
                        index,
                        source: Box::new(source),
                    })?;
                }
                runtime.connections.validate_subtree_revision_budget(
                    subsequent_invalidations + u64::from(!prepared_bindings.is_empty()),
                )?;
                Ok(())
            },
        )?;
        runtime
            .connections
            .publish_subtree_connections(prepared_bindings);
        for ((id, parent), resources) in inserted.iter().zip(dependencies) {
            runtime.publish_figure_resources(*id, resources);
            runtime
                .tree
                .invalidate_child_insertion(&mut runtime.updates, *parent, *id);
        }
        for (id, _) in &inserted {
            runtime.register_layered_pane(*id);
        }
        Ok((inserted, retired.map(|retired| (parent, retired, damage))))
    }

    fn complete_subtree_attachment(&mut self, inserted: &[(FigureId, FigureId)]) {
        for (id, parent) in inserted {
            self.tree.complete_attachment(*id, *parent);
        }
    }

    fn add_layered_pane_inner(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<FigureId, LayerError> {
        let pane_id = self.tree.try_add_child_to(
            parent,
            Box::new(
                LayeredPane::new(bounds.x, bounds.y, bounds.width, bounds.height)
                    .expect("valid detached figure input"),
            ),
        )?;
        self.register_layered_pane(pane_id);
        self.tree.mark_invalid(&mut self.updates, parent);
        self.tree.mark_invalid(&mut self.updates, pane_id);
        self.tree.repaint(&mut self.updates, pane_id, None);
        self.tree.complete_attachment(pane_id, parent);
        Ok(pane_id)
    }

    fn validate_layered_pane_handle(&mut self, pane_id: FigureId) -> Result<(), LayerError> {
        if !self.tree.is_layered_pane(pane_id) || self.tree.is_attached(pane_id) != Ok(true) {
            return Err(LayerError::UnknownPane);
        }
        self.layered_panes.entry(pane_id).or_default();
        Ok(())
    }

    pub(crate) fn add_layer(
        &mut self,
        pane_id: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.add_layer_inner(pane_id, figure, key, placement))
    }

    fn add_layer_inner(
        &mut self,
        pane_id: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        if state.contains_key(&key) {
            return Err(LayerError::DuplicateKey);
        }
        let target_index = self.resolve_layer_index(pane_id, state, &placement, None)?;
        let prepared = self
            .tree
            .prepare_child(figure, pane_id, true, 0, None)
            .map_err(|error| {
                if error == crate::GraphMutationError::LayerChildRequired {
                    LayerError::NotLayer
                } else {
                    LayerError::Graph(error)
                }
            })?;
        let resources = self
            .admit_figure_resources(prepared.figure.as_ref(), &prepared.capabilities, &[])
            .map_err(|error| match error {
                RuntimeMutationError::Resource(error) => LayerError::Resource(error),
                _ => LayerError::Rejected,
            })?;
        let child = self
            .tree
            .attach_prepared_child(&mut self.updates, pane_id, prepared)?;
        let last_index = self
            .tree
            .child_order(pane_id)
            .map_or(0, |order| order.len().saturating_sub(1));
        if target_index != last_index {
            let _ = self.tree.move_child_to_index(pane_id, child, target_index);
        }
        self.layered_panes
            .get_mut(&pane_id)
            .expect("validated layered pane state must exist")
            .insert(key, child);
        self.complete_figure_insertion(pane_id, child, resources);
        Ok(child)
    }

    pub(crate) fn remove_layer(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
    ) -> Result<FigureId, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.remove_layer_inner(pane_id, key))
    }

    fn remove_layer_inner(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
    ) -> Result<FigureId, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let child = self.layer(pane_id, key)?;
        if self.tree.parent_id(child) != Ok(Some(pane_id)) {
            return Err(LayerError::InconsistentState);
        }
        self.dispose_subtree(child)
            .map_err(|_| LayerError::InconsistentState)?;
        Ok(child)
    }

    pub(crate) fn move_layer(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.move_layer_inner(pane_id, key, placement))
    }

    fn move_layer_inner(
        &mut self,
        pane_id: FigureId,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.ensure_layered_pane(pane_id)?;
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        let child = state.layer(key).ok_or(LayerError::UnknownKey)?;
        let target_index = self.resolve_layer_index(pane_id, state, &placement, Some(child))?;
        let Some(old_index) = self
            .tree
            .child_z_index(pane_id, child)
            .map_err(|_| LayerError::InconsistentState)?
        else {
            return Err(LayerError::InconsistentState);
        };
        if old_index == target_index {
            return Ok(false);
        }
        if !self.tree.move_child_to_index(pane_id, child, target_index) {
            return Err(LayerError::InconsistentState);
        }
        self.tree.repaint(&mut self.updates, pane_id, None);
        Ok(true)
    }

    pub(crate) fn reparent_layer(
        &mut self,
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        if self.faulted {
            return Err(LayerError::Faulted);
        }
        self.guarded(|runtime| runtime.reparent_layer_inner(child, new_pane, key, placement))
    }

    fn reparent_layer_inner(
        &mut self,
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.ensure_layered_pane(new_pane)?;
        if !self.tree.is_layer_figure(child) {
            return Err(LayerError::NotLayer);
        }
        let target_state = self
            .layered_panes
            .get(&new_pane)
            .ok_or(LayerError::InconsistentState)?;
        if target_state.contains_key(&key) {
            return Err(LayerError::DuplicateKey);
        }
        if target_state.contains_child(child) {
            return Err(LayerError::AlreadyLayerMember);
        }
        let target_index = self.resolve_layer_index(new_pane, target_state, &placement, None)?;
        let old_parent = self
            .tree
            .parent_id(child)
            .map_err(|_| LayerError::InconsistentState)?
            .ok_or(LayerError::InconsistentState)?;
        if self.tree.is_layered_pane(old_parent)
            && self
                .layered_panes
                .get(&old_parent)
                .is_none_or(|state| !state.contains_child(child))
        {
            return Err(LayerError::InconsistentState);
        }
        if !self
            .tree
            .reparent_layer_child(&mut self.updates, child, new_pane)?
        {
            return Ok(false);
        }
        if let Some(state) = self.layered_panes.get_mut(&old_parent) {
            state.remove_child(child);
        }
        self.layered_panes
            .get_mut(&new_pane)
            .expect("validated layered pane state must exist")
            .insert(key, child);
        let last_index = self
            .tree
            .child_order(new_pane)
            .map_or(0, |order| order.len().saturating_sub(1));
        if target_index != last_index {
            let _ = self.tree.move_child_to_index(new_pane, child, target_index);
        }
        self.invalidate_connection_figure_change(child, true);
        self.tree.complete_detachment(child, old_parent);
        self.tree.complete_attachment(child, new_pane);
        Ok(true)
    }

    pub(crate) fn layer(&self, pane_id: FigureId, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?
            .layer(key)
            .ok_or(LayerError::UnknownKey)
    }

    pub(crate) fn layer_key(
        &self,
        pane_id: FigureId,
        child: FigureId,
    ) -> Result<LayerKey, LayerError> {
        self.layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?
            .key(child)
            .cloned()
            .ok_or(LayerError::UnknownKey)
    }

    pub(crate) fn layer_ids(&self, pane_id: FigureId) -> Result<Vec<FigureId>, LayerError> {
        let state = self
            .layered_panes
            .get(&pane_id)
            .ok_or(LayerError::InconsistentState)?;
        let order = self
            .tree
            .child_order(pane_id)
            .map_err(|_| LayerError::UnknownPane)?;
        if order.len() != state.len() || order.iter().any(|child| !state.contains_child(*child)) {
            return Err(LayerError::InconsistentState);
        }
        Ok(order)
    }

    pub(crate) fn remove_figure(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.remove_figure_checked(parent, child))
    }

    fn remove_figure_checked(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        if self.tree.parent_id(child) != Ok(Some(parent)) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.dispose_subtree(child)?;
        Ok(true)
    }

    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    pub fn dispose_subtree(&mut self, root: FigureId) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(root)?;
        let ids = self
            .tree
            .disposal_ids(root)
            .map_err(RuntimeMutationError::Graph)?;
        let parent = self
            .tree
            .parent_id(root)
            .expect("validated attached root")
            .ok_or(RuntimeMutationError::Graph(
                crate::GraphMutationError::InvalidParentRelation,
            ))?;
        self.connections
            .validate_disposal()
            .map_err(|_| RuntimeMutationError::Rejected)?;
        self.guarded(|runtime| {
            let damage = runtime.updates.freeze_removed_damage(&runtime.tree, &ids);
            let retired = runtime.tree.extract_subtree(&ids, &mut runtime.updates)?;
            runtime.complete_subtree_retirement(parent, retired, damage);
            Ok(())
        })
    }

    fn complete_subtree_retirement(
        &mut self,
        parent: FigureId,
        retired: crate::graph::RetiredSubtree,
        damage: Vec<Rectangle>,
    ) {
        let removed: HashSet<_> = retired.nodes.iter().map(|node| node.id).collect();
        let focused = self
            .interaction
            .focus_owner()
            .filter(|id| removed.contains(id));
        self.animations.retire_figures(&removed);
        self.interaction.forget_figures(&removed);
        self.updates.forget_figures(&removed);
        let listeners = self.updates.retire_listeners(&removed);
        let connections = self.connections.retire_figures(&removed);
        self.resources
            .retain_dependencies(|id| !removed.contains(&id));
        self.layered_panes.retain(|id, state| {
            state.retain_children(|child| !removed.contains(&child));
            !removed.contains(id)
        });
        self.anchor_geometries
            .retain(|(id, _), _| !removed.contains(id));
        for rect in damage {
            self.updates.add_frozen_surface_region(rect);
        }
        self.tree.mark_invalid(&mut self.updates, parent);
        self.scenes.retire_figures(&removed);
        if let Some(node) = retired.nodes.iter().find(|node| Some(node.id) == focused) {
            crate::EventContext::retired_focus_lost(
                node,
                &mut self.mutations,
                &mut self.updates,
                &mut self.tree,
            );
        }
        retired.complete();
        drop(connections);
        drop(listeners);
    }

    fn guarded<T>(&mut self, operation: impl FnOnce(&mut Self) -> T) -> T {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self))) {
            Ok(value) => value,
            Err(payload) => {
                self.animations.clear_presentation();
                self.faulted = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn guarded_runtime_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, RuntimeMutationError>,
    ) -> Result<T, RuntimeMutationError> {
        if self.faulted {
            return Err(RuntimeMutationError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_widget_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, WidgetError>,
    ) -> Result<T, WidgetError> {
        if self.faulted {
            return Err(WidgetError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_connection_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ConnectionRuntimeError>,
    ) -> Result<T, ConnectionRuntimeError> {
        if self.faulted {
            return Err(ConnectionRuntimeError::Faulted);
        }
        self.guarded(operation)
    }

    fn guarded_resource_mutation<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        self.guarded(operation)
    }

    pub(crate) fn reparent(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.reparent_checked(child, new_parent))
    }

    fn reparent_checked(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        self.validate_attached_figure(new_parent)?;
        let old_parent = self
            .tree
            .parent_id(child)
            .expect("validated attached child");
        if old_parent.is_some_and(|parent| self.tree.is_layered_pane(parent)) {
            return Err(RuntimeMutationError::LayeredParent(
                old_parent.expect("checked parent"),
            ));
        }
        if self.tree.is_layered_pane(new_parent) {
            return Err(RuntimeMutationError::LayeredParent(new_parent));
        }
        self.reparent_inner(child, new_parent)
    }

    fn reparent_inner(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        let old_parent = self
            .tree
            .parent_id(child)
            .expect("validated attached child")
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        let prepared_constraint = if old_parent == new_parent {
            None
        } else {
            let scene = FigureTreeRouteSceneRead::new(&self.tree, &self.anchor_geometries);
            self.connections.prepare_reparent_constraint(
                ConnectionId::from_figure(child),
                CoordinateSpace::ChildContent(old_parent),
                CoordinateSpace::ChildContent(new_parent),
                &scene,
            )?
        };
        let changed = self.tree.reparent(&mut self.updates, child, new_parent)?;
        if changed {
            if let Some(constraint) = prepared_constraint {
                self.connections
                    .commit_reparent_constraint(ConnectionId::from_figure(child), constraint);
            }
            self.invalidate_connection_figure_change(child, true);
            self.retain_interactive_figures();
            self.tree.complete_detachment(child, old_parent);
            self.tree.complete_attachment(child, new_parent);
        }
        Ok(changed)
    }

    pub(crate) fn set_layout_manager(
        &mut self,
        container: FigureId,
        manager: Box<dyn LayoutManager>,
    ) -> Result<bool, RuntimeMutationError> {
        self.replace_layout_manager(container, Some(manager))
    }

    pub(crate) fn clear_layout_manager(
        &mut self,
        container: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.replace_layout_manager(container, None)
    }

    fn replace_layout_manager(
        &mut self,
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.replace_layout_manager_inner(container, manager)
        })
    }

    fn replace_layout_manager_inner(
        &mut self,
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(container)?;
        if let Some(manager) = manager.as_deref() {
            self.tree
                .validate_layout_manager_constraints(container, manager)?;
        }
        if !self.tree.replace_layout_manager(container, manager) {
            return Ok(false);
        }
        self.tree.mark_invalid(&mut self.updates, container);
        Ok(true)
    }

    pub(crate) fn set_layout_constraint<C>(
        &mut self,
        child: FigureId,
        constraint: C,
    ) -> Result<bool, RuntimeMutationError>
    where
        C: LayoutConstraint,
    {
        self.set_boxed_layout_constraint(child, Box::new(constraint))
    }

    fn set_boxed_layout_constraint(
        &mut self,
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.set_boxed_layout_constraint_inner(child, constraint)
        })
    }

    fn set_boxed_layout_constraint_inner(
        &mut self,
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        let parent = self
            .tree
            .parent_id(child)
            .expect("validated attached child")
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        if let Some(manager) = self.tree.layout_manager(parent).expect("validated parent") {
            manager.validate_constraint(parent, child, constraint.as_ref())?;
        }
        if !self.tree.set_boxed_constraint(child, constraint) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.tree.mark_invalid(&mut self.updates, parent);
        Ok(true)
    }

    pub(crate) fn remove_layout_constraint(
        &mut self,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.remove_layout_constraint_inner(child))
    }

    fn remove_layout_constraint_inner(
        &mut self,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        let parent = self
            .tree
            .parent_id(child)
            .expect("validated attached child")
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        if !self.tree.remove_constraint(child) {
            return Ok(false);
        }
        self.tree.mark_invalid(&mut self.updates, parent);
        Ok(true)
    }

    pub(crate) fn set_preferred_size(
        &mut self,
        figure: FigureId,
        size: Dimension,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Preferred, Some(size))
    }

    pub(crate) fn clear_preferred_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Preferred, None)
    }

    pub(crate) fn set_minimum_size(
        &mut self,
        figure: FigureId,
        size: Dimension,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Minimum, Some(size))
    }

    pub(crate) fn clear_minimum_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Minimum, None)
    }

    pub(crate) fn set_maximum_size(
        &mut self,
        figure: FigureId,
        size: Dimension,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Maximum, Some(size))
    }

    pub(crate) fn clear_maximum_size(
        &mut self,
        figure: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_size_override(figure, SizeOverrideKind::Maximum, None)
    }

    fn set_size_override(
        &mut self,
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<Dimension>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.set_size_override_inner(figure, kind, size))
    }

    fn set_size_override_inner(
        &mut self,
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<Dimension>,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        if let Some(size) = size
            && (!size.width.is_finite()
                || !size.height.is_finite()
                || size.width < 0.0
                || size.height < 0.0)
        {
            return Err(RuntimeMutationError::InvalidSize { figure, size });
        }
        let changed = match kind {
            SizeOverrideKind::Preferred => self.tree.set_preferred_size(figure, size),
            SizeOverrideKind::Minimum => self.tree.set_minimum_size(figure, size),
            SizeOverrideKind::Maximum => self.tree.set_maximum_size(figure, size),
        };
        if changed {
            self.tree.mark_invalid(&mut self.updates, figure);
        }
        Ok(changed)
    }

    pub(crate) fn move_child_to_index(
        &mut self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.move_child_to_index_inner(parent, child, index)
        })
    }

    fn move_child_to_index_inner(
        &mut self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_child_order_mutation(parent, child, index)?;
        if self.tree.child_at_index(parent, index) == Ok(Some(child)) {
            return Ok(false);
        }
        if !self.tree.move_child_to_index(parent, child, index) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        self.tree.repaint(&mut self.updates, parent, None);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        Ok(true)
    }

    /// Atomically replaces the complete direct-child order of a non-layered parent.
    pub(crate) fn set_child_order(
        &mut self,
        parent: FigureId,
        order: &[FigureId],
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| runtime.set_child_order_inner(parent, order))
    }

    fn set_child_order_inner(
        &mut self,
        parent: FigureId,
        order: &[FigureId],
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        let current = self
            .tree
            .child_order(parent)
            .expect("attached Figure must have a child list");
        if current.as_slice() == order {
            return Ok(false);
        }
        if current.len() != order.len() {
            return Err(RuntimeMutationError::InvalidChildOrder { parent });
        }
        let mut seen = HashSet::with_capacity(order.len());
        for child in order {
            self.validate_attached_figure(*child)?;
            if self.tree.parent_id(*child) != Ok(Some(parent)) || !seen.insert(*child) {
                return Err(RuntimeMutationError::InvalidChildOrder { parent });
            }
        }
        if !self.tree.set_child_order(parent, order) {
            return Err(RuntimeMutationError::InvalidChildOrder { parent });
        }
        self.tree.repaint(&mut self.updates, parent, None);
        if let Err(error) = self.connections.invalidate_all() {
            self.connection_error = Some(error);
        }
        Ok(true)
    }

    pub(crate) fn bring_child_to_front(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        let child_count = self
            .tree
            .child_order(parent)
            .expect("validated attached parent")
            .len();
        let index = child_count
            .checked_sub(1)
            .ok_or(RuntimeMutationError::InvalidParentRelation { parent, child })?;
        self.move_child_to_index(parent, child, index)
    }

    pub(crate) fn send_child_to_back(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, RuntimeMutationError> {
        self.move_child_to_index(parent, child, 0)
    }

    pub(crate) fn set_child_clipping_strategy(
        &mut self,
        figure: FigureId,
        strategy: ChildClippingStrategy,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        if !self.tree.set_child_clipping_strategy(figure, strategy) {
            return Ok(false);
        }
        self.full_redraw_pending = true;
        Ok(true)
    }

    pub fn take_deferred_mutation_errors(&mut self) -> Vec<RuntimeMutationError> {
        self.deferred_mutation_errors.drain(..).collect()
    }

    fn validate_attached_figure(&self, figure: FigureId) -> Result<(), RuntimeMutationError> {
        if self.faulted {
            return Err(RuntimeMutationError::Faulted);
        }
        if figure.namespace() != self.tree.namespace() {
            return Err(RuntimeMutationError::ForeignRuntime(figure));
        }
        if figure == self.tree.synthetic_root() {
            return Err(RuntimeMutationError::SyntheticRootOperation(figure));
        }
        if !self.tree.contains(figure) {
            return Err(RuntimeMutationError::UnknownOrDisposedFigure(figure));
        }
        if !self
            .tree
            .is_attached(figure)
            .expect("identity checked above")
        {
            return Err(RuntimeMutationError::DetachedFigure(figure));
        }
        Ok(())
    }

    fn validate_container_figure(&self, figure: FigureId) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(figure)?;
        if self
            .tree
            .capability(figure, crate::figure::CONTAINER)
            .map_err(|_| RuntimeMutationError::WrongCapability {
                figure,
                capability: crate::figure::CONTAINER.name(),
            })?
            .is_none()
        {
            return Err(RuntimeMutationError::WrongCapability {
                figure,
                capability: crate::figure::CONTAINER.name(),
            });
        }
        Ok(())
    }

    fn validate_child_order_mutation(
        &self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(parent)?;
        self.validate_attached_figure(child)?;
        if self.tree.is_layered_pane(parent) {
            return Err(RuntimeMutationError::LayeredParent(parent));
        }
        if self.tree.parent_id(child) != Ok(Some(parent)) {
            return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
        }
        let child_count = self
            .tree
            .node(parent)
            .expect("attached Figure must have a node")
            .children_count();
        if index >= child_count {
            return Err(RuntimeMutationError::InvalidChildIndex {
                parent,
                index,
                child_count,
            });
        }
        Ok(())
    }

    pub(crate) fn set_bounds(
        &mut self,
        id: FigureId,
        bounds: crate::geometry::Rectangle,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            if !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || !bounds.width.is_finite()
                || !bounds.height.is_finite()
                || bounds.width < 0.0
                || bounds.height < 0.0
            {
                return Err(RuntimeMutationError::InvalidBounds { figure: id, bounds });
            }
            Ok(runtime.set_bounds_inner(id, bounds))
        })
    }

    fn set_bounds_inner(&mut self, id: FigureId, bounds: crate::geometry::Rectangle) -> bool {
        let is_contents = self.tree.contents() == Some(id);
        let changed = self.tree.set_bounds_with_update(
            &mut self.updates,
            id,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
        if changed && is_contents {
            // The synthetic tree root has no drawable background to repair
            // pixels exposed by a moved or resized contents node.
            self.full_redraw_pending = true;
        }
        if changed {
            self.invalidate_connection_figure_change(id, false);
        }
        changed
    }

    pub(crate) fn set_visible(
        &mut self,
        id: FigureId,
        visible: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_visible_inner(id, visible))
        })
    }

    fn set_visible_inner(&mut self, id: FigureId, visible: bool) -> bool {
        let changed = self
            .tree
            .set_visible_with_update(&mut self.updates, id, visible);
        if changed {
            self.animations
                .access_mut(
                    &self.tree,
                    &self.resources,
                    &mut self.updates,
                    &mut self.full_redraw_pending,
                    &mut self.faulted,
                )
                .reconcile_visibility();
        }
        self.retain_interactive_figures();
        changed
    }

    pub(crate) fn set_enabled(
        &mut self,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_enabled_inner(id, enabled))
        })
    }

    fn set_enabled_inner(&mut self, id: FigureId, enabled: bool) -> bool {
        let changed = self
            .tree
            .set_enabled_with_update(&mut self.updates, id, enabled);
        self.retain_interactive_figures();
        self.sync_clickable_visuals();
        changed
    }

    pub(crate) fn set_focusable(
        &mut self,
        id: FigureId,
        focusable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_focusable_inner(id, focusable))
        })
    }

    fn set_focusable_inner(&mut self, id: FigureId, focusable: bool) -> bool {
        let changed = self.tree.set_focusable(id, focusable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub(crate) fn set_focus_traversable(
        &mut self,
        id: FigureId,
        traversable: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_focus_traversable_inner(id, traversable))
        })
    }

    fn set_focus_traversable_inner(&mut self, id: FigureId, traversable: bool) -> bool {
        let changed = self.tree.set_focus_traversable(id, traversable);
        if changed {
            self.retain_interactive_figures();
        }
        changed
    }

    pub(crate) fn set_figure_style(
        &mut self,
        id: FigureId,
        style: FigureStyle,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_figure_style_inner(id, style))
        })
    }

    fn set_figure_style_inner(&mut self, id: FigureId, style: FigureStyle) -> bool {
        let changed = self
            .tree
            .set_figure_style_with_update(&mut self.updates, id, style);
        if changed {
            self.sync_tooltip();
        }
        changed
    }

    pub(crate) fn label_text_layout(
        &self,
        id: FigureId,
    ) -> Result<&TextLayout, ShapeMutationError> {
        self.label_figure(id)?
            .text_layout()
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub(crate) fn text_flow_layout(&self, id: FigureId) -> Result<&TextLayout, ShapeMutationError> {
        if !self.tree.contains(id) {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .prepared_presentation(id)
            .map_err(|_| ShapeMutationError::UnknownFigure(id))?
            .and_then(|presentation| presentation.data::<crate::TextFlowFigure>())
            .and_then(crate::TextFlowFigure::text_layout)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub(crate) fn text_flow_hit_test(
        &self,
        id: FigureId,
        surface_point: Point,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        let flow = self.text_flow_for_query(id)?;
        let transform = self
            .tree
            .surface_to_local_transform(id)
            .map_err(TextFlowQueryError::Coordinate)?
            .ok_or(TextFlowQueryError::NonInvertibleTransform(id))?;
        flow.hit_test_text(transform.transform_point(surface_point))
            .map_err(Into::into)
    }

    pub(crate) fn text_flow_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        let mut geometry = self.text_flow_local_caret_geometry(id, position)?;
        let transform = self
            .tree
            .local_to_surface_transform(id)
            .map_err(TextFlowQueryError::Coordinate)?;
        let mut bounds = geometry.bounds();
        bounds.transform(transform);
        geometry = CaretGeometry::new(bounds, geometry.line_index());
        Ok(geometry)
    }

    /// Returns caret geometry in the TextFlow Figure's node-local coordinates.
    pub(crate) fn text_flow_local_caret_geometry(
        &self,
        id: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        self.text_flow_for_query(id)?
            .caret_geometry(position)
            .map_err(Into::into)
    }

    pub(crate) fn text_flow_selection_geometry(
        &self,
        id: FigureId,
        range: FlowTextRange,
    ) -> Result<Vec<SelectionQuad>, TextFlowQueryError> {
        let flow = self.text_flow_for_query(id)?;
        let transform = self
            .tree
            .local_to_surface_transform(id)
            .map_err(TextFlowQueryError::Coordinate)?;
        flow.selection_geometry(range)?
            .into_iter()
            .map(|quad| {
                let mut bounds = quad.bounds();
                bounds.transform(transform);
                Ok(SelectionQuad::new(bounds, quad.line_index()))
            })
            .collect()
    }

    pub(crate) fn text_flow_move_position(
        &self,
        id: FigureId,
        position: FlowTextPosition,
        movement: TextMovement,
    ) -> Result<FlowTextPosition, TextFlowQueryError> {
        self.text_flow_for_query(id)?
            .move_text_position(position, movement)
            .map_err(Into::into)
    }

    fn text_flow_for_query(
        &self,
        id: FigureId,
    ) -> Result<&crate::TextFlowFigure, TextFlowQueryError> {
        if !self.tree.contains(id) {
            return Err(TextFlowQueryError::UnknownFigure(id));
        }
        self.tree
            .prepared_presentation(id)
            .map_err(|_| TextFlowQueryError::UnknownFigure(id))?
            .and_then(|presentation| presentation.data::<crate::TextFlowFigure>())
            .ok_or(TextFlowQueryError::WrongCapability(id))
    }

    pub(crate) fn label_text(&self, id: FigureId) -> Result<&str, ShapeMutationError> {
        Ok(self.label_figure(id)?.text())
    }

    pub(crate) fn title_bar_text_layout(
        &self,
        id: FigureId,
    ) -> Result<&TextLayout, ShapeMutationError> {
        if !self.tree.contains(id) {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .border_snapshot(id)
            .map(BorderSnapshot::text_layout)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    fn label_figure(&self, id: FigureId) -> Result<&LabelFigure, ShapeMutationError> {
        if !self.tree.contains(id) {
            return Err(ShapeMutationError::UnknownFigure(id));
        }
        self.tree
            .label(id)
            .ok_or(ShapeMutationError::WrongCapability(id))
    }

    pub(crate) fn set_opaque(
        &mut self,
        id: FigureId,
        opaque: bool,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(id)?;
            Ok(runtime.set_opaque_inner(id, opaque))
        })
    }

    fn set_opaque_inner(&mut self, id: FigureId, opaque: bool) -> bool {
        if !self.tree.set_opaque(id, opaque) {
            return false;
        }
        self.tree.repaint(&mut self.updates, id, None);
        true
    }

    pub(crate) fn translate(
        &mut self,
        id: FigureId,
        dx: f64,
        dy: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.validate_attached_figure(id)?;
        let bounds = self
            .tree
            .figure_bounds(id)
            .map_err(|_| RuntimeMutationError::UnknownOrDisposedFigure(id))?;
        self.set_bounds(
            id,
            crate::geometry::Rectangle::new(
                bounds.x + dx,
                bounds.y + dy,
                bounds.width,
                bounds.height,
            ),
        )
    }

    pub(crate) fn add_update_listener(&mut self, listener: Box<dyn UpdateListener>) -> ListenerId {
        self.add_update_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_update_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn UpdateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_figure_listener(&mut self, listener: Box<dyn FigureListener>) -> ListenerId {
        self.add_figure_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_figure_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn FigureListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_figure_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_coordinate_listener(
        &mut self,
        listener: Box<dyn CoordinateListener>,
    ) -> ListenerId {
        self.add_coordinate_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_coordinate_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn CoordinateListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_coordinate_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_ancestor_listener(
        &mut self,
        listener: Box<dyn AncestorListener>,
    ) -> ListenerId {
        self.add_ancestor_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_ancestor_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn AncestorListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_ancestor_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_property_listener(
        &mut self,
        listener: Box<dyn PropertyChangeListener>,
    ) -> ListenerId {
        self.add_property_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_property_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn PropertyChangeListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_property_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_action_listener(&mut self, listener: Box<dyn ActionListener>) -> ListenerId {
        self.add_action_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_action_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ActionListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_action_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_layout_listener(&mut self, listener: Box<dyn LayoutListener>) -> ListenerId {
        self.add_layout_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_layout_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn LayoutListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_layout_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn add_observation_listener(
        &mut self,
        listener: Box<dyn ObservationListener>,
    ) -> ListenerId {
        self.add_observation_listener_scoped(ListenerScope::Runtime, listener)
            .expect("Runtime listener scope is always valid")
    }

    pub(crate) fn add_observation_listener_scoped(
        &mut self,
        scope: ListenerScope,
        listener: Box<dyn ObservationListener>,
    ) -> Result<ListenerId, RuntimeMutationError> {
        self.validate_listener_scope(scope)?;
        let id = self.updates.add_observation_listener(listener);
        let scope_was_set = self.updates.set_listener_scope(id, scope);
        debug_assert!(scope_was_set);
        Ok(id)
    }

    pub(crate) fn remove_listener(&mut self, id: ListenerId) -> bool {
        if self.faulted {
            return false;
        }
        self.updates.remove_listener(id)
    }

    fn validate_listener_scope(&self, scope: ListenerScope) -> Result<(), RuntimeMutationError> {
        match scope {
            ListenerScope::Runtime => {
                if self.faulted {
                    Err(RuntimeMutationError::Faulted)
                } else {
                    Ok(())
                }
            }
            ListenerScope::Figure(owner) => self.validate_attached_figure(owner),
        }
    }

    pub(crate) fn clickable_snapshot(
        &self,
        id: FigureId,
    ) -> Result<ClickableSnapshot, WidgetError> {
        if !self.tree.contains(id) {
            return Err(WidgetError::UnknownFigure(id));
        }
        self.tree
            .clickable_snapshot(id)
            .ok_or(WidgetError::WrongCapability(id))
    }

    pub(crate) fn do_click(&mut self, id: FigureId) -> Result<bool, WidgetError> {
        self.guarded_widget_mutation(|runtime| runtime.do_click_inner(id))
    }

    fn do_click_inner(&mut self, id: FigureId) -> Result<bool, WidgetError> {
        if !self.tree.contains(id) {
            return Err(WidgetError::UnknownFigure(id));
        }
        if self.tree.clickable_snapshot(id).is_none() {
            return Err(WidgetError::WrongCapability(id));
        }
        let changed = self.tree.activate_clickable(&mut self.updates, id);
        self.sync_clickable_visuals();
        Ok(changed)
    }

    pub(crate) fn has_pending_update(&self) -> bool {
        self.full_redraw_pending
            || (self.session_sync_pending && self.in_flight.is_none())
            || self.updates.is_update_queued()
            || self.resources.has_pending_delta()
            || !self.mutations.is_empty()
            || !self.connections.dirty_connections().is_empty()
            || !self.tree.notification_effects().is_empty()
            || !self.updates.notification_effects().is_empty()
            || self.derivation_epoch != self.stable_epoch
    }

    pub(crate) fn stable_query(&self) -> Result<StableSceneQuery<'_>, StableQueryError> {
        if self.faulted {
            return Err(StableQueryError::Faulted);
        }
        if self.has_pending_update() {
            return Err(StableQueryError::NotStable {
                latest_stable_epoch: self.stable_epoch,
            });
        }
        Ok(StableSceneQuery::new(self.stable_epoch, &self.tree))
    }

    pub(crate) fn last_validation_error(&self) -> Option<&ValidationError> {
        self.updates.last_validation_error()
    }

    pub fn take_validation_error(&mut self) -> Option<ValidationError> {
        self.updates.take_validation_error()
    }

    pub(crate) fn request_full_redraw(&mut self) {
        if !self.faulted {
            self.full_redraw_pending = true;
        }
    }

    /// Queues validation for an attached Figure.
    pub(crate) fn revalidate(&mut self, figure: FigureId) -> Result<(), RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(figure)?;
            runtime.tree.mark_invalid(&mut runtime.updates, figure);
            Ok(())
        })
    }

    /// Queues repaint damage expressed in the Figure's node-local domain.
    pub(crate) fn repaint(
        &mut self,
        figure: FigureId,
        rect: Option<Rectangle>,
    ) -> Result<(), RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(figure)?;
            if let Some(bounds) = rect
                && (!bounds.x.is_finite()
                    || !bounds.y.is_finite()
                    || !bounds.width.is_finite()
                    || !bounds.height.is_finite()
                    || bounds.width < 0.0
                    || bounds.height < 0.0)
            {
                return Err(RuntimeMutationError::InvalidBounds { figure, bounds });
            }
            runtime.tree.repaint(&mut runtime.updates, figure, rect);
            Ok(())
        })
    }

    pub(crate) fn dispatch_mouse_moved(&mut self, x: f64, y: f64) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_moved(ctx, x, y));
        self.sync_tooltip();
        outcome
    }

    pub(crate) fn dispatch_mouse_pressed(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_pressed(ctx, x, y, button));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn dispatch_mouse_released(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_released(ctx, x, y, button));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn dispatch_mouse_double_clicked(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| {
            dispatcher.dispatch_mouse_double_clicked(ctx, x, y, button)
        });
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn dispatch_mouse_hover(&mut self, x: f64, y: f64) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.tooltip_controller
            .set_pointer_position(Point::new(x, y));
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_mouse_hover(ctx, x, y));
        self.sync_tooltip();
        outcome
    }

    pub(crate) fn dispatch_scroll(&mut self, event: WheelEvent) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome = self.dispatch(|dispatcher, ctx| dispatcher.dispatch_scroll(ctx, event));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn set_view_location(
        &mut self,
        viewport: &ViewportHandle,
        x: f64,
        y: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(viewport.figure_id())?;
            if !x.is_finite() || !y.is_finite() {
                return Err(RuntimeMutationError::Rejected);
            }
            // Materialization can fail; preserve every owner and source value until it succeeds.
            runtime.viewport_input_location(viewport)?;
            let contents = viewport
                .contents(&runtime.tree)
                .map_err(|_| RuntimeMutationError::Rejected)?;
            let owners = runtime
                .animations
                .viewport_owners(viewport.figure_id(), contents);
            for owner in owners {
                runtime
                    .animations()
                    .cancel(owner)
                    .map_err(RuntimeMutationError::Animation)?;
            }
            viewport
                .set_view_location(&mut runtime.tree, &mut runtime.updates, x, y)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub(crate) fn viewport_input_location(
        &mut self,
        viewport: &ViewportHandle,
    ) -> Result<crate::geometry::Point, RuntimeMutationError> {
        let contents = viewport
            .contents(&self.tree)
            .map_err(|_| RuntimeMutationError::Rejected)?;
        if self
            .animations
            .viewport_owners(viewport.figure_id(), contents)
            .is_empty()
        {
            return Ok(viewport.view_location());
        }
        let capture = self
            .animation()
            .capture_geometry([viewport.figure_id()])
            .map_err(|error| match error {
                crate::animation::GeometryCaptureError::Animation(error) => {
                    RuntimeMutationError::Animation(error)
                }
                crate::animation::GeometryCaptureError::Source(_) => RuntimeMutationError::Rejected,
            })?;
        let geometry = capture
            .geometry()
            .figure(viewport.figure_id())
            .map_err(RuntimeMutationError::Animation)?;
        let offset = geometry
            .child_transform
            .transform_point(crate::geometry::Point::ZERO);
        let scale = viewport.content_scale();
        Ok(crate::geometry::Point::new(
            (geometry.client_box.x - offset.x()) / scale,
            (geometry.client_box.y - offset.y()) / scale,
        ))
    }

    pub(crate) fn set_zoom_at(
        &mut self,
        zoom: &ZoomManager,
        scale: f64,
        anchor: Option<crate::geometry::Point>,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(zoom.viewport().figure_id())?;
            runtime.validate_attached_figure(zoom.scalable().figure_id())?;
            zoom.set_zoom_at(&mut runtime.tree, &mut runtime.updates, scale, anchor)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub(crate) fn fit_zoom_to_contents(
        &mut self,
        zoom: &ZoomManager,
    ) -> Result<bool, RuntimeMutationError> {
        self.guarded_runtime_mutation(|runtime| {
            runtime.validate_attached_figure(zoom.viewport().figure_id())?;
            runtime.validate_attached_figure(zoom.scalable().figure_id())?;
            zoom.fit_all(&mut runtime.tree, &mut runtime.updates)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub(crate) fn dispatch_zoom(&mut self, event: ZoomEvent) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        self.dispatch(|dispatcher, ctx| dispatcher.dispatch_zoom(ctx, event))
    }

    pub(crate) fn dispatch_key_pressed(
        &mut self,
        key: Key,
        modifiers: KeyModifiers,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_pressed(ctx, key, modifiers));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn dispatch_key_released(
        &mut self,
        key: Key,
        modifiers: KeyModifiers,
    ) -> crate::DispatchOutcome {
        if self.faulted {
            return crate::DispatchOutcome::default();
        }
        let outcome =
            self.dispatch(|dispatcher, ctx| dispatcher.dispatch_key_released(ctx, key, modifiers));
        self.tooltip_controller.dismiss();
        outcome
    }

    pub(crate) fn request_focus(&mut self, target: FigureId) -> Result<FocusChange, FocusError> {
        if self.faulted {
            return Err(FocusError::Faulted);
        }
        self.validate_direct_focus(target)?;
        let change = self.dispatch(|dispatcher, ctx| dispatcher.set_focus(ctx, Some(target)));
        if matches!(change, FocusChange::Changed { .. }) {
            self.tooltip_controller.dismiss();
        }
        Ok(change)
    }

    pub(crate) fn clear_focus(&mut self) -> FocusChange {
        if self.faulted {
            return FocusChange::Unchanged;
        }
        let change = self.dispatch(|dispatcher, ctx| dispatcher.release_focus(ctx));
        if matches!(change, FocusChange::Changed { .. }) {
            self.tooltip_controller.dismiss();
        }
        change
    }

    pub(crate) fn release_focus(&mut self) -> FocusChange {
        self.clear_focus()
    }

    pub(crate) fn traverse_focus(
        &mut self,
        direction: FocusTraversalDirection,
    ) -> FocusTraversalOutcome {
        if self.faulted {
            return FocusTraversalOutcome::Boundary;
        }
        self.guarded(|runtime| runtime.traverse_focus_inner(direction))
    }

    fn traverse_focus_inner(
        &mut self,
        direction: FocusTraversalDirection,
    ) -> FocusTraversalOutcome {
        let Some(scope) = self.tree.contents() else {
            return FocusTraversalOutcome::Boundary;
        };
        let candidate = self.focus_traversal_policy.traverse(
            &self.tree,
            scope,
            self.interaction.focus_owner(),
            direction,
        );
        let Some(candidate) = candidate else {
            return FocusTraversalOutcome::Boundary;
        };
        let in_scope = candidate == scope || self.tree.is_ancestor_of(scope, candidate) == Ok(true);
        if !in_scope || self.tree.can_traverse_focus(candidate) != Ok(true) {
            return FocusTraversalOutcome::Boundary;
        }

        match self.dispatch(|dispatcher, ctx| dispatcher.set_focus(ctx, Some(candidate))) {
            FocusChange::Changed { .. } => {
                self.tooltip_controller.dismiss();
                FocusTraversalOutcome::Moved(candidate)
            }
            FocusChange::Unchanged => FocusTraversalOutcome::Boundary,
        }
    }

    pub(crate) fn set_focus_traversal_policy(&mut self, policy: Box<dyn FocusTraversalPolicy>) {
        if !self.faulted {
            self.focus_traversal_policy = policy;
        }
    }

    pub(crate) fn cancel_gestures(&mut self) {
        if self.faulted {
            return;
        }
        self.dispatch(|dispatcher, ctx| dispatcher.cancel_gestures(ctx));
    }

    pub(crate) fn pointer_exited(&mut self) {
        if self.faulted {
            return;
        }
        if let Some(point) = self.tooltip_controller.pointer_position() {
            self.dispatch(|dispatcher, ctx| {
                dispatcher.dispatch_pointer_exited(ctx, point.x(), point.y())
            });
        }
        self.tooltip_controller.clear_pointer_position();
    }

    /// Applies all callback effects and structural mutations before returning.
    fn dispatch<R>(
        &mut self,
        action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>) -> R,
    ) -> R {
        assert!(!self.faulted, "cannot dispatch into a faulted Runtime");
        self.guarded(|runtime| runtime.dispatch_inner(action))
    }

    fn dispatch_inner<R>(
        &mut self,
        action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>) -> R,
    ) -> R {
        self.poll_activities();
        let result = {
            let mut context = SceneDispatchContext::new(
                &mut self.tree,
                &mut self.interaction,
                &mut self.updates,
                &mut self.mutations,
            );
            action(&mut self.interaction_dispatcher, &mut context)
        };
        let mutations = self.mutations.drain();
        self.apply_runtime_mutations(mutations);
        self.retain_interactive_figures();
        self.sync_clickable_visuals();
        self.poll_activities();
        result
    }

    fn poll_activities(&mut self) -> bool {
        let changed = SceneDispatchContext::new(
            &mut self.tree,
            &mut self.interaction,
            &mut self.updates,
            &mut self.mutations,
        )
        .poll_activities(self.activity_now);
        let mutations = self.mutations.drain();
        changed | self.apply_runtime_mutations(mutations)
    }

    fn ensure_layered_pane(&mut self, pane_id: FigureId) -> Result<(), LayerError> {
        if !self.tree.is_layered_pane(pane_id) || self.tree.is_attached(pane_id) != Ok(true) {
            return Err(LayerError::UnknownPane);
        }
        self.layered_panes.entry(pane_id).or_default();
        Ok(())
    }

    fn apply_runtime_mutations(&mut self, mutations: Vec<PendingMutation>) -> bool {
        let mut changed = false;
        for mutation in mutations {
            match self.apply_runtime_mutation(mutation.into_kind()) {
                Ok(mutation_changed) => changed |= mutation_changed,
                Err(error) => self.deferred_mutation_errors.push_back(error),
            }
        }
        changed
    }

    fn apply_runtime_mutation(
        &mut self,
        mutation: PendingMutationKind,
    ) -> Result<bool, RuntimeMutationError> {
        match mutation {
            PendingMutationKind::SetViewportAxis {
                viewport,
                horizontal,
                value,
                relative,
            } => {
                if relative {
                    self.viewports().scroll_by(
                        viewport,
                        if horizontal { value } else { 0.0 },
                        if horizontal { 0.0 } else { value },
                    )
                } else if horizontal {
                    self.viewports().set_horizontal_location(viewport, value)
                } else {
                    self.viewports().set_vertical_location(viewport, value)
                }
            }
            PendingMutationKind::SetLayoutManager { container, manager } => {
                self.replace_layout_manager(container, manager)
            }
            PendingMutationKind::SetLayoutConstraint { child, constraint } => {
                self.set_boxed_layout_constraint(child, constraint)
            }
            PendingMutationKind::RemoveLayoutConstraint { child } => {
                self.remove_layout_constraint(child)
            }
            PendingMutationKind::SetSizeOverride { figure, kind, size } => {
                self.set_size_override(figure, kind, size)
            }
            PendingMutationKind::SetBounds { figure, bounds } => self.set_bounds(figure, bounds),
            PendingMutationKind::ScrollViewport { viewport, dx, dy } => {
                self.viewports().scroll_by(viewport, dx, dy)
            }
            PendingMutationKind::SetScale { figure, scale } => {
                self.viewports().set_scale(figure, scale)
            }
            PendingMutationKind::SetStyle { figure, style } => self.set_figure_style(figure, style),
            PendingMutationKind::MoveChildToIndex {
                parent,
                child,
                index,
            } => self.move_child_to_index(parent, child, index),
            PendingMutationKind::BringChildToFront { parent, child } => {
                self.bring_child_to_front(parent, child)
            }
            PendingMutationKind::SendChildToBack { parent, child } => {
                self.send_child_to_back(parent, child)
            }
            PendingMutationKind::SetChildClippingStrategy { figure, strategy } => {
                self.set_child_clipping_strategy(figure, strategy)
            }
            PendingMutationKind::AddLayerFigure {
                pane,
                figure,
                key,
                placement,
            } => self
                .add_layer(pane, figure, key, placement)
                .map(|_| true)
                .map_err(|error| match error {
                    LayerError::Resource(error) => RuntimeMutationError::Resource(error),
                    LayerError::Graph(error) => error.into(),
                    _ => RuntimeMutationError::Rejected,
                }),
            PendingMutationKind::RemoveLayer { pane, key } => self
                .remove_layer(pane, &key)
                .map(|_| true)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::MoveLayer {
                pane,
                key,
                placement,
            } => self
                .move_layer(pane, &key, placement)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::ReparentLayer {
                child,
                new_pane,
                key,
                placement,
            } => self
                .reparent_layer(child, new_pane, key, placement)
                .map_err(|_| RuntimeMutationError::Rejected),
            PendingMutationKind::RemoveChild { parent, child } => self.remove_figure(parent, child),
            PendingMutationKind::Reparent { child, new_parent } => self.reparent(child, new_parent),
            PendingMutationKind::AddChildFigure { parent, figure } => {
                self.add_figure(parent, figure).map(|_| true)
            }
            PendingMutationKind::SetContentsSubtree {
                viewport,
                subtree,
                completed,
            } => self.guarded(|runtime| {
                match match viewport {
                    Some(viewport) => runtime.viewports().set_contents_subtree(viewport, subtree),
                    None => runtime.set_contents_subtree(subtree),
                } {
                    Ok(receipt) => {
                        completed(Ok(receipt));
                        Ok(true)
                    }
                    Err(error) => {
                        runtime.deferred_mutation_errors.push_back(error);
                        completed(Err(runtime
                            .deferred_mutation_errors
                            .back()
                            .expect("rejection recorded before completion")));
                        Ok(false)
                    }
                }
            }),
            PendingMutationKind::AddSubtree {
                parent,
                subtree,
                replacing,
                completed,
            } => self.guarded(|runtime| {
                match runtime.replace_or_add_subtree(parent, replacing, subtree) {
                    Ok(receipt) => {
                        completed(Ok(receipt));
                        Ok(true)
                    }
                    Err(error) => {
                        runtime.deferred_mutation_errors.push_back(error);
                        completed(Err(runtime
                            .deferred_mutation_errors
                            .back()
                            .expect("rejection recorded before completion")));
                        Ok(false)
                    }
                }
            }),
            PendingMutationKind::UpdateComponent(update) => update.apply(self),
            PendingMutationKind::UpdateCapability(update) => update.apply(self),
        }
    }

    fn replace_or_add_subtree(
        &mut self,
        parent: FigureId,
        replacing: Option<FigureId>,
        subtree: crate::tree::FigureSubtree,
    ) -> Result<crate::tree::SubtreeReceipt, RuntimeMutationError> {
        let Some(child) = replacing else {
            return self.add_subtree(parent, subtree);
        };
        self.guarded_runtime_mutation(move |runtime| {
            runtime.validate_container_figure(parent)?;
            runtime.validate_attached_figure(child)?;
            if runtime.tree.parent_id(child) != Ok(Some(parent)) {
                return Err(RuntimeMutationError::InvalidParentRelation { parent, child });
            }
            runtime
                .connections
                .validate_disposal()
                .map_err(|_| RuntimeMutationError::Rejected)?;
            let owner = subtree.owner;
            let (inserted, retired) = runtime.publish_subtree(parent, subtree, Some(child))?;
            if let Some((parent, retired, damage)) = retired {
                runtime.complete_subtree_retirement(parent, retired, damage);
            }
            runtime.complete_subtree_attachment(&inserted);
            Ok(crate::tree::SubtreeReceipt::new(
                owner,
                inserted.into_iter().map(|(id, _)| id).collect(),
            ))
        })
    }

    fn initialize_layered_panes(&mut self) {
        let Some(contents) = self.tree.contents() else {
            return;
        };
        let mut ids = vec![contents];
        ids.extend(self.tree.descendant_ids(contents).unwrap_or_default());
        for id in ids {
            self.register_layered_pane(id);
        }
    }

    fn activate_attached_figures(&mut self) {
        for (figure, parent) in self.tree.attached_ids_parent_first() {
            self.tree.complete_attachment(figure, parent);
        }
    }

    fn register_layered_pane(&mut self, pane_id: FigureId) {
        if !self.tree.is_layered_pane(pane_id) {
            return;
        }
        if self
            .tree
            .layout_manager(pane_id)
            .expect("live layered pane")
            .is_none()
        {
            self.tree
                .replace_layout_manager(pane_id, Some(Box::new(StackLayout)));
        }
        self.layered_panes.entry(pane_id).or_default();
    }

    fn resolve_layer_index(
        &self,
        pane_id: FigureId,
        state: &LayeredPaneState,
        placement: &LayerPlacement,
        moving: Option<FigureId>,
    ) -> Result<usize, LayerError> {
        let order = self
            .tree
            .child_order(pane_id)
            .map_err(|_| LayerError::UnknownPane)?;
        let filtered: Vec<_> = order
            .into_iter()
            .filter(|child| Some(*child) != moving)
            .collect();
        match placement {
            LayerPlacement::Last => Ok(filtered.len()),
            LayerPlacement::Before(reference) | LayerPlacement::After(reference) => {
                let reference_child = state.layer(reference).ok_or(LayerError::UnknownKey)?;
                if Some(reference_child) == moving {
                    return self
                        .tree
                        .child_z_index(pane_id, reference_child)
                        .map_err(|_| LayerError::InconsistentState)?
                        .ok_or(LayerError::InconsistentState);
                }
                let index = filtered
                    .iter()
                    .position(|child| *child == reference_child)
                    .ok_or(LayerError::InconsistentState)?;
                if matches!(placement, LayerPlacement::After(_)) {
                    Ok(index + 1)
                } else {
                    Ok(index)
                }
            }
        }
    }

    fn retain_runtime_state(&mut self) {
        let tree = &self.tree;
        self.layered_panes.retain(|pane, state| {
            if tree.is_attached(*pane) != Ok(true) || !tree.is_layered_pane(*pane) {
                return false;
            }
            state.retain_children(|child| tree.parent_id(child) == Ok(Some(*pane)));
            true
        });
        self.connections
            .retain_connections(|connection| tree.is_attached(connection) == Ok(true));
        self.anchor_geometries
            .retain(|(figure, _), _| tree.is_attached(*figure) == Ok(true));
    }

    fn invalidate_connection_figure_change(&mut self, figure: FigureId, topology_changed: bool) {
        if let Err(error) = self
            .connections
            .invalidate_figure_change(figure, topology_changed)
        {
            self.connection_error = Some(error);
        }
    }

    fn retain_interactive_figures(&mut self) {
        self.retain_runtime_state();
        self.interaction.reconcile_non_focus(&self.tree);
        self.resources
            .retain_dependencies(|id| self.tree.is_attached(id) == Ok(true));

        let invalid_focus = self
            .interaction
            .focus_owner()
            .is_some_and(|id| !self.tree.can_retain_focus(id));
        if invalid_focus {
            {
                let mut context = SceneDispatchContext::new(
                    &mut self.tree,
                    &mut self.interaction,
                    &mut self.updates,
                    &mut self.mutations,
                );
                self.interaction_dispatcher.set_focus(&mut context, None);
            }
            let mutations = self.mutations.drain();
            self.apply_runtime_mutations(mutations);
            self.interaction.reconcile_non_focus(&self.tree);
            self.resources
                .retain_dependencies(|id| self.tree.is_attached(id) == Ok(true));
            self.tooltip_controller.dismiss();
        }
        self.sync_tooltip();
    }

    fn sync_tooltip(&mut self) {
        let source = if self.interaction.captured().is_none() {
            let source = self
                .interaction
                .cursor_target()
                .and_then(|id| self.tree.tooltip_source(id));
            self.interaction
                .set_hover_source(source.as_ref().map(|(id, _)| *id));
            source
        } else {
            None
        };
        self.tooltip_controller.reconcile(source);
    }

    fn sync_clickable_visuals(&mut self) {
        let cursor_target = self.interaction.cursor_target();
        let focus_owner = self.interaction.focus_owner();
        for id in self.tree.clickable_ids() {
            let hovered = cursor_target.is_some_and(|target| {
                target == id || self.tree.is_ancestor_of(id, target) == Ok(true)
            });
            let visual = ClickableVisualState {
                hovered,
                pressed: self.interaction.is_keyboard_pressed(id)
                    || (hovered && self.interaction.is_pointer_pressed(id)),
                focused: focus_owner == Some(id),
                enabled: self
                    .tree
                    .is_effectively_enabled(id)
                    .expect("live clickable"),
            };
            self.tree
                .sync_clickable_visual(&mut self.updates, id, visual);
        }
    }

    fn validate_direct_focus(&self, target: FigureId) -> Result<(), FocusError> {
        if !self.tree.is_attached(target)? {
            return Err(FocusError::Detached(target));
        }
        if !self.tree.is_effectively_visible(target)? {
            return Err(FocusError::Hidden(target));
        }
        if !self.tree.is_effectively_enabled(target)? {
            return Err(FocusError::Disabled(target));
        }
        if !self.tree.is_focusable(target)? {
            return Err(FocusError::NotFocusable(target));
        }
        Ok(())
    }

    fn record_resource_input(&mut self, resource: crate::render::ResourceId, figure: FigureId) {
        let status = self.resources.status(resource).ok().cloned();
        if let Some(node) = self.tree.node_mut(figure) {
            if let Some((_, old)) = node
                .resource_inputs
                .iter_mut()
                .find(|(id, _)| *id == resource)
            {
                *old = status;
            } else {
                node.resource_inputs.push((resource, status));
            }
        }
    }

    fn invalidate_resource_dependents(
        &mut self,
        resource: crate::render::ResourceId,
        dependents: Vec<FigureId>,
    ) {
        self.tree
            .synchronize_preparation_text_revision(self.text.revision());
        for figure in dependents {
            if self.tree.is_attached(figure) == Ok(true) {
                self.record_resource_input(resource, figure);
                self.tree.mark_invalid(&mut self.updates, figure);
                self.tree.repaint(&mut self.updates, figure, None);
            }
        }
    }
}

fn route_metadata_in_local(metadata: &RouteMetadata, path_bounds: Rectangle) -> RouteMetadata {
    let translate_endpoint = |endpoint: crate::RouteEndpoint| crate::RouteEndpoint {
        reference: Point::new(
            endpoint.reference.x() - path_bounds.x,
            endpoint.reference.y() - path_bounds.y,
        ),
        site: crate::AnchorSite {
            point: Point::new(
                endpoint.site.point.x() - path_bounds.x,
                endpoint.site.point.y() - path_bounds.y,
            ),
            outward_normal: endpoint.site.outward_normal,
        },
    };
    RouteMetadata {
        source: translate_endpoint(metadata.source),
        target: translate_endpoint(metadata.target),
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::render::{FontData, ImageData, ResourceId};
    use crate::{
        Bounded, EventContext, FigureEvent, FigureEventHandler, FocusEvent, FocusEventKind,
        Rectangle, RectangleFigure, ResourceStatus,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    struct PaintCounterFigure {
        paints: Arc<AtomicUsize>,
    }

    impl Figure for PaintCounterFigure {
        fn name(&self) -> &'static str {
            "PaintCounterFigure"
        }

        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }

        fn paint(
            &self,
            _context: &mut crate::PaintContext<'_>,
            _bounds: Rectangle,
        ) -> Result<(), crate::GraphicsError> {
            self.paints.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FocusProbeFigure {
        bounds: Rectangle,
        events: Arc<Mutex<Vec<FocusEvent>>>,
    }

    impl Bounded for FocusProbeFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(
            &mut self,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        ) -> Result<(), crate::figure::ShapeGeometryError> {
            self.bounds = crate::figure::ShapeGeometryError::validate_bounds(Rectangle::new(
                x, y, width, height,
            ))?;

            Ok(())
        }

        fn name(&self) -> &'static str {
            "FocusProbeFigure"
        }
    }

    impl Figure for FocusProbeFigure {
        fn initial_bounds(&self) -> Rectangle {
            self.bounds
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn register_capabilities(
            &self,
            out: &mut crate::FigureCapabilityBuilder,
        ) -> Result<(), crate::FigureCapabilityRegistrationError> {
            out.register(crate::INPUT, crate::InputCapability::of::<Self>())
        }
    }

    impl FigureEventHandler for FocusProbeFigure {
        fn on_focus_gained(&self, event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
            self.events.lock().unwrap().push(*event);
            true
        }

        fn on_focus_lost(&self, event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
            self.events.lock().unwrap().push(*event);
            true
        }
    }

    fn focused_probe_runtime() -> (Runtime, FigureId, FigureId, Arc<Mutex<Vec<FocusEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ));
        let probe = tree.add_child_to(
            root,
            Box::new(FocusProbeFigure {
                bounds: Rectangle::new(10.0, 10.0, 20.0, 20.0),
                events: Arc::clone(&events),
            }),
        );
        tree.set_focusable(probe, true);
        let mut runtime = Runtime::new(tree);
        assert!(matches!(
            runtime.request_focus(probe),
            Ok(FocusChange::Changed { .. })
        ));
        (runtime, root, probe, events)
    }

    fn surface(width: u32, height: u32) -> SurfaceInfo {
        SurfaceInfo {
            logical_width: f64::from(width),
            logical_height: f64::from(height),
            pixel_width: width,
            pixel_height: height,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn runtime_imports_builder_validation_state_before_first_frame() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid FigureTree construction");
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds")),
            )
            .unwrap();
        tree.builder()
            .set_layout_manager(root, Box::new(StackLayout::new()))
            .unwrap();

        let mut runtime = Runtime::new(tree);
        assert!(!runtime.tree().is_valid(root).unwrap());

        assert!(runtime.prepare_frame().is_some());
        assert!(runtime.tree().is_valid(root).unwrap());
        assert_eq!(
            runtime.tree().figure_bounds(child),
            Ok(Rectangle::new(0.0, 0.0, 100.0, 100.0))
        );
    }

    #[test]
    fn first_full_submission_paints_each_figure_once() {
        let paints = Arc::new(AtomicUsize::new(0));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(PaintCounterFigure {
                paints: Arc::clone(&paints),
            }))
            .expect("valid FigureTree construction");
        tree.builder().validate_subtree(root).unwrap();
        let mut runtime = Runtime::new(tree);

        assert!(matches!(
            runtime.prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Ready(_)
        ));
        assert_eq!(paints.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dispatch_flushes_structural_mutations_before_returning() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 100.0, 100.0, Color::WHITE)
                .expect("valid figure bounds"),
        ));
        let mut runtime = Runtime::new(tree);

        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds")),
            ));
        runtime.dispatch(|_, _| {});

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.has_pending_update());
    }

    #[test]
    fn runtime_owns_interaction_state_separately_from_tree() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");

        runtime.interaction.set_mouse_target(Some(root));

        assert_eq!(runtime.interaction().mouse_target(), Some(root));
        assert_eq!(runtime.tree().contents(), Some(root));
    }

    #[test]
    fn runtime_owns_an_isolated_text_layout_context() {
        let mut first = Runtime::empty();
        let mut second = Runtime::empty();
        first.register_builtin_font(BuiltinFont::Inter).unwrap();
        second.register_builtin_font(BuiltinFont::Inter).unwrap();
        let font = FontDescriptor::default();

        let first_layout = first
            .layout_text("runtime text", &font, TextConstraints::UNBOUNDED)
            .unwrap();
        let second_layout = second
            .layout_text("runtime text", &font, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(first_layout.width() > 0.0);
        assert_eq!(first_layout.width(), second_layout.width());
        assert_eq!(first.text_revision(), 1);
        assert_eq!(second.text_revision(), 1);
    }

    #[test]
    fn builtin_fonts_require_explicit_runtime_registration() {
        let mut runtime = Runtime::empty();
        let font = FontDescriptor::default();

        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), None);
        assert_eq!(
            runtime.layout_text("text", &font, TextConstraints::UNBOUNDED),
            Err(TextError::NoUsableFont)
        );

        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), Some(id));
        assert_eq!(
            runtime.register_builtin_font(BuiltinFont::Inter).unwrap(),
            id
        );
        assert!(
            !runtime
                .layout_text("text", &font, TextConstraints::UNBOUNDED)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn explicit_font_registration_is_included_in_the_next_submission() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert!(matches!(
            &submission.resources,
            ResourceSync::Snapshot(snapshot)
                if matches!(
                    snapshot.ready.as_slice(),
                    [crate::render::ResourceUpdate {
                        id: resource_id,
                        revision: 1,
                        payload: crate::render::ResourcePayload::Font(_),
                    }] if *resource_id == id.resource_id()
                )
        ));
    }

    #[test]
    fn invalid_font_replacement_preserves_the_ready_revision() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime.resources.take_delta();

        assert_eq!(
            runtime.complete_font(id, FontData::new(vec![1, 2, 3])),
            Err(ResourceError::InvalidFontData)
        );
        assert_eq!(
            runtime.resource_status(id.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
        assert!(runtime.resources.take_delta().is_empty());

        let layout = runtime
            .layout_text(
                "text",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        assert!(
            layout
                .glyph_runs()
                .iter()
                .all(|run| run.font.resource_id() == id.resource_id() && run.font.revision() == 1)
        );
    }

    #[test]
    fn removing_a_builtin_font_removes_its_layout_and_backend_resources() {
        let mut runtime = Runtime::empty();
        let id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime.resources.take_delta();

        runtime.remove_resource(id.resource_id()).unwrap();

        assert_eq!(runtime.builtin_font(BuiltinFont::Inter), None);
        assert_eq!(
            runtime.layout_text(
                "text",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            ),
            Err(TextError::NoUsableFont)
        );
        assert_eq!(
            runtime.resources.take_delta().ops,
            vec![crate::render::ResourceOp::Remove(id.resource_id())]
        );
    }

    #[test]
    fn runtime_accepts_a_non_parley_text_layout_engine() {
        use crate::render::{
            FontFaceRef, GlyphRun, PositionedGlyph, TextLayoutParts, TextLineMetrics,
        };

        struct StubTextEngine {
            face: Option<FontFaceRef>,
        }

        impl TextLayoutEngine for StubTextEngine {
            fn revision(&self) -> u64 {
                42
            }

            fn register_font(
                &mut self,
                resource_id: ResourceId,
                revision: u64,
                _bytes: &[u8],
            ) -> Result<(), TextError> {
                self.face = Some(FontFaceRef::new(resource_id, revision, 0));
                Ok(())
            }

            fn remove_font(&mut self, resource_id: ResourceId) {
                if self
                    .face
                    .as_ref()
                    .is_some_and(|face| face.resource_id() == resource_id)
                {
                    self.face = None;
                }
            }

            fn layout(
                &mut self,
                text: &str,
                font: &FontDescriptor,
                constraints: TextConstraints,
            ) -> Result<TextLayout, TextError> {
                let face = self.face.clone().ok_or(TextError::NoUsableFont)?;
                TextLayout::from_parts(TextLayoutParts {
                    text: text.to_owned(),
                    font: font.clone(),
                    constraints,
                    engine_revision: self.revision(),
                    width: 20.0,
                    full_width: 20.0,
                    height: 12.0,
                    lines: vec![TextLineMetrics {
                        ascent: 8.0,
                        descent: 2.0,
                        leading: 2.0,
                        baseline: 8.0,
                        advance: 20.0,
                    }],
                    glyph_runs: vec![GlyphRun {
                        font: face,
                        font_size: font.size,
                        normalized_coords: Arc::from([]),
                        skew_degrees: None,
                        glyphs: crate::text::shaping::GlyphRange::new(vec![PositionedGlyph {
                            id: 1,
                            x: 0.0,
                            y: 8.0,
                        }])?,
                    }],
                    visible_range: 0..text.len(),
                    truncated: false,
                })
            }
        }

        let mut runtime = Runtime::with_text_layout_engine(
            FigureTree::new(),
            Box::new(StubTextEngine { face: None }),
        );
        let font_id = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();

        assert_eq!(runtime.text_revision(), 42);
        let layout = runtime
            .layout_text(
                "custom",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        assert!(!layout.is_empty());
        assert_eq!(
            layout.glyph_runs()[0].font.resource_id(),
            font_id.resource_id()
        );
    }

    #[test]
    fn direct_and_traversal_focus_eligibility_are_independent() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let traversal_only = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        let direct_only = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        runtime
            .set_focus_traversable(traversal_only, true)
            .expect("valid Runtime mutation");
        runtime
            .set_focusable(direct_only, true)
            .expect("valid Runtime mutation");

        assert_eq!(
            runtime.request_focus(traversal_only),
            Err(FocusError::NotFocusable(traversal_only))
        );
        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Moved(traversal_only)
        );
        assert_eq!(runtime.interaction().focus_owner(), Some(traversal_only));
        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Boundary
        );

        assert!(matches!(
            runtime.request_focus(direct_only),
            Ok(FocusChange::Changed { .. })
        ));
        assert_eq!(runtime.interaction().focus_owner(), Some(direct_only));
    }

    #[test]
    fn direct_focus_reports_structured_eligibility_errors_without_changing_owner() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let target = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        runtime
            .set_focusable(root, true)
            .expect("valid Runtime mutation");
        runtime.request_focus(root).unwrap();

        assert_eq!(
            runtime.request_focus(FigureId::null()),
            Err(FocusError::ForeignFigure(FigureId::null()))
        );
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::NotFocusable(target))
        );
        runtime
            .set_focusable(target, true)
            .expect("valid Runtime mutation");
        runtime
            .set_visible(target, false)
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Hidden(target))
        );
        runtime
            .set_visible(target, true)
            .expect("valid Runtime mutation");
        runtime
            .set_enabled(target, false)
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::Disabled(target))
        );
        assert_eq!(runtime.interaction().focus_owner(), Some(root));

        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        assert_eq!(
            runtime.request_focus(target),
            Err(FocusError::UnknownFigure(target))
        );
    }

    #[test]
    fn invalid_focus_owner_emits_one_lost_event_for_hide_disable_and_remove() {
        enum Transition {
            Hide,
            Disable,
            Remove,
        }

        for transition in [Transition::Hide, Transition::Disable, Transition::Remove] {
            let (mut runtime, root, probe, events) = focused_probe_runtime();
            match transition {
                Transition::Hide => assert!(
                    runtime
                        .set_visible(probe, false)
                        .expect("valid Runtime mutation")
                ),
                Transition::Disable => assert!(
                    runtime
                        .set_enabled(probe, false)
                        .expect("valid Runtime mutation")
                ),
                Transition::Remove => assert!(
                    runtime
                        .remove_figure(root, probe)
                        .expect("valid Runtime mutation")
                ),
            }

            assert_eq!(runtime.interaction().focus_owner(), None);
            assert_eq!(
                *events.lock().unwrap(),
                vec![
                    FocusEvent {
                        kind: FocusEventKind::Gained,
                        related_target: None,
                    },
                    FocusEvent {
                        kind: FocusEventKind::Lost,
                        related_target: None,
                    },
                ]
            );
        }
    }

    #[test]
    fn reparent_under_hidden_ancestor_releases_focus() {
        let (mut runtime, root, probe, events) = focused_probe_runtime();
        let hidden_parent = runtime
            .add_figure(
                root,
                Box::new(
                    RectangleFigure::new(40.0, 40.0, 40.0, 40.0).expect("valid figure bounds"),
                ),
            )
            .expect("valid Runtime mutation");
        runtime
            .set_visible(hidden_parent, false)
            .expect("valid Runtime mutation");

        assert!(
            runtime
                .reparent(probe, hidden_parent)
                .expect("valid Runtime mutation")
        );

        assert_eq!(runtime.interaction().focus_owner(), None);
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&FocusEvent {
                kind: FocusEventKind::Lost,
                related_target: None,
            })
        );
    }

    #[test]
    fn runtime_rejects_invalid_custom_policy_candidate() {
        struct FixedPolicy(FigureId);

        impl FocusTraversalPolicy for FixedPolicy {
            fn traverse(
                &mut self,
                _tree: &FigureTree,
                _scope: FigureId,
                _current: Option<FigureId>,
                _direction: FocusTraversalDirection,
            ) -> Option<FigureId> {
                Some(self.0)
            }
        }

        let mut tree = FigureTree::new();
        let detached = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ));
        tree.set_focus_traversable(detached, true);
        let scope = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ));
        let eligible = tree.add_child_to(
            scope,
            Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
        );
        tree.set_focus_traversable(eligible, true);
        let mut runtime = Runtime::new(tree);
        runtime.set_focus_traversal_policy(Box::new(FixedPolicy(detached)));

        assert_eq!(
            runtime.traverse_focus(FocusTraversalDirection::Forward),
            FocusTraversalOutcome::Boundary
        );
        assert_eq!(runtime.interaction().focus_owner(), None);
    }

    #[test]
    fn cursor_and_tooltip_resolve_from_the_current_pointer_targets() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        runtime
            .set_figure_style(
                root,
                FigureStyle {
                    cursor: Some(CursorIcon::Crosshair),
                    tooltip: Some(Some("root tip".to_string())),
                    ..FigureStyle::default()
                },
            )
            .expect("valid Runtime mutation");

        runtime.dispatch_mouse_moved(50.0, 50.0);

        assert_eq!(runtime.cursor_icon(), CursorIcon::Crosshair);
        assert_eq!(runtime.tooltip().text().as_deref(), Some("root tip"));

        runtime.dispatch_mouse_moved(150.0, 150.0);
        assert_eq!(runtime.cursor_icon(), CursorIcon::Default);
        assert_eq!(runtime.tooltip().text(), None);
    }

    #[test]
    fn submission_contains_surface_resources_and_monotonic_frame_id() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
            .unwrap();

        let first = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(first.frame_id, FrameId::INITIAL);
        assert_eq!(first.surface, surface(100, 100));
        assert!(matches!(
            &first.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        assert_eq!(first.damage.mode(), DamageMode::Full);
        assert!(
            runtime
                .complete_submission(
                    first.session_id,
                    first.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_ok()
        );

        runtime.request_full_redraw();
        let second = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(second.frame_id, first.frame_id.next());
        assert!(
            runtime
                .complete_submission(
                    second.session_id,
                    second.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_ok()
        );
        assert!(matches!(
            runtime.prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Idle
        ));
    }

    #[test]
    fn resource_only_completion_produces_a_submission_without_scene_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 0, 0], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
        assert!(resource_only.commands.is_empty());
        assert!(matches!(
            &resource_only.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(update)]
                        if update.id == image.resource_id()
                )
        ));
    }

    #[test]
    fn resource_completion_invalidates_and_repaints_dependent_figure() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(
                root,
                Box::new(
                    RectangleFigure::new(10.0, 10.0, 20.0, 20.0).expect("valid figure bounds"),
                ),
            )
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let image = runtime.register_image().expect("image registration");
        runtime
            .add_resource_dependency(image.resource_id(), child)
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(2, 2, vec![255; 2 * 2 * 4], 1.0))
            .unwrap();

        assert_eq!(
            runtime.resource_status(image.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
        assert!(runtime.has_pending_update());
        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(matches!(
            &submission.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(update)]
                        if update.id == image.resource_id()
                )
        ));
        assert_ne!(submission.damage.mode(), DamageMode::None);
    }

    #[test]
    fn replacing_contents_removes_resource_dependencies_from_detached_figures() {
        let mut runtime = Runtime::empty();
        let old_contents = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .add_resource_dependency(image.resource_id(), old_contents)
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                replacement.session_id,
                replacement.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
            .unwrap();
        let resource_only = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert_eq!(resource_only.damage.mode(), DamageMode::None);
    }

    #[test]
    fn backend_capability_promotes_partial_damage_to_full() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(
                root,
                Box::new(
                    RectangleFigure::new(10.0, 10.0, 20.0, 20.0).expect("valid figure bounds"),
                ),
            )
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        runtime
            .set_bounds(
                child,
                crate::geometry::Rectangle::new(15.0, 15.0, 20.0, 20.0),
            )
            .expect("valid Runtime mutation");
        let partial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(partial.damage.mode(), DamageMode::Partial);
        runtime
            .complete_submission(
                partial.session_id,
                partial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        runtime
            .set_bounds(
                child,
                crate::geometry::Rectangle::new(20.0, 20.0, 20.0, 20.0),
            )
            .expect("valid Runtime mutation");
        let promoted = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY)
            .into_ready()
            .unwrap();
        assert_eq!(promoted.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn moving_contents_forces_full_damage_to_clear_exposed_pixels() {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(20.0, 20.0, 60.0, 60.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        assert!(
            runtime
                .translate(root, 10.0, 10.0)
                .expect("valid Runtime mutation")
        );
        let moved = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert_eq!(moved.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn surface_change_and_retry_force_full_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(
                image,
                ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
            )
            .unwrap();
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(
            runtime
                .complete_submission(
                    initial.session_id,
                    initial.frame_id,
                    Ok(RenderDisposition::Retry)
                )
                .is_ok()
        );

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(retry.damage.mode(), DamageMode::Full);
        assert!(matches!(
            &retry.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        runtime
            .complete_submission(
                retry.session_id,
                retry.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let resized = runtime
            .prepare_submission(surface(120, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(resized.damage.mode(), DamageMode::Full);
        assert!(matches!(
            resized.resources,
            ResourceSync::Delta(ref delta) if delta.is_empty()
        ));
    }

    #[test]
    fn retry_restores_in_flight_resource_updates_before_newer_updates() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let first_update = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        runtime
            .complete_submission(
                first_update.session_id,
                first_update.frame_id,
                Ok(RenderDisposition::Retry),
            )
            .expect("frame completion");

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        let ResourceSync::Delta(delta) = &retry.resources else {
            panic!("incremental retry must carry a delta");
        };
        let revisions: Vec<_> = delta
            .ops
            .iter()
            .filter_map(|operation| match operation {
                crate::render::ResourceOp::Upsert(update) => Some(update.revision),
                crate::render::ResourceOp::Remove(_) => None,
            })
            .collect();

        assert_eq!(revisions, vec![1, 2]);
    }

    #[test]
    fn resource_state_transitions_preserve_order_within_one_delta() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        runtime
            .fail_resource(image.resource_id(), "transient")
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        let ResourceSync::Delta(delta) = &submission.resources else {
            panic!("established session must use an incremental delta");
        };
        assert!(matches!(
            delta.ops.as_slice(),
            [
                crate::render::ResourceOp::Upsert(crate::render::ResourceUpdate {
                    revision: 1,
                    ..
                }),
                crate::render::ResourceOp::Remove(id),
                crate::render::ResourceOp::Upsert(crate::render::ResourceUpdate {
                    revision: 2,
                    ..
                }),
            ] if *id == image.resource_id()
        ));
    }

    #[test]
    fn backend_session_reset_rejects_old_completion_and_resends_ready_snapshot() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let old = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        let new_session = runtime.reset_backend_session().unwrap();
        assert_ne!(new_session, old.session_id);
        assert!(
            runtime
                .complete_submission(
                    old.session_id,
                    old.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_err()
        );

        let replacement = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(replacement.session_id, new_session);
        assert_eq!(replacement.damage.mode(), DamageMode::Full);
        assert!(matches!(
            &replacement.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        ));
        assert!(
            runtime
                .complete_submission(
                    replacement.session_id,
                    replacement.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_ok()
        );

        runtime.request_full_redraw();
        let redraw = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(redraw.session_id, new_session);
        assert!(matches!(
            redraw.resources,
            ResourceSync::Delta(ref delta) if delta.is_empty()
        ));
    }

    #[test]
    fn backend_sessions_are_namespaced_per_runtime() {
        let first = Runtime::empty().backend_session_id();
        let second = Runtime::empty().backend_session_id();

        assert_ne!(first.runtime_namespace(), second.runtime_namespace());
        assert_eq!(first.generation(), 1);
        assert_eq!(second.generation(), 1);
    }

    #[test]
    fn snapshot_retry_refreezes_current_registry_state() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let snapshot = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        runtime
            .fail_resource(image.resource_id(), "transient")
            .unwrap();
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        assert!(
            runtime
                .complete_submission(
                    snapshot.session_id,
                    snapshot.frame_id,
                    Ok(RenderDisposition::Retry)
                )
                .is_ok()
        );

        let retry = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(matches!(
            &retry.resources,
            ResourceSync::Snapshot(snapshot)
                if matches!(
                    snapshot.ready.as_slice(),
                    [crate::render::ResourceUpdate {
                        revision: 2,
                        ..
                    }]
                )
        ));
    }

    #[test]
    fn mutations_after_snapshot_freeze_remain_as_incremental_delta() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let image = runtime.register_image().expect("image registration");
        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();
        let snapshot = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        runtime
            .complete_image(image, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        assert!(
            runtime
                .complete_submission(
                    snapshot.session_id,
                    snapshot.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_ok()
        );

        let delta = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(matches!(
            &delta.resources,
            ResourceSync::Delta(delta)
                if matches!(
                    delta.ops.as_slice(),
                    [crate::render::ResourceOp::Upsert(
                        crate::render::ResourceUpdate { revision: 2, .. }
                    )]
                )
        ));
    }

    #[test]
    fn dpi_change_updates_surface_metadata_and_forces_full_damage() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        let initial = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let hidpi_surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 200,
            pixel_height: 200,
            scale_factor: 2.0,
        };
        let submission = runtime
            .prepare_submission(hidpi_surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert_eq!(submission.surface, hidpi_surface);
        assert_eq!(submission.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn suspended_surface_preserves_full_redraw_request() {
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");

        assert!(matches!(
            runtime.prepare_submission(surface(0, 100), BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Suspended
        ));
        assert!(runtime.has_pending_update());

        let resumed = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_eq!(resumed.damage.mode(), DamageMode::Full);
    }

    #[test]
    fn frame_preparation_reports_distinct_non_ready_states() {
        let mut runtime = Runtime::empty();
        let surface = surface(100, 100);
        let FramePreparation::Ready(first) =
            runtime.prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("initial frame must be ready");
        };
        assert!(matches!(
            runtime.prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::AwaitingCompletion
        ));
        assert!(
            runtime
                .complete_submission(
                    first.session_id,
                    first.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .is_ok()
        );
        assert!(matches!(
            runtime.prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Idle
        ));
        assert!(matches!(
            runtime.prepare_submission(
                SurfaceInfo {
                    pixel_width: 0,
                    ..surface
                },
                BackendCapabilities::RETAINED_PARTIAL
            ),
            FramePreparation::Suspended
        ));
        runtime.faulted = true;
        assert!(matches!(
            runtime.prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Error(FramePreparationError::Faulted)
        ));
    }

    #[test]
    fn frame_preparation_reports_permanent_backend_capability_mismatch() {
        let mut runtime = Runtime::empty();
        runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime
            .set_contents(Box::new(crate::LabelFigure::new("unsupported")))
            .expect("valid Runtime mutation");

        let preparation =
            runtime.prepare_submission(surface(100, 100), BackendCapabilities::FULL_FRAME_ONLY);
        assert!(
            matches!(
                preparation,
                FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(
                    UnsupportedRenderCapability {
                        capability: crate::render::RenderCapability::GlyphRuns,
                    }
                ))
            ),
            "unexpected preparation state: {preparation:?}"
        );
    }

    #[test]
    fn submission_notifications_are_flushed_in_causal_order() {
        struct CaptureUpdates(Arc<Mutex<Vec<UpdateEvent>>>);

        impl UpdateListener for CaptureUpdates {
            fn on_update_event(&self, event: UpdateEvent) -> crate::ListenerDirective {
                self.0.lock().unwrap().push(event);
                crate::ListenerDirective::Keep
            }

            fn on_figure_event(&self, _event: FigureEvent) -> crate::ListenerDirective {
                crate::ListenerDirective::Keep
            }

            fn on_notify(&self, _figure_id: FigureId) -> crate::ListenerDirective {
                crate::ListenerDirective::Keep
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::empty();
        runtime.add_update_listener(Box::new(CaptureUpdates(events.clone())));
        runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        runtime
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .expect("frame completion");

        let events = events.lock().unwrap();
        let validated = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Validated))
            .unwrap();
        let painted = events
            .iter()
            .position(|event| matches!(event, UpdateEvent::Painted { .. }))
            .unwrap();
        let prepared = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Prepared { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        let submitted = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    UpdateEvent::Submitted { frame_id, .. } if *frame_id == submission.frame_id
                )
            })
            .unwrap();
        assert!(validated < painted);
        assert!(painted < prepared);
        assert!(prepared < submitted);
    }

    #[test]
    fn pending_mutations_are_applied_before_validation_and_recording() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
        ));
        let mut runtime = Runtime::new(tree);
        runtime
            .mutations
            .enqueue(crate::runtime::mutation::PendingMutation::add_child_figure(
                root,
                Box::new(
                    RectangleFigure::new(10.0, 10.0, 20.0, 20.0).expect("valid figure bounds"),
                ),
            ));

        let submission = runtime
            .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        assert_eq!(runtime.tree.child_order(root).unwrap().len(), 1);
        assert!(runtime.tree.is_valid(root).unwrap());
        assert!(!submission.commands.is_empty());
    }
}

#[cfg(test)]
mod adr014_tests {
    use super::*;
    use crate::{ChopboxAnchor, DirectRouter, ImageFigure, RectangleFigure, ResourceStatus};
    use std::{cell::Cell, rc::Rc};

    fn scene() -> (Runtime, FigureId) {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        (runtime, root)
    }

    #[test]
    fn scoped_editor_rejects_an_internally_detached_figure() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).expect("valid figure bounds"),
            ))
            .expect("valid FigureTree construction");
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid FigureTree construction");
        let mut updates = UpdateManager::with_namespace(tree.namespace());
        assert!(tree.remove_child(&mut updates, root, child));
        let mut runtime = Runtime::new(tree);

        assert!(matches!(
            runtime.figure(child),
            Err(RuntimeMutationError::DetachedFigure(id)) if id == child
        ));
    }

    #[test]
    fn foreign_figure_does_not_read_or_modify_matching_local_slot() {
        let (_first, foreign) = scene();
        let (mut second, local) = scene();
        let before = second.tree().figure_bounds(local);
        assert_eq!(
            second.tree().figure_bounds(foreign),
            Err(crate::tree::TreeQueryError::ForeignFigure(foreign))
        );
        assert_eq!(
            second.set_bounds(foreign, Rectangle::new(1.0, 2.0, 3.0, 4.0)),
            Err(RuntimeMutationError::ForeignRuntime(foreign))
        );
        assert_eq!(second.tree().figure_bounds(local), before);
    }

    #[test]
    fn foreign_registry_handles_do_not_remove_local_entries() {
        let (mut first, first_root) = scene();
        let (mut second, second_root) = scene();
        let foreign_anchor =
            first.register_connection_anchor(Box::new(ChopboxAnchor::new(first_root)));
        let local_anchor =
            second.register_connection_anchor(Box::new(ChopboxAnchor::new(second_root)));
        let foreign_router = first.register_connection_router(Box::new(DirectRouter));
        let local_router = second.register_connection_router(Box::new(DirectRouter));
        assert!(second.remove_connection_anchor(foreign_anchor).is_err());
        assert!(second.remove_connection_router(foreign_router).is_err());
        assert!(second.remove_connection_anchor(local_anchor).is_ok());
        assert!(second.remove_connection_router(local_router).is_ok());
    }

    #[test]
    fn foreign_listener_does_not_unregister_local_subscription() {
        let (mut first, foreign_owner) = scene();
        let (mut second, _) = scene();
        let foreign = first.add_update_listener(Box::new(()));
        let local = second.add_update_listener(Box::new(()));
        assert!(matches!(
            second.add_update_listener_scoped(
                ListenerScope::Figure(foreign_owner),
                Box::new(())
            ),
            Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign_owner
        ));
        assert!(!second.remove_listener(foreign));
        assert!(second.remove_listener(local));
        assert!(!second.remove_listener(local));
    }

    struct DropProbe(Rc<Cell<usize>>);

    impl Figure for DropProbe {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "DropProbe"
        }
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn remove_releases_subtree_and_invalidates_old_ids() {
        let (mut runtime, root) = scene();
        let drops = Rc::new(Cell::new(0));
        let child = runtime
            .add_figure(root, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        let grandchild = runtime
            .add_figure(child, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        assert!(
            runtime
                .remove_figure(root, child)
                .expect("valid Runtime mutation")
        );
        assert_eq!(drops.get(), 2);
        assert!(runtime.tree().node(child).is_err());
        assert!(runtime.tree().node(grandchild).is_err());
        assert_eq!(
            runtime.remove_figure(root, child),
            Err(RuntimeMutationError::UnknownOrDisposedFigure(child))
        );
    }

    #[test]
    fn dispose_supports_the_maximum_tree_depth() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds"),
        ));
        let mut deepest = root;
        for _ in 1..crate::MAX_TREE_DEPTH {
            deepest = tree.add_child_to(
                deepest,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds")),
            );
        }
        assert_eq!(tree.depth(deepest), Ok(crate::MAX_TREE_DEPTH));

        let mut runtime = Runtime::new(tree);
        runtime
            .set_preferred_size(deepest, Dimension::new(2.0, 2.0))
            .unwrap();
        let initial = runtime
            .prepare_frame()
            .expect("deep tree validation and render must complete");
        assert!(!initial.commands().is_empty());
        assert_eq!(runtime.tree().hit_test_simple((0.5, 0.5)), Some(deepest));
        assert!(
            runtime
                .set_bounds(deepest, Rectangle::new(0.0, 0.0, 0.75, 0.75))
                .expect("valid Runtime mutation")
        );
        assert!(
            runtime.prepare_frame().is_some(),
            "deep mutation must complete validation and render"
        );
        runtime.dispose_subtree(root).unwrap();

        assert!(runtime.tree().contents().is_none());
        assert!(runtime.tree().node(root).is_err());
        assert!(runtime.tree().node(deepest).is_err());
    }

    #[test]
    fn contents_replacement_releases_old_tree_and_preserves_namespace() {
        let (mut runtime, root) = scene();
        let drops = Rc::new(Cell::new(0));
        runtime
            .add_figure(root, Box::new(DropProbe(Rc::clone(&drops))))
            .expect("valid Runtime mutation");
        let replacement = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 40.0, 40.0).expect("valid figure bounds"),
            ))
            .expect("valid Runtime mutation");
        assert_eq!(drops.get(), 1);
        assert!(runtime.tree().node(root).is_err());
        assert_ne!(root, replacement);
        assert_eq!(replacement.namespace(), root.namespace());
        assert_eq!(
            runtime.resources().registry().namespace(),
            root.namespace().as_uuid()
        );
        assert_eq!(
            runtime.backend_session_id().runtime_namespace(),
            root.namespace().as_uuid()
        );
    }

    #[test]
    fn disposal_removes_owned_listener_but_preserves_shared_registrations() {
        let (mut runtime, root) = scene();
        let owner = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        let owned = runtime
            .add_update_listener_scoped(ListenerScope::Figure(owner), Box::new(()))
            .unwrap();
        let global = runtime.add_update_listener(Box::new(()));
        let router = runtime.register_connection_router(Box::new(DirectRouter));
        let image = runtime.register_image().expect("image registration");
        runtime
            .add_resource_dependency(image.resource_id(), owner)
            .unwrap();
        runtime.dispose_subtree(owner).unwrap();
        assert!(!runtime.remove_listener(owned));
        assert!(runtime.remove_listener(global));
        assert!(runtime.remove_connection_router(router).is_ok());
        assert!(matches!(
            runtime.resource_status(image.resource_id()),
            Ok(ResourceStatus::Pending)
        ));
    }

    struct PanicOnDetach;
    struct PanicOnAttachLayer;

    impl Figure for PanicOnDetach {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }
        fn name(&self) -> &'static str {
            "PanicOnDetach"
        }
        fn register_capabilities(
            &self,
            out: &mut crate::FigureCapabilityBuilder,
        ) -> Result<(), crate::FigureCapabilityRegistrationError> {
            out.register(crate::LIFECYCLE, crate::LifecycleCapability::of::<Self>())
        }
    }

    impl crate::FigureLifecycle for PanicOnDetach {
        fn on_detached(&mut self, _context: crate::FigureLifecycleContext) {
            panic!("lifecycle fault probe");
        }
    }

    impl Figure for PanicOnAttachLayer {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "PanicOnAttachLayer"
        }

        fn register_capabilities(
            &self,
            out: &mut crate::FigureCapabilityBuilder,
        ) -> Result<(), crate::FigureCapabilityRegistrationError> {
            out.register(crate::LIFECYCLE, crate::LifecycleCapability::of::<Self>())?;
            out.register(crate::LAYER, crate::LayerCapability)
        }
    }

    impl crate::FigureLifecycle for PanicOnAttachLayer {
        fn on_attached(&mut self, _context: crate::FigureLifecycleContext) {
            panic!("layer lifecycle fault probe");
        }
    }

    #[test]
    fn lifecycle_panic_happens_after_extraction_and_blocks_recording() {
        let (mut runtime, root) = scene();
        let child = runtime
            .add_figure(root, Box::new(PanicOnDetach))
            .expect("valid Runtime mutation");
        let source = runtime.scenes().register_source(child).unwrap();
        let (_, _, handoff) = runtime
            .scenes()
            .subscribe(
                source,
                crate::scene::SOURCE_LOCAL_CAPTURE,
                crate::scene::SourceLocalCapture::default(),
            )
            .unwrap();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.dispose_subtree(child).unwrap();
        }));
        assert!(failure.is_err());
        assert!(runtime.is_faulted());
        assert!(runtime.tree().node(child).is_err());
        assert_eq!(runtime.tree().child_order(root), Ok(vec![]));
        assert!(matches!(
            handoff.take().unwrap(),
            Some(crate::scene::SceneEvent::Closed(
                crate::scene::SceneClosed {
                    reason: crate::scene::SceneCloseReason::SourceDisposed,
                    ..
                }
            ))
        ));
        assert!(runtime.prepare_frame().is_none());
        assert!(matches!(
            runtime.add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds"))
            ),
            Err(RuntimeMutationError::Faulted)
        ));
        assert!(matches!(
            runtime.reparent(root, root),
            Err(RuntimeMutationError::Faulted)
        ));
        assert!(matches!(
            runtime.set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds")
            )),
            Err(RuntimeMutationError::Faulted)
        ));
    }

    #[test]
    fn subtree_attachment_fault_keeps_all_published_resource_dependencies() {
        let (mut runtime, root) = scene();
        let image = runtime.resources().register_image().unwrap();
        let mut subtree = crate::tree::FigureSubtree::new(Box::new(PanicOnAttachLayer));
        subtree
            .add(subtree.root(), Box::new(ImageFigure::new(image)))
            .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime.add_subtree(root, subtree)
            }))
            .is_err()
        );
        assert!(runtime.is_faulted());
        let ids = runtime.tree.descendant_ids(root).unwrap();
        assert_eq!(ids.len(), 2);
        // Inspect the registry directly: public mutation correctly rejects a faulted Runtime.
        assert_eq!(
            runtime.resources.remove(image.resource_id()).unwrap(),
            vec![ids[1]]
        );
    }

    #[test]
    fn contents_retirement_fault_keeps_new_scene_resource_dependencies() {
        let (mut runtime, root) = scene();
        runtime.add_figure(root, Box::new(PanicOnDetach)).unwrap();
        let image = runtime.resources().register_image().unwrap();
        let mut subtree = crate::tree::FigureSubtree::new(Box::new(
            RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
        ));
        subtree
            .add(subtree.root(), Box::new(ImageFigure::new(image)))
            .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime.set_contents_subtree(subtree)
            }))
            .is_err()
        );
        assert!(runtime.is_faulted());
        let new = runtime.tree.contents().unwrap();
        assert_ne!(new, root);
        let child = runtime.tree.child_order(new).unwrap()[0];
        assert_eq!(
            runtime.resources.remove(image.resource_id()).unwrap(),
            vec![child]
        );
    }

    #[test]
    fn child_reorder_checks_identity_relation_and_index_before_noop() {
        let (mut runtime, root) = scene();
        let first = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0).expect("valid figure bounds")),
            )
            .unwrap();
        let second = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(10.0, 0.0, 5.0, 5.0).expect("valid figure bounds")),
            )
            .unwrap();
        runtime.record_full_frame();
        let frozen = runtime.updates.frozen_surface_regions.clone();
        let dirty = runtime.updates.dirty_regions.clone();
        assert_eq!(runtime.move_child_to_index(root, first, 0), Ok(false));
        assert_eq!(runtime.updates.frozen_surface_regions, frozen);
        assert_eq!(runtime.updates.dirty_regions, dirty);
        assert_eq!(
            runtime.move_child_to_index(first, second, 0),
            Err(RuntimeMutationError::InvalidParentRelation {
                parent: first,
                child: second
            })
        );
        assert_eq!(
            runtime.move_child_to_index(root, first, 2),
            Err(RuntimeMutationError::InvalidChildIndex {
                parent: root,
                index: 2,
                child_count: 2
            })
        );
        let (_, foreign) = scene();
        assert_eq!(
            runtime.move_child_to_index(root, foreign, 0),
            Err(RuntimeMutationError::ForeignRuntime(foreign))
        );
        assert_eq!(runtime.tree.child_order(root), Ok(vec![first, second]));
        assert_eq!(runtime.updates.frozen_surface_regions, frozen);
        assert_eq!(runtime.updates.dirty_regions, dirty);
        assert_eq!(runtime.move_child_to_index(root, first, 1), Ok(true));
        assert_eq!(runtime.tree.child_order(root), Ok(vec![second, first]));
        assert_eq!(runtime.move_child_to_index(root, first, 1), Ok(false));
        runtime.dispose_subtree(first).unwrap();
        assert_eq!(
            runtime.move_child_to_index(root, first, 0),
            Err(RuntimeMutationError::UnknownOrDisposedFigure(first))
        );
    }

    #[test]
    fn layered_lifecycle_panic_faults_runtime_after_structural_commit() {
        let (mut runtime, root) = scene();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let pane = runtime
                .layers()
                .add_pane(root, Rectangle::new(0.0, 0.0, 100.0, 100.0))
                .unwrap();
            runtime
                .layers()
                .add(
                    pane,
                    Box::new(PanicOnAttachLayer),
                    LayerKey::new("fault-probe").unwrap(),
                    LayerPlacement::Last,
                )
                .unwrap();
        }));
        assert!(failure.is_err());
        assert!(runtime.is_faulted());
        assert!(runtime.prepare_frame().is_none());
    }

    #[test]
    fn disposal_freezes_old_damage_before_the_node_is_removed() {
        let (mut runtime, root) = scene();
        let child = runtime
            .add_figure(
                root,
                Box::new(
                    RectangleFigure::new(20.0, 20.0, 10.0, 10.0).expect("valid figure bounds"),
                ),
            )
            .expect("valid Runtime mutation");
        runtime.record_full_frame();
        runtime.updates.dirty_regions.clear();
        runtime.dispose_subtree(child).unwrap();
        let frozen = runtime.updates.frozen_surface_regions.first().unwrap();
        assert!(frozen.x <= 20.0 && frozen.y <= 20.0);
        assert!(frozen.x + frozen.width >= 30.0 && frozen.y + frozen.height >= 30.0);
        assert!(runtime.tree().figure_bounds(child).is_err());
    }

    #[test]
    fn s06_figure_batch_preflight_preserves_topology_damage_and_capability_registration() {
        struct InvalidExternalFigure;
        impl Figure for InvalidExternalFigure {
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::new(0.0, 0.0, -1.0, 1.0)
            }
            fn name(&self) -> &'static str {
                "invalid external geometry"
            }
        }
        struct RegistrationProbe(Rc<Cell<usize>>);
        impl Figure for RegistrationProbe {
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::new(0.0, 0.0, 10.0, 10.0)
            }
            fn name(&self) -> &'static str {
                "registration probe"
            }
            fn register_capabilities(
                &self,
                _: &mut crate::FigureCapabilityBuilder,
            ) -> Result<(), crate::FigureCapabilityRegistrationError> {
                self.0.set(self.0.get() + 1);
                Ok(())
            }
        }
        let (mut runtime, root) = scene();
        runtime.record_full_frame();
        let before_children = runtime.tree.child_order(root);
        let before_damage = runtime.updates.dirty_regions.clone();
        let calls = Rc::new(Cell::new(0));
        let error = runtime
            .add_figures(vec![
                (root, Box::new(RegistrationProbe(Rc::clone(&calls))), None),
                (root, Box::new(InvalidExternalFigure), None),
            ])
            .unwrap_err();
        assert!(matches!(
            error,
            RuntimeMutationError::FigureBatch { index: 1, .. }
        ));
        assert_eq!(runtime.tree.child_order(root), before_children);
        assert_eq!(runtime.updates.dirty_regions, before_damage);
        assert!(!runtime.is_faulted());
        assert_eq!(calls.get(), 1);
        let ids = runtime
            .add_figures(vec![(
                root,
                Box::new(RegistrationProbe(Rc::clone(&calls))),
                None,
            )])
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(
            calls.get(),
            2,
            "descriptor registration runs once per input"
        );
    }

    #[test]
    fn resource_refusal_preserves_damage_notifications_and_dependency_registry() {
        let (mut runtime, root) = scene();
        runtime.record_full_frame();
        let image = runtime.resources().register_image().unwrap();
        let mut foreign = Runtime::empty();
        let foreign_image = foreign.resources().register_image().unwrap();
        let children = runtime.tree.child_order(root);
        let damage = runtime.updates.dirty_regions.clone();
        let notifications = runtime.updates.notification_effects().len();
        let result = runtime.add_figures(vec![
            (root, Box::new(ImageFigure::new(image)), None),
            (root, Box::new(ImageFigure::new(foreign_image)), None),
        ]);
        assert!(matches!(
            result,
            Err(RuntimeMutationError::FigureBatch { index: 1, .. })
        ));
        assert_eq!(runtime.tree.child_order(root), children);
        assert_eq!(runtime.updates.dirty_regions, damage);
        assert_eq!(runtime.updates.notification_effects().len(), notifications);
        assert!(
            runtime
                .resources
                .remove(image.resource_id())
                .unwrap()
                .is_empty()
        );
        assert!(!runtime.is_faulted());
    }

    #[test]
    fn layer_resource_refusal_preserves_damage_and_all_notification_queues() {
        struct ResourceLayer(crate::render::ResourceId);
        impl Figure for ResourceLayer {
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::ZERO
            }
            fn name(&self) -> &'static str {
                "ResourceLayer"
            }
            fn register_capabilities(
                &self,
                out: &mut crate::figure::FigureCapabilityBuilder,
            ) -> Result<(), crate::figure::FigureCapabilityRegistrationError> {
                out.register(crate::figure::LAYER, crate::figure::LayerCapability)?;
                out.register(
                    crate::figure::RESOURCE_DEPENDENCIES,
                    crate::figure::ResourceDependenciesCapability::of::<Self>(),
                )
            }
        }
        impl crate::figure::FigureResourceDependencies for ResourceLayer {
            fn resource_dependencies(&self) -> Vec<crate::render::ResourceId> {
                vec![self.0]
            }
        }
        let (mut runtime, root) = scene();
        let pane = runtime.layers().add_pane(root, Rectangle::ZERO).unwrap();
        runtime.record_full_frame();
        let mut foreign = Runtime::empty();
        let resource = foreign.resources().register_image().unwrap().resource_id();
        let children = runtime.tree.child_order(pane);
        let damage = runtime.updates.dirty_regions.clone();
        let tree_notifications = runtime.tree.notification_effects().len();
        let update_notifications = runtime.updates.notification_effects().len();
        assert_eq!(
            runtime.layers().add(
                pane,
                Box::new(ResourceLayer(resource)),
                LayerKey::new("rejected").unwrap(),
                LayerPlacement::Last,
            ),
            Err(LayerError::Resource(ResourceError::WrongNamespace)),
        );
        assert_eq!(runtime.tree.child_order(pane), children);
        assert_eq!(runtime.updates.dirty_regions, damage);
        assert_eq!(
            runtime.tree.notification_effects().len(),
            tree_notifications
        );
        assert_eq!(
            runtime.updates.notification_effects().len(),
            update_notifications
        );
        assert!(runtime.layers().ids(pane).unwrap().is_empty());
        assert!(!runtime.is_faulted());
    }

    #[test]
    fn subtree_binding_replacement_checks_the_complete_revision_budget_before_publication() {
        use crate::{
            connection::{ConnectionFigure, ConnectionId, RouterBinding},
            tree::{FigureSubtree, SubtreeBindingNodes, SubtreeConnection},
        };
        let (mut runtime, root) = scene();
        let figure = runtime
            .add_figure(root, Box::new(ConnectionFigure::new()))
            .unwrap();
        let router = RouterBinding::Explicit {
            router: runtime.connections.direct_router(),
        };
        let connection = ConnectionId::from_figure(figure);
        runtime
            .connections
            .register_connection(connection, None, None, router, None)
            .unwrap();
        runtime
            .connections
            .set_dirty_revision_for_test(connection, u64::MAX - 2);
        let before = runtime.connections.state(connection).unwrap();
        let mut subtree = FigureSubtree::new(Box::new(ConnectionFigure::new()));
        subtree
            .set_connection_binding(subtree.root(), move |_: &SubtreeBindingNodes| {
                Ok(SubtreeConnection {
                    source: None,
                    target: None,
                    router,
                    constraint: None,
                })
            })
            .unwrap();
        assert!(matches!(
            runtime.set_contents_subtree(subtree),
            Err(RuntimeMutationError::Connection(
                ConnectionRuntimeError::GenerationExhausted
            ))
        ));
        assert_eq!(runtime.tree.contents(), Some(root));
        assert_eq!(runtime.connections.state(connection).unwrap(), before);
        assert!(!runtime.is_faulted());
    }

    #[test]
    fn s06_connection_admission_and_disposal_refuse_before_revision_overflow_writes() {
        use crate::connection::{ConnectionFigure, ConnectionId, RouterBinding};
        let (mut runtime, root) = scene();
        let existing = runtime
            .add_figure(root, Box::new(ConnectionFigure::new()))
            .unwrap();
        let candidate = runtime
            .add_figure(root, Box::new(ConnectionFigure::new()))
            .unwrap();
        let router = RouterBinding::Explicit {
            router: runtime.connections.direct_router(),
        };
        runtime
            .connections
            .register_connection(
                ConnectionId::from_figure(existing),
                None,
                None,
                router,
                None,
            )
            .unwrap();
        runtime.record_full_frame();
        runtime
            .connections
            .exhaust_dirty_revision_for_test(ConnectionId::from_figure(existing));
        let before_children = runtime.tree.child_order(root);
        let before_dirty = runtime.updates.dirty_regions.clone();
        let before_frozen = runtime.updates.frozen_surface_regions.clone();
        let before_state = runtime
            .connections
            .state(ConnectionId::from_figure(existing))
            .unwrap();
        assert!(
            runtime
                .connections
                .register_connection(
                    ConnectionId::from_figure(candidate),
                    None,
                    None,
                    router,
                    None,
                )
                .is_err()
        );
        assert!(
            runtime
                .connections
                .state(ConnectionId::from_figure(candidate))
                .is_err()
        );
        assert_eq!(
            runtime
                .connections
                .state(ConnectionId::from_figure(existing))
                .unwrap(),
            before_state
        );
        assert!(runtime.dispose_subtree(candidate).is_err());
        assert!(!runtime.is_faulted());
        assert_eq!(runtime.tree.is_attached(candidate), Ok(true));
        assert_eq!(runtime.tree.child_order(root), before_children);
        assert_eq!(runtime.updates.dirty_regions, before_dirty);
        assert_eq!(runtime.updates.frozen_surface_regions, before_frozen);
    }

    #[test]
    fn s06_late_disposal_prepare_rejection_keeps_retired_prefix_and_live_suffix() {
        use crate::connection::{ConnectionFigure, ConnectionId, RouterBinding};
        let (mut runtime, root) = scene();
        let connection = runtime
            .add_figure(root, Box::new(ConnectionFigure::new()))
            .unwrap();
        let first = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0).expect("valid figure bounds")),
            )
            .unwrap();
        let second = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(10.0, 0.0, 5.0, 5.0).expect("valid figure bounds")),
            )
            .unwrap();
        let connection_id = ConnectionId::from_figure(connection);
        runtime
            .connections
            .register_connection(
                connection_id,
                None,
                None,
                RouterBinding::Explicit {
                    router: runtime.connections.direct_router(),
                },
                None,
            )
            .unwrap();
        runtime.record_full_frame();
        runtime
            .connections
            .set_dirty_revision_for_test(connection_id, u64::MAX - 1);
        runtime.dispose_subtree(first).unwrap();
        assert_eq!(
            runtime.tree.is_attached(first),
            Err(crate::tree::TreeQueryError::UnknownFigure(first))
        );
        let before_children = runtime.tree.child_order(root);
        let before_dirty = runtime.updates.dirty_regions.clone();
        let before_frozen = runtime.updates.frozen_surface_regions.clone();
        let before_state = runtime.connections.state(connection_id).unwrap();
        assert_eq!(
            runtime.dispose_subtree(second),
            Err(RuntimeMutationError::Rejected)
        );
        assert_eq!(runtime.tree.is_attached(second), Ok(true));
        assert_eq!(runtime.tree.is_attached(connection), Ok(true));
        assert!(!runtime.is_faulted());
        assert_eq!(runtime.tree.child_order(root), before_children);
        assert_eq!(runtime.updates.dirty_regions, before_dirty);
        assert_eq!(runtime.updates.frozen_surface_regions, before_frozen);
        assert_eq!(
            runtime.connections.state(connection_id).unwrap(),
            before_state
        );
        // All disposals consult the same exhausted revision, even independent Figures.
        // The valid prefix stays retired; no second structural commit has occurred.
        assert_eq!(
            runtime.dispose_subtree(connection),
            Err(RuntimeMutationError::Rejected)
        );
        assert_eq!(
            runtime.tree.is_attached(first),
            Err(crate::tree::TreeQueryError::UnknownFigure(first))
        );
    }

    #[test]
    fn s06_explicit_cleanup_keeps_real_notifications_damage_and_retires_subscriptions() {
        use std::sync::{Arc, Mutex};
        struct Capture(Arc<Mutex<Vec<crate::AncestorEvent>>>);
        impl crate::AncestorListener for Capture {
            fn ancestor_changed(&self, event: crate::AncestorEvent) -> crate::ListenerDirective {
                self.0.lock().unwrap().push(event);
                crate::ListenerDirective::Keep
            }
        }
        let (mut runtime, root) = scene();
        runtime.record_full_frame();
        let events = Arc::new(Mutex::new(Vec::new()));
        runtime.add_ancestor_listener(Box::new(Capture(Arc::clone(&events))));
        let figure = runtime
            .add_figure(
                root,
                Box::new(
                    RectangleFigure::new(20.0, 30.0, 10.0, 10.0).expect("valid figure bounds"),
                ),
            )
            .unwrap();
        let scoped = Arc::new(Mutex::new(Vec::new()));
        let subscription = runtime
            .add_ancestor_listener_scoped(
                crate::ListenerScope::Figure(figure),
                Box::new(Capture(Arc::clone(&scoped))),
            )
            .unwrap();
        runtime.dispose_subtree(figure).unwrap();
        assert!(!runtime.remove_listener(subscription));
        assert_eq!(
            runtime.tree.is_attached(figure),
            Err(crate::tree::TreeQueryError::UnknownFigure(figure))
        );
        assert!(
            runtime
                .updates
                .frozen_surface_regions
                .iter()
                .any(|damage| damage.x <= 20.0
                    && damage.y <= 30.0
                    && damage.x + damage.width >= 30.0
                    && damage.y + damage.height >= 40.0)
        );
        runtime.record_full_frame();
        assert!(scoped.lock().unwrap().is_empty());
        let events: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.figure_id == figure)
            .map(|event| event.kind)
            .collect();
        assert_eq!(
            events,
            vec![
                crate::AncestorEventKind::Added,
                crate::AncestorEventKind::Removed
            ]
        );
        assert!(runtime.dispose_subtree(figure).is_err());
    }

    #[test]
    fn reparent_preserves_identity_and_foreign_checked_mutation_is_rejected() {
        let (mut runtime, root) = scene();
        let left = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        let right = runtime
            .add_figure(
                root,
                Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        let child = runtime
            .add_figure(
                left,
                Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0).expect("valid figure bounds")),
            )
            .expect("valid Runtime mutation");
        assert!(
            runtime
                .reparent(child, right)
                .expect("valid Runtime mutation")
        );
        assert_eq!(runtime.tree().parent_id(child), Ok(Some(right)));
        let (_, foreign) = scene();
        assert!(matches!(runtime.clear_preferred_size(foreign),
            Err(RuntimeMutationError::ForeignRuntime(id)) if id == foreign));
    }

    #[test]
    fn synthetic_root_is_rejected_by_public_lifecycle_mutations() {
        let (mut runtime, root) = scene();
        let synthetic_root = runtime.tree.synthetic_root();

        assert_eq!(
            runtime.dispose_subtree(synthetic_root),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
        assert_eq!(
            runtime.add_figure(
                synthetic_root,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).expect("valid figure bounds"))
            ),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
        assert_eq!(
            runtime.reparent(root, synthetic_root),
            Err(RuntimeMutationError::SyntheticRootOperation(synthetic_root))
        );
    }

    #[test]
    fn moving_runtime_preserves_registry_identity_and_state() {
        let (mut runtime, _) = scene();
        let image = runtime.register_image().expect("image registration");
        let listener = runtime.add_update_listener(Box::new(()));
        let router = runtime.register_connection_router(Box::new(DirectRouter));
        let session = runtime.backend_session_id();

        let mut moved = runtime;

        assert_eq!(moved.backend_session_id(), session);
        assert_eq!(
            moved.resource_status(image.resource_id()),
            Ok(&ResourceStatus::Pending)
        );
        assert!(moved.remove_listener(listener));
        assert!(moved.remove_connection_router(router).is_ok());
    }
}
