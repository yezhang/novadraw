use crate::geometry::{Insets, Point, Rectangle};

use super::layered::{
    LayeredEdgeRanks, LayeredGraphResult, LayeredNodeRank, LayeredVirtualNode, edge_constraint,
};
use super::ranking::{self, RankEdge};
use super::{
    AttachmentOffset, AttachmentSide, GraphAlgorithmKey, GraphAlgorithmPayload, GraphDirection,
    GraphEndpoint, GraphLayoutCandidate, GraphLayoutError, GraphLayoutRequest, GraphLayoutService,
    GraphPlacement, GraphRoute, GraphSolveContext, GraphStage, LAYERED_RESULT, LAYERED_ROW,
    PreparedGraphAttachment,
};

/// Draw2D performs 45 bidirectional ordering sweeps.
pub const DIRECTED_ORDERING_SWEEPS: u32 = 45;
const LONG_EDGE_TERMINAL_WEIGHT: u32 = 2;
const LONG_EDGE_INTERIOR_WEIGHT: u32 = 8;

#[derive(Clone, Copy, Debug)]
pub struct DirectedGraphConfig {
    pub ordering_sweeps: u32,
}
impl Default for DirectedGraphConfig {
    fn default() -> Self {
        Self {
            ordering_sweeps: DIRECTED_ORDERING_SWEEPS,
        }
    }
}
impl GraphAlgorithmPayload for DirectedGraphConfig {}
pub const DIRECTED_GRAPH: GraphAlgorithmKey<DirectedGraphConfig> =
    GraphAlgorithmKey::new("draw2d.directed");

/// Uses the same registry as consumer algorithms. Install the layered protocol first.
pub fn register_directed_graph(service: &mut GraphLayoutService) -> Result<(), GraphLayoutError> {
    service.register(DIRECTED_GRAPH, super::layered::capabilities(), solve)
}

#[derive(Clone, Debug)]
pub(super) struct Vertex {
    pub bounds: Rectangle,
    pub padding: Insets,
    pub rank: u32,
    pub row: Option<u32>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Segment {
    pub source: usize,
    pub target: usize,
    pub source_offset: f64,
    pub target_offset: f64,
    pub weight: u32,
}

pub(super) fn transpose_rect(bounds: Rectangle, direction: GraphDirection) -> Rectangle {
    match direction {
        GraphDirection::South => bounds,
        GraphDirection::East => Rectangle::new(bounds.y, bounds.x, bounds.height, bounds.width),
    }
}
pub(super) fn transpose_point(point: Point, direction: GraphDirection) -> Point {
    match direction {
        GraphDirection::South => point,
        GraphDirection::East => Point::new(point.y(), point.x()),
    }
}
pub(super) fn transpose_insets(insets: Insets, direction: GraphDirection) -> Insets {
    match direction {
        GraphDirection::South => insets,
        GraphDirection::East => Insets::new(insets.left, insets.top, insets.right, insets.bottom),
    }
}
fn offset(value: AttachmentOffset, width: f64) -> f64 {
    match value {
        AttachmentOffset::Center => width / 2.0,
        AttachmentOffset::Distance(value) => value,
    }
}

fn solve(
    config: &DirectedGraphConfig,
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<GraphLayoutCandidate, GraphLayoutError> {
    let mut candidate = GraphLayoutCandidate::new(request);
    let mut evidence = LayeredGraphResult::default();
    if request.nodes().is_empty() {
        candidate
            .capabilities
            .insert(LAYERED_RESULT, evidence, context)?;
        return Ok(candidate);
    }
    context.allocate(GraphStage::Algorithm, request.edges().len() as u64)?;
    let mut edges = Vec::with_capacity(request.edges().len());
    for edge in request.edges() {
        let (GraphEndpoint::Node(source), GraphEndpoint::Node(target)) = (edge.source, edge.target)
        else {
            return Err(GraphLayoutError::UnsupportedGroups);
        };
        let constraint = edge_constraint(request, edge.id())?;
        edges.push(RankEdge {
            source: source.index,
            target: target.index,
            span: f64::from(constraint.minimum_span),
            weight: constraint.weight,
        });
    }
    let feedback = break_cycles(request.nodes().len(), &mut edges, context)?;
    let ranks = ranking::solve(request.nodes().len(), &edges, context)?;
    context.allocate(GraphStage::Placement, request.nodes().len() as u64)?;
    let mut vertices = Vec::with_capacity(request.nodes().len());
    let clearance = request
        .edges()
        .iter()
        .map(|edge| edge.padding + edge.width / 2.0)
        .fold(0.0_f64, f64::max);
    for (node, rank) in request.nodes().iter().zip(ranks) {
        let size = node.size.preferred();
        let bounds = transpose_rect(
            Rectangle::new(0.0, 0.0, size.width, size.height),
            request.direction(),
        );
        vertices.push(Vertex {
            bounds,
            padding: transpose_insets(
                node.padding.unwrap_or(request.default_padding()),
                request.direction(),
            ),
            rank: rank_value(rank)?,
            row: node.constraints.get(LAYERED_ROW)?.map(|row| row.0),
        });
    }
    context.allocate(GraphStage::Ordering, request.edges().len() as u64)?;
    let mut chains = Vec::with_capacity(edges.len());
    let mut segments = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        let input = &request.edges()[index];
        let source_attachment = request.attachment(input, !feedback[index])?;
        let target_attachment = request.attachment(input, feedback[index])?;
        let source_offset = offset(source_attachment.offset, vertices[edge.source].bounds.width);
        let target_offset = offset(target_attachment.offset, vertices[edge.target].bounds.width);
        let span = vertices[edge.target].rank - vertices[edge.source].rank;
        let auxiliary = span.saturating_sub(1) as u64;
        context.allocate(GraphStage::Ordering, auxiliary + 2)?;
        context.allocate(GraphStage::Ordering, auxiliary)?;
        context.allocate(GraphStage::Ordering, auxiliary + 1)?;
        let mut chain = Vec::with_capacity(auxiliary as usize + 2);
        chain.push(edge.source);
        for rank in u64::from(vertices[edge.source].rank) + 1..u64::from(vertices[edge.target].rank)
        {
            chain.push(vertices.len());
            vertices.push(Vertex {
                bounds: Rectangle::new(0.0, 0.0, input.width, 0.0),
                padding: Insets::new(0.0, input.padding, 0.0, input.padding),
                rank: rank as u32,
                row: None,
            });
        }
        chain.push(edge.target);
        for (position, pair) in chain.windows(2).enumerate() {
            let factor = if chain.len() == 2 {
                1
            } else if position == 0 || position + 2 == chain.len() {
                LONG_EDGE_TERMINAL_WEIGHT
            } else {
                LONG_EDGE_INTERIOR_WEIGHT
            };
            segments.push(Segment {
                source: pair[0],
                target: pair[1],
                source_offset: if position == 0 {
                    source_offset
                } else {
                    vertices[pair[0]].bounds.width / 2.0
                },
                target_offset: if position + 2 == chain.len() {
                    target_offset
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
    let mut rows = populate_rows(&vertices, context)?;
    order_rows(
        &mut rows,
        &vertices,
        &segments,
        config.ordering_sweeps,
        context,
    )?;
    place_rows(&rows, &mut vertices, request, clearance, context)?;
    place_horizontal(
        &rows,
        &mut vertices,
        &segments,
        transpose_insets(request.margin(), request.direction()),
        clearance,
        context,
    )?;
    context.allocate(GraphStage::Result, request.nodes().len() as u64 * 2)?;
    for (index, node) in request.nodes().iter().enumerate() {
        candidate.placements.push(GraphPlacement {
            entity: node.id().into(),
            bounds: transpose_rect(vertices[index].bounds, request.direction()),
        });
        evidence.nodes.push(LayeredNodeRank {
            node: node.id(),
            rank: vertices[index].rank,
        });
    }
    for (index, edge) in request.edges().iter().enumerate() {
        context.allocate(GraphStage::Result, 2)?;
        let (working_source, working_target) = (edges[index].source, edges[index].target);
        let low = if request.direction() == GraphDirection::South {
            AttachmentSide::Top
        } else {
            AttachmentSide::Left
        };
        let high = if request.direction() == GraphDirection::South {
            AttachmentSide::Bottom
        } else {
            AttachmentSide::Right
        };
        let source_input = request.attachment(edge, true)?;
        let target_input = request.attachment(edge, false)?;
        let source_node = if feedback[index] {
            working_target
        } else {
            working_source
        };
        let target_node = if feedback[index] {
            working_source
        } else {
            working_target
        };
        let source = PreparedGraphAttachment::on_bounds(
            edge.source,
            candidate.placements[source_node].bounds,
            source_input
                .side
                .unwrap_or(if feedback[index] { low } else { high }),
            source_input,
            0.0,
        );
        let target = PreparedGraphAttachment::on_bounds(
            edge.target,
            candidate.placements[target_node].bounds,
            target_input
                .side
                .unwrap_or(if feedback[index] { high } else { low }),
            target_input,
            0.0,
        );
        let points = super::graph_routing::route_directed(
            request,
            &candidate.placements,
            edge,
            source,
            target,
            &vertices,
            &rows,
            &chains[index],
            feedback[index],
            context,
        )?;
        candidate.routes.push(GraphRoute {
            edge: edge.id(),
            source,
            target,
            points,
        });
        context.allocate(
            GraphStage::Result,
            chains[index].len().saturating_sub(2) as u64,
        )?;
        let mut virtual_nodes = Vec::with_capacity(chains[index].len().saturating_sub(2));
        for vertex in &chains[index][1..chains[index].len() - 1] {
            virtual_nodes.push(LayeredVirtualNode {
                rank: vertices[*vertex].rank,
                bounds: transpose_rect(vertices[*vertex].bounds, request.direction()),
            });
        }
        if feedback[index] {
            virtual_nodes.reverse();
        }
        evidence.edges.push(LayeredEdgeRanks {
            edge: edge.id(),
            feedback: feedback[index],
            source_rank: vertices[working_source].rank,
            target_rank: vertices[working_target].rank,
            virtual_nodes,
        });
    }
    candidate.extent = super::graph_routing::extent(request, &candidate, context)?;
    for edge in &evidence.edges {
        for virtual_node in &edge.virtual_nodes {
            context.consume(GraphStage::Result, 1)?;
            candidate.extent = super::graph_routing::union(candidate.extent, virtual_node.bounds);
        }
    }
    candidate
        .capabilities
        .insert(LAYERED_RESULT, evidence, context)?;
    Ok(candidate)
}

pub(super) fn rank_value(value: f64) -> Result<u32, GraphLayoutError> {
    if value < 0.0 || value > f64::from(u32::MAX) || value.fract() != 0.0 {
        return Err(GraphLayoutError::Overflow(GraphStage::Ranking));
    }
    Ok(value as u32)
}

/// Greedy source/sink removal; an acyclic input never has an edge inverted.
pub(super) fn break_cycles(
    count: usize,
    edges: &mut [RankEdge],
    context: &mut GraphSolveContext,
) -> Result<Vec<bool>, GraphLayoutError> {
    context.allocate(GraphStage::Algorithm, (count * 4 + edges.len()) as u64)?;
    let mut remaining = vec![true; count];
    let mut left = Vec::with_capacity(count);
    let mut right = Vec::with_capacity(count);
    while left.len() + right.len() < count {
        let mut source = None;
        let mut sink = None;
        let mut greedy = None;
        let mut maximum = i128::MIN;
        for node in 0..count {
            context.consume(GraphStage::Algorithm, 1)?;
            if !remaining[node] {
                continue;
            }
            let mut incoming = 0_i128;
            let mut outgoing = 0_i128;
            for edge in edges.iter() {
                context.consume(GraphStage::Algorithm, 1)?;
                if edge.source == node && remaining[edge.target] {
                    outgoing += 1;
                }
                if edge.target == node && remaining[edge.source] {
                    incoming += 1;
                }
            }
            if incoming == 0 && source.is_none() {
                source = Some(node);
            }
            if outgoing == 0 && sink.is_none() {
                sink = Some(node);
            }
            if outgoing - incoming > maximum {
                maximum = outgoing - incoming;
                greedy = Some(node);
            }
        }
        let node = if let Some(node) = sink {
            right.push(node);
            node
        } else {
            let node = source
                .or(greedy)
                .ok_or(GraphLayoutError::CapabilityRejected("cycle ordering"))?;
            left.push(node);
            node
        };
        remaining[node] = false;
    }
    let mut order = vec![0; count];
    for (position, node) in left.into_iter().chain(right.into_iter().rev()).enumerate() {
        order[node] = position;
    }
    let mut feedback = Vec::with_capacity(edges.len());
    for edge in edges {
        context.consume(GraphStage::Algorithm, 1)?;
        let inverted = order[edge.source] > order[edge.target];
        if inverted {
            std::mem::swap(&mut edge.source, &mut edge.target);
        }
        feedback.push(inverted);
    }
    Ok(feedback)
}

pub(super) fn populate_rows(
    vertices: &[Vertex],
    context: &mut GraphSolveContext,
) -> Result<Vec<Vec<usize>>, GraphLayoutError> {
    let count = vertices
        .iter()
        .map(|vertex| u64::from(vertex.rank) + 1)
        .max()
        .unwrap_or(0);
    context.allocate(GraphStage::Ordering, count)?;
    context.allocate(GraphStage::Ordering, vertices.len() as u64)?;
    let mut rows = vec![
        Vec::new();
        usize::try_from(count)
            .map_err(|_| GraphLayoutError::Overflow(GraphStage::Ordering))?
    ];
    for (index, vertex) in vertices.iter().enumerate() {
        rows[vertex.rank as usize].push(index);
    }
    for row in &mut rows {
        context.consume(GraphStage::Ordering, row.len() as u64 * row.len() as u64)?;
        row.sort_by_key(|vertex| vertices[*vertex].row);
    }
    Ok(rows)
}

fn constrained(left: &Vertex, right: &Vertex) -> bool {
    matches!((left.row, right.row), (Some(a), Some(b)) if a != b)
}

pub(super) fn order_rows(
    rows: &mut [Vec<usize>],
    vertices: &[Vertex],
    segments: &[Segment],
    sweeps: u32,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    context.allocate(GraphStage::Ordering, vertices.len() as u64 * 3)?;
    let mut positions = vec![0; vertices.len()];
    let mut scores = vec![0.0; vertices.len()];
    let mut neighbors = Vec::with_capacity(vertices.len());
    for _ in 0..sweeps {
        for reverse in [false, true] {
            for step in 0..rows.len() {
                let row_index = if reverse { rows.len() - step - 1 } else { step };
                for row in rows.iter() {
                    context.consume(GraphStage::Ordering, row.len() as u64)?;
                    for (position, vertex) in row.iter().enumerate() {
                        positions[*vertex] = position;
                    }
                }
                for vertex in &rows[row_index] {
                    neighbors.clear();
                    for segment in segments {
                        context.consume(GraphStage::Ordering, 1)?;
                        let neighbor = if !reverse
                            && segment.target == *vertex
                            && vertices[segment.source].rank < vertices[*vertex].rank
                        {
                            Some(segment.source)
                        } else if reverse
                            && segment.source == *vertex
                            && vertices[segment.target].rank > vertices[*vertex].rank
                        {
                            Some(segment.target)
                        } else {
                            None
                        };
                        if let Some(neighbor) = neighbor {
                            context.allocate(GraphStage::Ordering, 1)?;
                            neighbors.push(positions[neighbor]);
                        }
                    }
                    context.consume(
                        GraphStage::Ordering,
                        neighbors.len() as u64 * neighbors.len() as u64,
                    )?;
                    neighbors.sort_unstable();
                    scores[*vertex] = if neighbors.is_empty() {
                        positions[*vertex] as f64
                    } else {
                        (neighbors[(neighbors.len() - 1) / 2] as f64
                            + neighbors[neighbors.len() / 2] as f64)
                            / 2.0
                    };
                }
                let row = &mut rows[row_index];
                loop {
                    let mut changed = false;
                    for index in 1..row.len() {
                        context.consume(GraphStage::Ordering, 1)?;
                        if !constrained(&vertices[row[index - 1]], &vertices[row[index]])
                            && scores[row[index - 1]] > scores[row[index]]
                        {
                            row.swap(index - 1, index);
                            changed = true;
                        }
                    }
                    if !changed {
                        break;
                    }
                }
            }
        }
    }
    // Local adjacent optimization counts both incoming and outgoing crossings, including
    // attachment offsets when two edges meet the same neighbor.
    loop {
        for row in rows.iter() {
            context.consume(GraphStage::Ordering, row.len() as u64)?;
            for (position, vertex) in row.iter().enumerate() {
                positions[*vertex] = position;
            }
        }
        let mut changed = false;
        for row in rows.iter_mut() {
            for index in 1..row.len() {
                let (left, right) = (row[index - 1], row[index]);
                if constrained(&vertices[left], &vertices[right]) {
                    continue;
                }
                let mut gain = 0_i128;
                for a in segments {
                    for b in segments {
                        context.consume(GraphStage::Ordering, 1)?;
                        let neighbors = if a.target == left && b.target == right {
                            Some((
                                (positions[a.source], a.source_offset),
                                (positions[b.source], b.source_offset),
                            ))
                        } else if a.source == left && b.source == right {
                            Some((
                                (positions[a.target], a.target_offset),
                                (positions[b.target], b.target_offset),
                            ))
                        } else {
                            None
                        };
                        if let Some((a, b)) = neighbors {
                            let comparison = a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1));
                            gain += match comparison {
                                std::cmp::Ordering::Greater => 1,
                                std::cmp::Ordering::Less => -1,
                                std::cmp::Ordering::Equal => 0,
                            };
                        }
                    }
                }
                if gain > 0 {
                    row.swap(index - 1, index);
                    positions[left] = index;
                    positions[right] = index - 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(())
}

fn place_rows(
    rows: &[Vec<usize>],
    vertices: &mut [Vertex],
    request: &GraphLayoutRequest,
    clearance: f64,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    let mut cursor = transpose_insets(request.margin(), request.direction()).top;
    for row in rows {
        context.consume(GraphStage::Placement, row.len() as u64 * 2)?;
        let height = row
            .iter()
            .map(|index| vertices[*index].bounds.height)
            .fold(0.0_f64, f64::max);
        let top = row
            .iter()
            .map(|index| vertices[*index].padding.top)
            .fold(0.0_f64, f64::max);
        let mut y = cursor + top + clearance;
        // Admission compares the computed padded boxes, not an algebraically equivalent
        // rank sum. Avoid inward rounding at the previous row's actual padded boundary.
        for index in row {
            context.consume(GraphStage::Placement, 1)?;
            y = outside_leading_boundary(y, vertices[*index].padding.top, cursor, context)?;
        }
        for index in row {
            if *index < request.nodes().len() {
                let node = &request.nodes()[*index];
                let axis = match request.direction() {
                    GraphDirection::South => node.size.height,
                    GraphDirection::East => node.size.width,
                };
                if !axis.allow_stretch && height != axis.preferred {
                    return Err(GraphLayoutError::UnsatisfiedSizeConstraint(
                        node.id().into(),
                    ));
                }
            }
            vertices[*index].bounds.y = y;
            vertices[*index].bounds.height = height;
        }
        cursor = row
            .iter()
            .map(|index| {
                let vertex = &vertices[*index];
                vertex.bounds.y - vertex.padding.top
                    + (vertex.bounds.height + vertex.padding.height())
            })
            .fold(y + height, f64::max)
            + clearance;
        if !cursor.is_finite() {
            return Err(GraphLayoutError::Overflow(GraphStage::Placement));
        }
    }
    Ok(())
}

pub(super) fn place_horizontal(
    rows: &[Vec<usize>],
    vertices: &mut [Vertex],
    segments: &[Segment],
    margin: Insets,
    clearance: f64,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    let left = vertices.len();
    let right = left
        .checked_add(1)
        .ok_or(GraphLayoutError::Overflow(GraphStage::Placement))?;
    let count = right
        .checked_add(1)
        .and_then(|value| value.checked_add(segments.len()))
        .ok_or(GraphLayoutError::Overflow(GraphStage::Placement))?;
    let edge_count = vertices
        .len()
        .checked_add(rows.len())
        .and_then(|value| value.checked_add(segments.len().checked_mul(2)?))
        .ok_or(GraphLayoutError::Overflow(GraphStage::Placement))?;
    context.allocate(GraphStage::Placement, edge_count as u64)?;
    let mut edges = Vec::with_capacity(edge_count);
    for row in rows {
        let mut previous = None;
        for index in row {
            context.consume(GraphStage::Placement, 1)?;
            let vertex = &vertices[*index];
            if let Some(previous) = previous {
                let before: &Vertex = &vertices[previous];
                edges.push(RankEdge {
                    source: previous,
                    target: *index,
                    span: before.bounds.width
                        + before.padding.right
                        + vertex.padding.left
                        + clearance * 2.0,
                    weight: 0,
                });
            } else {
                edges.push(RankEdge {
                    source: left,
                    target: *index,
                    span: margin.left + vertex.padding.left + clearance,
                    weight: 0,
                });
            }
            previous = Some(*index);
        }
        if let Some(previous) = previous {
            edges.push(RankEdge {
                source: previous,
                target: right,
                span: vertices[previous].bounds.width
                    + vertices[previous].padding.right
                    + margin.right
                    + clearance,
                weight: 0,
            });
        }
    }
    for (index, segment) in segments.iter().enumerate() {
        let auxiliary = right + 1 + index;
        let difference = segment.source_offset - segment.target_offset;
        edges.push(RankEdge {
            source: auxiliary,
            target: segment.source,
            span: (-difference).max(0.0),
            weight: segment.weight,
        });
        edges.push(RankEdge {
            source: auxiliary,
            target: segment.target,
            span: difference.max(0.0),
            weight: segment.weight,
        });
    }
    let ranks = ranking::solve(count, &edges, context)?;
    let origin = ranks[left];
    for (index, vertex) in vertices.iter_mut().enumerate() {
        context.consume(GraphStage::Placement, 1)?;
        vertex.bounds.x = ranks[index] - origin;
    }
    // Apply the same directed rounding to the actual horizontal padded boxes.
    // It only moves an inward-rounded boundary outward; the weighted solver owns placement.
    for row in rows {
        let mut leading = margin.left + clearance;
        for index in row {
            context.consume(GraphStage::Placement, 1)?;
            let vertex = &mut vertices[*index];
            vertex.bounds.x =
                outside_leading_boundary(vertex.bounds.x, vertex.padding.left, leading, context)?;
            leading = vertex.bounds.x - vertex.padding.left
                + (vertex.bounds.width + vertex.padding.width())
                + clearance * 2.0;
            if !leading.is_finite() {
                return Err(GraphLayoutError::Overflow(GraphStage::Placement));
            }
        }
    }
    Ok(())
}

fn outside_leading_boundary(
    coordinate: f64,
    padding: f64,
    minimum: f64,
    context: &mut GraphSolveContext,
) -> Result<f64, GraphLayoutError> {
    let mut coordinate = coordinate.max(minimum + padding);
    while coordinate - padding < minimum {
        context.consume(GraphStage::Placement, 1)?;
        coordinate = coordinate.next_up();
    }
    if coordinate.is_finite() {
        Ok(coordinate)
    } else {
        Err(GraphLayoutError::Overflow(GraphStage::Placement))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::graph::GraphBudget;

    fn context() -> GraphSolveContext {
        GraphSolveContext::new(GraphBudget::new(1_000_000, 100_000))
    }
    fn crossings(rows: &[Vec<usize>], segments: &[Segment]) -> usize {
        let mut positions = vec![0; rows.iter().map(Vec::len).sum()];
        for row in rows {
            for (position, node) in row.iter().enumerate() {
                positions[*node] = position;
            }
        }
        let mut count = 0;
        for (index, left) in segments.iter().enumerate() {
            for right in &segments[index + 1..] {
                let source = positions[left.source].cmp(&positions[right.source]);
                let target = positions[left.target].cmp(&positions[right.target]);
                if source != std::cmp::Ordering::Equal
                    && target != std::cmp::Ordering::Equal
                    && source != target
                {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn ordering_independently_reduces_crossings() {
        let vertices: Vec<_> = (0..4)
            .map(|index| Vertex {
                bounds: Rectangle::new(0.0, 0.0, 20.0, 10.0),
                padding: Insets::ZERO,
                rank: index / 2,
                row: None,
            })
            .collect();
        let segments = [
            Segment {
                source: 0,
                target: 3,
                source_offset: 10.0,
                target_offset: 10.0,
                weight: 1,
            },
            Segment {
                source: 1,
                target: 2,
                source_offset: 10.0,
                target_offset: 10.0,
                weight: 1,
            },
            Segment {
                source: 0,
                target: 2,
                source_offset: 10.0,
                target_offset: 10.0,
                weight: 1,
            },
        ];
        let mut rows = vec![vec![0, 1], vec![2, 3]];
        assert_eq!(crossings(&rows, &segments), 1);
        order_rows(
            &mut rows,
            &vertices,
            &segments,
            DIRECTED_ORDERING_SWEEPS,
            &mut context(),
        )
        .unwrap();
        assert_eq!(crossings(&rows, &segments), 0);
    }

    #[test]
    fn ordering_and_placement_charge_allocation_before_work() {
        let vertices = [Vertex {
            bounds: Rectangle::ZERO,
            padding: Insets::ZERO,
            rank: 0,
            row: None,
        }];
        let mut tiny = GraphSolveContext::new(GraphBudget::new(100, 0));
        assert!(matches!(
            populate_rows(&vertices, &mut tiny),
            Err(GraphLayoutError::BudgetExceeded {
                stage: GraphStage::Ordering,
                allocation: true,
                ..
            })
        ));
        let mut vertices = vertices;
        let mut tiny = GraphSolveContext::new(GraphBudget::new(100, 0));
        assert!(matches!(
            place_horizontal(&[vec![0]], &mut vertices, &[], Insets::ZERO, 0.0, &mut tiny),
            Err(GraphLayoutError::BudgetExceeded {
                stage: GraphStage::Placement,
                allocation: true,
                ..
            })
        ));
    }
}
