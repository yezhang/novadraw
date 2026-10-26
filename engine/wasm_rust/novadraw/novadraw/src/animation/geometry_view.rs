use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    Affine2D, ChildClippingStrategy, FigureId, FigureTree, Rectangle, figure::FigurePresentation,
};

use super::{AnimationError, AnimationSampleVersion, AnimationValue};

pub(crate) type GeometryValues = HashMap<(FigureId, TypeId), Arc<dyn Any + Send + Sync>>;

/// Actual work performed by one immutable geometry pass; cached reads add no work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeometryMaterializationWork {
    /// Distinct nodes visited by the recursive geometry protocol.
    pub nodes: usize,
    /// Child-order entries inspected, including unselected siblings of read-only ancestors.
    pub edges: usize,
    /// Affine compositions and transformed rectangle corners.
    pub matrices: usize,
    /// Clip intersections.
    pub clips: usize,
    /// Self/child envelope calculations.
    pub envelopes: usize,
}

/// Complete geometry identity, independent of recorded submissions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationVersion {
    /// Last accepted Motion sampling time and active-owner revision.
    pub sample: AnimationSampleVersion,
    /// Source stabilization epoch used to materialize the view.
    pub source_epoch: u64,
    /// Revision of frozen geometry descriptor registrations and committed channel inputs.
    pub binding_revision: u64,
}

/// One owned capture. Consuming it transfers its frozen starting geometry to a new plan.
pub struct GeometryCapture {
    pub(crate) geometry: Arc<PresentationGeometry>,
    pub(crate) _retention: Arc<super::storage::CaptureStorageLease>,
}

impl GeometryCapture {
    /// Borrows the complete frozen starting geometry.
    pub fn geometry(&self) -> &PresentationGeometry {
        &self.geometry
    }
}

/// Failure while stabilizing source inputs or materializing cached samples.
#[derive(Debug)]
pub enum GeometryCaptureError {
    /// Source stabilization failed before materialization.
    Source(crate::runtime::FramePreparationError),
    /// Identity, value, or geometry materialization failed.
    Animation(AnimationError),
}

impl std::fmt::Display for GeometryCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Animation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeometryCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Animation(error) => Some(error),
        }
    }
}

/// Immutable geometry for one declared Figure dependency.
pub struct PresentedFigureGeometry {
    /// Bounds in parent child-content coordinates.
    pub bounds: Rectangle,
    /// Local client rectangle computed from sampled size and frozen insets.
    pub client_box: Rectangle,
    /// Explicit local transform; bounds size introduces no scale.
    pub local_transform: Affine2D,
    /// Includes the frozen client-origin translation.
    pub child_transform: Affine2D,
    /// Complete local-to-surface edge composition.
    pub local_to_surface: Affine2D,
    /// Conjunction of local and ancestor presentation visibility.
    pub effective_visible: bool,
    /// `None` means unbounded. Empty clips are represented by a zero-sized rectangle.
    pub effective_clip: Option<Rectangle>,
    /// Surface envelope of visible self-content and selected descendants.
    pub visual_envelope: Option<Rectangle>,
    pub(crate) parent: Option<FigureId>,
    pub(crate) children: Vec<FigureId>,
    pub(crate) content: Option<FigurePresentation>,
    pub(crate) border: Option<FigurePresentation>,
}

impl PresentedFigureGeometry {
    /// Frozen parent identity, including generation.
    pub fn parent(&self) -> Option<FigureId> {
        self.parent
    }

    /// Stable committed child order captured with this geometry.
    pub fn children(&self) -> &[FigureId] {
        &self.children
    }
}

/// A frozen, read-only geometry view. Queries neither sample Motion nor prepare a frame.
pub struct PresentationGeometry {
    version: PresentationVersion,
    namespace: crate::identity::RuntimeNamespace,
    pub(crate) data: Arc<PresentationGeometryData>,
}

pub(crate) struct PresentationGeometryData {
    pub(crate) roots: Vec<FigureId>,
    pub(crate) covered: HashSet<FigureId>,
    pub(crate) order: Vec<FigureId>,
    pub(crate) nodes: HashMap<FigureId, PresentedFigureGeometry>,
    pub(crate) values: GeometryValues,
    sources: HashMap<FigureId, Arc<super::frozen::FrozenGeometrySource>>,
    // Static capture materialization can outlive its original capture through the Runtime
    // cache or another independently charged capture. Shared data owns the retention lease.
    capture_retention: Option<Arc<super::storage::CaptureStorageLease>>,
    work: GeometryMaterializationWork,
}

impl PresentationGeometry {
    pub(crate) fn retain_capture(&mut self, lease: Arc<super::storage::CaptureStorageLease>) {
        Arc::get_mut(&mut self.data)
            .expect("unpublished capture geometry is exclusively owned")
            .capture_retention = Some(lease);
    }

    pub(crate) fn sources_valid(
        &self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
    ) -> bool {
        let mut seen = HashSet::new();
        self.data
            .sources
            .values()
            .all(|source| !seen.insert(Arc::as_ptr(source)) || source.is_valid(tree, resources))
    }

    pub(crate) fn frozen_node(&self, id: FigureId) -> Option<&super::frozen::FrozenGeometryNode> {
        self.data.sources.get(&id)?.nodes.get(&id)
    }

    /// Charges collection storage conservatively, including independently retained sources.
    /// Frozen content commands and resource revisions are included in source storage.
    /// Geometry values use fixed owned `Copy` representations; their allocations are charged.
    /// Size-dependent recordings are bounded by the frozen descriptor's declaration.
    pub(crate) fn collection_bytes(&self) -> Result<usize, AnimationError> {
        let mut bytes =
            std::mem::size_of::<Self>() + std::mem::size_of::<PresentationGeometryData>();
        let mut add = |count: usize, item: usize| -> Result<(), AnimationError> {
            bytes = count
                .checked_mul(item)
                .and_then(|size| bytes.checked_add(size))
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
            Ok(())
        };
        add(self.data.roots.capacity(), std::mem::size_of::<FigureId>())?;
        add(self.data.order.capacity(), std::mem::size_of::<FigureId>())?;
        // Hash tables can contain spare buckets and control bytes. Twice the usable capacity
        // bounds both for the nonzero key/value sizes used here.
        add(
            self.data.covered.capacity(),
            2 * (std::mem::size_of::<FigureId>() + 1),
        )?;
        add(
            self.data.nodes.capacity(),
            2 * (std::mem::size_of::<(FigureId, PresentedFigureGeometry)>() + 1),
        )?;
        add(
            self.data.values.capacity(),
            2 * (std::mem::size_of::<((FigureId, TypeId), Arc<dyn Any + Send + Sync>)>() + 1),
        )?;
        add(
            self.data.sources.capacity(),
            2 * (std::mem::size_of::<(FigureId, Arc<super::frozen::FrozenGeometrySource>)>() + 1),
        )?;
        for value in self.data.values.values() {
            // The Arc allocation includes two reference counters in addition to its payload.
            add(
                1,
                std::mem::size_of_val(value.as_ref()) + 2 * std::mem::size_of::<usize>(),
            )?;
        }
        for (id, node) in &self.data.nodes {
            add(node.children.capacity(), std::mem::size_of::<FigureId>())?;
            if let Some(frozen) = self.frozen_node(*id) {
                for drawing in [
                    frozen.bounds_drawing.as_ref(),
                    frozen.border_drawing.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    add(1, drawing.recording_bytes())?;
                }
            }
        }
        let mut seen = HashSet::new();
        for source in self.data.sources.values() {
            if seen.insert(Arc::as_ptr(source)) {
                add(1, source.collection_bytes()?)?;
            }
        }
        Ok(bytes)
    }

    pub(crate) fn with_version(&self, version: PresentationVersion) -> Self {
        Self {
            version,
            namespace: self.namespace,
            data: self.data.clone(),
        }
    }

    /// Returns the complete materialization identity.
    pub fn version(&self) -> PresentationVersion {
        self.version
    }

    /// Returns counts from materialization, independent of subsequent queries and recordings.
    pub fn materialization_work(&self) -> GeometryMaterializationWork {
        self.data.work
    }

    /// Queries a declared Figure; never expands the dependency closure.
    pub fn figure(&self, figure: FigureId) -> Result<&PresentedFigureGeometry, AnimationError> {
        if figure.namespace() != self.namespace {
            return Err(AnimationError::ForeignTarget);
        }
        self.data
            .nodes
            .get(&figure)
            .ok_or(AnimationError::UndeclaredGeometryDependency { figure })
    }

    /// Queries a semantic channel value frozen with the same geometry version.
    pub fn value<V: AnimationValue>(&self, figure: FigureId) -> Result<&V, AnimationError> {
        self.figure(figure)?;
        self.data
            .values
            .get(&(figure, TypeId::of::<V>()))
            .and_then(|value| value.downcast_ref())
            .ok_or(AnimationError::UnknownChannel)
    }

    /// Converts local coordinates through the same edge protocol used for recording.
    ///
    /// A singular destination returns `Ok(None)`, matching committed coordinate queries.
    pub fn relative_transform(
        &self,
        from: FigureId,
        to: FigureId,
    ) -> Result<Option<Affine2D>, AnimationError> {
        let from = self.figure(from)?.local_to_surface;
        let to_world = self.figure(to)?.local_to_surface;
        crate::graph::checked_coordinate_inverse(to, to_world)
            .map_err(AnimationError::Coordinate)?
            .map(|inverse| checked(inverse * from))
            .transpose()
    }

    /// Local coordinates mapped into a container's child-content domain.
    ///
    /// This includes internal scales while excluding the container's scroll/client mapping.
    pub fn to_content_transform(
        &self,
        from: FigureId,
        container: FigureId,
    ) -> Result<Option<Affine2D>, AnimationError> {
        let target = self.figure(container)?;
        let content_world = checked(target.local_to_surface * target.child_transform)?;
        let from_world = self.figure(from)?.local_to_surface;
        crate::graph::checked_coordinate_inverse(container, content_world)
            .map_err(AnimationError::Coordinate)?
            .map(|inverse| checked(inverse * from_world))
            .transpose()
    }

    pub(crate) fn freeze(
        tree: &FigureTree,
        effects: &HashMap<FigureId, super::FigurePresentationEffect>,
        dependencies: impl IntoIterator<Item = FigureId>,
        version: PresentationVersion,
        values: GeometryValues,
    ) -> Result<Self, AnimationError> {
        Self::freeze_sources(tree, effects, dependencies, version, values, &[])
    }

    pub(crate) fn freeze_sources(
        tree: &FigureTree,
        effects: &HashMap<FigureId, super::FigurePresentationEffect>,
        dependencies: impl IntoIterator<Item = FigureId>,
        version: PresentationVersion,
        values: GeometryValues,
        sources: &[Arc<super::frozen::FrozenGeometrySource>],
    ) -> Result<Self, AnimationError> {
        let mut selected = HashSet::new();
        let mut descendants = Vec::new();
        let mut roots: Vec<_> = dependencies.into_iter().collect();
        let mut distinct_roots = HashSet::new();
        roots.retain(|root| distinct_roots.insert(*root));
        // Only maximal roots contribute repair envelopes. Their descendants are expanded once.
        let declared: HashSet<_> = roots.iter().copied().collect();
        let mut ancestry = HashMap::new();
        let mut maximal_roots = Vec::new();
        for root in roots {
            let mut path = Vec::new();
            let mut parent = tree.node(root)?.parent;
            let mut nested = false;
            while let Some(id) = parent {
                if declared.contains(&id) {
                    nested = true;
                    break;
                }
                if let Some(cached) = ancestry.get(&id) {
                    nested = *cached;
                    break;
                }
                path.push(id);
                parent = tree.node(id)?.parent;
            }
            for id in path {
                ancestry.insert(id, nested);
            }
            if !nested {
                maximal_roots.push(root);
            }
        }
        let roots = maximal_roots;
        for figure in &roots {
            let figure = *figure;
            tree.node(figure)?;
            descendants.push(figure);
            let mut parent = Some(figure);
            while let Some(id) = parent {
                if !selected.insert(id) {
                    break;
                }
                parent = tree.node(id)?.parent;
            }
        }
        let mut expanded = HashSet::new();
        while let Some(id) = descendants.pop() {
            if !expanded.insert(id) {
                continue;
            }
            selected.insert(id);
            descendants.extend(tree.node(id)?.children.iter().copied());
        }
        let mut source_nodes: HashMap<FigureId, Arc<super::frozen::FrozenGeometrySource>> =
            HashMap::new();
        for source in sources {
            for (id, node) in &source.nodes {
                let replace = source_nodes
                    .get(id)
                    .is_none_or(|previous| previous.nodes[id].ancestor_only && !node.ancestor_only);
                if replace {
                    source_nodes.insert(*id, source.clone());
                }
            }
        }
        let mut view = Self {
            version,
            namespace: tree.synthetic_root().namespace(),
            data: Arc::new(PresentationGeometryData {
                roots,
                covered: expanded,
                order: Vec::new(),
                nodes: HashMap::new(),
                values,
                sources: source_nodes,
                capture_retention: None,
                work: GeometryMaterializationWork::default(),
            }),
        };
        view.visit(
            tree,
            effects,
            &selected,
            tree.synthetic_root(),
            Affine2D::IDENTITY,
            true,
            None,
        )?;
        let work = view.data.work;
        let actual = work
            .matrices
            .checked_add(work.clips)
            .and_then(|n| n.checked_add(work.envelopes))
            .and_then(|n| n.checked_add(work.edges))
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::GeometryOperations,
            ))?;
        let bound = work
            .nodes
            .checked_mul(super::capacity::GEOMETRY_OPERATIONS_PER_NODE / 3)
            .and_then(|n| n.checked_add(work.edges))
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::GeometryOperations,
            ))?;
        if actual > bound {
            // A violated engine proof is a shared preparation error, not a provider failure.
            return Err(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::GeometryOperations,
            ));
        }
        Ok(view)
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        tree: &FigureTree,
        effects: &HashMap<FigureId, super::FigurePresentationEffect>,
        selected: &HashSet<FigureId>,
        id: FigureId,
        parent_content_world: Affine2D,
        ancestor_visible: bool,
        incoming_clip: Option<Rectangle>,
    ) -> Result<Option<Rectangle>, AnimationError> {
        const STACK_RED_ZONE: usize = 128 * 1024;
        const STACK_GROWTH: usize = 4 * 1024 * 1024;
        stacker::maybe_grow(STACK_RED_ZONE, STACK_GROWTH, || {
            self.visit_inner(
                tree,
                effects,
                selected,
                id,
                parent_content_world,
                ancestor_visible,
                incoming_clip,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_inner(
        &mut self,
        tree: &FigureTree,
        effects: &HashMap<FigureId, super::FigurePresentationEffect>,
        selected: &HashSet<FigureId>,
        id: FigureId,
        parent_content_world: Affine2D,
        ancestor_visible: bool,
        incoming_clip: Option<Rectangle>,
    ) -> Result<Option<Rectangle>, AnimationError> {
        if !selected.contains(&id) {
            return Ok(None);
        }
        let node = tree.node(id)?;
        let mut matrices = 4; // local, world (two compositions), child-content world
        let mut clips = 0;
        let mut envelopes = 1;
        let mut edges = 0;
        let source = self.data.sources.get(&id).cloned();
        let frozen = source.as_ref().and_then(|source| source.nodes.get(&id));
        Arc::get_mut(&mut self.data)
            .expect("unpublished geometry is exclusively owned")
            .order
            .push(id);
        if node.depth > crate::graph::MAX_TREE_DEPTH {
            return Err(AnimationError::InvalidValue);
        }
        let effect = effects.get(&id);
        let patch = effect.map(|effect| effect.geometry).unwrap_or_default();
        let bounds = patch.bounds.unwrap_or_else(|| {
            frozen.map_or_else(
                || node.figure_bounds(),
                super::frozen::FrozenGeometryNode::bounds,
            )
        });
        let local_transform = checked(
            effect
                .and_then(|effect| effect.transform)
                .unwrap_or(Affine2D::IDENTITY)
                * patch.local_transform.unwrap_or(Affine2D::IDENTITY),
        )?;
        let world = checked(
            parent_content_world * Affine2D::from_translation(bounds.x, bounds.y) * local_transform,
        )?;
        let insets = frozen.map_or(node.state().insets, |node| node.inputs.insets);
        let client_box = Rectangle::new(
            insets.left,
            insets.top,
            (bounds.width - insets.width()).max(0.0),
            (bounds.height - insets.height()).max(0.0),
        );
        let child_transform = checked(
            patch
                .child_transform
                .map(|transform| Affine2D::from_translation(insets.left, insets.top) * transform)
                .unwrap_or_else(|| {
                    frozen.map_or_else(
                        || node.child_transform().affine(),
                        super::frozen::FrozenGeometryNode::child_transform,
                    )
                }),
        )?;
        if patch.child_transform.is_some() {
            matrices += 1;
        }
        let effective_visible = ancestor_visible
            && if frozen
                .is_some_and(|frozen| frozen.visibility_revision != node.visibility_revision)
            {
                node.is_visible
            } else {
                patch.visible.unwrap_or(node.is_visible)
            };
        let local_bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        let bounds_content = frozen
            .and_then(|node| node.bounds_drawing.as_ref().map(|drawing| (node, drawing)))
            .map(|(node, drawing)| {
                drawing
                    .materialize(
                        local_bounds,
                        node.drawing_foreground,
                        node.drawing_background,
                    )
                    .map_err(|error| bounds_drawing_error(id, error))
            })
            .transpose()?;
        let border = frozen
            .and_then(|node| node.border_drawing.as_ref().map(|drawing| (node, drawing)))
            .map(|(node, drawing)| {
                drawing
                    .materialize(
                        local_bounds,
                        node.drawing_foreground,
                        node.drawing_background,
                    )
                    .map_err(|error| bounds_drawing_error(id, error))
            })
            .transpose()?
            .or_else(|| frozen.and_then(|node| node.border_content.clone()));
        let content = effect
            .and_then(|effect| effect.content.clone())
            .or(bounds_content)
            .or_else(|| frozen.and_then(|node| node.content.clone()))
            .or_else(|| {
                node.prepared
                    .as_ref()
                    .map(|prepared| prepared.presentation.clone())
            });
        let mut self_bounds = content.as_ref().map_or_else(
            || node.figure.visual_bounds_in(bounds),
            |content| {
                Rectangle::new(0.0, 0.0, bounds.width, bounds.height).union(content.visual_bounds())
            },
        );
        if let Some(border) = &border {
            self_bounds = self_bounds.union(border.visual_bounds());
        }
        let mut envelope = if effective_visible {
            matrices += 4;
            clips += usize::from(incoming_clip.is_some());
            clip(project(world, self_bounds)?, incoming_clip)
        } else {
            None
        };
        // Recording starts at contents, so its synthetic parent never contributes a client
        // clip. Applying the zero-sized bootstrap root here would erase every repair envelope.
        let strategy = if id == tree.synthetic_root() && tree.contents().is_some() {
            ChildClippingStrategy::OverflowVisible
        } else {
            frozen.map_or_else(|| node.child_clipping_strategy(), |node| node.clipping)
        };
        let client_clip = if strategy == ChildClippingStrategy::OverflowVisible {
            incoming_clip
        } else {
            matrices += 4;
            clips += usize::from(incoming_clip.is_some());
            Some(intersect(incoming_clip, project(world, client_box)?))
        };
        let child_world = checked(world * child_transform)?;
        for child in &node.children {
            edges += 1;
            if !selected.contains(child) {
                continue;
            }
            let child_node = tree.node(*child)?;
            let child_source = self.data.sources.get(child);
            let child_frozen = child_source.and_then(|source| source.nodes.get(child));
            let child_effect = effects.get(child);
            let child_patch = child_effect
                .map(|effect| effect.geometry)
                .unwrap_or_default();
            let legacy_overflow = child_effect
                .and_then(|effect| effect.transform)
                .is_some_and(|transform| transform != Affine2D::IDENTITY);
            let child_clip = if strategy == ChildClippingStrategy::ClipToChildBounds
                && child_frozen.map_or_else(
                    || child_node.child_clipping_strategy(),
                    |node| node.clipping,
                ) != ChildClippingStrategy::OverflowVisible
                && !legacy_overflow
            {
                matrices += 4;
                clips += usize::from(client_clip.is_some());
                Some(intersect(
                    client_clip,
                    project(
                        child_world,
                        child_patch.bounds.unwrap_or_else(|| {
                            child_frozen.map_or_else(
                                || child_node.figure_bounds(),
                                super::frozen::FrozenGeometryNode::bounds,
                            )
                        }),
                    )?,
                ))
            } else {
                client_clip
            };
            let child_envelope = self.visit(
                tree,
                effects,
                selected,
                *child,
                child_world,
                effective_visible,
                child_clip,
            )?;
            envelopes += 1;
            envelope = match (envelope, child_envelope) {
                (Some(a), Some(b)) => Some(a.union(b)),
                (a, b) => a.or(b),
            };
        }
        Arc::get_mut(&mut self.data)
            .expect("unpublished geometry is exclusively owned")
            .nodes
            .insert(
                id,
                PresentedFigureGeometry {
                    bounds,
                    client_box,
                    local_transform,
                    child_transform,
                    local_to_surface: world,
                    effective_visible,
                    effective_clip: incoming_clip,
                    visual_envelope: envelope,
                    parent: node.parent,
                    children: node.children.clone(),
                    content,
                    border,
                },
            );
        let work = &mut Arc::get_mut(&mut self.data)
            .expect("unpublished geometry is exclusively owned")
            .work;
        work.nodes += 1;
        work.edges += edges;
        work.matrices += matrices;
        work.clips += clips;
        work.envelopes += envelopes;
        Ok(envelope)
    }
}

fn bounds_drawing_error(id: FigureId, error: crate::graphics::GraphicsError) -> AnimationError {
    match error {
        crate::graphics::GraphicsError::RecordingBudgetExceeded { limit, actual } => {
            AnimationError::ProviderOutputExceeded {
                kind: super::AnimationBudgetKind::SnapshotBytes,
                declared: limit,
                actual,
                figure: id,
                value_type: TypeId::of::<crate::figure::BoundsDrawing>(),
            }
        }
        _ => AnimationError::InvalidValue,
    }
}

fn checked(transform: Affine2D) -> Result<Affine2D, AnimationError> {
    if transform.is_valid() {
        Ok(transform)
    } else {
        Err(AnimationError::InvalidValue)
    }
}

fn project(transform: Affine2D, bounds: Rectangle) -> Result<Rectangle, AnimationError> {
    let points = [
        crate::Point::new(bounds.x, bounds.y),
        crate::Point::new(bounds.x + bounds.width, bounds.y),
        crate::Point::new(bounds.x, bounds.y + bounds.height),
        crate::Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
    ]
    .map(|point| transform.transform_point(point));
    let left = points
        .iter()
        .map(|point| point.x())
        .fold(f64::INFINITY, f64::min);
    let top = points
        .iter()
        .map(|point| point.y())
        .fold(f64::INFINITY, f64::min);
    let right = points
        .iter()
        .map(|point| point.x())
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = points
        .iter()
        .map(|point| point.y())
        .fold(f64::NEG_INFINITY, f64::max);
    let bounds = Rectangle::new(left, top, right - left, bottom - top);
    if bounds.is_valid() {
        Ok(bounds)
    } else {
        Err(AnimationError::InvalidValue)
    }
}

fn intersect(clip: Option<Rectangle>, bounds: Rectangle) -> Rectangle {
    clip.map_or(bounds, |clip| {
        clip.intersection(bounds)
            .unwrap_or(Rectangle::new(bounds.x, bounds.y, 0.0, 0.0))
    })
}

fn clip(bounds: Rectangle, clip: Option<Rectangle>) -> Option<Rectangle> {
    let result = intersect(clip, bounds);
    (result.width > 0.0 && result.height > 0.0).then_some(result)
}
