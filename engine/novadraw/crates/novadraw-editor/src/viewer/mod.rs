//! Viewer contents, registries, and model-to-Figure projection.

mod failure;
mod projection;
pub use failure::{ProjectionCleanup, ProjectionCleanupFailure};

use std::{
    any::Any,
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::Arc,
};

use novadraw::connection::{
    AnchorId, AnchorSemanticKey, ChopboxAnchor, ConnectionAnchor, ConnectionId,
    ConnectionLayerFigure, ConnectionRouter, ConnectionRuntimeError, ConnectionStateSnapshot,
    CoordinateSpace, RouterBinding, RouterId, UnresolvedConnection, XYAnchor,
};
use novadraw::container::{
    FreeformLayerFigure, FreeformLayeredPane, LayerError, LayerFigure, LayerKey, LayerPlacement,
    LayeredPane, MouseLocationZoomScrollPolicy, RangeModelSnapshot, ScalableFreeformLayeredPane,
    ViewportHandle, ZoomError, ZoomManager, ZoomScrollPolicy,
};
use novadraw::event::{
    DispatchOutcome, FocusChange, KeyModifiers, MonotonicTime, MouseButton, TimeError,
};
use novadraw::figure::{
    Border, Figure, FigureStyle, FlowTextPosition, FlowTextRange, RectangleFigure,
    ShapeMutationError, TextFlowFigure, TextFlowViewport,
};
use novadraw::geometry::{ApproxEq, Point, Precision, Rectangle, Translatable};
use novadraw::graphics::Color;
use novadraw::layout::StackLayout;
use novadraw::render::{
    BackendCapabilities, BackendSessionId, CaretGeometry, FrameId, RenderResult, SelectionQuad,
    SurfaceInfo, TextLayout,
};
use novadraw::runtime::{
    BackendSessionError, ComponentUpdateError, ComponentUpdateReceipt, FigureComponentContext,
    FigureComponentUpdate, FontId, FramePreparation, FramePreparationError,
    LogicalViewportResizeError, PreparedFigureUpdate, ReplaceFigureBorder, ResourceError, Runtime,
    RuntimeMutationError, SubmissionAcknowledgement, SubmissionCompletionError, TextFlowQueryError,
};
use novadraw::text::TextMovement;
use novadraw::tree::{FigureId, FigureTree};

use crate::{
    BendpointCreateAction, BendpointHandleSite, BendpointMoveAction, Command, CompoundCommand,
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionCreation, ConnectionEndpoint,
    ConnectionFeedbackRoute, ConnectionPartFactoryContext, ConnectionPartId,
    ConnectionReconnection, ConnectionRouterKey, ConnectionRouterSelection,
    ConnectionRoutingDescriptor, CreateConnection, CreateConnectionRequest, DirectTextEditError,
    DirectTextEditRequest, DirectTextEditSessionId, DirectTextEditState, DirectTextFeedback,
    EditPartError, EditPartFactory, EditPartId, EditingContribution, EditorNamespace, FeedbackId,
    FeedbackVisual, HandleAction, HandleActionProtocol, HandleId, ModelAdapter, ModelConnection,
    ModelRevision, PartFactoryContext, PartKind, PartTree, PartTreeError, PolicyError, PolicyHost,
    ReconnectConnection, ReconnectConnectionRequest, RequestMetadata, RequestProtocol,
    RequestResolution, SelectionAction, SelectionDelta, SelectionModel, StartDirectTextEdit,
    TextEditMode, TextInputEffect, TextInputPurpose, TextInputSnapshot, VisualBuildContext,
    VisualOwner, VisualUpdateContext,
    direct_edit::{
        ActiveDirectTextEdit, PreparedDirectTextEdit, position_from_offset, position_to_offset,
    },
    part::{BehaviorStore, validate_runtime_namespace},
    policy::PolicyStore,
    request::ErasedRequestKey,
    tool::{ErasedRequestContributions, ToolHost},
};

const MAX_PART_TREE_DEPTH: usize = 10_000;
const RECURSIVE_STACK_CHECK_INTERVAL: usize = 16;
const RECURSIVE_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_STACK_GROWTH: usize = 4 * 1024 * 1024;
const VIEWPORT_LAYER: &str = "viewport";
const GRID_LAYER: &str = "grid";
const PRINTABLE_LAYERS: &str = "printable";
const PRIMARY_LAYER: &str = "primary";
const CONNECTION_LAYER: &str = "connection";
const SCALED_FEEDBACK_LAYER: &str = "scaled-feedback";
const FEEDBACK_LAYER: &str = "feedback";
const HANDLE_LAYER: &str = "handles";
const DIRECT_TEXT_SELECTION_COLOR: Color = Color::rgba(0.18, 0.49, 0.89, 0.28);
const DIRECT_TEXT_CARET_COLOR: Color = Color::rgba(0.07, 0.09, 0.12, 1.0);
const DIRECT_TEXT_CARET_MINIMUM_WIDTH: f64 = 1.5;
const DIRECT_TEXT_PREEDIT_COLOR: Color = Color::rgba(0.12, 0.38, 0.78, 1.0);
const DIRECT_TEXT_PREEDIT_THICKNESS: f64 = 1.5;
const DIRECT_TEXT_CARET_BLINK_INTERVAL_MICROS: u64 = 530_000;
const DIRECT_TEXT_REVEAL_MARGIN: f64 = 2.0;

fn reveal_horizontal_caret(
    previous: f64,
    natural_caret: Rectangle,
    viewport_width: f64,
    content_width: f64,
) -> f64 {
    let maximum = (content_width - viewport_width).max(0.0);
    if maximum == 0.0 {
        return 0.0;
    }
    let margin = DIRECT_TEXT_REVEAL_MARGIN.min(viewport_width / 2.0);
    let mut scroll = previous.clamp(0.0, maximum);
    let visible_left = natural_caret.x - scroll;
    let visible_right = visible_left + natural_caret.width.max(DIRECT_TEXT_CARET_MINIMUM_WIDTH);
    if visible_left < margin {
        scroll = natural_caret.x - margin;
    } else if visible_right > viewport_width - margin {
        scroll += visible_right - (viewport_width - margin);
    }
    scroll.clamp(0.0, maximum)
}

impl<A, F> ToolHost<A> for GraphicalViewer<A, F>
where
    A: ModelAdapter + 'static,
    F: EditPartFactory<A>,
{
    fn dispatch_request_erased(
        &mut self,
        key: ErasedRequestKey,
        metadata: &RequestMetadata,
        payload: &dyn Any,
    ) -> Result<ErasedRequestContributions, ViewerError> {
        self.with_extension_fault_boundary(|viewer| {
            viewer.dispatch_request_erased_inner(key, metadata, payload)
        })
    }

    fn attach_feedback(
        &mut self,
        contributions: Vec<(EditPartId, FeedbackVisual)>,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.add_feedback_batch(
            contributions
                .into_iter()
                .map(|(owner, feedback)| (Some(owner), feedback))
                .collect(),
        )
    }

    fn dispatch_pressed(
        &mut self,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
        update_selection: bool,
    ) -> Result<ViewerInputOutcome, ViewerError> {
        if update_selection {
            self.dispatch_mouse_pressed(location.x(), location.y(), button, modifiers)
        } else {
            Ok(self.dispatch_mouse_pressed_without_selection(location.x(), location.y(), button))
        }
    }

    fn dispatch_moved(&mut self, location: Point) -> DispatchOutcome {
        self.dispatch_mouse_moved(location.x(), location.y())
    }

    fn dispatch_released(&mut self, location: Point, button: MouseButton) -> DispatchOutcome {
        self.dispatch_mouse_released(location.x(), location.y(), button)
    }

    fn target_at(&self, location: Point) -> ViewerTarget {
        self.target_at(location.x(), location.y())
    }

    fn selection(&self) -> &[EditPartId] {
        self.selection().items()
    }

    fn contents(&self) -> EditPartId {
        self.contents()
    }

    fn parts_enclosed_by(&self, bounds: Rectangle) -> Vec<EditPartId> {
        self.parts_enclosed_by(bounds)
    }

    fn set_selection(&mut self, parts: &[EditPartId]) -> Result<(), ViewerError> {
        self.set_selection(parts)
    }

    fn model_point_from_surface(&self, location: Point) -> Result<Point, ViewerError> {
        self.model_point_from_surface(location)
    }

    fn surface_point_from_model(&self, location: Point) -> Result<Point, ViewerError> {
        let point = self
            .runtime
            .tree()
            .child_content_to_surface_transform(self.root_layers.scalable())
            .map_err(ViewerError::Coordinate)?
            .transform_point(location);
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(ViewerError::InconsistentState);
        }
        Ok(point)
    }

    fn connection_point_from_surface(
        &self,
        connection: ConnectionPartId,
        location: Point,
    ) -> Result<Point, ViewerError> {
        self.connection_point_from_surface(connection, location)
    }

    fn connection_routing_point_from_surface(&self, location: Point) -> Result<Point, ViewerError> {
        self.connection_routing_point_from_surface(location)
    }

    fn as_connection_part(&self, part: EditPartId) -> Option<ConnectionPartId> {
        self.as_connection_part(part)
    }

    fn replace_selection(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        self.replace_selection(part).map(|_| ())
    }

    fn set_focus(&mut self, part: Option<EditPartId>) -> Result<(), ViewerError> {
        self.set_focus(part).map(|_| ())
    }

    fn remove_overlay_visuals(&mut self, figures: &[FigureId]) -> Result<(), ViewerError> {
        self.remove_overlay_visuals(figures)
    }

    fn start_connection_creation(
        &mut self,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ViewerError> {
        self.start_connection_creation(request)
    }

    fn show_connection_feedback(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.show_connection_feedback(plan, request)
    }

    fn connection_command(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        self.connection_command(plan, request)
    }

    fn start_connection_reconnection(
        &mut self,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionReconnection<A>>>, ViewerError> {
        self.start_connection_reconnection(request)
    }

    fn show_reconnection_feedback(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.show_reconnection_feedback(plan, request)
    }

    fn reconnection_command(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        self.reconnection_command(plan, request)
    }

    fn add_autoexpose_range_reserve(
        &mut self,
        location: Point,
        feedback: &mut Vec<FigureId>,
    ) -> Result<(), ViewerError> {
        crate::autoexpose::add_range_reserve(self, location, feedback)
    }
}

type FeedbackInstallInput = (
    Option<EditPartId>,
    FigureId,
    Box<dyn Figure>,
    Option<FigureStyle>,
);

struct AttachedDirectTextFeedback {
    figures: Vec<FigureId>,
    text_feedback: FigureId,
    caret_feedback: Option<FigureId>,
    area: Rectangle,
    horizontal_scroll: f64,
}

struct SetDirectTextViewport(TextFlowViewport);

impl FigureComponentUpdate for SetDirectTextViewport {
    type Figure = TextFlowFigure;
    type Prepared = TextFlowViewport;
    type Error = &'static str;

    fn prepare(
        self,
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let offset = self.0.content_offset();
        if !offset.x().is_finite() || !offset.y().is_finite() {
            return Err("direct-edit TextFlow viewport offset must be finite");
        }
        Ok(PreparedFigureUpdate::paint(self.0))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_viewport(prepared);
    }
}

fn direct_text_viewport_error(error: ComponentUpdateError<&'static str>) -> ViewerError {
    match error {
        ComponentUpdateError::Runtime(error) | ComponentUpdateError::Effect(error) => error.into(),
        ComponentUpdateError::WrongFigureType { .. }
        | ComponentUpdateError::RevisionExhausted(_)
        | ComponentUpdateError::Rejected(_) => DirectTextEditError::InvalidFeedbackTarget.into(),
    }
}

/// Stable Figure identities for the standard graphical Viewer layer topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootLayers {
    root: FigureId,
    viewport_layer: FigureId,
    viewport: FigureId,
    scalable: FigureId,
    grid: FigureId,
    printable: FigureId,
    primary: FigureId,
    connection: FigureId,
    scaled_feedback: FigureId,
    feedback: FigureId,
    handles: FigureId,
}

impl RootLayers {
    /// Returns the root layered pane.
    pub const fn root(self) -> FigureId {
        self.root
    }

    /// Returns the root layer that clips and positions the Viewport.
    pub const fn viewport_layer(self) -> FigureId {
        self.viewport_layer
    }

    /// Returns the Viewport containing all model-scaled layers.
    pub const fn viewport(self) -> FigureId {
        self.viewport
    }

    /// Returns the scalable layered pane.
    pub const fn scalable(self) -> FigureId {
        self.scalable
    }

    /// Returns the grid layer.
    pub const fn grid(self) -> FigureId {
        self.grid
    }

    /// Returns the printable layered pane.
    pub const fn printable(self) -> FigureId {
        self.printable
    }

    /// Returns the primary model content layer.
    pub const fn primary(self) -> FigureId {
        self.primary
    }

    /// Returns the connection layer.
    pub const fn connection(self) -> FigureId {
        self.connection
    }

    /// Returns feedback that scales with model content.
    pub const fn scaled_feedback(self) -> FigureId {
        self.scaled_feedback
    }

    /// Returns feedback fixed in the surface coordinate domain.
    pub const fn feedback(self) -> FigureId {
        self.feedback
    }

    /// Returns the topmost interactive handle layer.
    pub const fn handles(self) -> FigureId {
        self.handles
    }
}

/// Result of resolving a surface point through the Viewer visual registry.
#[derive(Clone, Debug)]
pub enum ViewerTarget {
    /// The point resolves to an ordinary model EditPart.
    Part(EditPartId),
    /// The point resolves to an interactive handle.
    Handle {
        /// Handle identity.
        id: HandleId,
        /// EditPart manipulated by the handle.
        owner: EditPartId,
        /// Typed interaction represented by the handle.
        action: HandleAction,
    },
    /// No selectable visual was hit; the Viewer contents is the semantic fallback.
    Contents(EditPartId),
}

impl PartialEq for ViewerTarget {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Part(left), Self::Part(right))
            | (Self::Contents(left), Self::Contents(right)) => left == right,
            (
                Self::Handle {
                    id: left_id,
                    owner: left_owner,
                    action: left_action,
                },
                Self::Handle {
                    id: right_id,
                    owner: right_owner,
                    action: right_action,
                },
            ) => {
                left_id == right_id
                    && left_owner == right_owner
                    && left_action.name() == right_action.name()
            }
            _ => false,
        }
    }
}

/// Selection operation selected from pointer modifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionMode {
    /// Replace the visual selection.
    Replace,
    /// Add the target and make it primary.
    Append,
    /// Add an absent target or remove a selected target.
    Toggle,
}

/// Combined Figure dispatch and Editor fallback result.
#[derive(Debug)]
pub struct ViewerInputOutcome {
    dispatch: DispatchOutcome,
    target: ViewerTarget,
    selection: Option<SelectionDelta<EditPartId>>,
}

impl ViewerInputOutcome {
    /// Returns the underlying Figure dispatch result.
    pub const fn dispatch(&self) -> DispatchOutcome {
        self.dispatch
    }

    /// Returns the Editor target resolved at the entry point.
    pub fn target(&self) -> ViewerTarget {
        self.target.clone()
    }

    /// Returns the selection delta when Editor fallback changed selection.
    pub const fn selection(&self) -> Option<&SelectionDelta<EditPartId>> {
        self.selection.as_ref()
    }
}

/// Failure while constructing or refreshing a [`GraphicalViewer`].
#[derive(Debug)]
pub enum ViewerError {
    /// A committed tree identity query failed.
    Identity(novadraw::tree::TreeQueryError),
    /// A typed capability query failed.
    Capability(novadraw::figure::CapabilityQueryError),
    /// A Figure coordinate query failed while projecting or converting geometry.
    Coordinate(novadraw::tree::CoordinateQueryError),
    /// Creation failed and cleanup could not complete all steps.
    ProjectionCleanup {
        /// Original error, independent of subsequent cleanup failures.
        original: Box<ViewerError>,
        /// Actual cleanup progress, also available through Viewer diagnostics.
        report: Arc<ProjectionCleanup>,
    },
    /// An Overlay batch was rejected before any write.
    InvalidOverlayBatch {
        /// First invalid input position in caller order.
        index: usize,
        /// Figure at that position.
        figure: FigureId,
        /// Domain reason for rejection.
        reason: &'static str,
    },
    /// The Viewer was faulted by an earlier partial extension failure.
    Faulted,
    /// The adapter exposed the same model identity more than once.
    DuplicateModel,
    /// Two parts attempted to own the same Figure.
    DuplicateVisual,
    /// The adapter root changed while the Viewer was active.
    RootChanged,
    /// The model revision changed while a projection snapshot was being read.
    SnapshotRevisionChanged,
    /// The ordered connection snapshot contains a duplicate connection model.
    DuplicateConnectionModel {
        /// Debug representation of the duplicate connection model identity.
        connection: String,
    },
    /// A connection endpoint is absent from the desired containment snapshot.
    MissingConnectionEndpoint {
        /// Debug representation of the connection model identity.
        connection: String,
        /// Debug representation of the missing endpoint model identity.
        endpoint: String,
    },
    /// One model identity is used by both containment and connection projections.
    ConnectionModelCollision {
        /// Debug representation of the colliding model identity.
        model: String,
    },
    /// A connection factory returned a Figure without Connection behavior.
    InvalidConnectionFigure {
        /// Debug representation of the connection model identity.
        connection: String,
    },
    /// A connection bendpoint contains a non-finite coordinate.
    InvalidConnectionBendpoint {
        /// Debug representation of the connection model identity.
        connection: String,
    },
    /// A notification was older than the revision already projected.
    StaleRevision {
        /// Last successfully projected revision.
        applied: ModelRevision,
        /// Revision carried by the stale notification.
        actual: ModelRevision,
    },
    /// One or more model revisions were omitted from the notification stream.
    RevisionGap {
        /// Next revision required by the Viewer.
        expected: ModelRevision,
        /// Revision observed in the notification stream or adapter snapshot.
        actual: ModelRevision,
    },
    /// A rejected or recoverable command changed the model despite its rollback contract.
    RejectedEditChangedModel,
    /// Model containment exceeded the supported recursion depth.
    DepthLimitExceeded,
    /// An application model query failed.
    Model(String),
    /// An application EditPart extension failed.
    EditPart(EditPartError),
    /// Part topology became inconsistent.
    PartTree(PartTreeError),
    /// Figure Runtime mutation failed.
    Runtime(RuntimeMutationError),
    /// Zoom policy or level configuration failed.
    Zoom(ZoomError),
    /// Figure shape or border mutation failed.
    Shape(ShapeMutationError),
    /// Runtime derived state could not converge for an Editor query.
    RuntimePreparation(FramePreparationError),
    /// Host-provided monotonic time violated Runtime timing constraints.
    RuntimeTime(TimeError),
    /// Connection Runtime mutation or routing failed.
    Connection(ConnectionRuntimeError),
    /// Root layer construction or mutation failed.
    Layer(LayerError),
    /// EditPolicy rejected a request or failed.
    Policy(PolicyError),
    /// Direct text editing rejected the operation.
    DirectTextEdit(DirectTextEditError),
    /// A selection or overlay operation referenced an invalid EditPart.
    InvalidPart(EditPartId),
    /// An expected behavior or registry entry was absent.
    InconsistentState,
}

impl fmt::Display for ViewerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectionCleanup { original, report } => write!(
                formatter,
                "{original}; cleanup has {} failed steps and {} live Figures",
                report.failures.len(),
                report.live_figures.len()
            ),
            Self::InvalidOverlayBatch {
                index,
                figure,
                reason,
            } => write!(formatter, "Overlay input {index} ({figure:?}): {reason}"),
            Self::Faulted => formatter.write_str("Viewer is faulted"),
            Self::DuplicateModel => formatter.write_str("duplicate model identity"),
            Self::DuplicateVisual => formatter.write_str("duplicate visual registration"),
            Self::RootChanged => formatter.write_str("model root changed while Viewer was active"),
            Self::SnapshotRevisionChanged => {
                formatter.write_str("model revision changed while reading the projection snapshot")
            }
            Self::DuplicateConnectionModel { connection } => {
                write!(
                    formatter,
                    "duplicate connection model identity {connection}"
                )
            }
            Self::MissingConnectionEndpoint {
                connection,
                endpoint,
            } => {
                write!(
                    formatter,
                    "connection {connection} endpoint {endpoint} is absent from model containment"
                )
            }
            Self::ConnectionModelCollision { model } => {
                write!(
                    formatter,
                    "model identity {model} is both a containment and connection model"
                )
            }
            Self::InvalidConnectionFigure { connection } => {
                write!(
                    formatter,
                    "connection factory returned a non-connection Figure for model {connection}"
                )
            }
            Self::InvalidConnectionBendpoint { connection } => {
                write!(
                    formatter,
                    "connection {connection} has a non-finite bendpoint"
                )
            }
            Self::StaleRevision { applied, actual } => write!(
                formatter,
                "stale model revision {}; last applied revision is {}",
                actual.value(),
                applied.value()
            ),
            Self::RevisionGap { expected, actual } => write!(
                formatter,
                "model revision gap: expected {}, got {}",
                expected.value(),
                actual.value()
            ),
            Self::RejectedEditChangedModel => {
                formatter.write_str("a rejected Editor command changed the model")
            }
            Self::DepthLimitExceeded => {
                write!(
                    formatter,
                    "EditPart tree exceeds {MAX_PART_TREE_DEPTH} levels"
                )
            }
            Self::Model(message) => write!(formatter, "model query failed: {message}"),
            Self::EditPart(error) => error.fmt(formatter),
            Self::PartTree(error) => error.fmt(formatter),
            Self::Runtime(error) => error.fmt(formatter),
            Self::Zoom(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
            Self::RuntimePreparation(error) => error.fmt(formatter),
            Self::RuntimeTime(error) => error.fmt(formatter),
            Self::Connection(error) => error.fmt(formatter),
            Self::Layer(error) => error.fmt(formatter),
            Self::Policy(error) => error.fmt(formatter),
            Self::DirectTextEdit(error) => error.fmt(formatter),
            Self::InvalidPart(part) => write!(formatter, "invalid EditPart: {part:?}"),
            Self::InconsistentState => formatter.write_str("Viewer state is inconsistent"),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::Capability(error) => error.fmt(formatter),
        }
    }
}

impl Error for ViewerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Capability(error) => Some(error),
            Self::ProjectionCleanup { original, .. } => Some(original.as_ref()),
            Self::EditPart(error) => Some(error),
            Self::PartTree(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::Zoom(error) => Some(error),
            Self::Shape(error) => Some(error),
            Self::RuntimePreparation(error) => Some(error),
            Self::RuntimeTime(error) => Some(error),
            Self::Connection(error) => Some(error),
            Self::Layer(error) => Some(error),
            Self::Policy(error) => Some(error),
            Self::DirectTextEdit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EditPartError> for ViewerError {
    fn from(value: EditPartError) -> Self {
        Self::EditPart(value)
    }
}

impl From<novadraw::tree::TreeQueryError> for ViewerError {
    fn from(error: novadraw::tree::TreeQueryError) -> Self {
        Self::Identity(error)
    }
}

impl From<PartTreeError> for ViewerError {
    fn from(value: PartTreeError) -> Self {
        Self::PartTree(value)
    }
}

impl From<RuntimeMutationError> for ViewerError {
    fn from(value: RuntimeMutationError) -> Self {
        Self::Runtime(value)
    }
}

impl From<ZoomError> for ViewerError {
    fn from(value: ZoomError) -> Self {
        Self::Zoom(value)
    }
}

impl From<ShapeMutationError> for ViewerError {
    fn from(value: ShapeMutationError) -> Self {
        Self::Shape(value)
    }
}

impl From<FramePreparationError> for ViewerError {
    fn from(value: FramePreparationError) -> Self {
        Self::RuntimePreparation(value)
    }
}

impl From<TimeError> for ViewerError {
    fn from(value: TimeError) -> Self {
        Self::RuntimeTime(value)
    }
}

impl From<ConnectionRuntimeError> for ViewerError {
    fn from(value: ConnectionRuntimeError) -> Self {
        Self::Connection(value)
    }
}

impl From<LayerError> for ViewerError {
    fn from(value: LayerError) -> Self {
        Self::Layer(value)
    }
}

impl From<PolicyError> for ViewerError {
    fn from(value: PolicyError) -> Self {
        Self::Policy(value)
    }
}

impl From<DirectTextEditError> for ViewerError {
    fn from(value: DirectTextEditError) -> Self {
        Self::DirectTextEdit(value)
    }
}

struct ModelSnapshot<I> {
    revision: ModelRevision,
    root: I,
    children: HashMap<I, Vec<I>>,
    parents: HashMap<I, I>,
    models: HashSet<I>,
    connections: Vec<ModelConnection<I>>,
    connection_indexes: HashMap<I, usize>,
}

impl<I> ModelSnapshot<I>
where
    I: Copy + Eq + std::hash::Hash + fmt::Debug,
{
    fn capture<A>(model: &A) -> Result<Self, ViewerError>
    where
        A: ModelAdapter<ModelId = I>,
    {
        let revision = model.revision();
        let root = model.root();
        let mut children = HashMap::new();
        let mut parents = HashMap::new();
        let mut seen = HashSet::new();
        Self::capture_subtree(model, root, 0, &mut seen, &mut children, &mut parents)?;
        let connections = model
            .connections()
            .map_err(|error| ViewerError::Model(error.to_string()))?;
        let mut connection_indexes = HashMap::with_capacity(connections.len());
        for (index, connection) in connections.iter().enumerate() {
            if connection_indexes.insert(connection.id(), index).is_some() {
                return Err(ViewerError::DuplicateConnectionModel {
                    connection: format!("{:?}", connection.id()),
                });
            }
            if seen.contains(&connection.id()) {
                return Err(ViewerError::ConnectionModelCollision {
                    model: format!("{:?}", connection.id()),
                });
            }
            for endpoint in [connection.source(), connection.target()] {
                if !seen.contains(&endpoint) {
                    return Err(ViewerError::MissingConnectionEndpoint {
                        connection: format!("{:?}", connection.id()),
                        endpoint: format!("{endpoint:?}"),
                    });
                }
            }
        }
        if model.revision() != revision {
            return Err(ViewerError::SnapshotRevisionChanged);
        }
        Ok(Self {
            revision,
            root,
            children,
            parents,
            models: seen,
            connections,
            connection_indexes,
        })
    }

    fn capture_subtree<A>(
        model: &A,
        model_id: I,
        depth: usize,
        seen: &mut std::collections::HashSet<I>,
        children: &mut HashMap<I, Vec<I>>,
        parents: &mut HashMap<I, I>,
    ) -> Result<(), ViewerError>
    where
        A: ModelAdapter<ModelId = I>,
    {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(RECURSIVE_STACK_RED_ZONE, RECURSIVE_STACK_GROWTH, || {
                Self::capture_subtree_inner(model, model_id, depth, seen, children, parents)
            });
        }
        Self::capture_subtree_inner(model, model_id, depth, seen, children, parents)
    }

    fn capture_subtree_inner<A>(
        model: &A,
        model_id: I,
        depth: usize,
        seen: &mut std::collections::HashSet<I>,
        children: &mut HashMap<I, Vec<I>>,
        parents: &mut HashMap<I, I>,
    ) -> Result<(), ViewerError>
    where
        A: ModelAdapter<ModelId = I>,
    {
        if depth > MAX_PART_TREE_DEPTH {
            return Err(ViewerError::DepthLimitExceeded);
        }
        if !seen.insert(model_id) {
            return Err(ViewerError::DuplicateModel);
        }
        let direct = model
            .children(model_id)
            .map_err(|error| ViewerError::Model(error.to_string()))?;
        for child in direct.iter().copied() {
            parents.insert(child, model_id);
            Self::capture_subtree(model, child, depth + 1, seen, children, parents)?;
        }
        children.insert(model_id, direct);
        Ok(())
    }

    fn children_of(&self, model_id: I) -> Result<&[I], ViewerError> {
        self.children
            .get(&model_id)
            .map(Vec::as_slice)
            .ok_or(ViewerError::InconsistentState)
    }

    fn contains_model(&self, model_id: I) -> bool {
        self.models.contains(&model_id)
    }

    fn connection(&self, model_id: I) -> Option<ModelConnection<I>> {
        self.connection_indexes
            .get(&model_id)
            .map(|index| self.connections[*index])
    }
}

fn layer_key(value: &'static str) -> LayerKey {
    LayerKey::new(value).expect("Viewer layer keys are non-empty constants")
}

fn add_layer(
    runtime: &mut Runtime,
    parent: FigureId,
    figure: Box<dyn Figure>,
    key: &'static str,
) -> Result<FigureId, ViewerError> {
    Ok(runtime
        .layers()
        .add(parent, figure, layer_key(key), LayerPlacement::Last)?)
}

fn create_root_layers(bounds: Rectangle) -> Result<(Runtime, RootLayers), ViewerError> {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(LayeredPane::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let viewport_layer = add_layer(
        &mut runtime,
        root,
        Box::new(LayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        VIEWPORT_LAYER,
    )?;
    let feedback = add_layer(
        &mut runtime,
        root,
        Box::new(LayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        FEEDBACK_LAYER,
    )?;
    let handles = add_layer(
        &mut runtime,
        root,
        Box::new(LayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        HANDLE_LAYER,
    )?;
    let viewport = runtime.viewports().add(viewport_layer, bounds)?;
    let scalable =
        runtime
            .figure(viewport.figure_id())?
            .add(Box::new(ScalableFreeformLayeredPane::new(
                0.0,
                0.0,
                bounds.width,
                bounds.height,
            )))?;
    let grid = add_layer(
        &mut runtime,
        scalable,
        Box::new(LayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        GRID_LAYER,
    )?;
    let printable = add_layer(
        &mut runtime,
        scalable,
        Box::new(FreeformLayeredPane::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        PRINTABLE_LAYERS,
    )?;
    let scaled_feedback = add_layer(
        &mut runtime,
        scalable,
        Box::new(FreeformLayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        SCALED_FEEDBACK_LAYER,
    )?;
    let primary = add_layer(
        &mut runtime,
        printable,
        Box::new(FreeformLayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        PRIMARY_LAYER,
    )?;
    let connection = add_layer(
        &mut runtime,
        printable,
        Box::new(ConnectionLayerFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        CONNECTION_LAYER,
    )?;
    runtime
        .figure(root)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .figure(viewport_layer)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .figure(scalable)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .figure(printable)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    Ok((
        runtime,
        RootLayers {
            root,
            viewport_layer,
            viewport: viewport.figure_id(),
            scalable,
            grid,
            printable,
            primary,
            connection,
            scaled_feedback,
            feedback,
            handles,
        },
    ))
}

#[derive(Clone)]
struct ConnectionProjection<I> {
    part: ConnectionPartId,
    connection: ConnectionId,
    source_model: I,
    target_model: I,
    source_anchor: Option<AnchorId>,
    target_anchor: Option<AnchorId>,
    source_anchor_key: Option<AnchorSemanticKey>,
    target_anchor_key: Option<AnchorSemanticKey>,
    registered: bool,
    children: Vec<(
        FigureId,
        std::rc::Rc<dyn novadraw::connection::ConnectionLocatorStrategy>,
    )>,
}

struct SharedConnectionLocator(std::rc::Rc<dyn novadraw::connection::ConnectionLocatorStrategy>);
impl novadraw::connection::ConnectionLocatorStrategy for SharedConnectionLocator {
    fn locate(
        &self,
        points: &novadraw::geometry::PointList,
    ) -> Result<novadraw::connection::LocatorPlacement, novadraw::connection::LocatorError> {
        self.0.locate(points)
    }
    fn locate_child(
        &self,
        input: &novadraw::connection::LocatorInput<'_>,
    ) -> Result<novadraw::connection::ChildLocatorPlacement, novadraw::connection::LocatorError>
    {
        self.0.locate_child(input)
    }
}

/// Owns one application model projection, its EditPart tree, and its Figure Runtime.
///
/// Model mutation and host driving use scoped entry points rather than complete mutable access:
///
/// ```compile_fail
/// use novadraw_editor::{EditPartFactory, GraphicalViewer, ModelAdapter};
///
/// fn bypass_model_transaction<A, F>(viewer: &mut GraphicalViewer<A, F>)
/// where
///     A: ModelAdapter,
///     F: EditPartFactory<A>,
/// {
///     let _model = viewer.model_mut();
/// }
/// ```
///
/// ```compile_fail
/// use novadraw_editor::{EditPartFactory, GraphicalViewer, ModelAdapter};
///
/// fn bypass_drive<A, F>(viewer: &mut GraphicalViewer<A, F>)
/// where
///     A: ModelAdapter,
///     F: EditPartFactory<A>,
/// {
///     let _submission = viewer.prepare_submission;
/// }
/// ```
pub struct GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    model: A,
    factory: F,
    runtime: Runtime,
    root_layers: RootLayers,
    zoom_manager: ZoomManager,
    parts: PartTree<A::ModelId>,
    behaviors: BehaviorStore<A>,
    policies: PolicyStore<A>,
    model_registry: HashMap<A::ModelId, EditPartId>,
    connection_projections: HashMap<A::ModelId, ConnectionProjection<A::ModelId>>,
    connection_routers: HashMap<ConnectionRouterKey, RouterId>,
    managed_layer_router: Option<RouterId>,
    visual_registry: HashMap<FigureId, VisualOwner>,
    selection: SelectionModel<EditPartId>,
    direct_text_edit: Option<ActiveDirectTextEdit<A>>,
    next_direct_text_edit_session: u64,
    current_time: MonotonicTime,
    direct_text_blink_deadline: Option<MonotonicTime>,
    text_input_effects: Vec<TextInputEffect>,
    applied_revision: ModelRevision,
    model_root: A::ModelId,
    faulted: bool,
    active_behaviors: HashSet<EditPartId>,
    last_cleanup: Option<Arc<ProjectionCleanup>>,
}

pub(crate) struct ViewerEditSession<'a, A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    viewer: &'a mut GraphicalViewer<A, F>,
    closed: bool,
}

impl<A, F> ViewerEditSession<'_, A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    pub(crate) fn model(&mut self) -> &mut A {
        &mut self.viewer.model
    }

    pub(crate) fn commit(mut self) -> Result<bool, ViewerError> {
        self.closed = true;
        self.viewer.synchronize()
    }

    pub(crate) fn reject(mut self) -> Result<(), ViewerError> {
        self.closed = true;
        if self.viewer.synchronize()? {
            self.viewer.mark_faulted();
            return Err(ViewerError::RejectedEditChangedModel);
        }
        Ok(())
    }

    pub(crate) fn fault(mut self) {
        self.closed = true;
        self.viewer.mark_faulted();
    }
}

impl<A, F> Drop for ViewerEditSession<'_, A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    fn drop(&mut self) {
        if !self.closed {
            self.viewer.mark_faulted();
        }
    }
}

/// Narrow host-facing access to the Viewer-owned Runtime drive operations.
pub struct ViewerDrive<'a, A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    viewer: &'a mut GraphicalViewer<A, F>,
}

impl<A, F> ViewerDrive<'_, A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    /// Prepares one renderer submission through the Viewer-owned Runtime.
    pub fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        if self.viewer.ensure_ready().is_err() {
            return FramePreparation::Error(FramePreparationError::Faulted);
        }
        self.viewer
            .runtime
            .frames()
            .prepare_submission(surface, capabilities)
    }

    /// Completes the current renderer submission through the Viewer-owned Runtime.
    pub fn complete_submission(
        &mut self,
        session_id: BackendSessionId,
        frame_id: FrameId,
        result: RenderResult,
    ) -> Result<SubmissionAcknowledgement, SubmissionCompletionError> {
        self.viewer
            .runtime
            .frames()
            .complete_submission(session_id, frame_id, result)
    }

    /// Starts a fresh backend submission session after renderer recreation.
    pub fn reset_backend_session(&mut self) -> Result<BackendSessionId, BackendSessionError> {
        self.viewer.runtime.frames().reset_backend_session()
    }

    /// Resizes the Viewer-owned logical viewport.
    pub fn resize_logical_viewport(
        &mut self,
        width: f64,
        height: f64,
    ) -> Result<bool, LogicalViewportResizeError> {
        if self.viewer.ensure_ready().is_err() {
            return Err(LogicalViewportResizeError::Faulted);
        }
        self.viewer
            .runtime
            .frames()
            .resize_logical_viewport(width, height)
    }

    /// Requests a complete redraw from the Viewer-owned Runtime.
    pub fn request_full_redraw(&mut self) {
        if self.viewer.ensure_ready().is_err() {
            return;
        }
        self.viewer.runtime.frames().request_full_redraw();
    }

    /// Reports whether the Viewer-owned Runtime still has work to publish.
    pub fn has_pending_update(&mut self) -> bool {
        self.viewer.runtime.frames().has_pending_update()
    }

    /// Cancels Figure-level pointer capture and gesture state.
    pub fn cancel_gestures(&mut self) {
        self.viewer.runtime.input().cancel_gestures();
    }

    /// Releases Figure-level focus.
    pub fn release_focus(&mut self) -> FocusChange {
        self.viewer.runtime.focus().release()
    }

    /// Registers a packaged font resource.
    pub fn register_builtin_font(
        &mut self,
        font: novadraw::render::BuiltinFont,
    ) -> Result<FontId, ResourceError> {
        if self.viewer.ensure_ready().is_err() {
            return Err(ResourceError::Faulted);
        }
        self.viewer.runtime.resources().register_builtin_font(font)
    }

    /// Advances Runtime timers and the active direct-edit caret blink from host time.
    pub fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, ViewerError> {
        self.viewer.ensure_ready()?;
        self.viewer.advance_time_inner(now)
    }

    /// Returns the earliest Runtime or direct-edit deadline requiring a host wakeup.
    pub fn next_wake_deadline(&mut self) -> Option<MonotonicTime> {
        self.viewer.next_wake_deadline_inner()
    }
}

impl<A, F> GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    /// Creates a Viewer and recursively projects the current model snapshot.
    pub fn new(mut model: A, mut factory: F, bounds: Rectangle) -> Result<Self, ViewerError> {
        let snapshot = ModelSnapshot::capture(&model)?;
        let applied_revision = snapshot.revision;
        let initial_events = model.drain_events();
        if let Some(event) = initial_events
            .iter()
            .find(|event| event.revision() > applied_revision)
        {
            return Err(ViewerError::RevisionGap {
                expected: applied_revision.next().unwrap_or(applied_revision),
                actual: event.revision(),
            });
        }
        let (mut runtime, root_layers) = create_root_layers(bounds)?;
        let scalable = runtime
            .tree()
            .scale_handle(root_layers.scalable())
            .map_err(ViewerError::Capability)?
            .ok_or(ViewerError::InconsistentState)?;
        let viewport = runtime
            .tree()
            .viewport_handle(root_layers.viewport())?
            .ok_or(ViewerError::InconsistentState)?;
        let mut zoom_manager = ZoomManager::new(scalable, viewport);
        zoom_manager.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));
        let mut connection_routers = HashMap::new();
        for registration in factory.connection_routers()? {
            let (key, router) = registration.into_parts();
            if connection_routers.contains_key(&key) {
                return Err(EditPartError::operation("duplicate connection Router key").into());
            }
            let router = runtime.connections().try_register_router(router)?;
            connection_routers.insert(key, router);
        }
        let parts = PartTree::new(root_layers.root(), root_layers.primary());
        let namespace = parts.namespace();
        let root_part = parts.root();
        let mut viewer = Self {
            model,
            factory,
            runtime,
            root_layers,
            zoom_manager,
            parts,
            behaviors: BehaviorStore::new(namespace),
            policies: PolicyStore::new(namespace),
            model_registry: HashMap::new(),
            connection_projections: HashMap::new(),
            connection_routers,
            managed_layer_router: None,
            visual_registry: HashMap::from([(root_layers.root(), VisualOwner::part(root_part))]),
            selection: SelectionModel::new(),
            direct_text_edit: None,
            next_direct_text_edit_session: 1,
            current_time: MonotonicTime::ZERO,
            direct_text_blink_deadline: None,
            text_input_effects: Vec::new(),
            applied_revision,
            model_root: snapshot.root,
            faulted: false,
            active_behaviors: HashSet::new(),
            last_cleanup: None,
        };
        let contents = viewer.create_subtree(root_part, snapshot.root, &snapshot)?;
        viewer.parts.designate_contents(contents)?;
        viewer.synchronize_connections(&snapshot)?;
        Ok(viewer)
    }

    /// Returns the namespace that owns all Part IDs in this Viewer.
    pub const fn namespace(&self) -> EditorNamespace {
        self.parts.namespace()
    }

    /// Returns the synthetic root part.
    pub const fn root(&self) -> EditPartId {
        self.parts.root()
    }

    /// Returns the model-backed contents part.
    pub fn contents(&self) -> EditPartId {
        self.parts
            .contents()
            .expect("a successfully constructed Viewer always has contents")
    }

    /// Returns the read-only EditPart topology.
    pub const fn parts(&self) -> &PartTree<A::ModelId> {
        &self.parts
    }

    /// Returns the application model adapter.
    pub const fn model(&self) -> &A {
        &self.model
    }

    /// Returns narrow host-facing Runtime drive access.
    pub fn drive(&mut self) -> ViewerDrive<'_, A, F> {
        ViewerDrive { viewer: self }
    }

    /// Applies one non-history model update and immediately synchronizes its notifications.
    pub fn synchronize_model(&mut self, update: impl FnOnce(&mut A)) -> Result<bool, ViewerError> {
        let changed_before_update = self.synchronize()?;
        match catch_unwind(AssertUnwindSafe(|| update(&mut self.model))) {
            Ok(()) => Ok(self.synchronize()? || changed_before_update),
            Err(payload) => {
                self.mark_faulted_with_cause(&failure::panic_message(payload.as_ref()));
                resume_unwind(payload)
            }
        }
    }

    pub(crate) fn begin_edit_session(
        &mut self,
    ) -> Result<ViewerEditSession<'_, A, F>, ViewerError> {
        self.synchronize()?;
        Ok(ViewerEditSession {
            viewer: self,
            closed: false,
        })
    }

    /// Returns the Figure Runtime.
    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Returns the Runtime state for one projected Connection.
    pub fn connection_state(
        &mut self,
        connection: ConnectionId,
    ) -> Result<ConnectionStateSnapshot, ConnectionRuntimeError> {
        self.runtime.connections().state(connection)
    }

    /// Returns a stable copy of one TextFlow layout.
    pub fn text_flow_layout(&mut self, figure: FigureId) -> Result<TextLayout, ShapeMutationError> {
        self.runtime.text().flow_layout(figure).cloned()
    }

    /// Returns caret geometry in one TextFlow's local coordinate domain.
    pub fn text_flow_local_caret_geometry(
        &mut self,
        figure: FigureId,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextFlowQueryError> {
        self.runtime
            .text()
            .flow_local_caret_geometry(figure, position)
    }

    /// Returns the standard root layer identities.
    pub const fn root_layers(&self) -> RootLayers {
        self.root_layers
    }

    /// Returns the Viewer-owned zoom controller.
    pub const fn zoom_manager(&self) -> &ZoomManager {
        &self.zoom_manager
    }

    /// Replaces the discrete zoom levels retained by this Viewer.
    pub fn set_zoom_levels(&mut self, levels: Vec<f64>) -> Result<(), ViewerError> {
        self.ensure_ready()?;
        self.zoom_manager.set_zoom_levels(levels)?;
        Ok(())
    }

    /// Replaces the scroll compensation policy retained by this Viewer.
    pub fn set_zoom_scroll_policy(
        &mut self,
        policy: Arc<dyn ZoomScrollPolicy>,
    ) -> Result<(), ViewerError> {
        self.ensure_ready()?;
        self.zoom_manager.set_scroll_policy(policy);
        Ok(())
    }

    /// Converges pending layout and derived state without exposing Runtime mutation.
    pub fn stabilize_for_query(&mut self) -> Result<(), ViewerError> {
        self.ensure_ready()?;
        Ok(self.runtime.frames().stabilize_for_query()?)
    }

    /// Applies style to the standard root Viewport.
    pub fn set_root_viewport_style(
        &mut self,
        style: FigureStyle,
    ) -> Result<bool, RuntimeMutationError> {
        if self.ensure_ready().is_err() {
            return Err(RuntimeMutationError::Faulted);
        }
        self.runtime
            .figure(self.root_layers.viewport())?
            .set_style(style)
    }

    /// Replaces the standard root Viewport border.
    pub fn set_root_viewport_border(
        &mut self,
        border: impl Border + 'static,
    ) -> Result<bool, ViewerError> {
        self.ensure_ready()?;
        self.runtime
            .figure(self.root_layers.viewport())?
            .update_capability(ReplaceFigureBorder::set(border))
            .map(|receipt| receipt.changed)
            .map_err(|error| ViewerError::EditPart(EditPartError::operation(error.to_string())))
    }

    /// Registers a connection Router owned by this Viewer.
    pub fn register_connection_router(
        &mut self,
        router: Box<dyn ConnectionRouter>,
    ) -> Result<RouterId, ConnectionRuntimeError> {
        if self.ensure_ready().is_err() {
            return Err(ConnectionRuntimeError::Faulted);
        }
        self.runtime.connections().try_register_router(router)
    }

    /// Removes an application router after all layer and explicit bindings release it.
    pub fn remove_connection_router(
        &mut self,
        router: RouterId,
    ) -> Result<Box<dyn ConnectionRouter>, ConnectionRuntimeError> {
        self.runtime.connections().remove_router(router)
    }

    /// Replaces a Viewer-owned default router and releases the preceding managed router.
    /// Applications can refresh obstacle identities without accumulating router registrations.
    pub fn replace_connection_layer_router(
        &mut self,
        router: Box<dyn ConnectionRouter>,
    ) -> Result<(), ConnectionRuntimeError> {
        let next = self.register_connection_router(router)?;
        if let Err(error) = self.set_connection_layer_router(next) {
            self.remove_connection_router(next)?;
            return Err(error);
        }
        if let Some(previous) = self.managed_layer_router.replace(next) {
            self.remove_connection_router(previous)?;
        }
        Ok(())
    }

    /// Selects a Router for the standard connection layer.
    pub fn set_connection_layer_router(
        &mut self,
        router: RouterId,
    ) -> Result<Vec<ConnectionId>, ConnectionRuntimeError> {
        if self.ensure_ready().is_err() {
            return Err(ConnectionRuntimeError::Faulted);
        }
        self.runtime
            .connections()
            .set_layer_router(self.root_layers.connection(), router)
    }

    /// Applies a typed component update to a Viewer-owned visual.
    pub fn update_visual_component<U>(
        &mut self,
        figure: FigureId,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate,
    {
        if self.ensure_ready().is_err() {
            return Err(ComponentUpdateError::Runtime(RuntimeMutationError::Faulted));
        }
        if self
            .visual_registry
            .get(&figure)
            .is_none_or(|owner| !owner.is_part())
        {
            return Err(ComponentUpdateError::Runtime(
                RuntimeMutationError::UnknownOrDisposedFigure(figure),
            ));
        }
        self.runtime.figure(figure)?.update_component(update)
    }

    /// Adds an implementation-detail visual below a Part's content pane.
    ///
    /// The visual is intentionally resolved through its registered Part ancestor.
    pub fn add_internal_visual(
        &mut self,
        owner: EditPartId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, ViewerError> {
        self.ensure_ready()?;
        let pane = self
            .parts
            .get(owner)
            .ok_or(ViewerError::InvalidPart(owner))?
            .content_pane();
        Ok(self.runtime.figure(pane)?.add(figure)?)
    }

    fn advance_time_inner(&mut self, now: MonotonicTime) -> Result<bool, ViewerError> {
        let runtime_changed = self.runtime.animation().advance_time(now)?;
        self.current_time = now;
        let Some(deadline) = self.direct_text_blink_deadline else {
            return Ok(runtime_changed);
        };
        if now < deadline {
            return Ok(runtime_changed);
        }
        let Some(active) = self.direct_text_edit.as_mut() else {
            self.direct_text_blink_deadline = None;
            return Ok(runtime_changed);
        };
        let Some(caret) = active.caret_feedback else {
            self.direct_text_blink_deadline = None;
            return Ok(runtime_changed);
        };

        let elapsed = now.as_micros() - deadline.as_micros();
        let intervals = elapsed / DIRECT_TEXT_CARET_BLINK_INTERVAL_MICROS + 1;
        if intervals % 2 == 1 {
            active.caret_visible = !active.caret_visible;
        }
        let next = intervals
            .checked_mul(DIRECT_TEXT_CARET_BLINK_INTERVAL_MICROS)
            .and_then(|advance| deadline.as_micros().checked_add(advance))
            .map(MonotonicTime::from_micros)
            .filter(|next| *next > now);
        self.direct_text_blink_deadline = next;
        let caret_changed = self
            .runtime
            .figure(caret)?
            .set_visible(active.caret_visible)?;
        Ok(runtime_changed || caret_changed)
    }

    fn next_wake_deadline_inner(&mut self) -> Option<MonotonicTime> {
        match (
            self.runtime.animation().next_wake_deadline(),
            self.direct_text_blink_deadline,
        ) {
            (Some(runtime), Some(direct_edit)) => Some(runtime.min(direct_edit)),
            (Some(runtime), None) => Some(runtime),
            (None, Some(direct_edit)) => Some(direct_edit),
            (None, None) => None,
        }
    }

    /// Returns the current root Viewport origin in content coordinates.
    pub fn viewport_origin(&self) -> Result<Point, ViewerError> {
        Ok(self.viewport_handle()?.view_location())
    }

    /// Returns the current model-content zoom factor.
    pub fn viewport_scale(&self) -> Result<f64, ViewerError> {
        Ok(self
            .runtime
            .tree()
            .scale_handle(self.root_layers.scalable())
            .map_err(ViewerError::Capability)?
            .ok_or(ViewerError::InconsistentState)?
            .scale())
    }

    /// Returns the root Viewport client rectangle in logical surface coordinates.
    pub fn viewport_bounds_in_surface(&self) -> Result<Rectangle, ViewerError> {
        let viewport = self.root_layers.viewport();
        let bounds = self.runtime.tree().figure_bounds(viewport)?;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(viewport)
            .map_err(ViewerError::Coordinate)?;
        let mut client = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        client.transform(transform);
        Ok(client)
    }

    /// Converts a logical surface point into the model-content coordinate domain.
    pub fn model_point_from_surface(&self, point: Point) -> Result<Point, ViewerError> {
        self.point_from_surface_in(self.root_layers.scalable(), point)
    }

    /// Converts a logical surface point into the shared Connection routing domain.
    pub fn connection_routing_point_from_surface(
        &self,
        point: Point,
    ) -> Result<Point, ViewerError> {
        self.point_from_surface_in(self.root_layers.connection(), point)
    }

    /// Converts a logical surface point into one connection's routing domain.
    pub fn connection_point_from_surface(
        &self,
        connection: ConnectionPartId,
        point: Point,
    ) -> Result<Point, ViewerError> {
        self.parts
            .get(connection.edit_part())
            .ok_or(ViewerError::InvalidPart(connection.edit_part()))?;
        self.connection_routing_point_from_surface(point)
    }

    /// Sets the root Viewport origin through the Runtime mutation boundary.
    pub fn set_viewport_origin(&mut self, origin: Point) -> Result<bool, ViewerError> {
        let viewport = self.viewport_handle()?;
        Ok(self.runtime.viewports().set_view_location(
            viewport.figure_id(),
            origin.x(),
            origin.y(),
        )?)
    }

    /// Sets content zoom while preserving an optional logical surface anchor.
    pub fn set_viewport_scale_at(
        &mut self,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ViewerError> {
        let anchor = anchor
            .map(|point| {
                self.runtime
                    .tree()
                    .surface_to_local_transform(self.root_layers.viewport())
                    .map_err(ViewerError::Coordinate)?
                    .map(|transform| transform.transform_point(point))
                    .ok_or(ViewerError::InconsistentState)
            })
            .transpose()?;
        Ok(self
            .runtime
            .viewports()
            .set_zoom_at(&self.zoom_manager, scale, anchor)?)
    }

    pub(crate) fn scroll_viewport_by_surface_delta(
        &mut self,
        delta: novadraw::geometry::Vec2,
    ) -> Result<bool, ViewerError> {
        let transform = self
            .runtime
            .tree()
            .child_content_to_surface_transform(self.root_layers.scalable())
            .map_err(ViewerError::Coordinate)?
            .inverse()
            .ok_or(ViewerError::InconsistentState)?;
        let delta = transform.transform_vector(delta);
        let viewport = self.viewport_handle()?;
        let origin = viewport.view_location();
        Ok(self.runtime.viewports().set_view_location(
            viewport.figure_id(),
            origin.x() + delta.x(),
            origin.y() + delta.y(),
        )?)
    }

    pub(crate) fn viewport_ranges(
        &self,
    ) -> Result<(RangeModelSnapshot, RangeModelSnapshot), ViewerError> {
        let viewport = self.viewport_handle()?;
        Ok((viewport.horizontal_range(), viewport.vertical_range()))
    }

    pub(crate) fn clamp_viewport_to_feedback_replacement(
        &mut self,
        replaced_parts: &[EditPartId],
    ) -> Result<bool, ViewerError> {
        let excluded = replaced_parts
            .iter()
            .map(|part| {
                self.parts
                    .get(*part)
                    .map(|node| node.primary_figure())
                    .ok_or(ViewerError::InvalidPart(*part))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let extent = self
            .runtime
            .tree()
            .freeform_extent_excluding(self.root_layers.scalable(), &excluded)
            .map_err(|_| ViewerError::InconsistentState)?;
        let (horizontal, vertical) = self.viewport_ranges()?;
        let baseline = Rectangle::new(0.0, 0.0, horizontal.extent, vertical.extent);
        let envelope = extent.union(baseline);
        let maximum_x = envelope.x + envelope.width - horizontal.extent;
        let maximum_y = envelope.y + envelope.height - vertical.extent;
        let origin = self.viewport_origin()?;
        let projected = Point::new(
            origin.x().clamp(envelope.x, maximum_x.max(envelope.x)),
            origin.y().clamp(envelope.y, maximum_y.max(envelope.y)),
        );
        self.set_viewport_origin(projected)
    }

    fn point_from_surface_in(&self, figure: FigureId, point: Point) -> Result<Point, ViewerError> {
        let transform = self
            .runtime
            .tree()
            .child_content_to_surface_transform(figure)
            .map_err(ViewerError::Coordinate)?
            .inverse()
            .ok_or(ViewerError::InconsistentState)?;
        let point = transform.transform_point(point);
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(ViewerError::InconsistentState);
        }
        Ok(point)
    }

    fn viewport_handle(&self) -> Result<ViewportHandle, ViewerError> {
        self.runtime
            .tree()
            .viewport_handle(self.root_layers.viewport())?
            .ok_or(ViewerError::InconsistentState)
    }

    /// Returns whether an extension or consistency failure has stopped projection.
    pub const fn is_faulted(&self) -> bool {
        self.faulted
    }

    /// Returns the latest model revision successfully projected by this Viewer.
    pub const fn applied_revision(&self) -> ModelRevision {
        self.applied_revision
    }

    /// Returns the current ordered EditPart selection.
    pub const fn selection(&self) -> &SelectionModel<EditPartId> {
        &self.selection
    }

    /// Replaces the current selection with one active model-backed part.
    pub fn replace_selection(
        &mut self,
        part: EditPartId,
    ) -> Result<Option<SelectionDelta<EditPartId>>, ViewerError> {
        self.validate_selectable(part)?;
        Ok(self.selection.replace(part))
    }

    /// Appends one active model-backed part and makes it primary.
    pub fn append_selection(
        &mut self,
        part: EditPartId,
    ) -> Result<Option<SelectionDelta<EditPartId>>, ViewerError> {
        self.validate_selectable(part)?;
        Ok(self.selection.append(part))
    }

    /// Toggles one active model-backed part.
    pub fn toggle_selection(
        &mut self,
        part: EditPartId,
    ) -> Result<Option<SelectionDelta<EditPartId>>, ViewerError> {
        self.validate_selectable(part)?;
        Ok(self.selection.toggle(part))
    }

    /// Clears visual selection and EditPart focus.
    pub fn clear_selection(&mut self) -> Option<SelectionDelta<EditPartId>> {
        self.selection.clear()
    }

    /// Validates the entire batch before replacing selection, preserving input order.
    pub fn set_selection(&mut self, parts: &[EditPartId]) -> Result<(), ViewerError> {
        for part in parts {
            self.validate_selectable(*part)?;
        }
        self.selection.clear();
        for part in parts {
            self.selection.append(*part);
        }
        Ok(())
    }

    /// Returns fully enclosed node parts in containment order using surface bounds.
    /// Contents and connections are excluded; querying does not change selection.
    pub fn parts_enclosed_by(&self, bounds: Rectangle) -> Vec<EditPartId> {
        let mut enclosed = Vec::new();
        let mut pending = self
            .parts
            .children(self.contents())
            .unwrap_or_default()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        while let Some(part) = pending.pop() {
            if self.validate_selectable(part).is_ok()
                && self.part_bounds_in_surface(part).is_some_and(|b| {
                    bounds.contains(Point::new(b.x, b.y))
                        && bounds.contains(Point::new(b.x + b.width, b.y + b.height))
                })
            {
                enclosed.push(part);
            }
            pending.extend(
                self.parts
                    .children(part)
                    .unwrap_or_default()
                    .iter()
                    .rev()
                    .copied(),
            );
        }
        enclosed
    }

    /// Sets EditPart focus independently from Figure keyboard focus.
    pub fn set_focus(&mut self, focus: Option<EditPartId>) -> Result<bool, ViewerError> {
        if let Some(part) = focus {
            self.validate_selectable(part)?;
        }
        Ok(self.selection.set_focus(focus))
    }

    /// Resolves a model identity to its current EditPart.
    pub fn part_for_model(&self, model_id: A::ModelId) -> Option<EditPartId> {
        self.model_registry.get(&model_id).copied()
    }

    /// Resolves a connection model identity to its checked connection Part.
    pub fn connection_part_for_model(&self, model_id: A::ModelId) -> Option<ConnectionPartId> {
        self.connection_projections
            .get(&model_id)
            .map(|projection| projection.part)
    }

    /// Converts an active Part into its checked connection role.
    pub fn as_connection_part(&self, part: EditPartId) -> Option<ConnectionPartId> {
        self.parts.as_connection(part).ok()
    }

    /// Returns the source and target Parts of a live connection.
    pub fn connection_endpoints(
        &self,
        connection: ConnectionPartId,
    ) -> Result<crate::ConnectionEndpoints, ViewerError> {
        self.parts
            .connection_endpoints(connection)
            .map_err(Into::into)
    }

    /// Returns the resolved route's first and last points in logical surface coordinates.
    pub fn connection_route_endpoints_in_surface(
        &self,
        connection: ConnectionPartId,
    ) -> Option<(Point, Point)> {
        let points = self.connection_route_points_in_surface(connection)?;
        let first = *points.first()?;
        let last = *points.last()?;
        Some((first, last))
    }

    /// Returns all committed route points in logical surface coordinates.
    pub fn connection_route_points_in_surface(
        &self,
        connection: ConnectionPartId,
    ) -> Option<Vec<Point>> {
        let model = self.parts.get(connection.edit_part())?.model_id()?;
        let projection = self.connection_projections.get(&model)?;
        let figure = projection.connection.figure();
        let points = self.runtime.tree().connection_route_points(figure).ok()??;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(figure)
            .ok()?;
        Some(
            points
                .iter()
                .map(|point| transform.transform_point(*point))
                .collect(),
        )
    }

    /// Returns application-defined bendpoints for a live Connection Part.
    pub fn connection_bendpoints(
        &mut self,
        connection: ConnectionPartId,
    ) -> Result<Vec<Point>, ViewerError> {
        let model_id = self
            .parts
            .get(connection.edit_part())
            .and_then(|node| node.model_id())
            .ok_or(ViewerError::InvalidPart(connection.edit_part()))?;
        let bendpoints = self
            .behaviors
            .get_mut(connection.edit_part())
            .ok_or(ViewerError::InconsistentState)?
            .connection_bendpoints(&self.model, model_id)?;
        if bendpoints
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            return Err(ViewerError::InvalidConnectionBendpoint {
                connection: format!("{model_id:?}"),
            });
        }
        Ok(bendpoints)
    }

    /// Derives GEF-style creation and move handle sites from committed route geometry.
    pub fn connection_bendpoint_handle_sites(
        &mut self,
        connection: ConnectionPartId,
    ) -> Result<Vec<BendpointHandleSite>, ViewerError> {
        let route = self
            .connection_route_points_in_surface(connection)
            .ok_or(ViewerError::InconsistentState)?;
        let routing_to_surface = self
            .runtime
            .tree()
            .child_content_to_surface_transform(self.root_layers.connection())
            .map_err(ViewerError::Coordinate)?;
        let bendpoints = self
            .connection_bendpoints(connection)?
            .into_iter()
            .map(|bendpoint| routing_to_surface.transform_point(bendpoint))
            .collect::<Vec<_>>();
        let mut sites = Vec::with_capacity(route.len().saturating_sub(1) + bendpoints.len());
        let mut bendpoint_index = 0;
        for segment in route.windows(2) {
            sites.push(BendpointHandleSite::new(
                HandleAction::new::<BendpointCreateAction>(bendpoint_index),
                segment[0] + (segment[1] - segment[0]) / 2.0,
            ));
            if bendpoints
                .get(bendpoint_index)
                .is_some_and(|bendpoint| bendpoint.approx_eq(segment[1], Precision::DEFAULT))
            {
                sites.push(BendpointHandleSite::new(
                    HandleAction::new::<BendpointMoveAction>(bendpoint_index),
                    segment[1],
                ));
                bendpoint_index += 1;
            }
        }
        Ok(sites)
    }

    /// Returns the axis-aligned surface bounds of a part's primary Figure.
    pub fn part_bounds_in_surface(&self, part: EditPartId) -> Option<Rectangle> {
        let figure = self.parts.get(part)?.primary_figure();
        let bounds = self.runtime.tree().figure_bounds(figure).ok()?;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(figure)
            .ok()?;
        let mut surface_bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        surface_bounds.transform(transform);
        Some(surface_bounds)
    }

    /// Resolves a directly registered visual to its owner part.
    pub fn part_for_visual(&self, figure: FigureId) -> Option<EditPartId> {
        self.visual_registry
            .get(&figure)
            .and_then(|owner| owner.owner_part())
    }

    /// Returns the direct Editor owner registered for a Figure.
    pub fn visual_owner(&self, figure: FigureId) -> Option<VisualOwner> {
        self.visual_registry.get(&figure).cloned()
    }

    /// Resolves a visual or its nearest registered ancestor to an owner part.
    pub fn part_for_visual_or_ancestor(&self, figure: FigureId) -> Option<EditPartId> {
        self.visual_owner_or_ancestor(figure)
            .and_then(|(_, owner)| owner.owner_part())
    }

    /// Resolves a surface point with handle priority and contents fallback.
    pub fn target_at(&self, x: f64, y: f64) -> ViewerTarget {
        let mut excluded = Vec::new();
        loop {
            let Some((figure, _)) = self
                .runtime
                .tree()
                .hit_test_excluding((x, y), excluded.iter().copied())
            else {
                return ViewerTarget::Contents(self.contents());
            };
            match self.visual_owner_or_ancestor(figure) {
                Some((_, owner)) if owner.handle_parts().is_some() => {
                    let (id, owner, action) = owner
                        .handle_parts()
                        .expect("matched handle owner must expose handle parts");
                    return ViewerTarget::Handle {
                        id,
                        owner,
                        action: action.clone(),
                    };
                }
                Some((_, owner)) if owner.is_part() => {
                    let part = owner
                        .owner_part()
                        .expect("part owner must expose its EditPart");
                    if part != self.root() && part != self.contents() {
                        return ViewerTarget::Part(part);
                    }
                    return ViewerTarget::Contents(self.contents());
                }
                Some((owner_figure, _)) => {
                    excluded.push(owner_figure);
                }
                None => return ViewerTarget::Contents(self.contents()),
            }
        }
    }

    /// Dispatches a press to Figures first, then applies Editor selection fallback if unhandled.
    pub fn dispatch_mouse_pressed(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
        modifiers: KeyModifiers,
    ) -> Result<ViewerInputOutcome, ViewerError> {
        self.ensure_ready()?;
        let dispatch = self.runtime.input().dispatch_mouse_pressed(x, y, button);
        let target = self.target_at(x, y);
        let selection = if dispatch.is_handled() || button != MouseButton::Left {
            None
        } else {
            match target {
                ViewerTarget::Part(part) => {
                    let mode = if modifiers.control || modifiers.meta {
                        SelectionMode::Toggle
                    } else if modifiers.shift {
                        SelectionMode::Append
                    } else {
                        SelectionMode::Replace
                    };
                    let delta = if mode == SelectionMode::Replace
                        && self.selection.items().contains(&part)
                    {
                        None
                    } else {
                        self.select_part(part, mode)?
                    };
                    self.selection.set_focus(self.selection.primary());
                    delta
                }
                ViewerTarget::Contents(_) => self.clear_selection(),
                ViewerTarget::Handle { .. } => None,
            }
        };
        Ok(ViewerInputOutcome {
            dispatch,
            target,
            selection,
        })
    }

    /// Dispatches a press without applying SelectionTool fallback.
    pub fn dispatch_mouse_pressed_without_selection(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> ViewerInputOutcome {
        if self.ensure_ready().is_err() {
            return ViewerInputOutcome {
                dispatch: DispatchOutcome::default(),
                target: ViewerTarget::Contents(self.contents()),
                selection: None,
            };
        }
        ViewerInputOutcome {
            dispatch: self.runtime.input().dispatch_mouse_pressed(x, y, button),
            target: self.target_at(x, y),
            selection: None,
        }
    }

    /// Dispatches pointer movement while preserving any Figure-owned capture.
    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) -> DispatchOutcome {
        if self.ensure_ready().is_err() {
            return DispatchOutcome::default();
        }
        self.runtime.input().dispatch_mouse_moved(x, y)
    }

    /// Dispatches pointer release and lets the Figure Runtime clear its capture.
    pub fn dispatch_mouse_released(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        if self.ensure_ready().is_err() {
            return DispatchOutcome::default();
        }
        self.runtime.input().dispatch_mouse_released(x, y, button)
    }

    /// Clears hover and pointer targeting after the pointer leaves the surface.
    pub fn pointer_exited(&mut self) {
        if self.ensure_ready().is_err() {
            return;
        }
        self.runtime.input().pointer_exited();
    }

    /// Adds and registers an interactive handle in the unscaled handle layer.
    pub fn add_handle_visual(
        &mut self,
        owner: EditPartId,
        figure: Box<dyn Figure>,
    ) -> Result<(HandleId, FigureId), ViewerError> {
        self.add_handle_visual_action(owner, HandleAction::new::<SelectionAction>(()), figure)
    }

    /// Adds and registers a typed handle in the unscaled handle layer.
    pub fn add_handle_visual_with_action<R>(
        &mut self,
        owner: EditPartId,
        payload: R::Payload,
        figure: Box<dyn Figure>,
    ) -> Result<(HandleId, FigureId), ViewerError>
    where
        R: HandleActionProtocol,
    {
        self.add_handle_visual_action(owner, HandleAction::new::<R>(payload), figure)
    }

    fn add_handle_visual_action(
        &mut self,
        owner: EditPartId,
        action: HandleAction,
        figure: Box<dyn Figure>,
    ) -> Result<(HandleId, FigureId), ViewerError> {
        self.ensure_ready()?;
        self.validate_selectable(owner)?;
        let figure =
            self.install_overlay_figures(vec![(self.root_layers.handles(), figure, None)])?[0];
        let id = HandleId::new(self.namespace());
        self.visual_registry
            .insert(figure, VisualOwner::handle(id, owner, action));
        Ok((id, figure))
    }

    /// Adds and registers transient feedback in a scaled or unscaled feedback layer.
    pub fn add_feedback_visual(
        &mut self,
        owner: Option<EditPartId>,
        scaled: bool,
        figure: Box<dyn Figure>,
    ) -> Result<(FeedbackId, FigureId), ViewerError> {
        self.ensure_ready()?;
        if let Some(owner) = owner {
            self.validate_selectable(owner)?;
        }
        let parent = if scaled {
            self.root_layers.scaled_feedback()
        } else {
            self.root_layers.feedback()
        };
        let figure = self.install_feedback(vec![(owner, parent, figure, None)])?[0];
        Ok((
            self.visual_registry[&figure]
                .feedback_id()
                .expect("feedback ownership"),
            figure,
        ))
    }

    /// Removes a registered handle or feedback subtree.
    pub fn remove_overlay_visual(&mut self, figure: FigureId) -> Result<bool, ViewerError> {
        self.ensure_ready()?;
        match self.visual_registry.get(&figure) {
            Some(owner) if owner.is_overlay() => {
                self.remove_overlay_visuals(&[figure])?;
                Ok(true)
            }
            Some(_) | None => Ok(false),
        }
    }

    /// Removes a complete overlay batch after validating ownership up front.
    pub fn remove_overlay_visuals(&mut self, figures: &[FigureId]) -> Result<(), ViewerError> {
        self.ensure_ready()?;
        let mut seen = HashSet::new();
        for (index, figure) in figures.iter().copied().enumerate() {
            let reject = |reason| ViewerError::InvalidOverlayBatch {
                index,
                figure,
                reason,
            };
            if !seen.insert(figure) {
                return Err(reject("duplicate Figure identity"));
            }
            let Some(owner) = self
                .visual_registry
                .get(&figure)
                .filter(|owner| owner.is_overlay())
            else {
                return Err(reject("Figure is not owned Overlay"));
            };
            if self.runtime.tree().is_attached(figure) != Ok(true) {
                return Err(reject("foreign or retired Figure identity"));
            }
            if figures[..index].iter().copied().any(|other| {
                self.runtime.tree().is_ancestor_of(other, figure) == Ok(true)
                    || self.runtime.tree().is_ancestor_of(figure, other) == Ok(true)
            }) {
                return Err(reject("ancestor and descendant overlap"));
            }
            for descendant in self.collect_creation_figures(&[figure]).into_iter().skip(1) {
                if self
                    .visual_registry
                    .get(&descendant)
                    .is_some_and(|child_owner| {
                        !child_owner.is_overlay() || child_owner.owner_part() != owner.owner_part()
                    })
                {
                    return Err(reject("subtree contains resources of another owner"));
                }
            }
        }
        let acquired = self.collect_creation_figures(figures);
        for figure in figures.iter().copied() {
            let result = catch_unwind(AssertUnwindSafe(|| self.runtime.dispose_subtree(figure)));
            if !matches!(result, Ok(Ok(()))) {
                let mut report = ProjectionCleanup {
                    original: match &result {
                        Ok(Err(error)) => error.to_string(),
                        Err(_) => "Figure disposal panic".to_owned(),
                        _ => unreachable!(),
                    },
                    ..Default::default()
                };
                self.record_figure_progress(&acquired, &mut report);
                self.visual_registry
                    .retain(|id, _| self.runtime.tree().is_attached(*id) == Ok(true));
                self.faulted = true;
                self.drop_direct_text_edit_into(&mut report);
                self.last_cleanup = Some(Arc::new(report));
                return match result {
                    Ok(Err(error)) => Err(error.into()),
                    Err(payload) => resume_unwind(payload),
                    _ => unreachable!(),
                };
            }
            self.visual_registry
                .retain(|id, _| self.runtime.tree().is_attached(*id) == Ok(true));
        }
        Ok(())
    }

    fn add_feedback_batch(
        &mut self,
        contributions: Vec<(Option<EditPartId>, FeedbackVisual)>,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.ensure_ready()?;
        let mut inputs = Vec::with_capacity(contributions.len());
        for (owner, feedback) in contributions {
            if let Some(owner) = owner {
                self.validate_selectable(owner)?;
            }
            let (figure, scaled, style) = feedback.into_parts();
            let parent = if scaled {
                self.root_layers.scaled_feedback()
            } else {
                self.root_layers.feedback()
            };
            inputs.push((owner, parent, figure, style));
        }
        self.install_feedback(inputs)
    }

    fn install_feedback(
        &mut self,
        inputs: Vec<FeedbackInstallInput>,
    ) -> Result<Vec<FigureId>, ViewerError> {
        let owners: Vec<_> = inputs.iter().map(|(owner, _, _, _)| *owner).collect();
        let figures = self.install_overlay_figures(
            inputs
                .into_iter()
                .map(|(_, parent, figure, style)| (parent, figure, style))
                .collect(),
        )?;
        for (figure, owner) in figures.iter().copied().zip(owners) {
            let id = FeedbackId::new(self.namespace());
            self.visual_registry
                .insert(figure, VisualOwner::feedback(id, owner));
        }
        Ok(figures)
    }

    fn install_overlay_figures(
        &mut self,
        inputs: Vec<(FigureId, Box<dyn Figure>, Option<FigureStyle>)>,
    ) -> Result<Vec<FigureId>, ViewerError> {
        let parents: HashSet<_> = inputs.iter().map(|(parent, _, _)| *parent).collect();
        let before: HashSet<_> = parents
            .iter()
            .flat_map(|parent| self.runtime.tree().child_order(*parent).unwrap_or_default())
            .collect();
        let result = catch_unwind(AssertUnwindSafe(|| self.runtime.add_figures(inputs)));
        match result {
            Ok(Ok(figures)) => Ok(figures),
            failed => {
                let roots: Vec<_> = parents
                    .iter()
                    .flat_map(|parent| self.runtime.tree().child_order(*parent).unwrap_or_default())
                    .filter(|figure| !before.contains(figure))
                    .collect();
                if roots.is_empty()
                    && !self.runtime.is_faulted()
                    && let Ok(Err(error)) = failed
                {
                    return Err(error.into());
                }
                let acquired = self.collect_creation_figures(&roots);
                let mut report = ProjectionCleanup {
                    original: match &failed {
                        Ok(Err(error)) => error.to_string(),
                        Err(_) => "Overlay installation panic".to_owned(),
                        _ => unreachable!(),
                    },
                    ..Default::default()
                };
                self.cleanup_created_figures(&roots, &mut report);
                self.record_figure_progress(&acquired, &mut report);
                self.faulted = true;
                self.drop_direct_text_edit_into(&mut report);
                let report = Arc::new(report);
                self.last_cleanup = Some(Arc::clone(&report));
                match failed {
                    Ok(Err(error)) => Err(ViewerError::ProjectionCleanup {
                        original: Box::new(error.into()),
                        report,
                    }),
                    Err(payload) => resume_unwind(payload),
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Returns a snapshot of the active direct text edit session.
    pub fn direct_text_edit(&self) -> Option<&DirectTextEditState> {
        self.direct_text_edit.as_ref().map(|active| &active.state)
    }

    /// Drains platform text-input operations emitted since the previous call.
    pub fn take_text_input_effects(&mut self) -> Vec<TextInputEffect> {
        std::mem::take(&mut self.text_input_effects)
    }

    pub(crate) fn start_direct_text_edit(
        &mut self,
        metadata: &RequestMetadata,
        request: &DirectTextEditRequest,
    ) -> Result<DirectTextEditSessionId, ViewerError>
    where
        A: 'static,
    {
        self.with_extension_fault_boundary(|viewer| {
            viewer.start_direct_text_edit_inner(metadata, request)
        })
    }

    fn start_direct_text_edit_inner(
        &mut self,
        metadata: &RequestMetadata,
        request: &DirectTextEditRequest,
    ) -> Result<DirectTextEditSessionId, ViewerError>
    where
        A: 'static,
    {
        self.ensure_ready()?;
        if self.direct_text_edit.is_some() {
            return Err(DirectTextEditError::SessionAlreadyActive.into());
        }
        self.validate_selectable(request.source())
            .map_err(|_| DirectTextEditError::ForeignOrRetiredPart)?;
        let mut plans = self
            .dispatch_request_inner::<StartDirectTextEdit>(metadata, request)?
            .into_contributions();
        let plan = plans.pop().ok_or(DirectTextEditError::UnsupportedFeature)?;
        let descriptor = plan.descriptor().clone();
        if descriptor.feature() != request.feature() {
            return Err(DirectTextEditError::UnsupportedFeature.into());
        }
        if descriptor.source_revision() != self.model.revision() {
            return Err(DirectTextEditError::StaleSourceRevision.into());
        }
        let sequence = self.next_direct_text_edit_session;
        self.next_direct_text_edit_session = sequence
            .checked_add(1)
            .ok_or(DirectTextEditError::SessionIdentityExhausted)?;
        let session = DirectTextEditSessionId::new(self.namespace(), sequence);
        let state = DirectTextEditState::new(session, request.source(), &descriptor);
        let mut active = ActiveDirectTextEdit {
            state,
            plan,
            feedback: Vec::new(),
            text_feedback: self.root_layers.root(),
            horizontal_scroll: 0.0,
            caret_feedback: None,
            caret_visible: false,
        };
        let projection = active.plan.feedback(&active.state, &self.model)?;
        let attached = self.attach_direct_text_feedback(
            request.source(),
            &active.state,
            projection,
            active.horizontal_scroll,
        )?;
        active.feedback = attached.figures;
        active.text_feedback = attached.text_feedback;
        active.horizontal_scroll = attached.horizontal_scroll;
        active.caret_feedback = attached.caret_feedback;
        active.caret_visible = attached.caret_feedback.is_some();
        self.text_input_effects.push(TextInputEffect::Acquire {
            session,
            purpose: match active.state.mode() {
                TextEditMode::SingleLine => TextInputPurpose::SingleLine,
                TextEditMode::Multiline => TextInputPurpose::Multiline,
            },
            area: attached.area,
        });
        self.direct_text_edit = Some(active);
        self.reset_direct_text_blink();
        Ok(session)
    }

    pub(crate) fn set_direct_text_selection(
        &mut self,
        selection: FlowTextRange,
    ) -> Result<(), ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        state.set_selection(selection)?;
        self.replace_direct_text_state(state)
    }

    pub(crate) fn hit_test_direct_text(
        &mut self,
        point: Point,
        extend: bool,
    ) -> Result<FlowTextPosition, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let position = self
            .runtime
            .text()
            .flow_hit_test(active.text_feedback, point)
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?;
        let mut state = active.state.clone();
        state.set_selection(if extend {
            FlowTextRange::new(state.selection().anchor(), position)
        } else {
            FlowTextRange::new(position, position)
        })?;
        self.replace_direct_text_state(state)?;
        Ok(position)
    }

    pub(crate) fn insert_direct_text(&mut self, text: &str) -> Result<bool, ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        let changed = if state.composition().is_some() {
            state.commit_preedit(text)?
        } else {
            state.replace_selection(text)?
        };
        if changed {
            self.replace_direct_text_state(state)?;
        }
        Ok(changed)
    }

    pub(crate) fn synchronize_direct_text_input(
        &mut self,
        snapshot: &TextInputSnapshot,
    ) -> Result<bool, ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        let changed = state.synchronize_input(snapshot)?;
        if changed {
            self.replace_direct_text_state(state)?;
        }
        Ok(changed)
    }

    pub(crate) fn set_direct_text_preedit(
        &mut self,
        text: &str,
        selection: Option<std::ops::Range<usize>>,
    ) -> Result<bool, ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        let changed = state.set_preedit(text, selection)?;
        self.replace_direct_text_state(state)?;
        Ok(changed)
    }

    pub(crate) fn cancel_direct_text_preedit(&mut self) -> Result<bool, ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        let changed = state.cancel_preedit();
        if changed {
            self.replace_direct_text_state(state)?;
        }
        Ok(changed)
    }

    pub(crate) fn select_all_direct_text(&mut self) -> Result<(), ViewerError> {
        let mut state = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?
            .state
            .clone();
        state.select_all();
        self.replace_direct_text_state(state)
    }

    /// Returns the active draft caret rectangle in logical surface coordinates.
    pub fn direct_text_caret_geometry(&mut self) -> Result<CaretGeometry, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let caret = active.state.caret_position()?;
        self.runtime
            .text()
            .flow_caret_geometry(active.text_feedback, caret)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }

    /// Returns active draft selection quads in logical surface coordinates.
    pub fn direct_text_selection_geometry(&mut self) -> Result<Vec<SelectionQuad>, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let viewport = self.direct_text_viewport_in_surface(active.text_feedback)?;
        let selection = self
            .runtime
            .text()
            .flow_selection_geometry(active.text_feedback, active.state.selection())
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?
            .into_iter()
            .filter_map(|quad| {
                quad.bounds()
                    .intersection(viewport)
                    .map(|bounds| SelectionQuad::new(bounds, quad.line_index()))
            })
            .collect();
        Ok(selection)
    }

    /// Returns the surface bounds for one UTF-8 range in the active draft.
    pub fn direct_text_range_bounds(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> Result<Rectangle, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        if range.start > range.end {
            return Err(DirectTextEditError::InvalidTextPosition.into());
        }
        let start = position_from_offset(active.state.draft(), range.start)?;
        let end = position_from_offset(active.state.draft(), range.end)?;
        let bounds = self
            .runtime
            .text()
            .flow_selection_geometry(active.text_feedback, FlowTextRange::new(start, end))
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?
            .into_iter()
            .map(|quad| quad.bounds())
            .reduce(Rectangle::union);
        if let Some(bounds) = bounds {
            return Ok(bounds);
        }

        let caret = self
            .runtime
            .text()
            .flow_caret_geometry(active.text_feedback, start)
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?
            .bounds();
        Ok(Rectangle::new(
            caret.x,
            caret.y,
            caret.width.max(DIRECT_TEXT_CARET_MINIMUM_WIDTH),
            caret.height,
        ))
    }

    pub(crate) fn direct_text_feedback_contains(&self, point: Point) -> bool {
        let Some(active) = self.direct_text_edit.as_ref() else {
            return false;
        };
        let Ok(bounds) = self.runtime.tree().figure_bounds(active.text_feedback) else {
            return false;
        };
        let Ok(transform) = self
            .runtime
            .tree()
            .local_to_surface_transform(active.text_feedback)
        else {
            return false;
        };
        let mut surface_bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        surface_bounds.transform(transform);
        surface_bounds.contains(point)
    }

    /// Rebuilds direct-edit feedback and emits its candidate area after a transform change.
    pub fn synchronize_direct_text_input_area(&mut self) -> Result<bool, ViewerError> {
        self.with_extension_fault_boundary(Self::synchronize_direct_text_input_area_inner)
    }

    fn synchronize_direct_text_input_area_inner(&mut self) -> Result<bool, ViewerError> {
        let Some(active) = self.direct_text_edit.as_mut() else {
            return Ok(false);
        };
        let projection = active.plan.feedback(&active.state, &self.model)?;
        let mut active = self
            .direct_text_edit
            .take()
            .expect("direct text edit must remain active after feedback");
        let attached = match self.attach_direct_text_feedback(
            active.state.source(),
            &active.state,
            projection,
            active.horizontal_scroll,
        ) {
            Ok(attached) => attached,
            Err(error) => {
                self.direct_text_edit = Some(active);
                return Err(error);
            }
        };
        if !active.caret_visible
            && let Some(caret) = attached.caret_feedback
            && let Err(error) = self.runtime.figure(caret)?.set_visible(false)
        {
            let _ = self.remove_direct_text_feedback(&attached.figures);
            self.direct_text_edit = Some(active);
            return Err(error.into());
        }
        if let Err(error) = self.remove_direct_text_feedback(&active.feedback) {
            let _ = self.remove_direct_text_feedback(&attached.figures);
            self.direct_text_edit = Some(active);
            return Err(error);
        }
        active.feedback = attached.figures;
        active.text_feedback = attached.text_feedback;
        active.horizontal_scroll = attached.horizontal_scroll;
        active.caret_feedback = attached.caret_feedback;
        active.caret_visible &= attached.caret_feedback.is_some();
        self.text_input_effects.push(TextInputEffect::SetArea {
            session: active.state.session(),
            area: attached.area,
        });
        self.direct_text_edit = Some(active);
        Ok(true)
    }

    fn direct_text_viewport_in_surface(
        &self,
        text_feedback: FigureId,
    ) -> Result<Rectangle, ViewerError> {
        let bounds = self.runtime.tree().figure_bounds(text_feedback)?;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(text_feedback)
            .map_err(ViewerError::Coordinate)?;
        let mut viewport = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        viewport.transform(transform);
        Ok(viewport)
    }

    pub(crate) fn move_direct_text(
        &mut self,
        movement: TextMovement,
        extend: bool,
    ) -> Result<FlowTextPosition, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let position = self
            .runtime
            .text()
            .flow_move_position(
                active.text_feedback,
                active.state.selection().focus(),
                movement,
            )
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?;
        let mut state = active.state.clone();
        state.set_selection(if extend {
            FlowTextRange::new(state.selection().anchor(), position)
        } else {
            FlowTextRange::new(position, position)
        })?;
        self.replace_direct_text_state(state)?;
        Ok(position)
    }

    pub(crate) fn delete_direct_text(
        &mut self,
        deletion: crate::TextDelete,
    ) -> Result<bool, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let mut state = active.state.clone();
        let anchor = position_to_offset(state.draft(), state.selection().anchor())?;
        let focus = position_to_offset(state.draft(), state.selection().focus())?;
        let changed = if anchor != focus {
            state.replace_selection("")?
        } else {
            let moved = self
                .runtime
                .text()
                .flow_move_position(
                    active.text_feedback,
                    state.selection().focus(),
                    deletion.movement(),
                )
                .map_err(|_| DirectTextEditError::InvalidTextPosition)?;
            if deletion.is_backward() {
                state.delete_range(moved, state.selection().focus())?
            } else {
                state.delete_range(state.selection().focus(), moved)?
            }
        };
        if changed {
            self.replace_direct_text_state(state)?;
        }
        Ok(changed)
    }

    pub(crate) fn cancel_direct_text_edit(&mut self) -> Result<bool, ViewerError> {
        let Some(active) = self.direct_text_edit.as_ref() else {
            return Ok(false);
        };
        let feedback = active.feedback.clone();
        self.remove_direct_text_feedback(&feedback)?;
        let active = self
            .direct_text_edit
            .take()
            .expect("direct text edit must remain active after feedback cleanup");
        self.direct_text_blink_deadline = None;
        self.text_input_effects.push(TextInputEffect::Release {
            session: active.state.session(),
        });
        Ok(true)
    }

    pub(crate) fn prepare_direct_text_accept(
        &mut self,
    ) -> Result<PreparedDirectTextEdit<A>, ViewerError> {
        self.with_extension_fault_boundary(Self::prepare_direct_text_accept_inner)
    }

    fn prepare_direct_text_accept_inner(
        &mut self,
    ) -> Result<PreparedDirectTextEdit<A>, ViewerError> {
        let active = self
            .direct_text_edit
            .as_mut()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        if self
            .parts
            .get(active.state.source())
            .is_none_or(|part| !part.is_active())
        {
            let feedback = active.feedback.clone();
            let _ = active;
            self.direct_text_blink_deadline = None;
            self.remove_direct_text_feedback(&feedback)?;
            self.direct_text_edit = None;
            return Err(DirectTextEditError::ForeignOrRetiredPart.into());
        }
        if active.state.source_revision() != self.model.revision() {
            return Err(DirectTextEditError::StaleSourceRevision.into());
        }
        if active.state.composition().is_some() {
            return Err(DirectTextEditError::ActiveComposition.into());
        }
        active.plan.validate(&active.state, &self.model)?;
        let command = if active.state.is_changed() {
            Some(active.plan.command(&active.state, &self.model)?)
        } else {
            None
        };
        let feedback = active.feedback.clone();
        let _ = active;
        self.remove_direct_text_feedback(&feedback)?;
        let mut active = self
            .direct_text_edit
            .take()
            .expect("direct text edit must remain active after feedback cleanup");
        active.feedback.clear();
        active.caret_feedback = None;
        active.caret_visible = false;
        self.direct_text_blink_deadline = None;
        self.text_input_effects.push(TextInputEffect::Release {
            session: active.state.session(),
        });
        Ok(PreparedDirectTextEdit { active, command })
    }

    pub(crate) fn restore_direct_text_edit(
        &mut self,
        prepared: PreparedDirectTextEdit<A>,
    ) -> Result<(), ViewerError> {
        self.with_extension_fault_boundary(move |viewer| {
            viewer.restore_direct_text_edit_inner(prepared)
        })
    }

    fn restore_direct_text_edit_inner(
        &mut self,
        mut prepared: PreparedDirectTextEdit<A>,
    ) -> Result<(), ViewerError> {
        let projection = prepared
            .active
            .plan
            .feedback(&prepared.active.state, &self.model)?;
        let attached = self.attach_direct_text_feedback(
            prepared.active.state.source(),
            &prepared.active.state,
            projection,
            prepared.active.horizontal_scroll,
        )?;
        prepared.active.feedback = attached.figures;
        prepared.active.text_feedback = attached.text_feedback;
        prepared.active.horizontal_scroll = attached.horizontal_scroll;
        prepared.active.caret_feedback = attached.caret_feedback;
        prepared.active.caret_visible = attached.caret_feedback.is_some();
        self.text_input_effects.push(TextInputEffect::Acquire {
            session: prepared.active.state.session(),
            purpose: match prepared.active.state.mode() {
                TextEditMode::SingleLine => TextInputPurpose::SingleLine,
                TextEditMode::Multiline => TextInputPurpose::Multiline,
            },
            area: attached.area,
        });
        self.direct_text_edit = Some(prepared.active);
        self.reset_direct_text_blink();
        Ok(())
    }

    fn replace_direct_text_state(&mut self, state: DirectTextEditState) -> Result<(), ViewerError> {
        self.with_extension_fault_boundary(move |viewer| {
            viewer.replace_direct_text_state_inner(state)
        })
    }

    fn replace_direct_text_state_inner(
        &mut self,
        state: DirectTextEditState,
    ) -> Result<(), ViewerError> {
        let active = self
            .direct_text_edit
            .as_mut()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let previous_state = std::mem::replace(&mut active.state, state);
        let projection = match active.plan.feedback(&active.state, &self.model) {
            Ok(projection) => projection,
            Err(error) => {
                active.state = previous_state;
                return Err(error.into());
            }
        };
        let mut active = self
            .direct_text_edit
            .take()
            .expect("direct text edit must remain active after feedback");
        let attached = match self.attach_direct_text_feedback(
            active.state.source(),
            &active.state,
            projection,
            active.horizontal_scroll,
        ) {
            Ok(attached) => attached,
            Err(error) => {
                active.state = previous_state;
                self.direct_text_edit = Some(active);
                return Err(error);
            }
        };
        if let Err(error) = self.remove_direct_text_feedback(&active.feedback) {
            let _ = self.remove_direct_text_feedback(&attached.figures);
            active.state = previous_state;
            self.direct_text_edit = Some(active);
            return Err(error);
        }
        active.feedback = attached.figures;
        active.text_feedback = attached.text_feedback;
        active.horizontal_scroll = attached.horizontal_scroll;
        active.caret_feedback = attached.caret_feedback;
        active.caret_visible = attached.caret_feedback.is_some();
        self.text_input_effects.push(TextInputEffect::SetArea {
            session: active.state.session(),
            area: attached.area,
        });
        self.direct_text_edit = Some(active);
        self.reset_direct_text_blink();
        Ok(())
    }

    fn attach_direct_text_feedback(
        &mut self,
        owner: EditPartId,
        state: &DirectTextEditState,
        feedback: DirectTextFeedback,
        previous_horizontal_scroll: f64,
    ) -> Result<AttachedDirectTextFeedback, ViewerError> {
        let (visuals, text_visual) = feedback.into_parts();
        let mut figures = self.add_feedback_batch(
            visuals
                .into_iter()
                .map(|visual| (Some(owner), visual))
                .collect(),
        )?;
        let text_feedback = figures
            .get(text_visual)
            .copied()
            .ok_or(DirectTextEditError::InvalidFeedbackTarget)?;
        if let Err(error) =
            self.runtime
                .figure(text_feedback)?
                .update_component(SetDirectTextViewport(TextFlowViewport::clipped(
                    Point::new(-previous_horizontal_scroll, 0.0),
                )))
        {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(direct_text_viewport_error(error));
        }
        self.runtime.frames().stabilize_for_query()?;
        if self.runtime.text().flow_layout(text_feedback).is_err() {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        let caret = match state.caret_position() {
            Ok(caret) => caret,
            Err(error) => {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(error.into());
            }
        };
        let Ok(text_bounds) = self.runtime.tree().figure_bounds(text_feedback) else {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        };
        if text_bounds.is_empty() {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        let local_caret = match self
            .runtime
            .text()
            .flow_local_caret_geometry(text_feedback, caret)
        {
            Ok(caret) => caret.bounds(),
            Err(_) => {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(DirectTextEditError::InvalidFeedbackTarget.into());
            }
        };
        let content_width = self
            .runtime
            .text()
            .flow_layout(text_feedback)
            .map(|layout| f64::from(layout.full_width()))
            .map_err(|_| DirectTextEditError::InvalidFeedbackTarget)?;
        let natural_caret = Rectangle::new(
            local_caret.x + previous_horizontal_scroll,
            local_caret.y,
            local_caret.width,
            local_caret.height,
        );
        let content_width = content_width
            .max(natural_caret.x + natural_caret.width.max(DIRECT_TEXT_CARET_MINIMUM_WIDTH));
        let horizontal_scroll = reveal_horizontal_caret(
            previous_horizontal_scroll,
            natural_caret,
            text_bounds.width,
            content_width,
        );
        if horizontal_scroll != previous_horizontal_scroll {
            if let Err(error) =
                self.runtime
                    .figure(text_feedback)?
                    .update_component(SetDirectTextViewport(TextFlowViewport::clipped(
                        Point::new(-horizontal_scroll, 0.0),
                    )))
            {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(direct_text_viewport_error(error));
            }
            // Publish the source viewport offset before reading immutable text geometry.
            self.runtime.frames().stabilize_for_query()?;
        }
        let Ok(transform) = self
            .runtime
            .tree()
            .local_to_surface_transform(text_feedback)
        else {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        };
        let mut edit_viewport = Rectangle::new(0.0, 0.0, text_bounds.width, text_bounds.height);
        edit_viewport.transform(transform);
        let area = match self
            .runtime
            .text()
            .flow_caret_geometry(text_feedback, caret)
        {
            Ok(caret) => {
                let bounds = caret.bounds();
                Rectangle::new(
                    bounds.x,
                    bounds.y,
                    bounds.width.max(DIRECT_TEXT_CARET_MINIMUM_WIDTH),
                    bounds.height,
                )
                .intersection(edit_viewport)
                .unwrap_or_else(|| {
                    Rectangle::new(
                        edit_viewport.x,
                        edit_viewport.y,
                        DIRECT_TEXT_CARET_MINIMUM_WIDTH.min(edit_viewport.width),
                        edit_viewport.height,
                    )
                })
            }
            Err(_) => {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(DirectTextEditError::InvalidFeedbackTarget.into());
            }
        };
        let selection = match state.composition() {
            Some(_) => Vec::new(),
            None => match self
                .runtime
                .text()
                .flow_selection_geometry(text_feedback, state.selection())
            {
                Ok(selection) => selection,
                Err(_) => {
                    let _ = self.remove_direct_text_feedback(&figures);
                    return Err(DirectTextEditError::InvalidFeedbackTarget.into());
                }
            },
        };
        let preedit = match state.composition() {
            Some(composition) => {
                match self
                    .runtime
                    .text()
                    .flow_selection_geometry(text_feedback, composition.range())
                {
                    Ok(preedit) => preedit,
                    Err(_) => {
                        let _ = self.remove_direct_text_feedback(&figures);
                        return Err(DirectTextEditError::InvalidFeedbackTarget.into());
                    }
                }
            }
            None => Vec::new(),
        };

        for quad in selection {
            let Some(bounds) = quad.bounds().intersection(edit_viewport) else {
                continue;
            };
            if let Err(error) = self.attach_direct_text_decoration(
                owner,
                bounds,
                DIRECT_TEXT_SELECTION_COLOR,
                &mut figures,
            ) {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(error);
            }
        }
        for quad in preedit {
            let Some(bounds) = quad.bounds().intersection(edit_viewport) else {
                continue;
            };
            let underline = Rectangle::new(
                bounds.x,
                bounds.y + bounds.height - DIRECT_TEXT_PREEDIT_THICKNESS,
                bounds.width,
                DIRECT_TEXT_PREEDIT_THICKNESS,
            );
            if let Err(error) = self.attach_direct_text_decoration(
                owner,
                underline,
                DIRECT_TEXT_PREEDIT_COLOR,
                &mut figures,
            ) {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(error);
            }
        }
        let caret_visible = state
            .composition()
            .is_none_or(|composition| composition.selection().is_some());
        let caret_feedback = if caret_visible {
            let caret_bounds = Rectangle::new(
                area.x,
                area.y,
                area.width.max(DIRECT_TEXT_CARET_MINIMUM_WIDTH),
                area.height,
            );
            match self.attach_direct_text_decoration(
                owner,
                caret_bounds,
                DIRECT_TEXT_CARET_COLOR,
                &mut figures,
            ) {
                Ok(caret) => Some(caret),
                Err(error) => {
                    let _ = self.remove_direct_text_feedback(&figures);
                    return Err(error);
                }
            }
        } else {
            None
        };
        Ok(AttachedDirectTextFeedback {
            figures,
            text_feedback,
            caret_feedback,
            area,
            horizontal_scroll,
        })
    }

    fn attach_direct_text_decoration(
        &mut self,
        owner: EditPartId,
        bounds: Rectangle,
        color: Color,
        figures: &mut Vec<FigureId>,
    ) -> Result<FigureId, ViewerError> {
        let (_, figure) = self.add_feedback_visual(
            Some(owner),
            false,
            Box::new(RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                color,
            )),
        )?;
        figures.push(figure);
        Ok(figure)
    }

    fn reset_direct_text_blink(&mut self) {
        self.direct_text_blink_deadline = self
            .direct_text_edit
            .as_ref()
            .and_then(|active| active.caret_feedback)
            .and_then(|_| {
                self.current_time
                    .as_micros()
                    .checked_add(DIRECT_TEXT_CARET_BLINK_INTERVAL_MICROS)
                    .map(MonotonicTime::from_micros)
            });
    }

    fn remove_direct_text_feedback(&mut self, figures: &[FigureId]) -> Result<(), ViewerError> {
        self.remove_overlay_visuals(figures)
    }

    fn select_part(
        &mut self,
        part: EditPartId,
        mode: SelectionMode,
    ) -> Result<Option<SelectionDelta<EditPartId>>, ViewerError> {
        self.validate_selectable(part)?;
        Ok(match mode {
            SelectionMode::Replace => self.selection.replace(part),
            SelectionMode::Append => self.selection.append(part),
            SelectionMode::Toggle => self.selection.toggle(part),
        })
    }

    fn validate_selectable(&self, part: EditPartId) -> Result<(), ViewerError> {
        if part == self.root()
            || self
                .parts
                .get(part)
                .is_none_or(|node| !node.is_active() || node.model_id().is_none())
        {
            return Err(ViewerError::InvalidPart(part));
        }
        Ok(())
    }

    fn visual_owner_or_ancestor(&self, figure: FigureId) -> Option<(FigureId, VisualOwner)> {
        if figure.namespace() != self.runtime.tree().namespace()
            || self.runtime.tree().is_attached(figure) != Ok(true)
        {
            return None;
        }
        if let Some(owner) = self.visual_registry.get(&figure).cloned() {
            return Some((figure, owner));
        }
        self.runtime
            .tree()
            .ancestor_ids(figure)
            .ok()?
            .into_iter()
            .find_map(|ancestor| {
                self.visual_registry
                    .get(&ancestor)
                    .cloned()
                    .map(|owner| (ancestor, owner))
            })
    }

    fn resolve_policy_target<R>(
        &mut self,
        candidate: EditPartId,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Option<EditPartId>, ViewerError>
    where
        R: RequestProtocol<A>,
    {
        self.validate_selectable(candidate)?;
        let host = self.policy_host(candidate)?;
        let target = self
            .policies
            .resolve_target::<R>(host, metadata, payload, &self.model)?;
        if let Some(target) = target {
            self.validate_selectable(target)?;
        }
        Ok(target)
    }

    fn resolve_policy_targets<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Vec<EditPartId>, ViewerError>
    where
        R: RequestProtocol<A>,
    {
        let mut seen_sources = HashSet::new();
        let mut seen_targets = HashSet::new();
        let mut targets = Vec::new();
        for &source in metadata.source_parts() {
            if !seen_sources.insert(source) {
                return Err(
                    PolicyError::operation("request contains a duplicate source part").into(),
                );
            }
            if let Some(target) = self.resolve_policy_target::<R>(source, metadata, payload)?
                && seen_targets.insert(target)
            {
                targets.push(target);
            }
        }
        Ok(targets)
    }

    /// Dispatches one typed request through the installed policy registry.
    pub fn dispatch_request<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<RequestResolution<R::Contribution>, ViewerError>
    where
        A: 'static,
        R: RequestProtocol<A>,
    {
        self.with_extension_fault_boundary(|viewer| {
            viewer.dispatch_request_inner::<R>(metadata, payload)
        })
    }

    fn dispatch_request_inner<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<RequestResolution<R::Contribution>, ViewerError>
    where
        A: 'static,
        R: RequestProtocol<A>,
    {
        let mut contributions = Vec::new();
        for target in self.resolve_policy_targets::<R>(metadata, payload)? {
            let host = self.policy_host(target)?;
            contributions.extend(self.policies.contributions::<R>(
                host,
                metadata,
                payload,
                &self.model,
            )?);
        }
        Ok(R::reduce(contributions)?)
    }

    fn dispatch_request_erased_inner(
        &mut self,
        key: ErasedRequestKey,
        metadata: &RequestMetadata,
        payload: &dyn Any,
    ) -> Result<ErasedRequestContributions, ViewerError> {
        let mut seen_sources = HashSet::new();
        let mut seen_targets = HashSet::new();
        let mut targets = Vec::new();
        for &source in metadata.source_parts() {
            if !seen_sources.insert(source) {
                return Err(
                    PolicyError::operation("request contains a duplicate source part").into(),
                );
            }
            self.validate_selectable(source)?;
            let host = self.policy_host(source)?;
            let target =
                self.policies
                    .resolve_target_erased(host, key, metadata, payload, &self.model)?;
            if let Some(target) = target {
                self.validate_selectable(target)?;
                if seen_targets.insert(target) {
                    targets.push(target);
                }
            }
        }
        let mut contributions = Vec::new();
        for target in targets {
            let host = self.policy_host(target)?;
            contributions.extend(
                self.policies
                    .contributions_erased(host, key, metadata, payload, &self.model)?
                    .into_iter()
                    .map(|contribution| (target, contribution)),
            );
        }
        Ok(contributions)
    }

    /// Resolves command contributions for one typed request.
    pub fn command_for<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>
    where
        A: 'static,
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        let mut commands = self
            .dispatch_request::<R>(metadata, payload)?
            .into_contributions()
            .into_iter()
            .filter_map(|contribution| contribution.into_parts().0)
            .collect::<Vec<_>>();
        Ok(match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => {
                let mut compound = CompoundCommand::new(R::key().name());
                for command in commands {
                    compound.push(command);
                }
                Some(Box::new(compound))
            }
        })
    }

    /// Resolves and installs feedback contributions for one typed request.
    pub fn show_feedback<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Vec<FigureId>, ViewerError>
    where
        A: 'static,
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        self.with_extension_fault_boundary(|viewer| {
            viewer.show_feedback_inner::<R>(metadata, payload)
        })
    }

    fn show_feedback_inner<R>(
        &mut self,
        metadata: &RequestMetadata,
        payload: &R::Payload,
    ) -> Result<Vec<FigureId>, ViewerError>
    where
        A: 'static,
        R: RequestProtocol<A, Contribution = EditingContribution<A>>,
    {
        let mut contributions = Vec::new();
        for target in self.resolve_policy_targets::<R>(metadata, payload)? {
            let host = self.policy_host(target)?;
            for contribution in
                self.policies
                    .contributions::<R>(host, metadata, payload, &self.model)?
            {
                contributions.extend(
                    contribution
                        .into_parts()
                        .1
                        .into_iter()
                        .map(|feedback| (target, feedback)),
                );
            }
        }
        self.add_feedback_batch(
            contributions
                .into_iter()
                .map(|(owner, feedback)| (Some(owner), feedback))
                .collect(),
        )
    }

    fn with_extension_fault_boundary<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ViewerError>,
    ) -> Result<T, ViewerError> {
        self.ensure_ready()?;
        match catch_unwind(AssertUnwindSafe(|| operation(self))) {
            Ok(result) => result,
            Err(payload) => {
                self.mark_faulted_with_cause(&failure::panic_message(payload.as_ref()));
                resume_unwind(payload)
            }
        }
    }

    fn mark_faulted(&mut self) {
        self.mark_faulted_with_cause("Viewer extension fault");
    }

    fn mark_faulted_with_cause(&mut self, original: &str) {
        self.faulted = true;
        self.force_drop_direct_text_edit(original);
    }

    pub(crate) fn fault_extension(&mut self) {
        self.mark_faulted();
    }

    /// Resolves one unambiguous source policy into a connection-creation plan.
    pub fn start_connection_creation(
        &mut self,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ViewerError>
    where
        A: 'static,
    {
        let metadata = RequestMetadata::new(request.revision(), vec![request.source()]);
        self.with_extension_fault_boundary(|viewer| {
            viewer.start_connection_creation_inner(&metadata, request)
        })
    }

    fn start_connection_creation_inner(
        &mut self,
        metadata: &RequestMetadata,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ViewerError>
    where
        A: 'static,
    {
        self.connection_endpoint_host(request.source())?;
        Ok(self
            .dispatch_request_inner::<CreateConnection>(metadata, request)?
            .into_contributions()
            .pop())
    }

    /// Replaces connection-creation feedback for the latest target candidate.
    pub fn show_connection_feedback(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.with_extension_fault_boundary(|viewer| {
            viewer.show_connection_feedback_inner(plan, request)
        })
    }

    fn show_connection_feedback_inner(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        let source = self.connection_endpoint_host(request.source())?;
        let target = self.valid_connection_target(plan, source, request)?;
        let route = self.preview_connection_feedback_route(
            None,
            None,
            Some(source.part()),
            source.model(),
            target.map(PolicyHost::part),
            target.map(PolicyHost::model),
            request.location(),
        )?;
        let contributions =
            plan.feedback_with_route(source, target, request, route, &self.model)?;
        self.add_feedback_batch(
            contributions
                .into_iter()
                .map(|feedback| (Some(request.source()), feedback))
                .collect(),
        )
    }

    /// Builds the final connection Command when the current target is valid.
    pub fn connection_command(
        &mut self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        self.with_extension_fault_boundary(|viewer| viewer.connection_command_inner(plan, request))
    }

    fn connection_command_inner(
        &self,
        plan: &mut dyn ConnectionCreation<A>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        let source = self.connection_endpoint_host(request.source())?;
        let Some(target) = self.valid_connection_target(plan, source, request)? else {
            return Ok(None);
        };
        Ok(Some(plan.command(source, target, request, &self.model)?))
    }

    /// Resolves one unambiguous connection policy into a reconnect plan.
    pub fn start_connection_reconnection(
        &mut self,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionReconnection<A>>>, ViewerError>
    where
        A: 'static,
    {
        let metadata =
            RequestMetadata::new(request.revision(), vec![request.connection().edit_part()]);
        self.with_extension_fault_boundary(|viewer| {
            viewer.start_connection_reconnection_inner(&metadata, request)
        })
    }

    fn start_connection_reconnection_inner(
        &mut self,
        metadata: &RequestMetadata,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionReconnection<A>>>, ViewerError>
    where
        A: 'static,
    {
        let connection = request.connection().edit_part();
        self.policy_host(connection)?;
        Ok(self
            .dispatch_request_inner::<ReconnectConnection>(metadata, request)?
            .into_contributions()
            .pop())
    }

    /// Replaces reconnect feedback for the latest endpoint candidate.
    pub fn show_reconnection_feedback(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.with_extension_fault_boundary(|viewer| {
            viewer.show_reconnection_feedback_inner(plan, request)
        })
    }

    fn show_reconnection_feedback_inner(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        let connection = self.policy_host(request.connection().edit_part())?;
        let fixed = self.reconnection_fixed_host(request)?;
        let candidate = self.valid_reconnection_target(plan, connection, fixed, request)?;
        let endpoints = self.parts.connection_endpoints(request.connection())?;
        let source = self.policy_host(endpoints.source())?;
        let target = self.policy_host(endpoints.target())?;
        let (source_part, source_model, target_part, target_model) = match request.endpoint() {
            ConnectionEndpoint::Source => (
                candidate.map(PolicyHost::part),
                candidate.map_or(source.model(), PolicyHost::model),
                Some(target.part()),
                Some(target.model()),
            ),
            ConnectionEndpoint::Target => (
                Some(source.part()),
                source.model(),
                candidate.map(PolicyHost::part),
                candidate.map(PolicyHost::model),
            ),
        };
        let connection_figure = self
            .parts
            .get(request.connection().edit_part())
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        let route = self.preview_connection_feedback_route(
            Some(connection.model()),
            Some(connection_figure),
            source_part,
            source_model,
            target_part,
            target_model,
            request.location(),
        )?;
        let contributions =
            plan.feedback_with_route(connection, fixed, candidate, request, route, &self.model)?;
        self.add_feedback_batch(
            contributions
                .into_iter()
                .map(|feedback| (Some(request.connection().edit_part()), feedback))
                .collect(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn preview_connection_feedback_route(
        &mut self,
        connection_model: Option<A::ModelId>,
        connection_figure: Option<FigureId>,
        source_part: Option<EditPartId>,
        source_model: A::ModelId,
        target_part: Option<EditPartId>,
        target_model: Option<A::ModelId>,
        pointer: Point,
    ) -> Result<Option<ConnectionFeedbackRoute>, ViewerError> {
        let routing_space = CoordinateSpace::ChildContent(self.root_layers.connection());
        let source_anchor: Box<dyn ConnectionAnchor> = if let Some(source_part) = source_part {
            self.build_connection_anchor_for_models(
                ConnectionEndpoint::Source,
                source_part,
                connection_model,
                source_model,
                target_model,
                connection_figure,
            )?
            .1
        } else {
            Box::new(XYAnchor::new(pointer, routing_space))
        };
        let target_anchor: Box<dyn ConnectionAnchor> =
            if let (Some(target_part), Some(target_model)) = (target_part, target_model) {
                self.build_connection_anchor_for_models(
                    ConnectionEndpoint::Target,
                    target_part,
                    connection_model,
                    source_model,
                    Some(target_model),
                    connection_figure,
                )?
                .1
            } else {
                Box::new(XYAnchor::new(pointer, routing_space))
            };
        let metadata = self
            .runtime
            .connections()
            .preview_endpoints(
                source_anchor.as_ref(),
                target_anchor.as_ref(),
                routing_space,
            )
            .map_err(|error| {
                ConnectionRuntimeError::Unresolved(UnresolvedConnection::RouteFailed(error))
            })?;
        Ok(Some(ConnectionFeedbackRoute::new(
            metadata.source.site.point,
            metadata.target.site.point,
        )))
    }

    /// Builds the final reconnect Command when the current candidate is valid.
    pub fn reconnection_command(
        &mut self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        self.with_extension_fault_boundary(|viewer| {
            viewer.reconnection_command_inner(plan, request)
        })
    }

    fn reconnection_command_inner(
        &self,
        plan: &mut dyn ConnectionReconnection<A>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError> {
        let connection = self.policy_host(request.connection().edit_part())?;
        let fixed = self.reconnection_fixed_host(request)?;
        let Some(candidate) = self.valid_reconnection_target(plan, connection, fixed, request)?
        else {
            return Ok(None);
        };
        Ok(Some(plan.command(
            connection,
            fixed,
            candidate,
            request,
            &self.model,
        )?))
    }

    fn reconnection_fixed_host(
        &self,
        request: &ReconnectConnectionRequest,
    ) -> Result<PolicyHost<A::ModelId>, ViewerError> {
        let endpoints = self.parts.connection_endpoints(request.connection())?;
        let fixed = match request.endpoint() {
            ConnectionEndpoint::Source => endpoints.target(),
            ConnectionEndpoint::Target => endpoints.source(),
        };
        self.connection_endpoint_host(fixed)
    }

    fn valid_reconnection_target(
        &self,
        plan: &dyn ConnectionReconnection<A>,
        connection: PolicyHost<A::ModelId>,
        fixed: PolicyHost<A::ModelId>,
        request: &ReconnectConnectionRequest,
    ) -> Result<Option<PolicyHost<A::ModelId>>, ViewerError> {
        let Some(candidate) = request.target_candidate() else {
            return Ok(None);
        };
        let Some(candidate) = self.connection_target_host(candidate)? else {
            return Ok(None);
        };
        if plan.can_complete(connection, fixed, candidate, request, &self.model)? {
            Ok(Some(candidate))
        } else {
            Ok(None)
        }
    }

    fn valid_connection_target(
        &self,
        plan: &dyn ConnectionCreation<A>,
        source: PolicyHost<A::ModelId>,
        request: &CreateConnectionRequest,
    ) -> Result<Option<PolicyHost<A::ModelId>>, ViewerError> {
        let Some(target) = request.target_candidate() else {
            return Ok(None);
        };
        let Some(target) = self.connection_target_host(target)? else {
            return Ok(None);
        };
        if plan.can_complete(source, target, request, &self.model)? {
            Ok(Some(target))
        } else {
            Ok(None)
        }
    }

    fn connection_endpoint_host(
        &self,
        part: EditPartId,
    ) -> Result<PolicyHost<A::ModelId>, ViewerError> {
        self.validate_selectable(part)?;
        if self
            .parts
            .get(part)
            .is_none_or(|node| node.kind() != PartKind::Containment)
        {
            return Err(ViewerError::InvalidPart(part));
        }
        self.policy_host(part)
    }

    fn connection_target_host(
        &self,
        part: EditPartId,
    ) -> Result<Option<PolicyHost<A::ModelId>>, ViewerError> {
        if part.namespace() != self.namespace() {
            return Err(ViewerError::InvalidPart(part));
        }
        let Some(node) = self.parts.get(part) else {
            return Err(ViewerError::InvalidPart(part));
        };
        if !node.is_active() || node.kind() != PartKind::Containment {
            return Ok(None);
        }
        self.policy_host(part).map(Some)
    }

    fn policy_host(&self, part: EditPartId) -> Result<PolicyHost<A::ModelId>, ViewerError> {
        let node = self.parts.get(part).ok_or(ViewerError::InvalidPart(part))?;
        let model = node.model_id().ok_or(ViewerError::InvalidPart(part))?;
        let parent_model = self
            .parts
            .parent(part)
            .and_then(|parent| self.parts.get(parent))
            .and_then(|parent| parent.model_id());
        Ok(PolicyHost::new(part, model, parent_model))
    }
}
