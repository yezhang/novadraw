use std::sync::Arc;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct VelloDemo {
    window: Option<Arc<Window>>,
    render_context: Option<RenderContext>,
    renderer: Option<Renderer>,
    surface: Option<RenderSurface<'static>>,
    scene: vello::Scene,
}

impl VelloDemo {
    fn new() -> Self {
        Self {
            window: None,
            render_context: None,
            renderer: None,
            surface: None,
            scene: vello::Scene::new(),
        }
    }

    fn create_surface(&mut self, width: u32, height: u32) {
        let window = self.window.as_ref().unwrap();

        let surface_future = self.render_context.as_mut().unwrap().create_surface(
            window.clone(),
            width,
            height,
            vello::wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Failed to create surface");

        // Create renderer for this device
        let dev_id = surface.dev_id;
        if self.renderer.is_none() {
            let renderer = Renderer::new(
                &self.render_context.as_ref().unwrap().devices[dev_id].device,
                RendererOptions::default(),
            )
            .expect("Couldn't create renderer");
            self.renderer = Some(renderer);
        }

        self.surface = Some(surface);
    }

    fn render(&mut self) {
        let Some(surface) = &self.surface else {
            return;
        };

        let width = surface.config.width;
        let height = surface.config.height;

        // Build scene with rectangles
        self.scene.reset();

        // Blue rectangle
        let blue = Color::new([0.0, 0.478, 1.0, 1.0]);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            blue,
            None,
            &Rect::new(100.0, 100.0, 300.0, 200.0),
        );

        // Orange rectangle
        let orange = Color::new([1.0, 0.478, 0.0, 1.0]);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            orange,
            None,
            &Rect::new(200.0, 150.0, 400.0, 250.0),
        );

        let surface_status = {
            let (Some(render_context), Some(renderer), Some(surface)) =
                (&self.render_context, self.renderer.as_mut(), &self.surface)
            else {
                return;
            };
            let device_handle = &render_context.devices[surface.dev_id];
            let base_color = Color::new([0.933, 0.933, 0.933, 1.0]);
            renderer
                .render_to_texture(
                    &device_handle.device,
                    &device_handle.queue,
                    &self.scene,
                    &surface.target_view,
                    &vello::RenderParams {
                        base_color,
                        width,
                        height,
                        antialiasing_method: AaConfig::Msaa16,
                    },
                )
                .expect("Failed to render to texture");
            surface.surface.get_current_texture()
        };

        let (surface_texture, reconfigure_after_present) = match surface_status {
            vello::wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            vello::wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            vello::wgpu::CurrentSurfaceTexture::Outdated
            | vello::wgpu::CurrentSurfaceTexture::Lost => {
                self.create_surface(width, height);
                return;
            }
            vello::wgpu::CurrentSurfaceTexture::Timeout
            | vello::wgpu::CurrentSurfaceTexture::Occluded
            | vello::wgpu::CurrentSurfaceTexture::Validation => return,
        };

        let render_context = self.render_context.as_ref().unwrap();
        let surface = self.surface.as_ref().unwrap();
        let device_handle = &render_context.devices[surface.dev_id];

        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                    label: Some("Surface Blit"),
                });

        surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &surface.target_view,
            &surface_texture
                .texture
                .create_view(&vello::wgpu::TextureViewDescriptor::default()),
        );

        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
        if reconfigure_after_present {
            render_context.configure_surface(surface);
        }
    }
}

impl ApplicationHandler for VelloDemo {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Vello Resize Demo"))
                .unwrap(),
        );
        self.window = Some(window.clone());

        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let width = (size.width as f64 * scale_factor) as u32;
        let height = (size.height as f64 * scale_factor) as u32;

        let render_context = RenderContext::new();
        self.render_context = Some(render_context);
        self.create_surface(width, height);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(new_size) => {
                let scale_factor = self.window.as_ref().unwrap().scale_factor();
                let width = (new_size.width as f64 * scale_factor) as u32;
                let height = (new_size.height as f64 * scale_factor) as u32;

                // Recreate surface on resize
                self.create_surface(width, height);
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        _event: DeviceEvent,
    ) {
        // Handle device events if needed
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloDemo::new();

    event_loop.set_control_flow(ControlFlow::Poll);

    let _ = event_loop.run_app(&mut app);
}
