use super::*;
use crate::figure::Figure;

impl FigureTree {
    pub(super) fn validate_direct_child(
        &self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<usize, GraphMutationError> {
        let parent_node = self
            .blocks
            .get(parent)
            .ok_or(GraphMutationError::ParentNotFound)?;
        if parent_node.child_policy() == ChildPolicy::Layered {
            return Err(GraphMutationError::LayerKeyRequired);
        }
        self.ensure_figure(child)?;
        if self.parent_id(child) != Some(parent) {
            return Err(GraphMutationError::InvalidParentRelation);
        }
        Ok(parent_node.children.len())
    }

    pub(super) fn validate_child_order_mutation(
        &self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<(), GraphMutationError> {
        let child_count = self.validate_direct_child(parent, child)?;
        if index >= child_count {
            return Err(GraphMutationError::InvalidChildIndex {
                parent,
                index,
                child_count,
            });
        }
        Ok(())
    }

    pub(super) fn attach_child_checked(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<(), GraphMutationError> {
        self.attach_child_checked_admission(parent_id, child_id, false)
    }

    pub(super) fn attach_child_checked_admission(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
        layer_admission: bool,
    ) -> Result<(), GraphMutationError> {
        let new_depth = self.validate_attachment_admission(parent_id, child_id, layer_admission)?;

        self.blocks[parent_id].children.push(child_id);
        {
            let child = &mut self.blocks[child_id];
            child.parent = Some(parent_id);
            child.is_valid = false;
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Added,
            figure_id: child_id,
            parent_id,
        });
        self.set_subtree_depth(child_id, new_depth);
        self.mark_validation_path_invalid(parent_id);
        Ok(())
    }

    pub(super) fn detach_child(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };

        let old_len = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == old_len {
            return false;
        }
        parent.layout.constraints.remove(&child_id);

        if let Some(child) = self.blocks.get_mut(child_id) {
            child.parent = None;
            child.is_valid = false;
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Removed,
            figure_id: child_id,
            parent_id,
        });
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ChildRemoved,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        self.set_subtree_depth(child_id, 0);
        self.mark_validation_path_invalid(parent_id);
        true
    }

    pub(super) fn validate_attachment(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<usize, GraphMutationError> {
        self.validate_attachment_admission(parent_id, child_id, false)
    }

    pub(super) fn validate_attachment_admission(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
        layer_admission: bool,
    ) -> Result<usize, GraphMutationError> {
        let parent = self
            .blocks
            .get(parent_id)
            .ok_or(GraphMutationError::ParentNotFound)?;
        if !self.blocks.contains_key(child_id) {
            return Err(GraphMutationError::ChildNotFound);
        }

        if parent_id == child_id || self.is_descendant_or_self(parent_id, child_id) {
            return Err(GraphMutationError::CycleDetected);
        }
        if parent.children.contains(&child_id) {
            return Err(GraphMutationError::DuplicateChild);
        }
        match parent.child_policy() {
            ChildPolicy::Single if !parent.children.is_empty() => {
                return Err(GraphMutationError::ChildLimitExceeded { limit: 1 });
            }
            ChildPolicy::Layered if !layer_admission => {
                return Err(GraphMutationError::LayerKeyRequired);
            }
            ChildPolicy::Layered
                if self.blocks[child_id]
                    .capabilities
                    .get(LAYER)
                    .expect("typed marker descriptors must match their key")
                    .is_none() =>
            {
                return Err(GraphMutationError::LayerChildRequired);
            }
            _ => {}
        }

        let new_depth =
            parent
                .depth
                .checked_add(1)
                .ok_or(GraphMutationError::DepthLimitExceeded {
                    limit: MAX_TREE_DEPTH,
                })?;
        let subtree_height = self.subtree_height(child_id);
        if new_depth
            .checked_add(subtree_height)
            .is_none_or(|depth| depth > MAX_TREE_DEPTH)
        {
            return Err(GraphMutationError::DepthLimitExceeded {
                limit: MAX_TREE_DEPTH,
            });
        }

        Ok(new_depth)
    }

    fn subtree_height(&self, root_id: FigureId) -> usize {
        let Some(root) = self.blocks.get(root_id) else {
            return 0;
        };
        let root_depth = root.depth;
        let mut max_depth = root_depth;
        let mut stack = vec![root_id];
        while let Some(id) = stack.pop() {
            let Some(block) = self.blocks.get(id) else {
                continue;
            };
            max_depth = max_depth.max(block.depth);
            stack.extend(block.children.iter().copied());
        }
        max_depth.saturating_sub(root_depth)
    }

    fn set_subtree_depth(&mut self, root_id: FigureId, root_depth: usize) {
        let mut stack = vec![(root_id, root_depth)];
        while let Some((id, depth)) = stack.pop() {
            let children = match self.blocks.get_mut(id) {
                Some(block) => {
                    block.depth = depth;
                    block.children.clone()
                }
                None => continue,
            };
            stack.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
    }

    pub(super) fn contains_direct_child(&self, parent_id: FigureId, child_id: FigureId) -> bool {
        self.blocks
            .get(parent_id)
            .is_some_and(|parent| parent.children.contains(&child_id))
    }

    fn is_descendant_or_self(&self, mut node: FigureId, ancestor: FigureId) -> bool {
        for _ in 0..self.blocks.len() {
            if node == ancestor {
                return true;
            }
            let Some(parent) = self.blocks.get(node).and_then(|block| block.parent) else {
                return false;
            };
            node = parent;
        }
        false
    }

    /// 将直接 child 移动到指定 z-order index。
    ///
    /// `index == 0` 表示最底层；`index == children.len() - 1` 表示最顶层。
    pub(crate) fn move_child_to_index(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
        index: usize,
    ) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };

        let Some(old_index) = parent.children.iter().position(|&id| id == child_id) else {
            return false;
        };

        if index >= parent.children.len() || old_index == index {
            return false;
        }

        let child = parent.children.remove(old_index);
        parent.children.insert(index, child);
        self.notify_block_changed(parent_id);
        true
    }

    pub(crate) fn set_child_order(&mut self, parent_id: FigureId, order: &[FigureId]) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };
        if parent.children.as_slice() == order {
            return false;
        }
        parent.children.clear();
        parent.children.extend_from_slice(order);
        self.notify_block_changed(parent_id);
        true
    }

    /// 将直接 child 移动到最高 z-order。
    #[cfg(test)]
    pub(crate) fn bring_child_to_front(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        let Some(last_index) = self
            .blocks
            .get(parent_id)
            .and_then(|parent| parent.children.len().checked_sub(1))
        else {
            return false;
        };
        self.move_child_to_index(parent_id, child_id, last_index)
    }

    /// 将直接 child 移动到最低 z-order。
    #[cfg(test)]
    pub(crate) fn send_child_to_back(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        self.move_child_to_index(parent_id, child_id, 0)
    }
}

impl FigureTree {
    pub(crate) fn complete_attachment(&mut self, figure: FigureId, parent: FigureId) {
        let runtime_namespace = self.namespace();
        if let Some(lifecycle) = self.blocks[figure].lifecycle() {
            lifecycle.on_attached(crate::FigureLifecycleContext {
                figure_id: figure,
                parent_id: parent,
                runtime_namespace,
            });
        }
    }

    pub(crate) fn complete_detachment(&mut self, figure: FigureId, parent: FigureId) {
        let runtime_namespace = self.namespace();
        if let Some(lifecycle) = self.blocks[figure].lifecycle() {
            lifecycle.on_detached(crate::FigureLifecycleContext {
                figure_id: figure,
                parent_id: parent,
                runtime_namespace,
            });
        }
    }

    pub(crate) fn attached_ids_parent_first(&self) -> Vec<(FigureId, FigureId)> {
        let Some(contents) = self.contents else {
            return Vec::new();
        };
        let mut ids = vec![(contents, self.root)];
        for id in self.descendant_ids(contents).unwrap_or_default() {
            if let Some(parent) = self.parent_id(id) {
                ids.push((id, parent));
            }
        }
        ids
    }

    pub(crate) fn designate_contents(&mut self, id: FigureId) {
        assert_eq!(self.parent_id(id), Some(self.root));
        self.contents = Some(id);
    }

    pub(crate) fn disposal_ids(&self, root: FigureId) -> Result<Vec<FigureId>, GraphMutationError> {
        if root == self.root || !self.is_attached(root) {
            return Err(GraphMutationError::InvalidParentRelation);
        }
        let mut ids = vec![root];
        ids.extend(
            self.descendant_ids(root)
                .ok_or(GraphMutationError::ChildNotFound)?,
        );
        Ok(ids)
    }

    pub(crate) fn extract_subtree(
        &mut self,
        ids: &[FigureId],
        updates: &mut UpdateManager,
    ) -> RetiredSubtree {
        let root = ids[0];
        let parent = self.blocks[root].parent.expect("validated subtree parent");
        let mut nodes = Vec::with_capacity(ids.len());
        self.blocks[parent].children.retain(|id| *id != root);
        let parent_constraint = self.blocks[parent].layout.constraints.remove(&root);
        for &id in ids.iter().rev() {
            let node = self.blocks.remove(id).expect("validated subtree node");
            self.uuid_map.remove(&node.uuid);
            nodes.push(node);
        }
        if self.contents == Some(root) {
            self.contents = None;
        }
        // No component callbacks until Runtime has retired its other references.
        updates.add_invalid_figure(parent);
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Removed,
            figure_id: root,
            parent_id: parent,
        });
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ChildRemoved,
            container_id: parent,
            child_id: Some(root),
        });
        RetiredSubtree {
            nodes,
            parent_constraint,
        }
    }

    /// 设置内容块
    ///
    /// 对应 draw2d: LightweightSystem.setContents(IFigure)
    ///
    /// 设置场景的根容器，后续添加的子块将作为此容器的子元素。
    /// 注意：此方法不触发 revalidate()，用于批量构建场景。
    /// 交互式修改使用 SceneManager.set_contents() 方法。
    pub(crate) fn set_contents(
        &mut self,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        let contents_id =
            self.new_block_with_parent(figure, self.root)
                .map_err(|error| match error {
                    GraphMutationError::ParentNotFound => {
                        unreachable!("FigureTree synthetic root must exist")
                    }
                    other => other,
                })?;
        if let Some(previous) = self.contents {
            let ids = self.disposal_ids(previous).expect("attached contents");
            let mut updates = UpdateManager::with_namespace(self.namespace());
            drop(self.extract_subtree(&ids, &mut updates));
        }
        self.contents = Some(contents_id);
        self.invalidate();
        Ok(contents_id)
    }

    /// 获取内容块
    pub fn contents(&self) -> Option<FigureId> {
        self.contents
    }

    /// Returns the synthetic root that owns every attached Figure.
    pub const fn root_id(&self) -> FigureId {
        self.root
    }

    /// Adds a Figure to a parent during construction.
    ///
    /// 对应 draw2d: parent.addChild(child) (不触发 revalidate)
    ///
    /// 与 `add_child()` 的区别：此方法不触发 revalidate()，用于批量构建场景。
    #[cfg(test)]
    pub(crate) fn add_child_to(
        &mut self,
        parent_id: FigureId,
        figure: Box<dyn Figure>,
    ) -> FigureId {
        self.try_add_child_to(parent_id, figure)
            .expect("validated construction must preserve FigureTree invariants")
    }

    pub(crate) fn try_add_child_to(
        &mut self,
        parent_id: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.new_block_with_parent(figure, parent_id)
    }

    pub(crate) fn insert_child_at(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.insert_child_prepared(figure, parent, false, Some(index), |_, _| Ok(None))
    }

    pub(crate) fn insert_child_with_constraint_at(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn Figure>,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<FigureId, ChildInsertionError> {
        self.insert_child_prepared(figure, parent, false, Some(index), |tree, child| {
            if let Some(manager) = tree.layout_manager(parent) {
                manager.validate_constraint(parent, child, constraint.as_ref())?;
            }
            Ok(Some(constraint))
        })
    }

    #[cfg(test)]
    pub(crate) fn add_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        figure: Box<dyn Figure>,
    ) -> FigureId {
        self.try_add_child(update_manager, parent_id, figure)
            .unwrap_or_else(|_| FigureId::null())
    }

    pub(crate) fn try_add_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        let child_id = self.try_add_child_to(parent_id, figure)?;
        self.invalidate_child_insertion(update_manager, parent_id, child_id);
        Ok(child_id)
    }

    pub(crate) fn invalidate_child_insertion(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        child_id: FigureId,
    ) {
        let visual_bounds = self.blocks[child_id].visual_bounds();
        self.mark_invalid(update_manager, parent_id);
        update_manager.add_dirty_region(child_id, visual_bounds);
        self.mark_invalid(update_manager, child_id);
    }

    #[cfg(test)]
    pub(crate) fn apply_pending_mutations(
        &mut self,
        update_manager: &mut UpdateManager,
        mutations: Vec<PendingMutation>,
    ) -> bool {
        if mutations.is_empty() {
            return false;
        }

        let mut changed = false;
        for mutation in mutations {
            changed |= match mutation.into_kind() {
                kind @ PendingMutationKind::RemoveChild { .. } => {
                    self.apply_remove_mutation(update_manager, kind)
                }
                kind @ PendingMutationKind::Reparent { .. } => {
                    self.apply_reparent_mutation(update_manager, kind)
                }
                kind @ PendingMutationKind::AddChildFigure { .. } => {
                    self.apply_add_mutation(update_manager, kind)
                }
                PendingMutationKind::AddLayerFigure { .. }
                | PendingMutationKind::RemoveLayer { .. }
                | PendingMutationKind::MoveLayer { .. }
                | PendingMutationKind::ReparentLayer { .. }
                | PendingMutationKind::SetLayoutManager { .. }
                | PendingMutationKind::SetLayoutConstraint { .. }
                | PendingMutationKind::RemoveLayoutConstraint { .. }
                | PendingMutationKind::SetSizeOverride { .. }
                | PendingMutationKind::SetBounds { .. }
                | PendingMutationKind::MoveChildToIndex { .. }
                | PendingMutationKind::BringChildToFront { .. }
                | PendingMutationKind::SendChildToBack { .. }
                | PendingMutationKind::SetChildClippingStrategy { .. }
                | PendingMutationKind::UpdateComponent(_)
                | PendingMutationKind::UpdateCapability(_) => false,
            };
        }

        changed
    }

    /// Removes a direct child through the update transaction.
    pub(crate) fn remove_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent: FigureId,
        child: FigureId,
    ) -> bool {
        self.apply_remove_mutation(
            update_manager,
            PendingMutationKind::RemoveChild { parent, child },
        )
    }

    /// Reparents a block through the update transaction.
    pub(crate) fn reparent(
        &mut self,
        update_manager: &mut UpdateManager,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<bool, GraphMutationError> {
        let old_parent = self
            .blocks
            .get(child)
            .ok_or(GraphMutationError::ChildNotFound)?
            .parent
            .ok_or(GraphMutationError::InvalidParentRelation)?;
        self.blocks
            .get(new_parent)
            .ok_or(GraphMutationError::ParentNotFound)?;
        if old_parent == new_parent {
            return Ok(false);
        }
        if self.blocks[old_parent].child_policy() == ChildPolicy::Layered {
            return Err(GraphMutationError::LayerKeyRequired);
        }
        self.validate_attachment(new_parent, child)?;
        let changed = self.apply_reparent_mutation(
            update_manager,
            PendingMutationKind::Reparent { child, new_parent },
        );
        debug_assert!(changed, "validated reparent must commit");
        Ok(changed)
    }

    fn new_block_with_parent(
        &mut self,
        figure: Box<dyn Figure>,
        parent_id: FigureId,
    ) -> Result<FigureId, GraphMutationError> {
        self.new_block_with_parent_admission(figure, parent_id, false)
    }

    fn new_block_with_parent_admission(
        &mut self,
        figure: Box<dyn Figure>,
        parent_id: FigureId,
        layer_admission: bool,
    ) -> Result<FigureId, GraphMutationError> {
        self.insert_child_prepared(figure, parent_id, layer_admission, None, |_, _| Ok(None))
    }

    fn insert_child_prepared<E: From<GraphMutationError>>(
        &mut self,
        figure: Box<dyn Figure>,
        parent_id: FigureId,
        layer_admission: bool,
        index: Option<usize>,
        prepare: impl FnOnce(&Self, FigureId) -> Result<Option<Box<dyn LayoutConstraint>>, E>,
    ) -> Result<FigureId, E> {
        let bounds = figure.initial_bounds();
        if !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width < 0.0
            || bounds.height < 0.0
        {
            return Err(GraphMutationError::InvalidInitialBounds.into());
        }
        let insets = figure.initial_insets();
        let style = figure.initial_style();
        let is_focusable = figure.initial_focusable();
        let is_focus_traversable = figure.initial_focus_traversable();
        let capabilities = FigureCapabilitySet::build(figure.as_ref())
            .map_err(|error| E::from(GraphMutationError::CapabilityRegistration(error)))?;
        let layout = LayoutState::for_capabilities(&capabilities);
        let parent_depth = self
            .blocks
            .get(parent_id)
            .map(|parent| parent.depth)
            .ok_or(GraphMutationError::ParentNotFound)?;
        let parent = &self.blocks[parent_id];
        match parent.child_policy() {
            ChildPolicy::Single if !parent.children.is_empty() => {
                return Err(GraphMutationError::ChildLimitExceeded { limit: 1 }.into());
            }
            ChildPolicy::Layered if !layer_admission => {
                return Err(GraphMutationError::LayerKeyRequired.into());
            }
            ChildPolicy::Layered
                if capabilities
                    .get(LAYER)
                    .expect("typed marker descriptors must match their key")
                    .is_none() =>
            {
                return Err(GraphMutationError::LayerChildRequired.into());
            }
            _ => {}
        }
        let index = index.unwrap_or(parent.children.len());
        if index > parent.children.len() {
            return Err(GraphMutationError::InvalidChildIndex {
                parent: parent_id,
                index,
                child_count: parent.children.len(),
            }
            .into());
        }
        let depth = parent_depth
            .checked_add(1)
            .filter(|depth| *depth <= MAX_TREE_DEPTH)
            .ok_or(GraphMutationError::DepthLimitExceeded {
                limit: MAX_TREE_DEPTH,
            })?;

        let uuid = Uuid::new_v4();
        let id = self.blocks.insert_with_key(|key| FigureNode {
            id: key,
            uuid,
            children: Vec::new(),
            parent: None,
            depth,
            figure,
            component_revision: 0,
            capabilities,
            prepared: None,
            layout,
            state: NodeState {
                bounds,
                insets,
                is_focusable,
                is_focus_traversable,
                style,
                ..NodeState::default()
            },
        });
        // Reserve an identity without publishing topology, UUID lookup or effects. Retiring a
        // rejected reservation ensures that an ID seen by a validator never aliases a later child.
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(self, id)));
        let constraint = match prepared {
            Ok(Ok(constraint)) => constraint,
            Ok(Err(error)) => {
                drop(self.blocks.remove(id));
                return Err(error);
            }
            Err(payload) => {
                drop(self.blocks.remove(id));
                std::panic::resume_unwind(payload);
            }
        };
        self.blocks[id].parent = Some(parent_id);
        self.uuid_map.insert(uuid, id);
        self.blocks[parent_id].children.insert(index, id);
        let has_constraint = constraint.is_some();
        if let Some(constraint) = constraint {
            self.blocks[parent_id]
                .layout
                .constraints
                .insert(id, constraint);
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Added,
            figure_id: id,
            parent_id,
        });
        if has_constraint {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::ConstraintChanged,
                container_id: parent_id,
                child_id: Some(id),
            });
        }
        self.mark_validation_path_invalid(parent_id);
        Ok(id)
    }

    pub(crate) fn add_layer_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        let child_id = self.new_block_with_parent_admission(figure, parent_id, true)?;
        let visual_bounds = self.blocks[child_id].visual_bounds();
        self.mark_invalid(update_manager, parent_id);
        update_manager.add_dirty_region(child_id, visual_bounds);
        self.mark_invalid(update_manager, child_id);
        Ok(child_id)
    }

    fn apply_remove_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::RemoveChild { parent, child } = mutation else {
            return false;
        };
        if !self.blocks.contains_key(child) {
            return false;
        }
        if self
            .blocks
            .get(parent)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
        {
            return false;
        }

        self.remove_child_internal(update_manager, parent, child)
    }

    fn remove_child_internal(
        &mut self,
        update_manager: &mut UpdateManager,
        parent: FigureId,
        child: FigureId,
    ) -> bool {
        if !self.blocks.contains_key(child) || !self.detach_child(parent, child) {
            return false;
        }

        if self.contents == Some(child) {
            self.contents = None;
        }

        self.mark_invalid(update_manager, parent);
        self.repaint(update_manager, parent, None);
        true
    }

    pub(super) fn apply_reparent_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::Reparent { child, new_parent } = mutation else {
            return false;
        };
        let old_parent = self.blocks.get(child).and_then(|block| block.parent);
        if old_parent.is_none() || old_parent == Some(new_parent) {
            return false;
        }

        let Some(visual_bounds) = self.blocks.get(child).map(FigureNode::visual_bounds) else {
            return false;
        };
        let Some(old_parent) = old_parent else {
            return false;
        };
        if self.blocks[old_parent].child_policy() == ChildPolicy::Layered {
            return false;
        }
        if !self.contains_direct_child(old_parent, child)
            || self.contains_direct_child(new_parent, child)
            || self.validate_attachment(new_parent, child).is_err()
        {
            return false;
        }

        self.detach_child(old_parent, child);
        self.mark_invalid(update_manager, old_parent);
        self.repaint(update_manager, old_parent, None);

        if self.attach_child_checked(new_parent, child).is_err() {
            return false;
        }

        self.mark_invalid(update_manager, new_parent);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, new_parent, None);
        true
    }

    pub(crate) fn reparent_layer_child(
        &mut self,
        update_manager: &mut UpdateManager,
        child: FigureId,
        new_parent: FigureId,
    ) -> bool {
        let Some(old_parent) = self.blocks.get(child).and_then(|block| block.parent) else {
            return false;
        };
        if old_parent == new_parent
            || !self.contains_direct_child(old_parent, child)
            || self.contains_direct_child(new_parent, child)
            || self
                .validate_attachment_admission(new_parent, child, true)
                .is_err()
        {
            return false;
        }
        let visual_bounds = self.blocks[child].visual_bounds();
        self.detach_child(old_parent, child);
        self.mark_invalid(update_manager, old_parent);
        self.repaint(update_manager, old_parent, None);
        if self
            .attach_child_checked_admission(new_parent, child, true)
            .is_err()
        {
            return false;
        }
        self.mark_invalid(update_manager, new_parent);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, new_parent, None);
        true
    }

    #[cfg(test)]
    fn apply_add_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::AddChildFigure { parent, figure } = mutation else {
            return false;
        };
        let Ok(child) = self.new_block_with_parent(figure, parent) else {
            return false;
        };
        let visual_bounds = self.blocks[child].visual_bounds();

        self.mark_invalid(update_manager, parent);
        self.mark_invalid(update_manager, child);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, parent, None);
        true
    }
}
