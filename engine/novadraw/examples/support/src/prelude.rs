//! Novadraw 应用预导入模块
//!
//! 导入常用的类型和函数，方便快速开发。

#[cfg(feature = "native")]
pub use crate::{
    AppBuilder, DemoApp, run_demo_app, run_demo_app_with_scene_screenshot,
    run_demo_app_with_screenshot, run_runtime_demo_app, run_runtime_demo_app_with_scene_screenshot,
    run_runtime_demo_app_with_screenshot,
};
pub use novadraw::event::{FigureEvent, NotificationEffect, UpdateEvent, UpdateListener};
pub use novadraw::figure::{
    Border, ButtonFigure, EllipseFigure, Figure, FigureContainer, FigureLifecycle,
    FigureMeasurement, FigureStyle, ImageFigure, LabelFigure, MeasureConstraints, PolygonFigure,
    PolylineFigure, RectangleFigure, RoundedRectangleFigure, Shape, TextFlowFigure, ToggleFigure,
    TriangleFigure,
};
pub use novadraw::geometry::{
    Affine2D, ApproxEq, Dimension, Insets, Point, PointList, Rectangle, Translatable, Vec2,
};
pub use novadraw::graphics::{Color, Graphics, PaintContext};
pub use novadraw::host::PlatformHost;
pub use novadraw::layout::{
    BorderConstraint, BorderLayout, FillLayout, FlowLayout, FreeformConstraint, FreeformLayout,
    GridConstraint, GridLayout, LayoutConstraint, LayoutManager, StackLayout, ToolbarLayout,
    XYConstraint, XYLayout,
};
pub use novadraw::runtime::Runtime;
pub use novadraw::tree::{FigureId, FigureTree, FigureTreeBuilder};
#[cfg(feature = "native")]
pub use winit::event::ElementState;
