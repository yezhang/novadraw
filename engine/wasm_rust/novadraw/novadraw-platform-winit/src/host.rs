use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use novadraw::event::{AccessibilityUpdate, MonotonicTime, TooltipUpdate};
use novadraw::figure::CursorIcon;
use novadraw::host::ImeState;
use novadraw::host::PlatformHost;
use novadraw::render::SurfaceInfo;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{CursorIcon as WinitCursorIcon, Window};

/// Native platform adapter shared by every winit application.
pub struct WinitPlatformHost {
    window: Arc<Window>,
    accessibility_revision: AtomicU64,
    accessibility_update: Mutex<Option<AccessibilityUpdate>>,
    wake_deadline: Mutex<Option<MonotonicTime>>,
    tooltip_update: Mutex<Option<TooltipUpdate>>,
}

impl WinitPlatformHost {
    pub fn new(window: Arc<Window>) -> Self {
        Self {
            window,
            accessibility_revision: AtomicU64::new(0),
            accessibility_update: Mutex::new(None),
            wake_deadline: Mutex::new(None),
            tooltip_update: Mutex::new(None),
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_arc(&self) -> Arc<Window> {
        Arc::clone(&self.window)
    }

    pub fn accessibility_revision(&self) -> u64 {
        self.accessibility_revision.load(Ordering::Acquire)
    }

    pub fn accessibility_update(&self) -> Option<AccessibilityUpdate> {
        self.accessibility_update
            .lock()
            .expect("accessibility update lock poisoned")
            .clone()
    }

    pub fn wake_deadline(&self) -> Option<MonotonicTime> {
        *self
            .wake_deadline
            .lock()
            .expect("wake deadline lock poisoned")
    }

    pub fn tooltip_update(&self) -> Option<TooltipUpdate> {
        self.tooltip_update
            .lock()
            .expect("tooltip update lock poisoned")
            .clone()
    }
}

impl PlatformHost for WinitPlatformHost {
    fn request_redraw(&self) {
        // Winit coalesces duplicate redraw requests. Keeping a second pending
        // flag here can strand a request that the platform deferred at startup.
        self.window.request_redraw();
    }

    fn surface_info(&self) -> SurfaceInfo {
        let scale_factor = self.window.scale_factor();
        let size = self.window.inner_size();
        SurfaceInfo {
            logical_width: f64::from(size.width) / scale_factor,
            logical_height: f64::from(size.height) / scale_factor,
            pixel_width: size.width,
            pixel_height: size.height,
            scale_factor,
        }
    }

    fn set_cursor(&self, cursor: CursorIcon) {
        let cursor = match cursor {
            CursorIcon::Default => WinitCursorIcon::Default,
            CursorIcon::Pointer => WinitCursorIcon::Pointer,
            CursorIcon::Crosshair => WinitCursorIcon::Crosshair,
            CursorIcon::Text => WinitCursorIcon::Text,
            CursorIcon::Move => WinitCursorIcon::Move,
            CursorIcon::NotAllowed => WinitCursorIcon::NotAllowed,
            CursorIcon::EastWestResize => WinitCursorIcon::EwResize,
            CursorIcon::NorthSouthResize => WinitCursorIcon::NsResize,
            CursorIcon::NorthEastSouthWestResize => WinitCursorIcon::NeswResize,
            CursorIcon::NorthWestSouthEastResize => WinitCursorIcon::NwseResize,
        };
        self.window.set_cursor(cursor);
    }

    fn set_ime_state(&self, state: ImeState) {
        self.window.set_ime_allowed(state.enabled);
        if let Some(area) = state.cursor_area {
            self.window.set_ime_cursor_area(
                LogicalPosition::new(area.x, area.y),
                LogicalSize::new(area.width, area.height),
            );
        }
    }

    fn schedule_wake(&self, deadline: Option<MonotonicTime>) {
        *self
            .wake_deadline
            .lock()
            .expect("wake deadline lock poisoned") = deadline;
    }

    fn update_tooltip(&self, update: TooltipUpdate) {
        *self
            .tooltip_update
            .lock()
            .expect("tooltip update lock poisoned") = Some(update);
    }

    fn update_accessibility(&self, update: AccessibilityUpdate) {
        let revision = update.revision();
        self.accessibility_revision
            .store(revision, Ordering::Release);
        *self
            .accessibility_update
            .lock()
            .expect("accessibility update lock poisoned") = Some(update);
    }
}
