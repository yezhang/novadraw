#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod color;
#[allow(missing_docs)]
mod graph;
#[allow(missing_docs)]
mod identity;
#[allow(missing_docs)]
mod style;

/// Low-level types for diagnostics and deep engine integration.
pub mod advanced;
/// Runtime-owned animation clocks, typed channels, motions, and timelines.
pub mod animation;
/// Connection, anchor, router, and locator APIs.
#[allow(missing_docs)]
pub mod connection;
/// Viewport, scrolling, zoom, layering, and freeform containers.
#[allow(missing_docs)]
pub mod container;
/// Input events, listeners, focus, tooltip, and accessibility APIs.
pub mod event;
/// Figure traits, built-in figures, borders, and styles.
#[allow(missing_docs)]
pub mod figure;
/// Platform-independent geometry values and operations.
pub mod geometry;
/// Stateful drawing API and path primitives.
pub mod graphics;
/// Platform host integration.
#[allow(missing_docs)]
pub mod host;
/// Layout extension protocols and built-in layouts.
#[allow(missing_docs)]
pub mod layout;
/// Common imports for Figure and Runtime application code.
pub mod prelude;
/// Backend-neutral rendering, text, resource, and submission protocols.
pub mod render;
/// Runtime lifecycle, mutation, resource, and frame APIs.
#[allow(missing_docs)]
pub mod runtime;
/// Text measurement, immutable layouts and glyph outline extensions.
pub mod text;
/// Figure tree construction and query APIs.
pub mod tree;

pub use color::{Color, ColorError, ParseColorError};
pub use figure::{
    ButtonFigure, EllipseFigure, Figure, FigureMeasurement, ImageFigure, LabelFigure,
    MeasureConstraints, PolygonFigure, PolylineFigure, RectangleFigure, RoundedRectangleFigure,
    TextFlowFigure, ToggleFigure, TriangleFigure,
};
pub use geometry::{Affine2D, Dimension, Insets, Point, PointList, Rectangle, Vec2};
pub(crate) use graph::FigureNode;
pub use graph::{FigureId, FigureTree, FigureTreeBuilder};
pub use graphics::{Graphics, GraphicsError, PaintContext};
pub(crate) use identity::RuntimeNamespace;
pub use layout::{
    BorderLayout, FillLayout, FlowLayout, FreeformLayout, GridLayout, LayoutManager, StackLayout,
    ToolbarLayout, XYLayout,
};
pub(crate) use runtime::InteractionState;
pub(crate) use runtime::event::EventDispatcher;
pub(crate) use runtime::mutation;
pub(crate) use runtime::mutation::PendingMutations;
pub(crate) use runtime::update::UpdateManager;
pub use runtime::{FrameNotReady, FramePreparation, FramePreparationError, Runtime};
pub use style::FigureStyle;

// Internal modules still use the former root facade as a crate-local prelude.
// These aliases are deliberately not public and do not extend the root API.
#[allow(unused_imports)]
pub(crate) use connection::*;
#[allow(unused_imports)]
pub(crate) use container::*;
#[allow(unused_imports)]
pub(crate) use figure::border::*;
#[allow(unused_imports)]
pub(crate) use figure::{
    ACCESSIBILITY, AccessibilityCapability, AccessibleFigure, Alignment, AsAny, BORDER,
    BorderCapability, BorderedFigure, Bounded, CLICKABLE, CONNECTION, CONNECTION_DECORATION,
    CONTAINER, CapabilityKey, CapabilityQueryError, ChildClippingStrategy, ChildPolicy,
    ChildTransform, ClickableBehavior, ClickableCapability, ClickableFigure, ClickableKind,
    ClickableModel, ClickableSnapshot, ClickableVisualState, ConnectionCapability,
    ConnectionDecorationCapability, ContainerCapability, Direction, FREEFORM,
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
    FigureEventHandler, FigureLifecycle, FigureLifecycleContext, FlowPage, FlowParagraph,
    FlowTextPosition, FlowTextRange, FlowWrapping, FreeformCapability, HitParticipation, INPUT,
    ImageDisplayState, InlineTextFragment, InputCapability, LAYER, LIFECYCLE, LayerCapability,
    LifecycleCapability, MeasureConstraintsError, PREPARATION, PolygonScaleMode,
    PreparationCapability, RootFigure, SCALE, ScalablePolygonError, ScalablePolygonFigure,
    ScaleCapability, Shape, ShapeMutationError, TextFlowViewport, TextPlacement, WidgetError,
};
#[allow(unused_imports)]
pub(crate) use graph::{
    DEFAULT_VALIDATION_BUDGET, ExclusionSearch, FREEFORM_EXTENT_PROPERTY, FreeformError,
    GraphMutationError, IdentitySearch, MAX_TREE_DEPTH, TreeQueryError, TreeSearch,
    TreeSearchContext, ValidationError,
};
#[allow(unused_imports)]
pub(crate) use host::*;
#[allow(unused_imports)]
pub(crate) use layout::{
    BorderConstraint, BorderRegion, FlowDirection, FreeformConstraint, FreeformConstraintError,
    GridAlignment, GridConstraint, LayoutConstraint, LayoutError, LayoutInvalidation, LayoutOutput,
    LayoutSnapshot, MinorAlignment, ToolbarOrientation, XYConstraint,
};
#[allow(unused_imports)]
pub(crate) use render::{
    BackendCapabilities, CaretGeometry, DamageMode, NdCanvas, RenderBackend, RenderCapability,
    RenderOutcome, ResourceId, SelectionQuad, SurfaceInfo, TextAffinity, TextInteractionError,
    TextInteractionMap, TextInteractionProvider, TextLayoutRevision, TextMovement, TextPosition,
    TextRange, UnsupportedRenderCapability,
};
#[allow(unused_imports)]
pub(crate) use runtime::context::{EventContext, SceneDispatchContext};
#[allow(unused_imports)]
pub(crate) use runtime::event::{
    DispatchContext, DispatchOutcome, Event, FocusEvent, FocusEventKind, GesturePhase,
    GestureSessionId, Key, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
#[allow(unused_imports)]
pub(crate) use runtime::update::{
    ActionEvent, ActionListener, AncestorEvent, AncestorEventKind, AncestorListener,
    CoordinateListener, FigureEvent, FigureListener, LayoutEvent, LayoutEventKind, LayoutListener,
    ListenerDirective, ListenerId, ListenerScope, NotificationEffect, NotificationRecord,
    ObservationListener, PropertyChangeEvent, PropertyChangeListener, PropertyKey, PropertyValue,
    PropertyValueType, StableQueryError, StableSceneQuery, TypedPropertyChange, UpdateEvent,
    UpdateListener, ValidatingListener,
};
#[allow(unused_imports)]
pub(crate) use runtime::{
    AccessibilityAction, AccessibilityDelta, AccessibilityError, AccessibilityNode,
    AccessibilityNodeId, AccessibilityRole, AccessibilitySnapshot, AccessibilityState,
    AccessibilityUpdate, BackendSessionError, BorderMut, CapabilityUpdateError,
    CapabilityUpdateReceipt, ClickableMut, ComponentInvalidation, ComponentUpdateError,
    ComponentUpdateReceipt, ContainerMut, FigureCapabilityContext, FigureCapabilityUpdate,
    FigureComponentContext, FigureComponentUpdate, FigureMut, FocusChange, FocusError,
    FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy, FontId, ImageId,
    ImageMut, LabelMut, LogicalViewportResizeError, MonotonicTime, PointListMut, PointerId,
    PreparedCapabilityUpdate, ResourceError, ResourceKind, ResourceStatus, RoundedRectangleMut,
    RuntimeMutationError, ScalablePolygonMut, ScaleMut, ScrollPaneMut, TextFlowMut,
    TextFlowQueryError, TextLayoutStats, TimeError, TooltipPlacement, TooltipSide, TooltipSnapshot,
    TooltipTiming, TooltipUpdate, TreeOrderFocusTraversal, TriangleMut, ViewportMut, ZoomMut,
    place_tooltip,
};
#[allow(unused_imports)]
pub(crate) use style::{CursorIcon, ResolvedStyle};
