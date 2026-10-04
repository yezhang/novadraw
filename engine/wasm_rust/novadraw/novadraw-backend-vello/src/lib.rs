//! Vello 渲染器实现
//!
//! 实现 RenderCommand 解释器，维护独立的状态栈。
//! 状态管理从 NdCanvas 移到本模块（参考 Skia/Flutter 的 retained command state）。

mod glyph_outline;

use std::collections::HashMap;
use std::fmt;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::sync::Arc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use image::ImageBuffer;
use novadraw::geometry::{Affine2D, Point, Rectangle};
use tracing::debug;
#[cfg(any(feature = "native", target_arch = "wasm32"))]
use vello::RendererOptions;
use vello::kurbo::{Cap, Join, Stroke};
use vello::peniko::Color as VelloColor;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer};

use novadraw::graphics::{ClipPath, FillRule, Paint, StrokeStyle};
use novadraw::render::backend_support::{
    ImageDrawDisposition, NormalizedPathOp, for_each_normalized, validate_image_draw_geometry,
};
use novadraw::render::command::{LineCap, LineJoin, Path, RenderCommand};
use novadraw::render::submission::{
    BackendSessionDecision, BackendSessionGate, DamageMode, ResourcePayload,
};
use novadraw::render::text::{GlyphPaint, GlyphRun};
use novadraw::render::traits::{BackendCapabilities, RenderBackend, RenderOutcome};

const DEFAULT_BACKGROUND_COMPONENT: f64 = 238.0 / 255.0;
const DEFAULT_BACKGROUND_COLOR: vello::wgpu::Color = vello::wgpu::Color {
    r: DEFAULT_BACKGROUND_COMPONENT,
    g: DEFAULT_BACKGROUND_COMPONENT,
    b: DEFAULT_BACKGROUND_COMPONENT,
    a: 1.0,
};

/// A native window that can provide the raw handles required by the GPU surface.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub trait NativeWindow:
    raw_window_handle::HasDisplayHandle + raw_window_handle::HasWindowHandle + Send + Sync + 'static
{
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl<T> NativeWindow for T where
    T: raw_window_handle::HasDisplayHandle
        + raw_window_handle::HasWindowHandle
        + Send
        + Sync
        + 'static
{
}

/// Failure to initialize a Vello rendering surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelloInitializationError {
    message: String,
}

impl VelloInitializationError {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    fn from_vello(error: vello::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for VelloInitializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for VelloInitializationError {}

/// Backend-owned adapter metadata for diagnostics and performance evidence.
#[cfg(any(
    all(feature = "native", not(target_arch = "wasm32")),
    all(feature = "web", target_arch = "wasm32")
))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelloAdapterInfo {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
}

/// Failure while waiting for all submitted GPU work to complete.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelloGpuWaitError {
    message: String,
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl fmt::Display for VelloGpuWaitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl std::error::Error for VelloGpuWaitError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceRecovery {
    Reconfigure,
    Retry,
    Skip,
}

fn surface_recovery(status: &vello::wgpu::CurrentSurfaceTexture) -> Option<SurfaceRecovery> {
    match status {
        vello::wgpu::CurrentSurfaceTexture::Success(_)
        | vello::wgpu::CurrentSurfaceTexture::Suboptimal(_) => None,
        vello::wgpu::CurrentSurfaceTexture::Lost | vello::wgpu::CurrentSurfaceTexture::Outdated => {
            Some(SurfaceRecovery::Reconfigure)
        }
        vello::wgpu::CurrentSurfaceTexture::Timeout
        | vello::wgpu::CurrentSurfaceTexture::Validation => Some(SurfaceRecovery::Retry),
        vello::wgpu::CurrentSurfaceTexture::Occluded => Some(SurfaceRecovery::Skip),
    }
}

fn surface_is_suspended(width: u32, height: u32) -> bool {
    width == 0 || height == 0
}

#[derive(Clone, Copy, Debug)]
struct ImageDrawPlan {
    clip_rect: Rectangle,
    local_affine: vello::kurbo::Affine,
}

fn image_draw_plan(
    image_width: u32,
    image_height: u32,
    source_rect: Rectangle,
    dest_rect: Rectangle,
    scale_factor: f64,
) -> Option<ImageDrawPlan> {
    if !scale_factor.is_finite()
        || scale_factor <= 0.0
        || validate_image_draw_geometry(image_width, image_height, source_rect, dest_rect)
            != Ok(ImageDrawDisposition::Draw)
    {
        return None;
    }

    let source_to_dest_x = dest_rect.width / source_rect.width;
    let source_to_dest_y = dest_rect.height / source_rect.height;
    Some(ImageDrawPlan {
        clip_rect: dest_rect,
        local_affine: vello::kurbo::Affine::new([
            source_to_dest_x * scale_factor,
            0.0,
            0.0,
            source_to_dest_y * scale_factor,
            (dest_rect.x - source_rect.x * source_to_dest_x) * scale_factor,
            (dest_rect.y - source_rect.y * source_to_dest_y) * scale_factor,
        ]),
    })
}

fn path_to_vello(path: &Path, scale_factor: f64) -> vello::kurbo::BezPath {
    let mut bezier = vello::kurbo::BezPath::new();
    for_each_normalized(
        path.operations(),
        scale_factor,
        |operation| match operation {
            NormalizedPathOp::MoveTo(point) => {
                bezier.move_to((point.x(), point.y()));
            }
            NormalizedPathOp::LineTo(point) => {
                bezier.line_to((point.x(), point.y()));
            }
            NormalizedPathOp::CubicTo(segment) => {
                bezier.curve_to(
                    (segment.control1.x(), segment.control1.y()),
                    (segment.control2.x(), segment.control2.y()),
                    (segment.end.x(), segment.end.y()),
                );
            }
            NormalizedPathOp::QuadTo { control, end } => {
                bezier.quad_to((control.x(), control.y()), (end.x(), end.y()));
            }
            NormalizedPathOp::Close => bezier.close_path(),
        },
    );
    bezier
}

fn vello_fill(rule: FillRule) -> vello::peniko::Fill {
    match rule {
        FillRule::NonZero => vello::peniko::Fill::NonZero,
        FillRule::EvenOdd => vello::peniko::Fill::EvenOdd,
    }
}

fn append_image_draw(
    scene: &mut vello::Scene,
    image: &vello::peniko::ImageBrush,
    image_affine: vello::kurbo::Affine,
    clip_affine: vello::kurbo::Affine,
    clip_rect: Rectangle,
    scale_factor: f64,
) {
    let clip = vello::kurbo::Rect::new(
        clip_rect.x * scale_factor,
        clip_rect.y * scale_factor,
        (clip_rect.x + clip_rect.width) * scale_factor,
        (clip_rect.y + clip_rect.height) * scale_factor,
    );
    scene.push_clip_layer(vello::peniko::Fill::NonZero, clip_affine, &clip);
    scene.draw_image(image, image_affine);
    scene.pop_layer();
}

fn vello_stroke(style: &StrokeStyle, scale: f64) -> Stroke {
    let stroke = Stroke::new(style.width() * scale)
        .with_miter_limit(style.miter_limit())
        .with_caps(match style.cap() {
            LineCap::Butt => Cap::Butt,
            LineCap::Round => Cap::Round,
            LineCap::Square => Cap::Square,
        })
        .with_join(match style.join() {
            LineJoin::Miter => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        });

    stroke.with_dashes(
        style.normalized_dash_offset() * scale,
        style.dash_lengths().iter().map(|length| length * scale),
    )
}

fn damage_rect_to_copy_region(
    rect: Rectangle,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> Option<(u32, u32, u32, u32)> {
    let x0 = (rect.x * scale_factor).floor().max(0.0) as u32;
    let y0 = (rect.y * scale_factor).floor().max(0.0) as u32;
    let x1 = ((rect.x + rect.width) * scale_factor).ceil().max(0.0) as u32;
    let y1 = ((rect.y + rect.height) * scale_factor).ceil().max(0.0) as u32;
    let x0 = x0.min(width);
    let y0 = y0.min(height);
    let x1 = x1.min(width);
    let y1 = y1.min(height);
    let copy_width = x1.saturating_sub(x0);
    let copy_height = y1.saturating_sub(y0);

    (copy_width > 0 && copy_height > 0).then_some((x0, y0, copy_width, copy_height))
}

fn damage_rect_to_aligned_clip(
    rect: Rectangle,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> Option<Rectangle> {
    let (x, y, copy_width, copy_height) =
        damage_rect_to_copy_region(rect, width, height, scale_factor)?;
    Some(Rectangle::new(
        x as f64 / scale_factor,
        y as f64 / scale_factor,
        copy_width as f64 / scale_factor,
        copy_height as f64 / scale_factor,
    ))
}

#[cfg(all(target_os = "macos", feature = "native"))]
fn configure_macos_presentation_layer(surface: &vello::wgpu::Surface<'_>) {
    let Some(surface) = (unsafe { surface.as_hal::<vello::wgpu::hal::api::Metal>() }) else {
        return;
    };
    let layer = surface.render_layer().lock();
    layer.setContentsGravity(unsafe { objc2_quartz_core::kCAGravityTopLeft });
    layer.setOpaque(true);
    let background = objc2_core_graphics::CGColor::new_srgb(
        DEFAULT_BACKGROUND_COMPONENT,
        DEFAULT_BACKGROUND_COMPONENT,
        DEFAULT_BACKGROUND_COMPONENT,
        1.0,
    );
    layer.setBackgroundColor(Some(&background));
}

#[cfg(all(target_os = "macos", feature = "native"))]
fn set_macos_transactional_present(surface: &vello::wgpu::Surface<'_>, enabled: bool) {
    let Some(surface) = (unsafe { surface.as_hal::<vello::wgpu::hal::api::Metal>() }) else {
        return;
    };
    surface
        .render_layer()
        .lock()
        .setPresentsWithTransaction(enabled);
}

fn scratch_base_rgba() -> [f32; 4] {
    [
        DEFAULT_BACKGROUND_COMPONENT as f32,
        DEFAULT_BACKGROUND_COMPONENT as f32,
        DEFAULT_BACKGROUND_COMPONENT as f32,
        1.0,
    ]
}

fn vello_paint(paint: &Paint, scale_factor: f64) -> vello::peniko::Brush {
    use vello::peniko::color::ColorSpaceTag;
    use vello::peniko::{ColorStop, Extend, Gradient, InterpolationAlphaSpace};
    match paint {
        Paint::Solid(color) => VelloColor::new([
            color.red() as f32,
            color.green() as f32,
            color.blue() as f32,
            color.alpha() as f32,
        ])
        .into(),
        Paint::LinearGradient(gradient) => {
            let stops: Vec<_> = gradient
                .stops()
                .iter()
                .map(|stop| {
                    let color = stop.color();
                    ColorStop {
                        offset: stop.offset() as f32,
                        color: VelloColor::new([
                            color.red() as f32,
                            color.green() as f32,
                            color.blue() as f32,
                            color.alpha() as f32,
                        ])
                        .into(),
                    }
                })
                .collect();
            Gradient::new_linear(
                (
                    gradient.start().x() * scale_factor,
                    gradient.start().y() * scale_factor,
                ),
                (
                    gradient.end().x() * scale_factor,
                    gradient.end().y() * scale_factor,
                ),
            )
            .with_extend(Extend::Pad)
            .with_interpolation_cs(ColorSpaceTag::Srgb)
            .with_interpolation_alpha_space(InterpolationAlphaSpace::Premultiplied)
            .with_stops(stops.as_slice())
            .into()
        }
    }
}

fn append_glyph_run(
    scene: &mut vello::Scene,
    run: &GlyphRun,
    font: &vello::peniko::FontData,
    origin: Point,
    paint: &GlyphPaint,
    transform: &Affine2D,
    scale_factor: f64,
) {
    if run.glyphs.is_empty() {
        return;
    }
    let affine = VelloRenderer::transform_to_affine(transform, scale_factor);
    let brush = vello_paint(paint.paint(), scale_factor);
    let glyph_transform = run
        .skew_degrees
        .map(|degrees| vello::kurbo::Affine::skew((degrees.to_radians().tan()) as f64, 0.0));
    let glyphs = || {
        run.glyphs.iter().map(|glyph| vello::Glyph {
            id: glyph.id,
            x: ((origin.x() as f32) + glyph.x) * scale_factor as f32,
            y: ((origin.y() as f32) + glyph.y) * scale_factor as f32,
        })
    };
    if let GlyphPaint::Stroke { stroke, .. } = paint
        && stroke.width() > 0.0
        && !stroke.dash_lengths().is_empty()
    {
        let style = vello_stroke(stroke, scale_factor);
        for glyph in glyphs() {
            if let Some(path) = glyph_outline::outline(font, run, glyph.id, scale_factor) {
                let local = vello::kurbo::Affine::new([
                    1.0,
                    0.0,
                    0.0,
                    -1.0,
                    f64::from(glyph.x),
                    f64::from(glyph.y),
                ]) * glyph_transform.unwrap_or(vello::kurbo::Affine::IDENTITY);
                scene.stroke(&style, affine * local, &brush, Some(local.inverse()), &path);
            } else {
                // Keep Vello's color/bitmap glyph behavior when no vector outline exists.
                scene
                    .draw_glyphs(font)
                    .brush(&brush)
                    .hint(false)
                    .transform(affine)
                    .glyph_transform(glyph_transform)
                    .font_size(run.font_size * scale_factor as f32)
                    .normalized_coords(&run.normalized_coords)
                    .draw(&style, std::iter::once(glyph));
            }
        }
        return;
    }
    let builder = scene
        .draw_glyphs(font)
        .brush(&brush)
        .hint(false)
        .transform(affine)
        .glyph_transform(glyph_transform)
        .font_size(run.font_size * scale_factor as f32)
        .normalized_coords(&run.normalized_coords);
    match paint {
        GlyphPaint::Fill(_) => builder.draw(vello::peniko::Fill::NonZero, glyphs()),
        GlyphPaint::Stroke { stroke, .. } => {
            if stroke.width() > 0.0 {
                builder.draw(&vello_stroke(stroke, scale_factor), glyphs());
            }
        }
    }
}

/// 渲染状态
#[derive(Clone, Debug, Default)]
struct RenderState {
    /// 当前变换矩阵
    transform: Affine2D,
    /// 当前状态下可重放的裁剪层。
    clips: Vec<RenderClip>,
}

#[derive(Clone, Debug, PartialEq)]
struct RenderClip {
    transform: Affine2D,
    geometry: ClipGeometry,
}

#[derive(Clone, Debug, PartialEq)]
enum ClipGeometry {
    Rectangle(Rectangle),
    Path(ClipPath),
}

fn clip_restore_plan<'a>(
    current: &RenderState,
    saved_clips: &'a [RenderClip],
) -> (usize, &'a [RenderClip]) {
    let common_prefix = current
        .clips
        .iter()
        .zip(saved_clips)
        .take_while(|(current_clip, saved_clip)| current_clip == saved_clip)
        .count();
    (
        current.clips.len() - common_prefix,
        &saved_clips[common_prefix..],
    )
}

pub struct VelloRenderer {
    render_context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    scene: vello::Scene,
    surface: RenderSurface<'static>,
    scale_factor: f64,
    surface_suspended: bool,
    pending_resize: Option<(u32, u32, f64)>,
    /// 状态栈
    state_stack: Vec<RenderState>,
    session_gate: BackendSessionGate,
    font_faces: HashMap<(novadraw::render::ResourceId, u64), vello::peniko::Blob<u8>>,
    images: HashMap<(novadraw::render::ResourceId, u64), vello::peniko::ImageData>,
    /// 保留上一帧完整结果的纹理（也作为截图源）
    retained_texture: Option<(vello::wgpu::Texture, vello::wgpu::TextureView, u32, u32)>,
    /// 本帧临时渲染纹理
    scratch_texture: Option<(vello::wgpu::Texture, vello::wgpu::TextureView, u32, u32)>,
}

impl VelloRenderer {
    #[cfg(any(feature = "native", target_arch = "wasm32"))]
    async fn new_for_surface(
        target: vello::wgpu::SurfaceTarget<'static>,
        pixel_width: u32,
        pixel_height: u32,
        scale_factor: f64,
    ) -> Result<Self, vello::Error> {
        let mut render_context = RenderContext::new();
        let surface = render_context
            .create_surface(
                target,
                pixel_width,
                pixel_height,
                vello::wgpu::PresentMode::AutoVsync,
            )
            .await?;
        #[cfg(all(target_os = "macos", feature = "native"))]
        configure_macos_presentation_layer(&surface.surface);

        let mut renderers = vec![];
        renderers.resize_with(render_context.devices.len(), || None);
        renderers[surface.dev_id].get_or_insert_with(|| create_renderer(&render_context, &surface));

        Ok(Self {
            render_context,
            renderers,
            scene: vello::Scene::new(),
            surface,
            scale_factor,
            surface_suspended: false,
            pending_resize: None,
            state_stack: vec![RenderState::default()],
            session_gate: BackendSessionGate::default(),
            font_faces: HashMap::new(),
            images: HashMap::new(),
            retained_texture: None,
            scratch_texture: None,
        })
    }

    fn current_surface_size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }

    fn sync_submission_resources(
        &mut self,
        submission: &novadraw::render::RenderSubmission,
    ) -> bool {
        match self
            .session_gate
            .accept(submission.session_id, &submission.resources)
        {
            BackendSessionDecision::RejectStale | BackendSessionDecision::RejectMissingSnapshot => {
                return false;
            }
            BackendSessionDecision::Continue => {}
            BackendSessionDecision::Initialize | BackendSessionDecision::Replace => {
                self.font_faces.clear();
                self.images.clear();
                self.retained_texture = None;
                self.scratch_texture = None;
            }
        }
        sync_font_face_cache(&mut self.font_faces, &submission.resources);
        sync_image_cache(&mut self.images, &submission.resources);
        true
    }

    fn has_required_resources(&self, commands: &[RenderCommand]) -> bool {
        commands.iter().all(|command| match &command.kind {
            novadraw::render::RenderCommandKind::DrawGlyphRun { run, .. } => self
                .font_faces
                .contains_key(&(run.font.resource_id(), run.font.revision())),
            novadraw::render::RenderCommandKind::Image { image, .. } => self
                .images
                .contains_key(&(image.resource_id(), image.revision())),
            _ => true,
        })
    }

    fn apply_pending_resize(&mut self) {
        let Some((pixel_width, pixel_height, scale_factor)) = self.pending_resize.take() else {
            return;
        };
        if surface_is_suspended(pixel_width, pixel_height) {
            return;
        }

        self.scale_factor = scale_factor;
        self.render_context
            .resize_surface(&mut self.surface, pixel_width, pixel_height);
    }

    fn queue_resize(
        &mut self,
        pixel_width: u32,
        pixel_height: u32,
        scale_factor: f64,
        force: bool,
    ) {
        let requested = (pixel_width, pixel_height, scale_factor);
        let current = (
            self.surface.config.width,
            self.surface.config.height,
            self.scale_factor,
        );
        if !force
            && (self.pending_resize == Some(requested)
                || (!self.surface_suspended
                    && self.pending_resize.is_none()
                    && current == requested))
        {
            return;
        }
        self.retained_texture = None;
        self.scratch_texture = None;
        self.surface_suspended = surface_is_suspended(pixel_width, pixel_height);
        self.pending_resize = Some(requested);
    }

    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    pub fn new<W: NativeWindow>(window: Arc<W>, surface: novadraw::render::SurfaceInfo) -> Self {
        pollster::block_on(Self::new_for_surface(
            window.into(),
            surface.pixel_width,
            surface.pixel_height,
            surface.scale_factor,
        ))
        .expect("Failed to create surface")
    }

    /// Returns stable, backend-owned adapter metadata without exposing wgpu types.
    #[cfg(any(
        all(feature = "native", not(target_arch = "wasm32")),
        all(feature = "web", target_arch = "wasm32")
    ))]
    pub fn adapter_info(&self) -> VelloAdapterInfo {
        let info = self.render_context.devices[self.surface.dev_id]
            .adapter()
            .get_info();
        VelloAdapterInfo {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            device_type: format!("{:?}", info.device_type),
            backend: format!("{:?}", info.backend),
            driver: info.driver,
            driver_info: info.driver_info,
        }
    }

    /// Invokes `callback` after all queue work submitted before this call completes.
    ///
    /// This is a diagnostic synchronization point. Queue completion does not
    /// prove compositor presentation or physical display scanout.
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    pub fn on_submitted_work_done(&self, callback: impl FnOnce() + Send + 'static) {
        self.render_context.devices[self.surface.dev_id]
            .queue
            .on_submitted_work_done(callback);
    }

    /// Waits until all queue work submitted before this call has completed.
    ///
    /// This is a diagnostic synchronization point. The elapsed wait is not a
    /// native GPU timestamp and does not prove compositor presentation.
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    pub fn wait_for_gpu_idle(&self, timeout: std::time::Duration) -> Result<(), VelloGpuWaitError> {
        let device = &self.render_context.devices[self.surface.dev_id].device;
        device
            .poll(vello::wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(timeout),
            })
            .map(|_| ())
            .map_err(|error| VelloGpuWaitError {
                message: error.to_string(),
            })
    }

    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    pub async fn new_web(
        canvas: web_sys::HtmlCanvasElement,
        surface: novadraw::render::SurfaceInfo,
    ) -> Result<Self, VelloInitializationError> {
        Self::new_for_surface(
            vello::wgpu::SurfaceTarget::Canvas(canvas),
            surface.pixel_width,
            surface.pixel_height,
            surface.scale_factor,
        )
        .await
        .map_err(VelloInitializationError::from_vello)
    }

    fn create_offscreen_texture(
        &self,
        label: &'static str,
    ) -> (vello::wgpu::Texture, vello::wgpu::TextureView, u32, u32) {
        let width = self.surface.config.width;
        let height = self.surface.config.height;
        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let device = &device_handle.device;

        let texture = device.create_texture(&vello::wgpu::TextureDescriptor {
            label: Some(label),
            size: vello::wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: vello::wgpu::TextureDimension::D2,
            format: vello::wgpu::TextureFormat::Rgba8Unorm,
            usage: vello::wgpu::TextureUsages::COPY_SRC
                | vello::wgpu::TextureUsages::COPY_DST
                | vello::wgpu::TextureUsages::RENDER_ATTACHMENT
                | vello::wgpu::TextureUsages::TEXTURE_BINDING
                | vello::wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[vello::wgpu::TextureFormat::Rgba8Unorm],
        });

        let view = texture.create_view(&vello::wgpu::TextureViewDescriptor::default());
        (texture, view, width, height)
    }

    fn recover_surface(
        &mut self,
        recovery: SurfaceRecovery,
        surface: novadraw::render::SurfaceInfo,
    ) -> RenderOutcome {
        match recovery {
            SurfaceRecovery::Reconfigure => {
                self.queue_resize(
                    surface.pixel_width,
                    surface.pixel_height,
                    surface.scale_factor,
                    true,
                );
                if self.surface_suspended {
                    RenderOutcome::Skipped
                } else {
                    RenderOutcome::Retry
                }
            }
            SurfaceRecovery::Retry => RenderOutcome::Retry,
            SurfaceRecovery::Skip => RenderOutcome::Skipped,
        }
    }

    fn clear_texture_to_background(&self, view: &vello::wgpu::TextureView) {
        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                    label: Some("Clear Retained Texture"),
                });
        {
            let _pass = encoder.begin_render_pass(&vello::wgpu::RenderPassDescriptor {
                label: Some("Clear Retained Texture Pass"),
                color_attachments: &[Some(vello::wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: vello::wgpu::Operations {
                        load: vello::wgpu::LoadOp::Clear(DEFAULT_BACKGROUND_COLOR),
                        store: vello::wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
        }
        device_handle.queue.submit([encoder.finish()]);
    }

    /// 确保保留纹理存在
    fn ensure_retained_texture(&mut self) {
        let (width, height) = self.current_surface_size();

        #[allow(clippy::collapsible_if)]
        if let Some((_, _, old_width, old_height)) = &self.retained_texture {
            if *old_width == width && *old_height == height {
                return;
            }
        }

        let texture = self.create_offscreen_texture("Retained Texture");
        self.clear_texture_to_background(&texture.1);
        self.retained_texture = Some(texture);
    }

    /// 确保临时渲染纹理存在
    fn ensure_scratch_texture(&mut self) {
        let (width, height) = self.current_surface_size();

        #[allow(clippy::collapsible_if)]
        if let Some((_, _, old_width, old_height)) = &self.scratch_texture {
            if *old_width == width && *old_height == height {
                return;
            }
        }

        self.scratch_texture = Some(self.create_offscreen_texture("Scratch Texture"));
    }

    fn damage_rect_to_copy_region(
        &self,
        rect: Rectangle,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        damage_rect_to_copy_region(rect, width, height, self.scale_factor)
    }

    fn effective_damage_regions(
        &self,
        submission: &novadraw::render::RenderSubmission,
    ) -> Option<(Rectangle, Vec<Rectangle>)> {
        let (width, height) = self.current_surface_size();
        match submission.damage.mode() {
            DamageMode::None => None,
            DamageMode::Full => {
                let full = Rectangle::new(
                    0.0,
                    0.0,
                    width as f64 / self.scale_factor,
                    height as f64 / self.scale_factor,
                );
                Some((full, vec![full]))
            }
            DamageMode::Partial => {
                let union = submission.damage.union()?;
                let regions = if submission.damage.regions().is_empty() {
                    vec![union]
                } else {
                    submission.damage.regions().to_vec()
                };
                Some((union, regions))
            }
        }
    }

    /// 获取当前状态
    fn current_state(&self) -> &RenderState {
        self.state_stack.last().unwrap()
    }

    /// 获取可变当前状态
    fn current_state_mut(&mut self) -> &mut RenderState {
        self.state_stack.last_mut().unwrap()
    }

    fn pop_clip_layers(&mut self, count: usize) {
        for _ in 0..count {
            self.scene.pop_layer();
        }
    }

    fn restore_clip_layers(&mut self, clips: &[RenderClip]) {
        let (current_depth, clips_to_replay) = clip_restore_plan(self.current_state(), clips);
        self.pop_clip_layers(current_depth);
        for clip in clips_to_replay {
            self.push_clip_layer(clip);
        }
    }

    /// 处理单个渲染命令
    fn render_command(&mut self, cmd: &RenderCommand) {
        match &cmd.kind {
            // ===== 状态管理命令 =====
            novadraw::render::command::RenderCommandKind::PushState => {
                debug!("PushState, stack depth: {}", self.state_stack.len());
                self.state_stack.push(self.current_state().clone());
            }

            novadraw::render::command::RenderCommandKind::RestoreState => {
                debug!("RestoreState, stack depth: {}", self.state_stack.len());
                if self.state_stack.len() >= 2 {
                    let saved = self.state_stack[self.state_stack.len() - 2].clone();
                    self.restore_clip_layers(&saved.clips);
                    *self.current_state_mut() = saved;
                }
            }

            novadraw::render::command::RenderCommandKind::PopState => {
                debug!("PopState, stack depth: {}", self.state_stack.len());
                if self.state_stack.len() > 1 {
                    let saved = self.state_stack[self.state_stack.len() - 2].clone();
                    self.restore_clip_layers(&saved.clips);
                    self.state_stack.pop();
                }
            }

            novadraw::render::command::RenderCommandKind::ConcatTransform { matrix } => {
                debug!("ConcatTransform: {:?}", matrix);
                // 叠加变换
                let new_transform = self.current_state().transform.post_concat(*matrix);

                self.current_state_mut().transform = new_transform;
            }

            novadraw::render::command::RenderCommandKind::SetTransform { matrix } => {
                debug!("SetTransform: {:?}", matrix);
                self.current_state_mut().transform = *matrix;
            }

            novadraw::render::command::RenderCommandKind::ResetTransform => {
                debug!("ResetTransform");
                self.current_state_mut().transform = Affine2D::IDENTITY;
            }

            // NdCanvas bakes global alpha into every paint command. Retaining
            // this command in the IR preserves state-transition observability
            // without applying alpha a second time in the backend.
            novadraw::render::command::RenderCommandKind::SetGlobalAlpha { .. } => {}

            novadraw::render::command::RenderCommandKind::Clip { rect } => {
                debug!("Clip: {:?}", rect);
                let clip = RenderClip {
                    transform: self.current_state().transform,
                    geometry: ClipGeometry::Rectangle(*rect),
                };
                self.push_clip_layer(&clip);
                self.current_state_mut().clips.push(clip);
            }

            novadraw::render::command::RenderCommandKind::ClipPath { clip } => {
                let clip = RenderClip {
                    transform: self.current_state().transform,
                    geometry: ClipGeometry::Path(clip.clone()),
                };
                self.push_clip_layer(&clip);
                self.current_state_mut().clips.push(clip);
            }

            novadraw::render::command::RenderCommandKind::ResetClip => {
                debug!("ResetClip");
                let depth = self.current_state().clips.len();
                self.pop_clip_layers(depth);
                self.current_state_mut().clips.clear();
            }

            // ===== 绘制命令 =====
            novadraw::render::command::RenderCommandKind::ClearRect { rect, color } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect.x * self.scale_factor;
                let y0 = rect.y * self.scale_factor;
                let x1 = (rect.x + rect.width) * self.scale_factor;
                let y1 = (rect.y + rect.height) * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let vello_color = VelloColor::new([
                    color.red() as f32,
                    color.green() as f32,
                    color.blue() as f32,
                    color.alpha() as f32,
                ]);
                self.scene.fill(
                    vello::peniko::Fill::NonZero,
                    affine,
                    vello_color,
                    None,
                    &kurbo_rect,
                );
            }

            novadraw::render::command::RenderCommandKind::FillRect { rect, paint, rule } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect.x * self.scale_factor;
                let y0 = rect.y * self.scale_factor;
                let x1 = (rect.x + rect.width) * self.scale_factor;
                let y1 = (rect.y + rect.height) * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let brush = vello_paint(paint, self.scale_factor);
                self.scene
                    .fill(vello_fill(*rule), affine, &brush, None, &kurbo_rect);
            }

            novadraw::render::command::RenderCommandKind::StrokeRect {
                rect,
                paint,
                stroke,
            } => {
                if stroke.width() == 0.0 {
                    return;
                }
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect.x * self.scale_factor;
                let y0 = rect.y * self.scale_factor;
                let x1 = (rect.x + rect.width) * self.scale_factor;
                let y1 = (rect.y + rect.height) * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let brush = vello_paint(paint, self.scale_factor);
                let stroke = vello_stroke(stroke, self.scale_factor);
                self.scene
                    .stroke(&stroke, affine, &brush, None, &kurbo_rect);
            }

            novadraw::render::command::RenderCommandKind::Line {
                p1,
                p2,
                paint,
                stroke,
            } => {
                if stroke.width() == 0.0 {
                    return;
                }
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let v1 = vello::kurbo::Point::new(
                    p1.x() * self.scale_factor,
                    p1.y() * self.scale_factor,
                );
                let v2 = vello::kurbo::Point::new(
                    p2.x() * self.scale_factor,
                    p2.y() * self.scale_factor,
                );
                let brush = vello_paint(paint, self.scale_factor);

                let stroke = vello_stroke(stroke, self.scale_factor);

                self.scene.stroke(
                    &stroke,
                    affine,
                    &brush,
                    None,
                    &vello::kurbo::Line::new(v1, v2),
                );
            }

            novadraw::render::command::RenderCommandKind::Polyline {
                points,
                paint,
                stroke,
            } => {
                if points.len() < 2 || stroke.width() == 0.0 {
                    return;
                }
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let brush = vello_paint(paint, self.scale_factor);

                let stroke = vello_stroke(stroke, self.scale_factor);

                // 构建折线路径
                let mut path = vello::kurbo::BezPath::new();
                let first_point = points.get(0).expect("validated polyline length");
                path.move_to((
                    first_point.x() * self.scale_factor,
                    first_point.y() * self.scale_factor,
                ));
                for point in points.iter().skip(1) {
                    path.line_to((point.x() * self.scale_factor, point.y() * self.scale_factor));
                }

                self.scene.stroke(&stroke, affine, &brush, None, &path);
            }

            novadraw::render::command::RenderCommandKind::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill_paint,
                stroke_paint,
                stroke,
                rule,
            } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let center =
                    vello::kurbo::Point::new(cx * self.scale_factor, cy * self.scale_factor);
                let radii =
                    vello::kurbo::Vec2::new(*rx * self.scale_factor, *ry * self.scale_factor);
                let ellipse = vello::kurbo::Ellipse::new(center, radii, 0.0);

                // 填充椭圆
                if let Some(paint) = fill_paint {
                    let brush = vello_paint(paint, self.scale_factor);
                    self.scene
                        .fill(vello_fill(*rule), affine, &brush, None, &ellipse);
                }

                // 描边椭圆
                if stroke.width() == 0.0 {
                    return;
                }
                if let Some(paint) = stroke_paint {
                    let brush = vello_paint(paint, self.scale_factor);
                    let stroke = vello_stroke(stroke, self.scale_factor);
                    self.scene.stroke(&stroke, affine, &brush, None, &ellipse);
                }
            }

            novadraw::render::command::RenderCommandKind::FillPath { path, paint, rule } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let brush = vello_paint(paint, self.scale_factor);

                let bez_path = path_to_vello(path, self.scale_factor);

                self.scene
                    .fill(vello_fill(*rule), affine, &brush, None, &bez_path);
            }

            novadraw::render::command::RenderCommandKind::StrokePath {
                path,
                paint,
                stroke,
            } => {
                if stroke.width() == 0.0 {
                    return;
                }
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let brush = vello_paint(paint, self.scale_factor);

                let stroke = vello_stroke(stroke, self.scale_factor);

                let bez_path = path_to_vello(path, self.scale_factor);

                self.scene.stroke(&stroke, affine, &brush, None, &bez_path);
            }

            novadraw::render::command::RenderCommandKind::DrawGlyphRun { run, origin, paint } => {
                let Some(font_data) = self
                    .font_faces
                    .get(&(run.font.resource_id(), run.font.revision()))
                    .cloned()
                else {
                    return;
                };
                let font = vello::peniko::FontData::new(font_data, run.font.collection_index());
                let transform = self.current_state().transform;
                let scale_factor = self.scale_factor;
                append_glyph_run(
                    &mut self.scene,
                    run,
                    &font,
                    *origin,
                    paint,
                    &transform,
                    scale_factor,
                );
            }

            novadraw::render::command::RenderCommandKind::Image {
                image,
                source_rect,
                dest_rect,
                alpha,
            } => {
                if image.width() == 0 || image.height() == 0 || *alpha <= 0.0 {
                    return;
                }
                let Some(image_data) = self
                    .images
                    .get(&(image.resource_id(), image.revision()))
                    .cloned()
                else {
                    return;
                };
                let Some(plan) = image_draw_plan(
                    image_data.width,
                    image_data.height,
                    *source_rect,
                    *dest_rect,
                    self.scale_factor,
                ) else {
                    return;
                };
                let image = vello::peniko::ImageBrush {
                    image: image_data,
                    sampler: vello::peniko::ImageSampler::default().with_alpha(*alpha as f32),
                };
                let scale_factor = self.scale_factor;
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, scale_factor)
                        * plan.local_affine;
                let clip_affine =
                    Self::transform_to_affine(&self.current_state().transform, scale_factor);
                append_image_draw(
                    &mut self.scene,
                    &image,
                    affine,
                    clip_affine,
                    plan.clip_rect,
                    scale_factor,
                );
            }
        }
    }

    /// 将 Affine2D 转换为 vello Affine
    fn transform_to_affine(transform: &Affine2D, scale_factor: f64) -> vello::kurbo::Affine {
        let coeffs = transform.coeffs();
        // coeffs = [a, b, c, d, e, f]
        // Apply scale factor to translation components (e, f)
        vello::kurbo::Affine::new([
            coeffs[0],
            coeffs[1],
            coeffs[2],
            coeffs[3],
            coeffs[4] * scale_factor,
            coeffs[5] * scale_factor,
        ])
    }

    /// 推送裁剪层到场景
    fn push_clip_layer(&mut self, clip: &RenderClip) {
        let affine = Self::transform_to_affine(&clip.transform, self.scale_factor);
        append_clip_layer(&mut self.scene, &clip.geometry, affine, self.scale_factor);
    }
}

fn append_clip_layer(
    scene: &mut vello::Scene,
    geometry: &ClipGeometry,
    affine: vello::kurbo::Affine,
    scale: f64,
) {
    match geometry {
        ClipGeometry::Rectangle(rect) => {
            let rect = vello::kurbo::Rect::new(
                rect.x * scale,
                rect.y * scale,
                (rect.x + rect.width) * scale,
                (rect.y + rect.height) * scale,
            );
            scene.push_clip_layer(vello::peniko::Fill::NonZero, affine, &rect);
        }
        ClipGeometry::Path(clip) => {
            let path = path_to_vello(clip.path(), scale);
            scene.push_clip_layer(vello_fill(clip.rule()), affine, &path);
        }
    }
}

impl RenderBackend for VelloRenderer {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::RETAINED_PARTIAL
            .with_glyph_runs()
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
    }

    fn submit(&mut self, submission: &novadraw::render::RenderSubmission) -> RenderOutcome {
        if let Err(error) = self
            .capabilities()
            .validate_capabilities(&submission.commands)
        {
            return RenderOutcome::Unsupported(error);
        }
        if let Err(error) = novadraw::render::validate_graphics_input(
            &submission.commands,
            submission.surface.scale_factor,
        ) {
            return RenderOutcome::InvalidGraphicsInput(error);
        }
        if !self.sync_submission_resources(submission) {
            return RenderOutcome::Skipped;
        }
        if !self.has_required_resources(&submission.commands) {
            return RenderOutcome::Retry;
        }
        self.resize(
            submission.surface.pixel_width,
            submission.surface.pixel_height,
            submission.surface.scale_factor,
        );
        #[cfg(all(target_os = "macos", feature = "native"))]
        let is_resize_frame = self.pending_resize.is_some();
        self.apply_pending_resize();
        if self.surface_suspended {
            return RenderOutcome::Skipped;
        }
        let commands = &submission.commands;
        let damage = &submission.damage;
        debug!(
            "damage set: union={:?}, regions={}",
            damage.union(),
            damage.regions().len()
        );

        let Some((effective_damage, effective_regions)) = self.effective_damage_regions(submission)
        else {
            return RenderOutcome::Skipped;
        };
        let (width, height) = self.current_surface_size();
        let Some(clip_rect) =
            damage_rect_to_aligned_clip(effective_damage, width, height, self.scale_factor)
        else {
            return RenderOutcome::Skipped;
        };

        // self.scene.reset();
        self.scene = vello::Scene::new();

        // 重置状态栈
        self.state_stack.clear();
        self.state_stack.push(RenderState::default());

        self.push_clip_layer(&RenderClip {
            transform: Affine2D::IDENTITY,
            geometry: ClipGeometry::Rectangle(clip_rect),
        });
        for cmd in commands {
            self.render_command(cmd);
        }
        self.scene.pop_layer();

        debug!("渲染命令执行完成");

        self.ensure_retained_texture();
        self.ensure_scratch_texture();

        #[cfg(all(target_os = "macos", feature = "native"))]
        if is_resize_frame {
            set_macos_transactional_present(&self.surface.surface, true);
        }
        let surface_status = self.surface.surface.get_current_texture();
        let (surface_texture, reconfigure_after_present) = match surface_status {
            vello::wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            vello::wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            status => {
                #[cfg(all(target_os = "macos", feature = "native"))]
                set_macos_transactional_present(&self.surface.surface, false);
                let recovery =
                    surface_recovery(&status).expect("unavailable surface must define recovery");
                return self.recover_surface(recovery, submission.surface);
            }
        };
        let scratch_view = {
            let (_, view, _, _) = self
                .scratch_texture
                .as_ref()
                .expect("Scratch texture not created");
            view.clone()
        };
        let retained_view = {
            let (_, view, _, _) = self
                .retained_texture
                .as_ref()
                .expect("Retained texture not created");
            view.clone()
        };

        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let base_color = VelloColor::new(scratch_base_rgba());

        // 使用 surface 背景重建临时纹理；后续只把 damage regions 复制到 retained texture。
        self.renderers[self.surface.dev_id]
            .as_mut()
            .unwrap()
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &scratch_view,
                &vello::RenderParams {
                    base_color,
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .expect("Failed to render to scratch texture");

        let scratch_texture = &self
            .scratch_texture
            .as_ref()
            .expect("Scratch texture not created")
            .0;
        let retained_texture = &self
            .retained_texture
            .as_ref()
            .expect("Retained texture not created")
            .0;

        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                    label: Some("Surface Blit"),
                });
        for rect in effective_regions {
            let Some((origin_x, origin_y, copy_width, copy_height)) =
                self.damage_rect_to_copy_region(rect, width, height)
            else {
                continue;
            };
            encoder.copy_texture_to_texture(
                vello::wgpu::TexelCopyTextureInfo {
                    texture: scratch_texture,
                    mip_level: 0,
                    origin: vello::wgpu::Origin3d {
                        x: origin_x,
                        y: origin_y,
                        z: 0,
                    },
                    aspect: vello::wgpu::TextureAspect::All,
                },
                vello::wgpu::TexelCopyTextureInfo {
                    texture: retained_texture,
                    mip_level: 0,
                    origin: vello::wgpu::Origin3d {
                        x: origin_x,
                        y: origin_y,
                        z: 0,
                    },
                    aspect: vello::wgpu::TextureAspect::All,
                },
                vello::wgpu::Extent3d {
                    width: copy_width,
                    height: copy_height,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &retained_view,
            &surface_texture
                .texture
                .create_view(&vello::wgpu::TextureViewDescriptor::default()),
        );

        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
        #[cfg(all(target_os = "macos", feature = "native"))]
        if is_resize_frame {
            set_macos_transactional_present(&self.surface.surface, false);
        }
        if reconfigure_after_present {
            self.render_context.configure_surface(&self.surface);
        }
        RenderOutcome::Presented
    }

    fn resize(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f64) {
        self.queue_resize(pixel_width, pixel_height, scale_factor, false);
    }
}

impl VelloRenderer {
    /// Records a complete submission into the retained texture without acquiring
    /// a window surface drawable.
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    pub fn render_for_screenshot(
        &mut self,
        submission: &novadraw::render::RenderSubmission,
    ) -> RenderOutcome {
        if let Err(error) = self
            .capabilities()
            .validate_capabilities(&submission.commands)
        {
            return RenderOutcome::Unsupported(error);
        }
        if let Err(error) = novadraw::render::validate_graphics_input(
            &submission.commands,
            submission.surface.scale_factor,
        ) {
            return RenderOutcome::InvalidGraphicsInput(error);
        }
        if !self.sync_submission_resources(submission) {
            return RenderOutcome::Skipped;
        }
        if !self.has_required_resources(&submission.commands) {
            return RenderOutcome::Retry;
        }
        self.resize(
            submission.surface.pixel_width,
            submission.surface.pixel_height,
            submission.surface.scale_factor,
        );
        self.apply_pending_resize();
        if self.surface_suspended {
            return RenderOutcome::Skipped;
        }

        let (width, height) = self.current_surface_size();
        let full = Rectangle::new(
            0.0,
            0.0,
            width as f64 / self.scale_factor,
            height as f64 / self.scale_factor,
        );
        self.scene = vello::Scene::new();
        self.state_stack.clear();
        self.state_stack.push(RenderState::default());
        self.push_clip_layer(&RenderClip {
            transform: Affine2D::IDENTITY,
            geometry: ClipGeometry::Rectangle(full),
        });
        for command in &submission.commands {
            self.render_command(command);
        }
        self.scene.pop_layer();

        self.ensure_retained_texture();
        let retained_view = self
            .retained_texture
            .as_ref()
            .expect("Retained texture not created")
            .1
            .clone();
        let device_handle = &self.render_context.devices[self.surface.dev_id];
        self.renderers[self.surface.dev_id]
            .as_mut()
            .expect("Vello renderer not created")
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &retained_view,
                &vello::RenderParams {
                    base_color: VelloColor::new(scratch_base_rgba()),
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .expect("Failed to render screenshot texture");
        RenderOutcome::Presented
    }

    /// 截图并保存为 PNG 文件
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    pub fn screenshot(&self, path: &std::path::Path) -> std::io::Result<()> {
        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let width = self.surface.config.width;
        let height = self.surface.config.height;

        // 从 retained_texture 获取底层纹理
        let texture = match &self.retained_texture {
            Some((tex, _, _, _)) => tex,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Retained texture not created. Call render() first.",
                ));
            }
        };

        // 创建输出缓冲区
        let row_bytes = width * std::mem::size_of::<image::Rgba<u8>>() as u32;
        let alignment = vello::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row_bytes = row_bytes.div_ceil(alignment) * alignment;
        let buffer_size = u64::from(padded_row_bytes) * u64::from(height);
        let buffer = device_handle
            .device
            .create_buffer(&vello::wgpu::BufferDescriptor {
                label: Some("Screenshot Buffer"),
                size: buffer_size,
                usage: vello::wgpu::BufferUsages::COPY_DST | vello::wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

        // 创建命令编码器
        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                    label: Some("Screenshot Encoder"),
                });

        // 从 retained_texture 复制到 buffer
        encoder.copy_texture_to_buffer(
            vello::wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: vello::wgpu::Origin3d::ZERO,
                aspect: vello::wgpu::TextureAspect::All,
            },
            vello::wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: vello::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            vello::wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // 提交命令
        device_handle.queue.submit([encoder.finish()]);

        // 映射缓冲区并读取数据
        let buffer_slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(vello::wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        // 等待映射完成
        let _ = device_handle
            .device
            .poll(vello::wgpu::PollType::wait_indefinitely());

        // 检查映射结果
        receiver.recv().unwrap().unwrap();

        // 获取像素数据
        let data = buffer_slice.get_mapped_range();
        let data: Vec<u8> = data
            .chunks_exact(padded_row_bytes as usize)
            .flat_map(|row| row[..row_bytes as usize].iter().copied())
            .collect();

        // 创建 RGBA8 图片并保存为 PNG
        let buffer = ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, data)
            .expect("Failed to create image buffer");

        // 保存为 PNG
        buffer
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(std::io::Error::other)
    }

    /// 获取窗口尺寸（像素）
    pub fn size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }
}

fn sync_font_face_cache(
    font_faces: &mut HashMap<(novadraw::render::ResourceId, u64), vello::peniko::Blob<u8>>,
    resources: &novadraw::render::ResourceSync,
) {
    if matches!(resources, novadraw::render::ResourceSync::Snapshot(_)) {
        font_faces.clear();
    }
    let mut apply = |operation: &novadraw::render::ResourceOp| match operation {
        novadraw::render::ResourceOp::Upsert(update) => {
            if let ResourcePayload::Font(font) = &update.payload {
                font_faces.retain(|(resource_id, _), _| resource_id != &update.id);
                font_faces.insert(
                    (update.id, update.revision),
                    vello::peniko::Blob::new(font.shared_bytes()),
                );
            }
        }
        novadraw::render::ResourceOp::Remove(id) => {
            font_faces.retain(|(resource_id, _), _| resource_id != id);
        }
    };
    match resources {
        novadraw::render::ResourceSync::Delta(delta) => delta.ops.iter().for_each(&mut apply),
        novadraw::render::ResourceSync::Snapshot(snapshot) => snapshot
            .ready
            .iter()
            .cloned()
            .map(novadraw::render::ResourceOp::Upsert)
            .for_each(|operation| apply(&operation)),
    }
}

fn sync_image_cache(
    images: &mut HashMap<(novadraw::render::ResourceId, u64), vello::peniko::ImageData>,
    resources: &novadraw::render::ResourceSync,
) {
    if matches!(resources, novadraw::render::ResourceSync::Snapshot(_)) {
        images.clear();
    }
    let mut apply = |operation: &novadraw::render::ResourceOp| match operation {
        novadraw::render::ResourceOp::Upsert(update) => {
            if let ResourcePayload::Image(image) = &update.payload {
                images.retain(|(resource_id, _), _| resource_id != &update.id);
                images.insert(
                    (update.id, update.revision),
                    vello::peniko::ImageData {
                        data: image.pixels.clone().into(),
                        format: vello::peniko::ImageFormat::Rgba8,
                        alpha_type: vello::peniko::ImageAlphaType::Alpha,
                        width: image.width,
                        height: image.height,
                    },
                );
            }
        }
        novadraw::render::ResourceOp::Remove(id) => {
            images.retain(|(resource_id, _), _| resource_id != id);
        }
    };
    match resources {
        novadraw::render::ResourceSync::Delta(delta) => delta.ops.iter().for_each(&mut apply),
        novadraw::render::ResourceSync::Snapshot(snapshot) => snapshot
            .ready
            .iter()
            .cloned()
            .map(novadraw::render::ResourceOp::Upsert)
            .for_each(|operation| apply(&operation)),
    }
}

#[cfg(any(feature = "native", target_arch = "wasm32"))]
fn create_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions::default(),
    )
    .expect("Couldn't create renderer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::Color;
    use novadraw::render::{
        BuiltinFont, FontData, FontDescriptor, ResourceDelta, ResourceId, ResourceOp,
        ResourcePayload, ResourceSnapshot, ResourceSync, ResourceUpdate, TextConstraints,
        TextEngine,
    };
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn line_styles_map_to_width_scaled_vello_dash_patterns() {
        use novadraw::graphics::DashPattern;
        let style = StrokeStyle::default().with_width(2.0).unwrap();
        let solid = vello_stroke(&style, 1.0);
        let dash = vello_stroke(&style.clone().with_dash_pattern(DashPattern::Dash), 1.0);
        let dot = vello_stroke(&style.with_dash_pattern(DashPattern::Dot), 1.0);

        assert!(solid.dash_pattern.is_empty());
        assert_eq!(dash.dash_pattern.as_slice(), &[6.0, 2.0]);
        assert_eq!(dot.dash_pattern.as_slice(), &[2.0, 2.0]);
    }

    #[test]
    fn vello_scene_expands_dash_and_dot_into_multiple_path_segments() {
        use novadraw::graphics::LineStyle;
        let encoded_segments = |line_style: LineStyle| {
            let mut scene = vello::Scene::new();
            let style = StrokeStyle::default()
                .with_width(2.0)
                .unwrap()
                .with_dash_pattern(line_style.into());
            let stroke = vello_stroke(&style, 1.0);
            scene.stroke(
                &stroke,
                vello::kurbo::Affine::IDENTITY,
                VelloColor::new([0.0, 0.0, 0.0, 1.0]),
                None,
                &vello::kurbo::Line::new((0.0, 0.0), (100.0, 0.0)),
            );
            scene.encoding().n_path_segments
        };

        let solid_segments = encoded_segments(LineStyle::Solid);
        assert!(encoded_segments(LineStyle::Dash) > solid_segments);
        assert!(encoded_segments(LineStyle::Dot) > solid_segments);
    }

    #[test]
    fn custom_stroke_scales_lengths_once_and_preserves_dimensionless_miter() {
        use novadraw::graphics::{CustomDash, DashPattern};
        let style = StrokeStyle::try_new(
            3.0,
            LineCap::Square,
            LineJoin::Miter,
            DashPattern::Custom(CustomDash::try_new(&[6.0, 2.0, 1.0]).unwrap()),
            -2.0,
            8.0,
        )
        .unwrap();
        let lowered = vello_stroke(&style, 2.0);
        assert_eq!(lowered.width, 6.0);
        assert_eq!(
            lowered.dash_pattern.as_slice(),
            &[12.0, 4.0, 2.0, 12.0, 4.0, 2.0]
        );
        assert_eq!(lowered.dash_offset, 32.0);
        assert_eq!(lowered.miter_limit, 8.0);
        assert_eq!(lowered.start_cap, Cap::Square);
        assert_eq!(lowered.end_cap, Cap::Square);
    }

    #[test]
    fn clip_restore_plan_replays_saved_outer_clip_after_reset() {
        let outer = RenderClip {
            transform: Affine2D::from_translation(10.0, 20.0),
            geometry: ClipGeometry::Rectangle(Rectangle::new(0.0, 0.0, 100.0, 100.0)),
        };
        let current_after_reset = RenderState::default();
        let saved = RenderState {
            transform: Affine2D::IDENTITY,
            clips: vec![outer.clone()],
        };

        let (pop_count, clips_to_replay) = clip_restore_plan(&current_after_reset, &saved.clips);

        assert_eq!(pop_count, 0);
        assert_eq!(clips_to_replay, [outer]);
    }

    #[test]
    fn clip_restore_plan_keeps_the_common_prefix() {
        let outer = RenderClip {
            transform: Affine2D::IDENTITY,
            geometry: ClipGeometry::Rectangle(Rectangle::new(0.0, 0.0, 100.0, 100.0)),
        };
        let inner = RenderClip {
            transform: Affine2D::IDENTITY,
            geometry: ClipGeometry::Rectangle(Rectangle::new(10.0, 10.0, 10.0, 10.0)),
        };
        let current = RenderState {
            transform: Affine2D::IDENTITY,
            clips: vec![outer.clone(), inner],
        };

        let (pop_count, clips_to_replay) =
            clip_restore_plan(&current, std::slice::from_ref(&outer));

        assert_eq!(pop_count, 1);
        assert!(clips_to_replay.is_empty());
    }

    #[test]
    fn clip_restore_compares_path_geometry_rule_and_captured_transform() {
        let mut path = Path::new();
        path.rect(0.0, 0.0, 100.0, 100.0);
        path.rect(25.0, 25.0, 50.0, 50.0);
        let outer = RenderClip {
            transform: Affine2D::IDENTITY,
            geometry: ClipGeometry::Rectangle(Rectangle::new(0.0, 0.0, 100.0, 100.0)),
        };
        let saved_path = RenderClip {
            transform: Affine2D::from_translation(10.0, 20.0),
            geometry: ClipGeometry::Path(ClipPath::try_new(&path, FillRule::EvenOdd).unwrap()),
        };
        let saved = vec![outer.clone(), saved_path.clone()];
        let mut triangle = Path::new();
        triangle.move_to(0.0, 0.0);
        triangle.line_to(100.0, 0.0);
        triangle.line_to(50.0, 100.0);
        triangle.close();
        for changed in [
            RenderClip {
                transform: Affine2D::IDENTITY,
                ..saved_path.clone()
            },
            RenderClip {
                geometry: ClipGeometry::Path(ClipPath::try_new(&path, FillRule::NonZero).unwrap()),
                ..saved_path.clone()
            },
            RenderClip {
                geometry: ClipGeometry::Path(
                    ClipPath::try_new(&triangle, FillRule::EvenOdd).unwrap(),
                ),
                ..saved_path.clone()
            },
        ] {
            let current = RenderState {
                transform: Affine2D::IDENTITY,
                clips: vec![outer.clone(), changed],
            };
            let (pop, replay) = clip_restore_plan(&current, &saved);
            assert_eq!(pop, 1);
            assert_eq!(replay, std::slice::from_ref(&saved_path));
        }
        let current = RenderState {
            transform: Affine2D::from_scale(2.0, 3.0),
            clips: saved.clone(),
        };
        let (pop, replay) = clip_restore_plan(&current, &saved);
        assert_eq!(
            pop, 0,
            "later drawing transforms must not move the saved clip"
        );
        assert!(replay.is_empty());
        let (pop, replay) = clip_restore_plan(&RenderState::default(), &saved);
        assert_eq!(pop, 0);
        assert_eq!(replay, saved);
    }

    #[test]
    fn path_clip_lowering_keeps_curves_rules_and_empty_clip_layers() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.cubic_to(0.0, 10.0, 10.0, 10.0, 10.0, 0.0);
        let lowered = path_to_vello(&path, 2.0);
        assert!(matches!(lowered.elements()[1],
            vello::kurbo::PathEl::CurveTo(a, b, c)
            if a == (0.0, 20.0).into() && b == (20.0, 20.0).into() && c == (20.0, 0.0).into()));
        for (rule, expected) in [
            (FillRule::EvenOdd, vello::peniko::Fill::EvenOdd),
            (FillRule::NonZero, vello::peniko::Fill::NonZero),
        ] {
            assert_eq!(vello_fill(rule), expected);
            for source in [&path, &Path::new()] {
                let clip = ClipGeometry::Path(ClipPath::try_new(source, rule).unwrap());
                let mut scene = vello::Scene::new();
                append_clip_layer(&mut scene, &clip, vello::kurbo::Affine::IDENTITY, 2.0);
                assert_eq!(scene.encoding().n_open_clips, 1);
                assert!(
                    !scene.encoding().path_tags.is_empty(),
                    "empty clip still suppresses paint"
                );
                scene.pop_layer();
                assert_eq!(scene.encoding().n_open_clips, 0);
                assert_eq!(scene.encoding().n_clips, 2);
            }
        }
    }

    #[test]
    fn zero_surface_extent_suspends_rendering() {
        assert!(surface_is_suspended(0, 100));
        assert!(surface_is_suspended(100, 0));
        assert!(!surface_is_suspended(100, 100));
    }

    #[test]
    fn surface_statuses_select_the_expected_recovery() {
        assert_eq!(
            surface_recovery(&vello::wgpu::CurrentSurfaceTexture::Lost),
            Some(SurfaceRecovery::Reconfigure)
        );
        assert_eq!(
            surface_recovery(&vello::wgpu::CurrentSurfaceTexture::Outdated),
            Some(SurfaceRecovery::Reconfigure)
        );
        assert_eq!(
            surface_recovery(&vello::wgpu::CurrentSurfaceTexture::Timeout),
            Some(SurfaceRecovery::Retry)
        );
        assert_eq!(
            surface_recovery(&vello::wgpu::CurrentSurfaceTexture::Validation),
            Some(SurfaceRecovery::Retry)
        );
        assert_eq!(
            surface_recovery(&vello::wgpu::CurrentSurfaceTexture::Occluded),
            Some(SurfaceRecovery::Skip)
        );
    }

    #[test]
    fn scratch_uses_opaque_background_for_direct_retained_copy() {
        assert_eq!(
            scratch_base_rgba(),
            [
                DEFAULT_BACKGROUND_COMPONENT as f32,
                DEFAULT_BACKGROUND_COMPONENT as f32,
                DEFAULT_BACKGROUND_COMPONENT as f32,
                1.0,
            ]
        );
    }

    #[test]
    fn fractional_damage_is_rounded_outward_for_retained_surface_copy() {
        assert_eq!(
            damage_rect_to_copy_region(Rectangle::new(0.5, 1.25, 1.0, 2.0), 10, 10, 1.0),
            Some((0, 1, 2, 3))
        );
        assert_eq!(
            damage_rect_to_copy_region(Rectangle::new(4.5, 4.5, 2.0, 2.0), 6, 6, 2.0),
            None
        );
    }

    #[test]
    fn damage_clip_matches_outward_rounded_copy_pixels() {
        assert_eq!(
            damage_rect_to_aligned_clip(
                Rectangle::new(118.21875, 225.1171875, 237.1953125, 183.3359375),
                1_200,
                900,
                2.0,
            ),
            Some(Rectangle::new(118.0, 225.0, 237.5, 183.5))
        );
    }

    #[test]
    fn partial_copy_produces_the_same_pixels_as_full_replacement() {
        const WIDTH: usize = 4;
        const HEIGHT: usize = 3;
        let old = vec![1_u8; WIDTH * HEIGHT];
        let mut next = old.clone();
        for y in 1..3 {
            for x in 1..4 {
                next[y * WIDTH + x] = 9;
            }
        }

        let (x, y, width, height) =
            damage_rect_to_copy_region(Rectangle::new(1.0, 1.0, 3.0, 2.0), 4, 3, 1.0)
                .expect("damage intersects surface");
        let mut retained = old;
        for row in y as usize..(y + height) as usize {
            let start = row * WIDTH + x as usize;
            let end = start + width as usize;
            retained[start..end].copy_from_slice(&next[start..end]);
        }

        assert_eq!(retained, next);
    }

    #[test]
    fn positioned_glyph_run_is_encoded_into_the_vello_scene() {
        let mut engine = TextEngine::new();
        let font_id = ResourceId::new(Uuid::nil(), 1);
        engine
            .register_font(font_id, 1, BuiltinFont::Inter.bytes())
            .unwrap();
        let layout = engine
            .layout(
                "Vello",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let mut scene = vello::Scene::new();

        for run in layout.glyph_runs() {
            let font = vello::peniko::FontData::new(
                BuiltinFont::Inter.bytes().to_vec().into(),
                run.font.collection_index(),
            );
            append_glyph_run(
                &mut scene,
                run,
                &font,
                Point::new(8.0, 12.0),
                &GlyphPaint::Fill(Color::BLACK.into()),
                &Affine2D::IDENTITY,
                2.0,
            );
        }

        assert!(!scene.encoding().resources.glyph_runs.is_empty());
        assert!(!scene.encoding().resources.glyphs.is_empty());
        assert!(!scene.encoding().resources.patches.is_empty());
    }

    #[test]
    fn gradient_lowering_keeps_stops_and_explicit_interpolation_with_dpi() {
        use novadraw::graphics::{GradientStop, LinearGradient};
        use vello::peniko::{Brush, Extend, GradientKind, InterpolationAlphaSpace};
        let gradient = LinearGradient::try_new(
            Point::new(10.0, 20.0),
            Point::new(30.0, 40.0),
            &[
                GradientStop::try_new(0.0, Color::BLACK).unwrap(),
                GradientStop::try_new(0.5, Color::BLACK.with_alpha(0.25)).unwrap(),
                GradientStop::try_new(0.5, Color::WHITE).unwrap(),
                GradientStop::try_new(1.0, Color::WHITE).unwrap(),
            ],
        )
        .unwrap();
        let Brush::Gradient(lowered) = vello_paint(&gradient.into(), 2.0) else {
            panic!("expected gradient brush")
        };
        let GradientKind::Linear(position) = lowered.kind else {
            panic!("expected linear")
        };
        assert_eq!(position.start, vello::kurbo::Point::new(20.0, 40.0));
        assert_eq!(position.end, vello::kurbo::Point::new(60.0, 80.0));
        assert_eq!(lowered.extend, Extend::Pad);
        assert_eq!(
            lowered.interpolation_cs,
            vello::peniko::color::ColorSpaceTag::Srgb
        );
        assert_eq!(
            lowered.interpolation_alpha_space,
            InterpolationAlphaSpace::Premultiplied
        );
        assert_eq!(
            lowered.stops.iter().map(|s| s.offset).collect::<Vec<_>>(),
            vec![0.0, 0.5, 0.5, 1.0]
        );
        assert_eq!(lowered.stops[1].color.components[3], 0.25);
    }

    #[test]
    fn dashed_glyphs_use_paths_because_vello_glyph_cache_ignores_dash() {
        use novadraw::graphics::{CustomDash, DashPattern};
        let mut engine = TextEngine::new();
        engine
            .register_font(
                ResourceId::new(Uuid::nil(), 1),
                1,
                BuiltinFont::Inter.bytes(),
            )
            .unwrap();
        let layout = engine
            .layout(
                "Vello",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let paint = GlyphPaint::Stroke {
            paint: Color::BLACK.into(),
            stroke: StrokeStyle::default().with_dash_pattern(DashPattern::Custom(
                CustomDash::try_new(&[2.0, 1.0]).unwrap(),
            )),
        };
        let mut scene = vello::Scene::new();
        for run in layout.glyph_runs() {
            let font = vello::peniko::FontData::new(
                BuiltinFont::Inter.bytes().to_vec().into(),
                run.font.collection_index(),
            );
            append_glyph_run(
                &mut scene,
                run,
                &font,
                Point::new(8.0, 12.0),
                &paint,
                &Affine2D::IDENTITY,
                2.0,
            );
        }
        assert!(scene.encoding().resources.glyph_runs.is_empty());
        assert!(!scene.encoding().path_data.is_empty());
    }

    #[test]
    fn font_cache_replacement_and_removal_discard_stale_revisions() {
        let id = ResourceId::new(Uuid::nil(), 1);
        let mut cache = HashMap::new();
        cache.insert((id, 1), vello::peniko::Blob::new(Arc::new(vec![1_u8])));

        sync_font_face_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![ResourceOp::Upsert(ResourceUpdate {
                    id,
                    revision: 2,
                    payload: ResourcePayload::Font(Arc::new(FontData::new(vec![2]))),
                })],
            }),
        );

        assert!(!cache.contains_key(&(id, 1)));
        assert!(cache.contains_key(&(id, 2)));

        sync_font_face_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![
                    ResourceOp::Upsert(ResourceUpdate {
                        id,
                        revision: 3,
                        payload: ResourcePayload::Font(Arc::new(FontData::new(vec![3]))),
                    }),
                    ResourceOp::Remove(id),
                ],
            }),
        );
        assert!(!cache.keys().any(|(resource_id, _)| *resource_id == id));
    }

    #[test]
    fn image_cache_replacement_and_removal_discard_stale_revisions() {
        let id = ResourceId::new(Uuid::nil(), 2);
        let mut cache = HashMap::new();

        sync_image_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![ResourceOp::Upsert(ResourceUpdate {
                    id,
                    revision: 1,
                    payload: ResourcePayload::Image(Arc::new(
                        novadraw::render::ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0),
                    )),
                })],
            }),
        );
        assert!(cache.contains_key(&(id, 1)));

        sync_image_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![ResourceOp::Upsert(ResourceUpdate {
                    id,
                    revision: 2,
                    payload: ResourcePayload::Image(Arc::new(
                        novadraw::render::ImageData::from_rgba(1, 1, vec![0, 0, 255, 255], 1.0),
                    )),
                })],
            }),
        );
        assert!(!cache.contains_key(&(id, 1)));
        assert!(cache.contains_key(&(id, 2)));

        sync_image_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![ResourceOp::Remove(id)],
            }),
        );
        assert!(!cache.keys().any(|(resource_id, _)| *resource_id == id));
    }

    #[test]
    fn ordered_resource_ops_keep_the_latest_ready_image() {
        let id = ResourceId::new(Uuid::nil(), 3);
        let image = |revision, value| ResourceUpdate {
            id,
            revision,
            payload: ResourcePayload::Image(Arc::new(novadraw::render::ImageData::from_rgba(
                1,
                1,
                vec![value; 4],
                1.0,
            ))),
        };
        let mut cache = HashMap::new();

        sync_image_cache(
            &mut cache,
            &ResourceSync::Delta(ResourceDelta {
                ops: vec![
                    ResourceOp::Upsert(image(1, 1)),
                    ResourceOp::Remove(id),
                    ResourceOp::Upsert(image(2, 2)),
                ],
            }),
        );

        assert!(!cache.contains_key(&(id, 1)));
        assert!(cache.contains_key(&(id, 2)));
    }

    #[test]
    fn resource_snapshot_replaces_a_partially_applied_cache() {
        let stale = ResourceId::new(Uuid::nil(), 4);
        let ready = ResourceId::new(Uuid::nil(), 5);
        let mut cache = HashMap::new();
        cache.insert(
            (stale, 1),
            vello::peniko::ImageData {
                data: vec![1; 4].into(),
                format: vello::peniko::ImageFormat::Rgba8,
                alpha_type: vello::peniko::ImageAlphaType::Alpha,
                width: 1,
                height: 1,
            },
        );

        sync_image_cache(
            &mut cache,
            &ResourceSync::Snapshot(ResourceSnapshot {
                ready: vec![ResourceUpdate {
                    id: ready,
                    revision: 2,
                    payload: ResourcePayload::Image(Arc::new(
                        novadraw::render::ImageData::from_rgba(1, 1, vec![2; 4], 1.0),
                    )),
                }],
            }),
        );

        assert!(!cache.keys().any(|(id, _)| *id == stale));
        assert!(cache.contains_key(&(ready, 2)));
    }

    #[test]
    fn image_source_region_maps_to_destination_and_requires_a_destination_clip() {
        let plan = image_draw_plan(
            80,
            40,
            Rectangle::new(10.0, 5.0, 20.0, 10.0),
            Rectangle::new(100.0, 200.0, 60.0, 80.0),
            2.0,
        )
        .expect("valid image source region");

        assert_eq!(
            plan.local_affine.as_coeffs(),
            [6.0, 0.0, 0.0, 16.0, 140.0, 320.0]
        );
        assert_eq!(plan.clip_rect, Rectangle::new(100.0, 200.0, 60.0, 80.0));

        let image = vello::peniko::ImageBrush {
            image: vello::peniko::ImageData {
                data: vec![255; 80 * 40 * 4].into(),
                format: vello::peniko::ImageFormat::Rgba8,
                alpha_type: vello::peniko::ImageAlphaType::Alpha,
                width: 80,
                height: 40,
            },
            sampler: vello::peniko::ImageSampler::default(),
        };
        let mut scene = vello::Scene::new();
        append_image_draw(
            &mut scene,
            &image,
            plan.local_affine,
            vello::kurbo::Affine::IDENTITY,
            plan.clip_rect,
            2.0,
        );

        assert_eq!(scene.encoding().n_clips, 2);
        assert_eq!(scene.encoding().n_open_clips, 0);
        assert!(!scene.encoding().resources.patches.is_empty());
        assert!(
            image_draw_plan(
                80,
                40,
                Rectangle::new(79.0, 0.0, 2.0, 1.0),
                plan.clip_rect,
                2.0,
            )
            .is_none()
        );
    }
}
