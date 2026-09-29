//! Stateful drawing and path construction APIs used by figures.

pub use crate::render::command::{
    DEFAULT_STROKE_MITER_LIMIT, LineCap, LineJoin, LineStyle, Path, PathOp,
};
pub use crate::render::{ImageDrawError, NdCanvas};
pub use crate::{Color, ColorError, ParseColorError};
