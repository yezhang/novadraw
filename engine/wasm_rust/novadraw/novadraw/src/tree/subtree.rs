//! Owned construction descriptions for one Runtime subtree insertion.

use std::{collections::HashMap, error::Error, fmt};

use uuid::Uuid;

use crate::{Figure, FigureId, FigureStyle, LayoutConstraint, LayoutManager};

/// A construction identity that never denotes a live Runtime Figure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubtreeNodeId {
    owner: Uuid,
    index: usize,
}

/// Failure to address a node in an owned construction description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubtreeNodeError;

impl fmt::Display for SubtreeNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("node does not belong to this subtree")
    }
}

impl Error for SubtreeNodeError {}

pub(crate) struct SubtreeNode {
    pub(crate) parent: Option<usize>,
    pub(crate) figure: Box<dyn Figure>,
    pub(crate) style: Option<FigureStyle>,
    pub(crate) layout: Option<Box<dyn LayoutManager>>,
    pub(crate) constraint: Option<Box<dyn LayoutConstraint>>,
    pub(crate) resources: Vec<crate::render::ResourceId>,
}

/// Owned Connection configuration resolved from construction identities before publication.
///
/// Anchor owners must belong to this description. Router and constraint strategies use the
/// receiving Runtime's ordinary Connection registry and admission rules.
pub struct SubtreeConnection {
    /// Owned source endpoint strategy, or an unbound endpoint.
    pub source: Option<Box<dyn crate::connection::ConnectionAnchor>>,
    /// Owned target endpoint strategy, or an unbound endpoint.
    pub target: Option<Box<dyn crate::connection::ConnectionAnchor>>,
    /// Router identity resolved through the receiving Runtime.
    pub router: crate::connection::RouterBinding,
    /// Optional typed constraint checked by the selected Router.
    pub constraint: Option<Box<dyn crate::connection::RoutingConstraint>>,
}

/// Identity resolution available only while preparing an unpublished description.
pub struct SubtreeBindingNodes(pub(crate) SubtreeReceipt);

impl SubtreeBindingNodes {
    /// Resolves one reserved node; the returned identity is not yet attached.
    pub fn figure(&self, node: SubtreeNodeId) -> Result<FigureId, SubtreeNodeError> {
        self.0.figure(node)
    }
}

/// Construction payload for a node's Connection binding.
///
/// The map contains reserved identities, not published Figures. Preparation must not publish
/// application state; a refused description permanently retires these identities.
pub trait SubtreeConnectionBinding {
    /// Resolves the owned payload without publishing topology or application state.
    fn prepare(
        self: Box<Self>,
        nodes: &SubtreeBindingNodes,
    ) -> Result<SubtreeConnection, crate::runtime::RuntimeMutationError>;
}

impl<F> SubtreeConnectionBinding for F
where
    F: FnOnce(
        &SubtreeBindingNodes,
    ) -> Result<SubtreeConnection, crate::runtime::RuntimeMutationError>,
{
    fn prepare(
        self: Box<Self>,
        nodes: &SubtreeBindingNodes,
    ) -> Result<SubtreeConnection, crate::runtime::RuntimeMutationError> {
        (*self)(nodes)
    }
}

/// A complete owned branch, configured before any Runtime identity is allocated.
///
/// Nodes are added parent first. IDs are scoped to this description, including its receipt.
/// Figure, Border and Layout implementations use the same capabilities as ordinary insertion.
pub struct FigureSubtree {
    pub(crate) owner: Uuid,
    pub(crate) nodes: Vec<SubtreeNode>,
    pub(crate) connections: Vec<(SubtreeNodeId, Box<dyn SubtreeConnectionBinding>)>,
}

impl FigureSubtree {
    /// Starts a description with one root Figure.
    pub fn new(root: Box<dyn Figure>) -> Self {
        Self {
            owner: Uuid::new_v4(),
            connections: Vec::new(),
            nodes: vec![SubtreeNode {
                parent: None,
                figure: root,
                style: None,
                layout: None,
                constraint: None,
                resources: Vec::new(),
            }],
        }
    }

    /// Returns the root's construction identity.
    pub fn root(&self) -> SubtreeNodeId {
        self.id(0)
    }

    fn id(&self, index: usize) -> SubtreeNodeId {
        SubtreeNodeId {
            owner: self.owner,
            index,
        }
    }

    fn index(&self, id: SubtreeNodeId) -> Result<usize, SubtreeNodeError> {
        if id.owner != self.owner || id.index >= self.nodes.len() {
            return Err(SubtreeNodeError);
        }
        Ok(id.index)
    }

    /// Appends a child to a node in this description.
    pub fn add(
        &mut self,
        parent: SubtreeNodeId,
        figure: Box<dyn Figure>,
    ) -> Result<SubtreeNodeId, SubtreeNodeError> {
        let parent = self.index(parent)?;
        let index = self.nodes.len();
        self.nodes.push(SubtreeNode {
            parent: Some(parent),
            figure,
            style: None,
            layout: None,
            constraint: None,
            resources: Vec::new(),
        });
        Ok(self.id(index))
    }

    /// Sets local overrides; `None` fields retain normal inherited style semantics.
    pub fn set_style(
        &mut self,
        node: SubtreeNodeId,
        style: FigureStyle,
    ) -> Result<(), SubtreeNodeError> {
        let index = self.index(node)?;
        self.nodes[index].style = Some(style);
        Ok(())
    }

    /// Installs the node's owned layout strategy.
    pub fn set_layout_manager(
        &mut self,
        node: SubtreeNodeId,
        layout: Box<dyn LayoutManager>,
    ) -> Result<(), SubtreeNodeError> {
        let index = self.index(node)?;
        self.nodes[index].layout = Some(layout);
        Ok(())
    }

    /// Sets the parent-owned layout constraint, including the external parent's root constraint.
    pub fn set_constraint(
        &mut self,
        node: SubtreeNodeId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Result<(), SubtreeNodeError> {
        let index = self.index(node)?;
        self.nodes[index].constraint = Some(constraint);
        Ok(())
    }

    /// Declares a resource dependency checked before any node is published.
    ///
    /// The resource must belong to the receiving Runtime. Pending resources are permitted.
    pub fn add_resource_dependency(
        &mut self,
        node: SubtreeNodeId,
        resource: crate::render::ResourceId,
    ) -> Result<(), SubtreeNodeError> {
        let index = self.index(node)?;
        self.nodes[index].resources.push(resource);
        Ok(())
    }

    /// Sets or replaces a typed binding payload for one Connection node.
    pub fn set_connection_binding(
        &mut self,
        node: SubtreeNodeId,
        binding: impl SubtreeConnectionBinding + 'static,
    ) -> Result<(), SubtreeNodeError> {
        self.index(node)?;
        if let Some((_, payload)) = self.connections.iter_mut().find(|(id, _)| *id == node) {
            *payload = Box::new(binding);
        } else {
            self.connections.push((node, Box::new(binding)));
        }
        Ok(())
    }
}

/// Successful publication of a complete subtree, before the next layout pass.
///
/// This maps construction identities to real Figures. It is not a stable epoch or frame receipt.
#[derive(Debug)]
pub struct SubtreeReceipt {
    root: FigureId,
    nodes: HashMap<SubtreeNodeId, FigureId>,
}

impl SubtreeReceipt {
    pub(crate) fn new(owner: Uuid, ids: Vec<FigureId>) -> Self {
        Self {
            root: ids[0],
            nodes: ids
                .into_iter()
                .enumerate()
                .map(|(index, figure)| (SubtreeNodeId { owner, index }, figure))
                .collect(),
        }
    }

    /// Returns the published branch root.
    pub const fn root(&self) -> FigureId {
        self.root
    }

    /// Resolves an identity from this description; foreign construction IDs remain errors.
    pub fn figure(&self, node: SubtreeNodeId) -> Result<FigureId, SubtreeNodeError> {
        self.nodes.get(&node).copied().ok_or(SubtreeNodeError)
    }
}
