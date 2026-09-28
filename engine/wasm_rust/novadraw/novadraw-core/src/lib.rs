//! Novadraw Core Library
//!
//! 核心类型定义，包括颜色等基础类型。
//!
//! # 模块
//!
//! - [`color`] - RGBA 颜色类型

#![deny(missing_docs)]

pub mod color;

pub use color::{Color, ColorError, ParseColorError};
