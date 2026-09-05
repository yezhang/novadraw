//! Novadraw Scene Library
//!
//! 场景图库，提供 Figure 渲染和场景图管理。
//!
//! # 模块
//!
//! - [`figure`] - Figure 渲染接口和实现
//! - [`graph`] - FigureTree、图结构和渲染/命中测试集成
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
pub mod style;

pub use container::viewport;
pub use container::{
    DEFAULT_ZOOM_LEVELS, DefaultRangeModel, DefaultScrollPolicy, LayerError, LayerFigure, LayerKey,
    LayerKeyError, LayerPlacement, LayeredPane, LayeredPaneHandle, MouseLocationZoomScrollPolicy,
    RangeChange, RangeChangeSet, RangeListener, RangeListenerId, RangeModel, RangeModelError,
    RangeModelSnapshot, RangeProperty, ScalableFigure, ScalableLayeredPaneFigure, ScaleError,
    ScaleHandle, ScrollBarFigure, ScrollBarVisibility, ScrollOrientation, ScrollPaneError,
    ScrollPaneFigure, ScrollPaneHandle, ScrollPaneLayout, ZoomError, ZoomManager, ZoomScrollPolicy,
    ZoomViewportState,
};
pub use figure::border;
pub use figure::border::{Border, LineBorder, MarginBorder, RectangleBorder};
pub use figure::{
    AccessibleFigure, AsAny, Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform,
    Direction, EllipseFigure, Figure, FigureContainer, FigureEventHandler, FigureLifecycle,
    HitParticipation, Layer, PolygonFigure, PolylineFigure, RectangleFigure, RootFigure,
    RoundedRectangleFigure, Shape, TriangleFigure,
};
pub use graph as scene;
pub use graph::{
    DEFAULT_VALIDATION_BUDGET, ExclusionSearch, FigureId, FigureNode, FigureTree,
    FigureTreeBuilder, GraphMutationError, IdentitySearch, LayoutState, MAX_TREE_DEPTH, NodeState,
    TreeQueryError, TreeSearch, TreeSearchContext, ValidationError,
};
pub use host::{AccessibilityUpdate, HeadlessHost, ImeState, PlatformHost};
pub use layout::{
    BorderConstraint, BorderLayout, BorderRegion, FillLayout, FlowDirection, FlowLayout,
    GridAlignment, GridConstraint, GridLayout, LayoutConstraint, LayoutError, LayoutInvalidation,
    LayoutManager, LayoutOutput, LayoutSnapshot, MinorAlignment, StackLayout, ToolbarLayout,
    ToolbarOrientation, XYConstraint, XYLayout,
};
pub use novadraw_geometry::{Point, Rectangle};
pub use runtime::context::{EventContext, SceneDispatchContext};
pub use runtime::event::{
    DispatchContext, Event, EventDispatcher, FocusEvent, FocusEventKind, GesturePhase,
    GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
pub use runtime::mutation::PendingMutations;
pub use runtime::update;
pub use runtime::update::{
    AncestorEvent, AncestorEventKind, AncestorListener, CoordinateListener, FigureEvent,
    FigureListener, LayoutEvent, LayoutEventKind, LayoutListener, ListenerId, NotificationEffect,
    NotificationQueue, PropertyChangeEvent, PropertyChangeListener, PropertyValue, UpdateEvent,
    UpdateListener, UpdateManager, ValidatingListener,
};
pub use runtime::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    FontId, ImageId, InteractionState, PointerId, ResourceError, ResourceKind, ResourceRegistry,
    ResourceStatus, Runtime, TreeOrderFocusTraversal,
};
pub use runtime::{context, event, mutation};
pub use style::{CursorIcon, FigureStyle, ResolvedStyle};
pub use viewport::{ViewportError, ViewportFigure, ViewportHandle, ViewportLayout};
