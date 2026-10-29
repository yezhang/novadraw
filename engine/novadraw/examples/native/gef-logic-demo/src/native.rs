use crate::{model::Circuit, session::Session, ui::Ui};
use eframe::egui::{self, TextureId};
use egui_wgpu::wgpu;
use novadraw_backend_vello::texture::VelloTextureRenderer;

pub fn run() -> eframe::Result {
    eframe::run_native(
        "Novadraw - Logic · first slice",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1160.0, 660.0])
                .with_min_inner_size([1000.0, 580.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|context| Ok(Box::new(App::new(context)?))),
    )
}
struct App {
    session: Session,
    ui: Ui,
    state: egui_wgpu::RenderState,
    renderer: VelloTextureRenderer,
    target: Target,
    texture_id: TextureId,
}
impl App {
    fn new(context: &eframe::CreationContext<'_>) -> Result<Self, String> {
        crate::ui::configure_style(&context.egui_ctx);
        let state = context
            .wgpu_render_state
            .clone()
            .ok_or("WGPU renderer was not initialized")?;
        let target = Target::new(&state.device, [1, 1]);
        let texture_id = state.renderer.write().register_native_texture(
            &state.device,
            &target.view,
            wgpu::FilterMode::Linear,
        );
        let renderer =
            VelloTextureRenderer::new(&state.device, &state.queue).map_err(|e| e.to_string())?;
        Ok(Self {
            session: Session::new(Circuit::half_adder())?,
            ui: Ui::default(),
            state,
            renderer,
            target,
            texture_id,
        })
    }
}
impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let size = self.ui.show(root, &mut self.session, self.texture_id);
        let scale = f64::from(root.ctx().pixels_per_point());
        let pixels = [
            (size[0] * scale).ceil().max(1.0) as u32,
            (size[1] * scale).ceil().max(1.0) as u32,
        ];
        if pixels != self.target.size {
            self.target = Target::new(&self.state.device, pixels);
            self.state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    &self.state.device,
                    &self.target.view,
                    wgpu::FilterMode::Linear,
                    self.texture_id,
                );
        }
        match self
            .session
            .render(size, scale, &mut self.renderer.bind(&self.target.texture))
        {
            Ok(retry) => {
                if retry || self.session.domain.has_active_gesture() {
                    root.ctx().request_repaint();
                }
            }
            Err(error) => self.ui.error = Some(error),
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.state.renderer.write().free_texture(&self.texture_id);
    }
}
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}
impl Target {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Logic canvas"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            size,
        }
    }
}
