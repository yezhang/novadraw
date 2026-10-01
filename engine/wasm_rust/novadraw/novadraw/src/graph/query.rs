use super::*;

impl FigureTree {
    /// Returns the node for a Figure identity.
    pub fn node(&self, id: FigureId) -> Option<&FigureNode> {
        self.blocks.get(id)
    }

    pub(crate) fn is_layered_pane(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
    }

    pub(crate) fn is_layer_figure(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.figure.layer().is_some())
    }

    /// 返回指定父节点的 child 顺序。
    ///
    /// 顺序与 Draw2D 一致：数组靠前的 child 先绘制，靠后的 child 后绘制并位于更高 z-order。
    pub fn child_order(&self, parent_id: FigureId) -> Option<Vec<FigureId>> {
        self.blocks
            .get(parent_id)
            .map(|block| block.children.clone())
    }

    /// Returns the direct parent of a Figure.
    pub fn parent_id(&self, figure_id: FigureId) -> Option<FigureId> {
        self.blocks.get(figure_id).and_then(|block| block.parent)
    }

    /// Returns whether a node is currently attached to this tree's root.
    pub fn is_attached(&self, figure_id: FigureId) -> bool {
        let mut current = Some(figure_id);
        for _ in 0..=self.blocks.len() {
            let Some(id) = current else {
                return false;
            };
            if id == self.root {
                return true;
            }
            current = self.blocks.get(id).and_then(|block| block.parent);
        }
        false
    }

    /// 返回 child 在父节点内的 z-order index。
    ///
    /// index 越大表示越靠前绘制、越靠上层。
    pub fn child_z_index(&self, parent_id: FigureId, child_id: FigureId) -> Option<usize> {
        self.blocks
            .get(parent_id)?
            .children
            .iter()
            .position(|&id| id == child_id)
    }

    /// 获取指定块的 Figure bounds。
    pub fn figure_bounds(&self, id: FigureId) -> Option<Rectangle> {
        self.blocks.get(id).map(FigureNode::figure_bounds)
    }

    /// Returns the committed private-component revision for a Figure.
    pub fn component_revision(&self, id: FigureId) -> Option<u64> {
        self.blocks.get(id).map(|node| node.component_revision)
    }

    pub fn is_connection_figure(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.figure.connection().is_some())
    }

    pub fn connection_route_points(&self, id: FigureId) -> Option<&PointList> {
        self.blocks
            .get(id)?
            .figure
            .connection()
            .map(|connection| connection.route_points())
    }

    pub fn connection_stroke_color(&self, id: FigureId) -> Option<crate::Color> {
        self.blocks
            .get(id)?
            .figure
            .connection()
            .map(|connection| connection.connection_stroke_color())
    }

    pub fn point_list_points(&self, id: FigureId) -> Option<Vec<Point>> {
        let block = self.blocks.get(id)?;
        let bounds = block.figure_bounds();
        Some(
            block
                .figure
                .point_list()?
                .local_points()
                .iter()
                .map(|point| Point::new(point.x() + bounds.x, point.y() + bounds.y))
                .collect(),
        )
    }

    pub(crate) fn point_list_style(&self, id: FigureId) -> Option<(f64, crate::render::LineJoin)> {
        let point_list = self.blocks.get(id)?.figure.point_list()?;
        Some((point_list.stroke_width(), point_list.line_join()))
    }

    /// 返回节点从 FigureTree 根节点开始计算的深度。
    pub fn depth(&self, id: FigureId) -> Option<usize> {
        self.blocks.get(id).map(|block| block.depth)
    }

    /// 返回节点自身的本地可见性标志。
    pub fn is_visible(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_visible)
            .unwrap_or(false)
    }

    /// 返回节点自身的本地启用标志。
    pub fn is_enabled(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_enabled)
            .unwrap_or(false)
    }

    pub fn is_opaque(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| block.is_opaque)
    }

    pub fn insets(&self, id: FigureId) -> Option<(f64, f64, f64, f64)> {
        self.blocks.get(id).map(|block| block.insets)
    }

    pub fn border_is_effectively_opaque(&self, id: FigureId) -> Option<bool> {
        let block = self.blocks.get(id)?;
        let border = block.figure.get_border()?;
        Some(border.is_opaque() && self.resolved_style(id)?.alpha >= 1.0)
    }

    pub fn figure_style(&self, id: FigureId) -> Option<&FigureStyle> {
        self.blocks.get(id).map(|block| &block.style)
    }

    pub fn resolved_style(&self, id: FigureId) -> Option<ResolvedStyle> {
        self.resolved_style_with_node_visits(id)
            .map(|(style, _)| style)
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
        self.blocks.get(id)?.figure.label()
    }

    pub(crate) fn text_flow(&self, id: FigureId) -> Option<&crate::TextFlowFigure> {
        self.blocks.get(id)?.figure.as_ref().as_any().downcast_ref()
    }

    pub(crate) fn image_figure(&self, id: FigureId) -> Option<&ImageFigure> {
        self.blocks.get(id)?.figure.as_ref().as_any().downcast_ref()
    }

    pub(crate) fn clickable_snapshot(&self, id: FigureId) -> Option<ClickableSnapshot> {
        self.blocks
            .get(id)?
            .figure
            .clickable()
            .map(|clickable| clickable.clickable_model().snapshot())
    }

    pub(crate) fn clickable_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| block.figure.clickable().is_some().then_some(id))
            .collect()
    }

    pub(crate) fn border_snapshot(&self, id: FigureId) -> Option<&BorderSnapshot> {
        self.blocks.get(id)?.border_snapshot.as_ref()
    }

    /// 返回节点沿父链传播后的有效可见性。
    pub fn is_effectively_visible(&self, id: FigureId) -> bool {
        self.effective_flag_from(id, |block| block.is_visible)
    }

    /// 返回节点沿父链传播后的有效启用状态。
    pub fn is_effectively_enabled(&self, id: FigureId) -> bool {
        self.effective_flag_from(id, |block| block.is_enabled)
    }

    pub fn is_focusable(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| block.is_focusable)
    }

    pub fn is_focus_traversable(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.is_focus_traversable)
    }

    pub fn can_request_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && self.is_focusable(id)
    }

    pub fn can_traverse_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && self.is_focus_traversable(id)
    }

    pub(crate) fn can_retain_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && (self.is_focusable(id) || self.is_focus_traversable(id))
    }

    /// 返回 node-local 到 parent content domain 的变换。
    pub fn local_to_parent_transform(&self, figure_id: FigureId) -> Option<Affine2D> {
        let bounds = self.blocks.get(figure_id)?.figure_bounds();
        Some(Affine2D::from_translation(bounds.x, bounds.y))
    }

    /// 返回 parent content domain 到 node-local 的变换。
    pub fn parent_to_local_transform(&self, figure_id: FigureId) -> Option<Affine2D> {
        self.local_to_parent_transform(figure_id)?.inverse()
    }

    /// 返回 node-local 到 logical surface 的完整父链变换。
    pub fn local_to_surface_transform(&self, figure_id: FigureId) -> Option<Affine2D> {
        let mut transform = Affine2D::IDENTITY;
        let mut current_id = figure_id;

        loop {
            let current = self.blocks.get(current_id)?;
            let bounds = current.figure_bounds();
            transform = Affine2D::from_translation(bounds.x, bounds.y) * transform;

            let Some(parent_id) = current.parent else {
                break;
            };
            let parent = self.blocks.get(parent_id)?;
            transform = parent.child_transform().affine() * transform;
            current_id = parent_id;
        }

        Some(transform)
    }

    /// 返回 logical surface domain 到 node-local 的完整父链变换。
    pub fn surface_to_local_transform(&self, figure_id: FigureId) -> Option<Affine2D> {
        self.local_to_surface_transform(figure_id)?.inverse()
    }

    /// Returns the complete child-content to logical-surface transform.
    pub fn child_content_to_surface_transform(&self, figure_id: FigureId) -> Option<Affine2D> {
        let block = self.blocks.get(figure_id)?;
        Some(self.local_to_surface_transform(figure_id)? * block.child_transform().affine())
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
