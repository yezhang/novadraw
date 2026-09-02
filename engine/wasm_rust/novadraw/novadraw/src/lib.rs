//! Novadraw Main Library
//!
//! 此库作为所有子库的聚合入口，提供统一的 API。

pub use novadraw_core::Color;
pub use novadraw_geometry::{Affine2D, Transform};

pub use novadraw_render::{
    BackendCapabilities, DamageMode, DamageSet, FrameId, NdCanvas, RenderBackend, RenderCommand,
    RenderCommandKind, RenderOutcome, RenderSubmission, ResourceDelta, SurfaceInfo, command,
};

pub use novadraw_render as render;

#[cfg(any(feature = "vello", feature = "vello-web"))]
pub use novadraw_render::backend;

pub use novadraw_render::traits;

pub use novadraw_scene::{
    AccessibilityUpdate, AccessibleFigure, AncestorEvent, AncestorEventKind, AncestorListener,
    BasicEventDispatcher, Border, BorderConstraint, BorderLayout, BorderRegion, Bounded,
    ChildPolicy, ChildTransform, CoordinateListener, CursorIcon, DEFAULT_ZOOM_LEVELS,
    DefaultScrollPolicy, Direction, DispatchContext, EllipseFigure, Event, EventDispatcher, Figure,
    FigureContainer, FigureEvent, FigureEventHandler, FigureId, FigureLifecycle, FigureListener,
    FigureNode, FigureRenderer, FigureTree, FillLayout, FlowDirection, FlowLayout, FocusEvent,
    FocusEventKind, GesturePhase, GestureSessionId, GraphMutationError, GridAlignment,
    GridConstraint, GridLayout, HeadlessHost, ImeState, InteractionState, Key, KeyEvent,
    KeyEventKind, KeyModifiers, LayoutConstraint, LayoutEvent, LayoutEventKind, LayoutListener,
    LayoutManager, LayoutState, LineBorder, ListenerId, MAX_TREE_DEPTH, MarginBorder,
    MinorAlignment, MouseButton, MouseEvent, MouseEventKind, MouseLocationZoomScrollPolicy,
    NodeState, NotificationEffect, NotificationQueue, NovadrawContext, PendingMutationBatch,
    PendingMutations, PlatformHost, Point, PointerId, PolygonFigure, PolylineFigure,
    PropertyChangeEvent, PropertyChangeListener, PropertyValue, RangeChange, RangeChangeSet,
    RangeListener, RangeListenerId, RangeModel, RangeModelError, RangeModelSnapshot, RangeProperty,
    Rectangle, RectangleBorder, RectangleFigure, RootFigure, RoundedRectangleFigure, Runtime,
    ScalableFigure, ScalableLayeredPaneFigure, ScaleError, ScaleHandle, SceneDispatchContext,
    SceneNovadrawContext, SceneUpdateManager, ScrollBarFigure, ScrollBarVisibility,
    ScrollDeltaKind, ScrollOrientation, ScrollPaneError, ScrollPaneFigure, ScrollPaneHandle,
    ScrollPaneLayout, Shape, StackLayout, StyleOverride, ToolbarLayout, ToolbarOrientation,
    TriangleFigure, Updatable, UpdateEvent, UpdateListener, UpdateManager, ValidatingListener,
    ViewportError, ViewportFigure, ViewportHandle, ViewportLayout, WheelEvent, XYConstraint,
    XYLayout, ZoomError, ZoomEvent, ZoomManager, ZoomScrollPolicy, ZoomViewportState,
};

pub mod border {
    pub use novadraw_scene::border::*;
}
