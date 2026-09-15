use std::any::TypeId;

use novadraw::{
    ConnectionRouter, Point, RouteEndpoint, RouteError, RouteMetadata, RouteOutput, RouteRequest,
    RoutingGroupScope,
};
use novadraw_geometry::{PointList, Vector};

/// Demo policy that turns same-owner routes into a visible rectangular loop.
pub struct VisibleSelfLoopRouter {
    base: Box<dyn ConnectionRouter>,
    extent: f64,
}

impl VisibleSelfLoopRouter {
    pub fn new(base: Box<dyn ConnectionRouter>, extent: f64) -> Option<Self> {
        (extent.is_finite() && extent > 0.0).then_some(Self { base, extent })
    }
}

impl ConnectionRouter for VisibleSelfLoopRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        let RouteRequest {
            connection,
            routing_space,
            source,
            target,
            constraint,
            scene,
            group,
        } = request;
        let same_owner = source
            .owner()
            .zip(target.owner())
            .is_some_and(|(source, target)| source == target);
        let base = self.base.route(RouteRequest {
            connection,
            routing_space,
            source,
            target,
            constraint,
            scene: &mut *scene,
            group,
        })?;
        if !same_owner {
            return Ok(base);
        }

        let center = source
            .reference_point(scene, routing_space)
            .map_err(RouteError::Source)?;
        let source_reference = center + Vector::new(self.extent, -self.extent / 2.0);
        let target_reference = center + Vector::new(self.extent, self.extent / 2.0);
        let source_site = source
            .location(scene, source_reference, routing_space, routing_space)
            .map_err(RouteError::Source)?;
        let target_site = target
            .location(scene, target_reference, routing_space, routing_space)
            .map_err(RouteError::Target)?;
        let metadata = RouteMetadata {
            source: RouteEndpoint {
                reference: source_reference,
                site: source_site,
            },
            target: RouteEndpoint {
                reference: target_reference,
                site: target_site,
            },
        };
        let mut points = base.points().clone().into_vec();
        points[0] = source_site.point;
        let last = points.len() - 1;
        points[last] = target_site.point;
        if points.len() == 2 {
            let outer_x = source_site.point.x().max(target_site.point.x()) + self.extent;
            points = vec![
                source_site.point,
                Point::new(outer_x, source_site.point.y()),
                Point::new(outer_x, target_site.point.y()),
                target_site.point,
            ];
        }
        RouteOutput::new(PointList::from_points(points), metadata)
    }

    fn constraint_type(&self) -> Option<TypeId> {
        self.base.constraint_type()
    }

    fn constraint_type_name(&self) -> Option<&'static str> {
        self.base.constraint_type_name()
    }

    fn requires_group(&self) -> bool {
        self.base.requires_group()
    }

    fn routing_group_scope(&self) -> RoutingGroupScope {
        self.base.routing_group_scope()
    }
}
