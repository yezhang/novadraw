//! Standard layered constraints and checked evidence. These are ordinary registry payloads.

use crate::geometry::Rectangle;

use super::{
    GraphAlgorithmCapabilities, GraphCapabilityKey, GraphCapabilityPayload, GraphConstraintKey,
    GraphConstraintOwner, GraphConstraintPayload, GraphEdgeId, GraphEndpoint, GraphGroupId,
    GraphLayoutCandidate, GraphLayoutError, GraphLayoutRequest, GraphLayoutService, GraphNodeId,
    GraphSolveContext, GraphStage,
};

/// Minimum rank distance and weight in the weighted edge-length objective.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayeredEdgeConstraint {
    pub minimum_span: u32,
    pub weight: u32,
}
impl Default for LayeredEdgeConstraint {
    fn default() -> Self {
        Self {
            minimum_span: 1,
            weight: 1,
        }
    }
}
impl GraphConstraintPayload for LayeredEdgeConstraint {}
pub const LAYERED_EDGE: GraphConstraintKey<LayeredEdgeConstraint> =
    GraphConstraintKey::new("layered.edge");

/// Relative cross-axis order within one rank and one sibling containment domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayeredRowConstraint(pub u32);
impl GraphConstraintPayload for LayeredRowConstraint {}
pub const LAYERED_ROW: GraphConstraintKey<LayeredRowConstraint> =
    GraphConstraintKey::new("layered.row");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayeredNodeRank {
    pub node: GraphNodeId,
    pub rank: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayeredGroupRanks {
    pub group: GraphGroupId,
    pub head: u32,
    pub tail: u32,
    /// Independently solved boundary boxes, in consumer coordinates.
    pub head_bounds: Rectangle,
    pub tail_bounds: Rectangle,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayeredVirtualNode {
    pub rank: u32,
    /// Geometry in the requested consumer direction.
    pub bounds: Rectangle,
}
/// Rank endpoints in the acyclic working graph, while `edge` retains original identity.
#[derive(Clone, Debug, PartialEq)]
pub struct LayeredEdgeRanks {
    pub edge: GraphEdgeId,
    pub feedback: bool,
    pub source_rank: u32,
    pub target_rank: u32,
    /// Ordered in original source-to-target direction.
    pub virtual_nodes: Vec<LayeredVirtualNode>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayeredGraphResult {
    pub nodes: Vec<LayeredNodeRank>,
    pub groups: Vec<LayeredGroupRanks>,
    pub edges: Vec<LayeredEdgeRanks>,
}
impl GraphCapabilityPayload for LayeredGraphResult {}
pub const LAYERED_RESULT: GraphCapabilityKey<LayeredGraphResult> =
    GraphCapabilityKey::new("layered.result");

/// Install protocol validators once per service. Algorithms register separately, so consumers
/// can register any number of standard or external algorithms sharing these constraints.
pub fn register_layered_protocol(service: &mut GraphLayoutService) -> Result<(), GraphLayoutError> {
    service.register_constraint(
        LAYERED_EDGE,
        |payload: &LayeredEdgeConstraint,
         owner,
         request: &GraphLayoutRequest,
         context: &mut GraphSolveContext| {
            context.consume(GraphStage::Capability, 1)?;
            let GraphConstraintOwner::Edge(id) = owner else {
                return Err(GraphLayoutError::InvalidPayload(LAYERED_EDGE.name()));
            };
            let edge = request.edge(id)?;
            if edge.source == edge.target && payload.minimum_span > 0 {
                return Err(GraphLayoutError::UnsatisfiedRankConstraint(id));
            }
            Ok(())
        },
    )?;
    service.register_constraint(
        LAYERED_ROW,
        |_: &LayeredRowConstraint,
         owner,
         _: &GraphLayoutRequest,
         context: &mut GraphSolveContext| {
            context.consume(GraphStage::Capability, 1)?;
            if matches!(owner, GraphConstraintOwner::Edge(_)) {
                return Err(GraphLayoutError::InvalidPayload(LAYERED_ROW.name()));
            }
            Ok(())
        },
    )?;
    service.register_capability(LAYERED_RESULT, validate)?;
    Ok(())
}

pub(super) fn capabilities() -> GraphAlgorithmCapabilities {
    GraphAlgorithmCapabilities::new()
        .constraint(LAYERED_EDGE)
        .constraint(LAYERED_ROW)
        .result(LAYERED_RESULT)
}

pub(super) fn edge_constraint(
    request: &GraphLayoutRequest,
    id: GraphEdgeId,
) -> Result<LayeredEdgeConstraint, GraphLayoutError> {
    Ok(request
        .edge(id)?
        .constraints
        .get(LAYERED_EDGE)?
        .copied()
        .unwrap_or_default())
}

fn reject() -> GraphLayoutError {
    GraphLayoutError::CapabilityRejected(LAYERED_RESULT.name())
}

fn validate(
    evidence: &LayeredGraphResult,
    request: &GraphLayoutRequest,
    candidate: &GraphLayoutCandidate,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    if evidence.nodes.len() != request.nodes().len()
        || evidence.groups.len() != request.groups().len()
        || evidence.edges.len() != request.edges().len()
    {
        return Err(reject());
    }
    context.allocate(GraphStage::Capability, evidence.nodes.len() as u64)?;
    let mut nodes = std::collections::HashMap::with_capacity(evidence.nodes.len());
    for node in &evidence.nodes {
        context.consume(GraphStage::Capability, 1)?;
        request.node(node.node)?;
        if nodes.insert(node.node, node.rank).is_some() {
            return Err(reject());
        }
    }
    context.allocate(GraphStage::Capability, evidence.groups.len() as u64)?;
    let mut groups = std::collections::HashMap::with_capacity(evidence.groups.len());
    for group in &evidence.groups {
        context.consume(GraphStage::Capability, 1)?;
        request.group(group.group)?;
        if group.head > group.tail
            || groups
                .insert(group.group, (group.head, group.tail))
                .is_some()
        {
            return Err(reject());
        }
        context.consume(GraphStage::Capability, candidate.placements.len() as u64)?;
        let placement = candidate
            .placements
            .iter()
            .find(|placement| placement.entity == group.group.into())
            .ok_or_else(reject)?;
        let outer = super::directed::transpose_rect(placement.bounds, request.direction());
        let head = super::directed::transpose_rect(group.head_bounds, request.direction());
        let tail = super::directed::transpose_rect(group.tail_bounds, request.direction());
        let border = super::directed::transpose_insets(
            request.group(group.group)?.border_insets,
            request.direction(),
        );
        for boundary in [head, tail] {
            if ![
                boundary.x,
                boundary.y,
                boundary.width,
                boundary.height,
                boundary.x + boundary.width,
                boundary.y + boundary.height,
            ]
            .into_iter()
            .all(f64::is_finite)
                || boundary.width != super::compound::BOUNDARY_CROSS_SIZE
                || boundary.height < 0.0
                || boundary.x < outer.x
                || boundary.x + boundary.width > outer.x + outer.width
                || boundary.y < outer.y
                || boundary.y + boundary.height > outer.y + outer.height
            {
                return Err(reject());
            }
        }
        if head.y != outer.y
            || head.height < border.top
            || outer.height
                != super::compound::enclosing_span(head.y, tail.y + tail.height, context)?
            || tail.height < border.bottom
        {
            return Err(reject());
        }
    }
    for node in request.nodes() {
        context.consume(GraphStage::Capability, 1)?;
        if let Some(parent) = node.parent {
            let (head, tail) = groups[&parent];
            if nodes[&node.id()] < head || nodes[&node.id()] > tail {
                return Err(reject());
            }
        }
    }
    for group in request.groups() {
        context.consume(GraphStage::Capability, 1)?;
        if let Some(parent) = group.parent {
            let (head, tail) = groups[&parent];
            let (child_head, child_tail) = groups[&group.id()];
            if child_head < head || child_tail > tail {
                return Err(reject());
            }
        }
    }
    context.allocate(GraphStage::Capability, evidence.nodes.len() as u64)?;
    let mut bands = std::collections::BTreeMap::new();
    for node in &evidence.nodes {
        context.consume(GraphStage::Capability, candidate.placements.len() as u64)?;
        let bounds = candidate
            .placements
            .iter()
            .find(|placement| placement.entity == node.node.into())
            .ok_or_else(reject)?
            .bounds;
        let bounds = super::directed::transpose_rect(bounds, request.direction());
        let band = (bounds.y, bounds.height);
        if bands
            .insert(node.rank, band)
            .is_some_and(|existing| existing != band)
        {
            return Err(reject());
        }
    }
    let mut previous_end = None;
    for group in &evidence.groups {
        for (rank, bounds) in [
            (group.head, group.head_bounds),
            (group.tail, group.tail_bounds),
        ] {
            context.consume(GraphStage::Capability, 1)?;
            let canonical = super::directed::transpose_rect(bounds, request.direction());
            if bands
                .insert(rank, (canonical.y, canonical.height))
                .is_some_and(|existing| existing != (canonical.y, canonical.height))
            {
                return Err(reject());
            }
        }
    }
    for (y, height) in bands.values() {
        context.consume(GraphStage::Capability, 1)?;
        if previous_end.is_some_and(|end| *y < end) {
            return Err(reject());
        }
        previous_end = Some(y + height);
    }
    if evidence.groups.is_empty() && !nodes.is_empty() && nodes.values().min() != Some(&0) {
        return Err(reject());
    }
    context.allocate(GraphStage::Capability, evidence.edges.len() as u64)?;
    let mut seen = std::collections::HashSet::with_capacity(evidence.edges.len());
    for edge in &evidence.edges {
        context.consume(GraphStage::Capability, 1)?;
        let input = request.edge(edge.edge)?;
        if edge.feedback {
            for (owner, member) in [(input.source, input.target), (input.target, input.source)] {
                if let GraphEndpoint::Group(group) = owner
                    && super::compound::contains(request, group, member, context)?
                {
                    return Err(reject());
                }
            }
        }
        if !seen.insert(edge.edge) {
            return Err(reject());
        }
        let constraint = edge_constraint(request, edge.edge)?;
        if edge
            .target_rank
            .checked_sub(edge.source_rank)
            .is_none_or(|span| span < constraint.minimum_span)
        {
            return Err(GraphLayoutError::UnsatisfiedRankConstraint(edge.edge));
        }
        let (source, target) = if edge.feedback {
            (input.target, input.source)
        } else {
            (input.source, input.target)
        };
        for (entity, rank) in [(source, edge.source_rank), (target, edge.target_rank)] {
            match entity {
                GraphEndpoint::Node(id) if nodes[&id] != rank => return Err(reject()),
                GraphEndpoint::Group(id) => {
                    let (head, tail) = groups[&id];
                    if rank != head && rank != tail {
                        return Err(reject());
                    }
                }
                _ => {}
            }
        }
        context.consume(GraphStage::Capability, candidate.routes.len() as u64)?;
        let route = candidate
            .routes
            .iter()
            .find(|route| route.edge == edge.edge)
            .ok_or_else(reject)?;
        for (is_source, prepared) in [(true, route.source), (false, route.target)] {
            let attachment = request.attachment(input, is_source)?;
            if let GraphEndpoint::Group(id) = prepared.endpoint {
                let working_source = is_source != edge.feedback;
                let selected_head = super::compound::selected_head(
                    request,
                    input,
                    edge.feedback,
                    working_source,
                    context,
                )?;
                let group = evidence
                    .groups
                    .iter()
                    .find(|group| group.group == id)
                    .ok_or_else(reject)?;
                let expected_rank = if selected_head {
                    group.head
                } else {
                    group.tail
                };
                let actual_rank = if working_source {
                    edge.source_rank
                } else {
                    edge.target_rank
                };
                let boundary = if selected_head {
                    group.head_bounds
                } else {
                    group.tail_bounds
                };
                let Some(prepared_boundary) = prepared.boundary else {
                    return Err(reject());
                };
                if actual_rank != expected_rank
                    || prepared_boundary.bounds() != boundary
                    || !prepared_boundary.is_certified_by(LAYERED_RESULT)
                {
                    return Err(reject());
                }
                let canonical = super::directed::transpose_rect(boundary, request.direction());
                let border = super::directed::transpose_insets(
                    request.group(id)?.border_insets,
                    request.direction(),
                );
                let inset = if working_source && selected_head {
                    canonical.height - border.top
                } else {
                    0.0
                };
                let side = match (request.direction(), working_source) {
                    (super::GraphDirection::South, true) => super::AttachmentSide::Bottom,
                    (super::GraphDirection::South, false) => super::AttachmentSide::Top,
                    (super::GraphDirection::East, true) => super::AttachmentSide::Right,
                    (super::GraphDirection::East, false) => super::AttachmentSide::Left,
                };
                if prepared.inset != inset || (attachment.side.is_none() && prepared.side != side) {
                    return Err(reject());
                }
            }
            if matches!(prepared.endpoint, GraphEndpoint::Node(_)) && attachment.side.is_none() {
                let leading = edge.feedback == is_source;
                let expected = match (request.direction(), leading) {
                    (super::GraphDirection::South, true) => super::AttachmentSide::Top,
                    (super::GraphDirection::South, false) => super::AttachmentSide::Bottom,
                    (super::GraphDirection::East, true) => super::AttachmentSide::Left,
                    (super::GraphDirection::East, false) => super::AttachmentSide::Right,
                };
                if prepared.side != expected {
                    return Err(reject());
                }
            }
        }
        let virtual_count = edge
            .target_rank
            .saturating_sub(edge.source_rank)
            .saturating_sub(1);
        if edge.virtual_nodes.len() as u64 != u64::from(virtual_count) {
            return Err(reject());
        }
        for (index, virtual_node) in edge.virtual_nodes.iter().enumerate() {
            context.consume(GraphStage::Capability, 1)?;
            let expected = if edge.feedback {
                u64::from(edge.target_rank) - index as u64 - 1
            } else {
                u64::from(edge.source_rank) + index as u64 + 1
            };
            if u64::from(virtual_node.rank) != expected
                || ![
                    virtual_node.bounds.x,
                    virtual_node.bounds.y,
                    virtual_node.bounds.width,
                    virtual_node.bounds.height,
                ]
                .into_iter()
                .all(f64::is_finite)
                || virtual_node.bounds.width < 0.0
                || virtual_node.bounds.height < 0.0
            {
                return Err(reject());
            }
            let bounds = virtual_node.bounds;
            if bounds.x < candidate.extent.x
                || bounds.y < candidate.extent.y
                || bounds.x + bounds.width > candidate.extent.x + candidate.extent.width
                || bounds.y + bounds.height > candidate.extent.y + candidate.extent.height
            {
                return Err(reject());
            }
            let canonical = super::directed::transpose_rect(bounds, request.direction());
            if bands
                .get(&virtual_node.rank)
                .is_some_and(|band| *band != (canonical.y, canonical.height))
            {
                return Err(reject());
            }
        }
    }
    // Compare constraints in consumer coordinates, including group sibling blocks.
    for (index, left) in candidate.placements.iter().enumerate() {
        let constraints = match left.entity {
            GraphEndpoint::Node(id) => &request.node(id)?.constraints,
            GraphEndpoint::Group(id) => &request.group(id)?.constraints,
        };
        let Some(left_row) = constraints.get(LAYERED_ROW)? else {
            continue;
        };
        for right in &candidate.placements[index + 1..] {
            context.consume(GraphStage::Capability, 1)?;
            if request.parent(left.entity)? != request.parent(right.entity)? {
                continue;
            }
            let constraints = match right.entity {
                GraphEndpoint::Node(id) => &request.node(id)?.constraints,
                GraphEndpoint::Group(id) => &request.group(id)?.constraints,
            };
            let Some(right_row) = constraints.get(LAYERED_ROW)? else {
                continue;
            };
            let interval = |entity| match entity {
                GraphEndpoint::Node(id) => (nodes[&id], nodes[&id]),
                GraphEndpoint::Group(id) => groups[&id],
            };
            let a = interval(left.entity);
            let b = interval(right.entity);
            if a.0 > b.1 || b.0 > a.1 {
                continue;
            }
            let cross = |bounds: Rectangle| match request.direction() {
                super::GraphDirection::South => bounds.x,
                super::GraphDirection::East => bounds.y,
            };
            if (left_row.0 < right_row.0 && cross(left.bounds) >= cross(right.bounds))
                || (left_row.0 > right_row.0 && cross(left.bounds) <= cross(right.bounds))
            {
                return Err(reject());
            }
        }
    }
    Ok(())
}
