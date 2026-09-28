use std::{
    any::{Any, TypeId},
    error::Error,
    fmt,
};

use novadraw_geometry::{ApproxEq, Point, PointList, Precision, Vec2};

use super::{AnchorError, AnchorSite, ConnectionAnchor, ConnectionId, CoordinateSpace, SceneQuery};

/// Default perpendicular spacing between neighboring Fan routes.
pub const FAN_DEFAULT_SEPARATION: f64 = 16.0;
/// Default logical distance between shared Manhattan lanes.
pub const MANHATTAN_DEFAULT_LANE_SPACING: f64 = 8.0;
/// Default minimum length retained for endpoint-adjacent Manhattan stubs.
pub const MANHATTAN_DEFAULT_MINIMUM_STUB: f64 = 10.0;

/// Scope used by Routers with cross-connection behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RoutingGroupScope {
    /// Router calculations are independent.
    #[default]
    None,
    /// Members share an unordered source/target Anchor pair.
    AnchorPair,
    /// All members using one RouterId in one routing domain share state.
    RoutingDomain,
}

/// Router-specific constraint with checked runtime type information.
pub trait RoutingConstraint: Any {
    /// Returns this constraint as `Any` for checked downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns the concrete Rust type name for diagnostics.
    fn type_name(&self) -> &'static str;
}

impl<T: Any> RoutingConstraint for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Read-only group context for routers with cross-connection behavior.
pub trait RoutingGroupQuery {
    /// Returns connections in stable routing order.
    fn ordered_connections(&self) -> &[ConnectionId];

    /// Returns an already computed route when available.
    fn route(&self, connection: ConnectionId) -> Option<&RouteOutput>;
}

/// Immutable input to one Router calculation.
pub struct RouteRequest<'a> {
    /// Connection being routed.
    pub connection: ConnectionId,
    /// Canonical output domain.
    pub routing_space: CoordinateSpace,
    /// Source endpoint strategy.
    pub source: &'a dyn ConnectionAnchor,
    /// Target endpoint strategy.
    pub target: &'a dyn ConnectionAnchor,
    /// Optional router-specific constraint.
    pub constraint: Option<&'a dyn RoutingConstraint>,
    /// Read-only, dependency-tracking scene query.
    pub scene: &'a mut dyn SceneQuery,
    /// Optional stable group snapshot.
    pub group: Option<&'a dyn RoutingGroupQuery>,
}

/// Endpoint metadata retained with a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteEndpoint {
    /// Reference point supplied to the Anchor.
    pub reference: Point,
    /// Resolved endpoint and optional outward normal.
    pub site: AnchorSite,
}

/// Metadata produced with a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteMetadata {
    /// Resolved source endpoint.
    pub source: RouteEndpoint,
    /// Resolved target endpoint.
    pub target: RouteEndpoint,
}

/// Endpoint side used in structured route diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteEnd {
    /// Source endpoint.
    Source,
    /// Target endpoint.
    Target,
}

/// Validated output of a pure Router calculation.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteOutput {
    points: PointList,
    metadata: RouteMetadata,
}

impl RouteOutput {
    /// Creates a validated route.
    pub fn new(points: PointList, metadata: RouteMetadata) -> Result<Self, RouteError> {
        if points.len() < 2 {
            return Err(RouteError::TooFewPoints {
                point_count: points.len(),
            });
        }
        for (index, point) in points.iter().enumerate() {
            if !finite_point(*point) {
                return Err(RouteError::NonFinitePoint { index });
            }
        }
        validate_endpoint_metadata(RouteEnd::Source, metadata.source)?;
        validate_endpoint_metadata(RouteEnd::Target, metadata.target)?;
        if !points
            .get(0)
            .is_some_and(|point| point.approx_eq(metadata.source.site.point, Precision::DEFAULT))
        {
            return Err(RouteError::EndpointMismatch {
                endpoint: RouteEnd::Source,
            });
        }
        if !points
            .get(points.len() - 1)
            .is_some_and(|point| point.approx_eq(metadata.target.site.point, Precision::DEFAULT))
        {
            return Err(RouteError::EndpointMismatch {
                endpoint: RouteEnd::Target,
            });
        }
        Ok(Self { points, metadata })
    }

    /// Returns canonical route points.
    pub fn points(&self) -> &PointList {
        &self.points
    }

    /// Returns endpoint metadata.
    pub fn metadata(&self) -> &RouteMetadata {
        &self.metadata
    }
}

/// Pure routing strategy.
pub trait ConnectionRouter {
    /// Computes a complete route without mutating scene or runtime state.
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError>;

    /// Returns the supported concrete constraint type, if one is required.
    fn constraint_type(&self) -> Option<TypeId> {
        None
    }

    /// Returns the supported concrete constraint type name for diagnostics.
    fn constraint_type_name(&self) -> Option<&'static str> {
        None
    }

    /// Returns whether this Router requires a stable cross-connection group.
    fn requires_group(&self) -> bool {
        false
    }

    /// Returns the stable scope required by this Router.
    fn routing_group_scope(&self) -> RoutingGroupScope {
        if self.requires_group() {
            RoutingGroupScope::AnchorPair
        } else {
            RoutingGroupScope::None
        }
    }
}

/// Direct two-point Router equivalent to Draw2D's NullConnectionRouter.
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectRouter;

impl ConnectionRouter for DirectRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        if let Some(constraint) = request.constraint {
            return Err(RouteError::ConstraintTypeMismatch {
                expected: None,
                actual: constraint.type_name(),
            });
        }

        let metadata = resolve_endpoints(
            request.source,
            request.target,
            request.scene,
            request.routing_space,
            None,
            None,
        )?;

        RouteOutput::new(
            PointList::from_points(vec![metadata.source.site.point, metadata.target.site.point]),
            metadata,
        )
    }
}

/// One Bendpoint in the Connection routing domain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bendpoint {
    /// Fixed point in the canonical routing domain.
    Absolute(Point),
    /// Point derived from source/target references and endpoint-relative offsets.
    Relative {
        /// Offset from the source reference.
        source_offset: Vec2,
        /// Offset from the target reference.
        target_offset: Vec2,
        /// Interpolation weight in the inclusive range `[0, 1]`.
        weight: f64,
    },
}

/// Ordered Bendpoint list owned by one Connection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BendpointConstraint {
    bendpoints: Vec<Bendpoint>,
}

impl BendpointConstraint {
    /// Creates an ordered Bendpoint constraint.
    pub fn new(bendpoints: impl Into<Vec<Bendpoint>>) -> Self {
        Self {
            bendpoints: bendpoints.into(),
        }
    }

    /// Returns Bendpoints in route order.
    pub fn bendpoints(&self) -> &[Bendpoint] {
        &self.bendpoints
    }
}

/// Router that preserves an explicit ordered Bendpoint constraint.
#[derive(Clone, Copy, Debug, Default)]
pub struct BendpointConnectionRouter;

impl ConnectionRouter for BendpointConnectionRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        let constraint = match request.constraint {
            None => None,
            Some(constraint) => Some(
                constraint
                    .as_any()
                    .downcast_ref::<BendpointConstraint>()
                    .ok_or(RouteError::ConstraintTypeMismatch {
                        expected: Some(std::any::type_name::<BendpointConstraint>()),
                        actual: constraint.type_name(),
                    })?,
            ),
        };

        let source_reference = request
            .target
            .reference_point(request.scene, request.routing_space)
            .map_err(RouteError::Target)?;
        let target_reference = request
            .source
            .reference_point(request.scene, request.routing_space)
            .map_err(RouteError::Source)?;
        let bendpoints = constraint
            .map(|constraint| {
                constraint
                    .bendpoints()
                    .iter()
                    .map(|bendpoint| {
                        resolve_bendpoint(*bendpoint, target_reference, source_reference)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let metadata = resolve_endpoints(
            request.source,
            request.target,
            request.scene,
            request.routing_space,
            bendpoints.first().copied(),
            bendpoints.last().copied(),
        )?;
        let mut points = Vec::with_capacity(bendpoints.len() + 2);
        points.push(metadata.source.site.point);
        points.extend(bendpoints);
        points.push(metadata.target.site.point);
        RouteOutput::new(PointList::from_points(points), metadata)
    }

    fn constraint_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<BendpointConstraint>())
    }

    fn constraint_type_name(&self) -> Option<&'static str> {
        Some(std::any::type_name::<BendpointConstraint>())
    }
}

/// Deterministic orthogonal Router without obstacle avoidance.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManhattanConnectionRouter;

impl ConnectionRouter for ManhattanConnectionRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        if let Some(constraint) = request.constraint {
            return Err(RouteError::ConstraintTypeMismatch {
                expected: None,
                actual: constraint.type_name(),
            });
        }
        let metadata = resolve_endpoints(
            request.source,
            request.target,
            request.scene,
            request.routing_space,
            None,
            None,
        )?;
        let source = metadata.source.site.point;
        let target = metadata.target.site.point;
        let source_horizontal = metadata.source.site.outward_normal.map(horizontal_normal);
        let target_horizontal = metadata.target.site.outward_normal.map(horizontal_normal);
        let mut points = vec![source];
        if source.x() != target.x() && source.y() != target.y() {
            match (source_horizontal, target_horizontal) {
                (Some(true), Some(false)) => {
                    points.push(Point::new(target.x(), source.y()));
                }
                (Some(false), Some(true)) => {
                    points.push(Point::new(source.x(), target.y()));
                }
                (Some(true), _) | (_, Some(true)) => {
                    let middle_x = (source.x() + target.x()) / 2.0;
                    points.push(Point::new(middle_x, source.y()));
                    points.push(Point::new(middle_x, target.y()));
                }
                _ => {
                    let middle_y = (source.y() + target.y()) / 2.0;
                    points.push(Point::new(source.x(), middle_y));
                    points.push(Point::new(target.x(), middle_y));
                }
            }
        }
        points.push(target);
        remove_adjacent_duplicates(&mut points);
        let group = request.group.ok_or(RouteError::UnsupportedRoutingGroup)?;
        reserve_manhattan_lanes(&mut points, group, MANHATTAN_DEFAULT_LANE_SPACING);
        remove_adjacent_duplicates(&mut points);
        RouteOutput::new(PointList::from_points(points), metadata)
    }

    fn requires_group(&self) -> bool {
        true
    }

    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
}

/// Post-processing Router that separates parallel two-point routes.
pub struct FanRouter {
    base: Box<dyn ConnectionRouter>,
    separation: f64,
}

impl FanRouter {
    /// Creates a Fan Router over another Router.
    pub fn new(base: Box<dyn ConnectionRouter>, separation: f64) -> Result<Self, FanRouterError> {
        if !separation.is_finite() || separation <= 0.0 {
            return Err(FanRouterError);
        }
        Ok(Self { base, separation })
    }
}

impl ConnectionRouter for FanRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        let group = request.group.ok_or(RouteError::UnsupportedRoutingGroup)?;
        let connection = request.connection;
        let base = self.base.route(RouteRequest {
            connection,
            routing_space: request.routing_space,
            source: request.source,
            target: request.target,
            constraint: request.constraint,
            scene: request.scene,
            group: None,
        })?;
        if base.points().len() != 2 || group.ordered_connections().len() <= 1 {
            return Ok(base);
        }
        let Some(index) = group
            .ordered_connections()
            .iter()
            .position(|candidate| *candidate == connection)
        else {
            return Err(RouteError::UnsupportedRoutingGroup);
        };
        let source = base.points().get(0).ok_or(RouteError::TooFewPoints {
            point_count: base.points().len(),
        })?;
        let target =
            base.points()
                .get(base.points().len() - 1)
                .ok_or(RouteError::TooFewPoints {
                    point_count: base.points().len(),
                })?;
        let direction = target - source;
        let length = direction.length();
        if length <= f64::EPSILON {
            return Ok(base);
        }
        let direction = canonical_fan_direction(direction);
        let centered_index = index as f64 - (group.ordered_connections().len() as f64 - 1.0) / 2.0;
        if centered_index.abs() <= f64::EPSILON {
            return Ok(base);
        }
        let perpendicular = Vec2::new(-direction.y() / length, direction.x() / length);
        let midpoint = Point::new(
            (source.x() + target.x()) / 2.0,
            (source.y() + target.y()) / 2.0,
        ) + perpendicular * (centered_index * self.separation);
        RouteOutput::new(
            PointList::from_points(vec![source, midpoint, target]),
            *base.metadata(),
        )
    }

    fn constraint_type(&self) -> Option<TypeId> {
        self.base.constraint_type()
    }

    fn constraint_type_name(&self) -> Option<&'static str> {
        self.base.constraint_type_name()
    }

    fn requires_group(&self) -> bool {
        true
    }
}

fn canonical_fan_direction(direction: Vec2) -> Vec2 {
    let points_west = direction.x().abs() > direction.y().abs() && direction.x() < 0.0;
    let points_north = direction.x().abs() <= direction.y().abs() && direction.y() < 0.0;
    if points_west || points_north {
        direction
    } else {
        Vec2::new(-direction.x(), -direction.y())
    }
}

/// Failure to construct a Fan Router.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanRouterError;

impl fmt::Display for FanRouterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fan separation must be finite and greater than zero"
        )
    }
}

impl Error for FanRouterError {}

fn resolve_endpoints(
    source: &dyn ConnectionAnchor,
    target: &dyn ConnectionAnchor,
    scene: &mut dyn SceneQuery,
    routing_space: CoordinateSpace,
    source_reference_override: Option<Point>,
    target_reference_override: Option<Point>,
) -> Result<RouteMetadata, RouteError> {
    let source_reference = match source_reference_override {
        Some(reference) => reference,
        None => target
            .reference_point(scene, routing_space)
            .map_err(RouteError::Target)?,
    };
    let target_reference = match target_reference_override {
        Some(reference) => reference,
        None => source
            .reference_point(scene, routing_space)
            .map_err(RouteError::Source)?,
    };
    let source_site = source
        .location(scene, source_reference, routing_space, routing_space)
        .map_err(RouteError::Source)?;
    let target_site = target
        .location(scene, target_reference, routing_space, routing_space)
        .map_err(RouteError::Target)?;
    Ok(RouteMetadata {
        source: RouteEndpoint {
            reference: source_reference,
            site: source_site,
        },
        target: RouteEndpoint {
            reference: target_reference,
            site: target_site,
        },
    })
}

fn resolve_bendpoint(
    bendpoint: Bendpoint,
    source_reference: Point,
    target_reference: Point,
) -> Result<Point, RouteError> {
    match bendpoint {
        Bendpoint::Absolute(point) if finite_point(point) => Ok(point),
        Bendpoint::Absolute(_) => Err(RouteError::InvalidConstraint),
        Bendpoint::Relative {
            source_offset,
            target_offset,
            weight,
        } => {
            if !weight.is_finite()
                || !(0.0..=1.0).contains(&weight)
                || !finite_vector(source_offset)
                || !finite_vector(target_offset)
            {
                return Err(RouteError::InvalidConstraint);
            }
            let source = source_reference + source_offset;
            let target = target_reference + target_offset;
            Ok(Point::new(
                source.x() * (1.0 - weight) + target.x() * weight,
                source.y() * (1.0 - weight) + target.y() * weight,
            ))
        }
    }
}

fn horizontal_normal(normal: Vec2) -> bool {
    normal.x().abs() >= normal.y().abs()
}

fn remove_adjacent_duplicates(points: &mut Vec<Point>) {
    points.dedup_by(|left, right| left.approx_eq(*right, Precision::DEFAULT));
}

fn reserve_manhattan_lanes(points: &mut [Point], group: &dyn RoutingGroupQuery, lane_spacing: f64) {
    if points.len() < 4 {
        return;
    }
    let mut rows = Vec::new();
    let mut columns = Vec::new();
    for connection in group.ordered_connections() {
        let Some(route) = group.route(*connection) else {
            continue;
        };
        collect_internal_lanes(route.points().as_slice(), &mut rows, &mut columns);
    }

    for segment_index in 1..points.len() - 2 {
        let start = points[segment_index];
        let end = points[segment_index + 1];
        if start.y().approx_eq(end.y(), Precision::DEFAULT) {
            let adjacent = [points[segment_index - 1].y(), points[segment_index + 2].y()];
            let lane = nearest_available_lane(
                start.y(),
                &rows,
                adjacent,
                lane_spacing,
                MANHATTAN_DEFAULT_MINIMUM_STUB,
            );
            points[segment_index] = Point::new(start.x(), lane);
            points[segment_index + 1] = Point::new(end.x(), lane);
            rows.push(lane);
        } else if start.x().approx_eq(end.x(), Precision::DEFAULT) {
            let adjacent = [points[segment_index - 1].x(), points[segment_index + 2].x()];
            let lane = nearest_available_lane(
                start.x(),
                &columns,
                adjacent,
                lane_spacing,
                MANHATTAN_DEFAULT_MINIMUM_STUB,
            );
            points[segment_index] = Point::new(lane, start.y());
            points[segment_index + 1] = Point::new(lane, end.y());
            columns.push(lane);
        }
    }
}

fn collect_internal_lanes(points: &[Point], rows: &mut Vec<f64>, columns: &mut Vec<f64>) {
    if points.len() < 4 {
        return;
    }
    for segment_index in 1..points.len() - 2 {
        let start = points[segment_index];
        let end = points[segment_index + 1];
        if start.y().approx_eq(end.y(), Precision::DEFAULT) {
            rows.push(start.y());
        } else if start.x().approx_eq(end.x(), Precision::DEFAULT) {
            columns.push(start.x());
        }
    }
}

fn nearest_available_lane(
    preferred: f64,
    occupied: &[f64],
    adjacent: [f64; 2],
    spacing: f64,
    minimum_stub: f64,
) -> f64 {
    let available = |candidate: f64| {
        occupied
            .iter()
            .all(|lane| (candidate - lane).abs() + f64::EPSILON >= spacing)
            && adjacent
                .iter()
                .all(|endpoint| (candidate - endpoint).abs() + f64::EPSILON >= minimum_stub)
    };
    if available(preferred) {
        return preferred;
    }
    let geometry_steps = adjacent
        .iter()
        .map(|endpoint| ((preferred - endpoint).abs() + minimum_stub) / spacing)
        .fold(0.0, f64::max)
        .ceil() as usize;
    for distance in 1..=occupied.len() + geometry_steps + 1 {
        let offset = distance as f64 * spacing;
        let lower = preferred - offset;
        if available(lower) {
            return lower;
        }
        let upper = preferred + offset;
        if available(upper) {
            return upper;
        }
    }
    unreachable!("finite Manhattan lane constraints must leave an available lane")
}

/// Failure produced by a Router calculation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteError {
    /// Source Anchor resolution failed.
    Source(AnchorError),
    /// Target Anchor resolution failed.
    Target(AnchorError),
    /// The supplied constraint type is not accepted by this Router.
    ConstraintTypeMismatch {
        /// Expected concrete type, or `None` when constraints are unsupported.
        expected: Option<&'static str>,
        /// Actual concrete type.
        actual: &'static str,
    },
    /// The constraint value is semantically invalid.
    InvalidConstraint,
    /// A Router returned fewer than two points.
    TooFewPoints {
        /// Number of returned points.
        point_count: usize,
    },
    /// A Router returned a non-finite point.
    NonFinitePoint {
        /// Index of the invalid point.
        index: usize,
    },
    /// Anchor metadata contains a non-finite reference point or normal.
    NonFiniteEndpointMetadata {
        /// Invalid endpoint.
        endpoint: RouteEnd,
    },
    /// First or last route point does not match resolved endpoint metadata.
    EndpointMismatch {
        /// Mismatched endpoint.
        endpoint: RouteEnd,
    },
    /// Required group context is unavailable or incompatible.
    UnsupportedRoutingGroup,
    /// Endpoint topology cannot be represented by the active clipping policy.
    UnsupportedViewportTopology,
    /// Anchor, route, or locator dependencies contain a cycle.
    DependencyCycle,
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source Anchor failed: {error}"),
            Self::Target(error) => write!(formatter, "target Anchor failed: {error}"),
            Self::ConstraintTypeMismatch { expected, actual } => match expected {
                Some(expected) => {
                    write!(
                        formatter,
                        "expected routing constraint {expected}, got {actual}"
                    )
                }
                None => write!(formatter, "router does not accept constraint {actual}"),
            },
            Self::InvalidConstraint => write!(formatter, "invalid routing constraint"),
            Self::TooFewPoints { point_count } => {
                write!(
                    formatter,
                    "route requires at least two points, got {point_count}"
                )
            }
            Self::NonFinitePoint { index } => {
                write!(formatter, "route point at index {index} is not finite")
            }
            Self::NonFiniteEndpointMetadata { endpoint } => {
                write!(formatter, "{endpoint:?} route metadata is not finite")
            }
            Self::EndpointMismatch { endpoint } => {
                write!(
                    formatter,
                    "{endpoint:?} route point does not match Anchor metadata"
                )
            }
            Self::UnsupportedRoutingGroup => write!(formatter, "unsupported routing group"),
            Self::UnsupportedViewportTopology => {
                write!(formatter, "unsupported endpoint viewport topology")
            }
            Self::DependencyCycle => write!(formatter, "connection dependency cycle detected"),
        }
    }
}

fn validate_endpoint_metadata(
    endpoint: RouteEnd,
    metadata: RouteEndpoint,
) -> Result<(), RouteError> {
    let normal_is_finite = metadata.site.outward_normal.is_none_or(finite_vector);
    if finite_point(metadata.reference) && finite_point(metadata.site.point) && normal_is_finite {
        Ok(())
    } else {
        Err(RouteError::NonFiniteEndpointMetadata { endpoint })
    }
}

fn finite_point(point: Point) -> bool {
    point.x().is_finite() && point.y().is_finite()
}

fn finite_vector(vector: Vec2) -> bool {
    vector.x().is_finite() && vector.y().is_finite()
}

impl Error for RouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(error) | Self::Target(error) => Some(error),
            _ => None,
        }
    }
}
