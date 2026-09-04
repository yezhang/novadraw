use std::{error::Error, fmt};

use crate::{FigureId, FigureTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTraversalDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusChange {
    Unchanged,
    Changed {
        previous: Option<FigureId>,
        current: Option<FigureId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTraversalOutcome {
    Moved(FigureId),
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusError {
    UnknownFigure(FigureId),
    Detached(FigureId),
    Hidden(FigureId),
    Disabled(FigureId),
    NotFocusable(FigureId),
}

impl fmt::Display for FocusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure ID: {id:?}"),
            Self::Detached(id) => write!(formatter, "Figure is detached: {id:?}"),
            Self::Hidden(id) => write!(formatter, "Figure is not effectively visible: {id:?}"),
            Self::Disabled(id) => write!(formatter, "Figure is not effectively enabled: {id:?}"),
            Self::NotFocusable(id) => {
                write!(formatter, "Figure does not accept direct focus: {id:?}")
            }
        }
    }
}

impl Error for FocusError {}

pub trait FocusTraversalPolicy {
    fn traverse(
        &mut self,
        tree: &FigureTree,
        scope: FigureId,
        current: Option<FigureId>,
        direction: FocusTraversalDirection,
    ) -> Option<FigureId>;
}

#[derive(Debug, Default)]
pub struct TreeOrderFocusTraversal;

impl FocusTraversalPolicy for TreeOrderFocusTraversal {
    fn traverse(
        &mut self,
        tree: &FigureTree,
        scope: FigureId,
        current: Option<FigureId>,
        direction: FocusTraversalDirection,
    ) -> Option<FigureId> {
        let order = tree.descendant_ids(scope)?;
        let current_index =
            current.and_then(|id| order.iter().position(|candidate| *candidate == id));

        match (direction, current_index) {
            (FocusTraversalDirection::Forward, Some(index)) => order[index + 1..]
                .iter()
                .copied()
                .find(|id| tree.can_traverse_focus(*id)),
            (FocusTraversalDirection::Backward, Some(index)) => order[..index]
                .iter()
                .rev()
                .copied()
                .find(|id| tree.can_traverse_focus(*id)),
            (FocusTraversalDirection::Forward, None) => {
                order.into_iter().find(|id| tree.can_traverse_focus(*id))
            }
            (FocusTraversalDirection::Backward, None) => order
                .into_iter()
                .rev()
                .find(|id| tree.can_traverse_focus(*id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NotificationEffect, PropertyValue, RectangleFigure};

    #[test]
    fn focus_properties_are_local_typed_node_state() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        tree.drain_notification_effects();

        assert!(tree.set_focusable(root, true));
        assert!(tree.set_focus_traversable(root, true));
        assert!(tree.is_focusable(root));
        assert!(tree.is_focus_traversable(root));

        let properties: Vec<_> = tree
            .drain_notification_effects()
            .into_iter()
            .filter_map(|effect| match effect {
                NotificationEffect::EmitProperty(event) => {
                    Some((event.property, event.old_value, event.new_value))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            properties,
            vec![
                (
                    "focusable",
                    PropertyValue::Bool(false),
                    PropertyValue::Bool(true),
                ),
                (
                    "focus_traversable",
                    PropertyValue::Bool(false),
                    PropertyValue::Bool(true),
                ),
            ]
        );
    }

    #[test]
    fn tree_order_policy_is_stable_bidirectional_and_has_boundaries() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let first = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let skipped =
            tree.add_child_to(root, Box::new(RectangleFigure::new(20.0, 0.0, 10.0, 10.0)));
        let last = tree.add_child_to(root, Box::new(RectangleFigure::new(40.0, 0.0, 10.0, 10.0)));
        tree.set_focus_traversable(first, true);
        tree.set_focus_traversable(last, true);

        let mut policy = TreeOrderFocusTraversal;

        assert_eq!(
            policy.traverse(&tree, root, None, FocusTraversalDirection::Forward),
            Some(first)
        );
        assert_eq!(
            policy.traverse(&tree, root, Some(first), FocusTraversalDirection::Forward),
            Some(last)
        );
        assert_eq!(
            policy.traverse(&tree, root, Some(last), FocusTraversalDirection::Backward),
            Some(first)
        );
        assert_eq!(
            policy.traverse(&tree, root, None, FocusTraversalDirection::Backward),
            Some(last)
        );
        assert_eq!(
            policy.traverse(&tree, root, Some(last), FocusTraversalDirection::Forward),
            None
        );
        assert_eq!(
            policy.traverse(&tree, root, Some(first), FocusTraversalDirection::Backward),
            None
        );
        assert!(!tree.can_traverse_focus(skipped));
    }

    #[test]
    fn tree_order_policy_skips_hidden_and_disabled_candidates() {
        let mut tree = FigureTree::new();
        let root = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let hidden = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let disabled =
            tree.add_child_to(root, Box::new(RectangleFigure::new(20.0, 0.0, 10.0, 10.0)));
        let eligible =
            tree.add_child_to(root, Box::new(RectangleFigure::new(40.0, 0.0, 10.0, 10.0)));
        for id in [hidden, disabled, eligible] {
            tree.set_focus_traversable(id, true);
        }
        tree.set_visible(hidden, false);
        tree.set_enabled(disabled, false);

        assert_eq!(
            TreeOrderFocusTraversal.traverse(&tree, root, None, FocusTraversalDirection::Forward),
            Some(eligible)
        );
    }
}
