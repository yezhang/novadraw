//! Novadraw 演示应用公共库
//!
//! 提供通用的应用框架，简化演示应用的开发。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use novadraw_apps::run_demo_app;
//! use novadraw::FigureGraph;
//!
//! fn create_scene() -> FigureGraph {
//!     let mut scene = FigureGraph::new();
//!     // 创建场景...
//!     scene
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     run_demo_app("My App", "my-app", vec![
//!         ("Scene 1", Box::new(create_scene)),
//!     ])
//! }
//! ```

pub mod app;
pub mod prelude;
pub mod verification;

pub use app::{
    AppBuilder, DemoApp, run_demo_app, run_demo_app_with_scene_screenshot,
    run_demo_app_with_screenshot,
};
pub use prelude::*;
pub use verification::{VerificationCase, VerificationCli, VerificationMetrics, run_verification};
