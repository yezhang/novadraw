//! Figure bindings and routes consume one Runtime-owned checked graph snapshot.
use std::{collections::HashSet, sync::Arc};

use crate::{
    connection::{
        AnchorError, AnchorSite, ConnectionAnchor, ConnectionId, ConnectionRouter, CoordinateSpace,
        RouteEnd, RouteError, RouteGroupInput, RouteGroupOutput, RouteOutput, RouteRequest,
        RouteSceneQuery, RoutingGroupScope,
    },
    figure::{FigurePresentation, PreparedSceneSnapshot},
    geometry::{Point, PointList, Vec2},
    tree::FigureId,
};

use super::{
    AttachmentSide, GraphEdgeId, GraphEndpoint, GraphLayoutError, GraphLayoutRequest,
    GraphLayoutResult, GraphSolveContext, GraphStage,
};

/// Complete immutable identity map. Figure parenting is checked against the candidate scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphProjectionBinding {
    owner: FigureId,
    entities: Vec<(GraphEndpoint, FigureId)>,
    edges: Vec<(GraphEdgeId, ConnectionId)>,
}

impl GraphProjectionBinding {
    pub fn new(
        owner: FigureId,
        request: &GraphLayoutRequest,
        entities: Vec<(GraphEndpoint, FigureId)>,
        edges: Vec<(GraphEdgeId, ConnectionId)>,
        context: &mut GraphSolveContext,
    ) -> Result<Self, GraphLayoutError> {
        context.allocate(GraphStage::Request, (entities.len() + edges.len()) as u64)?;
        let mut ids = HashSet::new();
        let mut figures = HashSet::new();
        for &(entity, figure) in &entities {
            context.consume(GraphStage::Request, 1)?;
            match entity {
                GraphEndpoint::Node(id) => {
                    request.node(id)?;
                }
                GraphEndpoint::Group(id) => {
                    request.group(id)?;
                }
            }
            if !ids.insert(entity) || !figures.insert(figure) || figure == owner {
                return Err(GraphLayoutError::InvalidProjectionBinding);
            }
        }
        if ids.len() != request.nodes().len() + request.groups().len() {
            return Err(GraphLayoutError::InvalidProjectionBinding);
        }
        let mut edge_ids = HashSet::new();
        for &(edge, connection) in &edges {
            context.consume(GraphStage::Request, 1)?;
            request.edge(edge)?;
            if !edge_ids.insert(edge)
                || !figures.insert(connection.figure())
                || connection.figure() == owner
            {
                return Err(GraphLayoutError::InvalidProjectionBinding);
            }
        }
        if edge_ids.len() != request.edges().len() {
            return Err(GraphLayoutError::InvalidProjectionBinding);
        }
        Ok(Self {
            owner,
            entities,
            edges,
        })
    }
    pub fn owner(&self) -> FigureId {
        self.owner
    }
    pub fn routing_space(&self) -> CoordinateSpace {
        CoordinateSpace::ChildContent(self.owner)
    }
    pub fn entities(&self) -> &[(GraphEndpoint, FigureId)] {
        &self.entities
    }
    pub fn edges(&self) -> &[(GraphEdgeId, ConnectionId)] {
        &self.edges
    }
    pub fn figure(&self, entity: GraphEndpoint) -> Result<FigureId, GraphLayoutError> {
        self.entities
            .iter()
            .find(|entry| entry.0 == entity)
            .map(|entry| entry.1)
            .ok_or(GraphLayoutError::InvalidProjectionBinding)
    }
    pub fn connection(&self, edge: GraphEdgeId) -> Result<ConnectionId, GraphLayoutError> {
        self.edges
            .iter()
            .find(|entry| entry.0 == edge)
            .map(|entry| entry.1)
            .ok_or(GraphLayoutError::InvalidProjectionBinding)
    }
    pub fn anchor(
        self: &Arc<Self>,
        request: &GraphLayoutRequest,
        edge: GraphEdgeId,
        end: RouteEnd,
    ) -> Result<GraphProjectionAnchor, GraphLayoutError> {
        let edge_input = request.edge(edge)?;
        let entity = match end {
            RouteEnd::Source => edge_input.source,
            RouteEnd::Target => edge_input.target,
        };
        self.connection(edge)?;
        Ok(GraphProjectionAnchor {
            binding: self.clone(),
            edge,
            end,
            figure: self.figure(entity)?,
        })
    }
}

/// Immutable domain payload; retaining it never grants current-scene routing authority.
pub struct GraphProjectionSnapshot {
    pub(super) request: GraphLayoutRequest,
    pub(super) result: GraphLayoutResult,
    pub(super) binding: Arc<GraphProjectionBinding>,
    pub(super) measurements: Vec<super::GraphProjectionMeasurementResult>,
}
impl GraphProjectionSnapshot {
    pub fn request(&self) -> &GraphLayoutRequest {
        &self.request
    }
    pub fn result(&self) -> &GraphLayoutResult {
        &self.result
    }
    pub fn binding(&self) -> &GraphProjectionBinding {
        &self.binding
    }
    pub fn measurements(&self) -> &[super::GraphProjectionMeasurementResult] {
        &self.measurements
    }
}

fn domain<'a>(
    snapshot: &'a PreparedSceneSnapshot,
    binding: &GraphProjectionBinding,
) -> Result<&'a GraphProjectionSnapshot, RouteError> {
    let domain = snapshot
        .presentation()
        .data::<GraphProjectionSnapshot>()
        .ok_or(RouteError::InvalidGroupOutput)?;
    if domain.binding.as_ref() != binding || domain.result.identity() != domain.request.identity() {
        return Err(RouteError::InvalidGroupOutput);
    }
    Ok(domain)
}

/// Attachment strategy for both standard and external graph algorithms.
pub struct GraphProjectionAnchor {
    binding: Arc<GraphProjectionBinding>,
    edge: GraphEdgeId,
    end: RouteEnd,
    figure: FigureId,
}
impl GraphProjectionAnchor {
    fn site(
        &self,
        scene: &mut dyn RouteSceneQuery,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        let snapshot = scene.prepared_scene(self.binding.owner)?;
        let domain = domain(&snapshot, &self.binding)
            .map_err(|_| AnchorError::InvalidGeometry(self.figure))?;
        let route = domain
            .result
            .route(self.edge)
            .map_err(|_| AnchorError::InvalidGeometry(self.figure))?;
        let attachment = match self.end {
            RouteEnd::Source => route.source,
            RouteEnd::Target => route.target,
        };
        if domain.binding.figure(attachment.endpoint).ok() != Some(self.figure)
            || scene.parent_id(self.figure)? != Some(self.binding.owner)
        {
            return Err(AnchorError::InvalidGeometry(self.figure));
        }
        let local = scene.border_box(self.figure)?;
        let actual = scene.map_rect(
            local,
            CoordinateSpace::FigureLocal(self.figure),
            self.binding.routing_space(),
        )?;
        if domain.result.bounds(attachment.endpoint).ok() != Some(actual) {
            return Err(AnchorError::InvalidGeometry(self.figure));
        }
        let normal = outward_normal(attachment.side);
        Ok(AnchorSite {
            point: scene.map_point(attachment.position, self.binding.routing_space(), output)?,
            outward_normal: Some(scene.map_normal(normal, self.binding.routing_space(), output)?),
        })
    }
}
impl ConnectionAnchor for GraphProjectionAnchor {
    fn owner(&self) -> Option<FigureId> {
        Some(self.figure)
    }
    fn reference_point(
        &self,
        scene: &mut dyn RouteSceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError> {
        Ok(self.site(scene, output)?.point)
    }
    fn location(
        &self,
        scene: &mut dyn RouteSceneQuery,
        _: Point,
        _: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError> {
        self.site(scene, output)
    }
}

/// Complete graph paths are published through the ordinary group-solver protocol.
pub struct GraphProjectionRouter {
    binding: Arc<GraphProjectionBinding>,
}
impl GraphProjectionRouter {
    pub fn new(binding: Arc<GraphProjectionBinding>) -> Self {
        Self { binding }
    }
}
impl ConnectionRouter for GraphProjectionRouter {
    fn prepares_group_endpoints(&self) -> bool {
        true
    }
    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
    fn route(&self, _: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        Err(RouteError::UnsupportedRoutingGroup)
    }
    fn solve_group(
        &self,
        mut input: RouteGroupInput<'_>,
    ) -> Result<Option<RouteGroupOutput>, RouteError> {
        if input.routing_space() != self.binding.routing_space()
            || input.members().len() != self.binding.edges.len()
        {
            return Err(RouteError::InvalidGroupOutput);
        }
        let snapshot = input
            .scene()
            .prepared_scene(self.binding.owner)
            .map_err(RouteError::Scene)?;
        let domain = domain(&snapshot, &self.binding)?;
        let mut routes = Vec::with_capacity(input.members().len());
        for member in input.members() {
            let edge = self
                .binding
                .edges
                .iter()
                .find(|entry| entry.1 == member.connection())
                .ok_or(RouteError::InvalidGroupOutput)?
                .0;
            let route = domain
                .result
                .route(edge)
                .map_err(|_| RouteError::InvalidGroupOutput)?;
            if member.constraint().is_some()
                || route.source.position != member.metadata().source.site.point
                || route.target.position != member.metadata().target.site.point
                || member.metadata().source.site.outward_normal
                    != Some(outward_normal(route.source.side))
                || member.metadata().target.site.outward_normal
                    != Some(outward_normal(route.target.side))
            {
                return Err(RouteError::InvalidGroupOutput);
            }
            routes.push((
                member.connection(),
                RouteOutput::new(
                    PointList::from_points(route.points.clone()),
                    *member.metadata(),
                )?,
            ));
        }
        Ok(Some(RouteGroupOutput::new(routes)))
    }
}

pub(super) fn with_snapshot(
    presentation: FigurePresentation,
    snapshot: Arc<GraphProjectionSnapshot>,
) -> FigurePresentation {
    presentation.with_data(snapshot)
}
fn outward_normal(side: AttachmentSide) -> Vec2 {
    match side {
        AttachmentSide::Top => -Vec2::Y,
        AttachmentSide::Bottom => Vec2::Y,
        AttachmentSide::Left => -Vec2::X,
        AttachmentSide::Right => Vec2::X,
    }
}
