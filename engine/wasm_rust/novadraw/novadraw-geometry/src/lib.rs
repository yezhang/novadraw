//! Novadraw Geometry Library
//!
//! 几何库，提供 2D 图形所需的几何类型。
//!
//! # 模块
//!
//! - [`point`] - 2D 点类型
//! - [`vec2`] - 2D 向量类型
//! - [`rect`] - 矩形和尺寸类型
//! - [`point_list`] - 点序列类型
//! - [`precision`] - 几何精度工具
//! - [`transform`] - 仿射变换
//! - [`translatable`] - 可变形 Trait 和内边距类型

#![deny(missing_docs)]

pub mod point;
pub mod point_list;
pub mod precision;
pub mod rect;
pub mod transform;
pub mod translatable;
pub mod vec2;

pub use point::Point;
pub use point_list::PointList;
pub use precision::{ApproxEq, DEFAULT_EPSILON, Precision};
pub use rect::{Dimension, Rectangle};
pub use transform::Affine2D;
pub use translatable::{Insets, Translatable};
pub use vec2::Vec2;
