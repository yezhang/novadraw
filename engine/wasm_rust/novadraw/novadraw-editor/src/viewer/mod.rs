//! Viewer contents, registries, and model-to-Figure projection.

use std::{collections::HashMap, error::Error, fmt};

use novadraw_geometry::Rectangle;
use novadraw_scene::{FigureId, FigureTree, RootFigure, Runtime, RuntimeMutationError};

use crate::{
    EditPartError, EditPartFactory, EditPartId, EditorNamespace, ModelAdapter, ModelRevision,
    PartFactoryContext, PartTree, PartTreeError, VisualBuildContext, VisualUpdateContext,
    part::{BehaviorStore, validate_runtime_namespace},
};

const MAX_PART_TREE_DEPTH: usize = 10_000;

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

/// Owns one application model projection, its EditPart tree, and its Figure Runtime.
pub struct GraphicalViewer<A, F>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    model: A,
    factory: F,
    runtime: Runtime,
    parts: PartTree<A::ModelId>,
    behaviors: BehaviorStore<A>,
    model_registry: HashMap<A::ModelId, EditPartId>,
    visual_registry: HashMap<FigureId, EditPartId>,
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
        let mut tree = FigureTree::new();
        let root_figure = tree.builder().set_contents(Box::new(RootFigure::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )));
        let runtime = Runtime::new(tree);
        let parts = PartTree::new(root_figure);
        let namespace = parts.namespace();
        let root_part = parts.root();
        let mut viewer = Self {
            model,
            factory,
            runtime,
            parts,
            behaviors: BehaviorStore::new(namespace),
            model_registry: HashMap::new(),
            visual_registry: HashMap::from([(root_figure, root_part)]),
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

    /// Resolves a model identity to its current EditPart.
    pub fn part_for_model(&self, model_id: A::ModelId) -> Option<EditPartId> {
        self.model_registry.get(&model_id).copied()
    }

    /// Resolves a directly registered visual to its owner part.
    pub fn part_for_visual(&self, figure: FigureId) -> Option<EditPartId> {
        self.visual_registry.get(&figure).copied()
    }

    /// Resolves a visual or its nearest registered ancestor to an owner part.
    pub fn part_for_visual_or_ancestor(&self, figure: FigureId) -> Option<EditPartId> {
        if figure.namespace() != self.runtime.tree().namespace()
            || !self.runtime.tree().is_attached(figure)
        {
            return None;
        }
        self.part_for_visual(figure).or_else(|| {
            self.runtime
                .tree()
                .ancestor_ids(figure)?
                .into_iter()
                .find_map(|ancestor| self.part_for_visual(ancestor))
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
            self.visual_registry.insert(visual, part);
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
