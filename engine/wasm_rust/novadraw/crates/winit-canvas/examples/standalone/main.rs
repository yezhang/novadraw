//! A standalone winit-canvas consumer: no Novadraw, platform adapter or Vello.
mod paint;
mod readback;

use paint::Painter;
use std::{error::Error, io, path::Path, sync::Arc};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
use winit_canvas::{
    geometry::{CanvasRect, WindowMetrics},
    gpu::presentation::{SurfacePresentation, configure_surface},
    gpu::{CANVAS_FORMAT, CanvasCompositor, CanvasLayer, CanvasTarget},
    host::{CanvasEvent, CanvasHost, CanvasId, Delivery},
};

type AppResult<T = ()> = Result<T, Box<dyn Error>>;
const COUNT: usize = 2;
const GAP: f64 = 24.0;
const TOP: f64 = 40.0;
const STEP: f64 = 12.0;
const START: LogicalPosition<f64> = LogicalPosition::new(72.0, 72.0);
const SHAPE: [f64; 2] = [132.0, 92.0];
const INKS: [[f32; 4]; COUNT] = [[0.96, 0.34, 0.10, 1.0], [0.05, 0.48, 0.84, 1.0]];
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.05,
    b: 0.08,
    a: 1.0,
};
const TITLE: &str =
    "Standalone Canvas | drag | Tab focus | arrows move shape | H hide right | R reset";

fn failure(message: &str) -> Box<dyn Error> {
    io::Error::other(message).into()
}

struct Gpu {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    painter: Painter,
    compositor: CanvasCompositor,
    blitter: wgpu::util::TextureBlitter,
    targets: [Option<CanvasTarget>; COUNT],
    output: Option<CanvasTarget>,
    pending_resize_present: bool,
}

impl Gpu {
    fn new(window: Arc<Window>) -> AppResult<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| failure("surface has no supported configuration"))?;
        surface.configure(&device, &config);
        configure_surface(&surface, BACKGROUND);
        let painter = Painter::new(&device);
        let compositor = CanvasCompositor::new(&device, CANVAS_FORMAT);
        let blitter = wgpu::util::TextureBlitter::new(&device, config.format);
        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            painter,
            compositor,
            blitter,
            targets: [None, None],
            output: None,
            pending_resize_present: true,
        })
    }

    fn resize(&mut self, metrics: WindowMetrics, rects: [CanvasRect; COUNT]) -> AppResult {
        // Retain the marker across failed/occluded acquisition until a drawable
        // with the new window geometry has actually been presented.
        self.pending_resize_present = true;
        let pixels = metrics.pixels();
        if pixels.width == 0 || pixels.height == 0 {
            self.targets = [None, None];
            self.output = None;
            return Ok(());
        }
        if self.config.width != pixels.width || self.config.height != pixels.height {
            self.config.width = pixels.width;
            self.config.height = pixels.height;
            self.surface.configure(&self.device, &self.config);
        }
        for (index, rect) in rects.into_iter().enumerate() {
            let size = metrics.texture_size(rect)?;
            if self.targets[index].as_ref().map(CanvasTarget::size)
                != Some([size.width, size.height])
            {
                self.targets[index] = CanvasTarget::new(&self.device, rect, metrics)?;
            }
        }
        if self.output.as_ref().map(CanvasTarget::size) != Some([pixels.width, pixels.height]) {
            self.output = CanvasTarget::new(
                &self.device,
                CanvasRect::new(
                    0.0,
                    0.0,
                    f64::from(pixels.width) / metrics.scale_factor(),
                    f64::from(pixels.height) / metrics.scale_factor(),
                )?,
                metrics,
            )?;
        }
        Ok(())
    }
}

struct Demo {
    window: Arc<Window>,
    host: CanvasHost,
    ids: [CanvasId; COUNT],
    gpu: Option<Gpu>,
    positions: [LogicalPosition<f64>; COUNT],
    drag_offsets: [Option<LogicalPosition<f64>>; COUNT],
    renders: [u32; COUNT],
}

impl Demo {
    fn new(window: Arc<Window>) -> AppResult<Self> {
        let metrics = WindowMetrics::new(window.inner_size(), window.scale_factor())?;
        let mut host = CanvasHost::new(window.id(), metrics);
        let rects = layout(metrics)?;
        let ids = [host.insert(rects[0])?, host.insert(rects[1])?];
        let mut gpu = Gpu::new(window.clone())?;
        gpu.resize(metrics, rects)?;
        Ok(Self {
            window,
            host,
            ids,
            gpu: Some(gpu),
            positions: [START; COUNT],
            drag_offsets: [None, None],
            renders: [0; COUNT],
        })
    }

    fn dispatch(&mut self, event: &WindowEvent) -> AppResult {
        let deliveries = self.host.handle_window_event(self.window.id(), event)?;
        self.deliver(deliveries)
    }

    fn deliver(&mut self, deliveries: Vec<Delivery>) -> AppResult {
        for delivery in deliveries {
            let Some(index) = self.ids.iter().position(|id| *id == delivery.canvas) else {
                continue;
            };
            match delivery.event {
                CanvasEvent::MouseInput {
                    position,
                    button: MouseButton::Left,
                    state,
                } => {
                    let origin = self.positions[index];
                    if state == ElementState::Released {
                        self.drag_offsets[index] = None;
                    } else if position.x >= origin.x
                        && position.y >= origin.y
                        && position.x < origin.x + SHAPE[0]
                        && position.y < origin.y + SHAPE[1]
                    {
                        self.drag_offsets[index] = Some(LogicalPosition::new(
                            position.x - origin.x,
                            position.y - origin.y,
                        ));
                    }
                }
                CanvasEvent::PointerMoved { position } => {
                    if let Some(offset) = self.drag_offsets[index] {
                        self.positions[index] =
                            LogicalPosition::new(position.x - offset.x, position.y - offset.y);
                        self.host.request_redraw(&self.ids[index])?;
                    }
                    self.window.set_title(&format!(
                        "{TITLE} | Canvas {}: ({:.0}, {:.0})",
                        index + 1,
                        position.x,
                        position.y
                    ));
                }
                CanvasEvent::Keyboard(event) if event.state == ElementState::Pressed => {
                    let delta = match event.logical_key {
                        Key::Named(NamedKey::ArrowLeft) => (-STEP, 0.0),
                        Key::Named(NamedKey::ArrowRight) => (STEP, 0.0),
                        Key::Named(NamedKey::ArrowUp) => (0.0, -STEP),
                        Key::Named(NamedKey::ArrowDown) => (0.0, STEP),
                        _ => continue,
                    };
                    self.positions[index].x += delta.0;
                    self.positions[index].y += delta.1;
                    self.host.request_redraw(&self.ids[index])?;
                }
                CanvasEvent::Focused(_) => self.host.request_redraw(&self.ids[index])?,
                CanvasEvent::Cancelled | CanvasEvent::Suspended => self.drag_offsets[index] = None,
                _ => {}
            }
        }
        Ok(())
    }

    fn set_focus(&mut self, index: usize) -> AppResult {
        let events = self.host.set_focus(Some(&self.ids[index]))?;
        self.deliver(events)
    }

    fn toggle_right(&mut self) -> AppResult {
        let visible = !self.host.canvas(&self.ids[1])?.visible();
        let events = self
            .host
            .set_available(&self.ids[1].clone(), visible, visible)?;
        self.deliver(events)
    }

    /// Window shortcuts are application UI; arrow keys go through CanvasEvent::Keyboard.
    fn shortcut(&mut self, event: &WindowEvent) -> AppResult<bool> {
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return Ok(false);
        };
        if event.state != ElementState::Pressed {
            return Ok(false);
        }
        match &event.logical_key {
            Key::Named(NamedKey::Tab) => {
                let right_available = self.host.canvas(&self.ids[1])?.visible();
                let index = usize::from(
                    right_available && self.host.focused().as_ref() == Some(&self.ids[0]),
                );
                self.set_focus(index)?;
            }
            Key::Character(key) if key.eq_ignore_ascii_case("h") => self.toggle_right()?,
            Key::Character(key) if key.eq_ignore_ascii_case("r") => {
                self.positions = [START; COUNT];
                self.drag_offsets = [None, None];
                let events = self.host.set_available(&self.ids[1].clone(), true, true)?;
                self.deliver(events)?;
                for id in &self.ids {
                    self.host.request_redraw(id)?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn resize(&mut self) -> AppResult {
        let metrics = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        let events = self.host.set_metrics(metrics)?;
        self.deliver(events)?;
        let rects = layout(metrics)?;
        for (index, rect) in rects.into_iter().enumerate() {
            let events = self.host.set_rect(&self.ids[index].clone(), rect)?;
            self.deliver(events)?;
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(metrics, rects)?;
        }
        Ok(())
    }

    fn render(&mut self, present: bool) -> AppResult {
        // A queued redraw can arrive after the native size changed but before
        // its Resized event. Never acquire a drawable for stale geometry.
        let current = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        if current != self.host.metrics() {
            self.resize()?;
        }
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let Some(output) = &gpu.output else {
            return Ok(());
        };
        let metrics = self.host.metrics();
        let token = self.host.composite_token();
        let mut painted = Vec::new();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        for (index, target) in gpu.targets.iter().enumerate() {
            let Some(target) = target else { continue };
            let canvas = self.host.canvas(&self.ids[index])?;
            if !canvas.visible() {
                continue;
            }
            let Some(token) = canvas.redraw_token() else {
                continue;
            };
            let rect = canvas.rect();
            let p = self.positions[index];
            let ink = INKS[index];
            gpu.painter.paint(
                &gpu.device,
                &mut encoder,
                target,
                [
                    rect.width() as f32,
                    rect.height() as f32,
                    metrics.scale_factor() as f32,
                    f32::from(self.host.focused().as_ref() == Some(&self.ids[index])),
                    p.x as f32,
                    p.y as f32,
                    SHAPE[0] as f32,
                    SHAPE[1] as f32,
                    ink[0],
                    ink[1],
                    ink[2],
                    ink[3],
                ],
            );
            painted.push((index, token));
        }
        let mut layers = Vec::new();
        for (index, target) in gpu.targets.iter().enumerate() {
            let canvas = self.host.canvas(&self.ids[index])?;
            if canvas.visible()
                && let Some(target) = target
            {
                layers.push(CanvasLayer {
                    target,
                    rect: canvas.rect(),
                });
            }
        }
        gpu.compositor.compose(
            &gpu.device,
            &mut encoder,
            output.view(),
            metrics,
            BACKGROUND,
            &layers,
        )?;
        gpu.queue.submit([encoder.finish()]);
        for (index, token) in painted {
            self.host.acknowledge_canvas(&self.ids[index], token)?;
            self.renders[index] += 1;
        }
        if present {
            let presentation = SurfacePresentation::begin(&gpu.surface, gpu.pending_resize_present);
            let (frame, reconfigure) = match gpu.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Lost => {
                    drop(presentation);
                    gpu.surface = gpu.instance.create_surface(self.window.clone())?;
                    gpu.surface.configure(&gpu.device, &gpu.config);
                    configure_surface(&gpu.surface, BACKGROUND);
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    gpu.surface.configure(&gpu.device, &gpu.config);
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(failure("surface validation failed"));
                }
            };
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            gpu.blitter.copy(
                &gpu.device,
                &mut encoder,
                output.view(),
                &frame.texture.create_view(&Default::default()),
            );
            gpu.queue.submit([encoder.finish()]);
            self.window.pre_present_notify();
            frame.present();
            drop(presentation);
            gpu.pending_resize_present = false;
            if reconfigure {
                gpu.surface.configure(&gpu.device, &gpu.config);
            }
        }
        self.host.acknowledge_composite(token)?;
        Ok(())
    }

    fn pixels(&self) -> AppResult<readback::Pixels> {
        let gpu = self
            .gpu
            .as_ref()
            .ok_or_else(|| failure("GPU is suspended"))?;
        let target = gpu
            .output
            .as_ref()
            .ok_or_else(|| failure("window is empty"))?;
        readback::capture(&gpu.device, &gpu.queue, target.texture())
    }
}

fn layout(metrics: WindowMetrics) -> AppResult<[CanvasRect; COUNT]> {
    let pixels = metrics.pixels();
    let width =
        (f64::from(pixels.width) / metrics.scale_factor() - GAP * 3.0).max(0.0) / COUNT as f64;
    let height = (f64::from(pixels.height) / metrics.scale_factor() - TOP - GAP).max(0.0);
    Ok([
        CanvasRect::new(GAP, TOP, width, height)?,
        CanvasRect::new(GAP * 2.0 + width, TOP, width, height)?,
    ])
}

#[derive(Default)]
struct App {
    demo: Option<Demo>,
    verify: bool,
    resize_verify: bool,
    resize_step: usize,
    error: Option<Box<dyn Error>>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn Error>) {
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| -> AppResult {
            if let Some(demo) = &mut self.demo {
                if demo.gpu.is_none() {
                    demo.gpu = Some(Gpu::new(demo.window.clone())?);
                    let events = demo.host.resume(WindowMetrics::new(
                        demo.window.inner_size(),
                        demo.window.scale_factor(),
                    )?)?;
                    demo.deliver(events)?;
                    demo.resize()?;
                }
                demo.window.request_redraw();
            } else {
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title(TITLE)
                            .with_inner_size(LogicalSize::new(960.0, 600.0)),
                    )?,
                );
                window.request_redraw();
                self.demo = Some(Demo::new(window)?);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(demo) = &mut self.demo {
            let events = demo.host.suspend();
            let result = demo.deliver(events);
            demo.gpu = None;
            if let Err(error) = result {
                self.fail(event_loop, error);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let Some(demo) = &mut self.demo else { return };
        if id != demo.window.id() {
            return;
        }
        let result = (|| -> AppResult {
            match &event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    demo.render(true)?;
                    if self.resize_verify && verify_resize_frame(demo, self.resize_step)? {
                        self.resize_step += 1;
                        if let Some(size) = RESIZE_SIZES.get(self.resize_step) {
                            let _ = demo.window.request_inner_size(*size);
                        } else {
                            self.resize_verify = false;
                            demo.pixels()?.save(Path::new(
                                "target/verification/winit-canvas/standalone-resize.png",
                            ))?;
                            std::fs::write(
                                "target/verification/winit-canvas/standalone-resize-result.txt",
                                format!(
                                    "PASS\nNative resize frames: {}\nSurface/Canvas size agreement: passed\nResize transaction reset: passed\n",
                                    self.resize_step
                                ),
                            )?;
                            event_loop.exit();
                        }
                    }
                    if std::mem::take(&mut self.verify) {
                        verify(demo)?;
                        event_loop.exit();
                    }
                }
                _ => {
                    if !demo.shortcut(&event)? {
                        demo.dispatch(&event)?;
                    }
                    if matches!(
                        event,
                        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                    ) {
                        demo.resize()?;
                    }
                }
            }
            if !event_loop.exiting() && demo.host.needs_redraw() {
                demo.window.request_redraw();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
}

// Real Window::request_inner_size -> Resized -> redraw; shrink, expand, repeat.
const RESIZE_SIZES: [LogicalSize<f64>; 7] = [
    LogicalSize::new(960.0, 600.0),
    LogicalSize::new(820.0, 480.0),
    LogicalSize::new(680.0, 400.0),
    LogicalSize::new(1040.0, 680.0),
    LogicalSize::new(760.0, 520.0),
    LogicalSize::new(1120.0, 720.0),
    LogicalSize::new(960.0, 600.0),
];

fn verify_resize_frame(demo: &Demo, step: usize) -> AppResult<bool> {
    let gpu = demo
        .gpu
        .as_ref()
        .ok_or_else(|| failure("GPU is suspended"))?;
    let expected = RESIZE_SIZES[step].to_physical::<u32>(demo.window.scale_factor());
    if demo.window.inner_size() != expected {
        return Ok(false);
    }
    if gpu.pending_resize_present {
        return Ok(false);
    }
    if gpu.config.width != expected.width
        || gpu.config.height != expected.height
        || demo.host.metrics().pixels() != expected
    {
        return Err(failure(
            "resize frame did not present matching window/Canvas geometry",
        ));
    }
    #[cfg(target_os = "macos")]
    {
        let metal = unsafe { gpu.surface.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or_else(|| failure("resize verification requires the Metal surface"))?;
        let layer = metal.render_layer().lock();
        if layer.presentsWithTransaction()
            || !layer.isOpaque()
            || *layer.contentsGravity() != *unsafe { objc2_quartz_core::kCAGravityTopLeft }
        {
            return Err(failure("macOS resize presentation policy was not restored"));
        }
        drop(layer);
        // An abandoned acquisition attempt must also restore normal mode.
        {
            let _guard = SurfacePresentation::begin(&gpu.surface, true);
            if !metal.render_layer().lock().presentsWithTransaction() {
                return Err(failure(
                    "resize transaction was not enabled before acquisition",
                ));
            }
        }
        if metal.render_layer().lock().presentsWithTransaction() {
            return Err(failure("abandoned resize transaction was not reset"));
        }
    }
    Ok(true)
}

/// Runs the real event router and renderer; no test-only paint or coordinate path.
fn verify(demo: &mut Demo) -> AppResult {
    use winit::{dpi::PhysicalPosition, event::DeviceId};
    let directory = Path::new("target/verification/winit-canvas");
    let before = demo.pixels()?;
    demo.dispatch(&WindowEvent::Focused(true))?;
    let motion = |point: LogicalPosition<f64>, scale: f64| WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(point.x * scale, point.y * scale),
    };
    let button = |state| WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state,
        button: MouseButton::Left,
    };
    let scale = demo.host.metrics().scale_factor();
    let right_renders = demo.renders[1];
    for index in 0..COUNT {
        let rect = demo.host.canvas(&demo.ids[index])?.rect();
        let point = rect.local_to_window(LogicalPosition::new(START.x + STEP, START.y + STEP));
        demo.dispatch(&motion(point, scale))?;
        demo.dispatch(&button(ElementState::Pressed))?;
        if index == 0 {
            let right = demo.host.canvas(&demo.ids[1])?.rect().origin();
            demo.dispatch(&motion(
                LogicalPosition::new(right.x + STEP, right.y + STEP),
                scale,
            ))?;
            if demo.host.captured().as_ref() != Some(&demo.ids[0]) || demo.positions[1] != START {
                return Err(failure("cross-canvas capture retargeted the drag"));
            }
        }
        demo.dispatch(&motion(
            LogicalPosition::new(point.x + STEP * 3.0, point.y + STEP * 2.0),
            scale,
        ))?;
        demo.dispatch(&button(ElementState::Released))?;
        demo.render(false)?;
        if demo.positions[index] != LogicalPosition::new(START.x + STEP * 3.0, START.y + STEP * 2.0)
        {
            return Err(failure("DPI or local drag coordinates are incorrect"));
        }
        if index == 0 && demo.renders[1] != right_renders {
            return Err(failure("untouched canvas was repainted"));
        }
    }
    let positions = demo.positions;
    demo.toggle_right()?;
    demo.render(false)?;
    let hidden = demo.pixels()?;
    let right_origin = demo.host.canvas(&demo.ids[1])?.rect().origin();
    let hidden_pixel = hidden.at(
        ((right_origin.x + STEP) * scale) as u32,
        ((right_origin.y + STEP) * scale) as u32,
    );
    if hidden_pixel[..3].iter().any(|channel| *channel > 25) {
        return Err(failure("hidden canvas left pixels in the window"));
    }
    demo.toggle_right()?;
    demo.set_focus(1)?;
    demo.render(false)?;
    let after = demo.pixels()?;
    if before.rgba == after.rgba || demo.positions != positions {
        return Err(failure("canvas redraw or hidden-state isolation failed"));
    }
    for index in 0..COUNT {
        let rect = demo.host.canvas(&demo.ids[index])?.rect();
        let p = rect.local_to_window(LogicalPosition::new(
            demo.positions[index].x + STEP,
            demo.positions[index].y + STEP,
        ));
        let pixel = after.at((p.x * scale) as u32, (p.y * scale) as u32);
        if (index == 0 && pixel[0] < 200) || (index == 1 && pixel[2] < 180) {
            return Err(failure("independent WGPU canvas paint is missing"));
        }
    }
    after.save(&directory.join("standalone.png"))?;
    std::fs::write(
        directory.join("standalone-result.txt"),
        format!(
            "PASS\nNo Novadraw or Vello dependencies\nOne winit Window, two Canvas textures\nDPI: {scale}\nCross-canvas capture: passed\nLocal drag mapping: passed\nUnchanged texture reuse: passed\nHide/show erasure: passed\nNative GPU pixels: passed\nRender counts: {:?}\n",
            demo.renders,
        ),
    )?;
    println!(
        "Standalone Canvas verification passed: {}",
        directory.display()
    );
    Ok(())
}

fn main() -> AppResult {
    let mut app = App::default();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--verify" => app.verify = true,
            "--verify-resize" => app.resize_verify = true,
            "--help" | "-h" => {
                println!(
                    "{TITLE}\n--verify: run GPU/input checks and save a PNG\n--verify-resize: check native resize presentation"
                );
                return Ok(());
            }
            _ => return Err(failure("unknown option; use --help")),
        }
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
