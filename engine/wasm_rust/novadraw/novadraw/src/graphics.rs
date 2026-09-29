//! Stateful drawing and path construction APIs used by figures.

pub use novadraw_core::{Color, ColorError, ParseColorError};
pub use novadraw_render::command::{
    DEFAULT_STROKE_MITER_LIMIT, LineCap, LineJoin, LineStyle, Path, PathOp,
};
pub use novadraw_render::{ImageDrawError, NdCanvas};
