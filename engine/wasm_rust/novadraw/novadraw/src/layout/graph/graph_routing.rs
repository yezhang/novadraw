//! Finite virtual-node corridors and deterministic Euclidean visibility routing.

use crate::geometry::{Insets, Point, Rectangle};

use super::directed::{Vertex, transpose_insets, transpose_point, transpose_rect};
use super::{
    AttachmentSide, GraphEdge, GraphLayoutCandidate, GraphLayoutError, GraphLayoutRequest,
    GraphPlacement, GraphSolveContext, GraphStage, GraphViolation, PreparedGraphAttachment,
};

fn rejected(edge: &GraphEdge) -> GraphLayoutError {
    GraphLayoutError::RejectedCandidate {
        entity: None,
        edge: Some(edge.id()),
        violation: GraphViolation::Clearance,
    }
}
fn expand(bounds: Rectangle, padding: Insets, clearance: f64) -> Rectangle {
    // Match the validator's actual obstacle arithmetic, including its operation order.
    let padding = Insets::new(
        padding.top + clearance,
        padding.left + clearance,
        padding.bottom + clearance,
        padding.right + clearance,
    );
    Rectangle::new(
        bounds.x - padding.left,
        bounds.y - padding.top,
        bounds.width + padding.width(),
        bounds.height + padding.height(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn route_directed(
    request: &GraphLayoutRequest,
    placements: &[GraphPlacement],
    edge: &GraphEdge,
    source: PreparedGraphAttachment,
    target: PreparedGraphAttachment,
    vertices: &[Vertex],
    rows: &[Vec<usize>],
    chain: &[usize],
    feedback: bool,
    context: &mut GraphSolveContext,
) -> Result<Vec<Point>, GraphLayoutError> {
    let clearance = edge.padding + edge.width / 2.0;
    context.allocate(
        GraphStage::Routing,
        placements.len() as u64 + chain.len() as u64 * 2,
    )?;
    let mut obstacles = Vec::with_capacity(placements.len() + chain.len() * 2);
    let mut unrelated = Vec::new();
    let mut source_obstacle = None;
    let mut target_obstacle = None;
    for placement in placements {
        context.consume(GraphStage::Routing, 1)?;
        let bounds = transpose_rect(placement.bounds, request.direction());
        let padding = transpose_insets(
            request.entity_padding(placement.entity)?,
            request.direction(),
        );
        let obstacle = expand(bounds, padding, clearance);
        obstacles.push(obstacle);
        if placement.entity == edge.source {
            source_obstacle = Some(obstacle);
        }
        if placement.entity == edge.target {
            target_obstacle = Some(obstacle);
        }
        if placement.entity != edge.source && placement.entity != edge.target {
            context.allocate(GraphStage::Routing, 1)?;
            unrelated.push(obstacle);
        }
    }
    let source_position = transpose_point(source.position, request.direction());
    let target_position = transpose_point(target.position, request.direction());
    let source_side = canonical_side(source.side, request.direction());
    let target_side = canonical_side(target.side, request.direction());
    // An adjacent-rank edge can use Draw2D's direct path. Both endpoint owners exempt
    // this single segment, but every unrelated padded entity remains an obstacle.
    if chain.len() == 2
        && exits(source_position, target_position, source_side)
        && exits(target_position, source_position, target_side)
        && clear(source_position, target_position, &unrelated, context)?
    {
        context.allocate(GraphStage::Routing, 2)?;
        return Ok(vec![source.position, target.position]);
    }
    let source_stub = stub(
        source_position,
        source_side,
        source_obstacle.ok_or_else(|| rejected(edge))?,
    );
    let target_stub = stub(
        target_position,
        target_side,
        target_obstacle.ok_or_else(|| rejected(edge))?,
    );
    if !clear(source_position, source_stub, &unrelated, context)?
        || !clear(target_position, target_stub, &unrelated, context)?
    {
        return Err(rejected(edge));
    }
    // The finite clipping box is derived from every entity and terminal. Artificial
    // half-plane rectangles stop at this box; visibility vertices cannot escape it.
    let mut envelope = Rectangle::new(
        source_stub.x().min(target_stub.x()),
        source_stub.y().min(target_stub.y()),
        (source_stub.x() - target_stub.x()).abs(),
        (source_stub.y() - target_stub.y()).abs(),
    );
    for obstacle in &obstacles {
        envelope = union(envelope, *obstacle);
    }
    let room = envelope.width.max(envelope.height).max(clearance);
    envelope = expand(envelope, Insets::uniform(room), 0.0);
    if ![
        envelope.x,
        envelope.y,
        envelope.width,
        envelope.height,
        envelope.x + envelope.width,
        envelope.y + envelope.height,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return Err(GraphLayoutError::Overflow(GraphStage::Routing));
    }
    context.allocate(
        GraphStage::Routing,
        chain.len().saturating_sub(2) as u64 * 2 + 2,
    )?;
    let mut gates = Vec::with_capacity(chain.len().saturating_sub(2) * 2 + 2);
    gates.push(source_stub);
    for vertex_index in &chain[1..chain.len() - 1] {
        let vertex = &vertices[*vertex_index];
        let row = &rows[vertex.rank as usize];
        context.consume(GraphStage::Routing, row.len() as u64)?;
        let position = row
            .iter()
            .position(|index| index == vertex_index)
            .ok_or_else(|| rejected(edge))?;
        for left in [true, false] {
            let neighbor = if left {
                position.checked_sub(1).map(|index| row[index])
            } else {
                row.get(position + 1).copied()
            };
            if let Some(neighbor) = neighbor {
                let neighbor = &vertices[neighbor];
                let expanded = expand(neighbor.bounds, neighbor.padding, clearance);
                let bounds = if left {
                    Rectangle::new(
                        envelope.x,
                        expanded.y,
                        expanded.x + expanded.width - envelope.x,
                        expanded.height,
                    )
                } else {
                    Rectangle::new(
                        expanded.x,
                        expanded.y,
                        envelope.x + envelope.width - expanded.x,
                        expanded.height,
                    )
                };
                obstacles.push(bounds);
            }
        }
        let x = vertex.bounds.x + vertex.bounds.width / 2.0;
        gates.push(Point::new(x, vertex.bounds.y));
        gates.push(Point::new(x, vertex.bounds.y + vertex.bounds.height));
    }
    gates.push(target_stub);
    if feedback {
        gates[1..chain.len().saturating_sub(2) * 2 + 1].reverse();
    }
    context.allocate(GraphStage::Routing, 1)?;
    let mut points = vec![source_position];
    for pair in gates.windows(2) {
        let path = visibility_path(pair[0], pair[1], &obstacles, context)?
            .ok_or_else(|| rejected(edge))?;
        context.allocate(GraphStage::Routing, path.len() as u64)?;
        for point in path {
            if points.last() != Some(&point) {
                points.push(point);
            }
        }
    }
    context.allocate(GraphStage::Routing, 1)?;
    points.push(target_position);
    for point in &mut points {
        *point = transpose_point(*point, request.direction());
    }
    Ok(points)
}

fn canonical_side(side: AttachmentSide, direction: super::GraphDirection) -> AttachmentSide {
    if direction == super::GraphDirection::South {
        return side;
    }
    match side {
        AttachmentSide::Top => AttachmentSide::Left,
        AttachmentSide::Bottom => AttachmentSide::Right,
        AttachmentSide::Left => AttachmentSide::Top,
        AttachmentSide::Right => AttachmentSide::Bottom,
    }
}
fn exits(source: Point, target: Point, side: AttachmentSide) -> bool {
    match side {
        AttachmentSide::Top => target.y() <= source.y(),
        AttachmentSide::Bottom => target.y() >= source.y(),
        AttachmentSide::Left => target.x() <= source.x(),
        AttachmentSide::Right => target.x() >= source.x(),
    }
}
fn stub(point: Point, side: AttachmentSide, obstacle: Rectangle) -> Point {
    match side {
        AttachmentSide::Top => Point::new(point.x(), point.y().min(obstacle.y)),
        AttachmentSide::Bottom => {
            Point::new(point.x(), point.y().max(obstacle.y + obstacle.height))
        }
        AttachmentSide::Left => Point::new(point.x().min(obstacle.x), point.y()),
        AttachmentSide::Right => Point::new(point.x().max(obstacle.x + obstacle.width), point.y()),
    }
}

pub(super) fn clear(
    a: Point,
    b: Point,
    obstacles: &[Rectangle],
    context: &mut GraphSolveContext,
) -> Result<bool, GraphLayoutError> {
    for obstacle in obstacles {
        context.consume(GraphStage::Routing, 1)?;
        if super::validation::segment_intersects_interior(a, b, *obstacle) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn visibility_path(
    source: Point,
    target: Point,
    obstacles: &[Rectangle],
    context: &mut GraphSolveContext,
) -> Result<Option<Vec<Point>>, GraphLayoutError> {
    if clear(source, target, obstacles, context)? {
        context.allocate(GraphStage::Routing, 2)?;
        return Ok(Some(vec![source, target]));
    }
    let count = obstacles
        .len()
        .checked_mul(4)
        .and_then(|value| value.checked_add(2))
        .ok_or(GraphLayoutError::Overflow(GraphStage::Routing))?;
    context.allocate(GraphStage::Routing, count as u64 * 4)?;
    let mut vertices = Vec::with_capacity(count);
    vertices.extend([source, target]);
    for obstacle in obstacles {
        for point in [
            Point::new(obstacle.x, obstacle.y),
            Point::new(obstacle.x + obstacle.width, obstacle.y),
            Point::new(obstacle.x, obstacle.y + obstacle.height),
            Point::new(obstacle.x + obstacle.width, obstacle.y + obstacle.height),
        ] {
            context.consume(GraphStage::Routing, obstacles.len() as u64)?;
            if !obstacles.iter().any(|bounds| {
                point.x() > bounds.x
                    && point.x() < bounds.x + bounds.width
                    && point.y() > bounds.y
                    && point.y() < bounds.y + bounds.height
            }) {
                vertices.push(point);
            }
        }
    }
    let mut distance = vec![f64::INFINITY; vertices.len()];
    let mut previous = vec![None; vertices.len()];
    let mut settled = vec![false; vertices.len()];
    distance[0] = 0.0;
    loop {
        context.consume(GraphStage::Routing, vertices.len() as u64)?;
        let selected = (0..vertices.len())
            .filter(|index| !settled[*index] && distance[*index].is_finite())
            .min_by(|a, b| distance[*a].total_cmp(&distance[*b]).then(a.cmp(b)));
        let Some(index) = selected else {
            return Ok(None);
        };
        if index == 1 {
            break;
        }
        settled[index] = true;
        for other in 0..vertices.len() {
            context.consume(GraphStage::Routing, 1)?;
            if settled[other] || !clear(vertices[index], vertices[other], obstacles, context)? {
                continue;
            }
            let cost = distance[index]
                + (vertices[index].x() - vertices[other].x())
                    .hypot(vertices[index].y() - vertices[other].y());
            if !cost.is_finite() {
                return Err(GraphLayoutError::Overflow(GraphStage::Routing));
            }
            if cost < distance[other] {
                distance[other] = cost;
                previous[other] = Some(index);
            }
        }
    }
    context.allocate(GraphStage::Routing, vertices.len() as u64)?;
    let mut points = Vec::with_capacity(vertices.len());
    let mut cursor = Some(1);
    while let Some(index) = cursor {
        context.consume(GraphStage::Routing, 1)?;
        points.push(vertices[index]);
        cursor = previous[index];
    }
    points.reverse();
    Ok(Some(points))
}

pub(super) fn union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rectangle::new(
        x,
        y,
        (a.x + a.width).max(b.x + b.width) - x,
        (a.y + a.height).max(b.y + b.height) - y,
    )
}

pub(super) fn extent(
    request: &GraphLayoutRequest,
    candidate: &GraphLayoutCandidate,
    context: &mut GraphSolveContext,
) -> Result<Rectangle, GraphLayoutError> {
    let mut result = Rectangle::ZERO;
    for placement in &candidate.placements {
        context.consume(GraphStage::Result, 1)?;
        let mut bounds = expand(
            placement.bounds,
            request.entity_padding(placement.entity)?,
            0.0,
        );
        if request.parent(placement.entity)?.is_none() {
            bounds = expand(bounds, request.margin(), 0.0);
        }
        result = union(result, bounds);
    }
    for route in &candidate.routes {
        for point in &route.points {
            context.consume(GraphStage::Result, 1)?;
            result = union(result, Rectangle::new(point.x(), point.y(), 0.0, 0.0));
        }
    }
    Ok(result)
}
