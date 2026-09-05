//! Figure-level container primitives.
//!
//! Containers are part of the figure tree and provide structural composition
//! semantics such as clipping, viewport transforms, and future scroll panes.

pub mod layer;
pub mod range_model;
pub mod scalable;
pub mod scroll_pane;
pub mod viewport;
pub mod zoom;

pub use layer::{
    FreeformLayerFigure, FreeformLayeredPane, LayerError, LayerFigure, LayerKey, LayerKeyError,
    LayerPlacement, LayeredPane, LayeredPaneHandle,
};
pub use range_model::{
    DefaultRangeModel, RangeChange, RangeChangeSet, RangeListener, RangeListenerId, RangeModel,
    RangeModelError, RangeModelSnapshot, RangeProperty,
};
pub use scalable::{
    ScalableFigure, ScalableFreeformLayeredPane, ScalableLayeredPaneFigure, ScaleError, ScaleHandle,
};
pub use scroll_pane::{
    ScrollBarFigure, ScrollBarVisibility, ScrollOrientation, ScrollPaneError, ScrollPaneFigure,
    ScrollPaneHandle, ScrollPaneLayout,
};
pub use zoom::{
    DEFAULT_ZOOM_LEVELS, DefaultScrollPolicy, MouseLocationZoomScrollPolicy, ZoomError,
    ZoomManager, ZoomScrollPolicy, ZoomViewportState,
};
