//! Stateful drawing and path construction APIs used by figures.

pub use crate::render::command::{
    DEFAULT_STROKE_MITER_LIMIT, LineCap, LineJoin, LineStyle, Path, PathOp,
};
pub use crate::render::{
    ClipPath, CustomDash, DashPattern, FillRule, GradientStop, GraphicsInputError, ImageDrawError,
    LinearGradient, NdCanvas, Paint, StrokeStyle,
};
pub use crate::{Color, ColorError, ParseColorError};
