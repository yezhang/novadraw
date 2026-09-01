//! Vello 渲染器实现
//!
//! 实现 RenderCommand 解释器，维护独立的状态栈。
//! 状态管理从 NdCanvas 移到本模块（参考 skia/Flutter DisplayList 设计）。

use std::sync::Arc;

use glam::DVec2;
use image::ImageBuffer;
use novadraw_geometry::{Rectangle, Transform};
use tracing::debug;
use vello::kurbo::{Cap, Join, Stroke};
use vello::peniko::Color as VelloColor;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions};

use crate::command::RenderCommand;
use crate::submission::DamageMode;
use crate::traits::{RenderBackend, RenderOutcome, WindowProxy};

pub mod winit;
pub use winit::{WinitWindowProxy, WinitWindowProxyInner};

const DEFAULT_BACKGROUND_COMPONENT: f64 = 238.0 / 255.0;
const DEFAULT_BACKGROUND_COLOR: vello::wgpu::Color = vello::wgpu::Color {
    r: DEFAULT_BACKGROUND_COMPONENT,
    g: DEFAULT_BACKGROUND_COMPONENT,
    b: DEFAULT_BACKGROUND_COMPONENT,
    a: 1.0,
};

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

fn scratch_base_rgba(full_damage: bool) -> [f32; 4] {
    if full_damage {
        [
            DEFAULT_BACKGROUND_COMPONENT as f32,
            DEFAULT_BACKGROUND_COMPONENT as f32,
            DEFAULT_BACKGROUND_COMPONENT as f32,
            1.0,
        ]
    } else {
        [0.0, 0.0, 0.0, 0.0]
    }
}

/// 渲染状态
#[derive(Clone, Debug, Default)]
struct RenderState {
    /// 当前变换矩阵
    transform: Transform,
    /// 当前状态下可重放的裁剪层。
    clips: Vec<RenderClip>,
}

#[derive(Clone, Debug, PartialEq)]
struct RenderClip {
    transform: Transform,
    rect: [DVec2; 2],
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
    window: Arc<WinitWindowProxy>,
    scale_factor: f64,
    surface_suspended: bool,
    /// 状态栈
    state_stack: Vec<RenderState>,
    /// 保留上一帧完整结果的纹理（也作为截图源）
    retained_texture: Option<(vello::wgpu::Texture, vello::wgpu::TextureView, u32, u32)>,
    /// 本帧临时渲染纹理
    scratch_texture: Option<(vello::wgpu::Texture, vello::wgpu::TextureView, u32, u32)>,
}

impl VelloRenderer {
    fn current_surface_size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }

    pub fn new(window: Arc<WinitWindowProxy>, logical_width: f64, logical_height: f64) -> Self {
        let scale_factor = window.scale_factor();
        let width = (logical_width * scale_factor) as u32;
        let height = (logical_height * scale_factor) as u32;

        let mut render_context = RenderContext::new();
        let surface_future = render_context.create_surface(
            window.window().clone(),
            width,
            height,
            vello::wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Failed to create surface");

        let mut renderers = vec![];
        renderers.resize_with(render_context.devices.len(), || None);
        renderers[surface.dev_id].get_or_insert_with(|| create_renderer(&render_context, &surface));

        VelloRenderer {
            render_context,
            renderers,
            scene: vello::Scene::new(),
            surface,
            window,
            scale_factor,
            surface_suspended: false,
            state_stack: vec![RenderState::default()],
            retained_texture: None,
            scratch_texture: None,
        }
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

    fn recover_surface(&mut self, recovery: SurfaceRecovery) -> RenderOutcome {
        match recovery {
            SurfaceRecovery::Reconfigure => {
                let width = self.window.width();
                let height = self.window.height();
                let scale_factor = self.window.scale_factor();
                self.resize(width, height, scale_factor);
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
        let origin_x = (rect.x * self.scale_factor).floor().max(0.0) as u32;
        let origin_y = (rect.y * self.scale_factor).floor().max(0.0) as u32;
        let max_width = width.saturating_sub(origin_x);
        let max_height = height.saturating_sub(origin_y);
        let copy_width = ((rect.width * self.scale_factor).ceil().max(0.0) as u32).min(max_width);
        let copy_height =
            ((rect.height * self.scale_factor).ceil().max(0.0) as u32).min(max_height);

        if copy_width == 0 || copy_height == 0 {
            return None;
        }

        Some((origin_x, origin_y, copy_width, copy_height))
    }

    fn effective_damage_regions(
        &self,
        submission: &crate::RenderSubmission,
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
            crate::command::RenderCommandKind::PushState => {
                debug!("PushState, stack depth: {}", self.state_stack.len());
                self.state_stack.push(self.current_state().clone());
            }

            crate::command::RenderCommandKind::RestoreState => {
                debug!("RestoreState, stack depth: {}", self.state_stack.len());
                if self.state_stack.len() >= 2 {
                    let saved = self.state_stack[self.state_stack.len() - 2].clone();
                    self.restore_clip_layers(&saved.clips);
                    *self.current_state_mut() = saved;
                }
            }

            crate::command::RenderCommandKind::PopState => {
                debug!("PopState, stack depth: {}", self.state_stack.len());
                if self.state_stack.len() > 1 {
                    let saved = self.state_stack[self.state_stack.len() - 2].clone();
                    self.restore_clip_layers(&saved.clips);
                    self.state_stack.pop();
                }
            }

            crate::command::RenderCommandKind::ConcatTransform { matrix } => {
                debug!("ConcatTransform: {:?}", matrix);
                // 叠加变换
                let new_transform = self.current_state().transform.post_concat(*matrix);

                self.current_state_mut().transform = new_transform;
            }

            crate::command::RenderCommandKind::SetTransform { matrix } => {
                debug!("SetTransform: {:?}", matrix);
                self.current_state_mut().transform = *matrix;
            }

            crate::command::RenderCommandKind::ResetTransform => {
                debug!("ResetTransform");
                self.current_state_mut().transform = Transform::IDENTITY;
            }

            crate::command::RenderCommandKind::Clip { rect } => {
                debug!("Clip: {:?}", rect);
                let clip = RenderClip {
                    transform: self.current_state().transform,
                    rect: *rect,
                };
                self.push_clip_layer(&clip);
                self.current_state_mut().clips.push(clip);
            }

            crate::command::RenderCommandKind::ResetClip => {
                debug!("ResetClip");
                let depth = self.current_state().clips.len();
                self.pop_clip_layers(depth);
                self.current_state_mut().clips.clear();
            }

            // ===== 绘制命令 =====
            crate::command::RenderCommandKind::ClearRect { rect, color } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect[0].x * self.scale_factor;
                let y0 = rect[0].y * self.scale_factor;
                let x1 = rect[1].x * self.scale_factor;
                let y1 = rect[1].y * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);
                self.scene.fill(
                    vello::peniko::Fill::NonZero,
                    affine,
                    vello_color,
                    None,
                    &kurbo_rect,
                );
            }

            crate::command::RenderCommandKind::FillRect { rect, color } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect[0].x * self.scale_factor;
                let y0 = rect[0].y * self.scale_factor;
                let x1 = rect[1].x * self.scale_factor;
                let y1 = rect[1].y * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);
                self.scene.fill(
                    vello::peniko::Fill::NonZero,
                    affine,
                    vello_color,
                    None,
                    &kurbo_rect,
                );
            }

            crate::command::RenderCommandKind::StrokeRect {
                rect,
                color,
                width,
                line_style: _,
                cap,
                join,
            } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let x0 = rect[0].x * self.scale_factor;
                let y0 = rect[0].y * self.scale_factor;
                let x1 = rect[1].x * self.scale_factor;
                let y1 = rect[1].y * self.scale_factor;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);
                let stroke = Stroke::new(*width * self.scale_factor)
                    .with_caps(match cap {
                        crate::command::LineCap::Butt => Cap::Butt,
                        crate::command::LineCap::Round => Cap::Round,
                        crate::command::LineCap::Square => Cap::Square,
                    })
                    .with_join(match join {
                        crate::command::LineJoin::Miter => Join::Miter,
                        crate::command::LineJoin::Round => Join::Round,
                        crate::command::LineJoin::Bevel => Join::Bevel,
                    });
                self.scene
                    .stroke(&stroke, affine, vello_color, None, &kurbo_rect);
            }

            crate::command::RenderCommandKind::Clear { color: _ } => {
                // 未实现
            }

            crate::command::RenderCommandKind::Line {
                p1,
                p2,
                color,
                width,
                line_style: _,
                cap,
                join,
            } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let v1 =
                    vello::kurbo::Point::new(p1.x * self.scale_factor, p1.y * self.scale_factor);
                let v2 =
                    vello::kurbo::Point::new(p2.x * self.scale_factor, p2.y * self.scale_factor);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);

                let stroke = Stroke::new(*width * self.scale_factor)
                    .with_caps(match cap {
                        crate::command::LineCap::Butt => Cap::Butt,
                        crate::command::LineCap::Round => Cap::Round,
                        crate::command::LineCap::Square => Cap::Square,
                    })
                    .with_join(match join {
                        crate::command::LineJoin::Miter => Join::Miter,
                        crate::command::LineJoin::Round => Join::Round,
                        crate::command::LineJoin::Bevel => Join::Bevel,
                    });

                self.scene.stroke(
                    &stroke,
                    affine,
                    vello_color,
                    None,
                    &vello::kurbo::Line::new(v1, v2),
                );
            }

            crate::command::RenderCommandKind::Polyline {
                points,
                color,
                width,
                line_style: _,
                cap,
                join,
            } => {
                if points.len() < 2 {
                    return;
                }
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);

                let stroke = Stroke::new(*width * self.scale_factor)
                    .with_caps(match cap {
                        crate::command::LineCap::Butt => Cap::Butt,
                        crate::command::LineCap::Round => Cap::Round,
                        crate::command::LineCap::Square => Cap::Square,
                    })
                    .with_join(match join {
                        crate::command::LineJoin::Miter => Join::Miter,
                        crate::command::LineJoin::Round => Join::Round,
                        crate::command::LineJoin::Bevel => Join::Bevel,
                    });

                // 构建折线路径
                let mut path = vello::kurbo::BezPath::new();
                let first_point = points[0];
                path.move_to((
                    first_point.x * self.scale_factor,
                    first_point.y * self.scale_factor,
                ));
                for point in &points[1..] {
                    path.line_to((point.x * self.scale_factor, point.y * self.scale_factor));
                }

                self.scene.stroke(&stroke, affine, vello_color, None, &path);
            }

            crate::command::RenderCommandKind::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill_color,
                stroke_color,
                stroke_width,
                line_style: _,
                cap,
                join,
            } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let center =
                    vello::kurbo::Point::new(cx * self.scale_factor, cy * self.scale_factor);
                let radii =
                    vello::kurbo::Vec2::new(*rx * self.scale_factor, *ry * self.scale_factor);
                let ellipse = vello::kurbo::Ellipse::new(center, radii, 0.0);

                // 填充椭圆
                if let Some(color) = fill_color {
                    let vello_color = VelloColor::new([
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    ]);
                    self.scene.fill(
                        vello::peniko::Fill::NonZero,
                        affine,
                        vello_color,
                        None,
                        &ellipse,
                    );
                }

                // 描边椭圆
                if let Some(color) = stroke_color {
                    let vello_color = VelloColor::new([
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    ]);
                    let stroke = Stroke::new(*stroke_width * self.scale_factor)
                        .with_caps(match cap {
                            crate::command::LineCap::Butt => Cap::Butt,
                            crate::command::LineCap::Round => Cap::Round,
                            crate::command::LineCap::Square => Cap::Square,
                        })
                        .with_join(match join {
                            crate::command::LineJoin::Miter => Join::Miter,
                            crate::command::LineJoin::Round => Join::Round,
                            crate::command::LineJoin::Bevel => Join::Bevel,
                        });
                    self.scene
                        .stroke(&stroke, affine, vello_color, None, &ellipse);
                }
            }

            crate::command::RenderCommandKind::FillPath { path, color } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);

                // 构建填充路径
                let mut bez_path = vello::kurbo::BezPath::new();
                let mut current_pos: Option<(f64, f64)> = None;
                for op in path.operations() {
                    match op {
                        crate::command::PathOp::MoveTo(p) => {
                            let px = p.x * self.scale_factor;
                            let py = p.y * self.scale_factor;
                            bez_path.move_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::LineTo(p) => {
                            let px = p.x * self.scale_factor;
                            let py = p.y * self.scale_factor;
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::HLineTo(x) => {
                            let px = *x * self.scale_factor;
                            let py = current_pos.map(|(_, y)| y).unwrap_or(0.0);
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::VLineTo(y) => {
                            let px = current_pos.map(|(x, _)| x).unwrap_or(0.0);
                            let py = *y * self.scale_factor;
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::CubicTo(p0, p1, p2) => {
                            bez_path.curve_to(
                                (p0.x * self.scale_factor, p0.y * self.scale_factor),
                                (p1.x * self.scale_factor, p1.y * self.scale_factor),
                                (p2.x * self.scale_factor, p2.y * self.scale_factor),
                            );
                            current_pos =
                                Some((p2.x * self.scale_factor, p2.y * self.scale_factor));
                        }
                        crate::command::PathOp::QuadTo(p0, p1) => {
                            bez_path.quad_to(
                                (p0.x * self.scale_factor, p0.y * self.scale_factor),
                                (p1.x * self.scale_factor, p1.y * self.scale_factor),
                            );
                            current_pos =
                                Some((p1.x * self.scale_factor, p1.y * self.scale_factor));
                        }
                        crate::command::PathOp::Close => {
                            bez_path.close_path();
                            current_pos = None;
                        }
                        _ => {}
                    }
                }

                // 填充路径 - 需要从 context 获取颜色，这里简化处理
                // TODO: 正确传递颜色
                self.scene.fill(
                    vello::peniko::Fill::NonZero,
                    affine,
                    vello_color,
                    None,
                    &bez_path,
                );
            }

            crate::command::RenderCommandKind::StrokePath {
                path,
                color,
                width,
                line_style: _,
                line_cap,
                line_join,
            } => {
                let affine =
                    Self::transform_to_affine(&self.current_state().transform, self.scale_factor);
                let vello_color = VelloColor::new([
                    color.r as f32,
                    color.g as f32,
                    color.b as f32,
                    color.a as f32,
                ]);

                let cap = match line_cap {
                    crate::command::LineCap::Butt => Cap::Butt,
                    crate::command::LineCap::Round => Cap::Round,
                    crate::command::LineCap::Square => Cap::Square,
                };
                let join = match line_join {
                    crate::command::LineJoin::Miter => Join::Miter,
                    crate::command::LineJoin::Round => Join::Round,
                    crate::command::LineJoin::Bevel => Join::Bevel,
                };
                let stroke = Stroke::new(width * self.scale_factor)
                    .with_caps(cap)
                    .with_join(join);

                // 构建描边路径
                let mut bez_path = vello::kurbo::BezPath::new();
                let mut current_pos: Option<(f64, f64)> = None;
                for op in path.operations() {
                    match op {
                        crate::command::PathOp::MoveTo(p) => {
                            let px = p.x * self.scale_factor;
                            let py = p.y * self.scale_factor;
                            bez_path.move_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::LineTo(p) => {
                            let px = p.x * self.scale_factor;
                            let py = p.y * self.scale_factor;
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::HLineTo(x) => {
                            let px = *x * self.scale_factor;
                            let py = current_pos.map(|(_, y)| y).unwrap_or(0.0);
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::VLineTo(y) => {
                            let px = current_pos.map(|(x, _)| x).unwrap_or(0.0);
                            let py = *y * self.scale_factor;
                            bez_path.line_to((px, py));
                            current_pos = Some((px, py));
                        }
                        crate::command::PathOp::CubicTo(p0, p1, p2) => {
                            bez_path.curve_to(
                                (p0.x * self.scale_factor, p0.y * self.scale_factor),
                                (p1.x * self.scale_factor, p1.y * self.scale_factor),
                                (p2.x * self.scale_factor, p2.y * self.scale_factor),
                            );
                            current_pos =
                                Some((p2.x * self.scale_factor, p2.y * self.scale_factor));
                        }
                        crate::command::PathOp::QuadTo(p0, p1) => {
                            bez_path.quad_to(
                                (p0.x * self.scale_factor, p0.y * self.scale_factor),
                                (p1.x * self.scale_factor, p1.y * self.scale_factor),
                            );
                            current_pos =
                                Some((p1.x * self.scale_factor, p1.y * self.scale_factor));
                        }
                        crate::command::PathOp::Close => {
                            bez_path.close_path();
                            current_pos = None;
                        }
                        _ => {}
                    }
                }

                self.scene
                    .stroke(&stroke, affine, vello_color, None, &bez_path);
            }

            // 其他命令暂未实现
            _ => {}
        }
    }

    /// 将 Transform 转换为 vello Affine
    fn transform_to_affine(transform: &Transform, scale_factor: f64) -> vello::kurbo::Affine {
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
        let rect = &clip.rect;
        let x0 = rect[0].x * self.scale_factor;
        let y0 = rect[0].y * self.scale_factor;
        let x1 = rect[1].x * self.scale_factor;
        let y1 = rect[1].y * self.scale_factor;
        let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
        self.scene
            .push_clip_layer(vello::peniko::Fill::NonZero, affine, &kurbo_rect);
    }
}

impl RenderBackend for VelloRenderer {
    type Window = WinitWindowProxy;

    fn window(&self) -> &Self::Window {
        &self.window
    }

    fn render(&mut self, submission: &crate::RenderSubmission) -> RenderOutcome {
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
        let clip_rect = [
            DVec2::new(effective_damage.x, effective_damage.y),
            DVec2::new(
                effective_damage.x + effective_damage.width,
                effective_damage.y + effective_damage.height,
            ),
        ];

        // self.scene.reset();
        self.scene = vello::Scene::new();

        // 重置状态栈
        self.state_stack.clear();
        self.state_stack.push(RenderState::default());

        self.push_clip_layer(&RenderClip {
            transform: Transform::IDENTITY,
            rect: clip_rect,
        });
        for cmd in commands {
            self.render_command(cmd);
        }
        self.scene.pop_layer();

        debug!("渲染命令执行完成");

        self.ensure_retained_texture();
        self.ensure_scratch_texture();

        let surface_status = self.surface.surface.get_current_texture();
        let (surface_texture, reconfigure_after_present) = match surface_status {
            vello::wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            vello::wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            status => {
                let recovery =
                    surface_recovery(&status).expect("unavailable surface must define recovery");
                return self.recover_surface(recovery);
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
        let base_color = VelloColor::new(scratch_base_rgba(damage.is_full()));

        // 先把当前帧的脏区内容渲染到临时纹理，脏区外保持透明
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
        if reconfigure_after_present {
            self.render_context.configure_surface(&self.surface);
        }
        RenderOutcome::Presented
    }

    fn resize(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.retained_texture = None;
        self.scratch_texture = None;
        self.recreate_surface(pixel_width, pixel_height);
    }
}

impl VelloRenderer {
    /// Recreate the presentation surface while retaining the GPU device and renderer.
    ///
    /// Reconfiguring the existing surface through Vello's `resize_surface` causes visible
    /// oscillation during interactive winit resize on macOS. A fresh surface keeps its
    /// configuration synchronized with the native window without rebuilding render pipelines.
    fn recreate_surface(&mut self, pixel_width: u32, pixel_height: u32) {
        if surface_is_suspended(pixel_width, pixel_height) {
            self.surface_suspended = true;
            return;
        }
        let surface_future = self.render_context.create_surface(
            self.window.window().clone(),
            pixel_width,
            pixel_height,
            vello::wgpu::PresentMode::AutoVsync,
        );
        self.surface = pollster::block_on(surface_future).expect("Failed to recreate surface");
        self.surface_suspended = false;
    }

    /// 截图并保存为 PNG 文件
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
        let buffer_size = (width * height * 4) as u64;
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
                    bytes_per_row: Some(width * 4),
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
        let data: Vec<u8> = data.to_vec();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_restore_plan_replays_saved_outer_clip_after_reset() {
        let outer = RenderClip {
            transform: Transform::from_translation(10.0, 20.0),
            rect: [DVec2::new(0.0, 0.0), DVec2::new(100.0, 100.0)],
        };
        let current_after_reset = RenderState::default();
        let saved = RenderState {
            transform: Transform::IDENTITY,
            clips: vec![outer.clone()],
        };

        let (pop_count, clips_to_replay) = clip_restore_plan(&current_after_reset, &saved.clips);

        assert_eq!(pop_count, 0);
        assert_eq!(clips_to_replay, [outer]);
    }

    #[test]
    fn clip_restore_plan_keeps_the_common_prefix() {
        let outer = RenderClip {
            transform: Transform::IDENTITY,
            rect: [DVec2::new(0.0, 0.0), DVec2::new(100.0, 100.0)],
        };
        let inner = RenderClip {
            transform: Transform::IDENTITY,
            rect: [DVec2::new(10.0, 10.0), DVec2::new(20.0, 20.0)],
        };
        let current = RenderState {
            transform: Transform::IDENTITY,
            clips: vec![outer.clone(), inner],
        };

        let (pop_count, clips_to_replay) =
            clip_restore_plan(&current, std::slice::from_ref(&outer));

        assert_eq!(pop_count, 1);
        assert!(clips_to_replay.is_empty());
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
    fn full_damage_uses_opaque_background_while_partial_damage_stays_transparent() {
        assert_eq!(
            scratch_base_rgba(true),
            [
                DEFAULT_BACKGROUND_COMPONENT as f32,
                DEFAULT_BACKGROUND_COMPONENT as f32,
                DEFAULT_BACKGROUND_COMPONENT as f32,
                1.0,
            ]
        );
        assert_eq!(scratch_base_rgba(false), [0.0, 0.0, 0.0, 0.0]);
    }
}

fn create_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions::default(),
    )
    .expect("Couldn't create renderer")
}
