use std::{collections::HashSet, error::Error, fmt};

use super::{FigureId, FigureTree, NodeState, point_in_rect};
use crate::{ChildClippingStrategy, Figure, HitParticipation};

const RECURSIVE_STACK_CHECK_INTERVAL: usize = 16;
const RECURSIVE_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_STACK_GROWTH: usize = 4 * 1024 * 1024;

/// Read-only view of the current node passed to a tree search strategy.
#[derive(Clone, Copy)]
pub struct TreeSearchContext<'a> {
    tree: &'a FigureTree,
    id: FigureId,
}

impl<'a> TreeSearchContext<'a> {
    fn new(tree: &'a FigureTree, id: FigureId) -> Self {
        Self { tree, id }
    }

    pub fn id(self) -> FigureId {
        self.id
    }

    pub fn parent_id(self) -> Option<FigureId> {
        self.tree.blocks[self.id].parent
    }

    pub fn state(self) -> &'a NodeState {
        &self.tree.blocks[self.id].state
    }

    pub fn figure(self) -> &'a dyn Figure {
        self.tree.blocks[self.id].figure.as_ref()
    }
}

/// Acceptance and pruning policy for Figure tree searches.
pub trait TreeSearch {
    /// Returns whether the current node and its complete subtree should be skipped.
    fn prune(&mut self, _candidate: TreeSearchContext<'_>) -> bool {
        false
    }

    /// Returns whether the current node may be returned as the search result.
    fn accept(&mut self, _candidate: TreeSearchContext<'_>) -> bool {
        true
    }
}

/// Search policy that accepts every node and prunes nothing.
#[derive(Debug, Default)]
pub struct IdentitySearch;

impl TreeSearch for IdentitySearch {}

/// Search policy that excludes complete subtrees rooted at selected IDs.
#[derive(Debug, Default)]
pub struct ExclusionSearch {
    excluded: HashSet<FigureId>,
}

impl ExclusionSearch {
    pub fn new(excluded: impl IntoIterator<Item = FigureId>) -> Self {
        Self {
            excluded: excluded.into_iter().collect(),
        }
    }
}

impl TreeSearch for ExclusionSearch {
    fn prune(&mut self, candidate: TreeSearchContext<'_>) -> bool {
        self.excluded.contains(&candidate.id())
    }
}

/// Failure to start a structural Figure tree query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeQueryError {
    UnknownFigure(FigureId),
}

impl fmt::Display for TreeQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure ID: {id:?}"),
        }
    }
}

impl Error for TreeQueryError {}

struct MouseEventTargetSearch;

impl TreeSearch for MouseEventTargetSearch {
    fn accept(&mut self, candidate: TreeSearchContext<'_>) -> bool {
        candidate
            .figure()
            .event_handler()
            .is_some_and(|handler| handler.wants_mouse_events())
    }
}

impl FigureTree {
    /// Returns the topmost deepest Figure at a logical surface point.
    pub fn hit_test(&self, point: (f64, f64)) -> Option<(FigureId, Vec<FigureId>)> {
        self.hit_test_with(point, &mut IdentitySearch)
    }

    /// Runs hit-testing with an explicit acceptance and pruning strategy.
    pub fn hit_test_with(
        &self,
        point: (f64, f64),
        search: &mut dyn TreeSearch,
    ) -> Option<(FigureId, Vec<FigureId>)> {
        let start_id = self.contents.unwrap_or(self.root);
        let mut path = Vec::new();
        self.hit_test_from_with(start_id, point, &mut path, search, 0)
    }

    /// Runs hit-testing while excluding complete subtrees rooted at the given IDs.
    pub fn hit_test_excluding(
        &self,
        point: (f64, f64),
        excluded: impl IntoIterator<Item = FigureId>,
    ) -> Option<(FigureId, Vec<FigureId>)> {
        self.hit_test_with(point, &mut ExclusionSearch::new(excluded))
    }

    /// Returns only the target from [`Self::hit_test`].
    pub fn hit_test_simple(&self, point: (f64, f64)) -> Option<FigureId> {
        self.hit_test(point).map(|(target, _)| target)
    }

    pub fn find_mouse_event_target_at(&self, x: f64, y: f64) -> Option<FigureId> {
        self.hit_test_with((x, y), &mut MouseEventTargetSearch)
            .map(|(target, _)| target)
    }

    /// Returns ancestor IDs from the direct parent towards the public contents root.
    pub fn ancestor_ids(&self, id: FigureId) -> Option<Vec<FigureId>> {
        self.blocks.get(id)?;
        let mut ancestors = Vec::new();
        let mut current = self.blocks[id].parent;
        while let Some(parent) = current {
            if parent == self.root {
                break;
            }
            ancestors.push(parent);
            current = self.blocks.get(parent).and_then(|node| node.parent);
        }
        Some(ancestors)
    }

    /// Returns descendants in stable pre-order, excluding the supplied root.
    pub fn descendant_ids(&self, root: FigureId) -> Option<Vec<FigureId>> {
        let root = self.blocks.get(root)?;
        let mut descendants = Vec::new();
        let mut stack: Vec<_> = root.children.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let node = self
                .blocks
                .get(id)
                .expect("attached child ID must resolve during an immutable query");
            descendants.push(id);
            stack.extend(node.children.iter().rev().copied());
        }
        Some(descendants)
    }

    /// Returns whether `ancestor` is a strict ancestor of `candidate`.
    pub fn is_ancestor_of(&self, ancestor: FigureId, candidate: FigureId) -> bool {
        if ancestor == candidate || !self.blocks.contains_key(ancestor) {
            return false;
        }
        let mut current = self.blocks.get(candidate).and_then(|node| node.parent);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.blocks.get(id).and_then(|node| node.parent);
        }
        false
    }

    /// Finds the first accepted node in stable pre-order, including `root`.
    pub fn find_in_subtree(
        &self,
        root: FigureId,
        search: &mut dyn TreeSearch,
    ) -> Result<Option<FigureId>, TreeQueryError> {
        if !self.blocks.contains_key(root) {
            return Err(TreeQueryError::UnknownFigure(root));
        }

        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let context = TreeSearchContext::new(self, id);
            if search.prune(context) {
                continue;
            }
            if search.accept(context) {
                return Ok(Some(id));
            }
            stack.extend(self.blocks[id].children.iter().rev().copied());
        }
        Ok(None)
    }

    fn hit_test_from_with(
        &self,
        id: FigureId,
        point: (f64, f64),
        path: &mut Vec<FigureId>,
        search: &mut dyn TreeSearch,
        depth: usize,
    ) -> Option<(FigureId, Vec<FigureId>)> {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(RECURSIVE_STACK_RED_ZONE, RECURSIVE_STACK_GROWTH, || {
                self.hit_test_from_with_inner(id, point, path, search, depth)
            });
        }
        self.hit_test_from_with_inner(id, point, path, search, depth)
    }

    fn hit_test_from_with_inner(
        &self,
        id: FigureId,
        point: (f64, f64),
        path: &mut Vec<FigureId>,
        search: &mut dyn TreeSearch,
        depth: usize,
    ) -> Option<(FigureId, Vec<FigureId>)> {
        let node = self.blocks.get(id)?;
        if !node.is_visible || !node.is_enabled {
            return None;
        }

        let local_point = self
            .parent_to_local_transform(id)?
            .transform_point(point.0, point.1);
        let self_hit = node
            .figure
            .precise_hit(local_point.0, local_point.1, node.figure_bounds());
        let overflow_visible =
            node.child_clipping_strategy() == ChildClippingStrategy::OverflowVisible;
        if !self_hit && !overflow_visible {
            return None;
        }

        let context = TreeSearchContext::new(self, id);
        if search.prune(context) {
            return None;
        }

        path.push(id);
        let client_area = node.client_area();
        if overflow_visible || point_in_rect(local_point, &client_area) {
            let mut child_point = local_point;
            if node.child_transform().apply_inverse_to(&mut child_point) {
                for &child_id in node.children.iter().rev() {
                    if let Some(hit) =
                        self.hit_test_from_with(child_id, child_point, path, search, depth + 1)
                    {
                        return Some(hit);
                    }
                }
            }
        }

        let result = (self_hit
            && node.figure.hit_participation() == HitParticipation::SelfAndDescendants
            && search.accept(context))
        .then(|| (id, path.clone()));
        path.pop();
        result
    }
}

#[cfg(test)]
mod tests {
    use slotmap::KeyData;

    use super::*;
    use crate::RectangleFigure;

    #[derive(Default)]
    struct RecordingSearch {
        rejected: Option<FigureId>,
        pruned: Option<FigureId>,
        accepted_calls: Vec<FigureId>,
        pruned_calls: Vec<FigureId>,
    }

    impl TreeSearch for RecordingSearch {
        fn prune(&mut self, candidate: TreeSearchContext<'_>) -> bool {
            self.pruned_calls.push(candidate.id());
            self.pruned == Some(candidate.id())
        }

        fn accept(&mut self, candidate: TreeSearchContext<'_>) -> bool {
            self.accepted_calls.push(candidate.id());
            self.rejected != Some(candidate.id())
        }
    }

    #[test]
    fn hit_test_checks_deepest_candidate_before_parent_acceptance() {
        let mut tree = FigureTree::new();
        let parent = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = tree.add_child_to(
            parent,
            Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)),
        );
        let mut search = RecordingSearch {
            rejected: Some(child),
            ..RecordingSearch::default()
        };

        let hit = tree.hit_test_with((20.0, 20.0), &mut search);

        assert_eq!(hit.map(|(target, _)| target), Some(parent));
        assert_eq!(search.pruned_calls, vec![parent, child]);
        assert_eq!(search.accepted_calls, vec![child, parent]);
    }

    #[test]
    fn exclusion_search_prunes_the_complete_topmost_subtree() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let lower = tree.add_child_to(root, Box::new(RectangleFigure::new(10.0, 10.0, 60.0, 60.0)));
        let excluded =
            tree.add_child_to(root, Box::new(RectangleFigure::new(10.0, 10.0, 60.0, 60.0)));
        tree.add_child_to(
            excluded,
            Box::new(RectangleFigure::new(0.0, 0.0, 40.0, 40.0)),
        );

        let hit = tree.hit_test_excluding((20.0, 20.0), [excluded]);

        assert_eq!(hit.map(|(target, _)| target), Some(lower));
    }

    #[test]
    fn structural_queries_have_stable_order_and_hide_the_synthetic_root() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let first = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let nested = tree.add_child_to(first, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let second = tree.add_child_to(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)));

        assert_eq!(tree.ancestor_ids(root), Some(Vec::new()));
        assert_eq!(tree.ancestor_ids(nested), Some(vec![first, root]));
        assert_eq!(tree.descendant_ids(root), Some(vec![first, nested, second]));
        assert!(tree.is_ancestor_of(root, nested));
        assert!(!tree.is_ancestor_of(nested, root));
        assert!(!tree.is_ancestor_of(root, root));
    }

    #[test]
    fn find_in_subtree_distinguishes_unknown_root_from_no_match() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let mut no_match = RecordingSearch {
            rejected: Some(root),
            pruned: Some(child),
            ..RecordingSearch::default()
        };
        let stale = FigureId::from(KeyData::from_ffi(u64::MAX));

        assert_eq!(tree.find_in_subtree(root, &mut no_match), Ok(None));
        assert_eq!(
            tree.find_in_subtree(stale, &mut IdentitySearch),
            Err(TreeQueryError::UnknownFigure(stale))
        );
    }

    #[test]
    fn structural_prune_never_visits_descendants() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let branch = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let leaf = tree.add_child_to(branch, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let sibling =
            tree.add_child_to(root, Box::new(RectangleFigure::new(30.0, 0.0, 20.0, 20.0)));
        let mut search = RecordingSearch {
            rejected: Some(root),
            pruned: Some(branch),
            ..RecordingSearch::default()
        };

        assert_eq!(tree.find_in_subtree(root, &mut search), Ok(Some(sibling)));
        assert!(!search.pruned_calls.contains(&leaf));
        assert!(!search.accepted_calls.contains(&leaf));
    }
}
