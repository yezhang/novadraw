//! Containment-aware working graph, shared separators and independent head/tail geometry.
use crate::geometry::{Insets, Rectangle};

use super::directed::{self, Segment, Vertex, transpose_insets, transpose_rect};
use super::layered::{
    LayeredEdgeRanks, LayeredGraphResult, LayeredGroupRanks, LayeredNodeRank, LayeredVirtualNode,
    edge_constraint,
};
use super::ranking::{self, RankEdge};
use super::*;

/// Draw2D's private head/tail cross-axis width.
pub(super) const BOUNDARY_CROSS_SIZE: f64 = 5.0;
const GROUP_LENGTH_WEIGHT: u32 = 10;
const TERMINAL_WEIGHT: u32 = 2;
const INTERIOR_WEIGHT: u32 = 8;

/// Smallest representable span whose reconstructed trailing boundary covers `end`.
pub(super) fn enclosing_span(
    start: f64,
    end: f64,
    context: &mut GraphSolveContext,
) -> Result<f64, GraphLayoutError> {
    let mut span = end - start;
    while start + span < end {
        context.consume(GraphStage::Placement, 1)?;
        span = span.next_up();
    }
    if !span.is_finite() || span < 0.0 {
        return Err(GraphLayoutError::Overflow(GraphStage::Placement));
    }
    Ok(span)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompoundGraphConfig;
impl GraphAlgorithmPayload for CompoundGraphConfig {}
pub const COMPOUND_GRAPH: GraphAlgorithmKey<CompoundGraphConfig> =
    GraphAlgorithmKey::new("draw2d.compound");

pub fn register_compound_graph(service: &mut GraphLayoutService) -> Result<(), GraphLayoutError> {
    let mut capabilities = super::layered::capabilities();
    capabilities.groups = true;
    service.register(COMPOUND_GRAPH, capabilities, solve)
}

pub(super) fn contains(
    request: &GraphLayoutRequest,
    group: GraphGroupId,
    entity: GraphEndpoint,
    context: &mut GraphSolveContext,
) -> Result<bool, GraphLayoutError> {
    if entity == group.into() {
        return Ok(true);
    }
    let mut parent = request.parent(entity)?;
    while let Some(id) = parent {
        context.consume(GraphStage::Containment, 1)?;
        if id == group {
            return Ok(true);
        }
        parent = request.group(id)?.parent;
    }
    Ok(false)
}

/// Convert source first. A converted group's boundary belongs to that source group,
/// including when the target is one of its ancestors.
pub(super) fn selected_head(
    request: &GraphLayoutRequest,
    edge: &GraphEdge,
    feedback: bool,
    source: bool,
    context: &mut GraphSolveContext,
) -> Result<bool, GraphLayoutError> {
    let (a, b) = if feedback {
        (edge.target, edge.source)
    } else {
        (edge.source, edge.target)
    };
    if source {
        let GraphEndpoint::Group(group) = a else {
            return Ok(false);
        };
        contains(request, group, b, context)
    } else {
        let GraphEndpoint::Group(group) = b else {
            return Ok(false);
        };
        Ok(!contains(request, group, a, context)?)
    }
}

fn index(request: &GraphLayoutRequest, entity: GraphEndpoint) -> usize {
    match entity {
        GraphEndpoint::Node(id) => id.index,
        GraphEndpoint::Group(id) => request.nodes().len() + id.index,
    }
}
fn entity(request: &GraphLayoutRequest, index: usize) -> GraphEndpoint {
    if index < request.nodes().len() {
        request.nodes()[index].id().into()
    } else {
        request.groups()[index - request.nodes().len()].id().into()
    }
}

fn common_parent(
    request: &GraphLayoutRequest,
    a: Option<GraphGroupId>,
    b: Option<GraphGroupId>,
    context: &mut GraphSolveContext,
) -> Result<Option<GraphGroupId>, GraphLayoutError> {
    let mut cursor = a;
    while let Some(group) = cursor {
        let mut other = b;
        while let Some(id) = other {
            context.consume(GraphStage::Containment, 1)?;
            if id == group {
                return Ok(Some(id));
            }
            other = request.group(id)?.parent;
        }
        cursor = request.group(group)?.parent;
    }
    Ok(None)
}

fn block(
    request: &GraphLayoutRequest,
    mut entity: GraphEndpoint,
    parent: Option<GraphGroupId>,
    context: &mut GraphSolveContext,
) -> Result<GraphEndpoint, GraphLayoutError> {
    while request.parent(entity)? != parent {
        context.consume(GraphStage::Containment, 1)?;
        let Some(id) = request.parent(entity)? else {
            return Err(GraphLayoutError::CapabilityRejected(
                "compound sibling domain",
            ));
        };
        entity = id.into();
    }
    Ok(entity)
}

/// Each sibling quotient is ordered as a block. Edges between ancestor and descendant
/// are excluded from reversal; their head/tail conversion expresses containment directly.
fn orientations(
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<Vec<bool>, GraphLayoutError> {
    context.allocate(GraphStage::Algorithm, request.edges().len() as u64)?;
    let mut feedback = vec![false; request.edges().len()];
    for parent in std::iter::once(None).chain(request.groups().iter().map(|group| Some(group.id())))
    {
        context.allocate(
            GraphStage::Algorithm,
            (request.nodes().len() + request.groups().len()) as u64,
        )?;
        let mut siblings = Vec::new();
        for id in 0..request.nodes().len() + request.groups().len() {
            context.consume(GraphStage::Algorithm, 1)?;
            let value = entity(request, id);
            if request.parent(value)? == parent {
                siblings.push(value);
            }
        }
        context.allocate(GraphStage::Algorithm, request.edges().len() as u64 * 2)?;
        let mut edges = Vec::new();
        let mut identities = Vec::new();
        for edge in request.edges() {
            let a_parent = request.parent(edge.source)?;
            let b_parent = request.parent(edge.target)?;
            if common_parent(request, a_parent, b_parent, context)? != parent {
                continue;
            }
            if let GraphEndpoint::Group(group) = edge.source
                && contains(request, group, edge.target, context)?
            {
                continue;
            }
            if let GraphEndpoint::Group(group) = edge.target
                && contains(request, group, edge.source, context)?
            {
                continue;
            }
            let a = block(request, edge.source, parent, context)?;
            let b = block(request, edge.target, parent, context)?;
            context.consume(GraphStage::Algorithm, siblings.len() as u64 * 2)?;
            let source = siblings.iter().position(|value| *value == a).unwrap();
            let target = siblings.iter().position(|value| *value == b).unwrap();
            edges.push(RankEdge {
                source,
                target,
                span: 1.0,
                weight: 1,
            });
            identities.push(edge.id().index);
        }
        for (id, inverted) in
            identities
                .into_iter()
                .zip(directed::break_cycles(siblings.len(), &mut edges, context)?)
        {
            feedback[id] = inverted;
        }
    }
    Ok(feedback)
}

fn boundary_index(request: &GraphLayoutRequest, group: GraphGroupId, head: bool) -> usize {
    request.nodes().len() + group.index * 2 + usize::from(!head)
}

fn working_endpoint(
    request: &GraphLayoutRequest,
    edge: &GraphEdge,
    feedback: bool,
    source: bool,
    context: &mut GraphSolveContext,
) -> Result<usize, GraphLayoutError> {
    let value = if source != feedback {
        edge.source
    } else {
        edge.target
    };
    Ok(match value {
        GraphEndpoint::Node(id) => id.index,
        GraphEndpoint::Group(id) => boundary_index(
            request,
            id,
            selected_head(request, edge, feedback, source, context)?,
        ),
    })
}

fn solve(
    _: &CompoundGraphConfig,
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<GraphLayoutCandidate, GraphLayoutError> {
    let mut candidate = GraphLayoutCandidate::new(request);
    let mut evidence = LayeredGraphResult::default();
    let count = request
        .groups()
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(request.nodes().len()))
        .ok_or(GraphLayoutError::Overflow(GraphStage::Algorithm))?;
    if count == 0 {
        candidate
            .capabilities
            .insert(LAYERED_RESULT, evidence, context)?;
        return Ok(candidate);
    }
    let feedback = orientations(request, context)?;
    context.allocate(
        GraphStage::Ranking,
        request.edges().len() as u64 + count as u64 * 2,
    )?;
    let mut edges = Vec::new();
    for (i, edge) in request.edges().iter().enumerate() {
        let constraint = edge_constraint(request, edge.id())?;
        edges.push(RankEdge {
            source: working_endpoint(request, edge, feedback[i], true, context)?,
            target: working_endpoint(request, edge, feedback[i], false, context)?,
            span: f64::from(constraint.minimum_span),
            weight: constraint.weight,
        });
    }
    for group in request.groups() {
        let head = boundary_index(request, group.id(), true);
        let tail = head + 1;
        edges.push(RankEdge {
            source: head,
            target: tail,
            span: 1.0,
            weight: GROUP_LENGTH_WEIGHT,
        });
        if let Some(parent) = group.parent {
            edges.push(RankEdge {
                source: boundary_index(request, parent, true),
                target: head,
                span: 1.0,
                weight: 0,
            });
            edges.push(RankEdge {
                source: tail,
                target: boundary_index(request, parent, false),
                span: 1.0,
                weight: 0,
            });
        }
    }
    for node in request.nodes() {
        if let Some(parent) = node.parent {
            edges.push(RankEdge {
                source: boundary_index(request, parent, true),
                target: node.id().index,
                span: 1.0,
                weight: 0,
            });
            edges.push(RankEdge {
                source: node.id().index,
                target: boundary_index(request, parent, false),
                span: 1.0,
                weight: 0,
            });
        }
    }
    let ranks = ranking::solve(count, &edges, context)?;
    context.allocate(GraphStage::Placement, count as u64 * 2)?;
    let mut vertices = Vec::with_capacity(count);
    let mut parents = Vec::with_capacity(count);
    for node in request.nodes() {
        let size = node.size.preferred();
        vertices.push(Vertex {
            bounds: transpose_rect(
                Rectangle::new(0.0, 0.0, size.width, size.height),
                request.direction(),
            ),
            padding: transpose_insets(
                request.entity_padding(node.id().into())?,
                request.direction(),
            ),
            rank: directed::rank_value(ranks[node.id().index])?,
            row: node.constraints.get(LAYERED_ROW)?.map(|row| row.0),
        });
        parents.push(node.parent);
    }
    for group in request.groups() {
        let border = transpose_insets(group.border_insets, request.direction());
        let inner = transpose_insets(group.inner_padding, request.direction());
        let padding = transpose_insets(
            request.entity_padding(group.id().into())?,
            request.direction(),
        );
        for head in [true, false] {
            let id = boundary_index(request, group.id(), head);
            vertices.push(Vertex {
                bounds: Rectangle::new(
                    0.0,
                    0.0,
                    BOUNDARY_CROSS_SIZE,
                    if head { border.top } else { border.bottom },
                ),
                padding: if head {
                    Insets::new(padding.top, 0.0, inner.top, 0.0)
                } else {
                    Insets::new(inner.bottom, 0.0, padding.bottom, 0.0)
                },
                rank: directed::rank_value(ranks[id])?,
                row: None,
            });
            parents.push(Some(group.id()));
        }
    }
    let clearance = request
        .edges()
        .iter()
        .map(|edge| edge.padding + edge.width / 2.0)
        .fold(0.0_f64, f64::max);
    context.allocate(GraphStage::Ordering, request.edges().len() as u64)?;
    let mut chains = Vec::new();
    let mut segments = Vec::new();
    for (i, input) in request.edges().iter().enumerate() {
        let edge = edges[i];
        let start = vertices[edge.source].rank;
        let end = vertices[edge.target].rank;
        let auxiliary = end.saturating_sub(start).saturating_sub(1) as u64;
        context.allocate(GraphStage::Ordering, auxiliary * 4 + 3)?;
        let parent = common_parent(request, parents[edge.source], parents[edge.target], context)?;
        let mut chain = vec![edge.source];
        for rank in u64::from(start) + 1..u64::from(end) {
            chain.push(vertices.len());
            vertices.push(Vertex {
                bounds: Rectangle::new(0.0, 0.0, input.width, 0.0),
                padding: Insets::new(0.0, input.padding, 0.0, input.padding),
                rank: rank as u32,
                row: None,
            });
            parents.push(parent);
        }
        chain.push(edge.target);
        for (position, pair) in chain.windows(2).enumerate() {
            let offset = |attachment: GraphAttachment, width| match attachment.offset {
                AttachmentOffset::Center => width / 2.0,
                AttachmentOffset::Distance(value) => value,
            };
            let factor = if chain.len() == 2 {
                1
            } else if position == 0 || position + 2 == chain.len() {
                TERMINAL_WEIGHT
            } else {
                INTERIOR_WEIGHT
            };
            segments.push(Segment {
                source: pair[0],
                target: pair[1],
                source_offset: if position == 0 {
                    offset(
                        request.attachment(input, !feedback[i])?,
                        vertices[pair[0]].bounds.width,
                    )
                } else {
                    vertices[pair[0]].bounds.width / 2.0
                },
                target_offset: if position + 2 == chain.len() {
                    offset(
                        request.attachment(input, feedback[i])?,
                        vertices[pair[1]].bounds.width,
                    )
                } else {
                    vertices[pair[1]].bounds.width / 2.0
                },
                weight: edge
                    .weight
                    .checked_mul(factor)
                    .ok_or(GraphLayoutError::Overflow(GraphStage::Ordering))?,
            });
        }
        chains.push(chain);
    }
    // Every empty group rank has a private bridge, so a nonmember can never occupy its region.
    for group in request.groups() {
        let head = vertices[boundary_index(request, group.id(), true)].rank;
        let tail = vertices[boundary_index(request, group.id(), false)].rank;
        for rank in u64::from(head)..=u64::from(tail) {
            let mut occupied = false;
            for (vertex, parent) in vertices.iter().zip(&parents) {
                context.consume(GraphStage::Ordering, 1)?;
                if vertex.rank == rank as u32
                    && let Some(parent) = parent
                    && contains(request, group.id(), (*parent).into(), context)?
                {
                    occupied = true;
                    break;
                }
            }
            if !occupied {
                context.allocate(GraphStage::Ordering, 2)?;
                vertices.push(Vertex {
                    bounds: Rectangle::ZERO,
                    padding: Insets::ZERO,
                    rank: rank as u32,
                    row: None,
                });
                parents.push(Some(group.id()));
            }
        }
    }
    let mut rows = directed::populate_rows(&vertices, context)?;
    order_blocks(request, &mut rows, &vertices, &parents, &segments, context)?;
    directed::place_rows(&rows, &mut vertices, request, clearance, context)?;
    // Group minimum heights are constraints on the rank geometry before horizontal placement.
    for group in request.groups() {
        let head = boundary_index(request, group.id(), true);
        let tail = head + 1;
        let minimum = transpose_rect(
            Rectangle::new(0.0, 0.0, group.minimum.width, group.minimum.height),
            request.direction(),
        )
        .height;
        let actual =
            vertices[tail].bounds.y + vertices[tail].bounds.height - vertices[head].bounds.y;
        if actual < minimum {
            let delta = minimum - actual;
            let rank = vertices[tail].rank;
            for vertex in &mut vertices {
                context.consume(GraphStage::Placement, 1)?;
                if vertex.rank >= rank {
                    vertex.bounds.y += delta;
                }
            }
        }
    }
    place_horizontal(
        request,
        &rows,
        &mut vertices,
        &parents,
        &segments,
        clearance,
        context,
    )?;
    context.allocate(
        GraphStage::Result,
        (request.nodes().len() + request.groups().len()) as u64 * 2,
    )?;
    for node in request.nodes() {
        let vertex = &vertices[node.id().index];
        candidate.placements.push(GraphPlacement {
            entity: node.id().into(),
            bounds: transpose_rect(vertex.bounds, request.direction()),
        });
        evidence.nodes.push(LayeredNodeRank {
            node: node.id(),
            rank: vertex.rank,
        });
    }
    for group in request.groups() {
        let head = &vertices[boundary_index(request, group.id(), true)];
        let tail = &vertices[boundary_index(request, group.id(), false)];
        let left = &vertices[count_left(&vertices, request, group.id())];
        let right = &vertices[count_left(&vertices, request, group.id()) + 1];
        let bounds = Rectangle::new(
            left.bounds.x,
            head.bounds.y,
            enclosing_span(left.bounds.x, right.bounds.x, context)?,
            enclosing_span(head.bounds.y, tail.bounds.y + tail.bounds.height, context)?,
        );
        candidate.placements.push(GraphPlacement {
            entity: group.id().into(),
            bounds: transpose_rect(bounds, request.direction()),
        });
        evidence.groups.push(LayeredGroupRanks {
            group: group.id(),
            head: head.rank,
            tail: tail.rank,
            head_bounds: transpose_rect(head.bounds, request.direction()),
            tail_bounds: transpose_rect(tail.bounds, request.direction()),
        });
    }
    for (i, edge) in request.edges().iter().enumerate() {
        let prepare = |original_source: bool,
                       context: &mut GraphSolveContext|
         -> Result<PreparedGraphAttachment, GraphLayoutError> {
            let working_source = original_source != feedback[i];
            let id = if working_source {
                edges[i].source
            } else {
                edges[i].target
            };
            let endpoint = if original_source {
                edge.source
            } else {
                edge.target
            };
            let attachment = request.attachment(edge, original_source)?;
            let side = attachment
                .side
                .unwrap_or(match (request.direction(), working_source) {
                    (GraphDirection::South, true) => AttachmentSide::Bottom,
                    (GraphDirection::South, false) => AttachmentSide::Top,
                    (GraphDirection::East, true) => AttachmentSide::Right,
                    (GraphDirection::East, false) => AttachmentSide::Left,
                });
            let bounds = transpose_rect(vertices[id].bounds, request.direction());
            Ok(match endpoint {
                GraphEndpoint::Node(_) => {
                    PreparedGraphAttachment::on_bounds(endpoint, bounds, side, attachment, 0.0)
                }
                GraphEndpoint::Group(group) => {
                    let head = selected_head(request, edge, feedback[i], working_source, context)?;
                    let border =
                        transpose_insets(request.group(group)?.border_insets, request.direction());
                    let inset = if working_source && head {
                        vertices[id].bounds.height - border.top
                    } else {
                        0.0
                    };
                    PreparedGraphAttachment::on_boundary(
                        endpoint,
                        GraphAttachmentBoundary::new(bounds, LAYERED_RESULT),
                        side,
                        attachment,
                        inset,
                    )
                }
            })
        };
        let source = prepare(true, context)?;
        let target = prepare(false, context)?;
        let points = super::graph_routing::route_directed(
            request,
            &candidate.placements,
            edge,
            source,
            target,
            &vertices,
            &rows,
            &chains[i],
            feedback[i],
            context,
        )?;
        context.allocate(
            GraphStage::Result,
            2 + chains[i].len().saturating_sub(2) as u64,
        )?;
        candidate.routes.push(GraphRoute {
            edge: edge.id(),
            source,
            target,
            points,
        });
        let mut virtual_nodes: Vec<_> = chains[i][1..chains[i].len() - 1]
            .iter()
            .map(|id| LayeredVirtualNode {
                rank: vertices[*id].rank,
                bounds: transpose_rect(vertices[*id].bounds, request.direction()),
            })
            .collect();
        if feedback[i] {
            virtual_nodes.reverse();
        }
        evidence.edges.push(LayeredEdgeRanks {
            edge: edge.id(),
            feedback: feedback[i],
            source_rank: vertices[edges[i].source].rank,
            target_rank: vertices[edges[i].target].rank,
            virtual_nodes,
        });
    }
    candidate.extent = super::graph_routing::extent(request, &candidate, context)?;
    for edge in &evidence.edges {
        for vertex in &edge.virtual_nodes {
            context.consume(GraphStage::Result, 1)?;
            candidate.extent = super::graph_routing::union(candidate.extent, vertex.bounds);
        }
    }
    candidate
        .capabilities
        .insert(LAYERED_RESULT, evidence, context)?;
    Ok(candidate)
}

fn count_left(vertices: &[Vertex], request: &GraphLayoutRequest, group: GraphGroupId) -> usize {
    vertices.len() - request.groups().len() * 2 + group.index * 2
}

/// Sort nested sibling blocks from their aggregate connectivity. The same group block order
/// across ranks makes shared left/right separator constraints jointly feasible.
fn order_blocks(
    request: &GraphLayoutRequest,
    rows: &mut [Vec<usize>],
    vertices: &[Vertex],
    parents: &[Option<GraphGroupId>],
    segments: &[Segment],
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    context.allocate(GraphStage::Ordering, vertices.len() as u64 * 3)?;
    let mut paths = vec![Vec::new(); vertices.len()];
    let mut leaf_scores = vec![0.0; vertices.len()];
    for (id, parent) in parents.iter().enumerate() {
        let mut lineage = Vec::new();
        let mut cursor = *parent;
        while let Some(group) = cursor {
            context.allocate(GraphStage::Ordering, 1)?;
            lineage.push(index(request, group.into()));
            cursor = request.group(group)?.parent;
        }
        lineage.reverse();
        context.allocate(GraphStage::Ordering, lineage.len() as u64 + 1)?;
        lineage.push(id);
        paths[id] = lineage;
    }
    // Start from flat median/local optimization, then aggregate its cross-axis scores
    // within each sibling domain. Node constraints remain local to their parent.
    directed::order_rows(
        rows,
        vertices,
        segments,
        directed::DIRECTED_ORDERING_SWEEPS,
        context,
    )?;
    context.allocate(
        GraphStage::Ordering,
        (request.nodes().len() + request.groups().len()) as u64 * 2,
    )?;
    let mut scores = vec![(0.0, 0_u64); request.nodes().len() + request.groups().len()];
    for row in rows.iter() {
        for (position, id) in row.iter().enumerate() {
            leaf_scores[*id] = position as f64;
            for block in &paths[*id][..paths[*id].len() - 1] {
                context.consume(GraphStage::Ordering, 1)?;
                scores[*block].0 += position as f64;
                scores[*block].1 += 1;
            }
        }
    }
    for row in rows {
        context.consume(GraphStage::Ordering, row.len() as u64)?;
        let depth = row.iter().map(|id| paths[*id].len()).max().unwrap_or(0);
        let work = (row.len() as u64)
            .checked_mul(row.len() as u64)
            .and_then(|value| value.checked_mul(depth as u64 + 1))
            .ok_or(GraphLayoutError::Overflow(GraphStage::Ordering))?;
        context.consume(GraphStage::Ordering, work)?;
        row.sort_by(|a, b| {
            let pa = &paths[*a];
            let pb = &paths[*b];
            for (depth, (x, y)) in pa.iter().zip(pb).enumerate() {
                if x == y {
                    continue;
                }
                let key = |id: usize, path: &[usize]| {
                    if depth + 1 == path.len() {
                        (vertices[id].row, leaf_scores[id])
                    } else {
                        let GraphEndpoint::Group(group) = entity(request, id) else {
                            unreachable!()
                        };
                        let row = request
                            .group(group)
                            .unwrap()
                            .constraints
                            .get(LAYERED_ROW)
                            .unwrap()
                            .map(|row| row.0);
                        let (score, count) = scores[id];
                        (
                            row,
                            if count == 0 {
                                id as f64
                            } else {
                                score / count as f64
                            },
                        )
                    }
                };
                let (xr, xs) = key(*x, pa);
                let (yr, ys) = key(*y, pb);
                return match (xr, yr) {
                    (Some(x), Some(y)) if x != y => x.cmp(&y),
                    _ => xs.total_cmp(&ys).then(x.cmp(y)),
                };
            }
            pa.len().cmp(&pb.len())
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn place_horizontal(
    request: &GraphLayoutRequest,
    rows: &[Vec<usize>],
    vertices: &mut Vec<Vertex>,
    parents: &[Option<GraphGroupId>],
    segments: &[Segment],
    clearance: f64,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    let original = vertices.len();
    context.allocate(GraphStage::Placement, request.groups().len() as u64 * 2)?;
    for _ in request.groups() {
        vertices.extend([
            Vertex {
                bounds: Rectangle::ZERO,
                padding: Insets::ZERO,
                rank: 0,
                row: None,
            },
            Vertex {
                bounds: Rectangle::ZERO,
                padding: Insets::ZERO,
                rank: 0,
                row: None,
            },
        ]);
    }
    let global_left = vertices.len();
    let global_right = global_left + 1;
    let count = global_right
        .checked_add(1)
        .and_then(|value| value.checked_add(segments.len()))
        .ok_or(GraphLayoutError::Overflow(GraphStage::Placement))?;
    let mut constraints = Vec::new();
    let mut push = |source,
                    target,
                    span,
                    weight,
                    context: &mut GraphSolveContext|
     -> Result<(), GraphLayoutError> {
        context.allocate(GraphStage::Placement, 1)?;
        constraints.push(RankEdge {
            source,
            target,
            span,
            weight,
        });
        Ok(())
    };
    for group in request.groups() {
        let left = original + group.id().index * 2;
        let right = left + 1;
        let border = transpose_insets(group.border_insets, request.direction());
        let inner = transpose_insets(group.inner_padding, request.direction());
        let size = transpose_rect(
            Rectangle::new(0.0, 0.0, group.minimum.width, group.minimum.height),
            request.direction(),
        );
        push(
            left,
            right,
            size.width
                .max(border.width() + inner.width())
                .max(BOUNDARY_CROSS_SIZE),
            0,
            context,
        )?;
    }
    for id in 0..original {
        if let Some(parent) = parents[id] {
            let group = request.group(parent)?;
            let left = original + parent.index * 2;
            let right = left + 1;
            let border = transpose_insets(group.border_insets, request.direction());
            let inner = transpose_insets(group.inner_padding, request.direction());
            let is_boundary = id == boundary_index(request, parent, true)
                || id == boundary_index(request, parent, false);
            let leading = if is_boundary {
                0.0
            } else {
                border.left + inner.left + vertices[id].padding.left + clearance
            };
            let trailing = if is_boundary {
                0.0
            } else {
                border.right + inner.right + vertices[id].padding.right + clearance
            };
            push(left, id, leading, u32::from(is_boundary), context)?;
            push(
                id,
                right,
                vertices[id].bounds.width + trailing,
                u32::from(is_boundary),
                context,
            )?;
        }
    }
    for group in request.groups() {
        if let Some(parent) = group.parent {
            let outer = request.group(parent)?;
            let border = transpose_insets(outer.border_insets, request.direction());
            let inner = transpose_insets(outer.inner_padding, request.direction());
            let padding = transpose_insets(
                request.entity_padding(group.id().into())?,
                request.direction(),
            );
            push(
                original + parent.index * 2,
                original + group.id().index * 2,
                border.left + inner.left + padding.left + clearance,
                0,
                context,
            )?;
            push(
                original + group.id().index * 2 + 1,
                original + parent.index * 2 + 1,
                border.right + inner.right + padding.right + clearance,
                0,
                context,
            )?;
        }
    }
    let outer = |id: usize,
                 common: Option<GraphGroupId>,
                 right: bool,
                 context: &mut GraphSolveContext|
     -> Result<(usize, f64), GraphLayoutError> {
        let mut parent = parents[id];
        let mut selected = None;
        while parent != common {
            context.consume(GraphStage::Containment, 1)?;
            let Some(group) = parent else {
                break;
            };
            selected = Some(group);
            parent = request.group(group)?.parent;
        }
        if let Some(group) = selected {
            let padding =
                transpose_insets(request.entity_padding(group.into())?, request.direction());
            Ok((
                original + group.index * 2 + usize::from(right),
                if right { padding.right } else { padding.left },
            ))
        } else {
            Ok((
                id,
                if right {
                    vertices[id].bounds.width + vertices[id].padding.right
                } else {
                    vertices[id].padding.left
                },
            ))
        }
    };
    let margin = transpose_insets(request.margin(), request.direction());
    for row in rows {
        if let Some(first) = row.first() {
            let (id, pad) = outer(*first, None, false, context)?;
            push(global_left, id, margin.left + pad + clearance, 0, context)?;
        }
        for pair in row.windows(2) {
            let common = common_parent(request, parents[pair[0]], parents[pair[1]], context)?;
            let (left, pad_right) = outer(pair[0], common, true, context)?;
            let (right, pad_left) = outer(pair[1], common, false, context)?;
            push(
                left,
                right,
                pad_right + pad_left + clearance * 2.0,
                0,
                context,
            )?;
        }
        if let Some(last) = row.last() {
            let (id, pad) = outer(*last, None, true, context)?;
            push(id, global_right, margin.right + pad + clearance, 0, context)?;
        }
    }
    for (i, segment) in segments.iter().enumerate() {
        let auxiliary = global_right + 1 + i;
        let delta = segment.source_offset - segment.target_offset;
        push(
            auxiliary,
            segment.source,
            (-delta).max(0.0),
            segment.weight,
            context,
        )?;
        push(
            auxiliary,
            segment.target,
            delta.max(0.0),
            segment.weight,
            context,
        )?;
    }
    let ranks = ranking::solve(count, &constraints, context)?;
    for (id, vertex) in vertices.iter_mut().enumerate() {
        context.consume(GraphStage::Placement, 1)?;
        vertex.bounds.x = ranks[id] - ranks[global_left];
    }
    Ok(())
}
