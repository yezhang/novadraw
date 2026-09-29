//! Pure connection, anchor, and routing contracts.

mod anchor;
mod decoration;
mod figure;
mod locator;
mod query;
mod router;
mod runtime;
mod shortest_path;

use crate::FigureId;

pub use anchor::{
    AnchorError, AnchorGroupKey, AnchorSemanticKey, AnchorSemanticKeyError, AnchorSite,
    ChopboxAnchor, ConnectionAnchor, EllipseAnchor, LabelAnchor, RoundedRectangleAnchor, XYAnchor,
    rectangle_boundary_site,
};
pub use decoration::{
    ConnectionDecorationBehavior, DecorationError, PolygonDecorationFigure,
    PolylineDecorationFigure, PreparedDecorationGeometry,
};
pub use figure::{
    ConnectionFigure, ConnectionFigureBehavior, ConnectionGeometryError, ConnectionLayerFigure,
    PreparedConnectionGeometry,
};
pub use locator::{
    ConnectionLocator, ConnectionLocatorStrategy, EndpointLocator, LocatorError, LocatorPlacement,
    MidpointLocator, PathFractionLocator,
};
pub(crate) use query::FigureTreeSceneRead;
pub use query::{
    AnchorGeometry, AnchorGeometryKey, AnchorGeometryKeyError, CoordinateSpace,
    DependencyObservation, DependencySubject, SceneQuery, SceneQueryError, SceneRead,
    TrackedSceneQuery,
};
pub use router::{
    Bendpoint, BendpointConnectionRouter, BendpointConstraint, ConnectionRouter, DirectRouter,
    FAN_DEFAULT_SEPARATION, FanRouter, FanRouterError, MANHATTAN_DEFAULT_LANE_SPACING,
    MANHATTAN_DEFAULT_MINIMUM_STUB, ManhattanConnectionRouter, RouteEnd, RouteEndpoint, RouteError,
    RouteMetadata, RouteOutput, RouteRequest, RoutingConstraint, RoutingGroupQuery,
    RoutingGroupScope, RoutingObstacle,
};
pub(crate) use runtime::ConnectionRuntime;
pub use runtime::{
    ConnectionResolution, ConnectionRoutingStats, ConnectionRuntimeError, ConnectionStateSnapshot,
    RouterBinding, UnresolvedConnection,
};
pub use shortest_path::{
    SHORTEST_PATH_DEFAULT_BEND_PENALTY, SHORTEST_PATH_DEFAULT_CLEARANCE,
    SHORTEST_PATH_DEFAULT_MINIMUM_STUB, ShortestPathConnectionRouter, ShortestPathRouterError,
};

pub use crate::identity::{AnchorId, RouterId};

/// Type-safe identity of a Figure carrying Connection behavior.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConnectionId(FigureId);

impl ConnectionId {
    /// Creates a connection identity from its backing Figure identity.
    ///
    /// M9.2 Runtime APIs must verify that the Figure exposes Connection behavior
    /// before constructing this value for callers.
    pub const fn from_figure(figure: FigureId) -> Self {
        Self(figure)
    }

    /// Returns the backing Figure identity.
    pub const fn figure(self) -> FigureId {
        self.0
    }
}
