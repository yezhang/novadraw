//! Novadraw Scene Library
//!
//! 场景图库，提供 Figure 渲染和场景图管理。
//!
//! # 模块
//!
//! - [`figure`] - Figure 渲染接口和实现
//! - [`graph`] - FigureGraph、图结构和渲染/命中测试集成
//! - [`runtime`] - 事件、上下文、更新、延迟结构变更和组合根协议
//! - [`container`] - Viewport 等 Figure 级容器
//! - [`host`] - 平台宿主与渲染入口协调

#![allow(missing_docs)]

pub mod container;
pub mod figure;
pub mod graph;
pub mod host;
pub mod layout;
pub mod log;
pub mod runtime;

pub use container::viewport;
pub use figure::border;
pub use figure::border::{Border, LineBorder, MarginBorder, RectangleBorder};
pub use figure::{
    Bounded, ChildClippingStrategy, ChildTransform, Direction, EllipseFigure, Figure,
    PolygonFigure, PolylineFigure, RectangleFigure, RootFigure, RoundedRectangleFigure, Shape,
    TriangleFigure, Updatable,
};
pub use graph as scene;
pub use graph::{BlockId, FigureGraph, FigureRenderer, GraphMutationError, MAX_TREE_DEPTH};
pub use host::SceneHost;
pub use layout::{
    BorderConstraint, BorderLayout, BorderRegion, FillLayout, FlowDirection, FlowLayout,
    GridAlignment, GridConstraint, GridLayout, LayoutConstraint, LayoutManager, MinorAlignment,
    StackLayout, ToolbarLayout, ToolbarOrientation, XYConstraint, XYLayout,
};
pub use novadraw_geometry::{Point, Rectangle};
pub use runtime::context::{NovadrawContext, SceneDispatchContext, SceneNovadrawContext};
pub use runtime::event::{
    BasicEventDispatcher, DispatchContext, Event, EventDispatcher, FocusEvent, FocusEventKind, Key,
    KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, WheelEvent,
};
pub use runtime::mutation::{PendingMutationBatch, PendingMutations};
pub use runtime::system::NovadrawSystem;
pub use runtime::update;
pub use runtime::update::{
    AncestorEvent, AncestorEventKind, AncestorListener, CoordinateListener, FigureEvent,
    FigureListener, LayoutEvent, LayoutEventKind, LayoutListener, ListenerId, NotificationEffect,
    NotificationQueue, PropertyChangeEvent, PropertyChangeListener, PropertyValue,
    SceneUpdateManager, UpdateEvent, UpdateListener, UpdateManager, ValidatingListener,
};
pub use runtime::{context, event, mutation, system};
pub use viewport::{Viewport, ViewportFigure};
