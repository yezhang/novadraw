//! Viewer contents, registries, and model-to-Figure projection.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::Arc,
};

use novadraw::geometry::{ApproxEq, Point, Precision, Rectangle, Translatable};
use novadraw::{
    AnchorId, AnchorSemanticKey, CaretGeometry, ChopboxAnchor, Color, ComponentUpdateError,
    ConnectionAnchor, ConnectionId, ConnectionLayerFigure, ConnectionRuntimeError, CoordinateSpace,
    DispatchOutcome, Figure, FigureComponentContext, FigureComponentUpdate, FigureId, FigureTree,
    FlowTextPosition, FlowTextRange, FramePreparationError, FreeformLayerFigure,
    FreeformLayeredPane, KeyModifiers, LayerError, LayerFigure, LayerKey, LayerPlacement,
    LayeredPane, MonotonicTime, MouseButton, MouseLocationZoomScrollPolicy, PreparedFigureUpdate,
    RectangleFigure, RouterBinding, RouterId, Runtime, RuntimeMutationError,
    ScalableFreeformLayeredPane, SelectionQuad, StackLayout, TextFlowFigure, TextFlowViewport,
    TextMovement, TimeError, UnresolvedConnection, ViewportHandle, XYAnchor, ZoomManager,
};

use crate::{
    BendpointHandleSite, Command, CompoundCommand, ConnectionAnchorContext,
    ConnectionAnchorDescriptor, ConnectionCreation, ConnectionEndpoint, ConnectionFeedbackRoute,
    ConnectionPartFactoryContext, ConnectionPartId, ConnectionReconnection, ConnectionRouterKey,
    ConnectionRouterSelection, ConnectionRoutingDescriptor, CreateConnectionRequest,
    DirectTextEditError, DirectTextEditRequest, DirectTextEditSessionId, DirectTextEditState,
    DirectTextFeedback, EditPartError, EditPartFactory, EditPartId, EditorNamespace, EditorRequest,
    FeedbackId, FeedbackVisual, HandleId, HandleRole, ModelAdapter, ModelConnection, ModelRevision,
    PartFactoryContext, PartKind, PartTree, PartTreeError, PolicyError, PolicyHost, PolicyRole,
    ReconnectConnectionRequest, SelectionDelta, SelectionModel, TextEditMode, TextInputEffect,
    TextInputPurpose, TextInputSnapshot, VisualBuildContext, VisualOwner, VisualUpdateContext,
    direct_edit::{
        ActiveDirectTextEdit, PreparedDirectTextEdit, position_from_offset, position_to_offset,
    },
    part::{BehaviorStore, validate_runtime_namespace},
    policy::PolicyStore,
};

const MAX_PART_TREE_DEPTH: usize = 10_000;
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
        ComponentUpdateError::Runtime(error) => error.into(),
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerTarget {
    /// The point resolves to an ordinary model EditPart.
    Part(EditPartId),
    /// The point resolves to an interactive handle.
    Handle {
        /// Handle identity.
        id: HandleId,
        /// EditPart manipulated by the handle.
        owner: EditPartId,
        /// Interaction represented by the handle.
        role: HandleRole,
    },
    /// No selectable visual was hit; the Viewer contents is the semantic fallback.
    Contents(EditPartId),
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
    pub const fn target(&self) -> ViewerTarget {
        self.target
    }

    /// Returns the selection delta when Editor fallback changed selection.
    pub const fn selection(&self) -> Option<&SelectionDelta<EditPartId>> {
        self.selection.as_ref()
    }
}

/// Failure while constructing or refreshing a [`GraphicalViewer`].
#[derive(Debug)]
pub enum ViewerError {
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
            Self::RuntimePreparation(error) => error.fmt(formatter),
            Self::RuntimeTime(error) => error.fmt(formatter),
            Self::Connection(error) => error.fmt(formatter),
            Self::Layer(error) => error.fmt(formatter),
            Self::Policy(error) => error.fmt(formatter),
            Self::DirectTextEdit(error) => error.fmt(formatter),
            Self::InvalidPart(part) => write!(formatter, "invalid EditPart: {part:?}"),
            Self::InconsistentState => formatter.write_str("Viewer state is inconsistent"),
        }
    }
}

impl Error for ViewerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EditPart(error) => Some(error),
            Self::PartTree(error) => Some(error),
            Self::Runtime(error) => Some(error),
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
        .layered_pane(parent)?
        .add_layer(figure, layer_key(key), LayerPlacement::Last)?)
}

fn create_root_layers(bounds: Rectangle) -> Result<(Runtime, RootLayers), ViewerError> {
    let mut tree = FigureTree::new();
    let root = tree.builder().set_contents(Box::new(LayeredPane::new(
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
    )));
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
    let viewport = runtime.add_viewport(viewport_layer, bounds)?;
    let scalable = runtime.container(viewport.figure_id())?.add(Box::new(
        ScalableFreeformLayeredPane::new(0.0, 0.0, bounds.width, bounds.height),
    ))?;
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
        .container(root)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .container(viewport_layer)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .container(scalable)?
        .set_layout_manager(Box::new(StackLayout::new()))?;
    runtime
        .container(printable)?
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
}

/// Owns one application model projection, its EditPart tree, and its Figure Runtime.
pub struct GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    model: A,
    factory: F,
    runtime: Runtime,
    root_layers: RootLayers,
    parts: PartTree<A::ModelId>,
    behaviors: BehaviorStore<A>,
    policies: PolicyStore<A>,
    model_registry: HashMap<A::ModelId, EditPartId>,
    connection_projections: HashMap<A::ModelId, ConnectionProjection<A::ModelId>>,
    connection_routers: HashMap<ConnectionRouterKey, RouterId>,
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
        let mut connection_routers = HashMap::new();
        for registration in factory.connection_routers()? {
            let (key, router) = registration.into_parts();
            if connection_routers.contains_key(&key) {
                return Err(EditPartError::operation("duplicate connection Router key").into());
            }
            let router = runtime.try_register_connection_router(router)?;
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
            parts,
            behaviors: BehaviorStore::new(namespace),
            policies: PolicyStore::new(namespace),
            model_registry: HashMap::new(),
            connection_projections: HashMap::new(),
            connection_routers,
            visual_registry: HashMap::from([(root_layers.root(), VisualOwner::Part(root_part))]),
            selection: SelectionModel::new(),
            direct_text_edit: None,
            next_direct_text_edit_session: 1,
            current_time: MonotonicTime::ZERO,
            direct_text_blink_deadline: None,
            text_input_effects: Vec::new(),
            applied_revision,
            model_root: snapshot.root,
            faulted: false,
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

    /// Returns mutable application model access while the Viewer can still accept edits.
    pub fn model_mut(&mut self) -> Result<&mut A, ViewerError> {
        self.ensure_ready()?;
        Ok(&mut self.model)
    }

    /// Returns the Figure Runtime.
    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Returns the standard root layer identities.
    pub const fn root_layers(&self) -> RootLayers {
        self.root_layers
    }

    /// Returns mutable Runtime access for rendering and platform integration.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Advances Runtime timers and the active direct-edit caret blink from host time.
    pub fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, ViewerError> {
        let runtime_changed = self.runtime.advance_time(now)?;
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

    /// Returns the earliest Runtime or direct-edit deadline requiring a host wakeup.
    pub fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        match (
            self.runtime.next_wake_deadline(),
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
            .ok_or(ViewerError::InconsistentState)?
            .scale())
    }

    /// Returns the root Viewport client rectangle in logical surface coordinates.
    pub fn viewport_bounds_in_surface(&self) -> Result<Rectangle, ViewerError> {
        let viewport = self.root_layers.viewport();
        let bounds = self
            .runtime
            .tree()
            .figure_bounds(viewport)
            .ok_or(ViewerError::InconsistentState)?;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(viewport)
            .ok_or(ViewerError::InconsistentState)?;
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
        Ok(self
            .runtime
            .viewport(viewport.figure_id())?
            .set_view_location(origin.x(), origin.y())?)
    }

    /// Sets content zoom while preserving an optional logical surface anchor.
    pub fn set_viewport_scale_at(
        &mut self,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ViewerError> {
        let viewport = self.viewport_handle()?;
        let scalable = self
            .runtime
            .tree()
            .scale_handle(self.root_layers.scalable())
            .ok_or(ViewerError::InconsistentState)?;
        let anchor = anchor
            .map(|point| {
                self.runtime
                    .tree()
                    .surface_to_local_transform(self.root_layers.viewport())
                    .map(|transform| transform.transform_point(point))
                    .ok_or(ViewerError::InconsistentState)
            })
            .transpose()?;
        let mut zoom = ZoomManager::new(scalable, viewport);
        if anchor.is_some() {
            zoom.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));
        }
        Ok(self.runtime.zoom(&zoom)?.set_zoom_at(scale, anchor)?)
    }

    pub(crate) fn scroll_viewport_by_surface_delta(
        &mut self,
        delta: novadraw::geometry::Vec2,
    ) -> Result<bool, ViewerError> {
        let transform = self
            .runtime
            .tree()
            .child_content_to_surface_transform(self.root_layers.scalable())
            .and_then(|transform| transform.inverse())
            .ok_or(ViewerError::InconsistentState)?;
        let delta = transform.transform_vector(delta);
        let viewport = self.viewport_handle()?;
        let origin = viewport.view_location();
        Ok(self
            .runtime
            .viewport(viewport.figure_id())?
            .set_view_location(origin.x() + delta.x(), origin.y() + delta.y())?)
    }

    pub(crate) fn viewport_ranges(
        &self,
    ) -> Result<(novadraw::RangeModelSnapshot, novadraw::RangeModelSnapshot), ViewerError> {
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
            .and_then(|transform| transform.inverse())
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
            .viewport_handle(self.root_layers.viewport())
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
        let points = self.runtime.tree().connection_route_points(figure)?;
        let transform = self.runtime.tree().local_to_surface_transform(figure)?;
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
            .ok_or(ViewerError::InconsistentState)?;
        let bendpoints = self
            .connection_bendpoints(connection)?
            .into_iter()
            .map(|bendpoint| routing_to_surface.transform_point(bendpoint))
            .collect::<Vec<_>>();
        let mut sites = Vec::with_capacity(route.len().saturating_sub(1) + bendpoints.len());
        let mut bendpoint_index = 0;
        for segment in route.windows(2) {
            sites.push(BendpointHandleSite::new(
                HandleRole::BendpointCreate(bendpoint_index),
                segment[0] + (segment[1] - segment[0]) / 2.0,
            ));
            if bendpoints
                .get(bendpoint_index)
                .is_some_and(|bendpoint| bendpoint.approx_eq(segment[1], Precision::DEFAULT))
            {
                sites.push(BendpointHandleSite::new(
                    HandleRole::BendpointMove(bendpoint_index),
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
        let bounds = self.runtime.tree().figure_bounds(figure)?;
        let transform = self.runtime.tree().local_to_surface_transform(figure)?;
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
        self.visual_registry.get(&figure).copied()
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
                Some((_, VisualOwner::Handle { id, owner, role })) => {
                    return ViewerTarget::Handle { id, owner, role };
                }
                Some((_, VisualOwner::Part(part)))
                    if part != self.root() && part != self.contents() =>
                {
                    return ViewerTarget::Part(part);
                }
                Some((_, VisualOwner::Part(_))) | None => {
                    return ViewerTarget::Contents(self.contents());
                }
                Some((owner_figure, VisualOwner::Feedback { .. })) => {
                    excluded.push(owner_figure);
                }
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
        let dispatch = self.runtime.dispatch_mouse_pressed(x, y, button);
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
        ViewerInputOutcome {
            dispatch: self.runtime.dispatch_mouse_pressed(x, y, button),
            target: self.target_at(x, y),
            selection: None,
        }
    }

    /// Dispatches pointer movement while preserving any Figure-owned capture.
    pub fn dispatch_mouse_moved(&mut self, x: f64, y: f64) -> DispatchOutcome {
        self.runtime.dispatch_mouse_moved(x, y)
    }

    /// Dispatches pointer release and lets the Figure Runtime clear its capture.
    pub fn dispatch_mouse_released(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.runtime.dispatch_mouse_released(x, y, button)
    }

    /// Clears hover and pointer targeting after the pointer leaves the surface.
    pub fn pointer_exited(&mut self) {
        self.runtime.pointer_exited();
    }

    /// Adds and registers an interactive handle in the unscaled handle layer.
    pub fn add_handle_visual(
        &mut self,
        owner: EditPartId,
        figure: Box<dyn Figure>,
    ) -> Result<(HandleId, FigureId), ViewerError> {
        self.add_handle_visual_with_role(owner, HandleRole::Selection, figure)
    }

    /// Adds and registers a typed handle in the unscaled handle layer.
    pub fn add_handle_visual_with_role(
        &mut self,
        owner: EditPartId,
        role: HandleRole,
        figure: Box<dyn Figure>,
    ) -> Result<(HandleId, FigureId), ViewerError> {
        self.validate_selectable(owner)?;
        let figure = self
            .runtime
            .container(self.root_layers.handles())?
            .add(figure)?;
        let id = HandleId::new(self.namespace());
        self.visual_registry
            .insert(figure, VisualOwner::Handle { id, owner, role });
        Ok((id, figure))
    }

    /// Adds and registers transient feedback in a scaled or unscaled feedback layer.
    pub fn add_feedback_visual(
        &mut self,
        owner: Option<EditPartId>,
        scaled: bool,
        figure: Box<dyn Figure>,
    ) -> Result<(FeedbackId, FigureId), ViewerError> {
        if let Some(owner) = owner {
            self.validate_selectable(owner)?;
        }
        let parent = if scaled {
            self.root_layers.scaled_feedback()
        } else {
            self.root_layers.feedback()
        };
        let figure = self.runtime.container(parent)?.add(figure)?;
        let id = FeedbackId::new(self.namespace());
        self.visual_registry
            .insert(figure, VisualOwner::Feedback { id, owner });
        Ok((id, figure))
    }

    fn add_feedback_contribution(
        &mut self,
        owner: Option<EditPartId>,
        feedback: FeedbackVisual,
    ) -> Result<(FeedbackId, FigureId), ViewerError> {
        let (figure, scaled, style) = feedback.into_parts();
        let attached = self.add_feedback_visual(owner, scaled, figure)?;
        if let Some(style) = style
            && let Err(error) = self
                .runtime
                .figure(attached.1)
                .and_then(|mut figure| figure.set_style(style))
        {
            let _ = self.remove_overlay_visual(attached.1);
            return Err(error.into());
        }
        Ok(attached)
    }

    /// Removes a registered handle or feedback subtree.
    pub fn remove_overlay_visual(&mut self, figure: FigureId) -> Result<bool, ViewerError> {
        match self.visual_registry.get(&figure).copied() {
            Some(VisualOwner::Handle { .. } | VisualOwner::Feedback { .. }) => {
                self.runtime.dispose_subtree(figure)?;
                self.visual_registry.remove(&figure);
                Ok(true)
            }
            Some(VisualOwner::Part(_)) | None => Ok(false),
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
        let host = self.policy_host(request.source())?;
        let plan = self
            .policies
            .roles_mut(request.source())
            .and_then(|roles| roles.get_mut(&PolicyRole::DirectTextEdit))
            .map(|policy| policy.start_direct_text_edit(host, request, &self.model))
            .transpose()?
            .flatten()
            .ok_or(DirectTextEditError::UnsupportedFeature)?;
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
            .text_flow_hit_test(active.text_feedback, point)
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
    pub fn direct_text_caret_geometry(&self) -> Result<CaretGeometry, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let caret = active.state.caret_position()?;
        self.runtime
            .text_flow_caret_geometry(active.text_feedback, caret)
            .map_err(|_| DirectTextEditError::InvalidTextPosition.into())
    }

    /// Returns active draft selection quads in logical surface coordinates.
    pub fn direct_text_selection_geometry(&self) -> Result<Vec<SelectionQuad>, ViewerError> {
        let active = self
            .direct_text_edit
            .as_ref()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let viewport = self.direct_text_viewport_in_surface(active.text_feedback)?;
        let selection = self
            .runtime
            .text_flow_selection_geometry(active.text_feedback, active.state.selection())
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
        &self,
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
            .text_flow_selection_geometry(active.text_feedback, FlowTextRange::new(start, end))
            .map_err(|_| DirectTextEditError::InvalidTextPosition)?
            .into_iter()
            .map(|quad| quad.bounds())
            .reduce(Rectangle::union);
        if let Some(bounds) = bounds {
            return Ok(bounds);
        }

        let caret = self
            .runtime
            .text_flow_caret_geometry(active.text_feedback, start)
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
        let Some(bounds) = self.runtime.tree().figure_bounds(active.text_feedback) else {
            return false;
        };
        let Some(transform) = self
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
        let Some(mut active) = self.direct_text_edit.take() else {
            return Ok(false);
        };
        let projection = match active.plan.feedback(&active.state, &self.model) {
            Ok(projection) => projection,
            Err(error) => {
                self.direct_text_edit = Some(active);
                return Err(error.into());
            }
        };
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
        let bounds = self
            .runtime
            .tree()
            .figure_bounds(text_feedback)
            .ok_or(ViewerError::InconsistentState)?;
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(text_feedback)
            .ok_or(ViewerError::InconsistentState)?;
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
            .text_flow_move_position(
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
                .text_flow_move_position(
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
        let Some(active) = self.direct_text_edit.take() else {
            return Ok(false);
        };
        self.direct_text_blink_deadline = None;
        self.remove_direct_text_feedback(&active.feedback)?;
        self.text_input_effects.push(TextInputEffect::Release {
            session: active.state.session(),
        });
        Ok(true)
    }

    pub(crate) fn prepare_direct_text_accept(
        &mut self,
    ) -> Result<PreparedDirectTextEdit<A>, ViewerError> {
        let mut active = self
            .direct_text_edit
            .take()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        if self
            .parts
            .get(active.state.source())
            .is_none_or(|part| !part.is_active())
        {
            self.direct_text_blink_deadline = None;
            self.remove_direct_text_feedback(&active.feedback)?;
            return Err(DirectTextEditError::ForeignOrRetiredPart.into());
        }
        if active.state.source_revision() != self.model.revision() {
            self.direct_text_edit = Some(active);
            return Err(DirectTextEditError::StaleSourceRevision.into());
        }
        if active.state.composition().is_some() {
            self.direct_text_edit = Some(active);
            return Err(DirectTextEditError::ActiveComposition.into());
        }
        if let Err(error) = active.plan.validate(&active.state, &self.model) {
            self.direct_text_edit = Some(active);
            return Err(error.into());
        }
        let command = if active.state.is_changed() {
            match active.plan.command(&active.state, &self.model) {
                Ok(command) => Some(command),
                Err(error) => {
                    self.direct_text_edit = Some(active);
                    return Err(error.into());
                }
            }
        } else {
            None
        };
        if let Err(error) = self.remove_direct_text_feedback(&active.feedback) {
            self.direct_text_edit = Some(active);
            return Err(error);
        }
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
        let mut active = self
            .direct_text_edit
            .take()
            .ok_or(DirectTextEditError::NoActiveSession)?;
        let previous_state = std::mem::replace(&mut active.state, state);
        let projection = match active.plan.feedback(&active.state, &self.model) {
            Ok(projection) => projection,
            Err(error) => {
                active.state = previous_state;
                self.direct_text_edit = Some(active);
                return Err(error.into());
            }
        };
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
        let mut figures = Vec::with_capacity(visuals.len());
        for visual in visuals {
            match self.add_feedback_contribution(Some(owner), visual) {
                Ok((_, figure)) => figures.push(figure),
                Err(error) => {
                    let _ = self.remove_direct_text_feedback(&figures);
                    return Err(error);
                }
            }
        }
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
        self.runtime.stabilize_for_query()?;
        if self.runtime.text_flow_layout(text_feedback).is_err() {
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
        let Some(text_bounds) = self.runtime.tree().figure_bounds(text_feedback) else {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        };
        if text_bounds.is_empty() {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        }
        let local_caret = match self
            .runtime
            .text_flow_local_caret_geometry(text_feedback, caret)
        {
            Ok(caret) => caret.bounds(),
            Err(_) => {
                let _ = self.remove_direct_text_feedback(&figures);
                return Err(DirectTextEditError::InvalidFeedbackTarget.into());
            }
        };
        let content_width = self
            .runtime
            .text_flow_layout(text_feedback)
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
        if horizontal_scroll != previous_horizontal_scroll
            && let Err(error) =
                self.runtime
                    .figure(text_feedback)?
                    .update_component(SetDirectTextViewport(TextFlowViewport::clipped(
                        Point::new(-horizontal_scroll, 0.0),
                    )))
        {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(direct_text_viewport_error(error));
        }
        let Some(transform) = self
            .runtime
            .tree()
            .local_to_surface_transform(text_feedback)
        else {
            let _ = self.remove_direct_text_feedback(&figures);
            return Err(DirectTextEditError::InvalidFeedbackTarget.into());
        };
        let mut edit_viewport = Rectangle::new(0.0, 0.0, text_bounds.width, text_bounds.height);
        edit_viewport.transform(transform);
        let area = match self.runtime.text_flow_caret_geometry(text_feedback, caret) {
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
                .text_flow_selection_geometry(text_feedback, state.selection())
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
                    .text_flow_selection_geometry(text_feedback, composition.range())
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
        for figure in figures.iter().copied() {
            if !self.remove_overlay_visual(figure)? {
                return Err(ViewerError::InconsistentState);
            }
        }
        Ok(())
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
            || !self.runtime.tree().is_attached(figure)
        {
            return None;
        }
        if let Some(owner) = self.visual_registry.get(&figure).copied() {
            return Some((figure, owner));
        }
        self.runtime
            .tree()
            .ancestor_ids(figure)?
            .into_iter()
            .find_map(|ancestor| {
                self.visual_registry
                    .get(&ancestor)
                    .copied()
                    .map(|owner| (ancestor, owner))
            })
    }

    fn resolve_policy_target(
        &mut self,
        candidate: EditPartId,
        request: &EditorRequest,
    ) -> Result<Option<EditPartId>, ViewerError> {
        self.validate_selectable(candidate)?;
        let host = self.policy_host(candidate)?;
        let target = self.policies.roles_mut(candidate).and_then(|roles| {
            roles
                .values()
                .find_map(|policy| policy.target(host, request))
        });
        if let Some(target) = target {
            self.validate_selectable(target)?;
        }
        Ok(target)
    }

    fn resolve_policy_targets(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Vec<EditPartId>, ViewerError> {
        let mut seen_sources = HashSet::new();
        let mut seen_targets = HashSet::new();
        let mut targets = Vec::new();
        for &source in request.source_parts() {
            if !seen_sources.insert(source) {
                return Err(
                    PolicyError::operation("request contains a duplicate source part").into(),
                );
            }
            if let Some(target) = self.resolve_policy_target(source, request)?
                && seen_targets.insert(target)
            {
                targets.push(target);
            }
        }
        Ok(targets)
    }

    /// Resolves deterministic policy command contributions for a request.
    pub fn command_for_request(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>
    where
        A: 'static,
    {
        self.with_policy_fault_boundary(|viewer| viewer.command_for_request_inner(request))
    }

    fn command_for_request_inner(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>
    where
        A: 'static,
    {
        let mut commands = Vec::new();
        for target in self.resolve_policy_targets(request)? {
            let host = self.policy_host(target)?;
            let Some(roles) = self.policies.roles_mut(target) else {
                continue;
            };
            for policy in roles.values_mut() {
                if policy.understands(request)
                    && let Some(command) = policy.command(host, request, &self.model)?
                {
                    commands.push(command);
                }
            }
        }
        Ok(match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => {
                let mut compound = CompoundCommand::new(request.label());
                for command in commands {
                    compound.push(command);
                }
                Some(Box::new(compound))
            }
        })
    }

    /// Resolves and installs policy feedback in deterministic role order.
    pub fn show_feedback_for_request(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        self.with_policy_fault_boundary(|viewer| viewer.show_feedback_for_request_inner(request))
    }

    fn show_feedback_for_request_inner(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Vec<FigureId>, ViewerError> {
        let mut contributions = Vec::new();
        for target in self.resolve_policy_targets(request)? {
            let host = self.policy_host(target)?;
            let Some(roles) = self.policies.roles_mut(target) else {
                continue;
            };
            for policy in roles.values_mut() {
                if policy.understands(request) {
                    contributions.extend(
                        policy
                            .feedback(host, request, &self.model)?
                            .into_iter()
                            .map(|feedback| (target, feedback)),
                    );
                }
            }
        }
        let mut figures = Vec::with_capacity(contributions.len());
        for (owner, feedback) in contributions {
            let (_, figure) = self.add_feedback_contribution(Some(owner), feedback)?;
            figures.push(figure);
        }
        Ok(figures)
    }

    fn with_policy_fault_boundary<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ViewerError>,
    ) -> Result<T, ViewerError> {
        self.ensure_ready()?;
        match catch_unwind(AssertUnwindSafe(|| operation(self))) {
            Ok(result) => result,
            Err(payload) => {
                self.force_drop_direct_text_edit();
                self.faulted = true;
                resume_unwind(payload)
            }
        }
    }

    /// Resolves one unambiguous source policy into a connection-creation plan.
    pub fn start_connection_creation(
        &mut self,
        request: &CreateConnectionRequest,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, ViewerError>
    where
        A: 'static,
    {
        let source = self.connection_endpoint_host(request.source())?;
        let editor_request = EditorRequest::CreateConnection(request.clone());
        let Some(roles) = self.policies.roles_mut(request.source()) else {
            return Ok(None);
        };
        let mut accepted = None;
        for policy in roles.values_mut() {
            if !policy.understands(&editor_request) {
                continue;
            }
            if let Some(plan) = policy.start_connection(source, request, &self.model)? {
                if accepted.is_some() {
                    return Err(PolicyError::operation(
                        "multiple policies accepted connection creation",
                    )
                    .into());
                }
                accepted = Some(plan);
            }
        }
        Ok(accepted)
    }

    /// Replaces connection-creation feedback for the latest target candidate.
    pub fn show_connection_feedback(
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
        let mut figures = Vec::with_capacity(contributions.len());
        for feedback in contributions {
            match self.add_feedback_contribution(Some(request.source()), feedback) {
                Ok((_, figure)) => figures.push(figure),
                Err(error) => {
                    for figure in figures {
                        let _ = self.remove_overlay_visual(figure);
                    }
                    return Err(error);
                }
            }
        }
        Ok(figures)
    }

    /// Builds the final connection Command when the current target is valid.
    pub fn connection_command(
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
        let connection = request.connection().edit_part();
        let host = self.policy_host(connection)?;
        let editor_request = EditorRequest::ReconnectConnection(request.clone());
        let Some(roles) = self.policies.roles_mut(connection) else {
            return Ok(None);
        };
        let mut accepted = None;
        for policy in roles.values_mut() {
            if !policy.understands(&editor_request) {
                continue;
            }
            if let Some(plan) = policy.start_reconnection(host, request, &self.model)? {
                if accepted.is_some() {
                    return Err(
                        PolicyError::operation("multiple policies accepted reconnection").into(),
                    );
                }
                accepted = Some(plan);
            }
        }
        Ok(accepted)
    }

    /// Replaces reconnect feedback for the latest endpoint candidate.
    pub fn show_reconnection_feedback(
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
        let mut figures = Vec::with_capacity(contributions.len());
        for feedback in contributions {
            let (_, figure) =
                self.add_feedback_contribution(Some(request.connection().edit_part()), feedback)?;
            figures.push(figure);
        }
        Ok(figures)
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
            .preview_connection_endpoints(
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

    /// Applies one validated notification batch to the EditPart and Figure projections.
    pub fn refresh(&mut self) -> Result<bool, ViewerError> {
        self.ensure_ready()?;
        match catch_unwind(AssertUnwindSafe(|| self.refresh_projection())) {
            Ok(result) => result,
            Err(payload) => {
                self.force_drop_direct_text_edit();
                self.faulted = true;
                resume_unwind(payload)
            }
        }
    }

    fn refresh_projection(&mut self) -> Result<bool, ViewerError> {
        let events = self.model.drain_events();
        if events.is_empty() {
            if self.model.revision() == self.applied_revision {
                return Ok(false);
            }
            return self.fail_revision(self.model.revision());
        }
        let final_revision = self.validate_revisions(&events)?;
        if self.model.revision() != final_revision {
            return self.fail_revision(self.model.revision());
        }

        let snapshot = match ModelSnapshot::capture(&self.model) {
            Ok(snapshot) if snapshot.root != self.model_root => {
                return self.fail(ViewerError::RootChanged);
            }
            Ok(snapshot) if snapshot.revision != final_revision => {
                return self.fail(ViewerError::SnapshotRevisionChanged);
            }
            Ok(snapshot) => snapshot,
            Err(error) => return self.fail(error),
        };
        let result = self.parts_to_retire(&snapshot).and_then(|retiring| {
            self.prepare_connections_for_containment_change(&snapshot, &retiring)
                .and_then(|()| self.remove_reparented_subtrees(&snapshot))
                .and_then(|()| self.synchronize_subtree(self.contents(), snapshot.root, &snapshot))
                .and_then(|()| self.synchronize_connections(&snapshot))
        });
        match result {
            Ok(()) => {
                self.applied_revision = final_revision;
                self.synchronize_direct_text_input_area()?;
                Ok(true)
            }
            Err(error) => self.fail(error),
        }
    }

    fn validate_revisions(
        &mut self,
        events: &[crate::ModelEvent<A::ModelId, A::Event>],
    ) -> Result<ModelRevision, ViewerError> {
        let mut current = self.applied_revision;
        let mut advanced = false;
        for event in events {
            let actual = event.revision();
            if advanced && actual == current {
                continue;
            }
            if actual <= current {
                return self.fail(ViewerError::StaleRevision {
                    applied: current,
                    actual,
                });
            }
            let expected = current.next().map_err(|_| ViewerError::RevisionGap {
                expected: current,
                actual,
            })?;
            if actual != expected {
                return self.fail(ViewerError::RevisionGap { expected, actual });
            }
            current = actual;
            advanced = true;
        }
        Ok(current)
    }

    fn fail_revision<T>(&mut self, actual: ModelRevision) -> Result<T, ViewerError> {
        let expected = self
            .applied_revision
            .next()
            .unwrap_or(self.applied_revision);
        self.fail(if actual <= self.applied_revision {
            ViewerError::StaleRevision {
                applied: self.applied_revision,
                actual,
            }
        } else {
            ViewerError::RevisionGap { expected, actual }
        })
    }

    fn fail<T>(&mut self, error: ViewerError) -> Result<T, ViewerError> {
        self.force_drop_direct_text_edit();
        self.faulted = true;
        Err(error)
    }

    fn force_drop_direct_text_edit(&mut self) {
        self.direct_text_blink_deadline = None;
        let Some(active) = self.direct_text_edit.take() else {
            return;
        };
        self.text_input_effects.push(TextInputEffect::Release {
            session: active.state.session(),
        });
        for figure in active.feedback {
            if self.runtime.tree().is_attached(figure) {
                let _ = self.runtime.dispose_subtree(figure);
            }
            self.visual_registry.remove(&figure);
        }
    }

    fn ensure_ready(&self) -> Result<(), ViewerError> {
        if self.faulted {
            return Err(ViewerError::Faulted);
        }
        Ok(())
    }

    fn activate_part(&mut self, part: EditPartId, model_id: A::ModelId) -> Result<(), ViewerError> {
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .activate(&self.model, model_id)?;
        let host = match self.policy_host(part) {
            Ok(host) => host,
            Err(error) => {
                self.behaviors
                    .get_mut(part)
                    .expect("behavior remains installed during activation")
                    .deactivate(&self.model, model_id);
                return Err(error);
            }
        };
        let policy_error = self.policies.roles_mut(part).and_then(|roles| {
            let mut activated = 0;
            let error = roles.values_mut().find_map(|policy| {
                policy
                    .activate(host, &self.model)
                    .map(|()| {
                        activated += 1;
                    })
                    .err()
            });
            if error.is_some() {
                for policy in roles.values_mut().take(activated).rev() {
                    policy.deactivate(host, &self.model);
                }
            }
            error
        });
        if let Some(error) = policy_error {
            self.behaviors
                .get_mut(part)
                .expect("behavior remains installed during activation rollback")
                .deactivate(&self.model, model_id);
            return Err(error.into());
        }
        if let Err(error) = self.parts.set_active(part, true) {
            if let Some(roles) = self.policies.roles_mut(part) {
                for policy in roles.values_mut().rev() {
                    policy.deactivate(host, &self.model);
                }
            }
            self.behaviors
                .get_mut(part)
                .expect("behavior remains installed during activation rollback")
                .deactivate(&self.model, model_id);
            return Err(error.into());
        }
        Ok(())
    }

    fn deactivate_part(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        if !node.is_active() {
            return Ok(());
        }
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let parent_model = self
            .parts
            .parent(part)
            .and_then(|parent| self.parts.get(parent))
            .and_then(|parent| parent.model_id());
        let host = PolicyHost::new(part, model_id, parent_model);
        self.parts.set_active(part, false)?;
        if let Some(roles) = self.policies.roles_mut(part) {
            for policy in roles.values_mut() {
                policy.deactivate(host, &self.model);
            }
        }
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .deactivate(&self.model, model_id);
        Ok(())
    }

    fn create_subtree(
        &mut self,
        parent: EditPartId,
        model_id: A::ModelId,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<EditPartId, ViewerError> {
        if self.model_registry.contains_key(&model_id) {
            return Err(ViewerError::DuplicateModel);
        }
        let parent_node = self
            .parts
            .get(parent)
            .ok_or(ViewerError::InconsistentState)?;
        let parent_model = parent_node.model_id();
        let parent_figure = parent_node.content_pane();
        let context = PartFactoryContext::new(parent, parent_model, model_id);
        let mut behavior = self.factory.create(context, &self.model)?;
        let policies = behavior.create_policies(&self.model, model_id)?;
        let primary = self
            .runtime
            .container(parent_figure)?
            .add(behavior.create_figure(&self.model, model_id)?)?;
        let mut build = VisualBuildContext::new(&mut self.runtime, primary);
        if let Err(error) = behavior.configure_visual(&self.model, model_id, &mut build) {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(error.into());
        }
        let (content_pane, visuals) = build.finish();
        validate_runtime_namespace(self.runtime.tree().namespace(), primary)?;
        if visuals
            .iter()
            .any(|visual| self.visual_registry.contains_key(visual))
        {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(ViewerError::DuplicateVisual);
        }

        let part = self
            .parts
            .insert(parent, model_id, primary, content_pane, visuals.clone())?;
        self.model_registry.insert(model_id, part);
        for visual in visuals {
            self.visual_registry.insert(visual, VisualOwner::Part(part));
        }
        self.behaviors.insert(part, behavior)?;
        for (role, policy) in policies {
            self.policies.install(part, role, policy)?;
        }
        self.refresh_part_visuals(part)?;
        self.activate_part(part, model_id)?;

        for child in snapshot.children_of(model_id)?.iter().copied() {
            self.create_subtree(part, child, snapshot)?;
        }
        Ok(part)
    }

    fn build_connection_anchor(
        &mut self,
        endpoint: ConnectionEndpoint,
        endpoint_part: EditPartId,
        descriptor: ModelConnection<A::ModelId>,
        connection_figure: FigureId,
    ) -> Result<(AnchorSemanticKey, Box<dyn ConnectionAnchor>), ViewerError> {
        self.build_connection_anchor_for_models(
            endpoint,
            endpoint_part,
            Some(descriptor.id()),
            descriptor.source(),
            Some(descriptor.target()),
            Some(connection_figure),
        )
    }

    fn build_connection_anchor_for_models(
        &mut self,
        endpoint: ConnectionEndpoint,
        endpoint_part: EditPartId,
        connection_model: Option<A::ModelId>,
        source_model: A::ModelId,
        target_model: Option<A::ModelId>,
        connection_figure: Option<FigureId>,
    ) -> Result<(AnchorSemanticKey, Box<dyn ConnectionAnchor>), ViewerError> {
        let endpoint_node = self
            .parts
            .get(endpoint_part)
            .ok_or(ViewerError::InconsistentState)?;
        let endpoint_model = endpoint_node
            .model_id()
            .ok_or(ViewerError::InconsistentState)?;
        let endpoint_figure = endpoint_node.primary_figure();
        let context = ConnectionAnchorContext::new(
            connection_model,
            source_model,
            target_model,
            connection_figure,
            endpoint_figure,
        );
        let behavior = self
            .behaviors
            .get_mut(endpoint_part)
            .ok_or(ViewerError::InconsistentState)?;
        let custom = match endpoint {
            ConnectionEndpoint::Source => {
                behavior.source_connection_anchor(&self.model, endpoint_model, context)?
            }
            ConnectionEndpoint::Target => {
                behavior.target_connection_anchor(&self.model, endpoint_model, context)?
            }
        };
        let descriptor = custom.unwrap_or_else(|| {
            let anchor = ChopboxAnchor::new(endpoint_figure);
            let key = anchor
                .semantic_group_key()
                .expect("ChopboxAnchor always provides semantic identity");
            ConnectionAnchorDescriptor::new(key, Box::new(anchor))
        });
        Ok(descriptor.into_parts())
    }

    fn create_connection_part(
        &mut self,
        descriptor: ModelConnection<A::ModelId>,
    ) -> Result<ConnectionPartId, ViewerError> {
        if self.model_registry.contains_key(&descriptor.id()) {
            return Err(ViewerError::DuplicateModel);
        }
        let source_part = self
            .model_registry
            .get(&descriptor.source())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.source()),
            })?;
        let target_part = self
            .model_registry
            .get(&descriptor.target())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.target()),
            })?;
        let context = ConnectionPartFactoryContext::new(
            self.root(),
            descriptor.id(),
            source_part,
            descriptor.source(),
            target_part,
            descriptor.target(),
        );
        let mut behavior = self.factory.create_connection(context, &self.model)?;
        let policies = behavior.create_policies(&self.model, descriptor.id())?;
        let bendpoints = behavior.connection_bendpoints(&self.model, descriptor.id())?;
        Self::validate_connection_bendpoints(descriptor.id(), &bendpoints)?;
        let routing = behavior.connection_routing(&self.model, descriptor.id())?;
        let (router, constraint) = self.resolve_connection_routing(routing)?;
        let primary = self
            .runtime
            .container(self.root_layers.connection())?
            .add(behavior.create_figure(&self.model, descriptor.id())?)?;
        if !self.runtime.tree().is_connection_figure(primary) {
            self.runtime.dispose_subtree(primary)?;
            return Err(ViewerError::InvalidConnectionFigure {
                connection: format!("{:?}", descriptor.id()),
            });
        }
        let mut build = VisualBuildContext::new(&mut self.runtime, primary);
        if let Err(error) = behavior.configure_visual(&self.model, descriptor.id(), &mut build) {
            let _ = self.runtime.dispose_subtree(primary);
            return Err(error.into());
        }
        let (content_pane, visuals) = build.finish();
        if visuals
            .iter()
            .any(|visual| self.visual_registry.contains_key(visual))
        {
            self.runtime.dispose_subtree(primary)?;
            return Err(ViewerError::DuplicateVisual);
        }
        let (source_anchor_key, source_anchor_strategy) = match self.build_connection_anchor(
            ConnectionEndpoint::Source,
            source_part,
            descriptor,
            primary,
        ) {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error);
            }
        };
        let (target_anchor_key, target_anchor_strategy) = match self.build_connection_anchor(
            ConnectionEndpoint::Target,
            target_part,
            descriptor,
            primary,
        ) {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error);
            }
        };
        let source_anchor = self
            .runtime
            .try_register_connection_anchor(source_anchor_strategy)?;
        let target_anchor = match self
            .runtime
            .try_register_connection_anchor(target_anchor_strategy)
        {
            Ok(anchor) => anchor,
            Err(error) => {
                let _ = self.runtime.remove_connection_anchor(source_anchor);
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error.into());
            }
        };
        let connection = match self.runtime.register_connection_state(
            primary,
            Some(source_anchor),
            Some(target_anchor),
            router,
            constraint,
        ) {
            Ok(connection) => connection,
            Err(error) => {
                let _ = self.runtime.remove_connection_anchor(source_anchor);
                let _ = self.runtime.remove_connection_anchor(target_anchor);
                let _ = self.runtime.dispose_subtree(primary);
                return Err(error.into());
            }
        };

        let part = self.parts.insert_connection(
            descriptor.id(),
            primary,
            content_pane,
            visuals.clone(),
            source_part,
            target_part,
        )?;
        self.model_registry
            .insert(descriptor.id(), part.edit_part());
        for visual in visuals {
            self.visual_registry
                .insert(visual, VisualOwner::Part(part.edit_part()));
        }
        self.behaviors.insert(part.edit_part(), behavior)?;
        self.connection_projections.insert(
            descriptor.id(),
            ConnectionProjection {
                part,
                connection,
                source_model: descriptor.source(),
                target_model: descriptor.target(),
                source_anchor: Some(source_anchor),
                target_anchor: Some(target_anchor),
                source_anchor_key: Some(source_anchor_key),
                target_anchor_key: Some(target_anchor_key),
                registered: true,
            },
        );
        let initialization = (|| {
            for (role, policy) in policies {
                self.policies.install(part.edit_part(), role, policy)?;
            }
            self.refresh_part_visuals(part.edit_part())?;
            self.activate_part(part.edit_part(), descriptor.id())?;
            self.resolve_connection(connection)
        })();
        if let Err(error) = initialization {
            self.remove_connection_part(part)?;
            return Err(error);
        }
        Ok(part)
    }

    fn resolve_connection(&mut self, connection: ConnectionId) -> Result<(), ViewerError> {
        match self.runtime.resolve_connection_route(
            connection,
            CoordinateSpace::ChildContent(self.root_layers.connection()),
        ) {
            Ok(_) | Err(ConnectionRuntimeError::Unresolved(_)) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn synchronize_connections(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let mut order = Vec::with_capacity(snapshot.connections.len());
        let mut figure_order = Vec::with_capacity(snapshot.connections.len());
        for descriptor in snapshot.connections.iter().copied() {
            let connection = match self
                .connection_projections
                .get(&descriptor.id())
                .map(|projection| projection.part)
            {
                Some(connection) => {
                    self.bind_connection_part(descriptor)?;
                    self.refresh_part_visuals(connection.edit_part())?;
                    connection
                }
                None => self.create_connection_part(descriptor)?,
            };
            let figure = self
                .parts
                .get(connection.edit_part())
                .ok_or(ViewerError::InconsistentState)?
                .primary_figure();
            order.push(connection);
            figure_order.push(figure);
        }
        self.runtime
            .container(self.root_layers.connection())?
            .set_child_order(&figure_order)?;
        self.parts.set_connection_order(order)?;
        Ok(())
    }

    fn bind_connection_part(
        &mut self,
        descriptor: ModelConnection<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let source_part = self
            .model_registry
            .get(&descriptor.source())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.source()),
            })?;
        let target_part = self
            .model_registry
            .get(&descriptor.target())
            .copied()
            .ok_or_else(|| ViewerError::MissingConnectionEndpoint {
                connection: format!("{:?}", descriptor.id()),
                endpoint: format!("{:?}", descriptor.target()),
            })?;
        let mut current = self
            .connection_projections
            .get(&descriptor.id())
            .cloned()
            .ok_or(ViewerError::InconsistentState)?;
        let behavior = self
            .behaviors
            .get_mut(current.part.edit_part())
            .ok_or(ViewerError::InconsistentState)?;
        let bendpoints = behavior.connection_bendpoints(&self.model, descriptor.id())?;
        Self::validate_connection_bendpoints(descriptor.id(), &bendpoints)?;
        let routing = behavior.connection_routing(&self.model, descriptor.id())?;
        let (router, constraint) = self.resolve_connection_routing(routing)?;
        let (source_anchor_key, source_anchor_strategy) = self.build_connection_anchor(
            ConnectionEndpoint::Source,
            source_part,
            descriptor,
            current.connection.figure(),
        )?;
        let (target_anchor_key, target_anchor_strategy) = self.build_connection_anchor(
            ConnectionEndpoint::Target,
            target_part,
            descriptor,
            current.connection.figure(),
        )?;
        let source_reusable = current.source_model == descriptor.source()
            && current.source_anchor.is_some()
            && current.source_anchor_key.as_ref() == Some(&source_anchor_key);
        let target_reusable = current.target_model == descriptor.target()
            && current.target_anchor.is_some()
            && current.target_anchor_key.as_ref() == Some(&target_anchor_key);
        if current.registered
            && source_reusable
            && target_reusable
            && self.parts.connection_endpoints(current.part)?
                == crate::ConnectionEndpoints::new(source_part, target_part)
        {
            if self.configure_connection_route(current.connection, router, constraint)? {
                self.resolve_connection(current.connection)?;
            }
            return Ok(());
        }
        if current.registered {
            self.detach_connection_binding(current.part, !source_reusable, !target_reusable)?;
            current = self
                .connection_projections
                .get(&descriptor.id())
                .cloned()
                .ok_or(ViewerError::InconsistentState)?;
        }

        let source_anchor = if source_reusable {
            current.source_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(source_anchor_strategy)
        })?;
        let target_anchor = if target_reusable {
            current.target_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(target_anchor_strategy)
        })?;
        self.parts
            .bind_connection(current.part, source_part, target_part)?;
        self.runtime.register_connection_state(
            current.connection.figure(),
            Some(source_anchor),
            Some(target_anchor),
            router,
            constraint,
        )?;
        let connection = current.connection;
        let projection = self
            .connection_projections
            .get_mut(&descriptor.id())
            .ok_or(ViewerError::InconsistentState)?;
        projection.source_model = descriptor.source();
        projection.target_model = descriptor.target();
        projection.source_anchor = Some(source_anchor);
        projection.target_anchor = Some(target_anchor);
        projection.source_anchor_key = Some(source_anchor_key);
        projection.target_anchor_key = Some(target_anchor_key);
        projection.registered = true;
        self.resolve_connection(connection)
    }

    fn resolve_connection_routing(
        &self,
        descriptor: ConnectionRoutingDescriptor,
    ) -> Result<(RouterBinding, Option<Box<dyn novadraw::RoutingConstraint>>), ViewerError> {
        let (selection, constraint) = descriptor.into_parts();
        let binding = match selection {
            ConnectionRouterSelection::Inherited => RouterBinding::Inherited {
                layer: self.root_layers.connection(),
            },
            ConnectionRouterSelection::Registered(key) => RouterBinding::Explicit {
                router: *self.connection_routers.get(&key).ok_or_else(|| {
                    EditPartError::operation(format!(
                        "unknown connection Router key {}",
                        key.as_str()
                    ))
                })?,
            },
        };
        Ok((binding, constraint))
    }

    fn validate_connection_bendpoints(
        model_id: A::ModelId,
        bendpoints: &[Point],
    ) -> Result<(), ViewerError> {
        if bendpoints
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            Err(ViewerError::InvalidConnectionBendpoint {
                connection: format!("{model_id:?}"),
            })
        } else {
            Ok(())
        }
    }

    fn configure_connection_route(
        &mut self,
        connection: ConnectionId,
        router: RouterBinding,
        constraint: Option<Box<dyn novadraw::RoutingConstraint>>,
    ) -> Result<bool, ViewerError> {
        self.runtime
            .set_connection_route_configuration(connection, router, constraint)
            .map_err(Into::into)
    }

    fn parts_to_retire(
        &self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<HashSet<EditPartId>, ViewerError> {
        let mut retiring = HashSet::new();
        for part in self.parts.subtree_ids(self.contents())?.into_iter().skip(1) {
            if retiring.contains(&part) {
                continue;
            }
            let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let actual_parent = self
                .parts
                .parent(part)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            if !snapshot.contains_model(model_id)
                || snapshot.parents.get(&model_id).copied() != actual_parent
            {
                retiring.extend(self.parts.subtree_ids(part)?);
            }
        }
        Ok(retiring)
    }

    fn prepare_connections_for_containment_change(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
        retiring: &HashSet<EditPartId>,
    ) -> Result<(), ViewerError> {
        let current = self.parts.connection_parts().to_vec();
        for connection in current {
            let model_id = self
                .parts
                .get(connection.edit_part())
                .and_then(|node| node.model_id())
                .ok_or(ViewerError::InconsistentState)?;
            let Some(desired) = snapshot.connection(model_id) else {
                self.remove_connection_part(connection)?;
                continue;
            };
            let endpoints = self.parts.connection_endpoints(connection)?;
            let projection = self
                .connection_projections
                .get(&model_id)
                .ok_or(ViewerError::InconsistentState)?;
            let source_changes = projection.source_model != desired.source()
                || retiring.contains(&endpoints.source());
            let target_changes = projection.target_model != desired.target()
                || retiring.contains(&endpoints.target());
            if source_changes || target_changes {
                self.detach_connection_binding(connection, source_changes, target_changes)?;
            }
        }
        Ok(())
    }

    fn detach_connection_binding(
        &mut self,
        connection: ConnectionPartId,
        remove_source: bool,
        remove_target: bool,
    ) -> Result<(), ViewerError> {
        let model_id = self
            .parts
            .get(connection.edit_part())
            .and_then(|node| node.model_id())
            .ok_or(ViewerError::InconsistentState)?;
        let projection = self
            .connection_projections
            .get(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        if projection.registered {
            self.runtime
                .remove_connection_state(projection.connection)?;
        }
        let source_anchor = remove_source.then_some(projection.source_anchor).flatten();
        let target_anchor = remove_target.then_some(projection.target_anchor).flatten();
        if let Some(anchor) = source_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        if let Some(anchor) = target_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        self.parts.unbind_connection(connection)?;
        let projection = self
            .connection_projections
            .get_mut(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        projection.registered = false;
        if remove_source {
            projection.source_anchor = None;
            projection.source_anchor_key = None;
        }
        if remove_target {
            projection.target_anchor = None;
            projection.target_anchor_key = None;
        }
        Ok(())
    }

    fn remove_connection_part(&mut self, connection: ConnectionPartId) -> Result<(), ViewerError> {
        let part = connection.edit_part();
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let primary = node.primary_figure();
        let visuals = node.visuals().to_vec();
        self.selection.reconcile(|selected| selected != part);
        let overlays: Vec<_> = self
            .visual_registry
            .iter()
            .filter_map(|(figure, owner)| match owner {
                VisualOwner::Handle { owner, .. } if *owner == part => Some(*figure),
                VisualOwner::Feedback {
                    owner: Some(owner), ..
                } if *owner == part => Some(*figure),
                _ => None,
            })
            .collect();
        for overlay in overlays {
            if self.runtime.tree().is_attached(overlay) {
                self.runtime.dispose_subtree(overlay)?;
            }
            self.visual_registry.remove(&overlay);
        }
        self.deactivate_part(part)?;
        let projection = self
            .connection_projections
            .remove(&model_id)
            .ok_or(ViewerError::InconsistentState)?;
        if projection.registered {
            self.runtime
                .remove_connection_state(projection.connection)?;
        }
        if let Some(anchor) = projection.source_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        if let Some(anchor) = projection.target_anchor {
            self.runtime.remove_connection_anchor(anchor)?;
        }
        self.parts.unbind_connection(connection)?;
        self.model_registry.remove(&model_id);
        for visual in visuals {
            self.visual_registry.remove(&visual);
        }
        self.policies.remove(part);
        self.behaviors.remove(part);
        self.runtime.dispose_subtree(primary)?;
        self.parts.retire_connection(connection)?;
        Ok(())
    }

    fn synchronize_subtree(
        &mut self,
        part: EditPartId,
        model_id: A::ModelId,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        self.refresh_part_visuals(part)?;
        let desired = snapshot.children_of(model_id)?.to_vec();
        let existing = self
            .parts
            .children(part)
            .ok_or(ViewerError::InconsistentState)?
            .to_vec();

        for child in existing.iter().copied() {
            let child_model = self
                .parts
                .get(child)
                .and_then(|node| node.model_id())
                .ok_or(ViewerError::InconsistentState)?;
            if !desired.contains(&child_model) {
                self.remove_subtree(child)?;
            }
        }

        for (index, child_model) in desired.iter().copied().enumerate() {
            let child = match self.model_registry.get(&child_model).copied() {
                Some(existing) if self.parts.parent(existing) == Some(part) => existing,
                Some(_) => return Err(ViewerError::DuplicateModel),
                None => self.create_subtree(part, child_model, snapshot)?,
            };
            let primary = self
                .parts
                .get(child)
                .ok_or(ViewerError::InconsistentState)?
                .primary_figure();
            let content_pane = self
                .parts
                .get(part)
                .ok_or(ViewerError::InconsistentState)?
                .content_pane();
            self.runtime
                .container(content_pane)?
                .move_child_to_index(primary, index)?;
            self.parts.reorder_child(part, child, index)?;
            self.synchronize_subtree(child, child_model, snapshot)?;
        }
        Ok(())
    }

    fn remove_reparented_subtrees(
        &mut self,
        snapshot: &ModelSnapshot<A::ModelId>,
    ) -> Result<(), ViewerError> {
        let ids = self.parts.subtree_ids(self.contents())?;
        for part in ids.into_iter().skip(1) {
            let Some(node) = self.parts.get(part) else {
                continue;
            };
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let actual_parent = self
                .parts
                .parent(part)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            if snapshot.parents.get(&model_id).copied() != actual_parent {
                self.remove_subtree(part)?;
            }
        }
        Ok(())
    }

    fn refresh_part_visuals(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let node = self.parts.get(part).ok_or(ViewerError::InconsistentState)?;
        let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
        let mut context = VisualUpdateContext::new(
            &mut self.runtime,
            node.primary_figure(),
            node.content_pane(),
        );
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .refresh_visuals(&self.model, model_id, &mut context)?;
        Ok(())
    }

    fn remove_subtree(&mut self, part: EditPartId) -> Result<(), ViewerError> {
        let ids = self.parts.subtree_ids(part)?;
        if self
            .direct_text_edit
            .as_ref()
            .is_some_and(|active| ids.contains(&active.state.source()))
        {
            self.cancel_direct_text_edit()?;
        }
        let primary = self
            .parts
            .get(part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        self.selection
            .reconcile(|selected| !ids.contains(&selected));
        let overlays: Vec<_> = self
            .visual_registry
            .iter()
            .filter_map(|(figure, owner)| match owner {
                VisualOwner::Handle { owner, .. } if ids.contains(owner) => Some(*figure),
                VisualOwner::Feedback {
                    owner: Some(owner), ..
                } if ids.contains(owner) => Some(*figure),
                _ => None,
            })
            .collect();
        for overlay in overlays {
            if self.runtime.tree().is_attached(overlay) {
                self.runtime.dispose_subtree(overlay)?;
            }
            self.visual_registry.remove(&overlay);
        }

        for id in ids.iter().copied() {
            let node = self.parts.get(id).ok_or(ViewerError::InconsistentState)?;
            let model_id = node.model_id().ok_or(ViewerError::InconsistentState)?;
            let visuals = node.visuals().to_vec();
            self.deactivate_part(id)?;
            self.model_registry.remove(&model_id);
            for visual in visuals {
                self.visual_registry.remove(&visual);
            }
        }
        for id in ids.iter().rev().copied() {
            self.policies.remove(id);
            self.behaviors.remove(id);
        }
        self.runtime.dispose_subtree(primary)?;
        self.parts.retire_subtree(part)?;
        Ok(())
    }
}

impl<A, F> Drop for GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    fn drop(&mut self) {
        for connection in self.parts.connection_parts().to_vec() {
            let part = connection.edit_part();
            let Some(node) = self.parts.get(part) else {
                continue;
            };
            let Some(model_id) = (if node.is_active() {
                node.model_id()
            } else {
                None
            }) else {
                continue;
            };
            let host = PolicyHost::new(part, model_id, None);
            if let Some(roles) = self.policies.roles_mut(part) {
                for policy in roles.values_mut() {
                    policy.deactivate(host, &self.model);
                }
            }
            if let Some(behavior) = self.behaviors.get_mut(part) {
                behavior.deactivate(&self.model, model_id);
            }
        }
        let Ok(ids) = self.parts.subtree_ids(self.parts.root()) else {
            return;
        };
        for id in ids.into_iter().skip(1) {
            let Some(node) = self.parts.get(id) else {
                continue;
            };
            let Some(model_id) = (if node.is_active() {
                node.model_id()
            } else {
                None
            }) else {
                continue;
            };
            let parent_model = self
                .parts
                .parent(id)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            let host = PolicyHost::new(id, model_id, parent_model);
            if let Some(roles) = self.policies.roles_mut(id) {
                for policy in roles.values_mut() {
                    policy.deactivate(host, &self.model);
                }
            }
            if let Some(behavior) = self.behaviors.get_mut(id) {
                behavior.deactivate(&self.model, model_id);
            }
        }
    }
}
