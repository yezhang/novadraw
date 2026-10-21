//! Optional host UI. Panel callbacks own example state and use only public Runtime mutations.
use std::sync::{Arc, Mutex};

use novadraw::runtime::Runtime;
use novadraw_backend_vello::{NativeOverlay, wgpu};

pub use egui;

type PanelDraw = Box<dyn FnMut(&mut egui::Ui, &mut Runtime)>;

pub struct Panel {
    draw: PanelDraw,
    position: egui::Pos2,
}

impl Panel {
    pub fn new(
        position: [f32; 2],
        draw: impl FnMut(&mut egui::Ui, &mut Runtime) + 'static,
    ) -> Self {
        Self {
            draw: Box::new(draw),
            position: position.into(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, runtime: &mut Runtime) {
        (self.draw)(ui, runtime);
    }
}
pub type PanelScene = (Runtime, Option<Panel>);
pub type PanelSceneEntry = (&'static str, Box<dyn FnMut() -> PanelScene>);

#[derive(Default)]
struct Frame {
    primitives: Vec<egui::ClippedPrimitive>,
    textures: egui::TexturesDelta,
    pixels_per_point: f32,
}

pub(crate) struct PanelHost {
    pub context: egui::Context,
    pub state: egui_winit::State,
    frame: Arc<Mutex<Frame>>,
    pub repaint_at: std::time::Instant,
}

impl PanelHost {
    pub fn new(window: &winit::window::Window) -> Self {
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::light());
        context.style_mut_of(egui::Theme::Light, |style| style.animation_time = 0.0);
        let state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        Self {
            context,
            state,
            frame: Arc::default(),
            repaint_at: std::time::Instant::now(),
        }
    }

    pub fn overlay(&self) -> Box<dyn NativeOverlay> {
        Box::new(Overlay {
            frame: self.frame.clone(),
            renderer: None,
        })
    }

    pub fn prepare(
        &mut self,
        window: &winit::window::Window,
        panel: &mut Panel,
        runtime: &mut Runtime,
    ) {
        let input = self.state.take_egui_input(window);
        // A host action must commit once even when egui asks for another sizing pass.
        self.context.begin_pass(input);
        egui::Window::new("Example controls")
            .default_pos(panel.position)
            .default_width(320.0)
            .default_height(600.0)
            .resizable(true)
            .vscroll(true)
            .show(&self.context, |ui| panel.show(ui, runtime));
        let output = self.context.end_pass();
        let delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.repaint_delay)
            .unwrap_or(std::time::Duration::MAX);
        self.repaint_at = std::time::Instant::now()
            .checked_add(delay)
            .unwrap_or_else(|| std::time::Instant::now() + std::time::Duration::from_secs(3600));
        self.state
            .handle_platform_output(window, output.platform_output);
        let mut frame = self.frame.lock().expect("panel frame");
        frame.primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        frame.textures.append(output.textures_delta);
        frame.pixels_per_point = output.pixels_per_point;
    }
}

struct Overlay {
    frame: Arc<Mutex<Frame>>,
    renderer: Option<egui_wgpu::Renderer>,
}

impl NativeOverlay for Overlay {
    fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) {
        let renderer = self.renderer.get_or_insert_with(|| {
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default())
        });
        let mut frame = self.frame.lock().expect("panel frame");
        for (id, delta) in &frame.textures.set {
            renderer.update_texture(device, queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: frame.pixels_per_point,
        };
        let buffers = renderer.update_buffers(device, queue, encoder, &frame.primitives, &screen);
        queue.submit(buffers);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Example egui panel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &frame.primitives, &screen);
        }
        for id in &frame.textures.free {
            renderer.free_texture(id);
        }
        frame.textures = Default::default();
    }
}
