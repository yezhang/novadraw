use crate::{
    Config,
    research::{
        HEIGHT, INITIAL_WIDTH, Journal, NARROW_WIDTH, Operation, Research, SCROLL_STEP, replay,
    },
};
use anyhow::{Context, Result, bail};
use novadraw::{
    event::{KeyModifiers, MonotonicTime},
    geometry::Point,
    host::PlatformHost,
};
use novadraw_backend_vello::VelloRenderer;
use novadraw_platform_winit::{
    AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, WinitPlatformHost, WinitPointerAdapter,
    adapt_key_event, adapt_key_input, adapt_modifiers, adapt_physical_key,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const GPU_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const RESIZE_TIMEOUT: Duration = Duration::from_secs(5);
const NATIVE_FRAME_ATTEMPTS: usize = 8;

pub fn run(config: Config) -> Result<()> {
    let mut app = NativeApp::new(config);
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            app.journal
                .push(json!({"kind":"failure","phase":"event_loop","error":error.to_string()}));
            app.export()?;
            return Err(error.into());
        }
    };
    let result = event_loop.run_app(&mut app);
    if let Err(error) = &result {
        app.journal
            .push(json!({"kind":"failure","phase":"event_loop_run","error":error.to_string()}));
    }
    // Also flush if the event loop terminates without a CloseRequested event.
    app.export()?;
    result?;
    if let Some(error) = app.failure {
        bail!("{error}")
    }
    Ok(())
}

struct NativeApp {
    config: Config,
    journal: Journal,
    research: Research,
    host: Option<WinitPlatformHost>,
    renderer: Option<VelloRenderer>,
    queue: VecDeque<Operation>,
    waiting_resize: Option<(Operation, Instant)>,
    frame_pending: bool,
    frame_attempts: usize,
    occluded: bool,
    replay_active: bool,
    replay_finished: bool,
    screenshot_taken: bool,
    pointer: WinitPointerAdapter,
    gestures: WinitGestureAdapter,
    position: Option<Point>,
    modifiers: KeyModifiers,
    clock: Instant,
    failure: Option<String>,
}

impl NativeApp {
    fn new(config: Config) -> Self {
        let replay_active = config.replay;
        Self {
            journal: Journal::new(&config),
            research: Research::new(config.timing, config.requests),
            queue: if config.replay {
                replay(config.cycles).into()
            } else {
                [Operation::Reset].into()
            },
            config,
            host: None,
            renderer: None,
            waiting_resize: None,
            frame_pending: false,
            frame_attempts: 0,
            occluded: false,
            replay_active,
            replay_finished: false,
            screenshot_taken: false,
            pointer: WinitPointerAdapter::new(),
            gestures: WinitGestureAdapter::default(),
            position: None,
            modifiers: KeyModifiers::default(),
            clock: Instant::now(),
            failure: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        self.journal
            .push(json!({"kind":"failure","phase":"native","error":format!("{error:#}")}));
        self.failure = Some(format!("{error:#}"));
        if let Err(error) = self.export() {
            eprintln!("export failed: {error:#}");
        }
        event_loop.exit();
    }

    fn export(&mut self) -> Result<()> {
        if !self.journal.has_records() {
            return Ok(());
        }
        let path = self.journal.export(&self.config.output)?;
        println!("TextFlowLarge export: {}", path.display());
        Ok(())
    }

    fn sync_effects(&mut self) {
        if let Some(host) = &self.host {
            host.set_cursor(self.research.runtime.focus().cursor_icon());
            for update in self.research.runtime.tooltip().take_updates() {
                host.update_tooltip(update);
            }
            for update in self.research.runtime.accessibility().take_updates() {
                host.update_accessibility(update);
            }
            host.schedule_wake(self.research.runtime.animation().next_wake_deadline());
        }
    }

    fn perform(&mut self, operation: Operation) -> Result<()> {
        if matches!(operation, Operation::Reset) {
            if self.research.has_operations() {
                self.export()?;
            }
            self.pointer.reset();
            self.gestures.cancel_all();
            self.position = None;
        }
        self.research.apply(operation, &mut self.journal)?;
        self.frame_pending = true;
        self.frame_attempts = 0;
        self.sync_effects();
        self.host
            .as_ref()
            .context("window missing")?
            .request_redraw();
        Ok(())
    }

    fn enqueue(&mut self, operation: Operation) {
        self.queue.push_back(operation);
    }

    fn next_operation(&mut self) -> Result<()> {
        if self.frame_pending || self.waiting_resize.is_some() {
            return Ok(());
        }
        let Some(operation) = self.queue.pop_front() else {
            return Ok(());
        };
        let host = self.host.as_ref().context("window missing")?;
        let width = match operation {
            Operation::Reset => Some(INITIAL_WIDTH),
            Operation::Resize(width) => Some(width),
            _ => None,
        };
        if let Some(width) = width {
            let surface = host.surface_info();
            if (surface.logical_width - width).abs() > 1.0 / surface.scale_factor
                || (surface.logical_height - HEIGHT).abs() > 1.0 / surface.scale_factor
            {
                self.waiting_resize = Some((operation, Instant::now()));
                // request_inner_size may return an immediate size or a later Resized event.
                if let Some(size) = host
                    .window()
                    .request_inner_size(LogicalSize::new(width, HEIGHT))
                {
                    self.accept_resize(size)?;
                }
                return Ok(());
            }
        }
        self.perform(operation)
    }

    fn accept_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) -> Result<()> {
        let host = self.host.as_ref().context("window missing")?;
        let scale = host.window().scale_factor();
        let width = f64::from(size.width) / scale;
        let height = f64::from(size.height) / scale;
        if let Some((operation, started)) = self.waiting_resize {
            let expected = match operation {
                Operation::Resize(w) => w,
                _ => INITIAL_WIDTH,
            };
            if (width - expected).abs() <= 1.0 / scale && (height - HEIGHT).abs() <= 1.0 / scale {
                self.research.note_pending(
                    "window_resize_ack",
                    json!({"elapsed_ns":started.elapsed().as_nanos(),
                        "requested_width":expected,"actual_width":width,"actual_height":height}),
                    &mut self.journal,
                );
                self.waiting_resize = None;
                self.perform(operation)?;
            }
        } else if !self.replay_active {
            // Real window events are also research operations; preserve actual height.
            self.research
                .runtime
                .frames()
                .resize_logical_viewport(width, height)?;
            self.research.width = width;
            self.research.note_pending(
                "window_resize",
                json!({"width":width,"height":height}),
                &mut self.journal,
            );
            self.perform(Operation::Stable)?;
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        if self.waiting_resize.is_some() {
            return Ok(());
        }
        let host = self.host.as_ref().context("window missing")?;
        let renderer = self.renderer.as_mut().context("renderer missing")?;
        host.window().pre_present_notify();
        let presented = self.research.render_attempt(
            renderer,
            host.surface_info(),
            "native_backend_presented",
            self.frame_attempts,
            &mut self.journal,
        )?;
        self.frame_attempts += 1;
        if !presented {
            anyhow::ensure!(
                self.frame_attempts < NATIVE_FRAME_ATTEMPTS,
                "native frame retry limit reached"
            );
            self.frame_pending = true;
            host.request_redraw();
            return Ok(());
        }
        self.frame_attempts = 0;
        if self.config.gpu_wait {
            let start = Instant::now();
            let result = renderer.wait_for_gpu_idle(GPU_WAIT_TIMEOUT);
            let elapsed = start.elapsed().as_nanos();
            self.research.note(
                "gpu_queue_wait",
                json!({"elapsed_ns":elapsed,
                "succeeded":result.is_ok(),"error":result.as_ref().err().map(ToString::to_string)}),
                &mut self.journal,
            );
            result?;
        }
        if self.config.screenshot && !self.screenshot_taken {
            std::fs::create_dir_all(&self.config.output)?;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let path = self.config.output.join(format!("native-{timestamp}.png"));
            renderer.screenshot(&path)?;
            self.research.note(
                "screenshot",
                json!({"path":path,"source":"retained GPU target; not desktop capture"}),
                &mut self.journal,
            );
            self.screenshot_taken = true;
        }
        self.frame_pending = false;
        host.window().set_title(&format!(
            "TextFlowLarge · {} · F1 Reset  F2 Z  F3 Scroll  F4 Resize  F6 Replay  F7 Export  F8 Timing",
            if self.research.timing { "timing on" } else { "timing off" }));
        self.sync_effects();
        Ok(())
    }

    fn input_event(&mut self, event: WindowEvent) -> Result<()> {
        let host = self.host.as_ref().context("window missing")?;
        let scale = host.window().scale_factor();
        let start = Instant::now();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let point = Point::new(position.x / scale, position.y / scale);
                self.pointer.pointer_moved(point)?;
                self.position = Some(point);
                self.research
                    .runtime
                    .input()
                    .dispatch_mouse_moved(point.x(), point.y());
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer.reset();
                self.position = None;
                self.research.runtime.input().pointer_exited();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(position) = self.position {
                    self.pointer.dispatch_button_input(
                        &mut self.research.runtime,
                        state,
                        button,
                        position,
                        self.clock.elapsed(),
                    )?;
                }
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                self.pointer.reset();
                let position = self
                    .position
                    .unwrap_or(Point::new(self.research.width / 2.0, HEIGHT / 2.0));
                if let Some(AdaptedGesture::Scroll(event)) = self.gestures.adapt_mouse_wheel(
                    device_id,
                    delta,
                    phase,
                    position.x() * scale,
                    position.y() * scale,
                    scale,
                    self.modifiers,
                ) {
                    self.research.runtime.input().dispatch_scroll(event);
                }
            }
            WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => {
                self.pointer.reset();
                let position = self
                    .position
                    .unwrap_or(Point::new(self.research.width / 2.0, HEIGHT / 2.0));
                if let Some(AdaptedGesture::Zoom(event)) = self.gestures.adapt_pinch(
                    device_id,
                    delta,
                    phase,
                    position.x() * scale,
                    position.y() * scale,
                    scale,
                    self.modifiers,
                ) {
                    self.research.runtime.input().dispatch_zoom(event);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = adapt_modifiers(modifiers.state())
            }
            WindowEvent::Focused(false) => {
                self.pointer.reset();
                self.gestures.cancel_all();
                self.research.runtime.input().cancel_gestures();
                self.research.runtime.input().pointer_exited();
                self.research.runtime.focus().release();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                let key = if pressed {
                    adapt_key_event(event.physical_key, event.text.as_deref(), self.modifiers)
                } else {
                    adapt_physical_key(event.physical_key)
                };
                if let Some(key) = key {
                    match adapt_key_input(key, pressed, self.modifiers) {
                        AdaptedKeyInput::FocusTraversal(direction) => {
                            self.research.runtime.focus().traverse(direction);
                        }
                        AdaptedKeyInput::Key {
                            key,
                            pressed: true,
                            modifiers,
                        } => {
                            self.research
                                .runtime
                                .input()
                                .dispatch_key_pressed(key, modifiers);
                        }
                        AdaptedKeyInput::Key {
                            key,
                            pressed: false,
                            modifiers,
                        } => {
                            self.research
                                .runtime
                                .input()
                                .dispatch_key_released(key, modifiers);
                        }
                        AdaptedKeyInput::Ignored => {}
                    }
                }
            }
            _ => return Ok(()),
        }
        let elapsed = start.elapsed().as_nanos();
        self.research.note_pending(
            "input_dispatch",
            json!({"elapsed_ns":elapsed}),
            &mut self.journal,
        );
        self.perform(Operation::Stable)
    }

    fn event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<()> {
        match event {
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    self.frame_attempts = 0;
                    self.frame_pending = true;
                    self.host.as_ref().unwrap().request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                self.export()?;
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if !self.occluded && (self.frame_pending || !self.replay_active) {
                    self.draw()?;
                }
            }
            WindowEvent::Resized(size) => self.accept_resize(size)?,
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(host) = &self.host {
                    self.accept_resize(host.window().inner_size())?;
                }
            }
            WindowEvent::KeyboardInput {
                event: ref key_event,
                ..
            } if key_event.state == ElementState::Pressed && !key_event.repeat => {
                let shortcut = match key_event.physical_key {
                    PhysicalKey::Code(KeyCode::F1) => Some(Operation::Reset),
                    PhysicalKey::Code(KeyCode::F2) => Some(Operation::Append),
                    PhysicalKey::Code(KeyCode::F3) => Some(Operation::Scroll(SCROLL_STEP)),
                    PhysicalKey::Code(KeyCode::F4) => {
                        Some(Operation::Resize(if self.research.width > NARROW_WIDTH {
                            NARROW_WIDTH
                        } else {
                            INITIAL_WIDTH
                        }))
                    }
                    PhysicalKey::Code(KeyCode::F5) => Some(Operation::Stable),
                    PhysicalKey::Code(KeyCode::F6) => {
                        if !self.replay_active {
                            self.queue = replay(self.config.cycles).into();
                            self.replay_active = true;
                            self.replay_finished = false;
                        }
                        return Ok(());
                    }
                    PhysicalKey::Code(KeyCode::F7) => {
                        self.export()?;
                        return Ok(());
                    }
                    PhysicalKey::Code(KeyCode::F8) => {
                        self.research.timing = !self.research.timing;
                        self.research
                            .runtime
                            .frames()
                            .set_timing_enabled(self.research.timing);
                        self.research.note(
                            "timing_switch",
                            json!({"enabled":self.research.timing}),
                            &mut self.journal,
                        );
                        return Ok(());
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        self.export()?;
                        event_loop.exit();
                        return Ok(());
                    }
                    _ => None,
                };
                if let Some(operation) = shortcut {
                    if !self.replay_active {
                        self.enqueue(operation);
                    }
                } else if !self.replay_active {
                    self.input_event(event)?;
                }
            }
            _ if !self.replay_active => self.input_event(event)?,
            _ => {}
        }
        Ok(())
    }
}

impl ApplicationHandler for NativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| {
            if self.host.is_none() {
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title("TextFlowLarge")
                            .with_inner_size(LogicalSize::new(INITIAL_WIDTH, HEIGHT)),
                    )?,
                );
                window.set_visible(true);
                window.focus_window();
                self.host = Some(WinitPlatformHost::new(window));
            }
            let host = self.host.as_ref().unwrap();
            self.renderer = Some(VelloRenderer::new(host.window_arc(), host.surface_info())?);
            self.research.runtime.frames().reset_backend_session()?;
            let adapter = self.renderer.as_ref().unwrap().adapter_info();
            self.journal.push(
                json!({"kind":"adapter","name":adapter.name,"vendor":adapter.vendor,
                "device":adapter.device,"device_type":adapter.device_type,"backend":adapter.backend,
                "driver":adapter.driver,"driver_info":adapter.driver_info}),
            );
            self.next_operation()?;
            if self.queue.is_empty() && !self.frame_pending && self.waiting_resize.is_none() {
                self.frame_pending = true;
                self.host.as_ref().unwrap().request_redraw();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.renderer = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if self.failure.is_some() {
            return;
        }
        if self.renderer.is_none() && !matches!(event, WindowEvent::CloseRequested) {
            return;
        }
        if let Err(error) = self.event(event_loop, event) {
            self.fail(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| {
            if self.failure.is_some() {
                return Ok(());
            }
            if self.renderer.is_none() {
                return Ok(());
            }
            if self.occluded {
                event_loop.set_control_flow(ControlFlow::Wait);
                return Ok(());
            }
            if let Some((_, start)) = self.waiting_resize {
                anyhow::ensure!(
                    start.elapsed() < RESIZE_TIMEOUT,
                    "window resize acknowledgement timed out"
                );
                event_loop.set_control_flow(ControlFlow::WaitUntil(start + RESIZE_TIMEOUT));
                return Ok(());
            }
            self.next_operation()?;
            if self.replay_active
                && self.queue.is_empty()
                && !self.frame_pending
                && self.waiting_resize.is_none()
                && !self.replay_finished
            {
                self.replay_finished = true;
                self.replay_active = false;
                self.journal
                    .push(json!({"kind":"replay_complete","cycles":self.config.cycles}));
                self.export()?;
                if self.config.exit_after_replay {
                    event_loop.exit();
                    return Ok(());
                }
            }
            if self.frame_pending || !self.queue.is_empty() {
                event_loop.set_control_flow(ControlFlow::Poll);
                self.host.as_ref().unwrap().request_redraw();
            } else {
                let now = MonotonicTime::from_micros(
                    self.clock
                        .elapsed()
                        .as_micros()
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
                if self.research.runtime.animation().advance_time(now)? {
                    self.perform(Operation::Stable)?;
                }
                self.sync_effects();
                if let Some(deadline) = self
                    .host
                    .as_ref()
                    .and_then(WinitPlatformHost::wake_deadline)
                {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        self.clock + Duration::from_micros(deadline.as_micros()),
                    ));
                } else {
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
}
