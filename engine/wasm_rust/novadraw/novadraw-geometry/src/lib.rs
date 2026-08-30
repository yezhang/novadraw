//! Novadraw Geometry Library
//!
//! 几何库，提供 2D 图形所需的几何类型。
//!
//! # 模块
//!
//! - [`vec2`] - 2D 向量类型
//! - [`rect`] - 矩形、点、尺寸类型
//! - [`point_list`] - 点序列类型
//! - [`precision`] - 几何精度工具
//! - [`transform`] - 仿射变换
//! - [`translatable`] - 可变形 Trait 和内边距类型

#![deny(missing_docs)]

pub mod point_list;
pub mod precision;
pub mod rect;
pub mod transform;
pub mod translatable;
pub mod vec2;

pub use point_list::PointList;
pub use precision::{ApproxEq, DEFAULT_EPSILON, Precision};
pub use rect::{Dimension, Point, Rectangle, Size};
pub use transform::Transform;
pub use translatable::{Insets, Translatable};
pub use vec2::Vec2;

/// 精度点兼容别名。
///
/// Novadraw 的几何值默认使用 `f64`，因此 precision 命名映射到同一底层类型。
pub type PrecisionPoint = Point;

/// 精度矩形兼容别名。
///
/// Novadraw 的几何值默认使用 `f64`，因此 precision 命名映射到同一底层类型。
pub type PrecisionRectangle = Rectangle;

/// 精度尺寸兼容别名。
///
/// Novadraw 的几何值默认使用 `f64`，因此 precision 命名映射到同一底层类型。
pub type PrecisionDimension = Dimension;

/// 向量兼容别名。
pub type Vector = Vec2;

/// Canonical 2D affine transform name used by the scene architecture.
pub type Affine2D = Transform;

/// 仿射变换兼容别名。
pub type AffineTransform = Transform;
