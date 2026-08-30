use std::collections::HashMap;

use novadraw::{
    GesturePhase, GestureSessionId, KeyModifiers, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
use winit::event::{DeviceId, MouseScrollDelta, TouchPhase};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdaptedGesture {
    Scroll(WheelEvent),
    Zoom(ZoomEvent),
}

#[derive(Default)]
pub struct WinitGestureAdapter {
    next_session_id: u64,
    scroll_sessions: HashMap<DeviceId, GestureSessionId>,
    zoom_sessions: HashMap<DeviceId, GestureSessionId>,
}

impl WinitGestureAdapter {
    pub fn new() -> Self {
        Self {
            next_session_id: 1,
            ..Self::default()
        }
    }

    fn allocate_session(&mut self) -> GestureSessionId {
        let id = GestureSessionId::new(self.next_session_id);
        self.next_session_id = self.next_session_id.wrapping_add(1).max(1);
        id
    }

    fn session_for(
        &mut self,
        device_id: DeviceId,
        phase: TouchPhase,
        zoom: bool,
    ) -> (GestureSessionId, GesturePhase) {
        let mapped_phase = map_touch_phase(phase);
        match mapped_phase {
            GesturePhase::Begin => {
                let session = self.allocate_session();
                let sessions = if zoom {
                    &mut self.zoom_sessions
                } else {
                    &mut self.scroll_sessions
                };
                sessions.insert(device_id, session);
                (session, mapped_phase)
            }
            GesturePhase::Update => {
                let sessions = if zoom {
                    &mut self.zoom_sessions
                } else {
                    &mut self.scroll_sessions
                };
                (
                    sessions
                        .get(&device_id)
                        .copied()
                        .unwrap_or(GestureSessionId::IMPULSE),
                    if sessions.contains_key(&device_id) {
                        mapped_phase
                    } else {
                        GesturePhase::Impulse
                    },
                )
            }
            GesturePhase::End | GesturePhase::Cancel => {
                let sessions = if zoom {
                    &mut self.zoom_sessions
                } else {
                    &mut self.scroll_sessions
                };
                (
                    sessions
                        .remove(&device_id)
                        .unwrap_or(GestureSessionId::IMPULSE),
                    mapped_phase,
                )
            }
            GesturePhase::Impulse => (GestureSessionId::IMPULSE, mapped_phase),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adapt_mouse_wheel(
        &mut self,
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
        physical_x: f64,
        physical_y: f64,
        scale_factor: f64,
        modifiers: KeyModifiers,
    ) -> Option<AdaptedGesture> {
        let scale_factor = valid_scale_factor(scale_factor);
        let x = physical_x / scale_factor;
        let y = physical_y / scale_factor;
        let (delta_x, delta_y, delta_kind, phase, session_id) = match delta {
            MouseScrollDelta::LineDelta(delta_x, delta_y) => (
                f64::from(delta_x),
                f64::from(delta_y),
                ScrollDeltaKind::Lines,
                GesturePhase::Impulse,
                GestureSessionId::IMPULSE,
            ),
            MouseScrollDelta::PixelDelta(delta) => {
                let (session_id, phase) = self.session_for(device_id, phase, false);
                (
                    delta.x / scale_factor,
                    delta.y / scale_factor,
                    ScrollDeltaKind::LogicalPixels,
                    phase,
                    session_id,
                )
            }
        };
        if !x.is_finite() || !y.is_finite() || !delta_x.is_finite() || !delta_y.is_finite() {
            return None;
        }
        Some(AdaptedGesture::Scroll(WheelEvent::with_details(
            x, y, delta_x, delta_y, delta_kind, phase, modifiers, session_id,
        )))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adapt_pinch(
        &mut self,
        device_id: DeviceId,
        magnification_delta: f64,
        phase: TouchPhase,
        physical_x: f64,
        physical_y: f64,
        scale_factor: f64,
        modifiers: KeyModifiers,
    ) -> Option<AdaptedGesture> {
        let gesture_scale_factor = 1.0 + magnification_delta;
        if !gesture_scale_factor.is_finite() || gesture_scale_factor <= 0.0 {
            if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.zoom_sessions.remove(&device_id);
            }
            return None;
        }
        let scale_factor = valid_scale_factor(scale_factor);
        let (session_id, phase) = self.session_for(device_id, phase, true);
        Some(AdaptedGesture::Zoom(ZoomEvent::new(
            physical_x / scale_factor,
            physical_y / scale_factor,
            gesture_scale_factor,
            phase,
            modifiers,
            session_id,
        )))
    }

    pub fn cancel_all(&mut self) {
        self.scroll_sessions.clear();
        self.zoom_sessions.clear();
    }
}

fn map_touch_phase(phase: TouchPhase) -> GesturePhase {
    match phase {
        TouchPhase::Started => GesturePhase::Begin,
        TouchPhase::Moved => GesturePhase::Update,
        TouchPhase::Ended => GesturePhase::End,
        TouchPhase::Cancelled => GesturePhase::Cancel,
    }
}

fn valid_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;

    #[test]
    fn line_delta_is_an_impulse_and_pixel_delta_uses_logical_pixels() {
        let device_id = DeviceId::dummy();
        let mut adapter = WinitGestureAdapter::new();

        let line = adapter
            .adapt_mouse_wheel(
                device_id,
                MouseScrollDelta::LineDelta(1.0, -2.0),
                TouchPhase::Moved,
                200.0,
                100.0,
                2.0,
                KeyModifiers::default(),
            )
            .unwrap();
        let pixel = adapter
            .adapt_mouse_wheel(
                device_id,
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(20.0, -10.0)),
                TouchPhase::Started,
                200.0,
                100.0,
                2.0,
                KeyModifiers::default(),
            )
            .unwrap();

        assert!(matches!(
            line,
            AdaptedGesture::Scroll(WheelEvent {
                delta_kind: ScrollDeltaKind::Lines,
                phase: GesturePhase::Impulse,
                delta_x: 1.0,
                delta_y: -2.0,
                ..
            })
        ));
        assert!(matches!(
            pixel,
            AdaptedGesture::Scroll(WheelEvent {
                x: 100.0,
                y: 50.0,
                delta_kind: ScrollDeltaKind::LogicalPixels,
                phase: GesturePhase::Begin,
                delta_x: 10.0,
                delta_y: -5.0,
                ..
            })
        ));
    }

    #[test]
    fn pinch_updates_share_a_session_until_the_gesture_ends() {
        let device_id = DeviceId::dummy();
        let mut adapter = WinitGestureAdapter::new();
        let adapt = |adapter: &mut WinitGestureAdapter, phase| {
            let AdaptedGesture::Zoom(event) = adapter
                .adapt_pinch(
                    device_id,
                    0.1,
                    phase,
                    40.0,
                    20.0,
                    2.0,
                    KeyModifiers::default(),
                )
                .unwrap()
            else {
                panic!("expected zoom event");
            };
            event
        };

        let begin = adapt(&mut adapter, TouchPhase::Started);
        let update = adapt(&mut adapter, TouchPhase::Moved);
        let end = adapt(&mut adapter, TouchPhase::Ended);
        let orphan_update = adapt(&mut adapter, TouchPhase::Moved);

        assert_eq!(begin.phase, GesturePhase::Begin);
        assert_eq!(begin.session_id, update.session_id);
        assert_eq!(begin.session_id, end.session_id);
        assert_eq!(orphan_update.phase, GesturePhase::Impulse);
        assert_eq!(orphan_update.session_id, GestureSessionId::IMPULSE);
    }
}
