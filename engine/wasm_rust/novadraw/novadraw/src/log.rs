//! 日志工具模块
//!
//! 提供渲染调试日志宏，与 `tracing` 配合使用。
//!
//! ## 使用方式
//!
//! ```rust
//! use novadraw::{debug_render, info_render, trace_render};
//!
//! // 调试渲染
//! debug_render!("EnterFigure: rendering figure");
//!
//! // 详细执行路径
//! trace_render!("clip_rect at x={} y={}", 100.0, 200.0);
//!
//! // 重要事件
//! info_render!("Scene loaded: {} figures", 42);
//! ```
//!
//! ## 运行时控制
//!
//! 使用 `RUST_LOG` 环境变量控制日志级别：
//!
//! ```bash
//! # 开启调试日志
//! RUST_LOG=novadraw=debug cargo run
//!
//! # 开启详细追踪
//! RUST_LOG=novadraw=trace cargo run
//!
//! # 关闭日志
//! RUST_LOG=off cargo run
//! ```

/// 渲染调试日志 - debug 级别
///
/// 用于渲染过程中的临时调试（bounds、clip 区域等）。
#[macro_export]
macro_rules! debug_render {
    ($($arg:tt)*) => (tracing::debug!($($arg)*));
}

/// 渲染跟踪日志 - trace 级别
///
/// 用于详细的算法执行路径跟踪。
#[macro_export]
macro_rules! trace_render {
    ($($arg:tt)*) => (tracing::trace!($($arg)*));
}

/// 渲染信息日志 - info 级别
///
/// 用于重要的渲染事件。
#[macro_export]
macro_rules! info_render {
    ($($arg:tt)*) => (tracing::info!($($arg)*));
}

/// 渲染警告日志 - warn 级别
#[macro_export]
macro_rules! warn_render {
    ($($arg:tt)*) => (tracing::warn!($($arg)*));
}

/// 渲染错误日志 - error 级别
#[macro_export]
macro_rules! error_render {
    ($($arg:tt)*) => (tracing::error!($($arg)*));
}

/// 创建带 span 的调试日志
///
/// 用于追踪嵌套的渲染调用。
#[macro_export]
macro_rules! debug_span {
    ($name:expr $(, $key:expr => $value:expr)*) => {
        tracing::debug_span!($name $(, $key => $value)*)
    };
}
