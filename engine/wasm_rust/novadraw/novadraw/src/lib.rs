#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod color;
#[allow(missing_docs)]
mod graph;
#[allow(missing_docs)]
mod identity;
#[allow(missing_docs)]
mod log;
#[allow(missing_docs)]
mod style;

/// Low-level types for diagnostics and deep engine integration.
pub mod advanced;
/// Connection, anchor, router, and locator APIs.
#[allow(missing_docs)]
pub mod connection;
/// Viewport, scrolling, zoom, layering, and freeform containers.
#[allow(missing_docs)]
pub mod container;
/// Input events, listeners, focus, tooltip, and accessibility APIs.
pub mod event;
/// Figure traits, built-in figures, borders, and styles.
#[allow(missing_docs)]
pub mod figure;
/// Platform-independent geometry values and operations.
pub mod geometry;
/// Stateful drawing API and path primitives.
pub mod graphics;
/// Platform host integration.
#[allow(missing_docs)]
pub mod host;
/// Layout extension protocols and built-in layouts.
#[allow(missing_docs)]
pub mod layout;
/// Common imports for Figure and Runtime application code.
pub mod prelude;
/// Backend-neutral rendering, text, resource, and submission protocols.
pub mod render;
/// Runtime lifecycle, mutation, resource, and frame APIs.
#[allow(missing_docs)]
pub mod runtime;
/// Text measurement, immutable layouts and glyph outline extensions.
pub mod text;
/// Figure tree construction and query APIs.
pub mod tree;

pub use color::{Color, ColorError, ParseColorError};
pub use connection::{
    AnchorError, AnchorGeometry, AnchorGeometryKey, AnchorGeometryKeyError, AnchorGroupKey,
    AnchorId, AnchorSemanticKey, AnchorSemanticKeyError, AnchorSite, Bendpoint,
    BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, ConnectionAnchor,
    ConnectionDecorationBehavior, ConnectionFigure, ConnectionFigureBehavior,
    ConnectionGeometryError, ConnectionId, ConnectionLayerFigure, ConnectionLocator,
    ConnectionLocatorStrategy, ConnectionResolution, ConnectionRouter, ConnectionRuntimeError,
    ConnectionStateSnapshot, CoordinateSpace, DecorationError, DirectRouter, EllipseAnchor,
    EndpointLocator, FAN_DEFAULT_SEPARATION, FanRouter, FanRouterError, LabelAnchor, LocatorError,
    LocatorPlacement, MANHATTAN_DEFAULT_LANE_SPACING, MANHATTAN_DEFAULT_MINIMUM_STUB,
    ManhattanConnectionRouter, MidpointLocator, PathFractionLocator, PolygonDecorationFigure,
    PolylineDecorationFigure, RoundedRectangleAnchor, RouteEnd, RouteEndpoint, RouteError,
    RouterBinding, RouterId, RoutingConstraint, RoutingGroupScope,
    SHORTEST_PATH_DEFAULT_BEND_PENALTY, SHORTEST_PATH_DEFAULT_CLEARANCE,
    SHORTEST_PATH_DEFAULT_MINIMUM_STUB, ShortestPathConnectionRouter, ShortestPathRouterError,
    XYAnchor, rectangle_boundary_site,
};
pub use container::viewport;
pub use container::{
    DEFAULT_ZOOM_LEVELS, DefaultRangeModel, DefaultScrollPolicy, FreeformLayerFigure,
    FreeformLayeredPane, LayerError, LayerFigure, LayerKey, LayerKeyError, LayerPlacement,
    LayeredPane, LayeredPaneMut, MouseLocationZoomScrollPolicy, RangeChange, RangeChangeSet,
    RangeListener, RangeListenerId, RangeModel, RangeModelError, RangeModelSnapshot, RangeProperty,
    ScalableFigure, ScalableFreeformLayeredPane, ScalableLayeredPaneFigure, ScaleError,
    ScaleHandle, ScrollBarFigure, ScrollBarVisibility, ScrollOrientation, ScrollPaneError,
    ScrollPaneFigure, ScrollPaneHandle, ScrollPaneLayout, ZoomError, ZoomManager, ZoomScrollPolicy,
    ZoomViewportState,
};
pub use figure::{
    AccessibleFigure, Alignment, AsAny, BorderedFigure, Bounded, ButtonFigure,
    ChildClippingStrategy, ChildPolicy, ChildTransform, ClickableBehavior, ClickableFigure,
    ClickableKind, ClickableModel, ClickableSnapshot, ClickableVisualState, Direction,
    EllipseFigure, Figure, FigureContainer, FigureEventHandler, FigureLifecycle,
    FigureLifecycleContext, FigureMeasurement, FlowPage, FlowParagraph, FlowTextPosition,
    FlowTextRange, FlowWrapping, Freeform, HitParticipation, ImageDisplayState, ImageFigure,
    InlineTextFragment, LabelFigure, Layer, MeasureConstraints, MeasureConstraintsError,
    PointListFigureBehavior, PolygonFigure, PolygonScaleMode, PolylineFigure, RectangleFigure,
    RootFigure, RoundedRectangleFigure, ScalablePolygonBehavior, ScalablePolygonError,
    ScalablePolygonFigure, Shape, ShapeMutationError, TextFlowBehavior, TextFlowFigure,
    TextFlowViewport, TextPlacement, ToggleFigure, TriangleFigure, WidgetError,
};
pub use geometry::{Affine2D, Dimension, Insets, Point, PointList, Rectangle, Vec2};
pub(crate) use graph::FigureNode;
pub use graph::{
    DEFAULT_VALIDATION_BUDGET, ExclusionSearch, FREEFORM_EXTENT_PROPERTY, FigureId, FigureTree,
    FigureTreeBuilder, FreeformError, GraphMutationError, IdentitySearch, MAX_TREE_DEPTH,
    TreeQueryError, TreeSearch, TreeSearchContext, ValidationError,
};
pub use host::{HeadlessHost, ImeState, PlatformHost};
pub(crate) use identity::RuntimeNamespace;
pub use layout::{
    BorderConstraint, BorderLayout, BorderRegion, FillLayout, FlowDirection, FlowLayout,
    FreeformConstraint, FreeformConstraintError, FreeformLayout, GridAlignment, GridConstraint,
    GridLayout, LayoutConstraint, LayoutError, LayoutInvalidation, LayoutManager, LayoutOutput,
    LayoutSnapshot, MinorAlignment, StackLayout, ToolbarLayout, ToolbarOrientation, XYConstraint,
    XYLayout,
};
pub use render::{
    BackendCapabilities, CaretGeometry, DamageMode, NdCanvas, RenderBackend, RenderCapability,
    RenderOutcome, ResourceId, SelectionQuad, SurfaceInfo, TextAffinity, TextInteractionError,
    TextInteractionMap, TextInteractionProvider, TextLayoutRevision, TextMovement, TextPosition,
    TextRange, UnsupportedRenderCapability,
};
pub(crate) use runtime::InteractionState;
pub use runtime::context::{EventContext, SceneDispatchContext};
pub(crate) use runtime::event::EventDispatcher;
pub use runtime::event::{
    DispatchContext, DispatchOutcome, Event, FocusEvent, FocusEventKind, GesturePhase,
    GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
pub(crate) use runtime::mutation;
pub(crate) use runtime::mutation::PendingMutations;
pub(crate) use runtime::update::UpdateManager;
pub use runtime::update::{
    ActionEvent, ActionListener, AncestorEvent, AncestorEventKind, AncestorListener,
    CoordinateListener, FigureEvent, FigureListener, LayoutEvent, LayoutEventKind, LayoutListener,
    ListenerDirective, ListenerId, ListenerScope, NotificationEffect, NotificationRecord,
    ObservationListener, PropertyChangeEvent, PropertyChangeListener, PropertyValue,
    StableQueryError, StableSceneQuery, UpdateEvent, UpdateListener, ValidatingListener,
};
pub use runtime::{
    AccessibilityAction, AccessibilityDelta, AccessibilityError, AccessibilityNode,
    AccessibilityNodeId, AccessibilityRole, AccessibilitySnapshot, AccessibilityState,
    AccessibilityUpdate, BackendSessionError, BorderMut, ClickableMut, ComponentInvalidation,
    ComponentUpdateError, ComponentUpdateReceipt, ContainerMut, FigureComponentContext,
    FigureComponentUpdate, FigureMut, FocusChange, FocusError, FocusTraversalDirection,
    FocusTraversalOutcome, FocusTraversalPolicy, FontId, FrameNotReady, FramePreparation,
    FramePreparationError, ImageId, ImageMut, LabelMut, LogicalViewportResizeError, MonotonicTime,
    PointListMut, PointerId, ResourceError, ResourceKind, ResourceStatus, RoundedRectangleMut,
    Runtime, RuntimeMutationError, ScalablePolygonMut, ScaleMut, ScrollPaneMut, TextFlowMut,
    TextFlowQueryError, TextLayoutStats, TimeError, TooltipPlacement, TooltipSide, TooltipSnapshot,
    TooltipTiming, TooltipUpdate, TreeOrderFocusTraversal, TriangleMut, ViewportMut, ZoomMut,
    place_tooltip,
};
pub use style::{CursorIcon, FigureStyle, ResolvedStyle};
pub use viewport::{ViewportError, ViewportFigure, ViewportHandle, ViewportLayout};

pub use figure::border;
pub use figure::border::{
    BevelBorder, BevelStyle, Border, BorderStyle, CompoundBorder, EtchedBorder, LineBorder,
    MarginBorder, RectangleBorder, TitleBarBorder,
};
