use crate::{
    AppState, Session, session as create_session, sync,
    ui::{EditorUi, UiOutput},
};
use eframe::egui::{self, TextureId};
use egui_wgpu::wgpu;
use novadraw_backend_vello::texture::VelloTextureRenderer;

const WINDOW_WIDTH: f32 = 1180.0;
const WINDOW_HEIGHT: f32 = 820.0;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Novadraw - Shapes editor")
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([900.0, 620.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "novadraw-gef-shapes",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)?))),
    )
}

struct EditorApp {
    app: AppState,
    session: Session,
    editor_ui: EditorUi,
    render_state: egui_wgpu::RenderState,
    renderer: VelloTextureRenderer,
    target: CanvasTarget,
    texture_id: TextureId,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Result<Self, String> {
        configure_style(&context.egui_ctx);
        let render_state = context
            .wgpu_render_state
            .clone()
            .ok_or("eframe did not initialize its WGPU renderer")?;
        let target = CanvasTarget::new(&render_state.device, [1, 1]);
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &target.view,
            wgpu::FilterMode::Linear,
        );
        let renderer = VelloTextureRenderer::new(&render_state.device, &render_state.queue)
            .map_err(|error| error.to_string())?;
        let mut app = AppState::default();
        let mut session = create_session();
        sync(&mut app, &mut session)?;
        app.canvas = session.snapshot(None);
        Ok(Self {
            app,
            session,
            editor_ui: EditorUi::default(),
            render_state,
            renderer,
            target,
            texture_id,
        })
    }

    fn finish_frame(&mut self, context: &egui::Context, output: UiOutput) {
        let interaction_error = output.interaction.err();
        if let Err(error) = sync(&mut self.app, &mut self.session) {
            self.app.canvas = self.session.snapshot(Some(error));
        } else {
            self.app.canvas = self.session.snapshot(interaction_error);
        }

        let pixels_per_point = f64::from(context.pixels_per_point());
        let size = [
            (output.canvas_size[0] * pixels_per_point)
                .ceil()
                .clamp(1.0, f64::from(u32::MAX)) as u32,
            (output.canvas_size[1] * pixels_per_point)
                .ceil()
                .clamp(1.0, f64::from(u32::MAX)) as u32,
        ];
        if self.target.size != size {
            self.target = CanvasTarget::new(&self.render_state.device, size);
            self.render_state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    &self.render_state.device,
                    &self.target.view,
                    wgpu::FilterMode::Linear,
                    self.texture_id,
                );
        }

        let render_result = self.session.render(
            output.canvas_size,
            pixels_per_point,
            &mut self.renderer.bind(&self.target.texture),
        );
        match render_result {
            Ok(retry) => {
                if retry || self.session.domain.has_active_gesture() {
                    context.request_repaint();
                }
            }
            Err(error) => {
                self.app.canvas = self.session.snapshot(Some(error));
            }
        }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let output = self
            .editor_ui
            .show(root, &mut self.app, &mut self.session, self.texture_id);
        self.finish_frame(root.ctx(), output);
    }
}

impl Drop for EditorApp {
    fn drop(&mut self) {
        self.render_state
            .renderer
            .write()
            .free_texture(&self.texture_id);
    }
}

struct CanvasTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl CanvasTarget {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GEF editor canvas"),
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

fn configure_style(context: &egui::Context) {
    context.set_visuals(egui::Visuals::light());
    context.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.visuals.widgets.inactive.corner_radius = 3.0.into();
        style.visuals.widgets.hovered.corner_radius = 3.0.into();
        style.visuals.widgets.active.corner_radius = 3.0.into();
        style.visuals.widgets.open.corner_radius = 3.0.into();
        style.visuals.selection.bg_fill = crate::ui::SELECTION_BLUE;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, crate::ui::SELECTION_TEXT);
        style.animation_time = 0.0;
    });
}
