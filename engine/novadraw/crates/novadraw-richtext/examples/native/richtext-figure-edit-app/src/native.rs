use crate::{Config, Result, editor::PAYLOAD, embedded::Embedded};
use novadraw::{
    event::{KeyModifiers, MonotonicTime, MouseButton},
    geometry::Point,
    host::PlatformHost,
    render::RenderBackend,
    runtime::{FramePreparation, SubmissionAcknowledgement},
};
use novadraw_editor::TextInputEvent;
use novadraw_example_support::app::{VelloRenderer, WindowScene};
use novadraw_platform_winit::{WinitPlatformHost, WinitTextInputBridge, adapt_modifiers};
use std::{io, path::Path, sync::Arc, time::Instant};
use winit::{
    event::{ElementState, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

pub struct Native {
    editor: Embedded,
    host: WinitPlatformHost,
    renderer: Option<VelloRenderer>,
    input: WinitTextInputBridge,
    position: Option<Point>,
    modifiers: KeyModifiers,
    focused: bool,
    record_ns: u128,
    commands: usize,
}
impl Native {
    pub fn new(window: Arc<Window>, config: &Config) -> Result<Self> {
        let host = WinitPlatformHost::new(window);
        let renderer = VelloRenderer::new(host.window_arc(), host.surface_info())?;
        let mut scene = Self {
            editor: Embedded::new(config)?,
            host,
            renderer: Some(renderer),
            input: WinitTextInputBridge::new(),
            position: None,
            modifiers: Default::default(),
            focused: true,
            record_ns: 0,
            commands: 0,
        };
        scene.resize()?;
        if config.replay {
            scene.editor.start()?;
            scene.effects();
            let source = scene.editor.viewer.model().document.version();
            for i in 0..9 {
                scene.editor.locate(i % 3)?;
                scene.editor.input(TextInputEvent::Preedit {
                    text: PAYLOAD.into(),
                    selection: Some(0..PAYLOAD.len()),
                })?;
                if scene.editor.viewer.model().document.version() != source {
                    return Err("preedit changed model".into());
                }
                scene
                    .editor
                    .input(TextInputEvent::InsertText(PAYLOAD.into()))?;
            }
            scene.editor.input(TextInputEvent::Accept)?;
            scene.editor.domain.undo(&mut scene.editor.viewer)?;
            if scene.editor.viewer.model().document.version() != source {
                return Err("undo did not restore immutable document".into());
            }
            scene.editor.domain.redo(&mut scene.editor.viewer)?;
            scene.editor.locate(0)?;
            scene.effects();
        }
        scene.status();
        Ok(scene)
    }
    fn effects(&mut self) {
        for effect in self.editor.viewer.take_text_input_effects() {
            self.input.apply_effect(self.host.window(), &effect);
        }
    }
    fn resize(&mut self) -> Result<()> {
        let s = self.host.surface_info();
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(s.pixel_width, s.pixel_height, s.scale_factor);
        }
        self.editor.resize(s.logical_width, s.logical_height)?;
        self.effects();
        Ok(())
    }
    fn status(&self) {
        self.host.window().set_title(&format!(
            "Richtext · Editor 会话 · {} chars · 输入+反馈 {:.2} ms · 记录 {:.2} ms / {} commands · Ctrl+Enter 提交 · Esc 取消",
            self.editor.metadata["characters"], self.editor.timing.mutation_ns as f64/1e6,
            self.record_ns as f64/1e6, self.commands));
    }
    fn draw(&mut self, present: bool) -> Result<bool> {
        let Some(renderer) = &mut self.renderer else {
            return Ok(false);
        };
        let start = Instant::now();
        let frame = match self
            .editor
            .viewer
            .drive()
            .prepare_submission(self.host.surface_info(), renderer.capabilities())
        {
            FramePreparation::Ready(frame) => frame,
            FramePreparation::Idle => return Ok(false),
            other => return Err(format!("native preparation: {other:?}").into()),
        };
        self.record_ns = start.elapsed().as_nanos();
        self.commands = frame.commands.len();
        let result = if present {
            self.host.window().pre_present_notify();
            renderer.submit(&frame)
        } else {
            renderer.render_for_screenshot(&frame)
        };
        let outcome = self.editor.viewer.drive().complete_submission(
            frame.session_id,
            frame.frame_id,
            result.clone(),
        )?;
        match outcome {
            SubmissionAcknowledgement::Presented => {}
            SubmissionAcknowledgement::Skipped
            | SubmissionAcknowledgement::Retry
            | SubmissionAcknowledgement::SurfaceInvalidated { .. } => self.host.request_redraw(),
            other => return Err(format!("native publication: {other:?}; {result:?}").into()),
        }
        self.effects();
        self.status();
        Ok(outcome == SubmissionAcknowledgement::Presented)
    }
    fn event(&mut self, event: &WindowEvent) -> Result<()> {
        let scale = self.host.window().scale_factor();
        match event {
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.resize()?,
            WindowEvent::CursorMoved { position, .. } => {
                let p = Point::new(position.x / scale, position.y / scale);
                self.position = Some(p);
                self.editor
                    .domain
                    .pointer_moved(&mut self.editor.viewer, p)?;
            }
            WindowEvent::CursorLeft { .. } => {
                self.position = None;
                self.editor.domain.pointer_exited(&mut self.editor.viewer);
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(p) = self.position {
                    if *state == ElementState::Pressed {
                        self.editor.start()?;
                        self.editor.domain.pointer_pressed(
                            &mut self.editor.viewer,
                            p,
                            MouseButton::Left,
                            self.modifiers,
                        )?;
                    } else {
                        self.editor.domain.pointer_released(
                            &mut self.editor.viewer,
                            p,
                            MouseButton::Left,
                        )?;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -f64::from(*y) * 24.0,
                    MouseScrollDelta::PixelDelta(p) => -p.y / scale,
                };
                let origin = self.editor.viewer.viewport_origin()?;
                self.editor.domain.set_viewport_origin(
                    &mut self.editor.viewer,
                    Point::new(origin.x(), (origin.y() + dy).max(0.0)),
                )?;
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = adapt_modifiers(m.state()),
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
                if !focused {
                    if self.editor.viewer.edit_session().is_some() {
                        self.editor.input(TextInputEvent::FocusLost)?;
                    }
                    self.editor.viewer.drive().cancel_gestures();
                    self.editor.viewer.drive().release_focus();
                }
            }
            WindowEvent::Ime(ime) => {
                if let Some(input) = self.input.adapt_ime(ime.clone()) {
                    self.handle_input(input)?;
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && self.focused =>
            {
                if !self.input.is_composing() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::F2 | KeyCode::F3 | KeyCode::F4) => {
                            let region = match event.physical_key {
                                PhysicalKey::Code(KeyCode::F2) => 0,
                                PhysicalKey::Code(KeyCode::F3) => 1,
                                _ => 2,
                            };
                            self.editor.locate(region)?;
                            self.editor
                                .input(TextInputEvent::InsertText(PAYLOAD.into()))?;
                        }
                        PhysicalKey::Code(KeyCode::F5) => {
                            for i in 0..9 {
                                self.editor.locate(i % 3)?;
                                self.editor
                                    .input(TextInputEvent::InsertText(PAYLOAD.into()))?;
                            }
                        }
                        PhysicalKey::Code(KeyCode::F6) => self.editor.locate(0)?,
                        PhysicalKey::Code(KeyCode::F7) => self.editor.locate(2)?,
                        PhysicalKey::Code(KeyCode::KeyZ)
                            if self.modifiers.control || self.modifiers.meta =>
                        {
                            if self.editor.viewer.edit_session().is_some() {
                                self.editor.input(TextInputEvent::Accept)?;
                            }
                            if self.modifiers.shift {
                                self.editor.domain.redo(&mut self.editor.viewer)?;
                            } else {
                                self.editor.domain.undo(&mut self.editor.viewer)?;
                            }
                        }
                        _ => self.key(event.physical_key, event.text.as_deref())?,
                    }
                } else {
                    self.key(event.physical_key, event.text.as_deref())?;
                }
            }
            _ => return Ok(()),
        }
        self.effects();
        self.status();
        self.host.request_redraw();
        Ok(())
    }
    fn key(&mut self, key: PhysicalKey, text: Option<&str>) -> Result<()> {
        self.editor.start()?;
        self.effects();
        if let Some(input) = self.input.adapt_key_pressed(key, text, self.modifiers) {
            self.handle_input(input)?;
        }
        Ok(())
    }
    fn handle_input(&mut self, input: novadraw_editor::SessionTextInputEvent) -> Result<()> {
        let start = Instant::now();
        self.editor
            .domain
            .handle_text_input_event(&mut self.editor.viewer, input)?;
        self.editor.timing = crate::editor::Timing {
            mutation_ns: start.elapsed().as_nanos(),
            layout_ns: 0,
        };
        Ok(())
    }
}
fn io_result(result: Result<()>) -> io::Result<()> {
    result.map_err(|e| io::Error::other(e.to_string()))
}
impl WindowScene for Native {
    fn advance_time(&mut self, now: MonotonicTime) -> io::Result<bool> {
        self.editor
            .viewer
            .drive()
            .advance_time(now)
            .map_err(|e| io::Error::other(e.to_string()))
    }
    fn next_wake_deadline(&mut self) -> Option<MonotonicTime> {
        self.editor.viewer.drive().next_wake_deadline()
    }
    fn render(&mut self, present: bool) -> io::Result<()> {
        io_result(self.draw(present).map(|_| ()))
    }
    fn window_event(&mut self, event: &WindowEvent) -> io::Result<()> {
        match self.event(event) {
            Err(error)
                if !self.editor.viewer.is_faulted()
                    && error
                        .downcast_ref::<novadraw_editor::EditorDomainError>()
                        .is_some() =>
            {
                self.effects();
                self.host
                    .window()
                    .set_title(&format!("Richtext · 输入未接受：{error}"));
                self.host.request_redraw();
                Ok(())
            }
            result => io_result(result),
        }
    }
    fn resume(&mut self) -> io::Result<()> {
        io_result((|| {
            self.renderer = Some(VelloRenderer::new(
                self.host.window_arc(),
                self.host.surface_info(),
            )?);
            self.editor.viewer.drive().reset_backend_session()?;
            self.resize()
        })())
    }
    fn suspend(&mut self) -> io::Result<()> {
        self.renderer = None;
        Ok(())
    }
    fn screenshot(&mut self, path: &Path) -> io::Result<()> {
        io_result((|| {
            self.editor.viewer.drive().request_full_redraw();
            if !self.draw(false)? {
                return Err("offscreen frame not presented".into());
            }
            let renderer = self.renderer.as_ref().ok_or("renderer suspended")?;
            renderer.wait_for_gpu_idle(std::time::Duration::from_secs(10))?;
            renderer.screenshot(path)?;
            Ok(())
        })())
    }
}
