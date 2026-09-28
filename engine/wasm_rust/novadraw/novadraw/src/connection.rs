//! Connection figures, anchors, routers, locators, and runtime bindings.

pub use novadraw_scene::{
    AnchorError, AnchorGeometry, AnchorGeometryKey, AnchorGeometryKeyError, AnchorGroupKey,
    AnchorId, AnchorSemanticKey, AnchorSemanticKeyError, AnchorSite, Bendpoint,
    BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionAnchor,
    ConnectionFigure, ConnectionFigureBehavior, ConnectionGeometryError, ConnectionId,
    ConnectionLayerFigure, ConnectionLocator, ConnectionLocatorStrategy, ConnectionResolution,
    ConnectionRouter, ConnectionRuntimeError, ConnectionStateSnapshot, CoordinateSpace,
    DependencyObservation, DependencySubject, DirectRouter, EllipseAnchor, FAN_DEFAULT_SEPARATION,
    FanRouter, FanRouterError, LabelAnchor, LocatorError, LocatorPlacement,
    MANHATTAN_DEFAULT_LANE_SPACING, MANHATTAN_DEFAULT_MINIMUM_STUB, ManhattanConnectionRouter,
    MidpointLocator, PathFractionLocator, PreparedConnectionGeometry, RoundedRectangleAnchor,
    RouteEnd, RouteEndpoint, RouteError, RouteMetadata, RouteOutput, RouteRequest, RouterBinding,
    RouterId, RoutingConstraint, RoutingGroupQuery, RoutingGroupScope, SceneQuery, SceneQueryError,
    SceneRead, TrackedSceneQuery, UnresolvedConnection, XYAnchor, rectangle_boundary_site,
};
