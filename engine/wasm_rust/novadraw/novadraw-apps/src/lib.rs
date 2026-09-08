//! Novadraw 演示应用公共库
//!
//! 提供通用的应用框架，简化演示应用的开发。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use novadraw_apps::run_demo_app;
//! use novadraw::FigureTree;
//!
//! fn create_scene() -> FigureTree {
//!     let mut scene = FigureTree::new();
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

#[cfg(feature = "native")]
pub mod app;
pub mod input;
pub mod platform;
pub mod prelude;
#[cfg(feature = "native")]
pub mod verification;

#[cfg(feature = "native")]
pub use app::{
    AppBuilder, DemoApp, run_demo_app, run_demo_app_with_scene_screenshot,
    run_demo_app_with_screenshot, run_runtime_demo_app, run_runtime_demo_app_with_scene_screenshot,
    run_runtime_demo_app_with_screenshot,
};
#[cfg(feature = "native")]
pub use input::WinitGestureAdapter;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WebInputAdapter, WebPointerInput, WebWheelDeltaMode,
    adapt_key_input,
};
pub use platform::WebPlatformHost;
#[cfg(feature = "native")]
pub use platform::WinitPlatformHost;
pub use prelude::*;
#[cfg(feature = "native")]
pub use verification::{VerificationCase, VerificationCli, VerificationMetrics, run_verification};
