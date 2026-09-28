#[cfg(feature = "native")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "native")]
use std::sync::{Arc, Mutex};

use novadraw::PlatformHost;
use novadraw::event::{AccessibilityUpdate, MonotonicTime, TooltipUpdate};
use novadraw::figure::CursorIcon;
use novadraw::host::ImeState;
use novadraw::render::SurfaceInfo;
#[cfg(feature = "native")]
use winit::dpi::{LogicalPosition, LogicalSize};
#[cfg(feature = "native")]
use winit::window::{CursorIcon as WinitCursorIcon, Window};

/// Native platform adapter shared by every winit application.
#[cfg(feature = "native")]
pub struct WinitPlatformHost {
    window: Arc<Window>,
    accessibility_revision: AtomicU64,
    accessibility_update: Mutex<Option<AccessibilityUpdate>>,
    wake_deadline: Mutex<Option<MonotonicTime>>,
    tooltip_update: Mutex<Option<TooltipUpdate>>,
}

#[cfg(feature = "native")]
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

#[cfg(feature = "native")]
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

/// Browser host adapter without a hard dependency on a specific DOM binding.
///
/// The embedding layer supplies callbacks backed by `requestAnimationFrame`,
/// CSS cursor updates, IME integration and its accessibility tree.
pub struct WebPlatformHost {
    surface: std::cell::Cell<SurfaceInfo>,
    request_animation_frame: Box<dyn Fn()>,
    set_cursor: Box<dyn Fn(CursorIcon)>,
    set_ime_state: Box<dyn Fn(ImeState)>,
    schedule_wake: Box<dyn Fn(Option<MonotonicTime>)>,
    update_tooltip: Box<dyn Fn(TooltipUpdate)>,
    update_accessibility: Box<dyn Fn(AccessibilityUpdate)>,
}

impl WebPlatformHost {
    pub fn new(
        surface: SurfaceInfo,
        request_animation_frame: impl Fn() + 'static,
        set_cursor: impl Fn(CursorIcon) + 'static,
        set_ime_state: impl Fn(ImeState) + 'static,
        schedule_wake: impl Fn(Option<MonotonicTime>) + 'static,
        update_tooltip: impl Fn(TooltipUpdate) + 'static,
        update_accessibility: impl Fn(AccessibilityUpdate) + 'static,
    ) -> Self {
        Self {
            surface: std::cell::Cell::new(surface),
            request_animation_frame: Box::new(request_animation_frame),
            set_cursor: Box::new(set_cursor),
            set_ime_state: Box::new(set_ime_state),
            schedule_wake: Box::new(schedule_wake),
            update_tooltip: Box::new(update_tooltip),
            update_accessibility: Box::new(update_accessibility),
        }
    }

    pub fn set_surface_info(&self, surface: SurfaceInfo) {
        self.surface.set(surface);
    }
}

impl PlatformHost for WebPlatformHost {
    fn request_redraw(&self) {
        (self.request_animation_frame)();
    }

    fn surface_info(&self) -> SurfaceInfo {
        self.surface.get()
    }

    fn set_cursor(&self, cursor: CursorIcon) {
        (self.set_cursor)(cursor);
    }

    fn set_ime_state(&self, state: ImeState) {
        (self.set_ime_state)(state);
    }

    fn schedule_wake(&self, deadline: Option<MonotonicTime>) {
        (self.schedule_wake)(deadline);
    }

    fn update_tooltip(&self, update: TooltipUpdate) {
        (self.update_tooltip)(update);
    }

    fn update_accessibility(&self, update: AccessibilityUpdate) {
        (self.update_accessibility)(update);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::Runtime;
    use novadraw::event::{AccessibilityDelta, AccessibilityNodeId};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn web_host_forwards_platform_effects_and_surface_changes() {
        let redraws = Rc::new(Cell::new(0_u64));
        let cursor = Rc::new(Cell::new(CursorIcon::Default));
        let ime = Rc::new(Cell::new(ImeState::default()));
        let wake = Rc::new(Cell::new(None));
        let tooltip_revision = Rc::new(Cell::new(0_u64));
        let accessibility_revision = Rc::new(Cell::new(0_u64));
        let host = WebPlatformHost::new(
            SurfaceInfo::default(),
            {
                let redraws = redraws.clone();
                move || redraws.set(redraws.get() + 1)
            },
            {
                let cursor = cursor.clone();
                move |value| cursor.set(value)
            },
            {
                let ime = ime.clone();
                move |value| ime.set(value)
            },
            {
                let wake = wake.clone();
                move |deadline| wake.set(deadline)
            },
            {
                let revision = tooltip_revision.clone();
                move |update| {
                    let value = match update {
                        TooltipUpdate::Show(snapshot) | TooltipUpdate::Replace(snapshot) => {
                            snapshot.revision
                        }
                        TooltipUpdate::Hide { revision } => revision,
                    };
                    revision.set(value);
                }
            },
            {
                let revision = accessibility_revision.clone();
                move |value| {
                    let value = match value {
                        AccessibilityUpdate::Snapshot(snapshot) => snapshot.revision,
                        AccessibilityUpdate::Delta(delta) => delta.revision,
                    };
                    revision.set(value);
                }
            },
        );
        let surface = SurfaceInfo {
            logical_width: 640.0,
            logical_height: 480.0,
            pixel_width: 1280,
            pixel_height: 960,
            scale_factor: 2.0,
        };

        host.set_surface_info(surface);
        host.request_redraw();
        host.set_cursor(CursorIcon::Pointer);
        host.set_ime_state(ImeState {
            enabled: true,
            cursor_area: None,
        });
        host.schedule_wake(Some(MonotonicTime::from_micros(7)));
        host.update_tooltip(TooltipUpdate::Hide { revision: 8 });
        host.update_accessibility(AccessibilityUpdate::Delta(AccessibilityDelta {
            base_revision: 8,
            revision: 9,
            stable_epoch: 1,
            upserts: Vec::new(),
            removed: Vec::new(),
            root_children: Vec::new(),
            focus: Some(AccessibilityNodeId::Root(
                Runtime::empty().tree().namespace(),
            )),
        }));

        assert_eq!(host.surface_info(), surface);
        assert_eq!(redraws.get(), 1);
        assert_eq!(cursor.get(), CursorIcon::Pointer);
        assert!(ime.get().enabled);
        assert_eq!(wake.get(), Some(MonotonicTime::from_micros(7)));
        assert_eq!(tooltip_revision.get(), 8);
        assert_eq!(accessibility_revision.get(), 9);
    }
}
