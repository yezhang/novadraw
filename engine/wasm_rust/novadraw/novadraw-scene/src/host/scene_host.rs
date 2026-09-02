//! Platform services consumed by the runtime boundary.

use std::cell::{Cell, RefCell};

use novadraw_geometry::Rectangle;
use novadraw_render::SurfaceInfo;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Crosshair,
    Text,
    Move,
    NotAllowed,
    EastWestResize,
    NorthSouthResize,
    NorthEastSouthWestResize,
    NorthWestSouthEastResize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImeState {
    pub enabled: bool,
    pub cursor_area: Option<Rectangle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityUpdate {
    pub revision: u64,
}

/// Narrow platform boundary. It schedules redraws and exposes platform state,
/// but never executes scene or renderer work.
pub trait PlatformHost {
    fn request_redraw(&self);
    fn surface_info(&self) -> SurfaceInfo;
    fn set_cursor(&self, cursor: CursorIcon);
    fn set_ime_state(&self, state: ImeState);
    fn update_accessibility(&self, update: AccessibilityUpdate);
}

/// Deterministic host for tests and replay without a native window.
pub struct HeadlessHost {
    surface: Cell<SurfaceInfo>,
    redraw_requests: Cell<u64>,
    redraw_pending: Cell<bool>,
    cursor: Cell<CursorIcon>,
    ime: Cell<ImeState>,
    accessibility_updates: RefCell<Vec<AccessibilityUpdate>>,
}

impl HeadlessHost {
    pub fn new(surface: SurfaceInfo) -> Self {
        Self {
            surface: Cell::new(surface),
            redraw_requests: Cell::new(0),
            redraw_pending: Cell::new(false),
            cursor: Cell::new(CursorIcon::Default),
            ime: Cell::new(ImeState::default()),
            accessibility_updates: RefCell::new(Vec::new()),
        }
    }

    pub fn set_surface_info(&self, surface: SurfaceInfo) {
        self.surface.set(surface);
    }

    pub fn redraw_request_count(&self) -> u64 {
        self.redraw_requests.get()
    }

    pub fn take_redraw_request(&self) -> bool {
        self.redraw_pending.replace(false)
    }

    pub fn cursor(&self) -> CursorIcon {
        self.cursor.get()
    }

    pub fn ime_state(&self) -> ImeState {
        self.ime.get()
    }

    pub fn accessibility_updates(&self) -> Vec<AccessibilityUpdate> {
        self.accessibility_updates.borrow().clone()
    }
}

impl PlatformHost for HeadlessHost {
    fn request_redraw(&self) {
        if !self.redraw_pending.replace(true) {
            self.redraw_requests
                .set(self.redraw_requests.get().wrapping_add(1));
        }
    }

    fn surface_info(&self) -> SurfaceInfo {
        self.surface.get()
    }

    fn set_cursor(&self, cursor: CursorIcon) {
        self.cursor.set(cursor);
    }

    fn set_ime_state(&self, state: ImeState) {
        self.ime.set(state);
    }

    fn update_accessibility(&self, update: AccessibilityUpdate) {
        self.accessibility_updates.borrow_mut().push(update);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RectangleFigure, Runtime};
    use novadraw_render::{BackendCapabilities, DamageMode};

    #[test]
    fn headless_host_records_platform_effects_deterministically() {
        let host = HeadlessHost::new(SurfaceInfo {
            logical_width: 800.0,
            logical_height: 600.0,
            pixel_width: 1600,
            pixel_height: 1200,
            scale_factor: 2.0,
        });

        host.request_redraw();
        host.request_redraw();
        host.set_cursor(CursorIcon::Crosshair);
        host.set_ime_state(ImeState {
            enabled: true,
            cursor_area: Some(Rectangle::new(10.0, 20.0, 2.0, 18.0)),
        });
        host.update_accessibility(AccessibilityUpdate { revision: 3 });

        assert_eq!(host.redraw_request_count(), 1);
        assert!(host.take_redraw_request());
        assert!(!host.take_redraw_request());
        host.request_redraw();
        assert_eq!(host.redraw_request_count(), 2);
        assert_eq!(host.cursor(), CursorIcon::Crosshair);
        assert!(host.ime_state().enabled);
        assert_eq!(
            host.accessibility_updates(),
            vec![AccessibilityUpdate { revision: 3 }]
        );
    }

    #[test]
    fn headless_host_drives_a_deterministic_runtime_frame() {
        let surface = SurfaceInfo {
            logical_width: 320.0,
            logical_height: 200.0,
            pixel_width: 640,
            pixel_height: 400,
            scale_factor: 2.0,
        };
        let host = HeadlessHost::new(surface);
        let mut runtime = Runtime::empty();
        runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 200.0)));

        host.request_redraw();
        assert!(host.take_redraw_request());
        let submission = runtime
            .prepare_submission(host.surface_info(), BackendCapabilities::FULL_FRAME_ONLY)
            .expect("headless frame");

        assert_eq!(submission.surface, surface);
        assert_eq!(submission.damage.mode(), DamageMode::Full);
        assert!(!submission.commands.is_empty());
    }
}
