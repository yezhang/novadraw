//! 渲染器 traits
//!
//! 定义渲染器的抽象接口，支持不同的渲染后端实现。

use std::error::Error;
use std::fmt;

use crate::submission::RenderSubmission;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCapability {
    ProjectiveComposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedRenderCapability {
    pub capability: RenderCapability,
}

impl fmt::Display for UnsupportedRenderCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "render backend does not support {:?}",
            self.capability
        )
    }
}

impl Error for UnsupportedRenderCapability {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendCapabilities {
    pub partial_damage: bool,
    pub retained_surface: bool,
    pub projective_composition: bool,
}

impl BackendCapabilities {
    pub const FULL_FRAME_ONLY: Self = Self {
        partial_damage: false,
        retained_surface: false,
        projective_composition: false,
    };

    pub const RETAINED_PARTIAL: Self = Self {
        partial_damage: true,
        retained_surface: true,
        projective_composition: false,
    };

    pub const fn supports_partial_damage(self) -> bool {
        self.partial_damage && self.retained_surface
    }

    pub const fn supports(self, capability: RenderCapability) -> bool {
        match capability {
            RenderCapability::ProjectiveComposition => self.projective_composition,
        }
    }

    pub fn require(self, capability: RenderCapability) -> Result<(), UnsupportedRenderCapability> {
        if self.supports(capability) {
            Ok(())
        } else {
            Err(UnsupportedRenderCapability { capability })
        }
    }
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self::FULL_FRAME_ONLY
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderOutcome {
    Presented,
    Skipped,
    Retry,
}

/// 渲染后端 trait
///
/// 定义渲染后端的通用接口。
pub trait RenderBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
    }

    /// Submits one runtime-prepared frame.
    fn submit(&mut self, submission: &RenderSubmission) -> RenderOutcome;

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
