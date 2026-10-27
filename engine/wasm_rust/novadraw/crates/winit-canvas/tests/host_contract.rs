use winit::{
    dpi::{LogicalPosition, PhysicalPosition, PhysicalSize},
    event::{DeviceId, ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    window::WindowId,
};
use winit_canvas::{
    geometry::{CanvasError, CanvasRect, WindowMetrics},
    host::{CanvasEvent, CanvasHost, CanvasId, Delivery},
};

fn fixture() -> (CanvasHost, CanvasId, CanvasId) {
    let mut host = CanvasHost::new(
        WindowId::dummy(),
        WindowMetrics::new(PhysicalSize::new(1000, 600), 2.0).unwrap(),
    );
    let left = host
        .insert(CanvasRect::new(10.0, 20.0, 180.0, 200.0).unwrap())
        .unwrap();
    let right = host
        .insert(CanvasRect::new(230.0, 20.0, 180.0, 200.0).unwrap())
        .unwrap();
    (host, left, right)
}
fn dispatch(host: &mut CanvasHost, event: WindowEvent) -> Vec<Delivery> {
    host.handle_window_event(WindowId::dummy(), &event).unwrap()
}
fn motion(host: &mut CanvasHost, x: f64, y: f64) -> Vec<Delivery> {
    dispatch(
        host,
        WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(x, y),
        },
    )
}
fn button(host: &mut CanvasHost, button: MouseButton, state: ElementState) -> Vec<Delivery> {
    dispatch(
        host,
        WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            button,
            state,
        },
    )
}

#[test]
fn dpi_local_mapping_and_window_clip_are_consistent() {
    let (mut host, left, _) = fixture();
    let deliveries = motion(&mut host, 80.0, 100.0);
    let delivery = deliveries.last().unwrap();
    assert_eq!(delivery.canvas, left);
    assert!(
        matches!(delivery.event, CanvasEvent::PointerMoved { position } if position == LogicalPosition::new(30.0, 30.0))
    );
    let rect = host.canvas(&left).unwrap().rect();
    assert_eq!(
        rect.local_to_window(LogicalPosition::new(30.0, 30.0)),
        LogicalPosition::new(40.0, 50.0)
    );
    assert!(
        motion(&mut host, 1200.0, 100.0)
            .iter()
            .all(|e| !matches!(e.event, CanvasEvent::PointerMoved { .. }))
    );
}

#[test]
fn multiple_buttons_capture_original_canvas_until_last_release() {
    let (mut host, left, right) = fixture();
    motion(&mut host, 80.0, 100.0);
    button(&mut host, MouseButton::Left, ElementState::Pressed);
    button(&mut host, MouseButton::Right, ElementState::Pressed);
    let moved = motion(&mut host, 500.0, 100.0);
    assert_eq!(moved.last().unwrap().canvas, left);
    assert!(
        matches!(moved.last().unwrap().event, CanvasEvent::PointerMoved { position } if position.x == 240.0)
    );
    button(&mut host, MouseButton::Left, ElementState::Released);
    assert_eq!(host.captured(), Some(left.clone()));
    let released = button(&mut host, MouseButton::Right, ElementState::Released);
    assert_eq!(
        released
            .iter()
            .find(|d| matches!(d.event, CanvasEvent::MouseInput { .. }))
            .unwrap()
            .canvas,
        left
    );
    assert_eq!(host.captured(), None);
    assert_eq!(
        motion(&mut host, 500.0, 100.0).last().unwrap().canvas,
        right
    );
}

#[test]
fn removal_tombstones_a_drag_and_never_retargets_late_release() {
    let (mut host, left, right) = fixture();
    motion(&mut host, 80.0, 100.0);
    button(&mut host, MouseButton::Left, ElementState::Pressed);
    let removed = host.remove(&left).unwrap();
    assert!(
        removed
            .iter()
            .any(|d| matches!(d.event, CanvasEvent::Cancelled))
    );
    assert!(matches!(
        host.canvas(&left),
        Err(CanvasError::UnknownCanvas)
    ));
    assert!(
        motion(&mut host, 500.0, 100.0)
            .iter()
            .all(|d| !matches!(d.event, CanvasEvent::PointerMoved { .. }))
    );
    let released = button(&mut host, MouseButton::Left, ElementState::Released);
    assert!(
        released
            .iter()
            .all(|d| !matches!(d.event, CanvasEvent::MouseInput { .. }))
    );
    assert_eq!(
        motion(&mut host, 500.0, 100.0).last().unwrap().canvas,
        right
    );
}

#[test]
fn ime_follows_canvas_focus_and_blur_cancels_interaction() {
    let (mut host, left, right) = fixture();
    host.set_focus(Some(&left)).unwrap();
    assert_eq!(
        dispatch(&mut host, WindowEvent::Ime(Ime::Commit("一".into())))[0].canvas,
        left
    );
    host.set_focus(Some(&right)).unwrap();
    assert_eq!(
        dispatch(
            &mut host,
            WindowEvent::Ime(Ime::Preedit("二".into(), Some((0, 3))))
        )[0]
        .canvas,
        right
    );
    dispatch(&mut host, WindowEvent::Focused(false));
    assert!(dispatch(&mut host, WindowEvent::Ime(Ime::Commit("三".into()))).is_empty());
    assert_eq!(host.focused(), None);
}

#[test]
fn unavailable_and_suspend_release_focus_capture_and_redraw_on_resume() {
    let (mut host, left, _) = fixture();
    motion(&mut host, 80.0, 100.0);
    button(&mut host, MouseButton::Left, ElementState::Pressed);
    let events = host.set_available(&left, false, true).unwrap();
    assert!(
        events
            .iter()
            .any(|d| matches!(d.event, CanvasEvent::Cancelled))
    );
    assert_eq!(host.focused(), None);
    assert_eq!(
        host.set_focus(Some(&left)).unwrap_err(),
        CanvasError::Unavailable
    );
    host.suspend();
    assert!(!host.needs_redraw());
    assert!(motion(&mut host, 500.0, 100.0).is_empty());
    let events = host.resume(host.metrics()).unwrap();
    assert!(
        events
            .iter()
            .any(|d| matches!(d.event, CanvasEvent::Resumed))
    );
    assert!(host.needs_redraw());
}

#[test]
fn redraw_confirmation_is_scoped_and_does_not_erase_new_work() {
    let (mut host, left, right) = fixture();
    let token = host.canvas(&left).unwrap().redraw_token().unwrap();
    assert_eq!(
        host.acknowledge_canvas(&right, token.clone()).unwrap_err(),
        CanvasError::InvalidRedrawToken
    );
    host.request_redraw(&left).unwrap();
    host.acknowledge_canvas(&left, token).unwrap();
    assert!(host.canvas(&left).unwrap().redraw_token().is_some());
    let token = host.canvas(&left).unwrap().redraw_token().unwrap();
    host.acknowledge_canvas(&left, token).unwrap();
    assert!(host.canvas(&left).unwrap().redraw_token().is_none());
    let other = fixture().0;
    assert_eq!(
        host.acknowledge_composite(other.composite_token())
            .unwrap_err(),
        CanvasError::InvalidRedrawToken
    );
}

#[test]
fn overlap_hit_order_and_resize_recompute_local_origin() {
    let (mut host, _, _) = fixture();
    let top = host
        .insert(CanvasRect::new(30.0, 30.0, 80.0, 80.0).unwrap())
        .unwrap();
    assert_eq!(motion(&mut host, 80.0, 100.0).last().unwrap().canvas, top);
    host.set_rect(&top, CanvasRect::new(35.0, 40.0, 80.0, 80.0).unwrap())
        .unwrap();
    assert!(
        matches!(motion(&mut host, 80.0, 100.0).last().unwrap().event, CanvasEvent::PointerMoved { position } if position == LogicalPosition::new(5.0, 10.0))
    );
    let metrics = WindowMetrics::new(PhysicalSize::new(500, 300), 1.0).unwrap();
    host.set_metrics(metrics).unwrap();
    assert!(
        matches!(motion(&mut host, 80.0, 100.0).last().unwrap().event, CanvasEvent::PointerMoved { position } if position == LogicalPosition::new(45.0, 60.0))
    );
}

#[test]
fn pixel_scroll_session_locks_target_while_line_scroll_is_an_impulse() {
    let (mut host, left, right) = fixture();
    motion(&mut host, 80.0, 100.0);
    let wheel = |phase| WindowEvent::MouseWheel {
        device_id: DeviceId::dummy(),
        phase,
        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(8.0, 12.0)),
    };
    let started = dispatch(&mut host, wheel(TouchPhase::Started));
    assert_eq!(started[0].canvas, left);
    assert!(
        matches!(started[0].event, CanvasEvent::Wheel { delta: MouseScrollDelta::PixelDelta(p), .. } if p.x == 4.0 && p.y == 6.0)
    );
    motion(&mut host, 500.0, 100.0);
    assert_eq!(
        dispatch(&mut host, wheel(TouchPhase::Moved))[0].canvas,
        left
    );
    host.remove(&left).unwrap();
    assert!(dispatch(&mut host, wheel(TouchPhase::Moved)).is_empty());
    assert!(dispatch(&mut host, wheel(TouchPhase::Ended)).is_empty());
    let lines = dispatch(
        &mut host,
        WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(0.0, 1.0),
            phase: TouchPhase::Moved,
        },
    );
    assert_eq!(lines[0].canvas, right);
}

#[test]
fn invalid_geometry_empty_canvases_and_foreign_ids_are_checked() {
    assert_eq!(
        CanvasRect::new(0.0, 0.0, f64::NAN, 2.0).unwrap_err(),
        CanvasError::InvalidGeometry
    );
    assert_eq!(
        WindowMetrics::new(PhysicalSize::new(1, 1), 0.0).unwrap_err(),
        CanvasError::InvalidScaleFactor
    );
    let (mut host, _, _) = fixture();
    let (other, foreign, _) = fixture();
    assert_eq!(
        host.remove(&foreign).unwrap_err(),
        CanvasError::UnknownCanvas
    );
    let rect = CanvasRect::new(0.0, 0.0, 0.0, 0.0).unwrap();
    let empty = host.insert(rect).unwrap();
    assert!(host.canvas(&empty).unwrap().rect().is_empty());
    assert_eq!(
        other.metrics().texture_size(rect).unwrap(),
        PhysicalSize::new(0, 0)
    );
}

#[test]
fn touch_and_pinch_keep_their_canvas_until_the_session_ends() {
    let (mut host, left, right) = fixture();
    let touch = |phase, x| {
        WindowEvent::Touch(winit::event::Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x, 100.0),
            force: None,
            id: 1,
        })
    };
    assert_eq!(
        dispatch(&mut host, touch(TouchPhase::Started, 80.0))[0].canvas,
        left
    );
    let moved = dispatch(&mut host, touch(TouchPhase::Moved, 500.0));
    assert_eq!(moved[0].canvas, left);
    assert!(matches!(moved[0].event, CanvasEvent::Touch { position, .. } if position.x == 240.0));
    host.set_available(&left, true, false).unwrap();
    assert!(dispatch(&mut host, touch(TouchPhase::Moved, 500.0)).is_empty());
    assert!(dispatch(&mut host, touch(TouchPhase::Ended, 500.0)).is_empty());
    motion(&mut host, 500.0, 100.0);
    let pinch = |phase| WindowEvent::PinchGesture {
        device_id: DeviceId::dummy(),
        delta: 0.1,
        phase,
    };
    assert_eq!(
        dispatch(&mut host, pinch(TouchPhase::Started))[0].canvas,
        right
    );
    motion(&mut host, 800.0, 500.0);
    assert_eq!(
        dispatch(&mut host, pinch(TouchPhase::Moved))[0].canvas,
        right
    );
    assert_eq!(
        dispatch(&mut host, pinch(TouchPhase::Ended))[0].canvas,
        right
    );
}
