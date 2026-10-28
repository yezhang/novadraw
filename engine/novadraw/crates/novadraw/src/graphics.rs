//! Stateful drawing and path construction APIs used by figures.
mod antialias;
#[allow(missing_docs)]
mod context;
pub use antialias::AntialiasMode;
pub use context::{Graphics, GraphicsError, PaintContext, TextPaintStatistics};

pub use crate::color::{Color, ColorError, ParseColorError};
pub use crate::render::command::{
    DEFAULT_STROKE_MITER_LIMIT, LineCap, LineJoin, LineStyle, Path, PathOp,
};
pub use crate::render::{
    ClipPath, CustomDash, DashPattern, FillRule, GradientStop, GraphicsInputError, ImageDrawError,
    ImageRegion, LinearGradient, Paint, StrokeStyle,
};
