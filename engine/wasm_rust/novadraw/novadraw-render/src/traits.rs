//! 渲染器 traits
//!
//! 定义渲染器的抽象接口，支持不同的渲染后端实现。

use crate::submission::RenderSubmission;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderOutcome {
    Presented,
    Skipped,
    Retry,
}

/// 窗口代理 trait
///
/// 提供窗口的基本信息和方法。
pub trait WindowProxy: Send + Sync {
    /// 请求重绘
    fn request_redraw(&self);
    /// 获取缩放因子
    fn scale_factor(&self) -> f64;
    /// 获取窗口宽度
    fn width(&self) -> u32;
    /// 获取窗口高度
    fn height(&self) -> u32;
}

/// 渲染后端 trait
///
/// 定义渲染后端的通用接口。
pub trait RenderBackend {
    /// 关联的窗口代理类型
    type Window: WindowProxy;

    /// 获取关联的窗口代理
    fn window(&self) -> &Self::Window;

    /// 执行渲染
    fn render(&mut self, submission: &RenderSubmission) -> RenderOutcome;

    /// 处理窗口大小变化
    ///
    /// 后端可以将连续 resize 合并，并在下一次 `render` 开始时应用最新尺寸，以保证
    /// surface 配置、尺寸相关资源和对应帧处于同一个提交边界。
    ///
    /// # 参数
    /// - `pixel_width`: 物理像素宽度
    /// - `pixel_height`: 物理像素高度
    /// - `scale_factor`: 当前缩放因子
    fn resize(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f64);
}
