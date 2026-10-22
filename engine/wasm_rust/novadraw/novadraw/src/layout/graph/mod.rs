//! Independent, immutable graph layout requests and checked solver results.
//!
//! Graph identities belong to a request builder, never to a Figure tree. Algorithms and
//! additional constraints/results use the same instance-owned typed registry.

mod compound;
mod directed;
mod graph_routing;
mod layered;
mod model;
mod preparation;
mod projection;
mod protocol;
mod ranking;
mod validation;

pub use compound::{COMPOUND_GRAPH, CompoundGraphConfig, register_compound_graph};
pub use directed::{
    DIRECTED_GRAPH, DIRECTED_ORDERING_SWEEPS, DirectedGraphConfig, register_directed_graph,
};
pub use layered::{
    LAYERED_EDGE, LAYERED_RESULT, LAYERED_ROW, LayeredEdgeConstraint, LayeredEdgeRanks,
    LayeredGraphResult, LayeredGroupRanks, LayeredNodeRank, LayeredRowConstraint,
    LayeredVirtualNode, register_layered_protocol,
};
pub use model::{
    AttachmentOffset, AttachmentSide, GraphAttachment, GraphAxisSize, GraphDirection, GraphEdge,
    GraphEdgeId, GraphEndpoint, GraphGroup, GraphGroupId, GraphLayoutRequest, GraphNode,
    GraphNodeId, GraphRequestBuilder, GraphRequestIdentity, GraphSize,
};
pub use preparation::{
    GraphProjection, GraphProjectionMeasurement, GraphProjectionMeasurementResult,
};
pub use projection::{
    GraphProjectionAnchor, GraphProjectionBinding, GraphProjectionRouter, GraphProjectionSnapshot,
};
pub use protocol::{
    GraphAlgorithmCapabilities, GraphAlgorithmKey, GraphAlgorithmPayload, GraphAttachmentBoundary,
    GraphBudget, GraphCapabilityKey, GraphCapabilityPayload, GraphCapabilityValidator,
    GraphConstraintKey, GraphConstraintOwner, GraphConstraintPayload, GraphConstraintValidator,
    GraphConstraints, GraphLayoutAlgorithm, GraphLayoutCandidate, GraphLayoutError,
    GraphLayoutResult, GraphLayoutService, GraphPlacement, GraphResultCapabilities, GraphRoute,
    GraphSolveContext, GraphStage, GraphViolation, PreparedGraphAttachment,
};
