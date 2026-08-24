use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::scene_manager::{DPI_TEST_PROBE_BOUNDS, SceneType};
use crate::system::{RawPointerInput, WinitNovadrawSystem};
use novadraw::backend::vello::{VelloRenderer, WinitWindowProxy};
use novadraw::traits::WindowProxy;
use novadraw::{Key, KeyModifiers, NovadrawSystem, RenderBackend};
use tracing::info;
use winit::dpi::{self, PhysicalSize};
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta};
use winit::window::WindowAttributes;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

pub struct GraphicsApp {
    renderer: Option<VelloRenderer>,
    system: Option<WinitNovadrawSystem>,
    cached_window: Option<Arc<Window>>,
    cursor_position: Option<(f64, f64)>,
    last_press: Option<(Instant, novadraw::MouseButton, f64, f64)>,
    modifiers: KeyModifiers,
}

impl GraphicsApp {
    fn new() -> Self {
        GraphicsApp {
            renderer: None,
            system: None,
            cached_window: None,
            cursor_position: None,
            last_press: None,
            modifiers: KeyModifiers::default(),
        }
    }

    fn execute_update(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let Some(system) = &mut self.system else {
            return;
        };
        system.render(&mut *renderer);
    }

    fn request_update(&self) {
        if let Some(system) = &self.system {
            system.request_update();
        }
    }

    fn log_dpi_probe_cursor(&self, raw_x: f64, raw_y: f64, logical_x: f64, logical_y: f64) {
        let Some(window) = &self.cached_window else {
            return;
        };
        let Some(system) = &self.system else {
            return;
        };
        if !system.is_scene(SceneType::DpiTest) {
            return;
        }

        let scale_factor = window.scale_factor();
        let physical_size = window.inner_size();
        let logical_size = physical_size.to_logical::<f64>(scale_factor);
        let probe = DPI_TEST_PROBE_BOUNDS;
        let probe_physical = (
            probe.x * scale_factor,
            probe.y * scale_factor,
            (probe.x + probe.width) * scale_factor,
            (probe.y + probe.height) * scale_factor,
        );
        let inside_logical = logical_x >= probe.x
            && logical_x <= probe.x + probe.width
            && logical_y >= probe.y
            && logical_y <= probe.y + probe.height;
        let inside_physical = raw_x >= probe_physical.0
            && raw_x <= probe_physical.2
            && raw_y >= probe_physical.1
            && raw_y <= probe_physical.3;

        info!(
            "dpi_probe cursor raw=({:.1}, {:.1}) logical=({:.1}, {:.1}) scale_factor={:.2} window_logical=({:.1}, {:.1}) window_physical=({},{}) probe_logical=({:.1},{:.1})-({:.1},{:.1}) probe_physical=({:.1},{:.1})-({:.1},{:.1}) inside_logical={} inside_physical={}",
            raw_x,
            raw_y,
            logical_x,
            logical_y,
            scale_factor,
            logical_size.width,
            logical_size.height,
            physical_size.width,
            physical_size.height,
            probe.x,
            probe.y,
            probe.x + probe.width,
            probe.y + probe.height,
            probe_physical.0,
            probe_physical.1,
            probe_physical.2,
            probe_physical.3,
            inside_logical,
            inside_physical
        );
    }
}

impl ApplicationHandler<()> for GraphicsApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        let window = self.cached_window.take().unwrap_or_else(|| {
            Arc::new(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("Novadraw - 渲染验证 (按 0-9 切换场景, I 切换渲染模式)")
                            .with_inner_size(dpi::LogicalSize::new(800, 600))
                            .with_resizable(true),
                    )
                    .unwrap(),
            )
        });

        let logical_width = 800.0;
        let logical_height = 600.0;

        self.cached_window = Some(Arc::clone(&window));
        let window_proxy = Arc::new(WinitWindowProxy::new(window));

        let renderer = VelloRenderer::new(Arc::clone(&window_proxy), logical_width, logical_height);
        self.renderer = Some(renderer);

        if self.system.is_none() {
            self.system = Some(WinitNovadrawSystem::new(window_proxy));
        }

        self.request_update();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.execute_update();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    let scale_factor = renderer.window().scale_factor();
                    let PhysicalSize { width, height } = new_size;

                    renderer.resize(width, height, scale_factor);
                    self.request_update();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale_factor = self
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.window().scale_factor())
                    .unwrap_or(1.0);
                let raw = RawPointerInput::new(position.x, position.y, scale_factor);
                let logical_position = raw.logical_position();
                self.cursor_position = Some((position.x, position.y));

                self.log_dpi_probe_cursor(
                    position.x,
                    position.y,
                    logical_position.x,
                    logical_position.y,
                );
                if let Some(system) = &mut self.system {
                    let _ = system.dispatch_raw_mouse_moved(raw);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let (Some((physical_x, physical_y)), Some(system)) =
                    (self.cursor_position, &mut self.system)
                {
                    let scale_factor = self
                        .renderer
                        .as_ref()
                        .map(|renderer| renderer.window().scale_factor())
                        .unwrap_or(1.0);
                    let raw = RawPointerInput::new(physical_x, physical_y, scale_factor);
                    let button = match button {
                        WinitMouseButton::Left => novadraw::MouseButton::Left,
                        WinitMouseButton::Middle => novadraw::MouseButton::Middle,
                        WinitMouseButton::Right => novadraw::MouseButton::Right,
                        _ => novadraw::MouseButton::None,
                    };

                    match state {
                        ElementState::Pressed => {
                            let _ = system.dispatch_raw_mouse_pressed(raw, button);
                            let logical = raw.logical_position();
                            let now = Instant::now();
                            let double_clicked = self.last_press.is_some_and(
                                |(previous_time, previous_button, previous_x, previous_y)| {
                                    previous_button == button
                                        && now.duration_since(previous_time)
                                            <= DOUBLE_CLICK_INTERVAL
                                        && (logical.x - previous_x).abs() <= DOUBLE_CLICK_DISTANCE
                                        && (logical.y - previous_y).abs() <= DOUBLE_CLICK_DISTANCE
                                },
                            );
                            self.last_press = Some((now, button, logical.x, logical.y));
                            if double_clicked {
                                system.dispatch_raw_mouse_double_clicked(raw, button);
                                self.last_press = None;
                            }
                        }
                        ElementState::Released => {
                            let _ = system.dispatch_raw_mouse_released(raw, button);
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let (Some((physical_x, physical_y)), Some(system)) =
                    (self.cursor_position, &mut self.system)
                {
                    let scale_factor = self
                        .renderer
                        .as_ref()
                        .map(|renderer| renderer.window().scale_factor())
                        .unwrap_or(1.0);
                    let raw = RawPointerInput::new(physical_x, physical_y, scale_factor);
                    let (delta_x, delta_y) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x as f64, y as f64),
                        MouseScrollDelta::PixelDelta(position) => {
                            (position.x / scale_factor, position.y / scale_factor)
                        }
                    };
                    system.dispatch_raw_mouse_wheel(raw, delta_x, delta_y);
                }
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
                if let Some(system) = &mut self.system {
                    system.release_focus();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let engine_key = map_key(event.physical_key);
                if let (Some(system), Some(key)) = (&mut self.system, engine_key) {
                    match event.state {
                        ElementState::Pressed => system.dispatch_key_pressed(key, self.modifiers),
                        ElementState::Released => system.dispatch_key_released(key, self.modifiers),
                    }
                }

                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Digit0) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::BasicAnchors);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        event_loop.exit();
                    }
                    PhysicalKey::Code(KeyCode::Digit1) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::Nested);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit2) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::NestedWithRoot);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit3) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::ZOrder);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit4) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::Visibility);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit5) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::BoundsTranslate);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit6) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::ClipTest);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit7) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::EllipseTest);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit8) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::LineTest);
                        }
                    }
                    PhysicalKey::Code(KeyCode::Digit9) => {
                        if let Some(system) = &mut self.system {
                            system.switch_scene(SceneType::DpiTest);
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        if let Some(system) = &mut self.system {
                            system.translate_contents_if_scene(
                                SceneType::BoundsTranslate,
                                10.0,
                                10.0,
                            );
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => {
                        if let Some(system) = &mut self.system {
                            let scale_factor = self
                                .renderer
                                .as_ref()
                                .map(|renderer| renderer.window().scale_factor())
                                .unwrap_or(1.0);
                            let report = system.run_interaction_script(&[
                                crate::system::InteractionStep::Hover {
                                    input: RawPointerInput::new(
                                        150.0 * scale_factor,
                                        150.0 * scale_factor,
                                        scale_factor,
                                    ),
                                    duration_ms: 500,
                                },
                            ]);
                            println!("[Hover Test] traces={:?}", report.traces);
                            println!("[Hover Test] 完成");
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyM) => {
                        if let Some(system) = &mut self.system {
                            let scale_factor = self
                                .renderer
                                .as_ref()
                                .map(|renderer| renderer.window().scale_factor())
                                .unwrap_or(1.0);
                            let trace = system.dispatch_raw_mouse_moved(RawPointerInput::new(
                                150.0 * scale_factor,
                                150.0 * scale_factor,
                                scale_factor,
                            ));
                            println!("[Move Test] trace={trace:?}");
                            println!("[Move Test] 完成");
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        if let Some(system) = &mut self.system {
                            let scale_factor = self
                                .renderer
                                .as_ref()
                                .map(|renderer| renderer.window().scale_factor())
                                .unwrap_or(1.0);
                            let report = system.run_interaction_script(&[
                                crate::system::InteractionStep::Click {
                                    input: RawPointerInput::new(
                                        150.0 * scale_factor,
                                        150.0 * scale_factor,
                                        scale_factor,
                                    ),
                                    button: novadraw::MouseButton::Left,
                                },
                            ]);
                            println!("[Click Test] traces={:?}", report.traces);
                            println!("[Click Test] 完成");
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.take() {
            self.cached_window = Some(renderer.window().clone_window());
        }
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

pub fn start_app() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = GraphicsApp::new();

    println!("启动事件循环...");
    println!("  按 0-9 切换场景：");
    println!(
        "    0=基础定位点 1=嵌套父子 2=嵌套(含根) 3=Z-order 4=不可见 5=平移验证 6=裁剪测试 7=椭圆测试 8=直线测试 9=DPI 坐标验证"
    );
    println!("  按 T 键：在场景 5 中平移父节点");
    println!("  按 ESC 退出");
    event_loop.run_app(&mut app)?;

    Ok(())
}
