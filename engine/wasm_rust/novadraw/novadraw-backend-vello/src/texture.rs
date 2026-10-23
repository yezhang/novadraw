//! Novadraw rendering into an application-owned RGBA texture.
//!
//! No Window or Surface is created. The application owns composition, device
//! failure callbacks, and final window presentation. Use one renderer per Runtime.
use crate::{SceneEncoder, render_failure_from_vello};
use novadraw::geometry::Rectangle;
use novadraw::render::{
    BackendCapabilities, BackendSessionGate, DamageMode, RenderBackend, RenderDisposition,
    RenderFailure, RenderFailureDiagnostic, RenderResult, RenderSubmission,
};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, peniko::Color, wgpu};

/// Shares the application's device and queue without depending on a Canvas library.
pub struct VelloTextureRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    encoder: SceneEncoder,
    session_gate: BackendSessionGate,
}

impl VelloTextureRenderer {
    /// Device and queue must belong to the same WGPU device.
    /// Device-lost and uncaptured-error callbacks remain application-owned.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, vello::Error> {
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            renderer: Renderer::new(device, RendererOptions::default())?,
            encoder: SceneEncoder::new(1.0),
            session_gate: BackendSessionGate::default(),
        })
    }

    /// Binds an application-owned target for a frame. Rebinding retains resource
    /// caches and session validation; texture and renderer must share a device.
    pub fn bind<'a>(&'a mut self, texture: &'a wgpu::Texture) -> TextureBackend<'a> {
        TextureBackend {
            renderer: self,
            texture,
        }
    }
}

/// A scoped RenderBackend, suitable for Runtime prepare/complete_submission.
pub struct TextureBackend<'a> {
    renderer: &'a mut VelloTextureRenderer,
    texture: &'a wgpu::Texture,
}

impl RenderBackend for TextureBackend<'_> {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
            .with_glyph_runs()
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
            .with_host_background()
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        submission.validate_composition(self.capabilities())?;
        self.capabilities()
            .validate_capabilities(&submission.commands)
            .map_err(RenderFailure::Unsupported)?;
        novadraw::render::validate_graphics_input(
            &submission.commands,
            submission.surface.scale_factor,
        )
        .map_err(RenderFailure::InvalidInput)?;
        let surface = submission.surface;
        if surface.pixel_width == 0 || surface.pixel_height == 0 {
            return Ok(RenderDisposition::Skipped);
        }
        if self.texture.width() != surface.pixel_width
            || self.texture.height() != surface.pixel_height
            || self.texture.format() != wgpu::TextureFormat::Rgba8Unorm
            || self.texture.dimension() != wgpu::TextureDimension::D2
            || self.texture.depth_or_array_layers() != 1
            || self.texture.sample_count() != 1
            || !self
                .texture
                .usage()
                .contains(wgpu::TextureUsages::STORAGE_BINDING)
            || !matches!(submission.damage.mode(), DamageMode::Full)
        {
            return Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                "texture backend requires full-frame damage and a matching single-sample RGBA8 storage texture",
            )));
        }
        let renderer = &mut self.renderer;
        renderer
            .encoder
            .sync_resources(&mut renderer.session_gate, submission)?;
        if !renderer
            .encoder
            .has_required_resources(&submission.commands)
        {
            return Ok(RenderDisposition::Retry);
        }
        renderer.encoder.encode(
            &submission.commands,
            Rectangle::new(
                0.0,
                0.0,
                f64::from(surface.pixel_width) / surface.scale_factor,
                f64::from(surface.pixel_height) / surface.scale_factor,
            ),
            surface.scale_factor,
        );
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            array_layer_count: Some(1),
            ..Default::default()
        });
        renderer
            .renderer
            .render_to_texture(
                &renderer.device,
                &renderer.queue,
                &renderer.encoder.scene,
                &view,
                &RenderParams {
                    base_color: Color::new(submission.clear_color.map_or(
                        [0.0, 0.0, 0.0, 0.0],
                        |color| {
                            [
                                color.red() as f32,
                                color.green() as f32,
                                color.blue() as f32,
                                color.alpha() as f32,
                            ]
                        },
                    )),
                    width: surface.pixel_width,
                    height: surface.pixel_height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .map_err(render_failure_from_vello)?;
        // Runtime accepts the submitted texture. The host presents its composite.
        Ok(RenderDisposition::Presented)
    }

    fn resize(&mut self, _: u32, _: u32, _: f64) {
        // Allocation belongs to the application. bind() supplies the resized target.
    }
}
