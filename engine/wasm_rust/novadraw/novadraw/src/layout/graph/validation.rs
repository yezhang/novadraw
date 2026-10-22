use std::collections::HashMap;

use crate::geometry::{Dimension, Insets, Point, Rectangle};

use super::{
    AttachmentOffset, AttachmentSide, GraphAttachment, GraphEndpoint, GraphLayoutCandidate,
    GraphLayoutError, GraphLayoutRequest, GraphSolveContext, GraphStage, GraphViolation,
    PreparedGraphAttachment,
};

const MAX_CONTAINMENT_DEPTH: usize = 10_000;

fn input(entity: Option<GraphEndpoint>, violation: GraphViolation) -> GraphLayoutError {
    GraphLayoutError::InvalidInput {
        entity,
        edge: None,
        violation,
    }
}
fn rejected(
    entity: Option<GraphEndpoint>,
    edge: Option<super::GraphEdgeId>,
    violation: GraphViolation,
) -> GraphLayoutError {
    GraphLayoutError::RejectedCandidate {
        entity,
        edge,
        violation,
    }
}
fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn valid_size(size: Dimension) -> bool {
    nonnegative(size.width) && nonnegative(size.height)
}
fn valid_insets(insets: Insets) -> bool {
    [
        insets.top,
        insets.left,
        insets.bottom,
        insets.right,
        insets.width(),
        insets.height(),
    ]
    .into_iter()
    .all(nonnegative)
}
fn valid_rect(rect: Rectangle) -> bool {
    [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
        .into_iter()
        .all(f64::is_finite)
        && valid_size(Dimension::new(rect.width, rect.height))
}
fn valid_attachment(attachment: GraphAttachment) -> bool {
    match attachment.offset {
        AttachmentOffset::Center => true,
        AttachmentOffset::Distance(value) => nonnegative(value),
    }
}

pub(super) fn validate_request(
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    context.consume(GraphStage::Request, 1)?;
    if !valid_insets(request.default_padding()) || !valid_insets(request.margin()) {
        return Err(input(None, GraphViolation::Padding));
    }
    for (index, node) in request.nodes().iter().enumerate() {
        context.consume(GraphStage::Request, 1)?;
        let entity = Some(node.id().into());
        if node.id.domain != request.identity.domain || node.id.index != index {
            return Err(input(entity, GraphViolation::IdentityCoverage));
        }
        for axis in [node.size.width, node.size.height] {
            if !nonnegative(axis.minimum)
                || !nonnegative(axis.preferred)
                || axis.minimum > axis.preferred
            {
                return Err(input(entity, GraphViolation::Size));
            }
        }
        if !valid_insets(node.padding.unwrap_or(request.default_padding())) {
            return Err(input(entity, GraphViolation::Padding));
        }
        if !valid_attachment(node.incoming) || !valid_attachment(node.outgoing) {
            return Err(input(entity, GraphViolation::Attachment));
        }
        if let Some(parent) = node.parent {
            request.group(parent)?;
        }
    }
    for (index, group) in request.groups().iter().enumerate() {
        context.consume(GraphStage::Request, 1)?;
        let entity = Some(group.id().into());
        if group.id.domain != request.identity.domain || group.id.index != index {
            return Err(input(entity, GraphViolation::IdentityCoverage));
        }
        if !valid_size(group.minimum) {
            return Err(input(entity, GraphViolation::Size));
        }
        if !valid_insets(group.padding.unwrap_or(request.default_padding()))
            || !valid_insets(group.border_insets)
            || !valid_insets(group.inner_padding)
            || !valid_insets(add_insets(group.border_insets, group.inner_padding))
        {
            return Err(input(entity, GraphViolation::Padding));
        }
        if !valid_attachment(group.incoming) || !valid_attachment(group.outgoing) {
            return Err(input(entity, GraphViolation::Attachment));
        }
        // Iterative parent walk bounds stack use, validates every link and rejects cycles.
        let mut cursor = group.parent;
        let mut depth = 0;
        while let Some(parent) = cursor {
            context.consume(GraphStage::Containment, 1)?;
            if parent == group.id() {
                return Err(GraphLayoutError::ContainmentCycle(parent));
            }
            depth += 1;
            if depth > MAX_CONTAINMENT_DEPTH {
                return Err(GraphLayoutError::DepthExceeded);
            }
            if depth > request.groups().len() {
                return Err(GraphLayoutError::ContainmentCycle(parent));
            }
            cursor = request.group(parent)?.parent;
        }
    }
    for (index, edge) in request.edges().iter().enumerate() {
        context.consume(GraphStage::Request, 1)?;
        if edge.id.domain != request.identity.domain || edge.id.index != index {
            return Err(GraphLayoutError::InvalidInput {
                entity: None,
                edge: Some(edge.id()),
                violation: GraphViolation::IdentityCoverage,
            });
        }
        request.parent(edge.source)?;
        request.parent(edge.target)?;
        let violation = if !nonnegative(edge.width)
            || !nonnegative(edge.padding)
            || !nonnegative(edge.padding + edge.width / 2.0)
        {
            Some(GraphViolation::Padding)
        } else if !edge.source_attachment.is_none_or(valid_attachment)
            || !edge.target_attachment.is_none_or(valid_attachment)
        {
            Some(GraphViolation::Attachment)
        } else {
            None
        };
        if let Some(violation) = violation {
            return Err(GraphLayoutError::InvalidInput {
                entity: None,
                edge: Some(edge.id()),
                violation,
            });
        }
    }
    Ok(())
}

fn entity_count(request: &GraphLayoutRequest) -> Result<usize, GraphLayoutError> {
    request
        .nodes()
        .len()
        .checked_add(request.groups().len())
        .ok_or(GraphLayoutError::Overflow(GraphStage::Request))
}
fn entity_index(
    request: &GraphLayoutRequest,
    entity: GraphEndpoint,
) -> Result<usize, GraphLayoutError> {
    match entity {
        GraphEndpoint::Node(id) => {
            request.node(id)?;
            Ok(id.index)
        }
        GraphEndpoint::Group(id) => {
            request.group(id)?;
            request
                .nodes()
                .len()
                .checked_add(id.index)
                .ok_or(GraphLayoutError::Overflow(GraphStage::Connectivity))
        }
    }
}
fn root(
    parents: &mut [usize],
    mut cursor: usize,
    context: &mut GraphSolveContext,
) -> Result<usize, GraphLayoutError> {
    while parents[cursor] != cursor {
        context.consume(GraphStage::Connectivity, 1)?;
        parents[cursor] = parents[parents[cursor]];
        cursor = parents[cursor];
    }
    Ok(cursor)
}
pub(super) fn validate_connected(
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    let count = entity_count(request)?;
    if count == 0 {
        return Ok(());
    }
    context.allocate(GraphStage::Connectivity, count as u64)?;
    let mut parents: Vec<_> = (0..count).collect();
    for entity in request
        .nodes()
        .iter()
        .map(|node| node.id().into())
        .chain(request.groups().iter().map(|group| group.id().into()))
    {
        context.consume(GraphStage::Connectivity, 1)?;
        if let Some(parent) = request.parent(entity)? {
            let a = root(&mut parents, entity_index(request, entity)?, context)?;
            let b = root(&mut parents, entity_index(request, parent.into())?, context)?;
            parents[a] = b;
        }
    }
    for edge in request.edges() {
        context.consume(GraphStage::Connectivity, 1)?;
        let a = root(&mut parents, entity_index(request, edge.source)?, context)?;
        let b = root(&mut parents, entity_index(request, edge.target)?, context)?;
        parents[a] = b;
    }
    let first = root(&mut parents, 0, context)?;
    for index in 1..count {
        context.consume(GraphStage::Connectivity, 1)?;
        if root(&mut parents, index, context)? != first {
            return Err(GraphLayoutError::Disconnected);
        }
    }
    Ok(())
}

fn add_insets(a: Insets, b: Insets) -> Insets {
    Insets::new(
        a.top + b.top,
        a.left + b.left,
        a.bottom + b.bottom,
        a.right + b.right,
    )
}
fn expand(bounds: Rectangle, insets: Insets) -> Rectangle {
    Rectangle::new(
        bounds.x - insets.left,
        bounds.y - insets.top,
        bounds.width + insets.width(),
        bounds.height + insets.height(),
    )
}
fn inner(bounds: Rectangle, insets: Insets) -> Rectangle {
    Rectangle::new(
        bounds.x + insets.left,
        bounds.y + insets.top,
        bounds.width - insets.width(),
        bounds.height - insets.height(),
    )
}
fn covers(outer: Rectangle, bounds: Rectangle) -> bool {
    bounds.x >= outer.x
        && bounds.y >= outer.y
        && bounds.x + bounds.width <= outer.x + outer.width
        && bounds.y + bounds.height <= outer.y + outer.height
}
fn contains_point(bounds: Rectangle, point: Point) -> bool {
    covers(bounds, Rectangle::new(point.x(), point.y(), 0.0, 0.0))
}
fn overlaps(a: Rectangle, b: Rectangle) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
fn ancestor(
    request: &GraphLayoutRequest,
    group: GraphEndpoint,
    entity: GraphEndpoint,
    context: &mut GraphSolveContext,
) -> Result<bool, GraphLayoutError> {
    let GraphEndpoint::Group(group) = group else {
        return Ok(false);
    };
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

pub(super) fn validate_candidate(
    request: &GraphLayoutRequest,
    candidate: &GraphLayoutCandidate,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    context.consume(GraphStage::Result, 1)?;
    if candidate.request != request.identity() || candidate.direction != request.direction() {
        return Err(GraphLayoutError::RequestMismatch);
    }
    let count = entity_count(request)?;
    if candidate.placements.len() != count || candidate.routes.len() != request.edges().len() {
        return Err(rejected(None, None, GraphViolation::IdentityCoverage));
    }
    if !valid_rect(candidate.extent) {
        return Err(rejected(None, None, GraphViolation::Extent));
    }
    if count == 0 && candidate.extent != Rectangle::ZERO {
        return Err(rejected(None, None, GraphViolation::Extent));
    }
    context.allocate(GraphStage::Result, count as u64)?;
    let mut placements = HashMap::with_capacity(count);
    for placement in &candidate.placements {
        context.consume(GraphStage::Result, 1)?;
        request.parent(placement.entity)?;
        if placements
            .insert(placement.entity, placement.bounds)
            .is_some()
        {
            return Err(GraphLayoutError::DuplicateEntity(placement.entity));
        }
        let bounds = placement.bounds;
        let entity = Some(placement.entity);
        if !valid_rect(bounds) {
            return Err(rejected(entity, None, GraphViolation::Geometry));
        }
        match placement.entity {
            GraphEndpoint::Node(id) => {
                let size = request.node(id)?.size;
                for (axis, actual) in [(size.width, bounds.width), (size.height, bounds.height)] {
                    if actual < axis.minimum
                        || actual < axis.preferred
                        || (!axis.allow_stretch && actual != axis.preferred)
                    {
                        return Err(rejected(entity, None, GraphViolation::Size));
                    }
                }
            }
            GraphEndpoint::Group(id) => {
                let group = request.group(id)?;
                let insets = add_insets(group.border_insets, group.inner_padding);
                if bounds.width < group.minimum.width.max(insets.width())
                    || bounds.height < group.minimum.height.max(insets.height())
                {
                    return Err(rejected(entity, None, GraphViolation::Size));
                }
            }
        }
        let padded = expand(bounds, request.entity_padding(placement.entity)?);
        let envelope = if request.parent(placement.entity)?.is_none() {
            expand(padded, request.margin())
        } else {
            padded
        };
        if !valid_rect(envelope) || !covers(candidate.extent, envelope) {
            return Err(rejected(entity, None, GraphViolation::Extent));
        }
    }
    for (index, placement) in candidate.placements.iter().enumerate() {
        context.consume(GraphStage::Result, 1)?;
        if let Some(parent) = request.parent(placement.entity)? {
            let group = request.group(parent)?;
            let interior = inner(
                placements[&parent.into()],
                add_insets(group.border_insets, group.inner_padding),
            );
            if !valid_rect(interior)
                || !covers(
                    interior,
                    expand(placement.bounds, request.entity_padding(placement.entity)?),
                )
            {
                return Err(rejected(
                    Some(placement.entity),
                    None,
                    GraphViolation::Containment,
                ));
            }
        }
        for other in &candidate.placements[index + 1..] {
            context.consume(GraphStage::Result, 1)?;
            if ancestor(request, placement.entity, other.entity, context)?
                || ancestor(request, other.entity, placement.entity, context)?
            {
                continue;
            }
            let a = expand(placement.bounds, request.entity_padding(placement.entity)?);
            let b = expand(other.bounds, request.entity_padding(other.entity)?);
            if overlaps(a, b) {
                return Err(rejected(
                    Some(placement.entity),
                    None,
                    GraphViolation::Overlap,
                ));
            }
        }
    }
    context.allocate(GraphStage::Result, candidate.routes.len() as u64)?;
    let mut edges = std::collections::HashSet::with_capacity(candidate.routes.len());
    for route in &candidate.routes {
        context.consume(GraphStage::Result, 1)?;
        let edge = request.edge(route.edge)?;
        if !edges.insert(route.edge) {
            return Err(GraphLayoutError::DuplicateEdge(route.edge));
        }
        if route.points.len() < 2 {
            return Err(rejected(None, Some(route.edge), GraphViolation::Attachment));
        }
        if edge.source == edge.target && route.points.len() < 4 {
            return Err(rejected(None, Some(route.edge), GraphViolation::Clearance));
        }
        validate_endpoint(
            edge.source,
            request.attachment(edge, true)?,
            route.source,
            placements[&edge.source],
            request,
            route.edge,
        )?;
        validate_endpoint(
            edge.target,
            request.attachment(edge, false)?,
            route.target,
            placements[&edge.target],
            request,
            route.edge,
        )?;
        if route.points.first() != Some(&route.source.position)
            || route.points.last() != Some(&route.target.position)
        {
            return Err(rejected(None, Some(route.edge), GraphViolation::Attachment));
        }
        for point in &route.points {
            context.consume(GraphStage::Result, 1)?;
            if !point.x().is_finite() || !point.y().is_finite() {
                return Err(rejected(None, Some(route.edge), GraphViolation::Geometry));
            }
            if !contains_point(candidate.extent, *point) {
                return Err(rejected(None, Some(route.edge), GraphViolation::Extent));
            }
        }
        for segment in route.points.windows(2) {
            context.consume(GraphStage::Routing, 1)?;
            if !(segment[1].x() - segment[0].x()).is_finite()
                || !(segment[1].y() - segment[0].y()).is_finite()
            {
                return Err(rejected(None, Some(route.edge), GraphViolation::Geometry));
            }
        }
        for (endpoint, neighbor) in [
            (route.source, route.points[1]),
            (route.target, route.points[route.points.len() - 2]),
        ] {
            if matches!(endpoint.endpoint, GraphEndpoint::Node(_))
                && !exits_side(endpoint, neighbor)
            {
                return Err(rejected(
                    Some(endpoint.endpoint),
                    Some(route.edge),
                    GraphViolation::Attachment,
                ));
            }
        }
        for placement in &candidate.placements {
            context.consume(GraphStage::Routing, 1)?;
            // Containing groups may be entered/exited by a member's edge. Unrelated groups
            // remain obstacles; the endpoint owner exempts only its initial/final segment.
            if ancestor(request, placement.entity, edge.source, context)?
                || ancestor(request, placement.entity, edge.target, context)?
            {
                continue;
            }
            let clearance = edge.padding + edge.width / 2.0;
            let obstacle = expand(
                placement.bounds,
                add_insets(
                    request.entity_padding(placement.entity)?,
                    Insets::uniform(clearance),
                ),
            );
            if !valid_rect(obstacle) {
                return Err(rejected(
                    Some(placement.entity),
                    Some(route.edge),
                    GraphViolation::Clearance,
                ));
            }
            for (index, segment) in route.points.windows(2).enumerate() {
                context.consume(GraphStage::Routing, 1)?;
                if (placement.entity == edge.source && index == 0)
                    || (placement.entity == edge.target && index + 2 == route.points.len())
                {
                    continue;
                }
                if segment_intersects_interior(segment[0], segment[1], obstacle) {
                    return Err(rejected(
                        Some(placement.entity),
                        Some(route.edge),
                        GraphViolation::Clearance,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn exits_side(endpoint: PreparedGraphAttachment, neighbor: Point) -> bool {
    match endpoint.side {
        AttachmentSide::Top => neighbor.y() <= endpoint.position.y(),
        AttachmentSide::Bottom => neighbor.y() >= endpoint.position.y(),
        AttachmentSide::Left => neighbor.x() <= endpoint.position.x(),
        AttachmentSide::Right => neighbor.x() >= endpoint.position.x(),
    }
}

fn validate_endpoint(
    entity: GraphEndpoint,
    requested: GraphAttachment,
    actual: PreparedGraphAttachment,
    bounds: Rectangle,
    request: &GraphLayoutRequest,
    edge: super::GraphEdgeId,
) -> Result<(), GraphLayoutError> {
    let invalid = || rejected(Some(entity), Some(edge), GraphViolation::Attachment);
    if actual.endpoint != entity
        || actual.offset != requested.offset
        || requested.side.is_some_and(|side| side != actual.side)
        || !nonnegative(actual.inset)
    {
        return Err(invalid());
    }
    let max_inset = match entity {
        GraphEndpoint::Node(_) => 0.0,
        GraphEndpoint::Group(id) => {
            let border = request.group(id)?.border_insets;
            match actual.side {
                AttachmentSide::Top => border.top,
                AttachmentSide::Bottom => border.bottom,
                AttachmentSide::Left => border.left,
                AttachmentSide::Right => border.right,
            }
        }
    };
    if actual.inset > max_inset {
        return Err(invalid());
    }
    let cross = match actual.side {
        AttachmentSide::Top | AttachmentSide::Bottom => bounds.width,
        AttachmentSide::Left | AttachmentSide::Right => bounds.height,
    };
    let offset = match actual.offset {
        AttachmentOffset::Center => cross / 2.0,
        AttachmentOffset::Distance(value) if nonnegative(value) => value,
        _ => return Err(invalid()),
    };
    let expected = match actual.side {
        AttachmentSide::Top => Point::new(bounds.x + offset, bounds.y + actual.inset),
        AttachmentSide::Bottom => {
            Point::new(bounds.x + offset, bounds.y + bounds.height - actual.inset)
        }
        AttachmentSide::Left => Point::new(bounds.x + actual.inset, bounds.y + offset),
        AttachmentSide::Right => {
            Point::new(bounds.x + bounds.width - actual.inset, bounds.y + offset)
        }
    };
    if actual.position != expected {
        return Err(invalid());
    }
    Ok(())
}

/// Open-interior slab intersection. Boundary contact is legal; entering a padded obstacle is not.
fn segment_intersects_interior(a: Point, b: Point, rect: Rectangle) -> bool {
    if rect.width == 0.0 || rect.height == 0.0 {
        return false;
    }
    let mut low: f64 = 0.0;
    let mut high: f64 = 1.0;
    for (start, end, min, max) in [
        (a.x(), b.x(), rect.x, rect.x + rect.width),
        (a.y(), b.y(), rect.y, rect.y + rect.height),
    ] {
        let delta = end - start;
        if delta == 0.0 {
            if start <= min || start >= max {
                return false;
            }
        } else {
            let t1 = (min - start) / delta;
            let t2 = (max - start) / delta;
            low = low.max(t1.min(t2));
            high = high.min(t1.max(t2));
            if low >= high {
                return false;
            }
        }
    }
    low < high && high > 0.0 && low < 1.0
}
