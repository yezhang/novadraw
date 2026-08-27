//! Figure-level container primitives.
//!
//! Containers are part of the figure tree and provide structural composition
//! semantics such as clipping, viewport transforms, and future scroll panes.

pub mod range_model;
pub mod scalable;
pub mod scroll_pane;
pub mod viewport;

pub use range_model::{
    DefaultRangeModel, RangeChange, RangeChangeSet, RangeListener, RangeListenerId, RangeModel,
    RangeModelError, RangeModelSnapshot, RangeProperty,
};
pub use scalable::{ScalableFigure, ScalableLayeredPaneFigure, ScaleError, ScaleHandle};
pub use scroll_pane::{
    ScrollBarFigure, ScrollBarVisibility, ScrollOrientation, ScrollPaneError, ScrollPaneFigure,
    ScrollPaneHandle, ScrollPaneLayout,
};
