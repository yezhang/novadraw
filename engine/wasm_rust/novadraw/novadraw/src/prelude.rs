//! Common imports for backend-neutral Figure and Runtime development.

pub use crate::event::FigureEventHandler;
pub use crate::figure::{Border, FigureContainer, FigureLifecycle, Shape};
pub use crate::geometry::{ApproxEq, Translatable};
pub use crate::layout::{
    BorderConstraint, FreeformConstraint, GridConstraint, LayoutConstraint, XYConstraint,
};
pub use crate::{
    Affine2D, BorderLayout, ButtonFigure, Color, Dimension, EllipseFigure, EndpointLocator, Figure,
    FigureId, FigureStyle, FigureTree, FigureTreeBuilder, FillLayout, FlowLayout, FlowPage,
    FlowParagraph, FlowWrapping, FreeformLayout, GridLayout, ImageFigure, InlineTextFragment,
    Insets, LabelFigure, LayoutManager, NdCanvas, PlatformHost, Point, PointList,
    PolygonDecorationFigure, PolygonFigure, PolygonScaleMode, PolylineDecorationFigure,
    PolylineFigure, Rectangle, RectangleFigure, RenderBackend, RoundedRectangleFigure, Runtime,
    ScalablePolygonFigure, ShortestPathConnectionRouter, StackLayout, TextFlowFigure, ToggleFigure,
    ToolbarLayout, TriangleFigure, Vec2, XYLayout,
};
