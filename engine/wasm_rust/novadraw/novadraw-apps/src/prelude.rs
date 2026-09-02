//! Novadraw 应用预导入模块
//!
//! 导入常用的类型和函数，方便快速开发。

#[cfg(feature = "native")]
pub use crate::{
    AppBuilder, DemoApp, WinitPlatformHost, run_demo_app, run_demo_app_with_scene_screenshot,
    run_demo_app_with_screenshot,
};
pub use crate::{WebInputAdapter, WebPlatformHost, WebPointerInput, WebWheelDeltaMode};
pub use novadraw::{
    BlockId, Color, EllipseFigure, Figure, FigureEvent, FigureGraph, NotificationEffect,
    PolylineFigure, Rectangle, RectangleFigure, Runtime, UpdateEvent, UpdateListener,
};
#[cfg(feature = "native")]
pub use winit::event::ElementState;
