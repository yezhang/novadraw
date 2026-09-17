use novadraw_geometry::Point;

use super::focus::FocusChange;
use crate::FigureId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    Pressed,
    Released,
    Moved,
    Dragged,
    Hover,
    DoubleClicked,
    Entered,
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    /// 鼠标点在当前 target/source Figure node-local domain 中的 x 值。
    pub x: f64,
    /// 鼠标点在当前 target/source Figure node-local domain 中的 y 值。
    pub y: f64,
    pub button: MouseButton,
    entry_point: Point,
}

impl MouseEvent {
    /// 创建一个入口域鼠标事件。
    ///
    /// 此时 `x/y` 与 `entry_point()` 相同；引擎在投递给 target 前会调用
    /// `with_target_point()` 生成 target/source Figure node-local 事件点。
    pub fn new(kind: MouseEventKind, x: f64, y: f64, button: MouseButton) -> Self {
        Self {
            kind,
            x,
            y,
            button,
            entry_point: Point::new(x, y),
        }
    }

    /// 返回平台输入归一化后的入口节点坐标域点。
    ///
    /// 该点只读保留，用于调试、录制回放或跨 target 手势分析；Figure 的常规业务逻辑
    /// 应优先使用 `x/y`，它们已在引擎层转换到 target/source node-local domain。
    pub fn entry_point(&self) -> Point {
        self.entry_point
    }

    /// 返回一个保留 entry point、但使用 target/source 坐标域点的新事件。
    pub fn with_target_point(self, x: f64, y: f64) -> Self {
        Self { x, y, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelEvent {
    pub x: f64,
    pub y: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_kind: ScrollDeltaKind,
    pub phase: GesturePhase,
    pub modifiers: KeyModifiers,
    pub session_id: GestureSessionId,
    entry_point: Point,
}

impl WheelEvent {
    pub fn new(x: f64, y: f64, delta_x: f64, delta_y: f64) -> Self {
        Self::with_details(
            x,
            y,
            delta_x,
            delta_y,
            ScrollDeltaKind::Lines,
            GesturePhase::Impulse,
            KeyModifiers::default(),
            GestureSessionId::IMPULSE,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_details(
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        delta_kind: ScrollDeltaKind,
        phase: GesturePhase,
        modifiers: KeyModifiers,
        session_id: GestureSessionId,
    ) -> Self {
        Self {
            x,
            y,
            delta_x,
            delta_y,
            delta_kind,
            phase,
            modifiers,
            session_id,
            entry_point: Point::new(x, y),
        }
    }

    pub fn entry_point(&self) -> Point {
        self.entry_point
    }

    pub fn with_target_point(self, x: f64, y: f64) -> Self {
        Self { x, y, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GestureSessionId(u64);

impl GestureSessionId {
    pub const IMPULSE: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Begin,
    Update,
    End,
    Cancel,
    Impulse,
}

impl GesturePhase {
    fn starts_session(self) -> bool {
        matches!(self, Self::Begin)
    }

    fn ends_session(self) -> bool {
        matches!(self, Self::End | Self::Cancel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDeltaKind {
    Lines,
    LogicalPixels,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomEvent {
    pub x: f64,
    pub y: f64,
    pub scale_factor: f64,
    pub phase: GesturePhase,
    pub modifiers: KeyModifiers,
    pub session_id: GestureSessionId,
    entry_point: Point,
}

impl ZoomEvent {
    pub fn new(
        x: f64,
        y: f64,
        scale_factor: f64,
        phase: GesturePhase,
        modifiers: KeyModifiers,
        session_id: GestureSessionId,
    ) -> Self {
        Self {
            x,
            y,
            scale_factor,
            phase,
            modifiers,
            session_id,
            entry_point: Point::new(x, y),
        }
    }

    pub fn entry_point(&self) -> Point {
        self.entry_point
    }

    pub fn with_target_point(self, x: f64, y: f64) -> Self {
        Self { x, y, ..self }
    }

    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.scale_factor.is_finite()
            && self.scale_factor > 0.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Enter,
    Escape,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub kind: KeyEventKind,
    pub key: Key,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEventKind {
    Gained,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusEvent {
    pub kind: FocusEventKind,
    pub related_target: Option<FigureId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Mouse(MouseEvent),
    Wheel(WheelEvent),
    Zoom(ZoomEvent),
    Key(KeyEvent),
    Focus(FocusEvent),
}

/// Observable result of dispatching one normalized input event.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DispatchOutcome {
    target: Option<FigureId>,
    handled: bool,
    capture: Option<FigureId>,
}

impl DispatchOutcome {
    fn new(target: Option<FigureId>, handled: bool, capture: Option<FigureId>) -> Self {
        Self {
            target,
            handled,
            capture,
        }
    }

    /// Returns the Figure that received the event.
    pub const fn target(self) -> Option<FigureId> {
        self.target
    }

    /// Returns whether a Figure handler or built-in Runtime fallback handled the event.
    pub const fn is_handled(self) -> bool {
        self.handled
    }

    /// Returns the primary pointer capture after dispatch.
    pub const fn capture(self) -> Option<FigureId> {
        self.capture
    }
}

pub trait DispatchContext {
    fn find_mouse_event_target_at(&self, x: f64, y: f64) -> Option<FigureId>;
    fn find_cursor_target_at(&self, x: f64, y: f64) -> Option<FigureId> {
        self.find_mouse_event_target_at(x, y)
    }
    fn find_hover_source_at(&self, x: f64, y: f64) -> Option<FigureId> {
        self.find_cursor_target_at(x, y)
    }
    fn find_gesture_target_at(&self, x: f64, y: f64) -> Option<FigureId> {
        self.find_mouse_event_target_at(x, y)
    }
    fn mouse_target(&self) -> Option<FigureId>;
    fn set_mouse_target(&mut self, id: Option<FigureId>);
    fn cursor_target(&self) -> Option<FigureId>;
    fn set_cursor_target(&mut self, id: Option<FigureId>);
    fn hover_source(&self) -> Option<FigureId>;
    fn set_hover_source(&mut self, id: Option<FigureId>);
    fn set_hovered(&mut self, id: FigureId, hovered: bool);
    fn set_pressed(&mut self, id: FigureId, pressed: bool);
    fn focus_owner(&self) -> Option<FigureId>;
    fn set_focus_owner(&mut self, id: Option<FigureId>);
    fn can_request_focus(&self, _target_id: FigureId) -> bool {
        false
    }
    fn captured(&self) -> Option<FigureId>;
    fn set_captured(&mut self, id: Option<FigureId>);
    fn gesture_target(&self, _session_id: GestureSessionId) -> Option<FigureId> {
        None
    }
    fn has_gesture_session(&self, _session_id: GestureSessionId) -> bool {
        false
    }
    fn set_gesture_target(&mut self, _session_id: GestureSessionId, _target_id: Option<FigureId>) {}
    fn clear_gesture_target(&mut self, _session_id: GestureSessionId) {}
    fn clear_gesture_targets(&mut self) {}
    fn apply_scroll_fallback(&mut self, _target_id: FigureId, _event: &WheelEvent) -> bool {
        false
    }
    fn apply_zoom_fallback(&mut self, _target_id: FigureId, _event: &ZoomEvent) -> bool {
        false
    }
    /// 将事件投递给 target。
    ///
    /// 传入的 `Event` 使用入口节点坐标域；具体实现负责在投递前把鼠标点转换到
    /// target Figure 的坐标域，以对齐 draw2d 的 `source.translateToRelative()` 语义。
    fn dispatch_to_target(&mut self, target_id: Option<FigureId>, event: &Event) -> bool;
}

#[derive(Default)]
pub struct EventDispatcher;

impl EventDispatcher {
    fn gesture_target(
        &self,
        ctx: &mut dyn DispatchContext,
        session_id: GestureSessionId,
        phase: GesturePhase,
        x: f64,
        y: f64,
    ) -> Option<FigureId> {
        if phase == GesturePhase::Impulse || session_id == GestureSessionId::IMPULSE {
            return ctx.find_gesture_target_at(x, y);
        }
        if phase.starts_session() {
            let target = ctx.find_gesture_target_at(x, y);
            ctx.set_gesture_target(session_id, target);
            return target;
        }
        if ctx.has_gesture_session(session_id) {
            ctx.gesture_target(session_id)
        } else {
            let target = ctx.find_gesture_target_at(x, y);
            ctx.set_gesture_target(session_id, target);
            target
        }
    }

    fn refresh_mouse_target(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        let captured = ctx.captured();
        ctx.set_cursor_target(ctx.find_cursor_target_at(x, y));
        let hit_target = if captured.is_none() {
            let hit_target = ctx.find_mouse_event_target_at(x, y);
            ctx.set_hover_source(ctx.find_hover_source_at(x, y));
            hit_target
        } else {
            None
        };
        let next_target = captured.or(hit_target);
        let previous_target = ctx.mouse_target();
        if previous_target != next_target {
            if let Some(previous_target) = previous_target {
                ctx.set_hovered(previous_target, false);
                let exited = Event::Mouse(MouseEvent::new(
                    MouseEventKind::Exited,
                    x,
                    y,
                    MouseButton::None,
                ));
                let _ = ctx.dispatch_to_target(Some(previous_target), &exited);
            }
            ctx.set_mouse_target(next_target);
            if let Some(next_target) = next_target {
                ctx.set_hovered(next_target, true);
                let entered = Event::Mouse(MouseEvent::new(
                    MouseEventKind::Entered,
                    x,
                    y,
                    MouseButton::None,
                ));
                let _ = ctx.dispatch_to_target(Some(next_target), &entered);
            }
        }
    }

    fn update_focus(
        &mut self,
        ctx: &mut dyn DispatchContext,
        next: Option<FigureId>,
    ) -> FocusChange {
        let previous = ctx.focus_owner();
        if previous == next {
            return FocusChange::Unchanged;
        }
        ctx.set_focus_owner(next);
        if let Some(previous) = previous {
            let lost = Event::Focus(FocusEvent {
                kind: FocusEventKind::Lost,
                related_target: next,
            });
            let _ = ctx.dispatch_to_target(Some(previous), &lost);
        }
        if let Some(next) = next {
            let gained = Event::Focus(FocusEvent {
                kind: FocusEventKind::Gained,
                related_target: previous,
            });
            let _ = ctx.dispatch_to_target(Some(next), &gained);
        }
        FocusChange::Changed {
            previous,
            current: next,
        }
    }

    fn dispatch_mouse_event(
        &mut self,
        ctx: &mut dyn DispatchContext,
        kind: MouseEventKind,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.refresh_mouse_target(ctx, x, y);
        let target = ctx.mouse_target();
        let event = Event::Mouse(MouseEvent::new(kind, x, y, button));
        let handled = ctx.dispatch_to_target(target, &event);
        DispatchOutcome::new(target, handled, ctx.captured())
    }
}

impl EventDispatcher {
    pub fn receive(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        self.refresh_mouse_target(ctx, x, y);
    }

    pub fn dispatch_mouse_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.refresh_mouse_target(ctx, x, y);
        let target = ctx.mouse_target();
        let event = Event::Mouse(MouseEvent::new(MouseEventKind::Pressed, x, y, button));
        let handled = ctx.dispatch_to_target(target, &event);
        if handled {
            ctx.set_captured(target);
            if let Some(target) = target {
                ctx.set_pressed(target, true);
            }
            if target.is_some_and(|target| ctx.can_request_focus(target)) {
                self.update_focus(ctx, target);
            }
        }
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn dispatch_mouse_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.refresh_mouse_target(ctx, x, y);
        let target = ctx.mouse_target();
        let event = Event::Mouse(MouseEvent::new(MouseEventKind::Released, x, y, button));
        let handled = ctx.dispatch_to_target(target, &event);
        if let Some(captured) = ctx.captured() {
            ctx.set_pressed(captured, false);
            ctx.set_captured(None);
            self.refresh_mouse_target(ctx, x, y);
        }
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn dispatch_mouse_moved(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
    ) -> DispatchOutcome {
        let kind = if ctx.captured().is_some() {
            MouseEventKind::Dragged
        } else {
            MouseEventKind::Moved
        };
        self.dispatch_mouse_event(ctx, kind, x, y, MouseButton::None)
    }

    pub fn dispatch_pointer_exited(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        if let Some(captured) = ctx.captured() {
            ctx.set_pressed(captured, false);
            ctx.set_captured(None);
        }
        if let Some(previous_target) = ctx.mouse_target() {
            ctx.set_hovered(previous_target, false);
            let exited = Event::Mouse(MouseEvent::new(
                MouseEventKind::Exited,
                x,
                y,
                MouseButton::None,
            ));
            let _ = ctx.dispatch_to_target(Some(previous_target), &exited);
        }
        ctx.set_mouse_target(None);
        ctx.set_hover_source(None);
        ctx.set_cursor_target(None);
    }

    pub fn dispatch_mouse_double_clicked(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) -> DispatchOutcome {
        self.dispatch_mouse_event(ctx, MouseEventKind::DoubleClicked, x, y, button)
    }

    pub fn dispatch_mouse_hover(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
    ) -> DispatchOutcome {
        self.refresh_mouse_target(ctx, x, y);
        let event = Event::Mouse(MouseEvent::new(
            MouseEventKind::Hover,
            x,
            y,
            MouseButton::None,
        ));
        let target = ctx.mouse_target();
        let handled = ctx.dispatch_to_target(target, &event);
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn dispatch_mouse_wheel(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
    ) -> DispatchOutcome {
        self.dispatch_scroll(ctx, WheelEvent::new(x, y, delta_x, delta_y))
    }

    pub fn dispatch_scroll(
        &mut self,
        ctx: &mut dyn DispatchContext,
        wheel_event: WheelEvent,
    ) -> DispatchOutcome {
        if !wheel_event.x.is_finite()
            || !wheel_event.y.is_finite()
            || !wheel_event.delta_x.is_finite()
            || !wheel_event.delta_y.is_finite()
        {
            return DispatchOutcome::default();
        }
        let session_id = wheel_event.session_id;
        let phase = wheel_event.phase;
        let target = self.gesture_target(ctx, session_id, phase, wheel_event.x, wheel_event.y);
        let event = Event::Wheel(wheel_event);
        let handled = ctx.dispatch_to_target(target, &event);
        let handled =
            handled || target.is_some_and(|target| ctx.apply_scroll_fallback(target, &wheel_event));
        if phase.ends_session() {
            ctx.clear_gesture_target(session_id);
        }
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn dispatch_zoom(
        &mut self,
        ctx: &mut dyn DispatchContext,
        zoom_event: ZoomEvent,
    ) -> DispatchOutcome {
        if !zoom_event.is_valid() {
            return DispatchOutcome::default();
        }
        let session_id = zoom_event.session_id;
        let phase = zoom_event.phase;
        let initial_target =
            self.gesture_target(ctx, session_id, phase, zoom_event.x, zoom_event.y);
        let event = Event::Zoom(zoom_event);
        let handled = ctx.dispatch_to_target(initial_target, &event);
        let handled = handled
            || initial_target.is_some_and(|target| ctx.apply_zoom_fallback(target, &zoom_event));
        if phase.ends_session() {
            ctx.clear_gesture_target(session_id);
        }
        DispatchOutcome::new(initial_target, handled, ctx.captured())
    }

    pub fn cancel_gestures(&mut self, ctx: &mut dyn DispatchContext) {
        ctx.clear_gesture_targets();
    }

    pub fn dispatch_key_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    ) -> DispatchOutcome {
        let target = ctx.focus_owner();
        let event = Event::Key(KeyEvent {
            kind: KeyEventKind::Pressed,
            key,
            modifiers,
        });
        let handled = ctx.dispatch_to_target(target, &event);
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn dispatch_key_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    ) -> DispatchOutcome {
        let target = ctx.focus_owner();
        let event = Event::Key(KeyEvent {
            kind: KeyEventKind::Released,
            key,
            modifiers,
        });
        let handled = ctx.dispatch_to_target(target, &event);
        DispatchOutcome::new(target, handled, ctx.captured())
    }

    pub fn request_focus(
        &mut self,
        ctx: &mut dyn DispatchContext,
        target: FigureId,
    ) -> FocusChange {
        if !ctx.can_request_focus(target) {
            return FocusChange::Unchanged;
        }
        self.update_focus(ctx, Some(target))
    }

    pub fn release_focus(&mut self, ctx: &mut dyn DispatchContext) -> FocusChange {
        self.update_focus(ctx, None)
    }

    pub(crate) fn set_focus(
        &mut self,
        ctx: &mut dyn DispatchContext,
        target: Option<FigureId>,
    ) -> FocusChange {
        self.update_focus(ctx, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FigureTree, RectangleFigure};

    struct MockDispatchContext {
        hit_target: Option<FigureId>,
        cursor_hit: Option<FigureId>,
        hover_hit: Option<FigureId>,
        mouse_target: Option<FigureId>,
        cursor_target: Option<FigureId>,
        hover_source: Option<FigureId>,
        focus_owner: Option<FigureId>,
        captured: Option<FigureId>,
        dispatched: Vec<(Option<FigureId>, Event)>,
        scroll_fallbacks: Vec<(FigureId, WheelEvent)>,
        zoom_fallbacks: Vec<(FigureId, ZoomEvent)>,
        handled: bool,
        can_request_focus: bool,
        focus_owner_snapshots: Vec<Option<FigureId>>,
    }

    impl MockDispatchContext {
        fn new(hit_target: Option<FigureId>) -> Self {
            Self {
                hit_target,
                cursor_hit: hit_target,
                hover_hit: hit_target,
                mouse_target: None,
                cursor_target: None,
                hover_source: None,
                focus_owner: None,
                captured: None,
                dispatched: Vec::new(),
                scroll_fallbacks: Vec::new(),
                zoom_fallbacks: Vec::new(),
                handled: false,
                can_request_focus: false,
                focus_owner_snapshots: Vec::new(),
            }
        }
    }

    impl DispatchContext for MockDispatchContext {
        fn find_mouse_event_target_at(&self, _x: f64, _y: f64) -> Option<FigureId> {
            self.hit_target
        }

        fn find_cursor_target_at(&self, _x: f64, _y: f64) -> Option<FigureId> {
            self.cursor_hit
        }

        fn find_hover_source_at(&self, _x: f64, _y: f64) -> Option<FigureId> {
            self.hover_hit
        }

        fn mouse_target(&self) -> Option<FigureId> {
            self.mouse_target
        }

        fn set_mouse_target(&mut self, id: Option<FigureId>) {
            self.mouse_target = id;
        }

        fn cursor_target(&self) -> Option<FigureId> {
            self.cursor_target
        }

        fn set_cursor_target(&mut self, id: Option<FigureId>) {
            self.cursor_target = id;
        }

        fn hover_source(&self) -> Option<FigureId> {
            self.hover_source
        }

        fn set_hover_source(&mut self, id: Option<FigureId>) {
            self.hover_source = id;
        }

        fn set_hovered(&mut self, _id: FigureId, _hovered: bool) {}

        fn set_pressed(&mut self, _id: FigureId, _pressed: bool) {}

        fn focus_owner(&self) -> Option<FigureId> {
            self.focus_owner
        }

        fn set_focus_owner(&mut self, id: Option<FigureId>) {
            self.focus_owner = id;
        }

        fn captured(&self) -> Option<FigureId> {
            self.captured
        }

        fn set_captured(&mut self, id: Option<FigureId>) {
            self.captured = id;
        }

        fn apply_scroll_fallback(&mut self, target_id: FigureId, event: &WheelEvent) -> bool {
            self.scroll_fallbacks.push((target_id, *event));
            true
        }

        fn apply_zoom_fallback(&mut self, target_id: FigureId, event: &ZoomEvent) -> bool {
            self.zoom_fallbacks.push((target_id, *event));
            true
        }

        fn can_request_focus(&self, _target_id: FigureId) -> bool {
            self.can_request_focus
        }

        fn dispatch_to_target(&mut self, target_id: Option<FigureId>, event: &Event) -> bool {
            self.focus_owner_snapshots.push(self.focus_owner);
            self.dispatched.push((target_id, *event));
            self.handled
        }
    }

    #[test]
    fn test_receive_updates_mouse_target() {
        let mut dispatcher = EventDispatcher;
        let mut ctx = MockDispatchContext::new(None);
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        ctx.hit_target = Some(target);
        ctx.cursor_hit = Some(target);
        ctx.hover_hit = Some(target);

        dispatcher.receive(&mut ctx, 10.0, 20.0);

        assert_eq!(ctx.mouse_target(), Some(target));
        assert_eq!(ctx.cursor_target(), Some(target));
        assert_eq!(ctx.hover_source(), Some(target));
        assert_eq!(ctx.dispatched.len(), 1);
        assert_eq!(ctx.dispatched[0].0, Some(target));
        assert_eq!(
            ctx.dispatched[0].1,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::Entered,
                10.0,
                20.0,
                MouseButton::None,
            ))
        );
    }

    #[test]
    fn mouse_target_transitions_are_independent_of_cursor_and_hover_sources() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let event_target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let visual_hit = scene.add_child_to(
            event_target,
            Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)),
        );
        let mut ctx = MockDispatchContext::new(Some(event_target));
        ctx.cursor_hit = Some(visual_hit);
        ctx.hover_hit = Some(visual_hit);

        dispatcher.receive(&mut ctx, 2.0, 2.0);

        assert_eq!(ctx.mouse_target(), Some(event_target));
        assert_eq!(ctx.cursor_target(), Some(visual_hit));
        assert_eq!(ctx.hover_source(), Some(visual_hit));
        assert_eq!(
            ctx.dispatched,
            vec![(
                Some(event_target),
                Event::Mouse(MouseEvent::new(
                    MouseEventKind::Entered,
                    2.0,
                    2.0,
                    MouseButton::None,
                )),
            )]
        );

        ctx.dispatched.clear();
        ctx.cursor_hit = Some(event_target);
        ctx.hover_hit = Some(event_target);
        dispatcher.receive(&mut ctx, 8.0, 8.0);

        assert!(ctx.dispatched.is_empty());
        assert_eq!(ctx.mouse_target(), Some(event_target));
        assert_eq!(ctx.cursor_target(), Some(event_target));
        assert_eq!(ctx.hover_source(), Some(event_target));
    }

    #[test]
    fn test_captured_target_overrides_hit_target() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let hit_target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let captured = scene.add_child_to(
            hit_target,
            Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)),
        );
        let mut ctx = MockDispatchContext::new(Some(hit_target));
        ctx.set_captured(Some(captured));

        dispatcher.dispatch_mouse_moved(&mut ctx, 5.0, 6.0);

        assert_eq!(ctx.mouse_target(), Some(captured));
        assert_eq!(ctx.dispatched.last().unwrap().0, Some(captured));
    }

    #[test]
    fn test_press_sets_capture_when_handled() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.handled = true;

        dispatcher.dispatch_mouse_pressed(&mut ctx, 4.0, 4.0, MouseButton::Left);

        assert_eq!(ctx.mouse_target(), Some(target));
        assert_eq!(ctx.captured(), Some(target));
        assert_eq!(ctx.dispatched.last().unwrap().0, Some(target));
    }

    #[test]
    fn test_release_uses_capture_and_then_clears_it() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(None);
        ctx.mouse_target = Some(target);
        ctx.hover_source = Some(target);
        ctx.captured = Some(target);
        ctx.handled = true;

        dispatcher.dispatch_mouse_released(&mut ctx, 40.0, 40.0, MouseButton::Left);

        assert_eq!(ctx.captured(), None);
        assert_eq!(ctx.mouse_target(), None);
        assert_eq!(ctx.dispatched[0].0, Some(target));
        assert_eq!(
            ctx.dispatched[0].1,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::Released,
                40.0,
                40.0,
                MouseButton::Left,
            ))
        );
        assert_eq!(
            ctx.dispatched[1].1,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::Exited,
                40.0,
                40.0,
                MouseButton::None,
            ))
        );
    }

    #[test]
    fn pointer_exit_clears_capture_and_exits_the_mouse_target() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let tooltip_source =
            scene.add_child_to(target, Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.mouse_target = Some(target);
        ctx.hover_source = Some(tooltip_source);
        ctx.captured = Some(target);

        dispatcher.dispatch_pointer_exited(&mut ctx, 12.0, 13.0);

        assert_eq!(ctx.captured(), None);
        assert_eq!(ctx.mouse_target(), None);
        assert_eq!(ctx.cursor_target(), None);
        assert_eq!(ctx.hover_source(), None);
        assert_eq!(
            ctx.dispatched,
            vec![(
                Some(target),
                Event::Mouse(MouseEvent::new(
                    MouseEventKind::Exited,
                    12.0,
                    13.0,
                    MouseButton::None,
                )),
            )]
        );
    }

    #[test]
    fn test_drag_keeps_mouse_target_while_cursor_tracks_physical_hit() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let root = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let captured = scene.add_child_to(root, Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)));
        let mut ctx = MockDispatchContext::new(Some(root));
        ctx.captured = Some(captured);

        dispatcher.dispatch_mouse_moved(&mut ctx, 8.0, 8.0);

        assert_eq!(ctx.cursor_target(), Some(root));
        assert_eq!(ctx.hover_source(), None);
        assert_eq!(ctx.mouse_target(), Some(captured));
        assert_eq!(
            ctx.dispatched.last(),
            Some(&(
                Some(captured),
                Event::Mouse(MouseEvent::new(
                    MouseEventKind::Dragged,
                    8.0,
                    8.0,
                    MouseButton::None,
                )),
            ))
        );
    }

    #[test]
    fn test_handled_press_assigns_focus_and_key_events_follow_focus_owner() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.handled = true;
        ctx.can_request_focus = true;

        dispatcher.dispatch_mouse_pressed(&mut ctx, 4.0, 4.0, MouseButton::Left);
        dispatcher.dispatch_key_pressed(&mut ctx, Key::Character('a'), KeyModifiers::default());

        assert_eq!(ctx.focus_owner(), Some(target));
        assert!(ctx.dispatched.iter().any(|(_, event)| {
            *event
                == Event::Focus(FocusEvent {
                    kind: FocusEventKind::Gained,
                    related_target: None,
                })
        }));
        assert_eq!(
            ctx.dispatched.last(),
            Some(&(
                Some(target),
                Event::Key(KeyEvent {
                    kind: KeyEventKind::Pressed,
                    key: Key::Character('a'),
                    modifiers: KeyModifiers::default(),
                }),
            ))
        );
    }

    #[test]
    fn handled_press_on_ineligible_target_preserves_existing_focus() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let old_focus = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let target = scene.add_child_to(
            old_focus,
            Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)),
        );
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.focus_owner = Some(old_focus);
        ctx.handled = true;

        dispatcher.dispatch_mouse_pressed(&mut ctx, 2.0, 2.0, MouseButton::Left);

        assert_eq!(ctx.focus_owner(), Some(old_focus));
        assert!(
            !ctx.dispatched
                .iter()
                .any(|(_, event)| matches!(event, Event::Focus(_)))
        );
    }

    #[test]
    fn focus_transition_commits_owner_before_lost_and_gained_events() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let previous = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let next = scene.add_child_to(previous, Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)));
        let mut ctx = MockDispatchContext::new(None);
        ctx.focus_owner = Some(previous);
        ctx.can_request_focus = true;

        let change = dispatcher.request_focus(&mut ctx, next);

        assert_eq!(
            change,
            FocusChange::Changed {
                previous: Some(previous),
                current: Some(next),
            }
        );
        assert_eq!(
            ctx.dispatched,
            vec![
                (
                    Some(previous),
                    Event::Focus(FocusEvent {
                        kind: FocusEventKind::Lost,
                        related_target: Some(next),
                    }),
                ),
                (
                    Some(next),
                    Event::Focus(FocusEvent {
                        kind: FocusEventKind::Gained,
                        related_target: Some(previous),
                    }),
                ),
            ]
        );
        assert_eq!(ctx.focus_owner_snapshots, vec![Some(next), Some(next)]);
    }

    #[test]
    fn test_wheel_hover_and_double_click_use_pointer_target() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let hover_source =
            scene.add_child_to(target, Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.hover_hit = Some(hover_source);

        dispatcher.dispatch_mouse_hover(&mut ctx, 2.0, 3.0);
        dispatcher.dispatch_mouse_wheel(&mut ctx, 2.0, 3.0, 0.0, -1.0);
        dispatcher.dispatch_mouse_double_clicked(&mut ctx, 2.0, 3.0, MouseButton::Left);

        assert!(ctx.dispatched.iter().any(|(receiver, event)| {
            *receiver == Some(target)
                && matches!(
                    event,
                    Event::Mouse(MouseEvent {
                        kind: MouseEventKind::Hover,
                        ..
                    })
                )
        }));
        assert!(
            ctx.dispatched
                .iter()
                .any(|(_, event)| matches!(event, Event::Wheel(_)))
        );
        assert!(ctx.dispatched.iter().any(|(_, event)| {
            matches!(
                event,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::DoubleClicked,
                    ..
                })
            )
        }));
    }

    #[test]
    fn unhandled_gestures_dispatch_once_before_specialized_fallback() {
        let mut dispatcher = EventDispatcher;
        let mut scene = FigureTree::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        let wheel = WheelEvent::new(2.0, 3.0, 0.0, -1.0);
        let zoom = ZoomEvent::new(
            2.0,
            3.0,
            1.1,
            GesturePhase::Impulse,
            KeyModifiers::default(),
            GestureSessionId::IMPULSE,
        );

        dispatcher.dispatch_scroll(&mut ctx, wheel);
        dispatcher.dispatch_zoom(&mut ctx, zoom);

        assert_eq!(
            ctx.dispatched,
            vec![
                (Some(target), Event::Wheel(wheel)),
                (Some(target), Event::Zoom(zoom)),
            ]
        );
        assert_eq!(ctx.scroll_fallbacks, vec![(target, wheel)]);
        assert_eq!(ctx.zoom_fallbacks, vec![(target, zoom)]);
    }
}
