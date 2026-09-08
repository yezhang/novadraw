//! 渲染命令类型
//!
//! 定义了所有可用的渲染操作命令。

use std::fmt;

use novadraw_core::Color;
use novadraw_geometry::Transform;

use crate::submission::ResourceId;
use crate::text::{GlyphPaint, GlyphRun};

/// 渲染命令
///
/// 包含一个渲染操作类型。
#[derive(Clone, Debug)]
pub struct RenderCommand {
    /// 命令类型
    pub kind: RenderCommandKind,
}

/// 渲染命令类型
///
/// 初期只支持矩形绘制，其他图形保留扩展。
#[derive(Clone, Debug)]
pub enum RenderCommandKind {
    /// 保存当前 transform 和 clip 到栈
    PushState,

    /// 恢复到最近保存状态，不弹出
    RestoreState,

    /// 弹出并恢复到最近保存状态
    PopState,

    /// 叠加变换矩阵
    ConcatTransform {
        /// 变换矩阵
        matrix: Transform,
    },

    /// 替换当前变换矩阵
    SetTransform {
        /// 变换矩阵
        matrix: Transform,
    },

    /// 重置当前变换矩阵为单位矩阵
    ResetTransform,

    /// 设置裁剪区域
    Clip {
        /// 裁剪矩形 [左上角, 右下角]
        rect: [glam::DVec2; 2],
    },

    /// 清空当前裁剪区域
    ResetClip,

    /// 设置全局透明度
    SetGlobalAlpha {
        /// 透明度，范围 [0.0, 1.0]
        alpha: f64,
    },

    /// 清除矩形区域
    ClearRect {
        rect: [glam::DVec2; 2],
        color: Color,
    },

    /// 填充矩形
    FillRect {
        rect: [glam::DVec2; 2],
        color: Color,
    },

    /// 描边矩形
    StrokeRect {
        rect: [glam::DVec2; 2],
        color: Color,
        width: f64,
        /// 线型样式
        line_style: LineStyle,
        /// 线帽样式
        cap: LineCap,
        /// 连接样式
        join: LineJoin,
    },

    /// 绘制椭圆
    Ellipse {
        /// 椭圆中心 x
        cx: f64,
        /// 椭圆中心 y
        cy: f64,
        /// x 轴半径
        rx: f64,
        /// y 轴半径
        ry: f64,
        /// 填充颜色
        fill_color: Option<Color>,
        /// 描边颜色
        stroke_color: Option<Color>,
        /// 描边宽度
        stroke_width: f64,
        /// 线型样式
        line_style: LineStyle,
        /// 线帽样式
        cap: LineCap,
        /// 连接样式
        join: LineJoin,
    },

    /// 绘制直线
    Line {
        /// 起点
        p1: glam::DVec2,
        /// 终点
        p2: glam::DVec2,
        /// 线条颜色
        color: Color,
        /// 线条宽度
        width: f64,
        /// 线型样式
        line_style: LineStyle,
        /// 线帽样式
        cap: LineCap,
        /// 连接样式
        join: LineJoin,
    },

    /// 绘制折线
    Polyline {
        /// 点列表
        points: Vec<glam::DVec2>,
        /// 线条颜色
        color: Color,
        /// 线条宽度
        width: f64,
        /// 线型样式
        line_style: LineStyle,
        /// 线帽样式
        cap: LineCap,
        /// 连接样式
        join: LineJoin,
    },

    /// 绘制路径
    Path {
        /// 路径数据
        path: Path,
        /// 填充颜色
        fill_color: Option<Color>,
        /// 描边颜色
        stroke_color: Option<Color>,
        /// 描边宽度
        stroke_width: f64,
        /// 填充规则
        fill_rule: FillRule,
    },

    /// 填充路径
    FillPath {
        /// 路径数据
        path: Path,
        /// 填充颜色
        color: Color,
    },

    /// 描边路径
    StrokePath {
        /// 路径数据
        path: Path,
        /// 描边颜色
        color: Color,
        /// 描边宽度
        width: f64,
        /// 线型样式
        line_style: LineStyle,
        /// 线帽样式
        line_cap: LineCap,
        /// 线连接样式
        line_join: LineJoin,
    },

    /// 绘制图像
    Image {
        /// 精确的图像资源 revision。
        image: ImageResourceRef,
        /// 目标矩形 [左上角, 右下角]
        dest_rect: [glam::DVec2; 2],
        /// 源矩形 [左上角, 右下角]，None 表示整个图像
        src_rect: Option<[glam::DVec2; 2]>,
        /// 绘制透明度
        alpha: f64,
    },

    /// 绘制 backend-neutral、已完成 shaping 和定位的 glyph run。
    DrawGlyphRun {
        run: GlyphRun,
        origin: glam::DVec2,
        paint: GlyphPaint,
    },
}

/// 线帽样式
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// 线连接样式
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// 线型样式
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dash,
    Dot,
}

/// 填充规则
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// 路径数据类型
#[derive(Clone, Debug, Default)]
pub struct Path {
    operations: Vec<PathOp>,
}

impl Path {
    /// 创建空路径
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// 移动到指定点（起点）
    pub fn move_to(&mut self, x: f64, y: f64) {
        self.operations.push(PathOp::MoveTo(glam::DVec2::new(x, y)));
    }

    /// 直线连接到指定点
    pub fn line_to(&mut self, x: f64, y: f64) {
        self.operations.push(PathOp::LineTo(glam::DVec2::new(x, y)));
    }

    /// 水平线
    pub fn h_line_to(&mut self, x: f64) {
        self.operations.push(PathOp::HLineTo(x));
    }

    /// 垂直线
    pub fn v_line_to(&mut self, y: f64) {
        self.operations.push(PathOp::VLineTo(y));
    }

    /// 贝塞尔曲线
    pub fn cubic_to(&mut self, cx1: f64, cy1: f64, cx2: f64, cy2: f64, x: f64, y: f64) {
        self.operations.push(PathOp::CubicTo(
            glam::DVec2::new(cx1, cy1),
            glam::DVec2::new(cx2, cy2),
            glam::DVec2::new(x, y),
        ));
    }

    /// 二次贝塞尔曲线
    pub fn quad_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        self.operations.push(PathOp::QuadTo(
            glam::DVec2::new(cx, cy),
            glam::DVec2::new(x, y),
        ));
    }

    /// 闭合路径
    pub fn close(&mut self) {
        self.operations.push(PathOp::Close);
    }

    /// 绘制弧线到指定点
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        &mut self,
        rx: f64,
        ry: f64,
        rotation: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    ) {
        self.operations.push(PathOp::Arc {
            radii: glam::DVec2::new(rx, ry),
            rotation,
            large_arc,
            sweep,
            dest: glam::DVec2::new(x, y),
        });
    }

    /// 绘制矩形（添加到路径）
    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.move_to(x, y);
        self.line_to(x + width, y);
        self.line_to(x + width, y + height);
        self.line_to(x, y + height);
        self.close();
    }

    /// 获取包围盒
    pub fn bounding_box(&self) -> Option<glam::DVec4> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        let mut current = glam::DVec2::ZERO;
        for op in &self.operations {
            match op {
                PathOp::MoveTo(p) | PathOp::LineTo(p) => {
                    current = *p;
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                PathOp::HLineTo(x) => {
                    current = glam::DVec2::new(*x, current.y);
                    min_x = min_x.min(*x);
                    max_x = max_x.max(*x);
                }
                PathOp::VLineTo(y) => {
                    current = glam::DVec2::new(current.x, *y);
                    min_y = min_y.min(*y);
                    max_y = max_y.max(*y);
                }
                PathOp::CubicTo(_, _, p) | PathOp::QuadTo(_, p) => {
                    current = *p;
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                PathOp::Arc { dest, .. } => {
                    current = *dest;
                    min_x = min_x.min(dest.x);
                    min_y = min_y.min(dest.y);
                    max_x = max_x.max(dest.x);
                    max_y = max_y.max(dest.y);
                }
                PathOp::Close => {}
            }
        }

        if min_x.is_infinite() {
            None
        } else {
            Some(glam::DVec4::new(min_x, min_y, max_x, max_y))
        }
    }

    /// 获取路径操作列表
    pub fn operations(&self) -> &[PathOp] {
        &self.operations
    }
}

/// 路径操作
#[derive(Clone, Debug)]
pub enum PathOp {
    /// 移动到指定点（起点）
    MoveTo(glam::DVec2),
    /// 直线连接到指定点
    LineTo(glam::DVec2),
    /// 水平线到指定 x
    HLineTo(f64),
    /// 垂直线到指定 y
    VLineTo(f64),
    /// 三次贝塞尔曲线
    CubicTo(glam::DVec2, glam::DVec2, glam::DVec2),
    /// 二次贝塞尔曲线
    QuadTo(glam::DVec2, glam::DVec2),
    /// 弧线
    Arc {
        radii: glam::DVec2,
        rotation: f64,
        large_arc: bool,
        sweep: bool,
        dest: glam::DVec2,
    },
    /// 闭合路径
    Close,
}

/// 图像数据类型
#[derive(Clone, Debug, PartialEq)]
pub struct ImageData {
    /// 图像宽度
    pub width: u32,
    /// 图像高度
    pub height: u32,
    /// 像素数据 (RGBA)
    pub pixels: Vec<u8>,
    /// 像素缩放因子（用于高DPI）
    pub scale: f64,
}

/// Backend-neutral reference to an immutable image resource revision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageResourceRef {
    resource_id: ResourceId,
    revision: u64,
    width: u32,
    height: u32,
    scale: f64,
}

impl ImageResourceRef {
    pub const fn new(
        resource_id: ResourceId,
        revision: u64,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Self {
        Self {
            resource_id,
            revision,
            width,
            height,
            scale,
        }
    }

    pub const fn resource_id(self) -> ResourceId {
        self.resource_id
    }

    pub const fn revision(self) -> u64 {
        self.revision
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn scale(self) -> f64 {
        self.scale
    }

    pub fn logical_size(self) -> (f64, f64) {
        (
            f64::from(self.width) / self.scale,
            f64::from(self.height) / self.scale,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageDecodeError {
    EmptyInput,
    UnsupportedFormat,
    InvalidPng,
    InvalidSvg,
    InvalidScale,
    RasterizationFailed,
    ResourceUpdateFailed,
    Io(String),
}

impl fmt::Display for ImageDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => formatter.write_str("image data is empty"),
            Self::UnsupportedFormat => formatter.write_str("image format is not supported"),
            Self::InvalidPng => formatter.write_str("PNG data is invalid"),
            Self::InvalidSvg => formatter.write_str("SVG data is invalid"),
            Self::InvalidScale => formatter.write_str("image scale must be finite and positive"),
            Self::RasterizationFailed => formatter.write_str("SVG rasterization failed"),
            Self::ResourceUpdateFailed => formatter.write_str("image resource update failed"),
            Self::Io(message) => write!(formatter, "image file could not be read: {message}"),
        }
    }
}

impl std::error::Error for ImageDecodeError {}

impl ImageData {
    /// 从 RGBA 像素数据创建
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>, scale: f64) -> Self {
        Self {
            width,
            height,
            pixels,
            scale,
        }
    }

    pub fn decode(bytes: &[u8], scale: f64) -> Result<Self, ImageDecodeError> {
        if bytes.is_empty() {
            return Err(ImageDecodeError::EmptyInput);
        }
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Self::decode_png(bytes, scale);
        }
        let first_non_whitespace = bytes
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        let content = &bytes[first_non_whitespace..];
        if content.starts_with(b"<svg") || content.starts_with(b"<?xml") {
            return Self::decode_svg(bytes, scale);
        }
        Err(ImageDecodeError::UnsupportedFormat)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode_file(
        path: impl AsRef<std::path::Path>,
        scale: f64,
    ) -> Result<Self, ImageDecodeError> {
        let bytes = std::fs::read(path).map_err(|error| ImageDecodeError::Io(error.to_string()))?;
        Self::decode(&bytes, scale)
    }

    pub fn decode_png(bytes: &[u8], scale: f64) -> Result<Self, ImageDecodeError> {
        validate_scale(scale)?;
        let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|_| ImageDecodeError::InvalidPng)?
            .to_rgba8();
        Ok(Self::from_rgba(
            image.width(),
            image.height(),
            image.into_raw(),
            scale,
        ))
    }

    pub fn decode_svg(bytes: &[u8], scale: f64) -> Result<Self, ImageDecodeError> {
        validate_scale(scale)?;
        let mut options = resvg::usvg::Options::default();
        {
            let fontdb = options.fontdb_mut();
            for font in crate::text::BuiltinFont::ALL {
                fontdb.load_font_data(font.bytes().to_vec());
            }
            fontdb.set_sans_serif_family(crate::text::BuiltinFont::Inter.family());
            fontdb.set_serif_family(crate::text::BuiltinFont::Inter.family());
            fontdb.set_monospace_family(crate::text::BuiltinFont::JetBrainsMono.family());
        }
        let tree = resvg::usvg::Tree::from_data(bytes, &options)
            .map_err(|_| ImageDecodeError::InvalidSvg)?;
        let size = tree.size().to_int_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or(ImageDecodeError::RasterizationFailed)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let mut pixels = pixmap.take();
        unpremultiply_rgba(&mut pixels);
        Ok(Self::from_rgba(size.width(), size.height(), pixels, scale))
    }
}

fn validate_scale(scale: f64) -> Result<(), ImageDecodeError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(())
    } else {
        Err(ImageDecodeError::InvalidScale)
    }
}

fn unpremultiply_rgba(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for component in &mut pixel[..3] {
            *component = ((u32::from(*component) * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    #[test]
    fn decodes_png_and_svg_into_rgba() {
        let mut png_bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let png = ImageData::decode(&png_bytes, 1.0).unwrap();
        assert_eq!((png.width, png.height, png.pixels.len()), (1, 1, 4));

        let svg = ImageData::decode(
            br##"<?xml version="1.0"?><svg width="2" height="3" xmlns="http://www.w3.org/2000/svg"><rect width="2" height="3" fill="#ff0000"/></svg>"##,
            2.0,
        )
        .unwrap();
        assert_eq!((svg.width, svg.height, svg.scale), (2, 3, 2.0));
        assert!(svg.pixels.iter().any(|pixel| *pixel != 0));
    }
}
