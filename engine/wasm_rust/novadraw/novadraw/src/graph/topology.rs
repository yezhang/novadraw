use super::*;

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
            ChildPolicy::Layered if self.blocks[child_id].figure.layer().is_none() => {
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
