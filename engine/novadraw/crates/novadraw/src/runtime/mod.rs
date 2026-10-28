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
pub(crate) mod timing;
pub mod tooltip;
pub use timing::RuntimePhaseTiming;
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
    CapabilityUpdateError, CapabilityUpdateReceipt, ClearPolylinePoints, ComponentInvalidation,
    ComponentUpdateError, ComponentUpdateReceipt, FigureCapabilityContext, FigureCapabilityUpdate,
    FigureComponentContext, FigureComponentUpdate, FigureUpdateEffect, FigureUpdateEffectContext,
    InsertPolylinePoint, PreparedCapabilityUpdate, PreparedFigureUpdate, RemovePolylinePoint,
    ReplaceFigureBorder, ReplacePolygonGeometry, ReplacePolylinePoints,
    ReplaceScalablePolygonTemplate, ReplaceTextFlowPage, RuntimeMutationError,
    SetClickableRolloverEnabled, SetClickableSelected, SetImageAlignment, SetImageSource,
    SetLabelAlignment, SetLabelIcon, SetLabelIconAlignment, SetLabelIconTextGap, SetLabelText,
    SetLabelTextAlignment, SetLabelTextPlacement, SetPolygonCornerRadii, SetPolylineDashOffset,
    SetPolylineDashPattern, SetPolylineLineCap, SetPolylineLineJoin, SetPolylineMiterLimit,
    SetPolylinePoint, SetPolylineStrokeStyle, SetPolylineStrokeWidth, SetRectangleCorners,
    SetRectangleEdges, SetRoundedRectangleCorners, SetScalablePolygonAlignment,
    SetScalablePolygonScaleMode, SetTextFlowWrapping, SetTriangleCornerRadii, SetTriangleDirection,
};
pub use resource::{
    FontId, ImageCompletionError, ImageId, ResourceError, ResourceKind, ResourceRegistry,
    ResourceRequestToken, ResourceStatus,
};
pub use runtime::{
    BackendSessionError, FigureMut, FrameNotReady, FramePreparation, FramePreparationError,
    LogicalViewportResizeError, Runtime, RuntimeAccessibility, RuntimeAnimation,
    RuntimeConnections, RuntimeFocus, RuntimeFrames, RuntimeInput, RuntimeLayers,
    RuntimeObservations, RuntimeResources, RuntimeText, RuntimeTooltip, RuntimeViewports,
    SubmissionAcknowledgement, SubmissionCompletionError, TextFlowQueryError, TextLayoutStats,
};
pub use tooltip::{
    DEFAULT_TOOLTIP_GAP, DEFAULT_TOOLTIP_HIDE_DELAY, DEFAULT_TOOLTIP_SHOW_DELAY, MonotonicTime,
    TimeError, TooltipPlacement, TooltipSide, TooltipSnapshot, TooltipTiming, TooltipUpdate,
    place_tooltip,
};
pub use update::{StableQueryError, StableSceneQuery};
