use crate::{
    Config,
    research::{
        FrameAttempt, HEIGHT, INITIAL_WIDTH, NARROW_WIDTH, Operation, Research, SCROLL_STEP,
        TraceSession, replay,
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
    event::{DeviceId, ElementState, MouseScrollDelta, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const GPU_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const RESIZE_TIMEOUT: Duration = Duration::from_secs(5);
const NATIVE_FRAME_ATTEMPTS: usize = 8;
// A temporarily unavailable drawable must yield to window events. This host
// recovery poll is deliberately slower than frame pacing and is not a failure.
const SURFACE_RETRY_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct FrameRecovery {
    attempts: usize,
    retries: usize,
    redraw_after: Option<Instant>,
}
impl FrameRecovery {
    fn reset(&mut self) {
        *self = Self::default();
    }
    fn acknowledge(&mut self, outcome: FrameAttempt, now: Instant) -> Result<bool> {
        self.attempts = self.attempts.saturating_add(1);
        match outcome {
            FrameAttempt::Complete => {
                self.reset();
                Ok(true)
            }
            FrameAttempt::SurfaceUnavailable => {
                self.redraw_after = Some(now + SURFACE_RETRY_INTERVAL);
                Ok(false)
            }
            FrameAttempt::Retry => {
                self.retries += 1;
                anyhow::ensure!(
                    self.retries < NATIVE_FRAME_ATTEMPTS,
                    "native frame retry limit reached"
                );
                self.redraw_after = None;
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_focus_and_append_recover_after_release_and_reset() {
        let mut app = NativeApp::new(Config {
            headless: false,
            replay: false,
            cycles: 1,
            trace: false,
            requests: false,
            gpu_wait: false,
            exit_after_replay: false,
            screenshot: false,
            scroll_replay: false,
            interaction_replay: false,
            timings: false,
            natural_damage: true,
            text_raster_cache: false,
            flow_assembly_reuse: true,
            raster_validation: false,
            coarse_trace: false,
            offscreen_gpu: false,
            output: "target/verification/text-fr02-focus-test".into(),
        })
        .unwrap();
        app.research
            .apply(Operation::Stable, &mut app.trace)
            .unwrap();
        let old_page = app.research.runtime.input().state().focus_owner().unwrap();
        for operation in [Operation::Append, Operation::Reset, Operation::Append] {
            app.window_focus_changed(false).unwrap();
            app.research
                .apply(Operation::Stable, &mut app.trace)
                .unwrap();
            assert!(app.research.runtime.input().state().focus_owner().is_none());
            app.window_focus_changed(true).unwrap();
            assert!(app.research.runtime.input().state().focus_owner().is_some());
            // The fixture shortcut must also work if an intervening event
            // releases focus before the queued operation is executed.
            app.window_focus_changed(false).unwrap();
            app.research.apply(operation, &mut app.trace).unwrap();
            assert!(app.research.runtime.input().state().focus_owner().is_some());
            assert!(
                app.research
                    .runtime
                    .take_deferred_mutation_errors()
                    .is_empty()
            );
        }
        assert_ne!(
            app.research.runtime.input().state().focus_owner().unwrap(),
            old_page
        );
    }

    #[test]
    fn unavailable_drawables_yield_without_exhausting_retry_budget() {
        let mut recovery = FrameRecovery::default();
        let start = Instant::now();
        for step in 0..(NATIVE_FRAME_ATTEMPTS * 2) {
            let now = start + SURFACE_RETRY_INTERVAL * step as u32;
            assert!(
                !recovery
                    .acknowledge(FrameAttempt::SurfaceUnavailable, now)
                    .unwrap()
            );
            assert_eq!(recovery.redraw_after, Some(now + SURFACE_RETRY_INTERVAL));
            assert_eq!(recovery.retries, 0);
        }
        assert!(recovery.acknowledge(FrameAttempt::Complete, start).unwrap());
        assert_eq!(recovery.attempts, 0);
        assert!(recovery.redraw_after.is_none());
    }

    #[test]
    fn surface_wait_preserves_actual_retry_limit_and_success_resets_it() {
        let mut recovery = FrameRecovery::default();
        let now = Instant::now();
        for _ in 1..NATIVE_FRAME_ATTEMPTS {
            assert!(!recovery.acknowledge(FrameAttempt::Retry, now).unwrap());
            assert!(
                !recovery
                    .acknowledge(FrameAttempt::SurfaceUnavailable, now)
                    .unwrap()
            );
        }
        assert!(recovery.acknowledge(FrameAttempt::Retry, now).is_err());
        recovery.reset();
        assert!(!recovery.acknowledge(FrameAttempt::Retry, now).unwrap());
        assert!(recovery.acknowledge(FrameAttempt::Complete, now).unwrap());
        assert_eq!(recovery.retries, 0);
    }
}

struct ResearchBackend<'a> {
    renderer: &'a mut VelloRenderer,
    offscreen: bool,
}

impl novadraw::render::RenderBackend for ResearchBackend<'_> {
    fn capabilities(&self) -> novadraw::render::BackendCapabilities {
        novadraw::render::RenderBackend::capabilities(self.renderer)
    }
    fn submit(
        &mut self,
        submission: &novadraw::render::RenderSubmission,
    ) -> novadraw::render::RenderResult {
        if self.offscreen {
            self.renderer.render_for_screenshot(submission)
        } else {
            novadraw::render::RenderBackend::submit(self.renderer, submission)
        }
    }
    fn resize(&mut self, width: u32, height: u32, scale: f64) {
        novadraw::render::RenderBackend::resize(self.renderer, width, height, scale);
    }
}

pub fn run(config: Config) -> Result<()> {
    let mut app = NativeApp::new(config)?;
    if app.config.timings {
        eprintln!("TextFlowLarge startup: creating event loop");
    }
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            app.trace
                .push(json!({"kind":"failure","phase":"event_loop","error":error.to_string()}));
            app.export()?;
            return Err(error.into());
        }
    };
    if app.config.timings {
        eprintln!("TextFlowLarge startup: running event loop");
    }
    let result = event_loop.run_app(&mut app);
    if let Err(error) = &result {
        app.trace
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
    trace: TraceSession,
    research: Research,
    host: Option<WinitPlatformHost>,
    renderer: Option<VelloRenderer>,
    queue: VecDeque<Operation>,
    waiting_resize: Option<(Operation, Instant)>,
    frame_pending: bool,
    frame_recovery: FrameRecovery,
    occluded: bool,
    replay_active: bool,
    replay_finished: bool,
    screenshot_taken: bool,
    screenshot_sequence: usize,
    pointer: WinitPointerAdapter,
    gestures: WinitGestureAdapter,
    position: Option<Point>,
    modifiers: KeyModifiers,
    clock: Instant,
    failure: Option<String>,
    operation_started: Option<(Operation, Instant)>,
}

impl NativeApp {
    fn new(config: Config) -> Result<Self> {
        let replay_active = config.replay;
        let mut research = Research::new(config.trace, config.flow_assembly_reuse);
        research.full_redraw = !config.natural_damage;
        Ok(Self {
            trace: TraceSession::new(&config)?,
            research,
            queue: if config.replay {
                if config.raster_validation {
                    crate::research::raster_validation().into()
                } else if config.interaction_replay {
                    crate::research::interaction_replay(config.cycles).into()
                } else if config.scroll_replay {
                    crate::research::scroll_replay(config.cycles, config.natural_damage).into()
                } else {
                    replay(config.cycles).into()
                }
            } else {
                [Operation::Reset].into()
            },
            config,
            host: None,
            renderer: None,
            waiting_resize: None,
            frame_pending: false,
            frame_recovery: FrameRecovery::default(),
            occluded: false,
            replay_active,
            replay_finished: false,
            screenshot_taken: false,
            screenshot_sequence: 0,
            pointer: WinitPointerAdapter::new(),
            gestures: WinitGestureAdapter::default(),
            position: None,
            modifiers: KeyModifiers::default(),
            clock: Instant::now(),
            failure: None,
            operation_started: None,
        })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        self.trace
            .push(json!({"kind":"failure","phase":"native","error":format!("{error:#}")}));
        self.failure = Some(format!("{error:#}"));
        if let Err(error) = self.export() {
            eprintln!("export failed: {error:#}");
        }
        event_loop.exit();
    }

    fn export(&mut self) -> Result<()> {
        if !self.trace.has_records() {
            return Ok(());
        }
        let path = self.trace.export(&self.config.output)?;
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
        self.research.apply(operation, &mut self.trace)?;
        self.frame_pending = true;
        self.frame_recovery.reset();
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

    fn window_focus_changed(&mut self, focused: bool) -> Result<()> {
        if focused {
            self.research.focus_typing_page()?;
        } else {
            self.pointer.reset();
            self.gestures.cancel_all();
            self.research.runtime.input().cancel_gestures();
            self.research.runtime.input().pointer_exited();
            self.research.runtime.focus().release();
        }
        Ok(())
    }

    fn next_operation(&mut self) -> Result<()> {
        if self.frame_pending || self.waiting_resize.is_some() {
            return Ok(());
        }
        let Some(operation) = self.queue.pop_front() else {
            return Ok(());
        };
        self.operation_started = Some((operation, Instant::now()));
        if matches!(
            operation,
            Operation::DragStart | Operation::DragMove(_) | Operation::DragEnd
        ) {
            if matches!(operation, Operation::DragStart) {
                let point = self.research.scroll_thumb_center()?;
                self.position = Some(point);
                self.pointer.pointer_moved(point)?;
                self.research
                    .runtime
                    .input()
                    .dispatch_mouse_moved(point.x(), point.y());
            }
            let event = if let Operation::DragMove(delta) = operation {
                let point = self.position.context("drag pointer missing")?;
                let scale = self
                    .host
                    .as_ref()
                    .context("window missing")?
                    .window()
                    .scale_factor();
                WindowEvent::CursorMoved {
                    device_id: DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(
                        point.x() * scale,
                        (point.y() + delta) * scale,
                    ),
                }
            } else {
                WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    state: if matches!(operation, Operation::DragStart) {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    },
                    button: winit::event::MouseButton::Left,
                }
            };
            return self.input_event(event, operation);
        }
        if let Operation::Wheel(delta) = operation {
            let scale = self
                .host
                .as_ref()
                .context("window missing")?
                .window()
                .scale_factor();
            return self.input_event(
                WindowEvent::MouseWheel {
                    device_id: DeviceId::dummy(),
                    delta: MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                        0.0,
                        delta * scale,
                    )),
                    phase: TouchPhase::Moved,
                },
                operation,
            );
        }
        if matches!(operation, Operation::Pointer) {
            let scale = self
                .host
                .as_ref()
                .context("window missing")?
                .window()
                .scale_factor();
            return self.input_event(
                WindowEvent::CursorMoved {
                    device_id: DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(
                        self.research.width * scale / 2.0,
                        HEIGHT * scale / 2.0,
                    ),
                },
                operation,
            );
        }
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
        if let Some((operation, _started)) = self.waiting_resize {
            let expected = match operation {
                Operation::Resize(w) => w,
                _ => INITIAL_WIDTH,
            };
            if (width - expected).abs() <= 1.0 / scale && (height - HEIGHT).abs() <= 1.0 / scale {
                self.research.note_pending(
                    "window_resize_ack",
                    json!({"requested_width":expected,"actual_width":width,"actual_height":height}),
                    &mut self.trace,
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
                &mut self.trace,
            );
            self.perform(Operation::Stable)?;
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        let _draw = self.trace.scope("host-redraw-callback");
        if self.waiting_resize.is_some()
            || self
                .frame_recovery
                .redraw_after
                .is_some_and(|deadline| Instant::now() < deadline)
        {
            return Ok(());
        }
        let host = self.host.as_ref().context("window missing")?;
        let renderer = self.renderer.as_mut().context("renderer missing")?;
        host.window().pre_present_notify();
        let outcome = self.research.render_attempt(
            &mut ResearchBackend {
                renderer,
                offscreen: self.config.offscreen_gpu,
            },
            host.surface_info(),
            if self.config.offscreen_gpu {
                "gpu_offscreen_published"
            } else {
                "native_backend_presented"
            },
            self.frame_recovery.attempts,
            &mut self.trace,
        )?;
        if !self.frame_recovery.acknowledge(outcome, Instant::now())? {
            self.frame_pending = true;
            if self.frame_recovery.redraw_after.is_none() {
                host.request_redraw();
            }
            return Ok(());
        }
        let cache = renderer.text_raster_statistics();
        self.research.note(
            "text_raster_cache",
            json!({
                "enabled":self.config.text_raster_cache, "eligible":cache.eligible,
                "hits":cache.hits,"misses":cache.misses,"resident_bytes":cache.resident_bytes,
                "encoded_glyphs":cache.encoded_glyphs
            }),
            &mut self.trace,
        );
        if self.config.gpu_wait {
            let _wait = self.trace.scope("host-gpu-queue-wait");
            let result = renderer.wait_for_gpu_idle(GPU_WAIT_TIMEOUT);
            self.research.note(
                "gpu_queue_wait",
                json!({"succeeded":result.is_ok(),"error":result.as_ref().err().map(ToString::to_string)}),
                &mut self.trace,
            );
            result?;
        }
        if self.config.screenshot && (!self.screenshot_taken || self.config.raster_validation) {
            std::fs::create_dir_all(&self.config.output)?;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let path = if self.config.raster_validation {
                self.config
                    .output
                    .join(format!("frame-{:04}.png", self.screenshot_sequence))
            } else {
                self.config.output.join(format!("native-{timestamp}.png"))
            };
            renderer.screenshot(&path)?;
            self.screenshot_sequence += 1;
            self.research.note(
                "screenshot",
                json!({"path":path,"source":"retained GPU target; not desktop capture"}),
                &mut self.trace,
            );
            self.screenshot_taken = true;
        }
        self.frame_pending = false;
        if let Some((operation, start)) = self.operation_started.take() {
            self.research.note("interaction_latency", json!({
                "operation":format!("{operation:?}"),"elapsed_ns":start.elapsed().as_nanos(),
                "includes":"event dispatch, query, resize acknowledgement, redraw scheduling, backend submit; GPU completion only with --gpu-wait"
            }), &mut self.trace);
        }
        host.window().set_title(&format!(
            "TextFlowLarge · {} · F1 Reset  F2 Z  F3 Scroll  F4 Resize  F6 Replay  F7 Export  F8 Timing",
            if self.research.capture_enabled { "capture on" } else { "capture off" }));
        self.sync_effects();
        Ok(())
    }

    fn input_event(&mut self, event: WindowEvent, operation: Operation) -> Result<()> {
        let input_kind = match &event {
            WindowEvent::MouseWheel { .. } => "wheel",
            WindowEvent::CursorMoved { .. } => "pointer",
            _ => "other",
        };
        let before_y = self.research.viewport_y();
        let host = self.host.as_ref().context("window missing")?;
        let scale = host.window().scale_factor();
        let _dispatch = self.trace.scope("host-input-dispatch");
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
                self.research.note_pending(
                    "wheel_input",
                    json!({"delta":format!("{delta:?}"),"phase":format!("{phase:?}"),
                        "source":if self.replay_active {"fixed_winit_replay"} else {"os"}}),
                    &mut self.trace,
                );
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
            WindowEvent::Focused(focused) => self.window_focus_changed(focused)?,
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
        let after_y = self.research.viewport_y();
        self.research.note_pending(
            "input_dispatch",
            json!({"dispatched":true,"input_kind":input_kind,"viewport_before_y":before_y,
                "viewport_after_y":after_y}),
            &mut self.trace,
        );
        self.perform(operation)
    }

    fn event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<()> {
        match event {
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    self.frame_recovery.reset();
                    self.frame_pending = true;
                    self.host.as_ref().unwrap().request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                self.export()?;
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if (!self.occluded || self.config.offscreen_gpu)
                    && (self.frame_pending || !self.replay_active)
                {
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
                        let enabled = !self.research.capture_enabled;
                        if enabled && self.trace.has_records() {
                            self.export()?;
                        }
                        self.trace.set_enabled(enabled)?;
                        self.research.capture_enabled = enabled;
                        self.research.note(
                            "capture_switch",
                            json!({"enabled":self.research.capture_enabled}),
                            &mut self.trace,
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
                    self.input_event(event, Operation::Stable)?;
                }
            }
            _ if !self.replay_active => self.input_event(event, Operation::Stable)?,
            _ => {}
        }
        Ok(())
    }
}

impl ApplicationHandler for NativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| {
            if self.config.timings {
                eprintln!("TextFlowLarge startup: creating window");
            }
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
            if self.config.timings {
                eprintln!("TextFlowLarge startup: creating GPU renderer");
            }
            self.renderer = Some(VelloRenderer::new(host.window_arc(), host.surface_info())?);
            if self.config.timings {
                eprintln!("TextFlowLarge startup: GPU renderer ready");
            }
            self.renderer
                .as_mut()
                .expect("renderer created")
                .set_text_raster_cache_enabled(self.config.text_raster_cache);
            self.research.runtime.frames().reset_backend_session()?;
            let adapter = self.renderer.as_ref().unwrap().adapter_info();
            self.trace.push(
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
            if self.occluded && !self.config.offscreen_gpu {
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
                self.trace
                    .push(json!({"kind":"replay_complete","cycles":self.config.cycles}));
                self.export()?;
                if self.config.exit_after_replay {
                    event_loop.exit();
                    return Ok(());
                }
            }
            if self.frame_pending || !self.queue.is_empty() {
                if let Some(deadline) = self.frame_recovery.redraw_after
                    && Instant::now() < deadline
                {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                } else {
                    self.frame_recovery.redraw_after = None;
                    event_loop.set_control_flow(ControlFlow::Poll);
                    self.host.as_ref().unwrap().request_redraw();
                }
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
