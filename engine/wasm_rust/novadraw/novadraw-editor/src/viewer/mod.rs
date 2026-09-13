//! Viewer contents, registries, and model-to-Figure projection.

use std::{collections::HashMap, error::Error, fmt};

use novadraw_geometry::{Rectangle, Translatable};
use novadraw_scene::{
    DispatchOutcome, Figure, FigureId, FigureTree, KeyModifiers, LayerError, LayerFigure, LayerKey,
    LayerPlacement, LayeredPane, MouseButton, Runtime, RuntimeMutationError,
    ScalableFreeformLayeredPane, StackLayout,
};

use crate::{
    EditPartError, EditPartFactory, EditPartId, EditorNamespace, FeedbackId, HandleId,
    ModelAdapter, ModelRevision, PartFactoryContext, PartTree, PartTreeError, SelectionDelta,
    SelectionModel, VisualBuildContext, VisualOwner, VisualUpdateContext,
    part::{BehaviorStore, validate_runtime_namespace},
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
    /// Root layer construction or mutation failed.
    Layer(LayerError),
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
            Self::Layer(error) => error.fmt(formatter),
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
            Self::Layer(error) => Some(error),
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

impl From<LayerError> for ViewerError {
    fn from(value: LayerError) -> Self {
        Self::Layer(value)
    }
}

struct ModelSnapshot<I> {
    root: I,
    children: HashMap<I, Vec<I>>,
    parents: HashMap<I, I>,
}

impl<I> ModelSnapshot<I>
where
    I: Copy + Eq + std::hash::Hash,
{
    fn capture<A>(model: &A) -> Result<Self, ViewerError>
    where
        A: ModelAdapter<ModelId = I>,
    {
        let root = model.root();
        let mut children = HashMap::new();
        let mut parents = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        Self::capture_subtree(model, root, 0, &mut seen, &mut children, &mut parents)?;
        Ok(Self {
            root,
            children,
            parents,
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
        Box::new(LayerFigure::new(
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
    model_registry: HashMap<A::ModelId, EditPartId>,
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
        let applied_revision = model.revision();
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
            model_registry: HashMap::new(),
            visual_registry: HashMap::from([(root_layers.root(), VisualOwner::Part(root_part))]),
            selection: SelectionModel::new(),
            applied_revision,
            model_root: snapshot.root,
            faulted: false,
        };
        let contents = viewer.create_subtree(root_part, snapshot.root, &snapshot)?;
        viewer.parts.designate_contents(contents)?;
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
                Some((_, VisualOwner::Handle { id, owner })) => {
                    return ViewerTarget::Handle { id, owner };
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
                    let delta = self.select_part(part, mode)?;
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
        self.validate_selectable(owner)?;
        let figure = self
            .runtime
            .try_add_figure(self.root_layers.handles(), figure)?;
        let id = HandleId::new(self.namespace());
        self.visual_registry
            .insert(figure, VisualOwner::Handle { id, owner });
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
            Ok(snapshot) if snapshot.root == self.model_root => snapshot,
            Ok(_) => return self.fail(ViewerError::RootChanged),
            Err(error) => return self.fail(error),
        };
        let result = self
            .remove_reparented_subtrees(&snapshot)
            .and_then(|()| self.synchronize_subtree(self.contents(), snapshot.root, &snapshot));
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

        for child in snapshot.children_of(model_id)?.iter().copied() {
            self.create_subtree(part, child, snapshot)?;
        }
        Ok(part)
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
            let visuals = node.visuals().to_vec();
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
        let Some(contents) = self.parts.contents() else {
            return;
        };
        let Ok(ids) = self.parts.subtree_ids(contents) else {
            return;
        };
        for id in ids {
            let Some(model_id) = self.parts.get(id).and_then(|node| {
                if node.is_active() {
                    node.model_id()
                } else {
                    None
                }
            }) else {
                continue;
            };
            if let Some(behavior) = self.behaviors.get_mut(id) {
                behavior.deactivate(&self.model, model_id);
            }
        }
    }
}
