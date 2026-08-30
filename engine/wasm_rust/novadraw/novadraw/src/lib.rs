//! Novadraw Main Library
//!
//! 此库作为所有子库的聚合入口，提供统一的 API。

pub use novadraw_core::Color;
pub use novadraw_geometry::Transform;

#[cfg(feature = "vello")]
pub use novadraw_render::{
    DamageMode, DamageSet, NdCanvas, RenderBackend, RenderCommand, RenderCommandKind,
    RenderOutcome, RenderSubmission, WindowProxy, command,
};

#[cfg(feature = "vello")]
pub use novadraw_render as render;

#[cfg(feature = "vello")]
pub use novadraw_render::backend;

#[cfg(feature = "vello")]
pub use novadraw_render::traits;

#[cfg(feature = "vello")]
pub use novadraw_scene::{
    AncestorEvent, AncestorEventKind, AncestorListener, BasicEventDispatcher, BlockId, Border,
    BorderConstraint, BorderLayout, BorderRegion, Bounded, ChildPolicy, ChildTransform,
    CoordinateListener, DEFAULT_ZOOM_LEVELS, DefaultScrollPolicy, Direction, DispatchContext,
    EllipseFigure, Event, EventDispatcher, Figure, FigureEvent, FigureGraph, FigureListener,
    FigureRenderer, FillLayout, FlowDirection, FlowLayout, FocusEvent, FocusEventKind,
    GesturePhase, GestureSessionId, GraphMutationError, GridAlignment, GridConstraint, GridLayout,
    Key, KeyEvent, KeyEventKind, KeyModifiers, LayoutConstraint, LayoutEvent, LayoutEventKind,
    LayoutListener, LayoutManager, LineBorder, ListenerId, MAX_TREE_DEPTH, MarginBorder,
    MinorAlignment, MouseButton, MouseEvent, MouseEventKind, MouseLocationZoomScrollPolicy,
    NotificationEffect, NotificationQueue, NovadrawContext, NovadrawSystem, PendingMutationBatch,
    PendingMutations, Point, PolygonFigure, PolylineFigure, PropertyChangeEvent,
    PropertyChangeListener, PropertyValue, RangeChange, RangeChangeSet, RangeListener,
    RangeListenerId, RangeModel, RangeModelError, RangeModelSnapshot, RangeProperty, Rectangle,
    RectangleBorder, RectangleFigure, RootFigure, RoundedRectangleFigure, ScalableFigure,
    ScalableLayeredPaneFigure, ScaleError, ScaleHandle, SceneDispatchContext, SceneHost,
    SceneNovadrawContext, SceneUpdateManager, ScrollBarFigure, ScrollBarVisibility,
    ScrollDeltaKind, ScrollOrientation, ScrollPaneError, ScrollPaneFigure, ScrollPaneHandle,
    ScrollPaneLayout, Shape, StackLayout, ToolbarLayout, ToolbarOrientation, TriangleFigure,
    Updatable, UpdateEvent, UpdateListener, UpdateManager, ValidatingListener, ViewportError,
    ViewportFigure, ViewportHandle, ViewportLayout, WheelEvent, XYConstraint, XYLayout, ZoomError,
    ZoomEvent, ZoomManager, ZoomScrollPolicy, ZoomViewportState,
};

#[cfg(feature = "vello")]
pub mod border {
    pub use novadraw_scene::border::*;
}
