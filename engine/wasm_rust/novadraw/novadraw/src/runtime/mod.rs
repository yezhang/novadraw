//! Runtime services around the Figure tree.
//!
//! [`Runtime`] is the preferred composition root. It owns one tree together
//! with interaction, deferred mutation, and update state.

pub mod accessibility;
pub mod context;
pub mod event;
pub mod focus;
pub mod interaction;
pub mod mutation;
pub mod resource;
pub mod tooltip;
// `runtime::Runtime` is the deliberate public domain name.
#[allow(clippy::module_inception)]
pub mod runtime;
pub mod update;

pub use accessibility::{
    AccessibilityAction, AccessibilityDelta, AccessibilityError, AccessibilityNode,
    AccessibilityNodeId, AccessibilityRole, AccessibilitySnapshot, AccessibilityState,
    AccessibilityUpdate,
};
pub use focus::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    TreeOrderFocusTraversal,
};
pub use interaction::{InteractionState, PointerId};
pub use mutation::{
    ComponentInvalidation, ComponentUpdateError, ComponentUpdateReceipt, FigureComponentContext,
    FigureComponentUpdate, PreparedFigureUpdate, RuntimeMutationError,
};
pub use resource::{
    FontId, ImageId, ResourceError, ResourceKind, ResourceRegistry, ResourceStatus,
};
pub use runtime::{
    BackendSessionError, BorderMut, ClickableMut, ContainerMut, FigureMut,
    FramePreparation, FramePreparationError, ImageMut, LabelMut, LogicalViewportResizeError,
    PointListMut, RoundedRectangleMut, Runtime, ScalablePolygonMut, ScaleMut,
    ScrollPaneMut, TextFlowMut, TextFlowQueryError, TextLayoutStats, TriangleMut,
    ViewportMut, ZoomMut,
};
pub use tooltip::{
    DEFAULT_TOOLTIP_GAP, DEFAULT_TOOLTIP_HIDE_DELAY, DEFAULT_TOOLTIP_SHOW_DELAY, MonotonicTime,
    TimeError, TooltipPlacement, TooltipSide, TooltipSnapshot, TooltipTiming, TooltipUpdate,
    place_tooltip,
};
pub use update::{StableQueryError, StableSceneQuery};
