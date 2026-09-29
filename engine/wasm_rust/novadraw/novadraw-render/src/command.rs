//! 渲染命令类型
//!
//! 定义了所有可用的渲染操作命令。

use std::fmt;

use kurbo::Shape as _;
use novadraw_core::Color;
use novadraw_geometry::{Affine2D, Dimension, Point, PointList, Rectangle};

use crate::submission::ResourceId;
use crate::text::{GlyphPaint, GlyphRun};

/// Default ratio between miter length and stroke radius.
pub const DEFAULT_STROKE_MITER_LIMIT: f64 = 4.0;

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
        matrix: Affine2D,
    },

    /// 替换当前变换矩阵
    SetTransform {
        /// 变换矩阵
        matrix: Affine2D,
    },

    /// 重置当前变换矩阵为单位矩阵
    ResetTransform,

    /// 设置裁剪区域
    Clip {
        /// 裁剪矩形。
        rect: Rectangle,
    },

    /// 清空当前裁剪区域
    ResetClip,

    /// 设置全局透明度
    SetGlobalAlpha {
        /// 透明度，范围 [0.0, 1.0]
        alpha: f64,
    },

    /// 清除矩形区域
    ClearRect { rect: Rectangle, color: Color },

    /// 填充矩形
    FillRect { rect: Rectangle, color: Color },

    /// 描边矩形
    StrokeRect {
        rect: Rectangle,
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
        p1: Point,
        /// 终点
        p2: Point,
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
        points: PointList,
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
        /// 图像资源物理像素域中的源矩形。
        source_rect: Rectangle,
        /// 目标矩形。
        dest_rect: Rectangle,
        /// 绘制透明度
        alpha: f64,
    },

    /// 绘制 backend-neutral、已完成 shaping 和定位的 glyph run。
    DrawGlyphRun {
        run: GlyphRun,
        origin: Point,
        paint: GlyphPaint,
    },
}

impl RenderCommandKind {
    pub const fn required_capability(&self) -> Option<crate::RenderCapability> {
        match self {
            Self::Image { .. } => Some(crate::RenderCapability::ImageResources),
            Self::DrawGlyphRun { .. } => Some(crate::RenderCapability::GlyphRuns),
            _ => None,
        }
    }
}

/// Invalid geometry supplied to an image drawing operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDrawError {
    /// The source rectangle or one of its derived edges is not finite.
    NonFiniteSource,
    /// The destination rectangle or one of its derived edges is not finite.
    NonFiniteDestination,
    /// The source width or height is negative.
    NegativeSourceExtent,
    /// The destination width or height is negative.
    NegativeDestinationExtent,
    /// The non-empty source rectangle is outside the image's physical pixel bounds.
    SourceOutOfBounds,
}

impl fmt::Display for ImageDrawError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteSource => formatter.write_str("image source rectangle must be finite"),
            Self::NonFiniteDestination => {
                formatter.write_str("image destination rectangle must be finite")
            }
            Self::NegativeSourceExtent => {
                formatter.write_str("image source width and height must not be negative")
            }
            Self::NegativeDestinationExtent => {
                formatter.write_str("image destination width and height must not be negative")
            }
            Self::SourceOutOfBounds => {
                formatter.write_str("image source rectangle must be within the image pixel bounds")
            }
        }
    }
}

impl std::error::Error for ImageDrawError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageDrawDisposition {
    Draw,
    NoOp,
}

pub(crate) fn validate_image_draw_geometry(
    image_width: u32,
    image_height: u32,
    source_rect: Rectangle,
    dest_rect: Rectangle,
) -> Result<ImageDrawDisposition, ImageDrawError> {
    if !rectangle_has_finite_bounds(source_rect) {
        return Err(ImageDrawError::NonFiniteSource);
    }
    if !rectangle_has_finite_bounds(dest_rect) {
        return Err(ImageDrawError::NonFiniteDestination);
    }
    if source_rect.width < 0.0 || source_rect.height < 0.0 {
        return Err(ImageDrawError::NegativeSourceExtent);
    }
    if dest_rect.width < 0.0 || dest_rect.height < 0.0 {
        return Err(ImageDrawError::NegativeDestinationExtent);
    }
    if source_rect.is_empty() || dest_rect.is_empty() {
        return Ok(ImageDrawDisposition::NoOp);
    }
    if source_rect.x < 0.0
        || source_rect.y < 0.0
        || source_rect.x + source_rect.width > f64::from(image_width)
        || source_rect.y + source_rect.height > f64::from(image_height)
    {
        return Err(ImageDrawError::SourceOutOfBounds);
    }
    Ok(ImageDrawDisposition::Draw)
}

fn rectangle_has_finite_bounds(rectangle: Rectangle) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
        && (rectangle.x + rectangle.width).is_finite()
        && (rectangle.y + rectangle.height).is_finite()
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
        self.operations.push(PathOp::MoveTo(Point::new(x, y)));
    }

    /// 直线连接到指定点
    pub fn line_to(&mut self, x: f64, y: f64) {
        self.operations.push(PathOp::LineTo(Point::new(x, y)));
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
            Point::new(cx1, cy1),
            Point::new(cx2, cy2),
            Point::new(x, y),
        ));
    }

    /// 二次贝塞尔曲线
    pub fn quad_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        self.operations
            .push(PathOp::QuadTo(Point::new(cx, cy), Point::new(x, y)));
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
        rotation_degrees: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    ) {
        self.operations.push(PathOp::Arc {
            radii: Dimension::new(rx, ry),
            rotation: rotation_degrees.to_radians(),
            large_arc,
            sweep,
            dest: Point::new(x, y),
        });
    }

    /// Adds a center-defined circular arc using degree angles.
    pub fn arc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        start_degrees: f64,
        end_degrees: f64,
        anticlockwise: bool,
    ) {
        if !x.is_finite()
            || !y.is_finite()
            || !radius.is_finite()
            || radius < 0.0
            || !start_degrees.is_finite()
            || !end_degrees.is_finite()
        {
            return;
        }

        let start_angle = start_degrees.to_radians();
        let sweep_angle = canvas_sweep(start_degrees, end_degrees, anticlockwise);
        let start = kurbo::Point::new(
            x + radius * start_angle.cos(),
            y + radius * start_angle.sin(),
        );
        match self.current_point() {
            None => self.move_to(start.x, start.y),
            Some(current) if current.distance(start) > f64::EPSILON => {
                self.line_to(start.x, start.y);
            }
            Some(_) => {}
        }

        if radius == 0.0 || sweep_angle == 0.0 {
            return;
        }
        kurbo::Arc::new((x, y), (radius, radius), start_angle, sweep_angle, 0.0).to_cubic_beziers(
            0.1,
            |control1, control2, end| {
                self.cubic_to(control1.x, control1.y, control2.x, control2.y, end.x, end.y);
            },
        );
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
    pub fn bounding_box(&self) -> Option<Rectangle> {
        if self.operations.is_empty() {
            return None;
        }
        let bounds = self.to_kurbo_path(1.0).bounding_box();
        Some(Rectangle::new(
            bounds.x0,
            bounds.y0,
            bounds.width(),
            bounds.height(),
        ))
    }

    /// 获取路径操作列表
    pub fn operations(&self) -> &[PathOp] {
        &self.operations
    }

    pub(crate) fn to_kurbo_path(&self, scale: f64) -> kurbo::BezPath {
        let mut path = kurbo::BezPath::new();
        let mut current = None;
        let mut subpath_start = None;

        for operation in &self.operations {
            match operation {
                PathOp::MoveTo(point) => {
                    let point = scaled_point(*point, scale);
                    path.move_to(point);
                    current = Some(point);
                    subpath_start = Some(point);
                }
                PathOp::LineTo(point) => {
                    let point = scaled_point(*point, scale);
                    path.line_to(point);
                    current = Some(point);
                }
                PathOp::HLineTo(x) => {
                    let point = kurbo::Point::new(*x * scale, current.map_or(0.0, |point| point.y));
                    path.line_to(point);
                    current = Some(point);
                }
                PathOp::VLineTo(y) => {
                    let point = kurbo::Point::new(current.map_or(0.0, |point| point.x), *y * scale);
                    path.line_to(point);
                    current = Some(point);
                }
                PathOp::CubicTo(control1, control2, end) => {
                    let end = scaled_point(*end, scale);
                    path.curve_to(
                        scaled_point(*control1, scale),
                        scaled_point(*control2, scale),
                        end,
                    );
                    current = Some(end);
                }
                PathOp::QuadTo(control, end) => {
                    let end = scaled_point(*end, scale);
                    path.quad_to(scaled_point(*control, scale), end);
                    current = Some(end);
                }
                PathOp::Arc {
                    radii,
                    rotation,
                    large_arc,
                    sweep,
                    dest,
                } => {
                    let destination = scaled_point(*dest, scale);
                    let Some(source) = current else {
                        path.move_to(destination);
                        current = Some(destination);
                        subpath_start = Some(destination);
                        continue;
                    };
                    let svg_arc = kurbo::SvgArc {
                        from: source,
                        to: destination,
                        radii: kurbo::Vec2::new(radii.width * scale, radii.height * scale),
                        x_rotation: *rotation,
                        large_arc: *large_arc,
                        sweep: *sweep,
                    };
                    if let Some(arc) = kurbo::Arc::from_svg_arc(&svg_arc) {
                        arc.to_cubic_beziers(0.1, |control1, control2, end| {
                            path.curve_to(control1, control2, end);
                        });
                    } else {
                        path.line_to(destination);
                    }
                    current = Some(destination);
                }
                PathOp::Close => {
                    path.close_path();
                    current = subpath_start;
                }
            }
        }
        path
    }

    fn current_point(&self) -> Option<kurbo::Point> {
        let mut current = None;
        let mut subpath_start = None;
        for operation in &self.operations {
            match operation {
                PathOp::MoveTo(point) => {
                    let point = scaled_point(*point, 1.0);
                    current = Some(point);
                    subpath_start = Some(point);
                }
                PathOp::LineTo(point) | PathOp::CubicTo(_, _, point) | PathOp::QuadTo(_, point) => {
                    current = Some(scaled_point(*point, 1.0));
                }
                PathOp::HLineTo(x) => {
                    current = Some(kurbo::Point::new(*x, current.map_or(0.0, |point| point.y)));
                }
                PathOp::VLineTo(y) => {
                    current = Some(kurbo::Point::new(current.map_or(0.0, |point| point.x), *y));
                }
                PathOp::Arc { dest, .. } => {
                    current = Some(scaled_point(*dest, 1.0));
                }
                PathOp::Close => current = subpath_start,
            }
        }
        current
    }
}

fn scaled_point(point: Point, scale: f64) -> kurbo::Point {
    kurbo::Point::new(point.x() * scale, point.y() * scale)
}

fn canvas_sweep(start_degrees: f64, end_degrees: f64, anticlockwise: bool) -> f64 {
    const FULL_TURN_DEGREES: f64 = 360.0;
    let raw = end_degrees - start_degrees;
    if raw.abs() >= FULL_TURN_DEGREES {
        return if anticlockwise {
            -std::f64::consts::TAU
        } else {
            std::f64::consts::TAU
        };
    }

    let mut sweep = raw.to_radians();
    if anticlockwise && sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    } else if !anticlockwise && sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    }
    sweep
}

/// 路径操作
#[derive(Clone, Debug)]
pub enum PathOp {
    /// 移动到指定点（起点）
    MoveTo(Point),
    /// 直线连接到指定点
    LineTo(Point),
    /// 水平线到指定 x
    HLineTo(f64),
    /// 垂直线到指定 y
    VLineTo(f64),
    /// 三次贝塞尔曲线
    CubicTo(Point, Point, Point),
    /// 二次贝塞尔曲线
    QuadTo(Point, Point),
    /// 弧线
    Arc {
        radii: Dimension,
        rotation: f64,
        large_arc: bool,
        sweep: bool,
        dest: Point,
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
    fn path_bounds_include_curve_extrema() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.cubic_to(0.0, 100.0, 100.0, 100.0, 100.0, 0.0);

        let bounds = path.bounding_box().expect("non-empty path");

        assert!(bounds.x.abs() < 1e-9);
        assert!((bounds.width - 100.0).abs() < 1e-9);
        assert!((bounds.y + bounds.height - 75.0).abs() < 1e-9);
    }

    #[test]
    fn path_bounds_include_svg_arc_extent() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.arc_to(50.0, 50.0, 0.0, false, true, 100.0, 0.0);

        let bounds = path.bounding_box().expect("non-empty path");

        assert!(bounds.x.abs() < 1e-9);
        assert!((bounds.width - 100.0).abs() < 1e-9);
        assert!((bounds.height - 50.0).abs() < 1e-9);
    }

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
