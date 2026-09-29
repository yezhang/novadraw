//! Novadraw Render Library
//!
//! 渲染抽象层，包括渲染命令、渲染上下文和渲染器 traits。
//!
//! # 模块
//!
//! - [`command`] - 渲染命令类型
//! - [`context`] - 渲染上下文
//! - [`traits`] - 渲染器 traits
//! - [`backend`] - 渲染后端实现

#![allow(missing_docs)]

/// 渲染后端模块
#[cfg(any(feature = "vello", feature = "vello-web"))]
pub mod backend;
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
    BuiltinFont, FontDescriptor, FontFaceRef, FontStyle, GlyphPaint, GlyphRun, ParleyTextEngine,
    PositionedGlyph, TextConstraints, TextEngine, TextError, TextLayout, TextLayoutEngine,
    TextLayoutKey, TextLayoutParts, TextLineMetrics,
};
pub use traits::{
    BackendCapabilities, RenderBackend, RenderCapability, RenderOutcome,
    UnsupportedRenderCapability,
};
