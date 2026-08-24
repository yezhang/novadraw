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
    BorderConstraint, BorderLayout, BorderRegion, Bounded, ChildTransform, CoordinateListener,
    Direction, DispatchContext, EllipseFigure, Event, EventDispatcher, Figure, FigureEvent,
    FigureGraph, FigureListener, FigureRenderer, FillLayout, FlowDirection, FlowLayout, FocusEvent,
    FocusEventKind, GraphMutationError, GridAlignment, GridConstraint, GridLayout, Key, KeyEvent,
    KeyEventKind, KeyModifiers, LayoutConstraint, LayoutEvent, LayoutEventKind, LayoutListener,
    LayoutManager, LineBorder, ListenerId, MAX_TREE_DEPTH, MarginBorder, MinorAlignment,
    MouseButton, MouseEvent, MouseEventKind, NotificationEffect, NotificationQueue,
    NovadrawContext, NovadrawSystem, PendingMutationBatch, PendingMutations, Point, PolygonFigure,
    PolylineFigure, PropertyChangeEvent, PropertyChangeListener, PropertyValue, Rectangle,
    RectangleBorder, RectangleFigure, RootFigure, RoundedRectangleFigure, SceneDispatchContext,
    SceneHost, SceneNovadrawContext, SceneUpdateManager, Shape, StackLayout, ToolbarLayout,
    ToolbarOrientation, TriangleFigure, Updatable, UpdateEvent, UpdateListener, UpdateManager,
    ValidatingListener, Viewport, ViewportFigure, WheelEvent, XYConstraint, XYLayout,
};

#[cfg(feature = "vello")]
pub mod border {
    pub use novadraw_scene::border::*;
}
