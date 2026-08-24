//! Novadraw Main Library
//!
//! 此库作为所有子库的聚合入口，提供统一的 API。

pub use novadraw_core::Color;
pub use novadraw_geometry::Transform;

#[cfg(feature = "vello")]
pub use novadraw_render::{
    DamageMode, DamageSet, NdCanvas, RenderBackend, RenderCommand, RenderCommandKind,
    RenderSubmission, WindowProxy, command,
};

#[cfg(feature = "vello")]
pub use novadraw_render as render;

#[cfg(feature = "vello")]
pub use novadraw_render::backend;

#[cfg(feature = "vello")]
pub use novadraw_render::traits;

#[cfg(feature = "vello")]
pub use novadraw_scene::{
    BasicEventDispatcher, BlockId, Border, BorderConstraint, BorderLayout, BorderRegion, Bounded,
    ChildTransform, Direction, DispatchContext, EllipseFigure, Event, EventDispatcher, Figure,
    FigureEvent, FigureGraph, FigureRenderer, FillLayout, FlowDirection, FlowLayout,
    GraphMutationError, LayoutConstraint, LayoutManager, LineBorder, MAX_TREE_DEPTH, MarginBorder,
    MouseButton, MouseEvent, MouseEventKind, NotificationEffect, NotificationQueue,
    NovadrawContext, NovadrawSystem, PendingMutationBatch, PendingMutations, Point, PolygonFigure,
    PolylineFigure, Rectangle, RectangleBorder, RectangleFigure, RootFigure,
    RoundedRectangleFigure, SceneDispatchContext, SceneHost, SceneNovadrawContext,
    SceneUpdateManager, Shape, TriangleFigure, Updatable, UpdateEvent, UpdateListener,
    UpdateManager, Viewport, ViewportFigure, XYConstraint, XYLayout,
};

#[cfg(feature = "vello")]
pub mod border {
    pub use novadraw_scene::border::*;
}
