use super::*;

/// Numerical reason a coordinate query could not produce a transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateFailure {
    /// At least one affine input coefficient is non-finite.
    NonFiniteInput,
    /// The mathematical inverse exists but a result coefficient exceeds f64 range.
    InverseUnrepresentable,
    /// The calculation cannot reliably determine or represent its intermediate result.
    NumericalFailure,
    /// Finite forward inputs produced a non-finite composed transform.
    DerivedOverflow,
}

#[cfg(test)]
mod s07_tree_identity_tests {
    use super::*;
    use crate::identity::RuntimeHandle;

    #[test]
    fn never_live_identity_is_rejected_by_every_fact_query() {
        let tree = FigureTree::new();
        let id = FigureId::from_local(tree.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        let error = TreeQueryError::UnknownFigure(id);
        macro_rules! check {
            ($($query:ident),+ $(,)?) => {$(
                assert_eq!(tree.$query(id).err(), Some(error), stringify!($query));
            )+};
        }
        check!(
            node,
            figure_bounds,
            component_revision,
            child_order,
            parent_id,
            depth,
            is_attached,
            is_visible,
            is_enabled,
            is_opaque,
            insets,
            figure_style,
            resolved_style,
            border_is_effectively_opaque,
            is_effectively_visible,
            is_effectively_enabled,
            is_focusable,
            is_focus_traversable,
            can_request_focus,
            can_traverse_focus,
            is_connection_figure,
            connection_route_points,
            connection_stroke_color,
            point_list_points,
            point_list_stroke_style,
            child_clipping_strategy,
            is_valid,
            ancestor_ids,
            descendant_ids,
            layout_manager,
        );
        assert!(!tree.contains(id));
        assert_eq!(tree.viewport_handle(id).err(), Some(error));
        assert_eq!(
            tree.scale_handle(id).err(),
            Some(CapabilityQueryError::UnknownOrDisposedFigure(id))
        );
        assert_eq!(
            tree.image_display_state(id).err(),
            Some(crate::figure::ShapeMutationError::UnknownFigure(id))
        );
        assert_eq!(tree.child_z_index(tree.root, id), Err(error));
        assert_eq!(tree.child_z_index(id, tree.root), Err(error));
        assert_eq!(tree.is_ancestor_of(tree.root, id), Err(error));
        assert_eq!(tree.is_ancestor_of(id, id), Err(error));
        assert_eq!(
            tree.layout_constraint::<Rectangle>(id),
            Err(LayoutError::Identity(error))
        );
        assert_eq!(
            tree.freeform_extent(id),
            Err(FreeformError::UnknownFigure(id))
        );
    }
}

/// Identity or geometry failure of a checked Figure coordinate query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateQueryError {
    /// The requested identity does not belong to the live tree.
    Identity(TreeQueryError),
    /// Transform calculation failed for an otherwise valid identity.
    Geometry {
        /// Identity whose requested transform could not be calculated.
        figure: FigureId,
        /// Numerical classification of the failure.
        reason: CoordinateFailure,
    },
}

impl std::fmt::Display for CoordinateQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Geometry { figure, reason } => {
                write!(
                    formatter,
                    "coordinate query for {figure:?} failed: {reason:?}"
                )
            }
        }
    }
}

impl std::error::Error for CoordinateQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Geometry { .. } => None,
        }
    }
}

impl From<TreeQueryError> for CoordinateQueryError {
    fn from(error: TreeQueryError) -> Self {
        Self::Identity(error)
    }
}

fn checked_transform(
    figure: FigureId,
    transform: Affine2D,
    reason: CoordinateFailure,
) -> Result<Affine2D, CoordinateQueryError> {
    if transform.coeffs().into_iter().all(f64::is_finite) {
        Ok(transform)
    } else {
        Err(CoordinateQueryError::Geometry { figure, reason })
    }
}

pub(crate) fn checked_coordinate_inverse(
    figure: FigureId,
    transform: Affine2D,
) -> Result<Option<Affine2D>, CoordinateQueryError> {
    use crate::geometry::InverseFailure;
    transform
        .checked_inverse()
        .map_err(|error| CoordinateQueryError::Geometry {
            figure,
            reason: match error {
                InverseFailure::NonFiniteInput => CoordinateFailure::NonFiniteInput,
                InverseFailure::InverseUnrepresentable => CoordinateFailure::InverseUnrepresentable,
                InverseFailure::NumericalFailure => CoordinateFailure::NumericalFailure,
            },
        })
}

impl FigureTree {
    /// Returns membership in this tree's live identity set without reading facts.
    pub fn contains(&self, id: FigureId) -> bool {
        id.namespace() == self.namespace() && self.blocks.contains_key(id)
    }

    /// Returns the node for a Figure identity.
    pub fn node(&self, id: FigureId) -> Result<&FigureNode, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(&self.blocks[id])
    }

    /// Queries one attach-time capability descriptor without causing mutation.
    pub fn capability<C: 'static>(
        &self,
        figure: FigureId,
        key: CapabilityKey<C>,
    ) -> Result<Option<&C>, CapabilityQueryError> {
        if figure.namespace() != self.namespace() {
            return Err(CapabilityQueryError::ForeignRuntime(figure));
        }
        let node = self
            .blocks
            .get(figure)
            .ok_or(CapabilityQueryError::UnknownOrDisposedFigure(figure))?;
        node.capabilities.get(key)
    }

    pub(crate) fn is_layered_pane(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
    }

    pub(crate) fn is_layer_figure(&self, id: FigureId) -> bool {
        self.capability(id, LAYER)
            .is_ok_and(|layer| layer.is_some())
    }

    /// 返回指定父节点的 child 顺序。
    ///
    /// 顺序与 Draw2D 一致：数组靠前的 child 先绘制，靠后的 child 后绘制并位于更高 z-order。
    pub fn child_order(&self, parent_id: FigureId) -> Result<Vec<FigureId>, TreeQueryError> {
        Ok(self.node(parent_id)?.children.clone())
    }

    /// Returns the direct parent of a Figure.
    pub fn parent_id(&self, figure_id: FigureId) -> Result<Option<FigureId>, TreeQueryError> {
        Ok(self.node(figure_id)?.parent)
    }

    /// Returns whether a node is currently attached to this tree's root.
    pub fn is_attached(&self, figure_id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(figure_id)?;
        let mut current = Some(figure_id);
        for _ in 0..=self.blocks.len() {
            let Some(id) = current else {
                return Ok(false);
            };
            if id == self.root {
                return Ok(true);
            }
            current = self.blocks.get(id).and_then(|block| block.parent);
        }
        Ok(false)
    }

    /// 返回 child 在父节点内的 z-order index。
    ///
    /// index 越大表示越靠前绘制、越靠上层。
    pub fn child_z_index(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<Option<usize>, TreeQueryError> {
        self.validate_identity(parent_id)?;
        self.validate_identity(child_id)?;
        Ok(self.blocks[parent_id]
            .children
            .iter()
            .position(|&id| id == child_id))
    }

    /// 获取指定块的 Figure bounds。
    pub fn figure_bounds(&self, id: FigureId) -> Result<Rectangle, TreeQueryError> {
        Ok(self.node(id)?.figure_bounds())
    }

    /// Returns the committed private-component revision for a Figure.
    pub fn component_revision(&self, id: FigureId) -> Result<u64, TreeQueryError> {
        Ok(self.node(id)?.component_revision)
    }

    pub fn is_connection_figure(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.connection().is_some())
    }

    pub fn connection_route_points(
        &self,
        id: FigureId,
    ) -> Result<Option<&PointList>, TreeQueryError> {
        Ok(self
            .node(id)?
            .connection()
            .map(|connection| connection.route_points()))
    }

    pub fn connection_stroke_color(
        &self,
        id: FigureId,
    ) -> Result<Option<crate::Color>, TreeQueryError> {
        Ok(self
            .node(id)?
            .connection()
            .map(|connection| connection.connection_stroke_color()))
    }

    pub fn point_list_points(&self, id: FigureId) -> Result<Option<Vec<Point>>, TreeQueryError> {
        let block = self.node(id)?;
        let bounds = block.figure_bounds();
        Ok(block.point_list().map(|points| {
            points
                .local_points()
                .iter()
                .map(|point| Point::new(point.x() + bounds.x, point.y() + bounds.y))
                .collect()
        }))
    }

    pub fn image_display_state(
        &self,
        id: FigureId,
    ) -> Result<crate::figure::ImageDisplayState, crate::figure::ShapeMutationError> {
        self.validate_identity(id).map_err(|error| match error {
            TreeQueryError::ForeignFigure(id) => {
                crate::figure::ShapeMutationError::ForeignFigure(id)
            }
            TreeQueryError::UnknownFigure(id) => {
                crate::figure::ShapeMutationError::UnknownFigure(id)
            }
        })?;
        self.image_figure(id)
            .map(crate::figure::ImageFigure::display_state)
            .ok_or_else(|| {
                if self.contains(id) {
                    crate::figure::ShapeMutationError::WrongCapability(id)
                } else {
                    crate::figure::ShapeMutationError::UnknownFigure(id)
                }
            })
    }

    pub fn point_list_stroke_style(
        &self,
        id: FigureId,
    ) -> Result<Option<crate::render::StrokeStyle>, TreeQueryError> {
        Ok(self
            .node(id)?
            .point_list()
            .map(|points| points.stroke_style().clone()))
    }

    /// 返回节点从 FigureTree 根节点开始计算的深度。
    pub fn depth(&self, id: FigureId) -> Result<usize, TreeQueryError> {
        Ok(self.node(id)?.depth)
    }

    /// 返回节点自身的本地可见性标志。
    pub fn is_visible(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_visible)
    }

    /// 返回节点自身的本地启用标志。
    pub fn is_enabled(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_enabled)
    }

    pub fn is_opaque(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_opaque)
    }

    pub fn insets(&self, id: FigureId) -> Result<Insets, TreeQueryError> {
        Ok(self.node(id)?.insets)
    }

    pub fn border_is_effectively_opaque(
        &self,
        id: FigureId,
    ) -> Result<Option<bool>, TreeQueryError> {
        let block = self.node(id)?;
        let style = self.resolved_style(id)?;
        Ok(block
            .figure
            .get_border()
            .map(|border| border.is_opaque() && style.alpha >= 1.0))
    }

    pub fn figure_style(&self, id: FigureId) -> Result<&FigureStyle, TreeQueryError> {
        Ok(&self.node(id)?.style)
    }

    pub fn resolved_style(&self, id: FigureId) -> Result<ResolvedStyle, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self
            .resolved_style_with_node_visits(id)
            .expect("live immutable parent chain")
            .0)
    }

    fn resolved_style_with_node_visits(&self, id: FigureId) -> Option<(ResolvedStyle, u64)> {
        self.blocks.get(id)?;
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.blocks.get(node_id)?;
            chain.push(node_id);
            current = node.parent;
        }
        let node_visits = u64::try_from(chain.len()).unwrap_or(u64::MAX);
        let mut result = ResolvedStyle::default();
        for node_id in chain.into_iter().rev() {
            result.apply_override(&self.blocks[node_id].style);
        }
        Some((result, node_visits))
    }

    pub(super) fn resolved_styles_for(
        &self,
        mut include: impl FnMut(&FigureNode) -> bool,
    ) -> (Vec<(FigureId, ResolvedStyle)>, u64) {
        let mut matches = Vec::new();
        let mut nodes_visited = 0_u64;
        let mut stack = vec![(self.root, ResolvedStyle::default())];
        while let Some((id, mut style)) = stack.pop() {
            let node = &self.blocks[id];
            nodes_visited = nodes_visited.saturating_add(1);
            style.apply_override(&node.style);
            if include(node) {
                matches.push((id, style.clone()));
            }
            for child in node.children.iter().rev() {
                stack.push((*child, style.clone()));
            }
        }
        (matches, nodes_visited)
    }

    pub(crate) fn tooltip_source(&self, hit: FigureId) -> Option<(FigureId, String)> {
        let mut current = Some(hit);
        while let Some(id) = current {
            let node = self.blocks.get(id)?;
            match &node.style.tooltip {
                Some(Some(text)) => return Some((id, text.clone())),
                Some(None) => return None,
                None => current = node.parent,
            }
        }
        None
    }

    pub(crate) fn label(&self, id: FigureId) -> Option<&LabelFigure> {
        self.blocks.get(id)?.label()
    }

    pub(crate) fn text_flow(&self, id: FigureId) -> Option<&crate::TextFlowFigure> {
        self.blocks.get(id)?.text_flow()
    }

    pub(crate) fn image_figure(&self, id: FigureId) -> Option<&ImageFigure> {
        self.blocks.get(id)?.figure.as_ref().as_any().downcast_ref()
    }

    pub(crate) fn clickable_snapshot(&self, id: FigureId) -> Option<ClickableSnapshot> {
        self.blocks
            .get(id)?
            .clickable()
            .map(|clickable| clickable.clickable_model().snapshot())
    }

    pub(crate) fn activity_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| block.activity().is_some().then_some(id))
            .collect()
    }

    pub(crate) fn clickable_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| block.clickable().is_some().then_some(id))
            .collect()
    }

    pub(crate) fn border_snapshot(&self, id: FigureId) -> Option<&BorderSnapshot> {
        self.blocks.get(id)?.border_snapshot.as_ref()
    }

    /// 返回节点沿父链传播后的有效可见性。
    pub fn is_effectively_visible(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self.effective_flag_from(id, |block| block.is_visible))
    }

    /// 返回节点沿父链传播后的有效启用状态。
    pub fn is_effectively_enabled(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        self.validate_identity(id)?;
        Ok(self.effective_flag_from(id, |block| block.is_enabled))
    }

    pub fn is_focusable(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_focusable)
    }

    pub fn is_focus_traversable(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(id)?.is_focus_traversable)
    }

    pub fn can_request_focus(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.is_attached(id)?
            && self.is_effectively_visible(id)?
            && self.is_effectively_enabled(id)?
            && self.is_focusable(id)?)
    }

    pub fn can_traverse_focus(&self, id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.is_attached(id)?
            && self.is_effectively_visible(id)?
            && self.is_effectively_enabled(id)?
            && self.is_focus_traversable(id)?)
    }

    pub(crate) fn can_retain_focus(&self, id: FigureId) -> bool {
        self.is_attached(id).unwrap_or(false)
            && self.is_effectively_visible(id).unwrap_or(false)
            && self.is_effectively_enabled(id).unwrap_or(false)
            && (self.is_focusable(id).unwrap_or(false)
                || self.is_focus_traversable(id).unwrap_or(false))
    }

    /// 返回 node-local 到 parent content domain 的变换。
    pub fn local_to_parent_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let bounds = self.blocks[figure_id].figure_bounds();
        checked_transform(
            figure_id,
            Affine2D::from_translation(bounds.x, bounds.y),
            CoordinateFailure::NonFiniteInput,
        )
    }

    /// 返回 parent content domain 到 node-local 的变换。
    pub fn parent_to_local_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Option<Affine2D>, CoordinateQueryError> {
        checked_coordinate_inverse(figure_id, self.local_to_parent_transform(figure_id)?)
    }

    /// 返回 node-local 到 logical surface 的完整父链变换。
    pub fn local_to_surface_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        self.validate_identity(figure_id)?;
        let mut transform = Affine2D::IDENTITY;
        let mut current_id = figure_id;

        loop {
            let current = &self.blocks[current_id];
            transform = checked_transform(
                figure_id,
                self.local_to_parent_transform(current_id)? * transform,
                CoordinateFailure::DerivedOverflow,
            )?;

            let Some(parent_id) = current.parent else {
                break;
            };
            let parent = &self.blocks[parent_id];
            let child_transform = checked_transform(
                figure_id,
                parent.child_transform().affine(),
                CoordinateFailure::NonFiniteInput,
            )?;
            transform = checked_transform(
                figure_id,
                child_transform * transform,
                CoordinateFailure::DerivedOverflow,
            )?;
            current_id = parent_id;
        }

        Ok(transform)
    }

    /// 返回 logical surface domain 到 node-local 的完整父链变换。
    pub fn surface_to_local_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Option<Affine2D>, CoordinateQueryError> {
        checked_coordinate_inverse(figure_id, self.local_to_surface_transform(figure_id)?)
    }

    /// Returns the complete child-content to logical-surface transform.
    pub fn child_content_to_surface_transform(
        &self,
        figure_id: FigureId,
    ) -> Result<Affine2D, CoordinateQueryError> {
        let local_to_surface = self.local_to_surface_transform(figure_id)?;
        let child_transform = checked_transform(
            figure_id,
            self.blocks[figure_id].child_transform().affine(),
            CoordinateFailure::NonFiniteInput,
        )?;
        checked_transform(
            figure_id,
            local_to_surface * child_transform,
            CoordinateFailure::DerivedOverflow,
        )
    }

    pub(crate) fn presentation_envelope(
        &self,
        figure_id: FigureId,
        local_transform: Affine2D,
    ) -> Option<Rectangle> {
        if !self.is_effectively_visible(figure_id).ok()? {
            return None;
        }
        let root_to_surface = self.local_to_surface_transform(figure_id).ok()?;
        let surface_to_root = root_to_surface.inverse()?;
        let mut ids = vec![figure_id];
        ids.extend(self.descendant_ids(figure_id).ok()?);
        let mut local_envelope = None;
        for id in ids {
            if !self.is_effectively_visible(id).ok()? {
                continue;
            }
            let node = self.blocks.get(id)?;
            let descendant_to_root = surface_to_root * self.local_to_surface_transform(id).ok()?;
            let visual = super::transform_rectangle(descendant_to_root, node.visual_bounds())?;
            local_envelope =
                Some(local_envelope.map_or(visual, |envelope: Rectangle| envelope.union(visual)));
        }
        super::transform_rectangle(root_to_surface * local_transform, local_envelope?)
    }

    fn effective_flag_from(
        &self,
        mut figure_id: FigureId,
        local_flag: fn(&FigureNode) -> bool,
    ) -> bool {
        for _ in 0..self.blocks.len() {
            let Some(block) = self.blocks.get(figure_id) else {
                return false;
            };
            if !local_flag(block) {
                return false;
            }
            let Some(parent_id) = block.parent else {
                return true;
            };
            figure_id = parent_id;
        }
        false
    }
}
