//! Native button sequences, expressed in logical pixels before Core dispatch.
use std::time::Duration;

use novadraw::{event::MouseButton, geometry::Point, runtime::Runtime};
use winit::event::{ElementState, MouseButton as WinitMouseButton};

const DEFAULT_DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_DOUBLE_CLICK_DISTANCE: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerInputError {
    InvalidDoubleClickLimits,
    InvalidPointerPosition,
    NonMonotonicTime,
}

impl std::fmt::Display for PointerInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDoubleClickLimits => f.write_str("invalid double-click limits"),
            Self::InvalidPointerPosition => f.write_str("pointer position must be finite"),
            Self::NonMonotonicTime => f.write_str("pointer time must be monotonic"),
        }
    }
}
impl std::error::Error for PointerInputError {}

#[derive(Clone, Copy)]
struct Click {
    button: MouseButton,
    position: Point,
    time: Duration,
}

/// Bridges winit press/release sequences to Core, including double-click notifications.
///
/// Coordinates and the configurable distance threshold use logical pixels. The caller supplies
/// monotonic elapsed time; this adapter does not sample or advance the Runtime animation clock.
/// Defaults are a 500 ms / 4 logical-pixel fallback, not an OS settings query.
pub struct WinitPointerAdapter {
    max_interval: Duration,
    max_distance: f64,
    pressed: Option<Click>,
    released: Option<Click>,
    last_time: Option<Duration>,
}

impl Default for WinitPointerAdapter {
    fn default() -> Self {
        Self {
            max_interval: DEFAULT_DOUBLE_CLICK_INTERVAL,
            max_distance: DEFAULT_DOUBLE_CLICK_DISTANCE,
            pressed: None,
            released: None,
            last_time: None,
        }
    }
}

impl WinitPointerAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_double_click_limits(
        max_interval: Duration,
        max_distance: f64,
    ) -> Result<Self, PointerInputError> {
        if max_interval.is_zero() || !max_distance.is_finite() || max_distance < 0.0 {
            return Err(PointerInputError::InvalidDoubleClickLimits);
        }
        Ok(Self {
            max_interval,
            max_distance,
            ..Self::default()
        })
    }

    /// Interrupts the sequence when focus, scene, or input ownership changes.
    pub fn reset(&mut self) {
        self.pressed = None;
        self.released = None;
        self.last_time = None;
    }

    fn nearby(&self, a: Point, b: Point) -> bool {
        (a.x() - b.x()).hypot(a.y() - b.y()) <= self.max_distance
    }

    /// Moving beyond click tolerance prevents a drag from becoming a double-click.
    pub fn pointer_moved(&mut self, position: Point) -> Result<(), PointerInputError> {
        Self::validate_position(position)?;
        if self
            .pressed
            .or(self.released)
            .is_some_and(|click| !self.nearby(click.position, position))
        {
            self.pressed = None;
            self.released = None;
        }
        Ok(())
    }

    fn validate_position(position: Point) -> Result<(), PointerInputError> {
        if !position.x().is_finite() || !position.y().is_finite() {
            return Err(PointerInputError::InvalidPointerPosition);
        }
        Ok(())
    }

    /// Dispatches Pressed, optionally DoubleClicked, then the later Released event.
    /// Each pair is consumed once: four rapid clicks produce two double-clicks.
    pub fn dispatch_button_input(
        &mut self,
        runtime: &mut Runtime,
        state: ElementState,
        button: WinitMouseButton,
        position: Point,
        time: Duration,
    ) -> Result<(), PointerInputError> {
        Self::validate_position(position)?;
        if self.last_time.is_some_and(|previous| time < previous) {
            return Err(PointerInputError::NonMonotonicTime);
        }
        self.last_time = Some(time);
        let button = super::adapt_mouse_button(button);
        match state {
            ElementState::Pressed => {
                let double = self.released.take().is_some_and(|previous| {
                    previous.button == button
                        && button != MouseButton::None
                        && time.saturating_sub(previous.time) <= self.max_interval
                        && self.nearby(previous.position, position)
                });
                self.pressed = (!double && button != MouseButton::None).then_some(Click {
                    button,
                    position,
                    time,
                });
                runtime
                    .input()
                    .dispatch_mouse_pressed(position.x(), position.y(), button);
                if double {
                    runtime.input().dispatch_mouse_double_clicked(
                        position.x(),
                        position.y(),
                        button,
                    );
                }
            }
            ElementState::Released => {
                self.released = self.pressed.take().filter(|click| {
                    click.button == button && self.nearby(click.position, position)
                });
                runtime
                    .input()
                    .dispatch_mouse_released(position.x(), position.y(), button);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        event::{EventContext, MouseEvent, MouseEventKind},
        figure::{
            Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureEventHandler,
            INPUT, InputCapability,
        },
        geometry::Rectangle,
    };
    use std::sync::{Arc, Mutex};

    struct Recorder(Arc<Mutex<Vec<MouseEventKind>>>);
    impl Figure for Recorder {
        fn name(&self) -> &'static str {
            "PointerRecorder"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(INPUT, InputCapability::of::<Self>())
        }
    }
    impl FigureEventHandler for Recorder {
        fn on_mouse_pressed(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
            self.0.lock().unwrap().push(event.kind);
            true
        }
        fn on_mouse_released(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
            self.0.lock().unwrap().push(event.kind);
            true
        }
        fn on_mouse_double_clicked(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
            self.0.lock().unwrap().push(event.kind);
            true
        }
    }
    fn fixture() -> (Runtime, Arc<Mutex<Vec<MouseEventKind>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::empty();
        runtime
            .set_contents(Box::new(Recorder(events.clone())))
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        (runtime, events)
    }
    fn click(
        adapter: &mut WinitPointerAdapter,
        runtime: &mut Runtime,
        button: WinitMouseButton,
        position: Point,
        time: u64,
    ) {
        adapter
            .dispatch_button_input(
                runtime,
                ElementState::Pressed,
                button,
                position,
                Duration::from_millis(time),
            )
            .unwrap();
        adapter
            .dispatch_button_input(
                runtime,
                ElementState::Released,
                button,
                position,
                Duration::from_millis(time + 10),
            )
            .unwrap();
    }
    fn doubles(events: &Arc<Mutex<Vec<MouseEventKind>>>) -> usize {
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|kind| **kind == MouseEventKind::DoubleClicked)
            .count()
    }

    #[test]
    fn native_button_pairs_dispatch_pressed_double_clicked_released_in_order() {
        let (mut runtime, events) = fixture();
        let mut adapter = WinitPointerAdapter::new();
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            Point::new(10.0, 10.0),
            0,
        );
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            Point::new(11.0, 10.0),
            100,
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                MouseEventKind::Pressed,
                MouseEventKind::Released,
                MouseEventKind::Pressed,
                MouseEventKind::DoubleClicked,
                MouseEventKind::Released,
            ]
        );
        assert_eq!(runtime.animation().timeline().sample_version().time, None);
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            Point::new(10.0, 10.0),
            200,
        );
        assert_eq!(doubles(&events), 1);
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            Point::new(10.0, 10.0),
            300,
        );
        assert_eq!(doubles(&events), 2);
    }

    #[test]
    fn distant_delayed_different_button_and_drag_sequences_do_not_double_click() {
        for case in 0..4 {
            let (mut runtime, events) = fixture();
            let mut adapter = WinitPointerAdapter::new();
            let point = Point::new(10.0, 10.0);
            adapter
                .dispatch_button_input(
                    &mut runtime,
                    ElementState::Pressed,
                    WinitMouseButton::Left,
                    point,
                    Duration::ZERO,
                )
                .unwrap();
            if case == 3 {
                adapter.pointer_moved(Point::new(30.0, 10.0)).unwrap();
                adapter.pointer_moved(point).unwrap();
            }
            adapter
                .dispatch_button_input(
                    &mut runtime,
                    ElementState::Released,
                    WinitMouseButton::Left,
                    point,
                    Duration::from_millis(10),
                )
                .unwrap();
            let next = if case == 0 {
                Point::new(15.0, 10.0)
            } else {
                point
            };
            let time = if case == 1 { 501 } else { 100 };
            let button = if case == 2 {
                WinitMouseButton::Right
            } else {
                WinitMouseButton::Left
            };
            click(&mut adapter, &mut runtime, button, next, time);
            assert_eq!(doubles(&events), 0, "case {case}");
        }
    }

    #[test]
    fn reset_and_rejected_input_preserve_sequence_boundaries() {
        let (mut runtime, events) = fixture();
        let mut adapter = WinitPointerAdapter::new();
        let point = Point::new(10.0, 10.0);
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            point,
            100,
        );
        let count = events.lock().unwrap().len();
        assert_eq!(
            adapter.dispatch_button_input(
                &mut runtime,
                ElementState::Pressed,
                WinitMouseButton::Left,
                point,
                Duration::ZERO
            ),
            Err(PointerInputError::NonMonotonicTime)
        );
        assert_eq!(
            adapter.dispatch_button_input(
                &mut runtime,
                ElementState::Pressed,
                WinitMouseButton::Left,
                Point::new(f64::NAN, 10.0),
                Duration::from_millis(200)
            ),
            Err(PointerInputError::InvalidPointerPosition)
        );
        assert_eq!(events.lock().unwrap().len(), count);
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            point,
            200,
        );
        assert_eq!(doubles(&events), 1);
        adapter.reset();
        click(&mut adapter, &mut runtime, WinitMouseButton::Left, point, 0);
        adapter.reset();
        click(
            &mut adapter,
            &mut runtime,
            WinitMouseButton::Left,
            point,
            100,
        );
        assert_eq!(doubles(&events), 1);
        assert!(WinitPointerAdapter::with_double_click_limits(Duration::ZERO, 4.0).is_err());
        assert!(
            WinitPointerAdapter::with_double_click_limits(
                Duration::from_millis(100),
                f64::INFINITY
            )
            .is_err()
        );
        events.lock().unwrap().clear();
        let mut custom =
            WinitPointerAdapter::with_double_click_limits(Duration::from_millis(150), 1.0).unwrap();
        click(&mut custom, &mut runtime, WinitMouseButton::Left, point, 0);
        click(
            &mut custom,
            &mut runtime,
            WinitMouseButton::Left,
            Point::new(11.0, 10.0),
            150,
        );
        assert_eq!(doubles(&events), 1);
    }
}
