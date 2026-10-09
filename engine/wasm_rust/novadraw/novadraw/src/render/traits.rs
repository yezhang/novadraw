//! 渲染器 traits
//!
//! 定义渲染器的抽象接口，支持不同的渲染后端实现。

use std::error::Error;
use std::fmt;

use crate::render::submission::RenderSubmission;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCapability {
    ProjectiveComposition,
    GlyphRuns,
    ImageResources,
    CustomStrokes,
    PathClips,
    LinearGradients,
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
    pub glyph_runs: bool,
    pub image_resources: bool,
    pub custom_strokes: bool,
    pub path_clips: bool,
    pub linear_gradients: bool,
}

impl BackendCapabilities {
    pub const FULL_FRAME_ONLY: Self = Self {
        partial_damage: false,
        retained_surface: false,
        projective_composition: false,
        glyph_runs: false,
        image_resources: false,
        custom_strokes: false,
        path_clips: false,
        linear_gradients: false,
    };

    pub const RETAINED_PARTIAL: Self = Self {
        partial_damage: true,
        retained_surface: true,
        projective_composition: false,
        glyph_runs: true,
        image_resources: true,
        custom_strokes: false,
        path_clips: false,
        linear_gradients: false,
    };

    pub const fn supports_partial_damage(self) -> bool {
        self.partial_damage && self.retained_surface
    }

    pub const fn supports(self, capability: RenderCapability) -> bool {
        match capability {
            RenderCapability::ProjectiveComposition => self.projective_composition,
            RenderCapability::GlyphRuns => self.glyph_runs,
            RenderCapability::ImageResources => self.image_resources,
            RenderCapability::CustomStrokes => self.custom_strokes,
            RenderCapability::PathClips => self.path_clips,
            RenderCapability::LinearGradients => self.linear_gradients,
        }
    }

    pub const fn with_glyph_runs(mut self) -> Self {
        self.glyph_runs = true;
        self
    }

    pub const fn with_image_resources(mut self) -> Self {
        self.image_resources = true;
        self
    }

    pub const fn with_custom_strokes(mut self) -> Self {
        self.custom_strokes = true;
        self
    }

    pub const fn with_path_clips(mut self) -> Self {
        self.path_clips = true;
        self
    }

    pub const fn with_linear_gradients(mut self) -> Self {
        self.linear_gradients = true;
        self
    }

    /// Checks all requirements before accepting resources or publishing a frame.
    pub fn validate_capabilities(
        self,
        commands: &[crate::render::command::RenderCommand],
    ) -> Result<(), UnsupportedRenderCapability> {
        for command in commands {
            for capability in command.kind.required_capabilities() {
                self.require(capability)?;
            }
        }
        Ok(())
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

/// Successful backend handling of one submitted frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderDisposition {
    /// The frame was published to its target.
    Presented,
    /// The target was intentionally not updated, for example while occluded.
    Skipped,
    /// A transient condition prevented publication on the current backend session.
    Retry,
}

/// Stable backend-neutral diagnostics for an operational render failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderFailureDiagnostic {
    summary: String,
    source_chain: Vec<String>,
}

impl RenderFailureDiagnostic {
    /// Creates a diagnostic without a nested source.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            source_chain: Vec::new(),
        }
    }

    /// Captures an error and its source chain without exposing backend-specific types.
    pub fn from_error(error: &(dyn Error + 'static)) -> Self {
        let mut source_chain = Vec::new();
        let mut source = error.source();
        while let Some(error) = source {
            source_chain.push(error.to_string());
            source = error.source();
        }
        Self {
            summary: error.to_string(),
            source_chain,
        }
    }

    /// Returns the top-level backend diagnostic.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Returns nested source messages in causal order.
    pub fn source_chain(&self) -> &[String] {
        &self.source_chain
    }
}

impl fmt::Display for RenderFailureDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.summary)
    }
}

impl Error for RenderFailureDiagnostic {}

/// Stable class used by Runtime and hosts to choose a recovery action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderFailureKind {
    Unsupported,
    InvalidInput,
    SurfaceLost,
    DeviceLost,
    OutOfMemory,
    Fatal,
}

/// Failure to accept or publish a render submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderFailure {
    Unsupported(UnsupportedRenderCapability),
    InvalidInput(super::InvalidGraphicsInput),
    SurfaceLost(RenderFailureDiagnostic),
    DeviceLost(RenderFailureDiagnostic),
    OutOfMemory(RenderFailureDiagnostic),
    Fatal(RenderFailureDiagnostic),
}

impl RenderFailure {
    /// Returns the recovery-relevant failure class.
    pub const fn kind(&self) -> RenderFailureKind {
        match self {
            Self::Unsupported(_) => RenderFailureKind::Unsupported,
            Self::InvalidInput(_) => RenderFailureKind::InvalidInput,
            Self::SurfaceLost(_) => RenderFailureKind::SurfaceLost,
            Self::DeviceLost(_) => RenderFailureKind::DeviceLost,
            Self::OutOfMemory(_) => RenderFailureKind::OutOfMemory,
            Self::Fatal(_) => RenderFailureKind::Fatal,
        }
    }
}

impl fmt::Display for RenderFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(error) => error.fmt(formatter),
            Self::InvalidInput(error) => error.fmt(formatter),
            Self::SurfaceLost(error) => write!(formatter, "render surface lost: {error}"),
            Self::DeviceLost(error) => write!(formatter, "render device lost: {error}"),
            Self::OutOfMemory(error) => write!(formatter, "render backend out of memory: {error}"),
            Self::Fatal(error) => write!(formatter, "fatal render backend failure: {error}"),
        }
    }
}

impl Error for RenderFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unsupported(error) => Some(error),
            Self::InvalidInput(error) => Some(error),
            Self::SurfaceLost(error)
            | Self::DeviceLost(error)
            | Self::OutOfMemory(error)
            | Self::Fatal(error) => Some(error),
        }
    }
}

/// Complete result of one backend submission attempt.
pub type RenderResult = Result<RenderDisposition, RenderFailure>;

/// 渲染后端 trait
///
/// 定义渲染后端的通用接口。
pub trait RenderBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
    }

    /// Submits one runtime-prepared frame.
    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult;

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
