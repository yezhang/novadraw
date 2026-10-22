use uuid::Uuid;

use crate::geometry::{Dimension, Insets};

use super::protocol::{AlgorithmSelection, CapabilityIdentity};
use super::{
    GraphAlgorithmKey, GraphAlgorithmPayload, GraphCapabilityKey, GraphCapabilityPayload,
    GraphConstraints, GraphLayoutError, GraphSolveContext, GraphStage,
};

/// Immutable input identity. A fresh snapshot distinguishes different payload/measurement prefixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphRequestIdentity {
    pub(crate) domain: Uuid,
    pub(crate) snapshot: Uuid,
    pub(crate) revision: u64,
}
impl GraphRequestIdentity {
    /// Consumer-supplied revision of this immutable snapshot.
    pub fn revision(self) -> u64 {
        self.revision
    }
}

macro_rules! identity {
    ($name:ident) => {
        /// Opaque identity allocated in one graph request domain.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name {
            pub(crate) domain: Uuid,
            pub(crate) index: usize,
        }
    };
}
identity!(GraphNodeId);
identity!(GraphGroupId);
identity!(GraphEdgeId);

/// Entity kinds describe topology only; they do not dispatch algorithm behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraphEndpoint {
    /// An ordinary node.
    Node(GraphNodeId),
    /// A containing group, independently of Figure parenting.
    Group(GraphGroupId),
}
impl From<GraphNodeId> for GraphEndpoint {
    fn from(id: GraphNodeId) -> Self {
        Self::Node(id)
    }
}
impl From<GraphGroupId> for GraphEndpoint {
    fn from(id: GraphGroupId) -> Self {
        Self::Group(id)
    }
}

/// Consumer coordinate direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphDirection {
    /// Ranks advance downward.
    South,
    /// Ranks advance to the right.
    East,
}

/// One axis: preferred is an initial, non-shrinkable demand. Fixed axes cannot stretch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphAxisSize {
    /// Lower bound for this axis.
    pub minimum: f64,
    /// Initial demand; algorithms must not shrink below it.
    pub preferred: f64,
    /// Whether placement may enlarge this axis.
    pub allow_stretch: bool,
}
impl GraphAxisSize {
    /// An axis whose exact size must be preserved.
    pub fn fixed(value: f64) -> Self {
        Self {
            minimum: value,
            preferred: value,
            allow_stretch: false,
        }
    }
    /// An axis which may grow from its preferred demand.
    pub fn stretch(minimum: f64, preferred: f64) -> Self {
        Self {
            minimum,
            preferred,
            allow_stretch: true,
        }
    }
}

/// Outer-box requirements expressed separately for both axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphSize {
    /// Horizontal requirement.
    pub width: GraphAxisSize,
    /// Vertical requirement.
    pub height: GraphAxisSize,
}
impl GraphSize {
    /// Exact requirements for both axes.
    pub fn fixed(size: Dimension) -> Self {
        Self {
            width: GraphAxisSize::fixed(size.width),
            height: GraphAxisSize::fixed(size.height),
        }
    }
    /// Draw2D fixture semantics: only the layout's main axis may stretch.
    pub fn directed(size: Dimension, direction: GraphDirection) -> Self {
        let mut result = Self::fixed(size);
        match direction {
            GraphDirection::South => result.height.allow_stretch = true,
            GraphDirection::East => result.width.allow_stretch = true,
        }
        result
    }
    /// Initial outer size.
    pub fn preferred(self) -> Dimension {
        Dimension::new(self.width.preferred, self.height.preferred)
    }
}

/// Side of the endpoint's outer box in consumer coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentSide {
    /// Upper horizontal edge.
    Top,
    /// Lower horizontal edge.
    Bottom,
    /// Left vertical edge.
    Left,
    /// Right vertical edge.
    Right,
}

/// Cross-axis distance along the selected side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachmentOffset {
    /// Half the side length.
    Center,
    /// Distance from the side's leading corner, in the consumer coordinate domain.
    /// Draw2D permits offsets beyond the side length (notably after EAST transposition);
    /// they must remain finite and their resulting endpoints must fit the result extent.
    Distance(f64),
}

/// Optional side lets an algorithm select the endpoint boundary; its capability validator
/// checks that selection. Explicit sides are hard constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphAttachment {
    /// Required side, or algorithm-selected boundary when absent.
    pub side: Option<AttachmentSide>,
    /// Cross-axis position on that side.
    pub offset: AttachmentOffset,
}
impl Default for GraphAttachment {
    fn default() -> Self {
        Self {
            side: None,
            offset: AttachmentOffset::Center,
        }
    }
}

/// Immutable-request node data, editable only while the builder owns it.
#[derive(Clone)]
pub struct GraphNode {
    pub(crate) id: GraphNodeId,
    /// Outer size demand.
    pub size: GraphSize,
    /// External spacing; absent means the request's default.
    pub padding: Option<Insets>,
    /// Optional containing group.
    pub parent: Option<GraphGroupId>,
    /// Incoming edges' default attachment.
    pub incoming: GraphAttachment,
    /// Outgoing edges' default attachment.
    pub outgoing: GraphAttachment,
    /// Open, typed algorithm constraints.
    pub constraints: GraphConstraints,
}
impl GraphNode {
    /// Request-domain identity.
    pub fn id(&self) -> GraphNodeId {
        self.id
    }
}

/// Containment entity which may span multiple ranks.
#[derive(Clone)]
pub struct GraphGroup {
    pub(crate) id: GraphGroupId,
    /// Optional containing group.
    pub parent: Option<GraphGroupId>,
    /// Minimum outer size, including border and inner padding.
    pub minimum: Dimension,
    /// External spacing; absent means the request's default.
    pub padding: Option<Insets>,
    /// Physical border reserved by this group.
    pub border_insets: Insets,
    /// Additional spacing reserved between its border and members.
    pub inner_padding: Insets,
    /// Incoming edges' default attachment.
    pub incoming: GraphAttachment,
    /// Outgoing edges' default attachment.
    pub outgoing: GraphAttachment,
    /// Open, typed algorithm constraints, including optional row ordering.
    pub constraints: GraphConstraints,
}
impl GraphGroup {
    /// Request-domain identity.
    pub fn id(&self) -> GraphGroupId {
        self.id
    }
}

/// Original directed edge. Cycle removal never mutates these endpoints.
#[derive(Clone)]
pub struct GraphEdge {
    pub(crate) id: GraphEdgeId,
    /// Original source.
    pub source: GraphEndpoint,
    /// Original target.
    pub target: GraphEndpoint,
    /// Stroke width used for obstacle clearance.
    pub width: f64,
    /// Extra clearance in addition to half the stroke width.
    pub padding: f64,
    /// Optional override of the source's default.
    pub source_attachment: Option<GraphAttachment>,
    /// Optional override of the target's default.
    pub target_attachment: Option<GraphAttachment>,
    /// Open, typed edge constraints.
    pub constraints: GraphConstraints,
}
impl GraphEdge {
    /// Request-domain identity.
    pub fn id(&self) -> GraphEdgeId {
        self.id
    }
}

/// All source data is frozen. Cloning keeps the same snapshot and authority.
#[derive(Clone)]
pub struct GraphLayoutRequest {
    pub(crate) identity: GraphRequestIdentity,
    pub(crate) nodes: Vec<GraphNode>,
    pub(crate) groups: Vec<GraphGroup>,
    pub(crate) edges: Vec<GraphEdge>,
    pub(crate) direction: GraphDirection,
    pub(crate) padding: Insets,
    pub(crate) margin: Insets,
    pub(crate) algorithm: AlgorithmSelection,
    pub(crate) required: Vec<CapabilityIdentity>,
}
impl GraphLayoutRequest {
    /// Exact immutable source/measurement snapshot.
    pub fn identity(&self) -> GraphRequestIdentity {
        self.identity
    }
    /// Nodes in deterministic builder order.
    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }
    /// Groups in deterministic builder order.
    pub fn groups(&self) -> &[GraphGroup] {
        &self.groups
    }
    /// Edges in deterministic builder order.
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }
    /// Requested coordinate direction.
    pub fn direction(&self) -> GraphDirection {
        self.direction
    }
    /// Default entity spacing.
    pub fn default_padding(&self) -> Insets {
        self.padding
    }
    /// Spacing around top-level entities.
    pub fn margin(&self) -> Insets {
        self.margin
    }
    /// Query an identity, rejecting other request domains.
    pub fn node(&self, id: GraphNodeId) -> Result<&GraphNode, GraphLayoutError> {
        self.nodes
            .get(id.index)
            .filter(|node| node.id == id)
            .ok_or(GraphLayoutError::UnknownEntity(id.into()))
    }
    /// Query an identity, rejecting other request domains.
    pub fn group(&self, id: GraphGroupId) -> Result<&GraphGroup, GraphLayoutError> {
        self.groups
            .get(id.index)
            .filter(|group| group.id == id)
            .ok_or(GraphLayoutError::UnknownEntity(id.into()))
    }
    /// Query an identity, rejecting other request domains.
    pub fn edge(&self, id: GraphEdgeId) -> Result<&GraphEdge, GraphLayoutError> {
        self.edges
            .get(id.index)
            .filter(|edge| edge.id == id)
            .ok_or(GraphLayoutError::UnknownEdge(id))
    }
    pub(crate) fn parent(
        &self,
        entity: GraphEndpoint,
    ) -> Result<Option<GraphGroupId>, GraphLayoutError> {
        match entity {
            GraphEndpoint::Node(id) => Ok(self.node(id)?.parent),
            GraphEndpoint::Group(id) => Ok(self.group(id)?.parent),
        }
    }
    pub(crate) fn entity_padding(&self, entity: GraphEndpoint) -> Result<Insets, GraphLayoutError> {
        Ok(match entity {
            GraphEndpoint::Node(id) => self.node(id)?.padding,
            GraphEndpoint::Group(id) => self.group(id)?.padding,
        }
        .unwrap_or(self.padding))
    }
    pub(crate) fn attachment(
        &self,
        edge: &GraphEdge,
        source: bool,
    ) -> Result<GraphAttachment, GraphLayoutError> {
        let (entity, override_value) = if source {
            (edge.source, edge.source_attachment)
        } else {
            (edge.target, edge.target_attachment)
        };
        if let Some(value) = override_value {
            return Ok(value);
        }
        Ok(match entity {
            GraphEndpoint::Node(id) => {
                let node = self.node(id)?;
                if source { node.outgoing } else { node.incoming }
            }
            GraphEndpoint::Group(id) => {
                let group = self.group(id)?;
                if source {
                    group.outgoing
                } else {
                    group.incoming
                }
            }
        })
    }
}

/// Builder owns graph identities. Each allocation uses the same solve budget as admission,
/// solving, feedback and candidate checking.
pub struct GraphRequestBuilder {
    request: GraphLayoutRequest,
}
impl GraphRequestBuilder {
    /// Start a new immutable prefix while retaining existing graph identities for bindings.
    /// The old request stays unchanged and its checked result cannot certify the edited request.
    pub fn from_request(
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<Self, GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        context.allocate(GraphStage::Request, request.nodes.len() as u64)?;
        context.allocate(GraphStage::Request, request.groups.len() as u64)?;
        context.allocate(GraphStage::Request, request.edges.len() as u64)?;
        context.allocate(GraphStage::Request, request.required.len() as u64)?;
        for constraints in request
            .nodes
            .iter()
            .map(|node| &node.constraints)
            .chain(request.groups.iter().map(|group| &group.constraints))
            .chain(request.edges.iter().map(|edge| &edge.constraints))
        {
            context.consume(GraphStage::Request, 1)?;
            context.allocate(GraphStage::Request, constraints.len() as u64)?;
        }
        let mut request = request.clone();
        request.identity.snapshot = Uuid::new_v4();
        Ok(Self { request })
    }

    /// Create a fresh domain selecting a typed algorithm configuration.
    pub fn new<P: GraphAlgorithmPayload>(
        direction: GraphDirection,
        key: GraphAlgorithmKey<P>,
        payload: P,
        context: &mut GraphSolveContext,
    ) -> Result<Self, GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        Ok(Self {
            request: GraphLayoutRequest {
                identity: GraphRequestIdentity {
                    domain: Uuid::new_v4(),
                    snapshot: Uuid::new_v4(),
                    revision: 0,
                },
                nodes: Vec::new(),
                groups: Vec::new(),
                edges: Vec::new(),
                direction,
                padding: Insets::ZERO,
                margin: Insets::ZERO,
                algorithm: AlgorithmSelection::new(key, payload),
                required: Vec::new(),
            },
        })
    }
    /// Set the consumer's source or measurement revision.
    pub fn revision(&mut self, revision: u64) {
        self.request.identity.revision = revision;
    }
    /// Replace the algorithm configuration in this new prefix.
    pub fn algorithm<P: GraphAlgorithmPayload>(
        &mut self,
        key: GraphAlgorithmKey<P>,
        payload: P,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        self.request.algorithm = AlgorithmSelection::new(key, payload);
        Ok(())
    }
    /// Set fallback external spacing.
    pub fn default_padding(&mut self, padding: Insets) {
        self.request.padding = padding;
    }
    /// Set spacing around top-level entities.
    pub fn margin(&mut self, margin: Insets) {
        self.request.margin = margin;
    }
    /// Require a typed result capability; unsupported requests fail before solving.
    pub fn require<P: GraphCapabilityPayload>(
        &mut self,
        key: GraphCapabilityKey<P>,
        context: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        self.request.required.push(key.identity());
        Ok(())
    }
    /// Allocate an ordinary node.
    pub fn add_node(
        &mut self,
        size: GraphSize,
        context: &mut GraphSolveContext,
    ) -> Result<GraphNodeId, GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        let id = GraphNodeId {
            domain: self.request.identity.domain,
            index: self.request.nodes.len(),
        };
        self.request.nodes.push(GraphNode {
            id,
            size,
            padding: None,
            parent: None,
            incoming: GraphAttachment::default(),
            outgoing: GraphAttachment::default(),
            constraints: GraphConstraints::new(),
        });
        Ok(id)
    }
    /// Allocate a group, including an empty group.
    pub fn add_group(
        &mut self,
        minimum: Dimension,
        context: &mut GraphSolveContext,
    ) -> Result<GraphGroupId, GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        let id = GraphGroupId {
            domain: self.request.identity.domain,
            index: self.request.groups.len(),
        };
        self.request.groups.push(GraphGroup {
            id,
            parent: None,
            minimum,
            padding: None,
            border_insets: Insets::ZERO,
            inner_padding: Insets::ZERO,
            incoming: GraphAttachment::default(),
            outgoing: GraphAttachment::default(),
            constraints: GraphConstraints::new(),
        });
        Ok(id)
    }
    /// Allocate an edge in original source-to-target order.
    pub fn add_edge(
        &mut self,
        source: impl Into<GraphEndpoint>,
        target: impl Into<GraphEndpoint>,
        context: &mut GraphSolveContext,
    ) -> Result<GraphEdgeId, GraphLayoutError> {
        context.allocate(GraphStage::Request, 1)?;
        let id = GraphEdgeId {
            domain: self.request.identity.domain,
            index: self.request.edges.len(),
        };
        self.request.edges.push(GraphEdge {
            id,
            source: source.into(),
            target: target.into(),
            width: 0.0,
            padding: 0.0,
            source_attachment: None,
            target_attachment: None,
            constraints: GraphConstraints::new(),
        });
        Ok(id)
    }
    /// Edit owned source data before freezing the request.
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<&mut GraphNode, GraphLayoutError> {
        self.request
            .nodes
            .get_mut(id.index)
            .filter(|node| node.id == id)
            .ok_or(GraphLayoutError::UnknownEntity(id.into()))
    }
    /// Edit owned source data before freezing the request.
    pub fn group_mut(&mut self, id: GraphGroupId) -> Result<&mut GraphGroup, GraphLayoutError> {
        self.request
            .groups
            .get_mut(id.index)
            .filter(|group| group.id == id)
            .ok_or(GraphLayoutError::UnknownEntity(id.into()))
    }
    /// Edit owned source data before freezing the request.
    pub fn edge_mut(&mut self, id: GraphEdgeId) -> Result<&mut GraphEdge, GraphLayoutError> {
        self.request
            .edges
            .get_mut(id.index)
            .filter(|edge| edge.id == id)
            .ok_or(GraphLayoutError::UnknownEdge(id))
    }
    /// Validate structural input and transfer ownership into an immutable request.
    pub fn build(
        self,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutRequest, GraphLayoutError> {
        super::validation::validate_request(&self.request, context)?;
        Ok(self.request)
    }
}
