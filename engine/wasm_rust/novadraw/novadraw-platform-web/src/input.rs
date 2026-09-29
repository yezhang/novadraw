use novadraw::Point;
use novadraw::event::{
    FocusTraversalDirection, GesturePhase, GestureSessionId, Key, KeyModifiers, PointerId,
    ScrollDeltaKind, WheelEvent, ZoomEvent,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdaptedGesture {
    Scroll(WheelEvent),
    Zoom(ZoomEvent),
}

const WEB_WHEEL_LINE_HEIGHT: f64 = 16.0;
const WEB_WHEEL_ZOOM_SENSITIVITY: f64 = 0.002;
const MIN_WEB_WHEEL_ZOOM_FACTOR: f64 = 0.5;
const MAX_WEB_WHEEL_ZOOM_FACTOR: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebPointerInput {
    pub pointer_id: u64,
    pub client_x: f64,
    pub client_y: f64,
    pub canvas_left: f64,
    pub canvas_top: f64,
}

impl WebPointerInput {
    pub fn pointer_id(self) -> PointerId {
        PointerId::new(self.pointer_id)
    }

    /// DOM pointer coordinates are CSS pixels, which are the runtime logical unit.
    pub fn logical_position(self) -> Option<Point> {
        let x = self.client_x - self.canvas_left;
        let y = self.client_y - self.canvas_top;
        (x.is_finite() && y.is_finite()).then_some(Point::new(x, y))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WebWheelDeltaMode {
    Pixel,
    Line,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptedKeyInput {
    FocusTraversal(FocusTraversalDirection),
    Key {
        key: Key,
        pressed: bool,
        modifiers: KeyModifiers,
    },
    Ignored,
}

pub fn adapt_key_input(key: Key, pressed: bool, modifiers: KeyModifiers) -> AdaptedKeyInput {
    if key != Key::Tab {
        return AdaptedKeyInput::Key {
            key,
            pressed,
            modifiers,
        };
    }
    if !pressed {
        return AdaptedKeyInput::Ignored;
    }
    AdaptedKeyInput::FocusTraversal(if modifiers.shift {
        FocusTraversalDirection::Backward
    } else {
        FocusTraversalDirection::Forward
    })
}

#[derive(Debug, Default)]
pub struct WebInputAdapter;

impl WebInputAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn adapt_wheel_gesture(
        &self,
        pointer: WebPointerInput,
        delta_x: f64,
        delta_y: f64,
        delta_mode: WebWheelDeltaMode,
        viewport_width: f64,
        viewport_height: f64,
        modifiers: KeyModifiers,
    ) -> Option<AdaptedGesture> {
        if !modifiers.control {
            return self
                .adapt_wheel(
                    pointer,
                    delta_x,
                    delta_y,
                    delta_mode,
                    viewport_width,
                    viewport_height,
                    modifiers,
                )
                .map(AdaptedGesture::Scroll);
        }

        let point = pointer.logical_position()?;
        if !delta_y.is_finite() {
            return None;
        }
        let logical_delta_y = match delta_mode {
            WebWheelDeltaMode::Pixel => delta_y,
            WebWheelDeltaMode::Line => delta_y * WEB_WHEEL_LINE_HEIGHT,
            WebWheelDeltaMode::Page => delta_y * viewport_height.max(0.0),
        };
        if !logical_delta_y.is_finite() || logical_delta_y == 0.0 {
            return None;
        }
        let scale_factor = (-logical_delta_y * WEB_WHEEL_ZOOM_SENSITIVITY)
            .exp()
            .clamp(MIN_WEB_WHEEL_ZOOM_FACTOR, MAX_WEB_WHEEL_ZOOM_FACTOR);
        Some(AdaptedGesture::Zoom(ZoomEvent::new(
            point.x(),
            point.y(),
            scale_factor,
            GesturePhase::Impulse,
            modifiers,
            GestureSessionId::IMPULSE,
        )))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adapt_wheel(
        &self,
        pointer: WebPointerInput,
        delta_x: f64,
        delta_y: f64,
        delta_mode: WebWheelDeltaMode,
        viewport_width: f64,
        viewport_height: f64,
        modifiers: KeyModifiers,
    ) -> Option<WheelEvent> {
        let point = pointer.logical_position()?;
        if !delta_x.is_finite() || !delta_y.is_finite() {
            return None;
        }
        let (delta_x, delta_y, delta_kind) = match delta_mode {
            WebWheelDeltaMode::Pixel => (delta_x, delta_y, ScrollDeltaKind::LogicalPixels),
            WebWheelDeltaMode::Line => (delta_x, delta_y, ScrollDeltaKind::Lines),
            WebWheelDeltaMode::Page => (
                delta_x * viewport_width.max(0.0),
                delta_y * viewport_height.max(0.0),
                ScrollDeltaKind::LogicalPixels,
            ),
        };
        Some(WheelEvent::with_details(
            point.x(),
            point.y(),
            delta_x,
            delta_y,
            delta_kind,
            GesturePhase::Impulse,
            modifiers,
            GestureSessionId::IMPULSE,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_is_adapted_to_one_traversal_without_an_ordinary_key_release() {
        assert_eq!(
            adapt_key_input(Key::Tab, true, KeyModifiers::default()),
            AdaptedKeyInput::FocusTraversal(FocusTraversalDirection::Forward)
        );
        assert_eq!(
            adapt_key_input(
                Key::Tab,
                true,
                KeyModifiers {
                    shift: true,
                    ..KeyModifiers::default()
                },
            ),
            AdaptedKeyInput::FocusTraversal(FocusTraversalDirection::Backward)
        );
        assert_eq!(
            adapt_key_input(Key::Tab, false, KeyModifiers::default()),
            AdaptedKeyInput::Ignored
        );
    }
    #[test]
    fn web_pointer_coordinates_remain_in_css_logical_units() {
        let input = WebPointerInput {
            pointer_id: 17,
            client_x: 220.0,
            client_y: 140.0,
            canvas_left: 20.0,
            canvas_top: 40.0,
        };

        assert_eq!(input.pointer_id(), PointerId::new(17));
        assert_eq!(input.logical_position(), Some(Point::new(200.0, 100.0)));
    }

    #[test]
    fn web_wheel_delta_modes_map_to_engine_units() {
        let pointer = WebPointerInput {
            pointer_id: 1,
            client_x: 100.0,
            client_y: 80.0,
            canvas_left: 10.0,
            canvas_top: 20.0,
        };
        let adapter = WebInputAdapter;

        let pixel = adapter
            .adapt_wheel(
                pointer,
                5.0,
                -10.0,
                WebWheelDeltaMode::Pixel,
                800.0,
                600.0,
                KeyModifiers::default(),
            )
            .unwrap();
        assert_eq!(pixel.x, 90.0);
        assert_eq!(pixel.y, 60.0);
        assert_eq!(pixel.delta_kind, ScrollDeltaKind::LogicalPixels);
        assert_eq!(pixel.delta_y, -10.0);

        let page = adapter
            .adapt_wheel(
                pointer,
                0.0,
                1.0,
                WebWheelDeltaMode::Page,
                800.0,
                600.0,
                KeyModifiers::default(),
            )
            .unwrap();
        assert_eq!(page.delta_kind, ScrollDeltaKind::LogicalPixels);
        assert_eq!(page.delta_y, 600.0);
    }

    #[test]
    fn web_control_wheel_maps_to_anchored_zoom() {
        let pointer = WebPointerInput {
            pointer_id: 1,
            client_x: 220.0,
            client_y: 140.0,
            canvas_left: 20.0,
            canvas_top: 40.0,
        };
        let modifiers = KeyModifiers {
            control: true,
            ..KeyModifiers::default()
        };
        let gesture = WebInputAdapter
            .adapt_wheel_gesture(
                pointer,
                0.0,
                -100.0,
                WebWheelDeltaMode::Pixel,
                800.0,
                600.0,
                modifiers,
            )
            .unwrap();

        let AdaptedGesture::Zoom(zoom) = gesture else {
            panic!("control-wheel must produce a zoom gesture");
        };
        assert_eq!(zoom.entry_point(), Point::new(200.0, 100.0));
        assert_eq!(zoom.phase, GesturePhase::Impulse);
        assert_eq!(zoom.modifiers, modifiers);
        assert_eq!(
            zoom.scale_factor,
            (100.0_f64 * WEB_WHEEL_ZOOM_SENSITIVITY).exp()
        );
    }

    #[test]
    fn web_plain_wheel_remains_scroll() {
        let pointer = WebPointerInput {
            pointer_id: 1,
            client_x: 100.0,
            client_y: 80.0,
            canvas_left: 10.0,
            canvas_top: 20.0,
        };
        let gesture = WebInputAdapter
            .adapt_wheel_gesture(
                pointer,
                5.0,
                -10.0,
                WebWheelDeltaMode::Pixel,
                800.0,
                600.0,
                KeyModifiers::default(),
            )
            .unwrap();

        let AdaptedGesture::Scroll(wheel) = gesture else {
            panic!("plain wheel must remain a scroll gesture");
        };
        assert_eq!(wheel.entry_point(), Point::new(90.0, 60.0));
        assert_eq!(wheel.delta_x, 5.0);
        assert_eq!(wheel.delta_y, -10.0);
    }
}
