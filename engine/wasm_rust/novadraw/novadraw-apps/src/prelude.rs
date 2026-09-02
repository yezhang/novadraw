//! Novadraw 应用预导入模块
//!
//! 导入常用的类型和函数，方便快速开发。

pub use crate::{
    AppBuilder, DemoApp, WebInputAdapter, WebPlatformHost, WebPointerInput, WebWheelDeltaMode,
    WinitPlatformHost, run_demo_app, run_demo_app_with_scene_screenshot,
    run_demo_app_with_screenshot,
};
pub use novadraw::{
    BlockId, Color, EllipseFigure, Figure, FigureEvent, FigureGraph, NotificationEffect,
    PolylineFigure, Rectangle, RectangleFigure, Runtime, UpdateEvent, UpdateListener,
};
pub use winit::event::ElementState;
