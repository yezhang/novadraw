//! Shared eframe/egui GPU target. Window presentation remains host-owned.
use egui_wgpu::{RenderState, wgpu};
use novadraw::render::SurfaceInfo;
use novadraw_backend_vello::texture::{TextureBackend, VelloTextureRenderer};

/// Owns a renderer and one egui texture registration on the host's device.
pub struct CanvasSurface {
    gpu: RenderState,
    renderer: VelloTextureRenderer,
    target: Option<wgpu::Texture>,
    texture_id: Option<egui::TextureId>,
    surface: Option<SurfaceInfo>,
}

impl CanvasSurface {
    /// Pass `CreationContext::wgpu_render_state.clone()` from an eframe WGPU app.
    pub fn new(gpu: RenderState) -> Result<Self, String> {
        let renderer = VelloTextureRenderer::new(&gpu.device, &gpu.queue)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            gpu,
            renderer,
            target: None,
            texture_id: None,
            surface: None,
        })
    }

    /// Validates dimensions before allocation. Returns whether the surface changed.
    pub fn resize(&mut self, size: egui::Vec2, scale: f32) -> Result<bool, String> {
        let surface = canvas_extent(
            size,
            scale,
            self.gpu.device.limits().max_texture_dimension_2d,
        )?;
        if self.surface == surface {
            return Ok(false);
        }
        if let Some(surface) = surface {
            let dimensions = [surface.pixel_width, surface.pixel_height];
            let needs_texture = self
                .target
                .as_ref()
                .is_none_or(|t| [t.width(), t.height()] != dimensions);
            if needs_texture {
                let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Novadraw egui Canvas"),
                    size: wgpu::Extent3d {
                        width: dimensions[0],
                        height: dimensions[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                let mut renderer = self.gpu.renderer.write();
                if let Some(id) = self.texture_id {
                    renderer.update_egui_texture_from_wgpu_texture(
                        &self.gpu.device,
                        &view,
                        wgpu::FilterMode::Linear,
                        id,
                    );
                } else {
                    self.texture_id = Some(renderer.register_native_texture(
                        &self.gpu.device,
                        &view,
                        wgpu::FilterMode::Linear,
                    ));
                }
                self.target = Some(texture);
            }
        }
        self.surface = surface;
        Ok(true)
    }

    /// The validated active surface; zero extents suspend it.
    pub fn surface(&self) -> Option<SurfaceInfo> {
        self.surface
    }

    /// Registration remains stable across resize on the same device.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.texture_id
    }

    /// Submits through the supplied Runtime/Viewer protocol without owning it.
    pub fn with_backend<T>(
        &mut self,
        submit: impl FnOnce(SurfaceInfo, &mut TextureBackend<'_>) -> T,
    ) -> Option<T> {
        let surface = self.surface?;
        let target = self.target.as_ref()?;
        Some(submit(surface, &mut self.renderer.bind(target)))
    }

    /// Paints the current texture clipped by the calling UI's painter.
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        if self.surface.is_some()
            && let Some(texture) = self.texture_id
        {
            ui.painter().image(
                texture,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}

impl Drop for CanvasSurface {
    fn drop(&mut self) {
        if let Some(id) = self.texture_id.take() {
            self.gpu.renderer.write().free_texture(&id);
        }
    }
}

fn canvas_extent(size: egui::Vec2, scale: f32, limit: u32) -> Result<Option<SurfaceInfo>, String> {
    if !size.is_finite() || size.x < 0.0 || size.y < 0.0 || !scale.is_finite() || scale <= 0.0 {
        return Err("invalid Canvas extent or DPI".into());
    }
    let width = (f64::from(size.x) * f64::from(scale)).ceil();
    let height = (f64::from(size.y) * f64::from(scale)).ceil();
    if width > f64::from(limit) || height > f64::from(limit) {
        return Err("Canvas exceeds the GPU texture dimension limit".into());
    }
    if width == 0.0 || height == 0.0 {
        return Ok(None);
    }
    Ok(Some(SurfaceInfo {
        logical_width: f64::from(size.x),
        logical_height: f64::from(size.y),
        pixel_width: width as u32,
        pixel_height: height as u32,
        scale_factor: f64::from(scale),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_extent_before_allocation() {
        assert!(canvas_extent(egui::vec2(f32::NAN, 1.0), 1.0, 4096).is_err());
        assert!(canvas_extent(egui::vec2(-1.0, 1.0), 1.0, 4096).is_err());
        assert!(canvas_extent(egui::vec2(1.0, 1.0), 0.0, 4096).is_err());
        assert!(canvas_extent(egui::vec2(3000.0, 2.0), 2.0, 4096).is_err());
        assert!(
            canvas_extent(egui::Vec2::ZERO, 2.0, 4096)
                .unwrap()
                .is_none()
        );
        let surface = canvas_extent(egui::vec2(10.25, 12.5), 2.0, 4096)
            .unwrap()
            .unwrap();
        assert_eq!((surface.pixel_width, surface.pixel_height), (21, 25));
        assert_eq!(surface.logical_width, 10.25);
    }
}
