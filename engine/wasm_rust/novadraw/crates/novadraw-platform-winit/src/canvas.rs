//! Per-Canvas Novadraw hosts and arbitration of shared window services.
//! Input deliveries retain Canvas-local logical coordinates; do not apply DPI twice.
use crate::WinitPlatformHost;
use novadraw::{
    event::{AccessibilityUpdate, MonotonicTime, TooltipUpdate},
    figure::CursorIcon,
    geometry::Rectangle,
    host::{ImeState, PlatformHost},
    render::SurfaceInfo,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};
use winit_canvas::{
    geometry::{CanvasError, CanvasRect, WindowMetrics},
    host::{CanvasEvent, CanvasHost, CanvasId, Delivery, RedrawToken},
};

struct HostState {
    id: CanvasId,
    surface: Cell<SurfaceInfo>,
    rect: Cell<CanvasRect>,
    available: Cell<bool>,
    redraw: Cell<bool>,
    cursor: Cell<CursorIcon>,
    ime: Cell<ImeState>,
    wake: Cell<Option<MonotonicTime>>,
    tooltip: RefCell<Option<TooltipUpdate>>,
    accessibility: RefCell<Option<AccessibilityUpdate>>,
}

/// One Canvas's PlatformHost. Clone shares its state, never its Runtime.
/// Created by WinitCanvasCoordinator::bind. Retained handles become inactive
/// after removal or coordinator drop; they cannot affect a replacement Canvas.
#[derive(Clone)]
pub struct WinitCanvasHost {
    state: Rc<HostState>,
    request_window_redraw: Rc<dyn Fn()>,
}

impl WinitCanvasHost {
    pub fn canvas_id(&self) -> CanvasId {
        self.state.id.clone()
    }
    pub fn available(&self) -> bool {
        self.state.available.get()
    }
    /// Latest Canvas-local tooltip data. The application composes its overlay.
    pub fn tooltip_update(&self) -> Option<TooltipUpdate> {
        self.state.tooltip.borrow().clone()
    }
    /// Latest accessibility publication, retaining the Runtime's namespace.
    pub fn accessibility_update(&self) -> Option<AccessibilityUpdate> {
        self.state.accessibility.borrow().clone()
    }
    fn wake_window(&self) {
        if self.state.surface.get().pixel_width != 0 {
            (self.request_window_redraw)();
        }
    }
}

impl PlatformHost for WinitCanvasHost {
    fn request_redraw(&self) {
        if self.state.surface.get().pixel_width != 0 {
            self.state.redraw.set(true);
            self.wake_window();
        }
    }
    fn surface_info(&self) -> SurfaceInfo {
        self.state.surface.get()
    }
    fn set_cursor(&self, cursor: CursorIcon) {
        self.state.cursor.set(cursor);
        self.wake_window();
    }
    fn set_ime_state(&self, state: ImeState) {
        self.state.ime.set(state);
        self.wake_window();
    }
    fn schedule_wake(&self, deadline: Option<MonotonicTime>) {
        self.state.wake.set(deadline);
        self.wake_window();
    }
    fn update_tooltip(&self, update: TooltipUpdate) {
        if self.available() {
            *self.state.tooltip.borrow_mut() = Some(update);
            self.wake_window();
        }
    }
    fn update_accessibility(&self, update: AccessibilityUpdate) {
        if self.available() {
            *self.state.accessibility.borrow_mut() = Some(update);
            self.wake_window();
        }
    }
}

struct WindowServices {
    redraw: Rc<dyn Fn()>,
    cursor: Box<dyn Fn(CursorIcon)>,
    ime: Box<dyn Fn(ImeState)>,
}

/// One window's Canvas router and Novadraw host coordinator.
/// Owns no Runtime, renderer or GPU resources. Use on the winit event-loop thread.
pub struct WinitCanvasCoordinator {
    window_id: WindowId,
    canvases: CanvasHost,
    states: Vec<Rc<HostState>>,
    services: WindowServices,
    hovered: Option<CanvasId>,
    suspended: bool,
    applied_cursor: Option<CursorIcon>,
    applied_ime: Option<ImeState>,
}

impl WinitCanvasCoordinator {
    pub fn new(window: Arc<Window>) -> Result<Self, CanvasError> {
        let metrics = WindowMetrics::new(window.inner_size(), window.scale_factor())?;
        let window_id = window.id();
        let host = Rc::new(WinitPlatformHost::new(window));
        Ok(Self::with_services(
            window_id,
            metrics,
            WindowServices {
                redraw: {
                    let host = host.clone();
                    Rc::new(move || host.request_redraw())
                },
                cursor: {
                    let host = host.clone();
                    Box::new(move |cursor| host.set_cursor(cursor))
                },
                ime: Box::new(move |ime| host.set_ime_state(ime)),
            },
        ))
    }
    fn with_services(
        window_id: WindowId,
        metrics: WindowMetrics,
        services: WindowServices,
    ) -> Self {
        Self {
            window_id,
            canvases: CanvasHost::new(window_id, metrics),
            states: Vec::new(),
            services,
            hovered: None,
            suspended: false,
            applied_cursor: None,
            applied_ime: None,
        }
    }
    pub fn canvases(&self) -> &CanvasHost {
        &self.canvases
    }
    pub fn insert(&mut self, rect: CanvasRect) -> Result<WinitCanvasHost, CanvasError> {
        let id = self.canvases.insert(rect)?;
        let host = self.bind(&id)?;
        self.synchronize()?;
        Ok(host)
    }
    /// Bind a Canvas created through update_canvases. Rebinding shares the existing host.
    pub fn bind(&mut self, id: &CanvasId) -> Result<WinitCanvasHost, CanvasError> {
        let canvas = self.canvases.canvas(id)?;
        let state = if let Some(state) = self.states.iter().find(|state| &state.id == id) {
            state.clone()
        } else {
            let state = Rc::new(HostState {
                id: id.clone(),
                rect: Cell::new(canvas.rect()),
                surface: Cell::new(SurfaceInfo::default()),
                available: Cell::new(false),
                redraw: Cell::new(false),
                cursor: Cell::new(CursorIcon::Default),
                ime: Cell::new(ImeState::default()),
                wake: Cell::new(None),
                tooltip: RefCell::new(None),
                accessibility: RefCell::new(None),
            });
            self.states.push(state.clone());
            state
        };
        self.refresh_states()?;
        Ok(WinitCanvasHost {
            state,
            request_window_redraw: self.services.redraw.clone(),
        })
    }
    /// Mutate Canvas geometry, visibility, focus, metrics or membership.
    /// Return its deliveries; state reconciliation also runs if the operation fails.
    /// Do not call CanvasHost::suspend/resume here; use coordinator lifecycle methods.
    pub fn update_canvases(
        &mut self,
        operation: impl FnOnce(&mut CanvasHost) -> Result<Vec<Delivery>, CanvasError>,
    ) -> Result<Vec<Delivery>, CanvasError> {
        let result = operation(&mut self.canvases);
        if let Ok(deliveries) = &result {
            self.observe(deliveries);
        }
        self.synchronize()?;
        result
    }
    pub fn handle_window_event(
        &mut self,
        window: WindowId,
        event: &WindowEvent,
    ) -> Result<Vec<Delivery>, CanvasError> {
        self.update_canvases(|host| host.handle_window_event(window, event))
    }
    pub fn suspend(&mut self) -> Result<Vec<Delivery>, CanvasError> {
        self.suspended = true;
        let deliveries = self.canvases.suspend();
        self.observe(&deliveries);
        self.synchronize()?;
        Ok(deliveries)
    }
    pub fn resume(&mut self, metrics: WindowMetrics) -> Result<Vec<Delivery>, CanvasError> {
        let deliveries = self.canvases.resume(metrics)?;
        self.suspended = false;
        self.observe(&deliveries);
        self.synchronize()?;
        Ok(deliveries)
    }
    pub fn acknowledge_canvas(
        &mut self,
        id: &CanvasId,
        token: RedrawToken,
    ) -> Result<(), CanvasError> {
        self.canvases.acknowledge_canvas(id, token)
    }
    pub fn acknowledge_composite(&mut self, token: RedrawToken) -> Result<(), CanvasError> {
        self.canvases.acknowledge_composite(token)
    }
    fn observe(&mut self, deliveries: &[Delivery]) {
        for delivery in deliveries {
            match delivery.event {
                CanvasEvent::PointerEntered => self.hovered = Some(delivery.canvas.clone()),
                CanvasEvent::PointerLeft if self.hovered.as_ref() == Some(&delivery.canvas) => {
                    self.hovered = None
                }
                _ => {}
            }
        }
    }
    fn refresh_states(&mut self) -> Result<(), CanvasError> {
        let metrics = self.canvases.metrics();
        for state in &self.states {
            match self.canvases.canvas(&state.id) {
                Ok(canvas) => {
                    let rect = canvas.rect();
                    let active = !self.suspended
                        && canvas.visible()
                        && !rect.is_empty()
                        && metrics.pixels().width != 0
                        && metrics.pixels().height != 0;
                    state.rect.set(rect);
                    state.available.set(active && canvas.enabled());
                    let size = metrics.texture_size(rect)?;
                    state.surface.set(SurfaceInfo {
                        logical_width: if active { rect.width() } else { 0.0 },
                        logical_height: if active { rect.height() } else { 0.0 },
                        pixel_width: if active { size.width } else { 0 },
                        pixel_height: if active { size.height } else { 0 },
                        scale_factor: metrics.scale_factor(),
                    });
                    if !state.available.get() {
                        clear_effects(state);
                    }
                    if !active {
                        state.redraw.set(false);
                    }
                }
                Err(CanvasError::UnknownCanvas) => deactivate(state),
                Err(error) => return Err(error),
            }
        }
        self.states
            .retain(|state| self.canvases.canvas(&state.id).is_ok());
        Ok(())
    }
    /// Call after Runtime publishes platform effects, before reading redraw tokens.
    /// Void PlatformHost effects are staged so Canvas mutation errors remain Result-based.
    pub fn synchronize(&mut self) -> Result<(), CanvasError> {
        self.refresh_states()?;
        for state in &self.states {
            if state.redraw.get() && state.surface.get().pixel_width != 0 {
                self.canvases.request_redraw(&state.id)?;
                state.redraw.set(false);
            }
        }
        let pointer = self.canvases.captured().or_else(|| self.hovered.clone());
        let cursor = self
            .states
            .iter()
            .find(|state| state.available.get() && pointer.as_ref() == Some(&state.id))
            .map_or(CursorIcon::Default, |state| state.cursor.get());
        let focused = self.canvases.focused();
        let ime = self
            .states
            .iter()
            .find(|state| state.available.get() && focused.as_ref() == Some(&state.id))
            .map_or(ImeState::default(), |state| {
                let mut ime = state.ime.get();
                if let Some(area) = ime.cursor_area {
                    let origin = state.rect.get().origin();
                    ime.cursor_area = Some(Rectangle::new(
                        area.x + origin.x,
                        area.y + origin.y,
                        area.width,
                        area.height,
                    ));
                }
                ime
            });
        if self.applied_cursor != Some(cursor) {
            (self.services.cursor)(cursor);
            self.applied_cursor = Some(cursor);
        }
        if self.applied_ime != Some(ime) {
            (self.services.ime)(ime);
            self.applied_ime = Some(ime);
        }
        if self.canvases.needs_redraw() {
            (self.services.redraw)();
        }
        Ok(())
    }
    /// Earliest active Canvas wake. The application maps this to ControlFlow::WaitUntil.
    pub fn wake_deadline(&self) -> Option<MonotonicTime> {
        self.states
            .iter()
            .filter(|state| state.available.get())
            .filter_map(|state| state.wake.get())
            .min()
    }
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }
}

fn clear_effects(state: &HostState) {
    state.cursor.set(CursorIcon::Default);
    state.ime.set(ImeState::default());
    state.wake.set(None);
    state.tooltip.borrow_mut().take();
    state.accessibility.borrow_mut().take();
}
fn deactivate(state: &HostState) {
    state.redraw.set(false);
    state.available.set(false);
    state.surface.set(SurfaceInfo {
        scale_factor: state.surface.get().scale_factor,
        ..Default::default()
    });
    clear_effects(state);
}
impl Drop for WinitCanvasCoordinator {
    fn drop(&mut self) {
        for state in &self.states {
            deactivate(state);
        }
        (self.services.cursor)(CursorIcon::Default);
        (self.services.ime)(ImeState::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{DeviceId, ElementState, MouseButton},
    };

    struct Effects {
        redraws: Cell<u32>,
        cursor: Cell<CursorIcon>,
        ime: Cell<ImeState>,
    }
    fn fixture() -> (
        WinitCanvasCoordinator,
        Rc<Effects>,
        WinitCanvasHost,
        WinitCanvasHost,
    ) {
        let effects = Rc::new(Effects {
            redraws: Cell::new(0),
            cursor: Cell::new(CursorIcon::Default),
            ime: Cell::new(ImeState::default()),
        });
        let mut coordinator = WinitCanvasCoordinator::with_services(
            WindowId::dummy(),
            WindowMetrics::new(PhysicalSize::new(1000, 600), 2.0).unwrap(),
            WindowServices {
                redraw: {
                    let effects = effects.clone();
                    Rc::new(move || effects.redraws.set(effects.redraws.get() + 1))
                },
                cursor: {
                    let effects = effects.clone();
                    Box::new(move |cursor| effects.cursor.set(cursor))
                },
                ime: {
                    let effects = effects.clone();
                    Box::new(move |ime| effects.ime.set(ime))
                },
            },
        );
        let left = coordinator
            .insert(CanvasRect::new(10.0, 20.0, 180.25, 200.0).unwrap())
            .unwrap();
        let right = coordinator
            .insert(CanvasRect::new(230.0, 20.0, 180.0, 200.0).unwrap())
            .unwrap();
        (coordinator, effects, left, right)
    }
    fn focus(coordinator: &mut WinitCanvasCoordinator, host: &WinitCanvasHost) {
        coordinator
            .update_canvases(|canvases| canvases.set_focus(Some(&host.canvas_id())))
            .unwrap();
    }
    #[test]
    fn per_canvas_surface_and_redraw_follow_identity_and_dpi() {
        let (mut coordinator, _, left, right) = fixture();
        assert_eq!(left.surface_info().pixel_width, 361);
        assert_eq!(left.surface_info().logical_width, 180.25);
        assert_eq!(right.surface_info().pixel_width, 360);
        for host in [&left, &right] {
            let id = host.canvas_id();
            let token = coordinator
                .canvases()
                .canvas(&id)
                .unwrap()
                .redraw_token()
                .unwrap();
            coordinator.acknowledge_canvas(&id, token).unwrap();
        }
        left.request_redraw();
        coordinator.synchronize().unwrap();
        assert!(
            coordinator
                .canvases()
                .canvas(&left.canvas_id())
                .unwrap()
                .redraw_token()
                .is_some()
        );
        assert!(
            coordinator
                .canvases()
                .canvas(&right.canvas_id())
                .unwrap()
                .redraw_token()
                .is_none()
        );
        coordinator
            .update_canvases(|host| {
                host.set_metrics(WindowMetrics::new(PhysicalSize::new(1500, 900), 3.0)?)
            })
            .unwrap();
        assert_eq!(left.surface_info().pixel_width, 541);
        assert_eq!(left.surface_info().scale_factor, 3.0);
    }
    #[test]
    fn ime_is_focus_scoped_and_reprojects_when_canvas_moves() {
        let (mut coordinator, effects, left, right) = fixture();
        let ime = ImeState {
            enabled: true,
            cursor_area: Some(Rectangle::new(3.0, 4.0, 2.0, 18.0)),
        };
        left.set_ime_state(ime);
        right.set_ime_state(ImeState {
            enabled: false,
            cursor_area: None,
        });
        focus(&mut coordinator, &left);
        assert_eq!(
            effects.ime.get().cursor_area,
            Some(Rectangle::new(13.0, 24.0, 2.0, 18.0))
        );
        coordinator
            .update_canvases(|host| {
                host.set_rect(
                    &left.canvas_id(),
                    CanvasRect::new(40.0, 50.0, 180.25, 200.0)?,
                )
            })
            .unwrap();
        assert_eq!(effects.ime.get().cursor_area.unwrap().x, 43.0);
        focus(&mut coordinator, &right);
        assert!(!effects.ime.get().enabled);
        coordinator
            .handle_window_event(WindowId::dummy(), &WindowEvent::Focused(false))
            .unwrap();
        assert_eq!(effects.ime.get(), ImeState::default());
    }
    #[test]
    fn pointer_capture_arbitrates_cursor_and_preserves_local_coordinates() {
        let (mut coordinator, effects, left, right) = fixture();
        left.set_cursor(CursorIcon::Move);
        right.set_cursor(CursorIcon::Text);
        let motion = |x| WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(x, 100.0),
        };
        let events = coordinator
            .handle_window_event(WindowId::dummy(), &motion(80.0))
            .unwrap();
        assert!(
            matches!(events.last().unwrap().event, CanvasEvent::PointerMoved { position } if position.x == 30.0 && position.y == 30.0)
        );
        assert_eq!(effects.cursor.get(), CursorIcon::Move);
        coordinator
            .handle_window_event(
                WindowId::dummy(),
                &WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                },
            )
            .unwrap();
        coordinator
            .handle_window_event(WindowId::dummy(), &motion(500.0))
            .unwrap();
        assert_eq!(effects.cursor.get(), CursorIcon::Move);
        coordinator
            .handle_window_event(
                WindowId::dummy(),
                &WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    button: MouseButton::Left,
                    state: ElementState::Released,
                },
            )
            .unwrap();
        assert_eq!(effects.cursor.get(), CursorIcon::Text);
    }
    #[test]
    fn removal_suspension_and_drop_invalidate_retained_hosts_and_wakes() {
        let (mut coordinator, effects, left, right) = fixture();
        left.schedule_wake(Some(MonotonicTime::from_micros(20)));
        right.schedule_wake(Some(MonotonicTime::from_micros(10)));
        assert_eq!(
            coordinator.wake_deadline(),
            Some(MonotonicTime::from_micros(10))
        );
        coordinator
            .update_canvases(|host| host.remove(&right.canvas_id()))
            .unwrap();
        assert!(!right.available());
        assert_eq!(right.surface_info().pixel_width, 0);
        assert_eq!(
            coordinator.wake_deadline(),
            Some(MonotonicTime::from_micros(20))
        );
        let requests = effects.redraws.get();
        right.request_redraw();
        assert_eq!(effects.redraws.get(), requests);
        coordinator.suspend().unwrap();
        assert!(!left.available());
        assert_eq!(coordinator.wake_deadline(), None);
        coordinator
            .resume(WindowMetrics::new(PhysicalSize::new(1000, 600), 2.0).unwrap())
            .unwrap();
        assert!(left.available());
        drop(coordinator);
        assert!(!left.available());
        let requests = effects.redraws.get();
        left.request_redraw();
        assert_eq!(effects.redraws.get(), requests);
    }
    #[test]
    fn tooltip_and_accessibility_keep_per_runtime_namespaces() {
        use novadraw::event::{AccessibilityNodeId, AccessibilitySnapshot};
        let (mut coordinator, _, left, right) = fixture();
        let runtime = novadraw::runtime::Runtime::empty();
        left.update_tooltip(TooltipUpdate::Hide { revision: 4 });
        left.update_accessibility(AccessibilityUpdate::Snapshot(Arc::new(
            AccessibilitySnapshot {
                revision: 8,
                stable_epoch: 1,
                root: AccessibilityNodeId::Root(runtime.tree().namespace()),
                nodes: Vec::new(),
                focus: None,
            },
        )));
        assert!(right.tooltip_update().is_none());
        assert!(right.accessibility_update().is_none());
        assert!(left.accessibility_update().is_some());
        coordinator
            .update_canvases(|host| host.set_available(&left.canvas_id(), false, false))
            .unwrap();
        assert!(left.tooltip_update().is_none());
        assert!(left.accessibility_update().is_none());
    }
    #[test]
    fn disabled_visible_canvas_still_accepts_render_requests() {
        let (mut coordinator, _, left, _) = fixture();
        coordinator
            .update_canvases(|host| host.set_available(&left.canvas_id(), true, false))
            .unwrap();
        assert!(!left.available());
        assert!(left.surface_info().pixel_width > 0);
        let token = coordinator
            .canvases()
            .canvas(&left.canvas_id())
            .unwrap()
            .redraw_token()
            .unwrap();
        coordinator
            .acknowledge_canvas(&left.canvas_id(), token)
            .unwrap();
        left.request_redraw();
        coordinator.synchronize().unwrap();
        assert!(
            coordinator
                .canvases()
                .canvas(&left.canvas_id())
                .unwrap()
                .redraw_token()
                .is_some()
        );
    }
}
