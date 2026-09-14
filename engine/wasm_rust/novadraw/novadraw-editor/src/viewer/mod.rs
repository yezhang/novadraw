//! Viewer contents, registries, and model-to-Figure projection.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use novadraw_geometry::{Rectangle, Translatable};
use novadraw_scene::{
    AnchorId, ChopboxAnchor, ConnectionId, ConnectionLayerFigure, ConnectionRuntimeError,
    CoordinateSpace, DispatchOutcome, Figure, FigureId, FigureTree, KeyModifiers, LayerError,
    LayerFigure, LayerKey, LayerPlacement, LayeredPane, MouseButton, RouterBinding, Runtime,
    RuntimeMutationError, ScalableFreeformLayeredPane, StackLayout,
};

use crate::{
    Command, CompoundCommand, ConnectionCreation, ConnectionPartFactoryContext, ConnectionPartId,
    CreateConnectionRequest, EditPartError, EditPartFactory, EditPartId, EditorNamespace,
    EditorRequest, FeedbackId, HandleId, HandleRole, ModelAdapter, ModelConnection, ModelRevision,
    PartFactoryContext, PartKind, PartTree, PartTreeError, PolicyError, PolicyHost, SelectionDelta,
    SelectionModel, VisualBuildContext, VisualOwner, VisualUpdateContext,
    part::{BehaviorStore, validate_runtime_namespace},
    policy::PolicyStore,
};

const MAX_PART_TREE_DEPTH: usize = 10_000;
const SCALABLE_LAYERS: &str = "scalable";
const GRID_LAYER: &str = "grid";
const PRINTABLE_LAYERS: &str = "printable";
const PRIMARY_LAYER: &str = "primary";
const CONNECTION_LAYER: &str = "connection";
const SCALED_FEEDBACK_LAYER: &str = "scaled-feedback";
const FEEDBACK_LAYER: &str = "feedback";
const HANDLE_LAYER: &str = "handles";

/// Stable Figure identities for the standard graphical Viewer layer topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootLayers {
    root: FigureId,
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
    /// Connection Runtime mutation or routing failed.
    Connection(ConnectionRuntimeError),
    /// Root layer construction or mutation failed.
    Layer(LayerError),
    /// EditPolicy rejected a request or failed.
    Policy(PolicyError),
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
            Self::Connection(error) => error.fmt(formatter),
            Self::Layer(error) => error.fmt(formatter),
            Self::Policy(error) => error.fmt(formatter),
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
            Self::Connection(error) => Some(error),
            Self::Layer(error) => Some(error),
            Self::Policy(error) => Some(error),
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
    let scalable = add_layer(
        &mut runtime,
        root,
        Box::new(ScalableFreeformLayeredPane::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )),
        SCALABLE_LAYERS,
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
        Box::new(LayeredPane::new(
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
        Box::new(LayerFigure::new(
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
        Box::new(LayerFigure::new(
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
    runtime.set_layout_manager(root, Box::new(StackLayout::new()))?;
    runtime.set_layout_manager(scalable, Box::new(StackLayout::new()))?;
    runtime.set_layout_manager(printable, Box::new(StackLayout::new()))?;
    Ok((
        runtime,
        RootLayers {
            root,
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

#[derive(Clone, Copy)]
struct ConnectionProjection<I> {
    part: ConnectionPartId,
    connection: ConnectionId,
    source_model: I,
    target_model: I,
    source_anchor: Option<AnchorId>,
    target_anchor: Option<AnchorId>,
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
    visual_registry: HashMap<FigureId, VisualOwner>,
    selection: SelectionModel<EditPartId>,
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
    pub fn new(mut model: A, factory: F, bounds: Rectangle) -> Result<Self, ViewerError> {
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
        let (runtime, root_layers) = create_root_layers(bounds)?;
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
            visual_registry: HashMap::from([(root_layers.root(), VisualOwner::Part(root_part))]),
            selection: SelectionModel::new(),
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

    /// Returns mutable application model access for commands and host integration.
    pub fn model_mut(&mut self) -> &mut A {
        &mut self.model
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
            .try_add_figure(self.root_layers.handles(), figure)?;
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
        let figure = self.runtime.try_add_figure(parent, figure)?;
        let id = FeedbackId::new(self.namespace());
        self.visual_registry
            .insert(figure, VisualOwner::Feedback { id, owner });
        Ok((id, figure))
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

    /// Resolves deterministic policy command contributions for a request.
    pub fn command_for_request(
        &mut self,
        request: &EditorRequest,
    ) -> Result<Option<Box<dyn Command<A>>>, ViewerError>
    where
        A: 'static,
    {
        let mut seen = HashSet::new();
        let mut commands = Vec::new();
        for &part in request.source_parts() {
            self.validate_selectable(part)?;
            if !seen.insert(part) {
                return Err(
                    PolicyError::operation("request contains a duplicate source part").into(),
                );
            }
            let host = self.policy_host(part)?;
            let Some(roles) = self.policies.roles_mut(part) else {
                continue;
            };
            for policy in roles.values_mut() {
                if policy.understands(request)
                    && policy.target(host, request).is_some()
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
        let mut seen = HashSet::new();
        let mut contributions = Vec::new();
        for &part in request.source_parts() {
            self.validate_selectable(part)?;
            if !seen.insert(part) {
                return Err(
                    PolicyError::operation("request contains a duplicate source part").into(),
                );
            }
            let host = self.policy_host(part)?;
            let Some(roles) = self.policies.roles_mut(part) else {
                continue;
            };
            for policy in roles.values_mut() {
                if policy.understands(request) && policy.target(host, request).is_some() {
                    contributions.extend(
                        policy
                            .feedback(host, request, &self.model)?
                            .into_iter()
                            .map(|feedback| (part, feedback)),
                    );
                }
            }
        }
        let mut figures = Vec::with_capacity(contributions.len());
        for (owner, feedback) in contributions {
            let (figure, scaled) = feedback.into_parts();
            let (_, figure) = self.add_feedback_visual(Some(owner), scaled, figure)?;
            figures.push(figure);
        }
        Ok(figures)
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
        let contributions = plan.feedback(source, target, request, &self.model)?;
        let mut figures = Vec::with_capacity(contributions.len());
        for feedback in contributions {
            let (figure, scaled) = feedback.into_parts();
            match self.add_feedback_visual(Some(request.source()), scaled, figure) {
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
        if self.faulted {
            return Err(ViewerError::Faulted);
        }
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
        self.faulted = true;
        Err(error)
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
        let primary = self.runtime.try_add_figure(
            parent_figure,
            behavior.create_figure(&self.model, model_id)?,
        )?;
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
        let activation = self
            .behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .activate(&self.model, model_id);
        if let Err(error) = activation {
            self.behaviors
                .get_mut(part)
                .expect("behavior exists until failed creation is discarded")
                .deactivate(&self.model, model_id);
            return Err(error.into());
        }
        self.parts.set_active(part, true)?;
        let host = self.policy_host(part)?;
        if let Some(roles) = self.policies.roles_mut(part) {
            for policy in roles.values_mut() {
                if let Err(error) = policy.activate(host, &self.model) {
                    for active in roles.values_mut() {
                        active.deactivate(host, &self.model);
                    }
                    return Err(error.into());
                }
            }
        }

        for child in snapshot.children_of(model_id)?.iter().copied() {
            self.create_subtree(part, child, snapshot)?;
        }
        Ok(part)
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
        let primary = self.runtime.try_add_figure(
            self.root_layers.connection(),
            behavior.create_figure(&self.model, descriptor.id())?,
        )?;
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
        let source_figure = self
            .parts
            .get(source_part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        let target_figure = self
            .parts
            .get(target_part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        let source_anchor = self
            .runtime
            .try_register_connection_anchor(Box::new(ChopboxAnchor::new(source_figure)))?;
        let target_anchor = match self
            .runtime
            .try_register_connection_anchor(Box::new(ChopboxAnchor::new(target_figure)))
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
            RouterBinding::Inherited {
                layer: self.root_layers.connection(),
            },
            None,
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
                registered: true,
            },
        );
        let initialization = (|| {
            for (role, policy) in policies {
                self.policies.install(part.edit_part(), role, policy)?;
            }
            self.refresh_part_visuals(part.edit_part())?;
            self.behaviors
                .get_mut(part.edit_part())
                .ok_or(ViewerError::InconsistentState)?
                .activate(&self.model, descriptor.id())?;
            self.parts.set_active(part.edit_part(), true)?;
            let host = self.policy_host(part.edit_part())?;
            if let Some(roles) = self.policies.roles_mut(part.edit_part()) {
                for policy in roles.values_mut() {
                    policy.activate(host, &self.model)?;
                }
            }
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
            order.push(connection);
        }
        for (index, connection) in order.iter().copied().enumerate() {
            let figure = self
                .parts
                .get(connection.edit_part())
                .ok_or(ViewerError::InconsistentState)?
                .primary_figure();
            self.runtime
                .move_child_to_index(self.root_layers.connection(), figure, index)?;
        }
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
            .copied()
            .ok_or(ViewerError::InconsistentState)?;
        if current.registered
            && current.source_model == descriptor.source()
            && current.target_model == descriptor.target()
            && self.parts.connection_endpoints(current.part)?
                == crate::ConnectionEndpoints::new(source_part, target_part)
        {
            return Ok(());
        }
        if current.registered {
            self.detach_connection_binding(current.part, true, true)?;
            current = self
                .connection_projections
                .get(&descriptor.id())
                .copied()
                .ok_or(ViewerError::InconsistentState)?;
        }

        let source_figure = self
            .parts
            .get(source_part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        let target_figure = self
            .parts
            .get(target_part)
            .ok_or(ViewerError::InconsistentState)?
            .primary_figure();
        let source_unchanged = current.source_model == descriptor.source();
        let target_unchanged = current.target_model == descriptor.target();
        let source_anchor = if source_unchanged {
            current.source_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(Box::new(ChopboxAnchor::new(source_figure)))
        })?;
        let target_anchor = if target_unchanged {
            current.target_anchor
        } else {
            None
        }
        .map(Ok)
        .unwrap_or_else(|| {
            self.runtime
                .try_register_connection_anchor(Box::new(ChopboxAnchor::new(target_figure)))
        })?;
        self.parts
            .bind_connection(current.part, source_part, target_part)?;
        self.runtime.register_connection_state(
            current.connection.figure(),
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Inherited {
                layer: self.root_layers.connection(),
            },
            None,
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
        projection.registered = true;
        self.resolve_connection(connection)
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
        }
        if remove_target {
            projection.target_anchor = None;
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
        let host = PolicyHost::new(part, model_id, None);
        if let Some(roles) = self.policies.roles_mut(part) {
            for policy in roles.values_mut() {
                policy.deactivate(host, &self.model);
            }
        }
        self.behaviors
            .get_mut(part)
            .ok_or(ViewerError::InconsistentState)?
            .deactivate(&self.model, model_id);
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
                .move_child_to_index(content_pane, primary, index)?;
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
            let parent_model = self
                .parts
                .parent(id)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            let host = PolicyHost::new(id, model_id, parent_model);
            let visuals = node.visuals().to_vec();
            if let Some(roles) = self.policies.roles_mut(id) {
                for policy in roles.values_mut() {
                    policy.deactivate(host, &self.model);
                }
            }
            self.behaviors
                .get_mut(id)
                .ok_or(ViewerError::InconsistentState)?
                .deactivate(&self.model, model_id);
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
        let Some(contents) = self.parts.contents() else {
            return;
        };
        let Ok(ids) = self.parts.subtree_ids(contents) else {
            return;
        };
        for id in ids {
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
