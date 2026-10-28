use anyhow::{Context, Result, bail};
use novadraw::{
    figure::{FigureStyle, LabelFigure, RectangleFigure},
    geometry::Rectangle,
    graphics::Color,
    host::PlatformHost,
    render::{BuiltinFont, RenderBackend, RenderDisposition},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    tree::{FigureId, FigureTree},
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
    host::{CanvasEvent, CanvasId, Delivery},
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
    "Novadraw Canvas A / B | drag shapes | Tab focus | arrows move Canvas | R reset";

struct CanvasScene {
    runtime: Runtime,
    root: FigureId,
    shape: FigureId,
}

impl CanvasScene {
    fn new(rect: CanvasRect, color: Color, title: &str) -> Result<Self> {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                rect.width(),
                rect.height(),
                Color::rgba(0.07, 0.13, 0.20, 1.0),
            )))?;
        let shape = tree.builder().add_child(
            root,
            Box::new(RectangleFigure::new_with_color(
                START.x,
                START.y,
                SHAPE_WIDTH,
                SHAPE_HEIGHT,
                color,
            )),
        )?;
        let label = tree.builder().add_child(
            root,
            Box::new(
                LabelFigure::new(title).with_bounds(Rectangle::new(MARGIN, MARGIN, 300.0, 32.0)),
            ),
        )?;
        let mut runtime = Runtime::new(tree);
        runtime
            .resources()
            .register_builtin_font(BuiltinFont::Inter)?;
        runtime.figure(label)?.set_style(FigureStyle {
            foreground: Some(Color::rgba(0.92, 0.95, 1.0, 1.0)),
            font: Some("18px Inter Variable".to_string()),
            ..FigureStyle::default()
        })?;
        Ok(Self {
            runtime,
            root,
            shape,
        })
    }

    fn position(&self) -> Result<LogicalPosition<f64>> {
        let bounds = self.runtime.tree().figure_bounds(self.shape)?;
        Ok(LogicalPosition::new(bounds.x, bounds.y))
    }
}

struct Live {
    window: Arc<Window>,
    context: RenderContext,
    surface: Option<RenderSurface<'static>>,
    host: WinitCanvasCoordinator,
    canvas_hosts: [WinitCanvasHost; 2],
    ids: [CanvasId; 2],
    targets: [Option<CanvasTarget>; 2],
    output: Option<CanvasTarget>,
    compositor: CanvasCompositor,
    renderers: [VelloTextureRenderer; 2],
    scenes: [CanvasScene; 2],
    drag: [Option<LogicalPosition<f64>>; 2],
    renders: [u32; 2],
    glyph_runs: [usize; 2],
    presented_metrics: Option<WindowMetrics>,
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
        configure_surface(&surface.surface, BACKGROUND);
        let gpu = &context.devices[surface.dev_id];
        let mut host = WinitCanvasCoordinator::new(window.clone())?;
        let rects = layout(metrics)?;
        let canvas_hosts = [host.insert(rects[0])?, host.insert(rects[1])?];
        let ids = [canvas_hosts[0].canvas_id(), canvas_hosts[1].canvas_id()];
        let scenes = [
            CanvasScene::new(
                rects[0],
                Color::rgba(0.95, 0.40, 0.12, 1.0),
                "Novadraw Canvas A",
            )?,
            CanvasScene::new(
                rects[1],
                Color::rgba(0.15, 0.65, 0.95, 1.0),
                "Novadraw Canvas B",
            )?,
        ];
        let compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
        let renderers = [
            VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
            VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
        ];
        let mut live = Self {
            window,
            context,
            surface: Some(surface),
            host,
            canvas_hosts,
            ids,
            targets: [None, None],
            output: None,
            compositor,
            renderers,
            scenes,
            drag: [None, None],
            renders: [0; 2],
            glyph_runs: [0; 2],
            presented_metrics: None,
        };
        live.allocate()?;
        Ok(live)
    }

    fn allocate(&mut self) -> Result<()> {
        let surface = self.surface.as_ref().context("surface is suspended")?;
        let device = &self.context.devices[surface.dev_id].device;
        let metrics = self.host.canvases().metrics();
        for index in 0..2 {
            let rect = self.host.canvases().canvas(&self.ids[index])?.rect();
            let size = metrics.texture_size(rect)?;
            if self.targets[index].as_ref().map(CanvasTarget::size)
                != Some([size.width, size.height])
            {
                self.targets[index] = CanvasTarget::new(device, rect, metrics)?;
            }
        }
        let pixels = metrics.pixels();
        let scale = metrics.scale_factor();
        if self.output.as_ref().map(CanvasTarget::size) != Some([pixels.width, pixels.height]) {
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
        }
        Ok(())
    }

    fn deliver(&mut self, deliveries: Vec<Delivery>) -> Result<()> {
        for delivery in deliveries {
            let Some(index) = self.ids.iter().position(|id| id == &delivery.canvas) else {
                continue;
            };
            match delivery.event {
                CanvasEvent::Geometry { rect, .. } => {
                    let scene = &mut self.scenes[index];
                    scene
                        .runtime
                        .figure(scene.root)?
                        .set_bounds(Rectangle::new(0.0, 0.0, rect.width(), rect.height()))?;
                }
                CanvasEvent::MouseInput {
                    position,
                    button: MouseButton::Left,
                    state,
                } => {
                    if state == ElementState::Pressed {
                        let origin = self.scenes[index].position()?;
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
                        let scene = &mut self.scenes[index];
                        scene
                            .runtime
                            .figure(scene.shape)?
                            .set_bounds(Rectangle::new(
                                position.x - offset.x,
                                position.y - offset.y,
                                SHAPE_WIDTH,
                                SHAPE_HEIGHT,
                            ))?;
                        self.canvas_hosts[index].request_redraw();
                    }
                }
                CanvasEvent::Cancelled | CanvasEvent::Suspended => self.drag[index] = None,
                CanvasEvent::Focused(_) => self.canvas_hosts[index].request_redraw(),
                _ => {}
            }
        }
        Ok(())
    }

    fn resize(&mut self) -> Result<()> {
        let metrics = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        let events = self
            .host
            .update_canvases(|host| host.set_metrics(metrics))?;
        self.deliver(events)?;
        for (index, rect) in layout(metrics)?.into_iter().enumerate() {
            let events = self
                .host
                .update_canvases(|host| host.set_rect(&self.ids[index], rect))?;
            self.deliver(events)?;
        }
        let pixels = metrics.pixels();
        if pixels.width == 0 || pixels.height == 0 {
            self.presented_metrics = None;
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
        // A queued redraw may precede Resized. Synchronize native dimensions
        // before rendering or acquiring a drawable for the new window bounds.
        let current = WindowMetrics::new(self.window.inner_size(), self.window.scale_factor())?;
        if current != self.host.canvases().metrics() {
            self.resize()?;
        }
        self.host.synchronize()?;
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let Some(output) = &self.output else {
            return Ok(());
        };
        let metrics = self.host.canvases().metrics();
        if metrics.pixels().width == 0 || metrics.pixels().height == 0 {
            return Ok(());
        }
        let gpu = &self.context.devices[surface.dev_id];
        for index in 0..2 {
            let Some(target) = &self.targets[index] else {
                continue;
            };
            let Some(token) = self
                .host
                .canvases()
                .canvas(&self.ids[index])?
                .redraw_token()
            else {
                continue;
            };
            {
                let runtime = &mut self.scenes[index].runtime;
                let mut backend = self.renderers[index].bind(target.texture());
                runtime.frames().request_full_redraw();
                let submission = match runtime.frames().prepare_submission(
                    self.canvas_hosts[index].surface_info(),
                    backend.capabilities(),
                ) {
                    FramePreparation::Ready(submission) => submission,
                    other => bail!("Novadraw preparation failed: {other:?}"),
                };
                self.glyph_runs[index] = submission
                    .commands
                    .iter()
                    .filter(|command| {
                        matches!(
                            command.kind,
                            novadraw::render::RenderCommandKind::DrawGlyphRun { .. }
                        )
                    })
                    .count();
                let result = backend.submit(&submission);
                let acknowledgement = runtime.frames().complete_submission(
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
        let token = self.host.canvases().composite_token();
        let layers: Vec<_> = self
            .targets
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                let canvas = self.host.canvases().canvas(&self.ids[index]).ok()?;
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
            // WGPU snapshots presentsWithTransaction at drawable acquisition.
            // Failed acquisition leaves presented_metrics unchanged for retry.
            let presentation = SurfacePresentation::begin(
                &surface.surface,
                self.presented_metrics != Some(metrics),
            );
            let (frame, reconfigure) = match surface.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    drop(presentation);
                    self.context.configure_surface(surface);
                    configure_surface(&surface.surface, BACKGROUND);
                    self.presented_metrics = None;
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
            drop(presentation);
            self.presented_metrics = Some(metrics);
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
            let index = usize::from(self.host.canvases().focused().as_ref() == Some(&self.ids[0]));
            let deliveries = self
                .host
                .update_canvases(|host| host.set_focus(Some(&self.ids[index])))?;
            self.deliver(deliveries)?;
        } else if key == &Key::Character("r".into()) {
            self.resize()?;
        } else if let Some(id) = self.host.canvases().focused() {
            let (dx, dy) = match key {
                Key::Named(NamedKey::ArrowLeft) => (-STEP, 0.0),
                Key::Named(NamedKey::ArrowRight) => (STEP, 0.0),
                Key::Named(NamedKey::ArrowUp) => (0.0, -STEP),
                Key::Named(NamedKey::ArrowDown) => (0.0, STEP),
                _ => return Ok(()),
            };
            let rect = self.host.canvases().canvas(&id)?.rect();
            let deliveries = self.host.update_canvases(|host| {
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
    verify_resize: bool,
    resize_step: usize,
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
                configure_surface(&live.surface.as_ref().unwrap().surface, BACKGROUND);
                live.presented_metrics = None;
                // A replacement device requires fresh consumers and targets.
                let gpu = &live.context.devices[live.surface.as_ref().unwrap().dev_id];
                live.renderers = [
                    VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
                    VelloTextureRenderer::new(&gpu.device, &gpu.queue)?,
                ];
                live.compositor = CanvasCompositor::new(&gpu.device, CANVAS_FORMAT);
                live.targets = [None, None];
                let deliveries = live
                    .host
                    .resume(WindowMetrics::new(pixels, live.window.scale_factor())?)?;
                live.deliver(deliveries)?;
                for scene in &mut live.scenes {
                    scene.runtime.frames().reset_backend_session()?;
                }
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
            let result = live
                .host
                .suspend()
                .map_err(anyhow::Error::from)
                .and_then(|deliveries| live.deliver(deliveries));
            live.surface = None;
            live.presented_metrics = None;
            live.output = None;
            live.targets = [None, None];
            if let Err(error) = result {
                self.fail(event_loop, error);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
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
                if self.verify_resize && verify_resize_frame(live, self.resize_step)? {
                    self.resize_step += 1;
                    if let Some(size) = RESIZE_SIZES.get(self.resize_step) {
                        let _ = live.window.request_inner_size(*size);
                    } else {
                        self.verify_resize = false;
                        let directory = Path::new("target/verification/winit-canvas");
                        live.screenshot(&directory.join("novadraw-resize.png"))?;
                        std::fs::write(
                            directory.join("novadraw-resize-result.txt"),
                            format!(
                                "PASS\nNative resize frames: {}\nWindow/Canvas/Runtime/texture dimensions: passed\nFigure positions preserved: passed\nResize transaction reset: passed\n",
                                self.resize_step,
                            ),
                        )?;
                        event_loop.exit();
                    }
                }
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
            live.host.synchronize()?;
            if !event_loop.exiting() && live.host.canvases().needs_redraw() {
                live.window.request_redraw();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
}

// Width only, height only, both axes; shrink, enlarge, then restore.
const RESIZE_SIZES: [LogicalSize<f64>; 9] = [
    LogicalSize::new(960.0, 600.0),
    LogicalSize::new(820.0, 600.0),
    LogicalSize::new(820.0, 480.0),
    LogicalSize::new(680.0, 400.0),
    LogicalSize::new(1040.0, 680.0),
    LogicalSize::new(1040.0, 520.0),
    LogicalSize::new(760.0, 520.0),
    LogicalSize::new(1120.0, 720.0),
    LogicalSize::new(960.0, 600.0),
];

fn verify_resize_frame(live: &Live, step: usize) -> Result<bool> {
    let expected = RESIZE_SIZES[step].to_physical::<u32>(live.window.scale_factor());
    if live.window.inner_size() != expected {
        return Ok(false);
    }
    let metrics = live.host.canvases().metrics();
    if live.presented_metrics != Some(metrics) {
        return Ok(false);
    }
    let surface = live.surface.as_ref().context("surface is suspended")?;
    if metrics.pixels() != expected
        || surface.config.width != expected.width
        || surface.config.height != expected.height
        || live.output.as_ref().map(CanvasTarget::size) != Some([expected.width, expected.height])
    {
        bail!("resize presented stale window geometry")
    }
    for (index, scene) in live.scenes.iter().enumerate() {
        let rect = live.host.canvases().canvas(&live.ids[index])?.rect();
        let size = metrics.texture_size(rect)?;
        if scene.position()? != START
            || scene.runtime.tree().figure_bounds(scene.root)?
                != Rectangle::new(0.0, 0.0, rect.width(), rect.height())
            || live.targets[index].as_ref().map(CanvasTarget::size)
                != Some([size.width, size.height])
        {
            bail!("Canvas {index} geometry did not converge after resize")
        }
    }
    #[cfg(target_os = "macos")]
    {
        let metal = unsafe { surface.surface.as_hal::<wgpu::hal::api::Metal>() }
            .context("resize check requires a Metal surface")?;
        if metal.render_layer().lock().presentsWithTransaction() {
            bail!("resize transaction did not reset after present")
        }
        {
            let _guard = SurfacePresentation::begin(&surface.surface, true);
            if !metal.render_layer().lock().presentsWithTransaction() {
                bail!("resize transaction did not enable before acquire")
            }
        }
        if metal.render_layer().lock().presentsWithTransaction() {
            bail!("abandoned resize transaction did not reset")
        }
    }
    Ok(true)
}

fn verify(live: &mut Live) -> Result<()> {
    use winit::{dpi::PhysicalPosition, event::DeviceId};
    let directory = Path::new("target/verification/winit-canvas");
    let before = live.screenshot(&directory.join("native-before.png"))?;
    let original = [live.scenes[0].position()?, live.scenes[1].position()?];
    if live.scenes[0].runtime.frames().backend_session_id()
        == live.scenes[1].runtime.frames().backend_session_id()
    {
        bail!("Canvas runtimes must have distinct backend sessions")
    }
    let original_renders = live.renders;
    for index in 0..2 {
        let rect = live.host.canvases().canvas(&live.ids[index])?.rect();
        let scale = live.host.canvases().metrics().scale_factor();
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
        if live.scenes[index].position()?
            != LogicalPosition::new(START.x + STEP * 3.0, START.y + STEP * 2.0)
            || (index == 0 && live.scenes[1].position()? != original[1])
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
    let scale = live.host.canvases().metrics().scale_factor();
    for (index, original_position) in original.iter().enumerate() {
        let rect = live.host.canvases().canvas(&live.ids[index])?.rect();
        let position = live.scenes[index].position()?;
        let p = rect.local_to_window(LogicalPosition::new(position.x + STEP, position.y + STEP));
        let pixel = after.get_pixel((p.x * scale) as u32, (p.y * scale) as u32);
        if (index == 0 && pixel[0] < 200) || (index == 1 && pixel[2] < 200) {
            bail!("renderer {index} did not produce its expected color: {pixel:?}")
        }
        let old = rect.local_to_window(LogicalPosition::new(
            original_position.x + STEP,
            original_position.y + STEP,
        ));
        let old_pixel = after.get_pixel((old.x * scale) as u32, (old.y * scale) as u32);
        if old_pixel.0[..3].iter().any(|value| *value > 100) {
            bail!("Canvas {index} did not erase the Figure's old location")
        }
        if live.glyph_runs[index] == 0 {
            bail!("Canvas {index} did not submit Novadraw text")
        }
    }
    std::fs::write(
        directory.join("result.txt"),
        format!(
            "PASS\nOne native winit Window; two Novadraw runtimes and textures\nDPI: {scale}\nIndependent backend sessions: passed\nInput isolation: passed\nDirty texture reuse: passed\nCanvas relocation: passed\nFigure old area erasure: passed\nNovadraw pixels and glyphs in both Canvas: passed\nRender counts: {:?}\nGlyph runs: {:?}\n",
            live.renders, live.glyph_runs,
        ),
    )?;
    println!("Canvas verification passed: {}", directory.display());
    Ok(())
}

fn main() -> Result<()> {
    let mut app = App {
        verify: std::env::args().any(|arg| arg == "--verify"),
        verify_resize: std::env::args().any(|arg| arg == "--verify-resize"),
        ..Default::default()
    };
    if app.verify && app.verify_resize {
        bail!("run --verify and --verify-resize separately");
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
