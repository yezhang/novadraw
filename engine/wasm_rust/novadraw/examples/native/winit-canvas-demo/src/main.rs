use anyhow::{Context, Result, bail};
use novadraw::{
    figure::RectangleFigure,
    geometry::Rectangle,
    graphics::Color,
    render::{RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    tree::{FigureId, FigureTree},
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use std::{path::Path, sync::Arc};
use vello::{
    AaConfig, Renderer, RendererOptions,
    kurbo::{Affine, Circle, Rect, Stroke},
    peniko::{Color as VelloColor, Fill},
    util::{RenderContext, RenderSurface},
    wgpu,
};
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
    gpu::{CANVAS_FORMAT, CanvasCompositor, CanvasLayer, CanvasTarget},
    host::{CanvasEvent, CanvasHost, CanvasId, Delivery},
};

const MARGIN: f64 = 24.0;
const TOP: f64 = 56.0;
const STEP: f64 = 12.0;
const SHAPE_WIDTH: f64 = 140.0;
const SHAPE_HEIGHT: f64 = 100.0;
const START: LogicalPosition<f64> = LogicalPosition::new(80.0, 80.0);
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.05,
    b: 0.08,
    a: 1.0,
};
const TITLE: &str =
    "Canvas: left Vello / right Novadraw | drag shapes | Tab focus | arrows move Canvas | R reset";

struct Live {
    window: Arc<Window>,
    context: RenderContext,
    surface: Option<RenderSurface<'static>>,
    host: CanvasHost,
    ids: [CanvasId; 2],
    targets: [Option<CanvasTarget>; 2],
    output: Option<CanvasTarget>,
    compositor: CanvasCompositor,
    vello: Renderer,
    novadraw: VelloTextureRenderer,
    runtime: Runtime,
    root: FigureId,
    shape: FigureId,
    positions: [LogicalPosition<f64>; 2],
    drag: [Option<LogicalPosition<f64>>; 2],
    renders: [u32; 2],
}

impl Live {
    fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let metrics = WindowMetrics::new(size, window.scale_factor())?;
        let mut context = RenderContext::new();
        let surface = pollster::block_on(context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        ))?;
        let gpu = &context.devices[surface.dev_id];
        let mut host = CanvasHost::new(window.id(), metrics);
        let rects = layout(metrics)?;
        let ids = [host.insert(rects[0])?, host.insert(rects[1])?];
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                rects[1].width(),
                rects[1].height(),
                Color::rgba(0.07, 0.13, 0.20, 1.0),
            )))?;
        let shape = tree.builder().add_child(
            root,
            Box::new(RectangleFigure::new_with_color(
                START.x,
                START.y,
                SHAPE_WIDTH,
                SHAPE_HEIGHT,
                Color::rgba(0.15, 0.65, 0.95, 1.0),
            )),
        )?;
        let compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
        let vello = Renderer::new(&gpu.device, RendererOptions::default())?;
        let novadraw = VelloTextureRenderer::new(&gpu.device, &gpu.queue)?;
        let mut live = Self {
            window,
            context,
            surface: Some(surface),
            host,
            ids,
            targets: [None, None],
            output: None,
            compositor,
            vello,
            novadraw,
            runtime: Runtime::new(tree),
            root,
            shape,
            positions: [START; 2],
            drag: [None, None],
            renders: [0; 2],
        };
        live.allocate()?;
        Ok(live)
    }

    fn allocate(&mut self) -> Result<()> {
        let surface = self.surface.as_ref().context("surface is suspended")?;
        let device = &self.context.devices[surface.dev_id].device;
        let metrics = self.host.metrics();
        for index in 0..2 {
            let rect = self.host.canvas(&self.ids[index])?.rect();
            let size = metrics.texture_size(rect)?;
            if self.targets[index].as_ref().map(CanvasTarget::size)
                != Some([size.width, size.height])
            {
                self.targets[index] = CanvasTarget::new(device, rect, metrics)?;
            }
        }
        let pixels = metrics.pixels();
        let scale = metrics.scale_factor();
        self.output = CanvasTarget::new(
            device,
            CanvasRect::new(
                0.0,
                0.0,
                f64::from(pixels.width) / scale,
                f64::from(pixels.height) / scale,
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
                CanvasEvent::Geometry { rect, .. } if index == 1 => {
                    self.runtime.figure(self.root)?.set_bounds(Rectangle::new(
                        0.0,
                        0.0,
                        rect.width(),
                        rect.height(),
                    ))?;
                }
                CanvasEvent::MouseInput {
                    position,
                    button: MouseButton::Left,
                    state,
                } => {
                    if state == ElementState::Pressed {
                        let origin = self.positions[index];
                        if position.x >= origin.x
                            && position.y >= origin.y
                            && position.x < origin.x + SHAPE_WIDTH
                            && position.y < origin.y + SHAPE_HEIGHT
                        {
                            self.drag[index] = Some(LogicalPosition::new(
                                position.x - origin.x,
                                position.y - origin.y,
                            ));
                        }
                    } else {
                        self.drag[index] = None;
                    }
                }
                CanvasEvent::PointerMoved { position } => {
                    if let Some(offset) = self.drag[index] {
                        self.positions[index] =
                            LogicalPosition::new(position.x - offset.x, position.y - offset.y);
                        if index == 1 {
                            self.runtime.figure(self.shape)?.set_bounds(Rectangle::new(
                                self.positions[1].x,
                                self.positions[1].y,
                                SHAPE_WIDTH,
                                SHAPE_HEIGHT,
                            ))?;
                        }
                        self.host.request_redraw(&self.ids[index])?;
                    }
                }
                CanvasEvent::Cancelled | CanvasEvent::Suspended => self.drag[index] = None,
                CanvasEvent::Focused(_) => self.host.request_redraw(&self.ids[index])?,
                _ => {}
            }
        }
        Ok(())
    }

    fn resize(&mut self) -> Result<()> {
        let metrics = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        let events = self.host.set_metrics(metrics)?;
        self.deliver(events)?;
        for (index, rect) in layout(metrics)?.into_iter().enumerate() {
            let events = self.host.set_rect(&self.ids[index].clone(), rect)?;
            self.deliver(events)?;
        }
        let pixels = metrics.pixels();
        if pixels.width == 0 || pixels.height == 0 {
            return Ok(());
        }
        if let Some(surface) = &mut self.surface {
            self.context
                .resize_surface(surface, pixels.width, pixels.height);
            self.allocate()?;
        }
        Ok(())
    }

    fn render(&mut self, present: bool) -> Result<()> {
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let Some(output) = &self.output else {
            return Ok(());
        };
        let metrics = self.host.metrics();
        if metrics.pixels().width == 0 || metrics.pixels().height == 0 {
            return Ok(());
        }
        let gpu = &self.context.devices[surface.dev_id];
        for index in 0..2 {
            let Some(target) = &self.targets[index] else {
                continue;
            };
            let Some(token) = self.host.canvas(&self.ids[index])?.redraw_token() else {
                continue;
            };
            let [width, height] = target.size();
            if index == 0 {
                let mut scene = vello::Scene::new();
                let scale = metrics.scale_factor();
                let transform = Affine::scale(scale);
                let p = self.positions[0];
                scene.fill(
                    Fill::NonZero,
                    transform,
                    VelloColor::new([0.95, 0.40, 0.12, 1.0]),
                    None,
                    &Rect::new(p.x, p.y, p.x + SHAPE_WIDTH, p.y + SHAPE_HEIGHT),
                );
                scene.stroke(
                    &Stroke::new(3.0),
                    transform,
                    VelloColor::new([1.0, 0.76, 0.4, 1.0]),
                    None,
                    &Circle::new((p.x + SHAPE_WIDTH / 2.0, p.y + SHAPE_HEIGHT / 2.0), 28.0),
                );
                self.vello.render_to_texture(
                    &gpu.device,
                    &gpu.queue,
                    &scene,
                    target.view(),
                    &vello::RenderParams {
                        base_color: VelloColor::new([0.12, 0.09, 0.08, 1.0]),
                        width,
                        height,
                        antialiasing_method: AaConfig::Msaa16,
                    },
                )?;
            } else {
                let mut backend = self.novadraw.bind(target.texture());
                self.runtime.frames().request_full_redraw();
                let submission = match self.runtime.frames().prepare_submission(
                    SurfaceInfo {
                        pixel_width: width,
                        pixel_height: height,
                        scale_factor: metrics.scale_factor(),
                        logical_width: f64::from(width) / metrics.scale_factor(),
                        logical_height: f64::from(height) / metrics.scale_factor(),
                    },
                    backend.capabilities(),
                ) {
                    FramePreparation::Ready(submission) => submission,
                    other => bail!("Novadraw preparation failed: {other:?}"),
                };
                let result = backend.submit(&submission);
                let acknowledgement = self.runtime.frames().complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    result.clone(),
                )?;
                if acknowledgement != SubmissionAcknowledgement::Presented {
                    bail!("Novadraw completion requires recovery: {acknowledgement:?}")
                }
                if result? != RenderDisposition::Presented {
                    bail!("Novadraw texture was not submitted")
                }
            }
            self.renders[index] += 1;
            self.host.acknowledge_canvas(&self.ids[index], token)?;
        }
        let token = self.host.composite_token();
        let layers: Vec<_> = self
            .targets
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                let canvas = self.host.canvas(&self.ids[index]).ok()?;
                canvas.visible().then_some(CanvasLayer {
                    target: target.as_ref()?,
                    rect: canvas.rect(),
                })
            })
            .collect();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.compositor.compose(
            &gpu.device,
            &mut encoder,
            output.view(),
            metrics,
            BACKGROUND,
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
                wgpu::CurrentSurfaceTexture::Validation => {
                    bail!("window surface validation failed")
                }
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
            }
        }
        self.host.acknowledge_composite(token)?;
        Ok(())
    }

    fn screenshot(&self, path: &Path) -> Result<image::RgbaImage> {
        let surface = self.surface.as_ref().context("surface is suspended")?;
        let gpu = &self.context.devices[surface.dev_id];
        let texture = self.output.as_ref().context("empty window")?.texture();
        let width = texture.width();
        let height = texture.height();
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Canvas demo screenshot"),
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

    fn key(&mut self, key: &Key) -> Result<()> {
        if *key == Key::Named(NamedKey::Tab) {
            let index = usize::from(self.host.focused().as_ref() == Some(&self.ids[0]));
            let deliveries = self.host.set_focus(Some(&self.ids[index]))?;
            self.deliver(deliveries)?;
        } else if key == &Key::Character("r".into()) {
            self.resize()?;
        } else if let Some(id) = self.host.focused() {
            let (dx, dy) = match key {
                Key::Named(NamedKey::ArrowLeft) => (-STEP, 0.0),
                Key::Named(NamedKey::ArrowRight) => (STEP, 0.0),
                Key::Named(NamedKey::ArrowUp) => (0.0, -STEP),
                Key::Named(NamedKey::ArrowDown) => (0.0, STEP),
                _ => return Ok(()),
            };
            let rect = self.host.canvas(&id)?.rect();
            let deliveries = self.host.set_rect(
                &id,
                CanvasRect::new(
                    rect.origin().x + dx,
                    rect.origin().y + dy,
                    rect.width(),
                    rect.height(),
                )?,
            )?;
            self.deliver(deliveries)?;
        }
        Ok(())
    }
}

fn layout(metrics: WindowMetrics) -> Result<[CanvasRect; 2]> {
    let pixels = metrics.pixels();
    let width = f64::from(pixels.width) / metrics.scale_factor();
    let height = f64::from(pixels.height) / metrics.scale_factor();
    let canvas_width = ((width - MARGIN * 3.0) / 2.0).max(0.0);
    let canvas_height = (height - TOP - MARGIN).max(0.0);
    Ok([
        CanvasRect::new(MARGIN, TOP, canvas_width, canvas_height)?,
        CanvasRect::new(
            MARGIN * 2.0 + canvas_width,
            TOP,
            canvas_width,
            canvas_height,
        )?,
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
                let pixels = live.window.inner_size();
                live.surface = Some(pollster::block_on(live.context.create_surface(
                    live.window.clone(),
                    pixels.width.max(1),
                    pixels.height.max(1),
                    wgpu::PresentMode::AutoVsync,
                ))?);
                // A replacement device requires fresh consumers and targets.
                let gpu = &live.context.devices[live.surface.as_ref().unwrap().dev_id];
                live.vello = Renderer::new(&gpu.device, RendererOptions::default())?;
                live.novadraw = VelloTextureRenderer::new(&gpu.device, &gpu.queue)?;
                live.compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
                live.targets = [None, None];
                let deliveries = live
                    .host
                    .resume(WindowMetrics::new(pixels, live.window.scale_factor())?)?;
                live.deliver(deliveries)?;
                live.runtime.frames().reset_backend_session()?;
                live.resize()?;
                live.window.request_redraw();
            } else {
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title(TITLE)
                            .with_inner_size(LogicalSize::new(960.0, 600.0)),
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
            let deliveries = live.host.suspend();
            let result = live.deliver(deliveries);
            live.surface = None;
            live.output = None;
            live.targets = [None, None];
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
                let deliveries = live.host.handle_window_event(id, &event)?;
                live.deliver(deliveries)?;
                match &event {
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
            if !event_loop.exiting() && live.host.needs_redraw() {
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
    use winit::{dpi::PhysicalPosition, event::DeviceId};
    let directory = Path::new("target/verification/winit-canvas");
    let before = live.screenshot(&directory.join("native-before.png"))?;
    let original = live.positions;
    let original_renders = live.renders;
    for index in 0..2 {
        let rect = live.host.canvas(&live.ids[index])?.rect();
        let scale = live.host.metrics().scale_factor();
        let window_point =
            rect.local_to_window(LogicalPosition::new(START.x + STEP, START.y + STEP));
        let events = [
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(window_point.x * scale, window_point.y * scale),
            },
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(
                    (window_point.x + STEP * 3.0) * scale,
                    (window_point.y + STEP * 2.0) * scale,
                ),
            },
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Released,
                button: MouseButton::Left,
            },
        ];
        for event in events {
            let deliveries = live.host.handle_window_event(live.window.id(), &event)?;
            live.deliver(deliveries)?;
        }
        if live.positions[index] != LogicalPosition::new(START.x + STEP * 3.0, START.y + STEP * 2.0)
            || (index == 0 && live.positions[1] != original[1])
        {
            bail!("canvas input isolation failed")
        }
        live.render(false)?;
        if index == 0 && live.renders[1] != original_renders[1] {
            bail!("unchanged Novadraw canvas rendered again")
        }
    }
    live.key(&Key::Named(NamedKey::ArrowDown))?;
    live.render(false)?;
    let after = live.screenshot(&directory.join("native-after.png"))?;
    if before == after {
        bail!("interactive rendering did not change pixels")
    }
    let scale = live.host.metrics().scale_factor();
    for index in 0..2 {
        let rect = live.host.canvas(&live.ids[index])?.rect();
        let p = rect.local_to_window(LogicalPosition::new(
            live.positions[index].x + STEP,
            live.positions[index].y + STEP,
        ));
        let pixel = after.get_pixel((p.x * scale) as u32, (p.y * scale) as u32);
        if (index == 0 && pixel[0] < 200) || (index == 1 && pixel[2] < 200) {
            bail!("renderer {index} did not produce its expected color: {pixel:?}")
        }
    }
    std::fs::write(
        directory.join("result.txt"),
        format!(
            "PASS\nOne native winit Window; two independent textures\nDPI: {scale}\nInput isolation: passed\nDirty texture reuse: passed\nCanvas relocation: passed\nVello + Novadraw pixels: passed\nRender counts: {:?}\n",
            live.renders,
        ),
    )?;
    println!("Canvas verification passed: {}", directory.display());
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
