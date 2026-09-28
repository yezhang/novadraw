#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

/// Low-level types for diagnostics and deep engine integration.
pub mod advanced;
/// Optional rendering backend implementations.
#[cfg(any(feature = "native-vello", feature = "web-vello"))]
pub mod backend;
/// Connection, anchor, router, and locator APIs.
pub mod connection;
/// Viewport, scrolling, zoom, layering, and freeform containers.
pub mod container;
/// Model-driven editing framework.
pub mod editor;
/// Input events, listeners, focus, tooltip, and accessibility APIs.
pub mod event;
/// Figure traits, built-in figures, borders, and styles.
pub mod figure;
/// Platform-independent geometry values and operations.
pub mod geometry;
/// Stateful drawing API and path primitives.
pub mod graphics;
/// Platform host integration.
pub mod host;
/// Layout extension protocols and built-in layouts.
pub mod layout;
/// Common imports for Figure and Runtime application code.
pub mod prelude;
/// Backend-neutral rendering, text, resource, and submission protocols.
pub mod render;
/// Runtime lifecycle, mutation, resource, and frame APIs.
pub mod runtime;
/// Figure tree construction and query APIs.
pub mod tree;

pub use figure::{
    ButtonFigure, EllipseFigure, Figure, FigureStyle, ImageFigure, LabelFigure, PolygonFigure,
    PolylineFigure, RectangleFigure, RoundedRectangleFigure, ToggleFigure, TriangleFigure,
};
pub use geometry::{Affine2D, Dimension, Insets, Point, PointList, Rectangle, Vec2};
pub use graphics::{Color, NdCanvas};
pub use host::PlatformHost;
pub use layout::{
    BorderLayout, FillLayout, FlowLayout, FreeformLayout, GridLayout, LayoutManager, StackLayout,
    ToolbarLayout, XYLayout,
};
pub use render::RenderBackend;
pub use runtime::Runtime;
pub use tree::{FigureId, FigureTree, FigureTreeBuilder};
