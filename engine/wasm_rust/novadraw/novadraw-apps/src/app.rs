//! 应用框架
//!
//! 提供通用的演示应用构建和运行功能。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::input::{AdaptedGesture, WinitGestureAdapter};
pub use novadraw::{
    BlockId, FigureEvent, FigureGraph, Key, KeyModifiers, MouseButton, NotificationEffect,
    RenderBackend, RenderOutcome, Runtime, SurfaceInfo, UpdateEvent, UpdateListener, WindowProxy,
};
pub use novadraw_render::backend::vello::{VelloRenderer, WinitWindowProxy};
pub use winit::dpi::{LogicalSize, PhysicalSize};
pub use winit::event::WindowEvent;
pub use winit::event_loop::{ActiveEventLoop, EventLoop};
pub use winit::keyboard::{KeyCode, PhysicalKey};
pub use winit::window::WindowAttributes;
pub use winit::{application::ApplicationHandler, window::WindowId};

use tracing::{error, info, warn};

const SCREENSHOT_RENDER_RETRY_LIMIT: usize = 8;
const SCREENSHOT_RENDER_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(16);

/// 演示应用
///
/// 提供通用的演示应用框架，自动处理：
/// - 窗口创建
/// - 渲染器初始化
/// - 场景切换
/// - 事件处理
///
// 场景创建函数类型
type SceneCreator = Box<dyn FnMut() -> FigureGraph>;

struct DemoUpdateListener;

impl UpdateListener for DemoUpdateListener {
    fn on_update_event(&self, event: UpdateEvent) {
        tracing::debug!("[DemoApp] update event: {:?}", event);
    }

    fn on_figure_event(&self, event: FigureEvent) {
        tracing::debug!("[DemoApp] figure event: {:?}", event);
    }

    fn on_notify(&self, block_id: BlockId) {
        tracing::debug!("[DemoApp] notify: {:?}", block_id);
    }
}

pub struct DemoApp {
    #[allow(clippy::type_complexity)]
    scenes: Vec<(&'static str, SceneCreator)>,
    current_scene_idx: usize,
    runtime: Option<Runtime>,
    renderer: Option<VelloRenderer>,
    window: Option<Arc<winit::window::Window>>,
    title: String,
    app_name: String,
    width: f64,
    height: f64,
    use_update_manager: bool,
    screenshot_mode: Option<usize>,
    cursor_position: Option<(f64, f64)>,
    modifiers: KeyModifiers,
    gesture_adapter: WinitGestureAdapter,
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
        let default_scene = if screenshot_mode.is_some() { 0 } else { 4 };
        Self {
            scenes,
            current_scene_idx: default_scene,
            runtime: None,
            renderer: None,
            window: None,
            title: title.to_string(),
            app_name: app_name.to_string(),
            width,
            height,
            use_update_manager: true,
            screenshot_mode,
            cursor_position: None,
            modifiers: KeyModifiers::default(),
            gesture_adapter: WinitGestureAdapter::new(),
        }
    }

    /// 切换到指定场景
    pub fn switch_scene(&mut self, idx: usize) {
        if idx < self.scenes.len() {
            self.current_scene_idx = idx;
            let creator = &mut self.scenes[idx].1;
            let mut runtime = Runtime::new(creator());
            runtime.add_update_listener(Box::new(DemoUpdateListener));
            self.runtime = Some(runtime);
            eprintln!("切换到场景: {}", self.scenes[idx].0);

            // 更新窗口标题显示当前场景（只显示场景名称）
            let scene_name = self.scenes[idx].0;
            let new_title = format!("{} - {}", self.title, scene_name);
            eprintln!("设置窗口标题: {}", new_title);
            if let Some(window) = &self.window {
                window.set_title(&new_title);
                window.request_redraw(); // 忽略错误
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
        let Some(renderer) = &mut self.renderer else {
            return RenderOutcome::Skipped;
        };
        let Some(runtime) = &mut self.runtime else {
            return RenderOutcome::Skipped;
        };

        let canvas = if self.use_update_manager {
            let Some(canvas) = runtime.prepare_frame() else {
                return RenderOutcome::Skipped;
            };
            canvas
        } else {
            runtime.record_full_frame()
        };
        let scale_factor = renderer.window().scale_factor();
        let pixel_width = renderer.window().width();
        let pixel_height = renderer.window().height();
        let surface = SurfaceInfo {
            logical_width: f64::from(pixel_width) / scale_factor,
            logical_height: f64::from(pixel_height) / scale_factor,
            pixel_width,
            pixel_height,
            scale_factor,
        };
        let outcome = renderer.render(&canvas.to_submission_for_surface(surface));
        if outcome == RenderOutcome::Retry {
            renderer.window().request_redraw();
        }
        outcome
    }

    fn dispatch_input(&mut self, action: impl FnOnce(&mut Runtime)) {
        let Some(runtime) = &mut self.runtime else {
            return;
        };
        action(runtime);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// 截图并保存到文件
    ///
    /// 使用 VelloRenderer 直接捕获渲染结果
    /// 截图保存到 `{app目录}/screenshot/{app_name}_{scene}_{timestamp}.png`
    pub fn screenshot(&self, scene_name: &str) -> std::io::Result<std::path::PathBuf> {
        // 获取应用根目录（通过 CARGO_MANIFEST_DIR 环境变量）
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|_| std::env::current_dir())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));

        // 创建 screenshot 目录
        let screenshot_dir = manifest_dir.join("screenshot");
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
    fn handle_screenshot_mode(&mut self, mode: usize) {
        // mode: usize::MAX = 截图所有场景, 其他值 = 截图指定场景
        let scenes_to_capture: Vec<usize> = if mode == usize::MAX {
            // 截图所有场景
            (0..self.scenes.len()).collect()
        } else {
            // 截图指定场景
            vec![mode]
        };

        info!(
            "截图模式: {:?}, 场景数量: {}",
            scenes_to_capture,
            self.scenes.len()
        );

        let mut failed = false;
        for idx in scenes_to_capture {
            if idx >= self.scenes.len() {
                warn!("场景索引 {} 超出范围", idx);
                continue;
            }

            info!("截图场景 {}...", idx);

            // 切换到目标场景
            self.switch_scene(idx);

            let presented = (0..SCREENSHOT_RENDER_RETRY_LIMIT).any(|_| match self.render() {
                RenderOutcome::Presented => true,
                RenderOutcome::Retry => {
                    std::thread::sleep(SCREENSHOT_RENDER_RETRY_DELAY);
                    false
                }
                RenderOutcome::Skipped => false,
            });
            if !presented {
                error!("场景 {} 未产生可截图帧", self.scenes[idx].0);
                failed = true;
                continue;
            }

            // 截图
            let scene_name = self.scenes[idx].0;
            match self.screenshot(scene_name) {
                Ok(path) => {
                    info!("截图成功: {}", path.display());
                    // 分析截图
                    self.analyze_screenshot(&path, scene_name);
                }
                Err(e) => {
                    error!("截图失败: {}", e);
                    failed = true;
                }
            }
        }

        info!("截图完成，应用退出");
        // 截图完成后退出应用
        std::process::exit(i32::from(failed));
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

        let window: Arc<winit::window::Window> = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title(&self.title)
                        .with_inner_size(LogicalSize::new(self.width, self.height))
                        .with_resizable(true),
                )
                .unwrap(),
        );

        self.window = Some(window.clone());

        let window_proxy = Arc::new(WinitWindowProxy::new(window));
        let renderer = VelloRenderer::new(window_proxy, self.width, self.height);
        self.renderer = Some(renderer);

        // 创建初始场景（通过 switch_scene 以更新窗口标题）
        if !self.scenes.is_empty() {
            let idx = self.current_scene_idx.min(self.scenes.len() - 1);
            self.switch_scene(idx);
        }

        // 截图模式处理
        if let Some(screenshot_mode) = self.screenshot_mode {
            self.handle_screenshot_mode(screenshot_mode);
        }

        info!("应用启动: {}", self.title);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let _ = self.render();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    let scale_factor = renderer.window().scale_factor();
                    let PhysicalSize { width, height } = new_size;
                    renderer.resize(width, height, scale_factor);
                    renderer.window().request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some((position.x, position.y));
                let scale_factor = self
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.window().scale_factor())
                    .unwrap_or(1.0);
                self.dispatch_input(|runtime| {
                    runtime
                        .dispatch_mouse_moved(position.x / scale_factor, position.y / scale_factor);
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some((x, y)) = self.cursor_position else {
                    return;
                };
                let scale_factor = self
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.window().scale_factor())
                    .unwrap_or(1.0);
                let button = map_mouse_button(button);
                self.dispatch_input(|runtime| match state {
                    winit::event::ElementState::Pressed => {
                        runtime.dispatch_mouse_pressed(x / scale_factor, y / scale_factor, button)
                    }
                    winit::event::ElementState::Released => {
                        runtime.dispatch_mouse_released(x / scale_factor, y / scale_factor, button)
                    }
                });
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                let scale_factor = self
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.window().scale_factor())
                    .unwrap_or(1.0);
                let (physical_x, physical_y) = self.cursor_position.unwrap_or_else(|| {
                    let size = self
                        .window
                        .as_ref()
                        .map(|window| window.inner_size())
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
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.window().scale_factor())
                    .unwrap_or(1.0);
                let (physical_x, physical_y) = self.cursor_position.unwrap_or_else(|| {
                    let size = self
                        .window
                        .as_ref()
                        .map(|window| window.inner_size())
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
                let state = modifiers.state();
                self.modifiers = KeyModifiers {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
            }
            WindowEvent::Focused(false) => {
                self.gesture_adapter.cancel_all();
                self.dispatch_input(|runtime| {
                    runtime.cancel_gestures();
                    runtime.release_focus();
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == winit::event::ElementState::Pressed;
                if !pressed {
                    if let Some(key) = map_key(event.physical_key) {
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
                        self.window.as_ref().unwrap().request_redraw();
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
                            self.window.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight)
                    | PhysicalKey::Code(KeyCode::PageDown) => {
                        // 切换到下一个场景（循环）
                        let count = self.scenes.len();
                        if count > 0 {
                            let new_idx = (self.current_scene_idx + 1) % count;
                            self.switch_scene(new_idx);
                            self.window.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::Home) => {
                        // 切换到第一个场景
                        if !self.scenes.is_empty() {
                            self.switch_scene(0);
                            self.window.as_ref().unwrap().request_redraw();
                        }
                    }
                    PhysicalKey::Code(KeyCode::End) => {
                        // 切换到最后一个场景
                        let count = self.scenes.len();
                        if count > 0 {
                            self.switch_scene(count - 1);
                            self.window.as_ref().unwrap().request_redraw();
                        }
                    }
                    // 数字键 0-9 切换场景
                    _ => {
                        if let Some(digit) = get_digit_index(&event.physical_key) {
                            self.switch_scene(digit);
                            self.window.as_ref().unwrap().request_redraw();
                        } else if let Some(key) = map_key(event.physical_key) {
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = &self.renderer {
            renderer.window().request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.take() {
            self.window = Some(renderer.window().clone_window());
        }
    }
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

fn map_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back
        | winit::event::MouseButton::Forward
        | winit::event::MouseButton::Other(_) => MouseButton::None,
    }
}

fn map_key(key: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    Some(match code {
        KeyCode::Enter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::KeyA => Key::Character('a'),
        KeyCode::KeyB => Key::Character('b'),
        KeyCode::KeyC => Key::Character('c'),
        KeyCode::KeyH => Key::Character('h'),
        KeyCode::KeyM => Key::Character('m'),
        KeyCode::KeyT => Key::Character('t'),
        _ => return None,
    })
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
        creator: impl FnMut() -> FigureGraph + 'static,
    ) -> Self {
        self.scenes.push((name, Box::new(creator)));
        self
    }

    /// 批量添加场景（已装箱）
    #[allow(clippy::type_complexity)]
    pub fn with_scenes_boxed(
        mut self,
        scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureGraph>)>,
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
/// use novadraw_apps::run_demo_app;
///
/// fn create_rect_scene() -> novadraw::FigureGraph {
///     let mut scene = novadraw::FigureGraph::new();
///     let rect = novadraw::RectangleFigure::new(100.0, 100.0, 200.0, 150.0);
///     scene.set_contents(Box::new(rect));
///     scene
/// }
///
/// fn main() {
///     run_demo_app("My App", "rect-demo", vec![
///         ("Rectangle", Box::new(|| create_rect_scene())),
///     ]);
/// }
/// ```
#[allow(clippy::type_complexity)]
pub fn run_demo_app(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureGraph>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    run_demo_app_with_options(title, app_name, scenes, false, None)
}

#[allow(clippy::type_complexity)]
pub fn run_demo_app_with_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureGraph>)>,
    screenshot_all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_demo_app_with_options(title, app_name, scenes, screenshot_all, None)
}

#[allow(clippy::type_complexity)]
pub fn run_demo_app_with_scene_screenshot(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureGraph>)>,
    scene_index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    run_demo_app_with_options(title, app_name, scenes, false, Some(scene_index))
}

#[allow(clippy::type_complexity)]
fn run_demo_app_with_options(
    title: &str,
    app_name: &str,
    scenes: Vec<(&'static str, Box<dyn FnMut() -> FigureGraph>)>,
    screenshot_all: bool,
    screenshot_scene: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = AppBuilder::new(title)
        .with_size(800.0, 600.0)
        .with_app_name(app_name)
        .with_scenes_boxed(scenes);

    if screenshot_all {
        builder = builder.with_screenshot(true);
    } else if let Some(idx) = screenshot_scene {
        builder = builder.with_screenshot_scene(idx);
    }

    builder.run()
}
