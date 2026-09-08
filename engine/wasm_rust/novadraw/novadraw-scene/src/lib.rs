//! Novadraw Scene Library
//!
//! 场景图库，提供 Figure 渲染和场景图管理。
//!
//! # 模块
//!
//! - [`figure`] - Figure 渲染接口和实现
//! - [`graph`] - FigureTree、图结构和渲染/命中测试集成
//! - [`runtime`] - 事件、上下文、更新、延迟结构变更和组合根协议
//! - [`container`] - Viewport 等 Figure 级容器
//! - [`host`] - 平台宿主与渲染入口协调

#![allow(missing_docs)]

pub mod connection;
pub mod container;
pub mod figure;
pub mod graph;
pub mod host;
pub mod layout;
pub mod log;
pub mod runtime;
pub mod style;

pub use connection::{
    AnchorError, AnchorGeometry, AnchorGeometryKey, AnchorGeometryKeyError, AnchorGroupKey,
    AnchorId, AnchorSemanticKey, AnchorSemanticKeyError, AnchorSite, Bendpoint,
    BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionAnchor,
    ConnectionFigure, ConnectionFigureBehavior, ConnectionId, ConnectionLayerFigure,
    ConnectionLocator, ConnectionLocatorStrategy, ConnectionResolution, ConnectionRouter,
    ConnectionRuntimeError, ConnectionStateSnapshot, CoordinateSpace, DependencyObservation,
    DependencySubject, DirectRouter, EllipseAnchor, FAN_DEFAULT_SEPARATION, FanRouter,
    FanRouterError, LabelAnchor, LocatorError, LocatorPlacement, MANHATTAN_DEFAULT_LANE_SPACING,
    MANHATTAN_DEFAULT_MINIMUM_STUB, ManhattanConnectionRouter, MidpointLocator,
    PathFractionLocator, RoundedRectangleAnchor, RouteEnd, RouteEndpoint, RouteError,
    RouteMetadata, RouteOutput, RouteRequest, RouterBinding, RouterId, RoutingConstraint,
    RoutingGroupQuery, RoutingGroupScope, SceneQuery, SceneQueryError, SceneRead,
    TrackedSceneQuery, UnresolvedConnection, XYAnchor,
};
pub use container::viewport;
pub use container::{
    DEFAULT_ZOOM_LEVELS, DefaultRangeModel, DefaultScrollPolicy, FreeformLayerFigure,
    FreeformLayeredPane, LayerError, LayerFigure, LayerKey, LayerKeyError, LayerPlacement,
    LayeredPane, LayeredPaneHandle, MouseLocationZoomScrollPolicy, RangeChange, RangeChangeSet,
    RangeListener, RangeListenerId, RangeModel, RangeModelError, RangeModelSnapshot, RangeProperty,
    ScalableFigure, ScalableFreeformLayeredPane, ScalableLayeredPaneFigure, ScaleError,
    ScaleHandle, ScrollBarFigure, ScrollBarVisibility, ScrollOrientation, ScrollPaneError,
    ScrollPaneFigure, ScrollPaneHandle, ScrollPaneLayout, ZoomError, ZoomManager, ZoomScrollPolicy,
    ZoomViewportState,
};
pub use figure::border;
pub use figure::border::{
    BevelBorder, BevelStyle, Border, CompoundBorder, EtchedBorder, LineBorder, MarginBorder,
    RectangleBorder, TitleBarBorder,
};
pub use figure::{
    AccessibleFigure, Alignment, AsAny, BorderedFigure, Bounded, ButtonFigure,
    ChildClippingStrategy, ChildPolicy, ChildTransform, ClickableBehavior, ClickableFigure,
    ClickableKind, ClickableModel, ClickableSnapshot, ClickableVisualState, Direction,
    EllipseFigure, Figure, FigureContainer, FigureEventHandler, FigureLifecycle, Freeform,
    HitParticipation, ImageDisplayState, ImageFigure, LabelFigure, Layer, PointListFigureBehavior,
    PolygonFigure, PolylineFigure, RectangleFigure, RootFigure, RoundedRectangleFigure, Shape,
    ShapeMutationError, TextPlacement, ToggleFigure, TriangleFigure, WidgetError,
};
pub use graph as scene;
pub use graph::{
    DEFAULT_VALIDATION_BUDGET, ExclusionSearch, FREEFORM_EXTENT_PROPERTY, FigureId, FigureNode,
    FigureTree, FigureTreeBuilder, FreeformError, FreeformState, GraphMutationError,
    IdentitySearch, LayoutState, MAX_TREE_DEPTH, NodeState, TreeQueryError, TreeSearch,
    TreeSearchContext, ValidationError,
};
pub use host::{AccessibilityUpdate, HeadlessHost, ImeState, PlatformHost};
pub use layout::{
    BorderConstraint, BorderLayout, BorderRegion, FillLayout, FlowDirection, FlowLayout,
    FreeformConstraint, FreeformConstraintError, FreeformLayout, GridAlignment, GridConstraint,
    GridLayout, LayoutConstraint, LayoutError, LayoutInvalidation, LayoutManager, LayoutOutput,
    LayoutSnapshot, MinorAlignment, StackLayout, ToolbarLayout, ToolbarOrientation, XYConstraint,
    XYLayout,
};
pub use novadraw_geometry::{Point, Rectangle};
pub use runtime::context::{EventContext, SceneDispatchContext};
pub use runtime::event::{
    DispatchContext, Event, EventDispatcher, FocusEvent, FocusEventKind, GesturePhase,
    GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
pub use runtime::mutation::PendingMutations;
pub use runtime::update;
pub use runtime::update::{
    ActionEvent, ActionListener, AncestorEvent, AncestorEventKind, AncestorListener,
    CoordinateListener, FigureEvent, FigureListener, LayoutEvent, LayoutEventKind, LayoutListener,
    ListenerId, NotificationEffect, NotificationQueue, PropertyChangeEvent, PropertyChangeListener,
    PropertyValue, UpdateEvent, UpdateListener, UpdateManager, ValidatingListener,
};
pub use runtime::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    FontId, ImageId, InteractionState, PointerId, ResourceError, ResourceKind, ResourceRegistry,
    ResourceStatus, Runtime, TreeOrderFocusTraversal,
};
pub use runtime::{context, event, mutation};
pub use style::{CursorIcon, FigureStyle, ResolvedStyle};
pub use viewport::{ViewportError, ViewportFigure, ViewportHandle, ViewportLayout};
