use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{Affine2D, ChildClippingStrategy, FigureId, FigureTree, Rectangle};

use super::{AnimationError, GeometryInputs};

/// Owned source dependencies. Motion and materialization never borrow mutable Figure state.
pub(crate) struct FrozenGeometrySource {
    pub(crate) roots: Vec<FigureId>,
    pub(crate) nodes: HashMap<FigureId, FrozenGeometryNode>,
    retained_versions: Vec<(crate::render::ResourceId, u64)>,
}

pub(crate) struct FrozenGeometryNode {
    pub(crate) inputs: GeometryInputs,
    pub(crate) parent: Option<FigureId>,
    pub(crate) children: Vec<FigureId>,
    pub(crate) clipping: ChildClippingStrategy,
    pub(crate) content: Option<crate::figure::FigurePresentation>,
    content_bytes: usize,
    component: u64,
    pub(crate) visibility_revision: u64,
    viewport_scale: Option<f64>,
    viewport_ranges: Option<[(f64, f64, f64); 2]>,
    pub(crate) style: crate::style::FigureStyle,
    pub(crate) bounds_drawing: Option<crate::figure::BoundsDrawing>,
    pub(crate) border_drawing: Option<crate::figure::BoundsDrawing>,
    pub(crate) border_content: Option<crate::figure::FigurePresentation>,
    pub(crate) drawing_foreground: crate::Color,
    pub(crate) drawing_background: crate::Color,
    pub(crate) opaque: bool,
    resources: Vec<(crate::render::ResourceId, Option<crate::ResourceStatus>)>,
    preparation: Option<(u64, u64, crate::text::FontDescriptor)>,
    content_revision: Option<u64>,
    bounds_revision: Option<u64>,
    pub(crate) ancestor_only: bool,
}

impl FrozenGeometrySource {
    /// Bounds one materialization before allocating its maps, scratch sets or recordings.
    /// Four slots per selected node cover table load factors and geometric capacity growth.
    pub(crate) fn materialization_bytes(&self) -> Result<usize, AnimationError> {
        let node_storage = std::mem::size_of::<super::PresentedFigureGeometry>()
            + std::mem::size_of::<(FigureId, Arc<Self>)>()
            + 8 * (std::mem::size_of::<FigureId>() + std::mem::size_of::<usize>());
        let headers = 4
            * (std::mem::size_of::<super::PresentationGeometry>()
                + std::mem::size_of::<super::geometry_view::PresentationGeometryData>());
        let mut bytes = self
            .nodes
            .len()
            .checked_mul(node_storage)
            .and_then(|bytes| bytes.checked_mul(4))
            .and_then(|bytes| bytes.checked_add(headers))
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))?;
        for node in self.nodes.values() {
            bytes = node
                .children
                .len()
                .checked_mul(std::mem::size_of::<FigureId>())
                .and_then(|children| bytes.checked_add(children))
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
        }
        bytes
            .checked_add(self.recording_bytes()?)
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))
    }

    pub(crate) fn recording_bytes(&self) -> Result<usize, AnimationError> {
        self.nodes.values().try_fold(0usize, |bytes, node| {
            [node.bounds_drawing.as_ref(), node.border_drawing.as_ref()]
                .into_iter()
                .flatten()
                .try_fold(bytes, |bytes, drawing| {
                    bytes.checked_add(drawing.recording_bytes()).ok_or(
                        AnimationError::BudgetExceeded(super::AnimationBudgetKind::SnapshotBytes),
                    )
                })
        })
    }

    pub(crate) fn collection_bytes(&self) -> Result<usize, AnimationError> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |count: usize, item: usize| -> Result<(), AnimationError> {
            bytes = count
                .checked_mul(item)
                .and_then(|size| bytes.checked_add(size))
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
            Ok(())
        };
        add(self.roots.capacity(), std::mem::size_of::<FigureId>())?;
        add(
            self.retained_versions.capacity(),
            std::mem::size_of::<(crate::render::ResourceId, u64)>(),
        )?;
        add(
            self.nodes.capacity(),
            2 * (std::mem::size_of::<(FigureId, FrozenGeometryNode)>() + 1),
        )?;
        for node in self.nodes.values() {
            add(1, node.content_bytes)?;
            for drawing in [node.bounds_drawing.as_ref(), node.border_drawing.as_ref()]
                .into_iter()
                .flatten()
            {
                add(1, drawing.retained_bytes())?;
            }
            add(node.children.capacity(), std::mem::size_of::<FigureId>())?;
            add(
                node.resources.capacity(),
                std::mem::size_of::<(crate::render::ResourceId, Option<crate::ResourceStatus>)>(),
            )?;
            for text in [
                node.style.font.as_ref().map(|font| &font.family),
                node.style.tooltip.as_ref().and_then(Option::as_ref),
                node.preparation.as_ref().map(|(_, _, font)| &font.family),
            ]
            .into_iter()
            .flatten()
            {
                add(text.capacity(), 1)?;
            }
        }
        Ok(bytes)
    }

    pub(crate) fn capture(
        tree: &FigureTree,
        registry: &crate::runtime::ResourceRegistry,
        roots: Vec<FigureId>,
        limits: super::AnimationWork,
    ) -> Result<Arc<Self>, AnimationError> {
        let mut selected = HashSet::new();
        let mut pending = Vec::new();
        for root in &roots {
            if selected.insert(*root) {
                if selected.len() > limits.nodes {
                    return Err(AnimationError::BudgetExceeded(
                        super::AnimationBudgetKind::GeometryNodes,
                    ));
                }
                pending.push(*root);
            }
        }
        while let Some(id) = pending.pop() {
            for child in &tree.node(id)?.children {
                if selected.insert(*child) {
                    if selected.len() > limits.nodes {
                        return Err(AnimationError::BudgetExceeded(
                            super::AnimationBudgetKind::GeometryNodes,
                        ));
                    }
                    pending.push(*child);
                }
            }
        }
        let affected = selected.clone();
        for root in &roots {
            let mut parent = tree.node(*root)?.parent;
            while let Some(id) = parent {
                if !selected.insert(id) {
                    break;
                }
                if selected.len() > limits.nodes {
                    return Err(AnimationError::BudgetExceeded(
                        super::AnimationBudgetKind::GeometryNodes,
                    ));
                }
                parent = tree.node(id)?.parent;
            }
        }
        // Ancestors may have many unrelated children. Their frozen child-order storage is
        // part of the dependency budget and is checked before cloning any of those lists.
        let mut edges = 0usize;
        let mut bytes = selected
            .len()
            .checked_mul(4 * (std::mem::size_of::<(FigureId, FrozenGeometryNode)>() + 1))
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))?;
        for id in &selected {
            let node = tree.node(*id)?;
            edges =
                edges
                    .checked_add(node.children.len())
                    .ok_or(AnimationError::BudgetExceeded(
                        super::AnimationBudgetKind::DependencyEdges,
                    ))?;
            if edges > limits.edges {
                return Err(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::DependencyEdges,
                ));
            }
            let heap = node
                .children
                .len()
                .checked_mul(std::mem::size_of::<FigureId>())
                .and_then(|size| {
                    node.resource_inputs
                        .len()
                        .checked_mul(std::mem::size_of::<(
                            crate::render::ResourceId,
                            Option<crate::ResourceStatus>,
                        )>())
                        .and_then(|resources| size.checked_add(resources))
                })
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
            bytes = bytes
                .checked_add(heap)
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
            for text in [
                node.style.font.as_ref().map(|font| &font.family),
                node.style.tooltip.as_ref().and_then(Option::as_ref),
                node.prepared
                    .as_ref()
                    .map(|prepared| &prepared.key.font.family),
            ]
            .into_iter()
            .flatten()
            {
                bytes = bytes
                    .checked_add(text.len())
                    .ok_or(AnimationError::BudgetExceeded(
                        super::AnimationBudgetKind::SnapshotBytes,
                    ))?;
            }
            if bytes > limits.bytes {
                return Err(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ));
            }
        }
        let mut nodes = HashMap::with_capacity(selected.len());
        let mut retained_resources = HashSet::new();
        for id in selected {
            let node = tree.node(id)?;
            let style = tree.resolved_style(id)?;
            let mut bounds_drawing = node
                .capabilities
                .get(crate::figure::BOUNDS_SNAPSHOT)
                .map_err(|_| AnimationError::InvalidValue)?
                .copied()
                .and_then(|capability| capability.resolve(node.figure.as_ref()))
                .map(crate::figure::FigureBoundsSnapshot::snapshot_bounds_drawing)
                .transpose()
                .map_err(content_error)?;
            let mut border_drawing = node
                .figure
                .get_border()
                .map(|border| {
                    border.snapshot_bounds_drawing_with_colors(
                        node.border_snapshot.as_ref(),
                        style.foreground,
                        style.background,
                    )
                })
                .transpose()
                .map_err(content_error)?;
            for drawing in [bounds_drawing.as_mut(), border_drawing.as_mut()]
                .into_iter()
                .flatten()
            {
                drawing
                    .freeze_resources(registry, &mut retained_resources)
                    .map_err(content_error)?;
                bytes = bytes.checked_add(drawing.retained_bytes()).ok_or(
                    AnimationError::BudgetExceeded(super::AnimationBudgetKind::SnapshotBytes),
                )?;
            }
            if bytes > limits.bytes {
                return Err(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ));
            }
            let (content, mut content_bytes) = if bounds_drawing.is_some() {
                (None, 0)
            } else {
                freeze_content(
                    node,
                    registry,
                    &style,
                    &mut retained_resources,
                    limits.bytes - bytes,
                    false,
                )?
            };
            bytes = bytes
                .checked_add(content_bytes)
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ))?;
            let border_content = if border_drawing.is_none() {
                let (content, border_bytes) = freeze_content(
                    node,
                    registry,
                    &style,
                    &mut retained_resources,
                    limits.bytes.saturating_sub(bytes),
                    true,
                )?;
                content_bytes = content_bytes.checked_add(border_bytes).ok_or(
                    AnimationError::BudgetExceeded(super::AnimationBudgetKind::SnapshotBytes),
                )?;
                bytes = bytes
                    .checked_add(border_bytes)
                    .ok_or(AnimationError::BudgetExceeded(
                        super::AnimationBudgetKind::SnapshotBytes,
                    ))?;
                content
            } else {
                None
            };
            if bytes > limits.bytes {
                return Err(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::SnapshotBytes,
                ));
            }
            nodes.insert(
                id,
                FrozenGeometryNode {
                    inputs: super::geometry::inputs(node),
                    parent: node.parent,
                    children: node.children.clone(),
                    clipping: node.child_clipping_strategy(),
                    content,
                    content_bytes,
                    component: node.component_revision,
                    visibility_revision: node.visibility_revision,
                    viewport_scale: tree
                        .viewport_handle(id)?
                        .map(|handle| handle.content_scale()),
                    viewport_ranges: tree.viewport_handle(id)?.map(|handle| {
                        [handle.horizontal_range(), handle.vertical_range()]
                            .map(|range| (range.minimum, range.maximum, range.extent))
                    }),
                    style: node.state().style.clone(),
                    bounds_drawing,
                    border_drawing,
                    border_content,
                    drawing_foreground: style.foreground,
                    drawing_background: style.background,
                    opaque: node.state().is_opaque(),
                    resources: node.resource_inputs.clone(),
                    preparation: preparation(node),
                    content_revision: content_revision(node),
                    bounds_revision: bounds_revision(node),
                    ancestor_only: !affected.contains(&id),
                },
            );
        }
        Ok(Arc::new(Self {
            roots,
            nodes,
            retained_versions: retained_resources.into_iter().collect(),
        }))
    }

    /// Bounds alone are legitimate new source output. Content/topology changes invalidate
    /// this group's old inputs; changes outside the declared closure do not participate.
    pub(crate) fn is_valid(
        &self,
        tree: &FigureTree,
        resources: &crate::runtime::ResourceRegistry,
    ) -> bool {
        self.retained_versions.iter().all(|(id, revision)| {
            resources
                .snapshot_resource(*id)
                .is_ok_and(|resource| resource.revision == *revision)
        }) && self.nodes.iter().all(|(id, frozen)| {
            tree.node(*id).is_ok_and(|node| {
                frozen.parent == node.parent
                    && (!frozen.ancestor_only || frozen.inputs.bounds == node.figure_bounds())
                    && frozen.children == node.children
                    && frozen.component == node.component_revision
                    && frozen.inputs.insets == node.state().insets
                    && if let Some(scale) = frozen.viewport_scale {
                        tree.viewport_handle(*id).is_ok_and(|handle| {
                            handle.is_some_and(|handle| {
                                handle.content_scale() == scale
                                    && Some(
                                        [handle.horizontal_range(), handle.vertical_range()].map(
                                            |range| (range.minimum, range.maximum, range.extent),
                                        ),
                                    ) == frozen.viewport_ranges
                            })
                        })
                    } else {
                        frozen.inputs.child_transform
                            == super::geometry::inputs(node).child_transform
                    }
                    && frozen.clipping == node.child_clipping_strategy()
                    && frozen.style == node.state().style
                    && frozen.opaque == node.state().is_opaque()
                    && frozen.resources == node.resource_inputs
                    && frozen.preparation == preparation(node)
                    && frozen.content_revision == content_revision(node)
                    && frozen.bounds_revision == bounds_revision(node)
            })
        })
    }
}

/// Converts the extension's immutable payload into storage with a known ownership model.
/// The opaque callback and its domain data are not retained by the animated drawing.
fn freeze_content(
    node: &crate::graph::FigureNode,
    registry: &crate::runtime::ResourceRegistry,
    style: &crate::style::ResolvedStyle,
    retained_resources: &mut HashSet<(crate::render::ResourceId, u64)>,
    byte_limit: usize,
    border: bool,
) -> Result<(Option<crate::figure::FigurePresentation>, usize), AnimationError> {
    let content = if border { None } else { content(node)? };
    let bounds = node.figure_bounds();
    let local_bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
    let mut recorder = crate::render::CommandRecorder::new();
    recorder.canvas.limit_recording_storage(byte_limit);
    recorder.canvas.set_foreground_color(style.foreground);
    recorder.canvas.set_background_color(style.background);
    {
        let mut context = crate::graphics::PaintContext::for_recorder(
            &mut recorder.canvas,
            Some(registry),
            Some(&mut recorder.leases),
        );
        if border {
            node.figure
                .paint_border_snapshot_in_bounds(
                    &mut context,
                    local_bounds,
                    node.border_snapshot.as_ref(),
                )
                .map_err(content_error)?;
        } else if let Some(content) = &content {
            content.paint(&mut context).map_err(content_error)?;
        } else {
            node.figure
                .paint(&mut context, local_bounds)
                .map_err(content_error)?;
        }
    }
    recorder
        .canvas
        .validate_recording(registry)
        .map_err(content_error)?;
    let mut seen = HashSet::new();
    for command in recorder.commands() {
        let resource = match &command.kind {
            crate::render::RenderCommandKind::Image { image, .. } => {
                Some((image.resource_id(), image.revision()))
            }
            crate::render::RenderCommandKind::DrawGlyphRun { run, .. } => {
                Some((run.font.resource_id(), run.font.revision()))
            }
            _ => None,
        };
        if let Some(resource) = resource {
            seen.insert(resource);
        }
    }
    for (id, revision) in seen {
        let resource = registry
            .snapshot_resource(id)
            .map_err(|_| AnimationError::InvalidValue)?;
        if resource.revision != revision {
            return Err(AnimationError::InvalidValue);
        }
        if !recorder.leases.iter().any(|old| old.id == id) {
            recorder.leases.push(resource);
        }
    }
    let drawing = recorder.finish().map_err(content_error)?;
    let bytes =
        drawing
            .retained_bytes(retained_resources)
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))?;
    let frozen = crate::figure::FigurePresentation::new(
        content.as_ref().map_or_else(
            || crate::figure::FigureMeasurement::new(bounds.width, bounds.height, None),
            crate::figure::FigurePresentation::measurement,
        ),
        content.as_ref().map_or(
            crate::Dimension::new(0.0, 0.0),
            crate::figure::FigurePresentation::minimum_size,
        ),
        content.as_ref().map_or_else(
            || node.figure.visual_bounds_in(bounds),
            crate::figure::FigurePresentation::visual_bounds,
        ),
        Arc::new(FrozenContentDrawing(drawing)),
    )
    .map_err(|_| AnimationError::InvalidValue)?;
    Ok((Some(frozen), bytes))
}

fn content_error(error: crate::graphics::GraphicsError) -> AnimationError {
    match error {
        crate::graphics::GraphicsError::RecordingBudgetExceeded { .. } => {
            AnimationError::BudgetExceeded(super::AnimationBudgetKind::SnapshotBytes)
        }
        _ => AnimationError::InvalidValue,
    }
}

struct FrozenContentDrawing(crate::render::RecordedDrawing);

impl crate::figure::FigureDrawing for FrozenContentDrawing {
    fn paint(
        &self,
        context: &mut crate::graphics::PaintContext<'_>,
    ) -> Result<(), crate::graphics::GraphicsError> {
        context.replay(&self.0)
    }
}

impl FrozenGeometryNode {
    pub(crate) fn child_transform(&self) -> Affine2D {
        Affine2D::from_translation(self.inputs.insets.left, self.inputs.insets.top)
            * self.inputs.child_transform
    }

    pub(crate) fn bounds(&self) -> Rectangle {
        self.inputs.bounds
    }
}

fn preparation(node: &crate::graph::FigureNode) -> Option<(u64, u64, crate::text::FontDescriptor)> {
    node.prepared.as_ref().map(|prepared| {
        (
            prepared.key.provider,
            prepared.key.text,
            prepared.key.font.clone(),
        )
    })
}

fn content_revision(node: &crate::graph::FigureNode) -> Option<u64> {
    node.capabilities
        .get(crate::figure::CONTENT_SNAPSHOT)
        .ok()
        .flatten()
        .copied()
        .and_then(|capability| capability.resolve(node.figure.as_ref()))
        .map(crate::figure::FigureContentSnapshot::content_revision)
}

fn bounds_revision(node: &crate::graph::FigureNode) -> Option<u64> {
    node.capabilities
        .get(crate::figure::BOUNDS_SNAPSHOT)
        .ok()
        .flatten()
        .copied()
        .and_then(|capability| capability.resolve(node.figure.as_ref()))
        .map(crate::figure::FigureBoundsSnapshot::bounds_drawing_revision)
}

fn content(
    node: &crate::graph::FigureNode,
) -> Result<Option<crate::figure::FigurePresentation>, AnimationError> {
    if let Some(prepared) = &node.prepared {
        return Ok(Some(prepared.presentation.clone()));
    }
    node.capabilities
        .get(crate::figure::CONTENT_SNAPSHOT)
        .map_err(|_| AnimationError::InvalidValue)?
        .copied()
        .and_then(|capability| capability.resolve(node.figure.as_ref()))
        .map(|content| {
            let bounds = node.figure_bounds();
            content
                .snapshot_content(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
                .map_err(|_| AnimationError::InvalidValue)
        })
        .transpose()
}
