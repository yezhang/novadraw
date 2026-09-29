//! Novadraw Render Library
//!
//! 渲染抽象层，包括渲染命令、渲染上下文和渲染器 traits。
//!
//! # 模块
//!
//! - `command` - 渲染命令类型
//! - `context` - 渲染上下文
//! - `traits` - 渲染器 traits
//! - backend support - 具体渲染后端共享的归一化协议

#![allow(missing_docs)]

/// 渲染命令模块
pub mod command;
/// 渲染上下文模块
pub mod context;
mod path_geometry;
/// 渲染提交协议模块
pub mod submission;
/// Text shaping and immutable layout snapshots.
pub mod text;
/// 渲染器 traits 模块
pub mod traits;

/// Shared implementation protocol for rendering backend crates.
#[doc(hidden)]
pub mod backend_support {
    pub use crate::render::command::{ImageDrawDisposition, validate_image_draw_geometry};
    pub use crate::render::path_geometry::{CubicSegment, NormalizedPathOp, for_each_normalized};
}

pub use command::{
    DEFAULT_STROKE_MITER_LIMIT, ImageData, ImageDecodeError, ImageDrawError, ImageResourceRef,
    LineCap, LineJoin, LineStyle, RenderCommand, RenderCommandKind,
};
pub use context::NdCanvas;
pub use submission::{
    BackendSessionDecision, BackendSessionGate, BackendSessionId, DamageMode, DamageSet, FontData,
    FrameId, RenderSubmission, ResourceDelta, ResourceId, ResourceOp, ResourcePayload,
    ResourceSnapshot, ResourceSync, ResourceUpdate, SurfaceInfo,
};
pub use text::{
    BuiltinFont, CaretGeometry, FontDescriptor, FontFaceRef, FontStyle, GlyphPaint, GlyphRun,
    ParleyTextEngine, PositionedGlyph, SelectionQuad, TextAffinity, TextConstraints, TextEngine,
    TextError, TextInteractionError, TextInteractionMap, TextInteractionProvider, TextLayout,
    TextLayoutEngine, TextLayoutKey, TextLayoutParts, TextLayoutRevision, TextLineMetrics,
    TextMovement, TextPosition, TextRange,
};
pub use traits::{
    BackendCapabilities, RenderBackend, RenderCapability, RenderOutcome,
    UnsupportedRenderCapability,
};
