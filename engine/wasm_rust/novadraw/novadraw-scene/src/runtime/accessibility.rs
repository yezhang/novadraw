//! Stable accessibility projection derived from the Figure tree.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    sync::Arc,
};

use novadraw_geometry::{Affine2D, Rectangle, Translatable};
use novadraw_render::SurfaceInfo;

use crate::{FigureId, FigureTree, InteractionState, MAX_TREE_DEPTH, RuntimeNamespace};

const RECURSIVE_STACK_CHECK_INTERVAL: usize = 16;
const RECURSIVE_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_STACK_GROWTH: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessibilityNodeId {
    Root(RuntimeNamespace),
    Figure(FigureId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityRole {
    Group,
    Text,
    Image,
    Button,
    ToggleButton,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityAction {
    Focus,
    Default,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccessibilityState {
    pub enabled: bool,
    pub focusable: bool,
    pub focused: bool,
    pub pressed: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibilityNode {
    pub id: AccessibilityNodeId,
    pub parent: AccessibilityNodeId,
    pub children: Vec<AccessibilityNodeId>,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub role: AccessibilityRole,
    pub state: AccessibilityState,
    pub bounds: Rectangle,
    pub default_action: Option<AccessibilityAction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibilitySnapshot {
    pub revision: u64,
    pub stable_epoch: u64,
    pub root: AccessibilityNodeId,
    pub nodes: Vec<AccessibilityNode>,
    pub focus: Option<AccessibilityNodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibilityDelta {
    pub base_revision: u64,
    pub revision: u64,
    pub stable_epoch: u64,
    pub upserts: Vec<AccessibilityNode>,
    pub removed: Vec<AccessibilityNodeId>,
    pub root_children: Vec<AccessibilityNodeId>,
    pub focus: Option<AccessibilityNodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccessibilityUpdate {
    Snapshot(Arc<AccessibilitySnapshot>),
    Delta(AccessibilityDelta),
}

impl AccessibilityUpdate {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Snapshot(snapshot) => snapshot.revision,
            Self::Delta(delta) => delta.revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityError {
    UnknownNode(AccessibilityNodeId),
    ForeignRuntime(AccessibilityNodeId),
    Unavailable(AccessibilityNodeId),
    UnsupportedAction {
        node: AccessibilityNodeId,
        action: AccessibilityAction,
    },
    RuntimeFaulted,
    DepthLimit,
    RevisionExhausted,
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(node) => write!(formatter, "unknown accessibility node: {node:?}"),
            Self::ForeignRuntime(node) => {
                write!(
                    formatter,
                    "accessibility node belongs to another Runtime: {node:?}"
                )
            }
            Self::Unavailable(node) => {
                write!(formatter, "accessibility node is unavailable: {node:?}")
            }
            Self::UnsupportedAction { node, action } => {
                write!(
                    formatter,
                    "accessibility node {node:?} does not support {action:?}"
                )
            }
            Self::RuntimeFaulted => formatter.write_str("Runtime is faulted"),
            Self::DepthLimit => formatter.write_str("accessibility tree exceeds depth limit"),
            Self::RevisionExhausted => {
                formatter.write_str("accessibility revision space is exhausted")
            }
        }
    }
}

impl std::error::Error for AccessibilityError {}

#[derive(Default)]
pub(crate) struct AccessibilityManager {
    revision: u64,
    published: Option<Arc<AccessibilitySnapshot>>,
    updates: VecDeque<AccessibilityUpdate>,
    force_snapshot: bool,
}

impl AccessibilityManager {
    pub(crate) fn reset(&mut self) {
        self.published = None;
        self.updates.clear();
        self.force_snapshot = true;
    }

    pub(crate) fn publish(
        &mut self,
        tree: &FigureTree,
        interaction: &InteractionState,
        stable_epoch: u64,
        surface: SurfaceInfo,
    ) -> Result<bool, AccessibilityError> {
        let mut candidate =
            build_snapshot(tree, interaction, self.revision, stable_epoch, surface)?;
        let Some(previous) = self.published.as_ref() else {
            let next_revision = self
                .revision
                .checked_add(1)
                .ok_or(AccessibilityError::RevisionExhausted)?;
            candidate.revision = next_revision;
            let candidate = Arc::new(candidate);
            self.revision = next_revision;
            self.published = Some(candidate.clone());
            self.force_snapshot = false;
            self.updates
                .push_back(AccessibilityUpdate::Snapshot(candidate));
            return Ok(true);
        };

        if same_semantics(previous, &candidate) {
            return Ok(false);
        }

        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(AccessibilityError::RevisionExhausted)?;
        candidate.revision = next_revision;
        let update = if self.force_snapshot {
            AccessibilityUpdate::Snapshot(Arc::new(candidate.clone()))
        } else {
            AccessibilityUpdate::Delta(build_delta(previous, &candidate))
        };
        self.revision = next_revision;
        self.published = Some(Arc::new(candidate));
        self.force_snapshot = false;
        self.updates.push_back(update);
        Ok(true)
    }

    pub(crate) fn snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.published.as_deref()
    }

    pub(crate) fn take_updates(&mut self) -> Vec<AccessibilityUpdate> {
        self.updates.drain(..).collect()
    }
}

fn build_snapshot(
    tree: &FigureTree,
    interaction: &InteractionState,
    revision: u64,
    stable_epoch: u64,
    surface: SurfaceInfo,
) -> Result<AccessibilitySnapshot, AccessibilityError> {
    let root_id = AccessibilityNodeId::Root(tree.namespace());
    let mut nodes = vec![AccessibilityNode {
        id: root_id,
        parent: root_id,
        children: Vec::new(),
        name: "Novadraw scene".to_string(),
        description: None,
        value: None,
        role: AccessibilityRole::Group,
        state: AccessibilityState {
            enabled: true,
            ..AccessibilityState::default()
        },
        bounds: Rectangle::new(0.0, 0.0, surface.logical_width, surface.logical_height),
        default_action: None,
    }];
    let mut node_indices = HashMap::from([(root_id, 0)]);

    if let Some(contents) = tree.contents() {
        append_subtree(
            tree,
            interaction,
            TraversalFrame {
                figure: contents,
                accessible_parent: root_id,
                depth: 1,
                parent_content_to_surface: Affine2D::IDENTITY,
                parent_visible: true,
                parent_enabled: true,
            },
            &mut nodes,
            &mut node_indices,
        )?;
    }
    let focus = interaction
        .focus_owner()
        .map(AccessibilityNodeId::Figure)
        .filter(|id| nodes.iter().any(|node| node.id == *id));
    Ok(AccessibilitySnapshot {
        revision,
        stable_epoch,
        root: root_id,
        nodes,
        focus,
    })
}

#[derive(Clone, Copy)]
struct TraversalFrame {
    figure: FigureId,
    accessible_parent: AccessibilityNodeId,
    depth: usize,
    parent_content_to_surface: Affine2D,
    parent_visible: bool,
    parent_enabled: bool,
}

fn append_subtree(
    tree: &FigureTree,
    interaction: &InteractionState,
    frame: TraversalFrame,
    nodes: &mut Vec<AccessibilityNode>,
    node_indices: &mut HashMap<AccessibilityNodeId, usize>,
) -> Result<(), AccessibilityError> {
    if frame.depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
        return stacker::maybe_grow(RECURSIVE_STACK_RED_ZONE, RECURSIVE_STACK_GROWTH, || {
            append_subtree_inner(tree, interaction, frame, nodes, node_indices)
        });
    }
    append_subtree_inner(tree, interaction, frame, nodes, node_indices)
}

fn append_subtree_inner(
    tree: &FigureTree,
    interaction: &InteractionState,
    frame: TraversalFrame,
    nodes: &mut Vec<AccessibilityNode>,
    node_indices: &mut HashMap<AccessibilityNodeId, usize>,
) -> Result<(), AccessibilityError> {
    if frame.depth > MAX_TREE_DEPTH {
        return Err(AccessibilityError::DepthLimit);
    }
    let Some(block) = tree.node(frame.figure) else {
        return Ok(());
    };
    let visible = frame.parent_visible && block.state().is_visible();
    if !visible {
        return Ok(());
    }
    let enabled = frame.parent_enabled && block.state().is_enabled();
    if block
        .accessible()
        .is_some_and(|accessible| accessible.accessibility_hidden())
    {
        return Ok(());
    }

    let figure_bounds = block.figure_bounds();
    let local_to_surface = frame.parent_content_to_surface
        * Affine2D::from_translation(figure_bounds.x, figure_bounds.y);
    let child_content_to_surface = local_to_surface * block.child_transform().affine();
    let current_parent = if let Some(accessible) = block.accessible() {
        let id = AccessibilityNodeId::Figure(frame.figure);
        let mut bounds = Rectangle::new(0.0, 0.0, figure_bounds.width, figure_bounds.height);
        bounds.transform(local_to_surface);
        let clickable = block.clickable_snapshot();
        let node = AccessibilityNode {
            id,
            parent: frame.accessible_parent,
            children: Vec::new(),
            name: accessible.accessible_name().unwrap_or_default().to_string(),
            description: accessible.accessible_description().map(str::to_string),
            value: accessible.accessible_value().map(str::to_string),
            role: accessible.accessible_role(),
            state: AccessibilityState {
                enabled,
                focusable: block.state().is_focusable() || block.state().is_focus_traversable(),
                focused: interaction.focus_owner() == Some(frame.figure),
                pressed: clickable.is_some_and(|snapshot| snapshot.visual.pressed),
                selected: clickable.is_some_and(|snapshot| snapshot.selected),
            },
            bounds,
            default_action: accessible.accessible_default_action(),
        };
        let parent_index = node_indices
            .get(&frame.accessible_parent)
            .copied()
            .expect("accessible parent is emitted before its children");
        nodes[parent_index].children.push(id);
        node_indices.insert(id, nodes.len());
        nodes.push(node);
        id
    } else {
        frame.accessible_parent
    };

    if let Some(children) = tree.child_order(frame.figure) {
        for child in children {
            append_subtree(
                tree,
                interaction,
                TraversalFrame {
                    figure: child,
                    accessible_parent: current_parent,
                    depth: frame.depth + 1,
                    parent_content_to_surface: child_content_to_surface,
                    parent_visible: visible,
                    parent_enabled: enabled,
                },
                nodes,
                node_indices,
            )?;
        }
    }
    Ok(())
}

fn same_semantics(previous: &AccessibilitySnapshot, candidate: &AccessibilitySnapshot) -> bool {
    previous.root == candidate.root
        && previous.nodes == candidate.nodes
        && previous.focus == candidate.focus
}

fn build_delta(
    previous: &AccessibilitySnapshot,
    candidate: &AccessibilitySnapshot,
) -> AccessibilityDelta {
    let previous_nodes = previous
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();
    let upserts = candidate
        .nodes
        .iter()
        .filter(|candidate_node| {
            previous_nodes.get(&candidate_node.id).copied() != Some(*candidate_node)
        })
        .cloned()
        .collect();
    let candidate_ids = candidate
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let removed = previous
        .nodes
        .iter()
        .filter(|previous_node| !candidate_ids.contains(&previous_node.id))
        .map(|node| node.id)
        .collect();
    let root_children = candidate
        .nodes
        .first()
        .map(|root| root.children.clone())
        .unwrap_or_default();
    AccessibilityDelta {
        base_revision: previous.revision,
        revision: candidate.revision,
        stable_epoch: candidate.stable_epoch,
        upserts,
        removed,
        root_children,
        focus: candidate.focus,
    }
}
