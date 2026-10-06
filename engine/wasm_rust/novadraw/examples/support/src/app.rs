//! 应用框架
//!
//! 提供通用的演示应用构建和运行功能。

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub use novadraw::event::{
    FigureEvent, Key, KeyModifiers, ListenerDirective, MonotonicTime, MouseButton,
    NotificationEffect, UpdateEvent, UpdateListener, place_tooltip,
};
pub use novadraw::render::command::RenderCommand;
pub use novadraw::render::text::{FontDescriptor, TextConstraints};
pub use novadraw::render::{BackendCapabilities, RenderOutcome, SurfaceInfo};
pub use novadraw::{
    Color, FigureId, FigureTree, FramePreparation, NdCanvas, PlatformHost, Rectangle,
    RenderBackend, Runtime,
};
pub use novadraw_backend_vello::VelloRenderer;
use novadraw_platform_winit::{
    AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, WinitPlatformHost, adapt_key_input,
    adapt_modifiers, adapt_mouse_button, adapt_physical_key,
};
pub use winit::dpi::{LogicalSize, PhysicalSize};
pub use winit::event::WindowEvent;
pub use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
pub use winit::keyboard::{KeyCode, PhysicalKey};
pub use winit::window::WindowAttributes;
pub use winit::{application::ApplicationHandler, window::WindowId};

use tracing::{error, info};

const SCREENSHOT_RENDER_RETRY_LIMIT: usize = 8;
const INITIAL_FRAME_RETRY_LIMIT: usize = 8;
const TOOLTIP_HORIZONTAL_PADDING: f64 = 8.0;
const TOOLTIP_VERTICAL_PADDING: f64 = 6.0;
const TOOLTIP_BACKGROUND: Color = Color::rgba(0.09, 0.13, 0.18, 1.0);
const TOOLTIP_FOREGROUND: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const TOOLTIP_BORDER: Color = Color::rgba(0.45, 0.5, 0.56, 1.0);
const TOOLTIP_BORDER_WIDTH: f64 = 1.0;

/// 演示应用
///
/// 提供通用的演示应用框架，自动处理：
/// - 窗口创建
/// - 渲染器初始化
/// - 场景切换
/// - 事件处理
///
// 场景创建函数类型
type SceneCreator = Box<dyn FnMut() -> Runtime>;

struct DemoUpdateListener;

impl UpdateListener for DemoUpdateListener {
    fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
        tracing::debug!("[DemoApp] update event: {:?}", event);
        ListenerDirective::Keep
    }

    fn on_figure_event(&self, event: FigureEvent) -> ListenerDirective {
        tracing::debug!("[DemoApp] figure event: {:?}", event);
        ListenerDirective::Keep
    }

    fn on_notify(&self, figure_id: FigureId) -> ListenerDirective {
        tracing::debug!("[DemoApp] notify: {:?}", figure_id);
        ListenerDirective::Keep
    }
}

pub struct DemoApp {
    #[allow(clippy::type_complexity)]
    scenes: Vec<(&'static str, SceneCreator)>,
    current_scene_idx: usize,
    runtime: Option<Runtime>,
    renderer: Option<VelloRenderer>,
    host: Option<WinitPlatformHost>,
    title: String,
    app_name: String,
    width: f64,
    height: f64,
    use_update_manager: bool,
    screenshot_mode: Option<usize>,
    screenshot_attempts: usize,
    cursor_position: Option<(f64, f64)>,
    modifiers: KeyModifiers,
    gesture_adapter: WinitGestureAdapter,
    clock_origin: Instant,
    initial_frame_presented: bool,
    initial_frame_attempts: usize,
}

impl DemoApp {
    /// 创建一个新的演示应用
    #[allow(clippy::type_complexity)]
    pub fn new(
        title: &str,
        scenes: Vec<(&'static str, SceneCreator)>,
        width: f64,
        height: f64,
        app_name: &str,
        screenshot_mode: Option<usize>,
    ) -> Self {
        let default_scene = screenshot_mode
            .filter(|mode| *mode != usize::MAX)
            .unwrap_or_else(|| if screenshot_mode.is_some() { 0 } else { 4 });
        Self {
            scenes,
            current_scene_idx: default_scene,
            runtime: None,
            renderer: None,
            host: None,
            title: title.to_string(),
            app_name: app_name.to_string(),
            width,
            height,
            use_update_manager: true,
            screenshot_mode,
            screenshot_attempts: 0,
            cursor_position: None,
            modifiers: KeyModifiers::default(),
            gesture_adapter: WinitGestureAdapter::new(),
            clock_origin: Instant::now(),
            initial_frame_presented: false,
            initial_frame_attempts: 0,
        }
    }

    /// 切换到指定场景
    pub fn switch_scene(&mut self, idx: usize) {
        if idx < self.scenes.len() {
            if let Some(runtime) = &mut self.runtime {
                runtime.pointer_exited();
            }
            self.sync_platform_effects();
            self.current_scene_idx = idx;
            let creator = &mut self.scenes[idx].1;
            let mut runtime = creator();
            runtime.add_update_listener(Box::new(DemoUpdateListener));
            self.runtime = Some(runtime);
            self.sync_logical_viewport();
            self.initial_frame_presented = false;
            self.initial_frame_attempts = 0;
            eprintln!("切换到场景: {}", self.scenes[idx].0);

            // 更新窗口标题显示当前场景（只显示场景名称）
            let scene_name = self.scenes[idx].0;
            let new_title = format!("{} - {}", self.title, scene_name);
            eprintln!("设置窗口标题: {}", new_title);
            if let Some(host) = &self.host {
                host.window().set_title(&new_title);
                host.request_redraw();
            }
        }
    }

    /// 获取当前场景名称
    pub fn current_scene_name(&self) -> Option<&'static str> {
        self.scenes
            .get(self.current_scene_idx)
            .map(|(name, _)| *name)
    }

    /// 获取场景数量
    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    /// 渲染当前场景
    pub fn render(&mut self) -> RenderOutcome {
        let Some(host) = &self.host else {
            return RenderOutcome::Skipped;
        };
        let Some(renderer) = &mut self.renderer else {
            return RenderOutcome::Skipped;
        };
        let Some(runtime) = &mut self.runtime else {
            return RenderOutcome::Skipped;
        };

        let surface = host.surface_info();
        if !self.use_update_manager {
            runtime.request_full_redraw();
        }
        if runtime.visible_tooltip().is_some() {
            runtime.request_full_redraw();
        }
        let tooltip_commands = tooltip_overlay_commands(runtime, surface);
        let mut submission = match runtime.prepare_submission(surface, renderer.capabilities()) {
            FramePreparation::Ready(submission) => submission,
            FramePreparation::Error(_) => {
                self.sync_platform_effects();
                return RenderOutcome::Retry;
            }
            FramePreparation::Idle
            | FramePreparation::Suspended
            | FramePreparation::AwaitingCompletion => {
                self.sync_platform_effects();
                return RenderOutcome::Skipped;
            }
        };
        submission.commands.extend(tooltip_commands);
        let outcome = renderer.submit(&submission);
        runtime.complete_submission(submission.session_id, submission.frame_id, outcome);
        if outcome == RenderOutcome::Retry {
            host.request_redraw();
        }
        self.record_interactive_frame_outcome(outcome);
        self.sync_platform_effects();
        outcome
    }

    fn render_screenshot_frame(&mut self) -> RenderOutcome {
        let Some(host) = &self.host else {
            return RenderOutcome::Skipped;
        };
        let Some(renderer) = &mut self.renderer else {
            return RenderOutcome::Skipped;
        };
        let Some(runtime) = &mut self.runtime else {
            return RenderOutcome::Skipped;
        };
        runtime.request_full_redraw();
        let surface = host.surface_info();
        let tooltip_commands = tooltip_overlay_commands(runtime, surface);
        debug_assert!(
            runtime.visible_tooltip().is_none() || !tooltip_commands.is_empty(),
            "a visible tooltip must lower to native overlay commands"
        );
        let mut submission = match runtime.prepare_submission(surface, renderer.capabilities()) {
            FramePreparation::Ready(submission) => submission,
            FramePreparation::Error(_) => {
                self.sync_platform_effects();
                return RenderOutcome::Retry;
            }
            FramePreparation::Idle
            | FramePreparation::Suspended
            | FramePreparation::AwaitingCompletion => {
                self.sync_platform_effects();
                return RenderOutcome::Skipped;
            }
        };
        submission.commands.extend(tooltip_commands);
        let outcome = renderer.render_for_screenshot(&submission);
        runtime.complete_submission(submission.session_id, submission.frame_id, outcome);
        self.sync_platform_effects();
        outcome
    }

    fn dispatch_input(&mut self, action: impl FnOnce(&mut Runtime)) {
        let now = self.monotonic_time();
        let Some(runtime) = &mut self.runtime else {
            return;
        };
        let _ = runtime.advance_time(now);
        action(runtime);
        self.sync_platform_effects();
        if let Some(host) = &self.host {
            host.request_redraw();
        }
    }

    fn monotonic_time(&self) -> MonotonicTime {
        let micros = self.clock_origin.elapsed().as_micros();
        MonotonicTime::from_micros(u64::try_from(micros).unwrap_or(u64::MAX))
    }

    fn sync_platform_effects(&mut self) {
        let (Some(runtime), Some(host)) = (&mut self.runtime, &self.host) else {
            return;
        };
        host.set_cursor(runtime.cursor_icon());
        let tooltip_updates = runtime.take_tooltip_updates();
        if !tooltip_updates.is_empty() {
            runtime.request_full_redraw();
        }
        for update in tooltip_updates {
            host.update_tooltip(update);
        }
        for update in runtime.take_accessibility_updates() {
            host.update_accessibility(update);
        }
        host.schedule_wake(runtime.next_wake_deadline());
    }

    fn request_surface_redraw(&mut self) {
        self.sync_logical_viewport();
        if let Some(runtime) = &mut self.runtime {
            // A surface size change invalidates retained pixels even when scene state is unchanged.
            runtime.request_full_redraw();
        }
        if let Some(host) = &self.host {
            host.request_redraw();
        }
    }

    fn sync_logical_viewport(&mut self) {
        let Some(surface) = self.host.as_ref().map(PlatformHost::surface_info) else {
            return;
        };
        if let Some(runtime) = &mut self.runtime {
            runtime
                .resize_logical_viewport(surface.logical_width, surface.logical_height)
                .expect("platform surface must provide a valid logical viewport");
        }
    }

    /// 截图并保存到文件
    ///
    /// 使用 VelloRenderer 直接捕获渲染结果
    /// 截图保存到 `target/visual-verification/screenshots/`。
    pub fn screenshot(&self, scene_name: &str) -> std::io::Result<std::path::PathBuf> {
        let screenshot_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("target/visual-verification/screenshots");
        std::fs::create_dir_all(&screenshot_dir)?;

        // 生成文件名：{app_name}_{scene}_{timestamp}.png
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let safe_scene_name: String = scene_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let filename = format!("{}_{}_{}.png", self.app_name, safe_scene_name, timestamp);
        let output_path = screenshot_dir.join(&filename);

        // 使用 VelloRenderer 捕获渲染结果
        if let Some(renderer) = &self.renderer {
            renderer.screenshot(&output_path)?;
            info!("截图已保存: {}", output_path.display());
            Ok(output_path)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Renderer not initialized",
            ))
        }
    }

    /// 处理截图模式
    fn handle_screenshot_mode(&mut self, mode: usize, event_loop: &ActiveEventLoop) {
        let idx = self.current_scene_idx;
        if idx >= self.scenes.len() {
            error!("场景索引 {} 超出范围", idx);
            std::process::exit(1);
        }

        info!("截图场景 {}...", idx);
        let presented = self.render_screenshot_frame() == RenderOutcome::Presented;
        if !presented {
            self.screenshot_attempts += 1;
            if self.screenshot_attempts >= SCREENSHOT_RENDER_RETRY_LIMIT {
                error!("场景 {} 未产生可截图帧", self.scenes[idx].0);
                std::process::exit(1);
            }
            if let Some(host) = &self.host {
                host.request_redraw();
            }
            return;
        }
        self.screenshot_attempts = 0;

        let scene_name = self.scenes[idx].0;
        match self.screenshot(scene_name) {
            Ok(path) => {
                info!("截图成功: {}", path.display());
                self.analyze_screenshot(&path, scene_name);
            }
            Err(e) => {
                error!("截图失败: {}", e);
                std::process::exit(1);
            }
        }

        if mode == usize::MAX && idx + 1 < self.scenes.len() {
            self.switch_scene(idx + 1);
            return;
        }
        info!("截图完成，应用退出");
        self.screenshot_mode = None;
        event_loop.exit();
    }

    /// 分析截图结果
    fn analyze_screenshot(&self, path: &std::path::Path, scene_name: &str) {
        info!("分析截图: {} - {}", scene_name, path.display());
        // 这里可以添加图像分析逻辑
        // 例如：检查图像是否存在、尺寸等
    }
}

impl ApplicationHandler<()> for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        let is_backend_replacement = self.host.is_some();
        let window = self
            .host
            .as_ref()
            .map(WinitPlatformHost::window_arc)
            .unwrap_or_else(|| {
                Arc::new(
                    event_loop
                        .create_window(
                            WindowAttributes::default()
                                .with_title(&self.title)
                                .with_inner_size(LogicalSize::new(self.width, self.height))
                                .with_resizable(true),
                        )
                        .unwrap(),
                )
            });
        if self.host.is_none() {
            self.host = Some(WinitPlatformHost::new(window.clone()));
        }

        let scale_factor = window.scale_factor();
        let renderer = match VelloRenderer::new(
            window,
            SurfaceInfo {
                logical_width: self.width,
                logical_height: self.height,
                pixel_width: (self.width * scale_factor).round() as u32,
                pixel_height: (self.height * scale_factor).round() as u32,
                scale_factor,
            },
        ) {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("failed to initialize Vello renderer: {error}");
                event_loop.exit();
                return;
            }
        };
        self.renderer = Some(renderer);
        self.initial_frame_presented = false;
        self.initial_frame_attempts = 0;
        if is_backend_replacement && let Some(runtime) = &mut self.runtime {
            runtime
                .reset_backend_session()
                .expect("backend session id space exhausted");
        }

        // 创建初始场景（通过 switch_scene 以更新窗口标题）
        if self.runtime.is_none() && !self.scenes.is_empty() {
            let idx = self.current_scene_idx.min(self.scenes.len() - 1);
            self.switch_scene(idx);
        } else if let Some(host) = &self.host {
            host.request_redraw();
        }

        info!("应用启动: {}", self.title);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self.screenshot_mode.is_some()
            && matches!(
                &event,
                WindowEvent::CursorMoved { .. }
                    | WindowEvent::CursorLeft { .. }
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::PinchGesture { .. }
                    | WindowEvent::KeyboardInput { .. }
                    | WindowEvent::ModifiersChanged(_)
                    | WindowEvent::Focused(_)
            )
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if self.screenshot_mode.is_some() {
                    return;
                }
                let _ = self.render();
            }
            WindowEvent::Resized(_) => {
                self.request_surface_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some((position.x, position.y));
                let scale_factor = self
                    .host
                    .as_ref()
                    .map(|host| host.window().scale_factor())
                    .unwrap_or(1.0);
                self.dispatch_input(|runtime| {
                    runtime
                        .dispatch_mouse_moved(position.x / scale_factor, position.y / scale_factor);
                });
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
                self.dispatch_input(Runtime::pointer_exited);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some((x, y)) = self.cursor_position else {
                    return;
                };
                let scale_factor = self
                    .host
                    .as_ref()
                    .map(|host| host.window().scale_factor())
                    .unwrap_or(1.0);
                let button = adapt_mouse_button(button);
                self.dispatch_input(|runtime| match state {
                    winit::event::ElementState::Pressed => {
                        runtime.dispatch_mouse_pressed(x / scale_factor, y / scale_factor, button);
                    }
                    winit::event::ElementState::Released => {
                        runtime.dispatch_mouse_released(x / scale_factor, y / scale_factor, button);
                    }
                });
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                let scale_factor = self
                    .host
                    .as_ref()
                    .map(|host| host.window().scale_factor())
                    .unwrap_or(1.0);
                let (physical_x, physical_y) = self.cursor_position.unwrap_or_else(|| {
                    let size = self
                        .host
                        .as_ref()
                        .map(|host| host.window().inner_size())
                        .unwrap_or(PhysicalSize::new(0, 0));
                    (f64::from(size.width) / 2.0, f64::from(size.height) / 2.0)
                });
                let Some(gesture) = self.gesture_adapter.adapt_mouse_wheel(
                    device_id,
                    delta,
                    phase,
                    physical_x,
                    physical_y,
                    scale_factor,
                    self.modifiers,
                ) else {
                    return;
                };
                self.dispatch_input(|runtime| {
                    if let AdaptedGesture::Scroll(event) = gesture {
                        runtime.dispatch_scroll(event);
                    }
                });
            }
            WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => {
                let scale_factor = self
                    .host
                    .as_ref()
                    .map(|host| host.window().scale_factor())
                    .unwrap_or(1.0);
                let (physical_x, physical_y) = self.cursor_position.unwrap_or_else(|| {
                    let size = self
                        .host
                        .as_ref()
                        .map(|host| host.window().inner_size())
                        .unwrap_or(PhysicalSize::new(0, 0));
                    (f64::from(size.width) / 2.0, f64::from(size.height) / 2.0)
                });
                let Some(gesture) = self.gesture_adapter.adapt_pinch(
                    device_id,
                    delta,
                    phase,
                    physical_x,
                    physical_y,
                    scale_factor,
                    self.modifiers,
                ) else {
                    return;
                };
                self.dispatch_input(|runtime| {
                    if let AdaptedGesture::Zoom(event) = gesture {
                        runtime.dispatch_zoom(event);
                    }
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = adapt_modifiers(modifiers.state());
            }
            WindowEvent::Focused(false) => {
                self.gesture_adapter.cancel_all();
                self.dispatch_input(|runtime| {
                    runtime.cancel_gestures();
                    runtime.pointer_exited();
                    runtime.release_focus();
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == winit::event::ElementState::Pressed;
                if adapt_physical_key(event.physical_key) == Some(Key::Tab) {
                    if let AdaptedKeyInput::FocusTraversal(direction) =
                        adapt_key_input(Key::Tab, pressed, self.modifiers)
                    {
                        self.dispatch_input(|runtime| {
                            runtime.traverse_focus(direction);
                        });
                    }
                    return;
                }
                if !pressed {
                    if let Some(key) = adapt_physical_key(event.physical_key) {
                        let modifiers = self.modifiers;
                        self.dispatch_input(|runtime| {
                            runtime.dispatch_key_released(key, modifiers);
                        });
                    }
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        event_loop.exit();
                    }
                    PhysicalKey::Code(KeyCode::KeyU) => {
                        self.use_update_manager = !self.use_update_manager;
                        info!(
                            "更新管理器: {}",
                            if self.use_update_manager {
                                "启用 (两阶段更新 + 通知)"
                            } else {
                                "禁用 (直接渲染)"
                            }
                        );
                        self.host.as_ref().unwrap().request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::KeyS) => {
                        // 截图
                        let scene_name = self.current_scene_name().unwrap_or("unknown");
                        if let Err(e) = self.screenshot(scene_name) {
                            error!("截图失败: {}", e);
                        }
                    }
                    // 左右方向键 / PageUp/PageDown 循环切换场景
                    PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::PageUp) => {
                        // 切换到上一个场景（循环）
                        let count = self.scenes.len();
                        if count > 0 {
                            let new_idx = if self.current_scene_idx == 0 {
                                count - 1
                            } else {
                                self.current_scene_idx - 1
                            };
                            self.switch_scene(new_idx);
                            self.host.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight)
                    | PhysicalKey::Code(KeyCode::PageDown) => {
                        // 切换到下一个场景（循环）
                        let count = self.scenes.len();
                        if count > 0 {
                            let new_idx = (self.current_scene_idx + 1) % count;
                            self.switch_scene(new_idx);
                            self.host.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        // 切换到第一个场景
                        if !self.scenes.is_empty() {
                            self.switch_scene(0);
                            self.host.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::End) => {
                        // 切换到最后一个场景
                        let count = self.scenes.len();
                        if count > 0 {
                            self.switch_scene(count - 1);
                            self.host.as_ref().unwrap().request_redraw();
                        }
                    }
                    // 数字键 0-9 切换场景
                    _ => {
                        if let Some(digit) = get_digit_index(&event.physical_key) {
                            self.switch_scene(digit);
                            self.host.as_ref().unwrap().request_redraw();
                        } else if let Some(key) = adapt_physical_key(event.physical_key) {
                            let modifiers = self.modifiers;
                            self.dispatch_input(|runtime| {
                                runtime.dispatch_key_pressed(key, modifiers);
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(screenshot_mode) = self.screenshot_mode {
            // Some platforms coalesce redraw requests issued from RedrawRequested.
            // Drive batch capture from the event-loop boundary instead.
            event_loop.set_control_flow(ControlFlow::Poll);
            self.handle_screenshot_mode(screenshot_mode, event_loop);
            return;
        }
        let now = self.monotonic_time();
        let time_changed = self
            .runtime
            .as_mut()
            .is_some_and(|runtime| runtime.advance_time(now).unwrap_or(false));
        self.sync_platform_effects();
        if let Some(host) = &self.host {
            if !self.initial_frame_presented
                && self.initial_frame_attempts < INITIAL_FRAME_RETRY_LIMIT
            {
                event_loop.set_control_flow(ControlFlow::Poll);
                host.request_redraw();
                return;
            }
            if let Some(deadline) = host.wake_deadline() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    self.clock_origin + Duration::from_micros(deadline.as_micros()),
                ));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            if time_changed {
                host.request_redraw();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(runtime) = &mut self.runtime {
            runtime.pointer_exited();
        }
        self.sync_platform_effects();
        self.renderer = None;
    }
}

impl DemoApp {
    fn record_interactive_frame_outcome(&mut self, outcome: RenderOutcome) {
        if self.initial_frame_presented {
            return;
        }
        if outcome == RenderOutcome::Presented {
            self.initial_frame_presented = true;
            self.initial_frame_attempts = 0;
        } else {
            self.initial_frame_attempts = self.initial_frame_attempts.saturating_add(1);
        }
    }
}

fn tooltip_overlay_commands(runtime: &mut Runtime, surface: SurfaceInfo) -> Vec<RenderCommand> {
    let Some(snapshot) = runtime.visible_tooltip().cloned() else {
        return Vec::new();
    };
    let Some(style) = runtime.tree().resolved_style(snapshot.source) else {
        return Vec::new();
    };
    let Ok(font) = FontDescriptor::parse(&style.font) else {
        return Vec::new();
    };
    let Ok(layout) = runtime.layout_text(&snapshot.text, &font, TextConstraints::UNBOUNDED) else {
        return Vec::new();
    };
    let popup_size = (
        f64::from(layout.width()) + TOOLTIP_HORIZONTAL_PADDING * 2.0,
        f64::from(layout.height()) + TOOLTIP_VERTICAL_PADDING * 2.0,
    );
    let Some(bounds) = place_tooltip(
        snapshot.anchor,
        popup_size,
        Rectangle::new(0.0, 0.0, surface.logical_width, surface.logical_height),
        snapshot.placement,
    ) else {
        return Vec::new();
    };

    let mut canvas = NdCanvas::new();
    canvas.push_state();
    canvas.reset_transform();
    canvas.set_alpha(1.0);
    canvas.fill_rect_with_color(
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        TOOLTIP_BORDER,
    );
    canvas.fill_rect_with_color(
        bounds.x + TOOLTIP_BORDER_WIDTH,
        bounds.y + TOOLTIP_BORDER_WIDTH,
        (bounds.width - TOOLTIP_BORDER_WIDTH * 2.0).max(0.0),
        (bounds.height - TOOLTIP_BORDER_WIDTH * 2.0).max(0.0),
        TOOLTIP_BACKGROUND,
    );
    canvas.set_background_color(TOOLTIP_FOREGROUND);
    canvas.fill_text_layout(
        &layout,
        bounds.x + TOOLTIP_HORIZONTAL_PADDING,
        bounds.y + TOOLTIP_VERTICAL_PADDING,
    );
    canvas.pop_state();
    canvas.commands().to_vec()
}

/// 从按键获取数字索引（0-9）
fn get_digit_index(key: &winit::keyboard::PhysicalKey) -> Option<usize> {
    match key {
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit0) => Some(0),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit1) => Some(1),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit2) => Some(2),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit3) => Some(3),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit4) => Some(4),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit5) => Some(5),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit6) => Some(6),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit7) => Some(7),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit8) => Some(8),
        winit::keyboard::PhysicalKey::Code(KeyCode::Digit9) => Some(9),
        _ => None,
    }
}

/// 应用构建器
///
/// 用于更灵活地配置应用
#[allow(clippy::type_complexity)]
pub struct AppBuilder {
    title: String,
    app_name: String,
    scenes: Vec<(&'static str, SceneCreator)>,
    width: f64,
    height: f64,
    /// 截图模式：None=正常模式, Some(true)=截图所有场景, Some(数字)=截图指定场景
    screenshot_mode: Option<usize>,
}

impl AppBuilder {
    /// 创建一个新的构建器
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            app_name: String::new(),
            scenes: Vec::new(),
            width: 800.0,
            height: 600.0,
            screenshot_mode: None,
        }
    }

    /// 设置窗口尺寸
    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// 设置应用名称（用于截图目录和文件名）
    pub fn with_app_name(mut self, name: &str) -> Self {
        self.app_name = name.to_string();
        self
    }

    /// 添加场景
    pub fn add_scene(
        mut self,
        name: &'static str,
        creator: impl FnMut() -> FigureTree + 'static,
    ) -> Self {
        let mut creator = creator;
        self.scenes
            .push((name, Box::new(move || Runtime::new(creator()))));
        self
    }

    /// 批量添加场景（已装箱）
    #[allow(clippy::type_complexity)]
    pub fn with_scenes_boxed(
        mut self,
        scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureTree>)>,
    ) -> Self {
        self.scenes = scenes
            .into_iter()
            .map(|(name, mut creator)| {
                (
                    name,
                    Box::new(move || Runtime::new(creator())) as SceneCreator,
                )
            })
            .collect();
        self
    }

    /// 批量添加由调用方显式配置 Runtime 的场景。
    #[allow(clippy::type_complexity)]
    pub fn with_runtime_scenes_boxed(
        mut self,
        scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    ) -> Self {
        self.scenes = scenes;
        self
    }

    /// 设置截图模式
    ///
    /// `screenshot_all`: true=截图所有场景, false=不截图
    pub fn with_screenshot(mut self, screenshot_all: bool) -> Self {
        if screenshot_all {
            self.screenshot_mode = Some(usize::MAX); // MAX 表示所有场景
        }
        self
    }

    /// 截图指定场景
    ///
    /// `scene_index`: 场景索引（从0开始）
    pub fn with_screenshot_scene(mut self, scene_index: usize) -> Self {
        self.screenshot_mode = Some(scene_index);
        self
    }

    /// 构建并运行应用
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let scenes: Vec<(_, SceneCreator)> = self.scenes;
        let app_name = if self.app_name.is_empty() {
            // 从 title 提取应用名称
            self.title
                .split_whitespace()
                .next()
                .unwrap_or("app")
                .to_string()
        } else {
            self.app_name
        };
        let mut app = DemoApp::new(
            &self.title,
            scenes,
            self.width,
            self.height,
            &app_name,
            self.screenshot_mode,
        );
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

/// 运行演示应用的简便函数
///
/// # 示例
///
/// ```rust,no_run
/// use novadraw_example_support::run_demo_app;
///
/// fn create_rect_scene() -> novadraw::Runtime {
///     let mut runtime = novadraw::Runtime::empty();
///     let rect = novadraw::RectangleFigure::new(100.0, 100.0, 200.0, 150.0);
///     runtime.set_contents(Box::new(rect)).expect("valid Runtime mutation");
///     runtime
/// }
///
/// fn main() {
///     run_demo_app("My App", "rect-demo", vec![
///         ("Rectangle", Box::new(|| create_rect_scene())),
///     ]).expect("demo app failed");
/// }
/// ```
#[allow(clippy::type_complexity)]
pub fn run_demo_app(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, false, None)
}

#[allow(clippy::type_complexity)]
pub fn run_demo_app_with_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    screenshot_all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, screenshot_all, None)
}

#[allow(clippy::type_complexity)]
pub fn run_demo_app_with_scene_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    scene_index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, false, Some(scene_index))
}

/// Runs demo scenes that explicitly configure their own [`Runtime`].
#[allow(clippy::type_complexity)]
pub fn run_runtime_demo_app(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, false, None)
}

/// Runs explicit Runtime demo scenes and captures one or all scenarios.
#[allow(clippy::type_complexity)]
pub fn run_runtime_demo_app_with_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    screenshot_all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, screenshot_all, None)
}

#[allow(clippy::type_complexity)]
pub fn run_runtime_demo_app_with_scene_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    scene_index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    run_runtime_demo_app_with_options(title, app_name, scenes, false, Some(scene_index))
}

#[allow(clippy::type_complexity)]
fn run_runtime_demo_app_with_options(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> Runtime>)>,
    screenshot_all: bool,
    screenshot_scene: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = AppBuilder::new(title)
        .with_size(800.0, 600.0)
        .with_app_name(app_name)
        .with_runtime_scenes_boxed(scenes);

    if screenshot_all {
        builder = builder.with_screenshot(true);
    } else if let Some(idx) = screenshot_scene {
        builder = builder.with_screenshot_scene(idx);
    }

    builder.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_frame_remains_pending_until_presentation_succeeds() {
        let mut app = DemoApp::new("test", Vec::new(), 100.0, 100.0, "test", None);

        app.record_interactive_frame_outcome(RenderOutcome::Skipped);
        assert!(!app.initial_frame_presented);
        assert_eq!(app.initial_frame_attempts, 1);

        app.record_interactive_frame_outcome(RenderOutcome::Retry);
        assert!(!app.initial_frame_presented);
        assert_eq!(app.initial_frame_attempts, 2);

        app.record_interactive_frame_outcome(RenderOutcome::Presented);
        assert!(app.initial_frame_presented);
        assert_eq!(app.initial_frame_attempts, 0);
    }

    #[test]
    fn visible_tooltip_lowers_to_native_overlay_commands() {
        use novadraw::event::TooltipTiming;
        use novadraw::render::command::RenderCommandKind;
        use novadraw::render::text::BuiltinFont;
        use novadraw::{FigureStyle, RectangleFigure};

        let mut runtime = Runtime::empty();
        runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        runtime
            .set_tooltip_timing(TooltipTiming::new(Duration::ZERO, Duration::from_secs(5)).unwrap())
            .unwrap();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)))
            .expect("valid Runtime mutation");
        assert!(
            runtime
                .figure(root)
                .unwrap()
                .set_style(FigureStyle {
                    font: Some("15px Inter Variable".to_string()),
                    tooltip: Some(Some("Native tooltip".to_string())),
                    ..FigureStyle::default()
                },)
                .expect("valid Runtime mutation")
        );
        runtime.dispatch_mouse_moved(300.0, 190.0);
        runtime.advance_time(MonotonicTime::ZERO).unwrap();
        let surface = SurfaceInfo {
            logical_width: 320.0,
            logical_height: 200.0,
            pixel_width: 640,
            pixel_height: 400,
            scale_factor: 2.0,
        };

        let commands = tooltip_overlay_commands(&mut runtime, surface);

        assert!(
            commands
                .iter()
                .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
        );
    }

    #[test]
    fn surface_change_requests_a_full_runtime_frame() {
        let mut app = DemoApp::new(
            "test",
            vec![("empty", Box::new(Runtime::empty))],
            800.0,
            600.0,
            "test",
            None,
        );
        app.switch_scene(0);

        let runtime = app.runtime.as_mut().unwrap();
        let submission = runtime
            .prepare_submission(
                SurfaceInfo {
                    logical_width: 800.0,
                    logical_height: 600.0,
                    pixel_width: 800,
                    pixel_height: 600,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL,
            )
            .into_ready()
            .unwrap();
        assert!(runtime.complete_submission(
            submission.session_id,
            submission.frame_id,
            RenderOutcome::Presented
        ));
        assert!(!runtime.has_pending_update());

        app.request_surface_redraw();

        assert!(app.runtime.as_ref().unwrap().has_pending_update());
    }

    #[test]
    fn unpresented_frame_restores_full_redraw_work() {
        let mut runtime = Runtime::empty();
        let surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        };
        let submission = runtime
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(!runtime.has_pending_update());

        assert!(runtime.complete_submission(
            submission.session_id,
            submission.frame_id,
            RenderOutcome::Skipped
        ));
        assert!(runtime.has_pending_update());

        let submission = runtime
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(runtime.complete_submission(
            submission.session_id,
            submission.frame_id,
            RenderOutcome::Presented
        ));
        assert!(!runtime.has_pending_update());
    }

    #[test]
    fn scene_switch_handoff_starts_each_runtime_with_a_snapshot_baseline() {
        use novadraw::render::submission::{
            BackendSessionDecision, BackendSessionGate, ResourceSync,
        };

        let mut app = DemoApp::new(
            "test",
            vec![
                ("first", Box::new(Runtime::empty)),
                ("second", Box::new(Runtime::empty)),
            ],
            100.0,
            100.0,
            "test",
            None,
        );
        let surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        };
        let mut gate = BackendSessionGate::default();

        app.switch_scene(0);
        let first = app
            .runtime
            .as_mut()
            .unwrap()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert!(matches!(&first.resources, ResourceSync::Snapshot(_)));
        assert_eq!(
            gate.accept(first.session_id, &first.resources),
            BackendSessionDecision::Initialize
        );

        app.switch_scene(1);
        let second = app
            .runtime
            .as_mut()
            .unwrap()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        assert_ne!(
            first.session_id.runtime_namespace(),
            second.session_id.runtime_namespace()
        );
        assert!(matches!(&second.resources, ResourceSync::Snapshot(_)));
        assert_eq!(
            gate.accept(second.session_id, &second.resources),
            BackendSessionDecision::Replace
        );
    }
}
