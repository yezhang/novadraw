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
#[cfg(feature = "vello")]
pub mod backend;
/// 渲染命令模块
pub mod command;
/// 渲染上下文模块
pub mod context;
/// 渲染提交协议模块
pub mod submission;
/// 渲染器 traits 模块
pub mod traits;

pub use command::{ImageData, LineCap, LineJoin, LineStyle, RenderCommand, RenderCommandKind};
pub use context::NdCanvas;
pub use submission::{DamageMode, DamageSet, RenderSubmission, ResourceDelta, SurfaceInfo};
pub use traits::{RenderBackend, RenderOutcome, WindowProxy};
