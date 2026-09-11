//! Novadraw Main Library
//!
//! 此库作为所有子库的聚合入口，提供统一的 API。

pub use novadraw_core::Color;
pub use novadraw_geometry::{Affine2D, Transform};

pub use novadraw_render::{
    BackendCapabilities, BackendSessionDecision, BackendSessionGate, BackendSessionId, BuiltinFont,
    DamageMode, DamageSet, FontData, FontDescriptor, FontFaceRef, FontStyle, FrameId, GlyphPaint,
    GlyphRun, ImageData, ImageDecodeError, ImageResourceRef, NdCanvas, ParleyTextEngine,
    PositionedGlyph, RenderBackend, RenderCapability, RenderCommand, RenderCommandKind,
    RenderOutcome, RenderSubmission, ResourceDelta, ResourceId, ResourceOp, ResourcePayload,
    ResourceSnapshot, ResourceSync, ResourceUpdate, SurfaceInfo, TextConstraints, TextEngine,
    TextError, TextLayout, TextLayoutEngine, TextLayoutKey, TextLayoutParts, TextLineMetrics,
    UnsupportedRenderCapability, command,
};

pub use novadraw_render as render;

#[cfg(any(feature = "vello", feature = "vello-web"))]
pub use novadraw_render::backend;

pub use novadraw_render::traits;

pub use novadraw_scene::{
    AccessibilityAction, AccessibilityDelta, AccessibilityError, AccessibilityNode,
    AccessibilityNodeId, AccessibilityRole, AccessibilitySnapshot, AccessibilityState,
    AccessibilityUpdate, AccessibleFigure, ActionEvent, ActionListener, Alignment, AncestorEvent,
    AncestorEventKind, AncestorListener, AnchorGeometry, AnchorGeometryKey, Bendpoint,
    BendpointConnectionRouter, BendpointConstraint, BevelBorder, BevelStyle, Border,
    BorderConstraint, BorderLayout, BorderRegion, BorderedFigure, Bounded, ButtonFigure,
    ChildClippingStrategy, ChildPolicy, ChildTransform, ChopboxAnchor, ClickableBehavior,
    ClickableFigure, ClickableKind, ClickableModel, ClickableSnapshot, ClickableVisualState,
    ComponentInvalidation, ComponentUpdateError, ComponentUpdateReceipt, CompoundBorder,
    ConnectionFigure, ConnectionId, ConnectionLayerFigure, ConnectionLocator,
    ConnectionLocatorStrategy, ConnectionResolution, ConnectionRuntimeError,
    ConnectionStateSnapshot, CoordinateListener, CoordinateSpace, CursorIcon, DEFAULT_TOOLTIP_GAP,
    DEFAULT_TOOLTIP_HIDE_DELAY, DEFAULT_TOOLTIP_SHOW_DELAY, DEFAULT_ZOOM_LEVELS,
    DefaultScrollPolicy, DirectRouter, Direction, DispatchContext, EllipseAnchor, EllipseFigure,
    EtchedBorder, Event, EventContext, EventDispatcher, ExclusionSearch, FAN_DEFAULT_SEPARATION,
    FREEFORM_EXTENT_PROPERTY, FanRouter, Figure, FigureComponentContext, FigureComponentUpdate,
    FigureContainer, FigureEvent, FigureEventHandler, FigureId, FigureLifecycle,
    FigureLifecycleContext, FigureListener, FigureMeasurement, FigureNode, FigureStyle, FigureTree,
    FillLayout, FlowDirection, FlowLayout, FocusChange, FocusError, FocusEvent, FocusEventKind,
    FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy, FontId, FramePreparation,
    FramePreparationError, Freeform, FreeformConstraint, FreeformConstraintError, FreeformError,
    FreeformLayerFigure, FreeformLayeredPane, FreeformLayout, GesturePhase, GestureSessionId,
    GraphMutationError, GridAlignment, GridConstraint, GridLayout, HeadlessHost, HitParticipation,
    IdentitySearch, ImageDisplayState, ImageFigure, ImageId, ImeState, InteractionState, Key,
    KeyEvent, KeyEventKind, KeyModifiers, LabelAnchor, LabelFigure, Layer, LayerError, LayerFigure,
    LayerKey, LayerKeyError, LayerPlacement, LayeredPane, LayeredPaneHandle, LayoutConstraint,
    LayoutEvent, LayoutEventKind, LayoutListener, LayoutManager, LayoutState, LineBorder,
    ListenerDirective, ListenerId, LocatorError, LocatorPlacement, MANHATTAN_DEFAULT_LANE_SPACING,
    MANHATTAN_DEFAULT_MINIMUM_STUB, MAX_TREE_DEPTH, ManhattanConnectionRouter, MarginBorder,
    MeasureConstraints, MidpointLocator, MinorAlignment, MonotonicTime, MouseButton, MouseEvent,
    MouseEventKind, MouseLocationZoomScrollPolicy, NodeState, NotificationEffect,
    NotificationQueue, NotificationRecord, ObservationListener, PathFractionLocator,
    PendingMutations, PlatformHost, Point, PointListFigureBehavior, PointerId, PolygonFigure,
    PolylineFigure, PreparedFigureUpdate, PropertyChangeEvent, PropertyChangeListener,
    PropertyValue, RangeChange, RangeChangeSet, RangeListener, RangeListenerId, RangeModel,
    RangeModelError, RangeModelSnapshot, RangeProperty, Rectangle, RectangleBorder,
    RectangleFigure, ResolvedStyle, ResourceError, ResourceKind, ResourceRegistry, ResourceStatus,
    RootFigure, RoundedRectangleAnchor, RoundedRectangleFigure, RouteError, RouteOutput,
    RouteRequest, RouterBinding, RoutingConstraint, RoutingGroupQuery, RoutingGroupScope, Runtime,
    RuntimeMutationError, ScalableFigure, ScalableFreeformLayeredPane, ScalableLayeredPaneFigure,
    ScaleError, ScaleHandle, SceneDispatchContext, ScrollBarFigure, ScrollBarVisibility,
    ScrollDeltaKind, ScrollOrientation, ScrollPaneError, ScrollPaneFigure, ScrollPaneHandle,
    ScrollPaneLayout, Shape, ShapeMutationError, StableQueryError, StableSceneQuery, StackLayout,
    TextPlacement, TimeError, TitleBarBorder, ToggleFigure, ToolbarLayout, ToolbarOrientation,
    TooltipPlacement, TooltipSide, TooltipSnapshot, TooltipTiming, TooltipUpdate,
    TreeOrderFocusTraversal, TreeQueryError, TreeSearch, TreeSearchContext, TriangleFigure,
    UnresolvedConnection, UpdateEvent, UpdateListener, UpdateManager, ValidatingListener,
    ValidationError, ViewportError, ViewportFigure, ViewportHandle, ViewportLayout, WheelEvent,
    WidgetError, XYAnchor, XYConstraint, XYLayout, ZoomEvent, ZoomManager, ZoomScrollPolicy,
    place_tooltip,
};

pub mod border {
    pub use novadraw_scene::border::*;
}
