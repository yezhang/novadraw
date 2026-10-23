use anyhow::{Context, Result, bail};
use draw2d_official_examples::inherit_background::{
    CanvasScene, GRADIENT, GradientBackground, LIGHT_GREEN, ORANGE, ParentGradient,
};
use novadraw::{
    geometry::{Affine2D, Rectangle},
    graphics::Color,
    host::{
        BackgroundFrameInput, BackgroundInputVersions, HostBackgroundId, HostBackgroundRegistry,
        PlatformHost,
    },
    render::{RenderBackend, RenderDisposition},
    runtime::SubmissionAcknowledgement,
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use novadraw_platform_winit::canvas::{WinitCanvasCoordinator, WinitCanvasHost};
use std::{path::Path, sync::Arc};
use vello::{
    util::{RenderContext, RenderSurface},
    wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
use winit_canvas::{
    geometry::{CanvasRect, WindowMetrics},
    gpu::{CANVAS_FORMAT, CanvasCompositor, CanvasLayer, CanvasTarget},
    host::{CanvasEvent, CanvasId, Delivery},
};

const MARGIN: f64 = 24.0;
const TOP: f64 = 56.0;
const STEP: f64 = 24.0;
const WIDTH: f64 = 1100.0;
const HEIGHT: f64 = 800.0;
const TITLE: &str = "Paints Background | Inherits Background — wheel: scroll | Tab: focus | arrows: move Canvas | B: background | X: remove | R: repair";

struct Live {
    window: Arc<Window>,
    context: RenderContext,
    surface: Option<RenderSurface<'static>>,
    coordinator: WinitCanvasCoordinator,
    hosts: [WinitCanvasHost; 2],
    ids: [CanvasId; 2],
    scenes: [CanvasScene; 2],
    renderers: [VelloTextureRenderer; 2],
    targets: [Option<CanvasTarget>; 2],
    output: Option<CanvasTarget>,
    compositor: CanvasCompositor,
    registry: HostBackgroundRegistry,
    source: HostBackgroundId,
    inputs: [Option<BackgroundFrameInput>; 2],
    versions: [BackgroundInputVersions; 2],
    revisions: [u64; 2],
    midpoint: Color,
    background_enabled: bool,
    background_dirty: bool,
}
impl Live {
    fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let mut context = RenderContext::new();
        let surface = pollster::block_on(context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        ))?;
        let gpu = &context.devices[surface.dev_id];
        let mut coordinator = WinitCanvasCoordinator::new(window.clone())?;
        let rects = layout(coordinator.canvases().metrics())?;
        let hosts = [coordinator.insert(rects[0])?, coordinator.insert(rects[1])?];
        let ids = [hosts[0].canvas_id(), hosts[1].canvas_id()];
        let scenes = [
            CanvasScene::new(true, rects[0].width(), rects[0].height()),
            CanvasScene::new(false, rects[1].width(), rects[1].height()),
        ];
        let renderers = [
            VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
            VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
        ];
        let compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
        let mut registry = HostBackgroundRegistry::default();
        registry.register(GRADIENT, ParentGradient)?;
        let mut live = Self {
            window,
            context,
            surface: Some(surface),
            coordinator,
            hosts,
            ids,
            scenes,
            renderers,
            targets: [None, None],
            output: None,
            compositor,
            registry,
            source: HostBackgroundId::new(),
            inputs: [None, None],
            versions: [BackgroundInputVersions {
                surface: 1,
                position: 1,
                dpi: 1,
            }; 2],
            revisions: [0; 2],
            midpoint: LIGHT_GREEN,
            background_enabled: true,
            background_dirty: true,
        };
        live.allocate()?;
        Ok(live)
    }
    fn allocate(&mut self) -> Result<()> {
        let surface = self.surface.as_ref().context("suspended")?;
        let device = &self.context.devices[surface.dev_id].device;
        let metrics = self.coordinator.canvases().metrics();
        for index in 0..2 {
            let rect = self.coordinator.canvases().canvas(&self.ids[index])?.rect();
            let size = metrics.texture_size(rect)?;
            if self.targets[index].as_ref().map(CanvasTarget::size)
                != Some([size.width, size.height])
            {
                self.targets[index] = CanvasTarget::new(device, rect, metrics)?;
                self.hosts[index].request_redraw();
            }
        }
        let pixels = metrics.pixels();
        self.output = CanvasTarget::new(
            device,
            CanvasRect::new(
                0.0,
                0.0,
                f64::from(pixels.width) / metrics.scale_factor(),
                f64::from(pixels.height) / metrics.scale_factor(),
            )?,
            metrics,
        )?;
        Ok(())
    }
    fn deliver(&mut self, deliveries: Vec<Delivery>) -> Result<()> {
        for delivery in deliveries {
            let Some(index) = self.ids.iter().position(|id| id == &delivery.canvas) else {
                continue;
            };
            match delivery.event {
                CanvasEvent::Geometry { rect, .. } => {
                    self.scenes[index].resize(rect.width(), rect.height())?;
                    self.hosts[index].request_redraw();
                }
                CanvasEvent::Wheel { delta, .. } => {
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            (f64::from(x) * STEP, f64::from(y) * STEP)
                        }
                        MouseScrollDelta::PixelDelta(p) => (p.x, p.y),
                    };
                    let scene = &mut self.scenes[index];
                    scene
                        .runtime
                        .viewports()
                        .scroll_by(scene.viewport, -dx, -dy)?;
                    self.hosts[index].request_redraw();
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn resize(&mut self) -> Result<()> {
        let metrics = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        let events = self
            .coordinator
            .update_canvases(|host| host.set_metrics(metrics))?;
        self.deliver(events)?;
        for (index, rect) in layout(metrics)?.into_iter().enumerate() {
            let events = self
                .coordinator
                .update_canvases(|host| host.set_rect(&self.ids[index], rect))?;
            self.deliver(events)?;
        }
        let pixels = metrics.pixels();
        if pixels.width != 0
            && pixels.height != 0
            && let Some(surface) = &mut self.surface
        {
            self.context
                .resize_surface(surface, pixels.width, pixels.height);
            self.allocate()?;
        }
        self.background_dirty = true;
        Ok(())
    }
    fn background(&mut self) -> Result<()> {
        let metrics = self.coordinator.canvases().metrics();
        let payload = GradientBackground {
            coverage: Rectangle::new(
                0.0,
                0.0,
                f64::from(metrics.pixels().width) / metrics.scale_factor(),
                f64::from(metrics.pixels().height) / metrics.scale_factor(),
            ),
            midpoint: self.midpoint,
        };
        for index in 0..2 {
            let rect = self.coordinator.canvases().canvas(&self.ids[index])?.rect();
            let surface = self.hosts[index].surface_info();
            let transform = Affine2D::from_translation(-rect.origin().x, -rect.origin().y);
            let old = self.inputs[index];
            if let Some(old) = old {
                if old.surface != surface {
                    increment(&mut self.versions[index].surface)?;
                }
                if old.surface.scale_factor != surface.scale_factor {
                    increment(&mut self.versions[index].dpi)?;
                }
                if old.host_to_canvas != transform {
                    increment(&mut self.versions[index].position)?;
                }
            }
            let input = BackgroundFrameInput {
                surface,
                versions: self.versions[index],
                host_to_canvas: transform,
            };
            if old != Some(input) || self.background_dirty {
                let runtime = &mut self.scenes[index].runtime;
                if self.background_enabled {
                    increment(&mut self.revisions[index])?;
                    let request =
                        runtime
                            .frames()
                            .begin_background(self.source, input, Color::WHITE)?;
                    let snapshot =
                        self.registry
                            .provide(GRADIENT, &payload, request, self.revisions[index]);
                    runtime.frames().complete_background(request, snapshot)?;
                } else {
                    runtime.frames().set_background_input(input)?;
                    runtime.frames().remove_background(Color::WHITE)?;
                }
                self.inputs[index] = Some(input);
                self.hosts[index].request_redraw();
            }
        }
        self.background_dirty = false;
        Ok(())
    }
    fn repair(&mut self) -> Result<()> {
        for (index, scene) in self.scenes.iter_mut().enumerate() {
            scene.runtime.frames().reset_backend_session()?;
            self.hosts[index].request_redraw();
        }
        Ok(())
    }
    fn render(&mut self, present: bool) -> Result<()> {
        let metrics = self.coordinator.canvases().metrics();
        if self.surface.is_none() || metrics.pixels().width == 0 || metrics.pixels().height == 0 {
            return Ok(());
        }
        self.background()?;
        self.coordinator.synchronize()?;
        let surface = self.surface.as_ref().context("suspended")?;
        let gpu = &self.context.devices[surface.dev_id];
        for index in 0..2 {
            let Some(target) = &self.targets[index] else {
                continue;
            };
            let Some(token) = self
                .coordinator
                .canvases()
                .canvas(&self.ids[index])?
                .redraw_token()
            else {
                continue;
            };
            let mut backend = self.renderers[index].bind(target.texture());
            let runtime = &mut self.scenes[index].runtime;
            runtime.frames().request_full_redraw();
            let frame = runtime
                .frames()
                .prepare_submission(self.hosts[index].surface_info(), backend.capabilities())
                .into_ready()
                .map_err(|error| anyhow::anyhow!("Canvas frame preparation: {error:?}"))?;
            let result = backend.submit(&frame);
            let ack = runtime.frames().complete_submission(
                frame.session_id,
                frame.frame_id,
                result.clone(),
            )?;
            if ack != SubmissionAcknowledgement::Presented
                || result? != RenderDisposition::Presented
            {
                bail!("Canvas {index} requires recovery: {ack:?}");
            }
            self.coordinator
                .acknowledge_canvas(&self.ids[index], token)?;
        }
        let output = self.output.as_ref().context("empty output")?;
        let token = self.coordinator.canvases().composite_token();
        let layers: Vec<_> = self
            .targets
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                Some(CanvasLayer {
                    target: target.as_ref()?,
                    rect: self
                        .coordinator
                        .canvases()
                        .canvas(&self.ids[index])
                        .ok()?
                        .rect(),
                })
            })
            .collect();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.compositor.compose(
            &gpu.device,
            &mut encoder,
            output.view(),
            metrics,
            wgpu::Color::WHITE,
            &layers,
        )?;
        gpu.queue.submit([encoder.finish()]);
        if present {
            let (frame, reconfigure) = match surface.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    self.context.configure_surface(surface);
                    self.window.request_redraw();
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => bail!("surface validation"),
            };
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            surface.blitter.copy(
                &gpu.device,
                &mut encoder,
                output.view(),
                &frame.texture.create_view(&Default::default()),
            );
            gpu.queue.submit([encoder.finish()]);
            self.window.pre_present_notify();
            frame.present();
            if reconfigure {
                self.context.configure_surface(surface);
                self.window.request_redraw();
            }
        }
        self.coordinator.acknowledge_composite(token)?;
        Ok(())
    }
    fn key(&mut self, key: &Key) -> Result<()> {
        match key {
            Key::Named(NamedKey::Tab) => {
                let index = usize::from(
                    self.coordinator.canvases().focused().as_ref() == Some(&self.ids[0]),
                );
                let events = self
                    .coordinator
                    .update_canvases(|host| host.set_focus(Some(&self.ids[index])))?;
                self.deliver(events)?;
            }
            Key::Character(s) if s == "b" => {
                self.midpoint = if self.midpoint == LIGHT_GREEN {
                    Color::rgba8(192, 208, 255, 255)
                } else {
                    LIGHT_GREEN
                };
                self.background_enabled = true;
                self.background_dirty = true;
            }
            Key::Character(s) if s == "x" => {
                self.background_enabled = !self.background_enabled;
                self.background_dirty = true;
            }
            Key::Character(s) if s == "r" => self.repair()?,
            _ => {
                let (dx, dy) = match key {
                    Key::Named(NamedKey::ArrowLeft) => (-STEP, 0.0),
                    Key::Named(NamedKey::ArrowRight) => (STEP, 0.0),
                    Key::Named(NamedKey::ArrowUp) => (0.0, -STEP),
                    Key::Named(NamedKey::ArrowDown) => (0.0, STEP),
                    _ => return Ok(()),
                };
                let id = self
                    .coordinator
                    .canvases()
                    .focused()
                    .unwrap_or_else(|| self.ids[1].clone());
                let rect = self.coordinator.canvases().canvas(&id)?.rect();
                let events = self.coordinator.update_canvases(|host| {
                    host.set_rect(
                        &id,
                        CanvasRect::new(
                            rect.origin().x + dx,
                            rect.origin().y + dy,
                            rect.width(),
                            rect.height(),
                        )?,
                    )
                })?;
                self.deliver(events)?;
            }
        }
        self.window.request_redraw();
        Ok(())
    }
    fn screenshot(&self, path: &Path) -> Result<image::RgbaImage> {
        let gpu = &self.context.devices[self.surface.as_ref().context("suspended")?.dev_id];
        let texture = self.output.as_ref().context("empty output")?.texture();
        let (width, height) = (texture.width(), texture.height());
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InheritBackground readback"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = tx.send(result);
        });
        gpu.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })?;
        rx.recv()??;
        let bytes = buffer.get_mapped_range(..);
        let mut image = image::RgbaImage::new(width, height);
        for (src, dst) in bytes
            .chunks_exact(row as usize)
            .zip(image.as_mut().chunks_exact_mut(width as usize * 4))
        {
            dst.copy_from_slice(&src[..width as usize * 4]);
        }
        drop(bytes);
        buffer.unmap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        image.save(path)?;
        Ok(image)
    }
}
fn increment(value: &mut u64) -> Result<()> {
    *value = value.checked_add(1).context("Host version exhausted")?;
    Ok(())
}
fn layout(metrics: WindowMetrics) -> Result<[CanvasRect; 2]> {
    let width = f64::from(metrics.pixels().width) / metrics.scale_factor();
    let height = f64::from(metrics.pixels().height) / metrics.scale_factor();
    let w = ((width - MARGIN * 3.0) / 2.0).max(0.0);
    let h = (height - TOP - MARGIN).max(0.0);
    Ok([
        CanvasRect::new(MARGIN, TOP, w, h)?,
        CanvasRect::new(MARGIN * 2.0 + w, TOP, w, h)?,
    ])
}

#[derive(Default)]
struct App {
    live: Option<Live>,
    verify: bool,
    error: Option<anyhow::Error>,
}
impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        self.error = Some(error);
        event_loop.exit();
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| -> Result<()> {
            if let Some(live) = &mut self.live {
                let size = live.window.inner_size();
                live.surface = Some(pollster::block_on(live.context.create_surface(
                    live.window.clone(),
                    size.width.max(1),
                    size.height.max(1),
                    wgpu::PresentMode::AutoVsync,
                ))?);
                let gpu = &live.context.devices[live.surface.as_ref().context("surface")?.dev_id];
                live.renderers = [
                    VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
                    VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
                ];
                live.compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
                live.targets = [None, None];
                let events = live
                    .coordinator
                    .resume(WindowMetrics::new(size, live.window.scale_factor())?)?;
                live.deliver(events)?;
                live.repair()?;
                live.resize()?;
                live.window.request_redraw();
            } else {
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title(TITLE)
                            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT)),
                    )?,
                );
                let live = Live::new(window)?;
                live.window.request_redraw();
                self.live = Some(live);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(live) = &mut self.live {
            let result = live
                .coordinator
                .suspend()
                .map_err(anyhow::Error::from)
                .and_then(|events| live.deliver(events));
            live.surface = None;
            live.targets = [None, None];
            live.output = None;
            if let Err(error) = result {
                self.fail(event_loop, error);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(live) = &mut self.live else { return };
        if id != live.window.id() {
            return;
        }
        let result = (|| -> Result<()> {
            if event == WindowEvent::CloseRequested {
                event_loop.exit();
                return Ok(());
            }
            if event == WindowEvent::RedrawRequested {
                live.render(true)?;
                if std::mem::take(&mut self.verify) {
                    verify(live)?;
                    event_loop.exit();
                }
            } else {
                let events = live.coordinator.handle_window_event(id, &event)?;
                live.deliver(events)?;
                match event {
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        live.resize()?
                    }
                    WindowEvent::KeyboardInput { event, .. }
                        if event.state == ElementState::Pressed =>
                    {
                        live.key(&event.logical_key)?
                    }
                    _ => {}
                }
            }
            live.coordinator.synchronize()?;
            if !event_loop.exiting() && live.coordinator.canvases().needs_redraw() {
                live.window.request_redraw();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
}
fn verify(live: &mut Live) -> Result<()> {
    let directory = Path::new("target/verification/k07-background");
    let initial = live.screenshot(&directory.join("initial.png"))?;
    let scale = live.coordinator.canvases().metrics().scale_factor();
    let sample = |image: &image::RgbaImage, rect: CanvasRect| {
        *image.get_pixel(
            ((rect.origin().x + rect.width() - STEP) * scale) as u32,
            ((rect.origin().y + STEP) * scale) as u32,
        )
    };
    let left = live.coordinator.canvases().canvas(&live.ids[0])?.rect();
    let right = live.coordinator.canvases().canvas(&live.ids[1])?.rect();
    if sample(&initial, left).0 != [255, 165, 0, 255]
        || sample(&initial, right)[1] < 240
        || sample(&initial, right)[0] == 255
    {
        bail!(
            "opaque/transparent background pixels differ from the official contract: {:?}",
            ORANGE
        );
    }
    let input = live.inputs[1];
    live.scenes[1].runtime.frames().stabilize_for_query()?;
    let viewport = live.scenes[1].viewport;
    live.scenes[1]
        .runtime
        .viewports()
        .set_view_location(viewport, 0.0, STEP)?;
    live.hosts[1].request_redraw();
    live.render(false)?;
    let scrolled = live.screenshot(&directory.join("scrolled.png"))?;
    if sample(&initial, right) != sample(&scrolled, right)
        || initial == scrolled
        || input != live.inputs[1]
    {
        bail!("content scroll moved the Host background or failed to move text");
    }
    live.key(&Key::Named(NamedKey::ArrowDown))?;
    live.render(false)?;
    let moved_rect = live.coordinator.canvases().canvas(&live.ids[1])?.rect();
    let moved = live.screenshot(&directory.join("moved.png"))?;
    if sample(&moved, moved_rect) == sample(&scrolled, right)
        || live.inputs[1].unwrap().versions.position == input.unwrap().versions.position
    {
        bail!("Canvas movement did not resample the Host background");
    }
    let old_origin = live.scenes[1]
        .runtime
        .viewports()
        .handle(viewport)?
        .view_location();
    live.key(&Key::Character("b".into()))?;
    live.render(false)?;
    let changed = live.screenshot(&directory.join("background-only.png"))?;
    if changed == moved
        || live.scenes[1]
            .runtime
            .viewports()
            .handle(viewport)?
            .view_location()
            != old_origin
    {
        bail!("Host-only update mutated content or did not repaint");
    }
    live.repair()?;
    live.render(false)?;
    let repaired = live.screenshot(&directory.join("repair.png"))?;
    if changed != repaired {
        bail!("backend reset lost frozen background/content");
    }
    let metrics = live.coordinator.canvases().metrics();
    let events = live.coordinator.suspend()?;
    live.deliver(events)?;
    let events = live.coordinator.resume(metrics)?;
    live.deliver(events)?;
    live.repair()?;
    live.render(false)?;
    if repaired != live.screenshot(&directory.join("resumed.png"))? {
        bail!("Canvas suspend/resume changed accepted content");
    }
    // Keep physical extent fixed and exercise independent logical extent/DPI versions.
    let dpi_version = live.inputs[1].unwrap().versions.dpi;
    let alternate = WindowMetrics::new(metrics.pixels(), scale * 1.25)?;
    let events = live
        .coordinator
        .update_canvases(|host| host.set_metrics(alternate))?;
    live.deliver(events)?;
    live.allocate()?;
    live.render(false)?;
    live.screenshot(&directory.join("dpi.png"))?;
    if live.inputs[1].unwrap().versions.dpi <= dpi_version {
        bail!("DPI change did not freeze a new input version");
    }
    let events = live
        .coordinator
        .update_canvases(|host| host.set_metrics(metrics))?;
    live.deliver(events)?;
    live.allocate()?;
    live.render(false)?;
    if repaired != live.screenshot(&directory.join("dpi-restored.png"))? {
        bail!("DPI restoration did not repair the complete Canvas");
    }
    live.key(&Key::Character("x".into()))?;
    live.render(false)?;
    let removed = live.screenshot(&directory.join("removed.png"))?;
    if sample(&removed, moved_rect).0 != [255, 255, 255, 255]
        || sample(&removed, left) != sample(&initial, left)
    {
        bail!("background removal did not erase old pixels");
    }
    std::fs::write(
        directory.join("result.txt"),
        format!(
            "PASS\nTwo independent Canvas Runtimes\nNative DPI: {scale}\nSynthetic DPI: {}\nOpaque/transparent: passed\nContent scroll: passed\nCanvas movement: passed\nHost-only change: passed\nBackend session repair: passed\nSuspend/resume: passed\nDPI repair/restoration: passed\nBackground removal: passed\n",
            alternate.scale_factor()
        ),
    )?;
    println!(
        "InheritBackground verification passed: {}",
        directory.display()
    );
    Ok(())
}
fn main() -> Result<()> {
    let mut app = App {
        verify: std::env::args().any(|arg| arg == "--verify"),
        ..Default::default()
    };
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
