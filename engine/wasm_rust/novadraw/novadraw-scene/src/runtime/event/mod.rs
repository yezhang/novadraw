use novadraw_geometry::Point;

use crate::BlockId;

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
    /// 鼠标点在当前 target/source Figure 坐标域中的 x 值。
    pub x: f64,
    /// 鼠标点在当前 target/source Figure 坐标域中的 y 值。
    pub y: f64,
    pub button: MouseButton,
    entry_point: Point,
}

impl MouseEvent {
    /// 创建一个入口域鼠标事件。
    ///
    /// 此时 `x/y` 与 `entry_point()` 相同；引擎在投递给 target 前会调用
    /// `with_target_point()` 生成 target/source Figure 坐标域中的事件点。
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
    /// 应优先使用 `x/y`，它们已在引擎层转换到当前 target/source Figure 坐标域。
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
    entry_point: Point,
}

impl WheelEvent {
    pub fn new(x: f64, y: f64, delta_x: f64, delta_y: f64) -> Self {
        Self {
            x,
            y,
            delta_x,
            delta_y,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Mouse(MouseEvent),
    Wheel(WheelEvent),
    Key(KeyEvent),
    Focus(FocusEvent),
}

pub trait DispatchContext {
    fn find_mouse_event_target_at(&self, x: f64, y: f64) -> Option<BlockId>;
    fn mouse_target(&self) -> Option<BlockId>;
    fn set_mouse_target(&mut self, id: Option<BlockId>);
    fn cursor_target(&self) -> Option<BlockId>;
    fn set_cursor_target(&mut self, id: Option<BlockId>);
    fn hover_source(&self) -> Option<BlockId>;
    fn set_hover_source(&mut self, id: Option<BlockId>);
    fn set_hovered(&mut self, id: BlockId, hovered: bool);
    fn set_pressed(&mut self, id: BlockId, pressed: bool);
    fn focus_owner(&self) -> Option<BlockId>;
    fn set_focus_owner(&mut self, id: Option<BlockId>);
    fn captured(&self) -> Option<BlockId>;
    fn set_captured(&mut self, id: Option<BlockId>);
    fn wants_key_events(&self, _target_id: BlockId) -> bool {
        false
    }
    /// 将事件投递给 target。
    ///
    /// 传入的 `Event` 使用入口节点坐标域；具体实现负责在投递前把鼠标点转换到
    /// target Figure 的坐标域，以对齐 draw2d 的 `source.translateToRelative()` 语义。
    fn dispatch_to_target(&mut self, target_id: Option<BlockId>, event: &Event) -> bool;
}

pub trait EventDispatcher: Send + Sync {
    fn receive(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64);
    fn dispatch_mouse_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    );
    fn dispatch_mouse_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    );
    fn dispatch_mouse_moved(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64);
    fn dispatch_mouse_double_clicked(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    );
    fn dispatch_mouse_hover(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64);
    fn dispatch_mouse_wheel(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
    );
    fn dispatch_key_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    );
    fn dispatch_key_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    );
    fn request_focus(&mut self, ctx: &mut dyn DispatchContext, target: Option<BlockId>);
    fn release_focus(&mut self, ctx: &mut dyn DispatchContext);
}

#[derive(Default)]
pub struct BasicEventDispatcher;

impl BasicEventDispatcher {
    fn refresh_mouse_target(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        let hit_target = ctx.find_mouse_event_target_at(x, y);
        ctx.set_cursor_target(hit_target);

        let previous_hover = ctx.hover_source();
        if previous_hover != hit_target {
            if let Some(previous_hover) = previous_hover {
                ctx.set_hovered(previous_hover, false);
                let exited = Event::Mouse(MouseEvent::new(
                    MouseEventKind::Exited,
                    x,
                    y,
                    MouseButton::None,
                ));
                let _ = ctx.dispatch_to_target(Some(previous_hover), &exited);
            }
            ctx.set_hover_source(hit_target);
            if let Some(hit_target) = hit_target {
                ctx.set_hovered(hit_target, true);
                let entered = Event::Mouse(MouseEvent::new(
                    MouseEventKind::Entered,
                    x,
                    y,
                    MouseButton::None,
                ));
                let _ = ctx.dispatch_to_target(Some(hit_target), &entered);
            }
        }

        let captured = ctx.captured();
        let next_target = captured.or(hit_target);
        ctx.set_mouse_target(next_target);
    }

    fn update_focus(&mut self, ctx: &mut dyn DispatchContext, requested: Option<BlockId>) {
        let next = requested.filter(|target| ctx.wants_key_events(*target));
        let previous = ctx.focus_owner();
        if previous == next {
            return;
        }
        if let Some(previous) = previous {
            let lost = Event::Focus(FocusEvent {
                kind: FocusEventKind::Lost,
            });
            let _ = ctx.dispatch_to_target(Some(previous), &lost);
        }
        ctx.set_focus_owner(next);
        if let Some(next) = next {
            let gained = Event::Focus(FocusEvent {
                kind: FocusEventKind::Gained,
            });
            let _ = ctx.dispatch_to_target(Some(next), &gained);
        }
    }

    fn dispatch_mouse_event(
        &mut self,
        ctx: &mut dyn DispatchContext,
        kind: MouseEventKind,
        x: f64,
        y: f64,
        button: MouseButton,
    ) {
        self.refresh_mouse_target(ctx, x, y);
        let event = Event::Mouse(MouseEvent::new(kind, x, y, button));
        let _ = ctx.dispatch_to_target(ctx.mouse_target(), &event);
    }
}

impl EventDispatcher for BasicEventDispatcher {
    fn receive(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        self.refresh_mouse_target(ctx, x, y);
    }

    fn dispatch_mouse_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) {
        self.refresh_mouse_target(ctx, x, y);
        let target = ctx.mouse_target();
        let event = Event::Mouse(MouseEvent::new(MouseEventKind::Pressed, x, y, button));
        let handled = ctx.dispatch_to_target(target, &event);
        if handled {
            ctx.set_captured(target);
            if let Some(target) = target {
                ctx.set_pressed(target, true);
            }
            self.update_focus(ctx, target);
        }
    }

    fn dispatch_mouse_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) {
        self.refresh_mouse_target(ctx, x, y);
        let target = ctx.mouse_target();
        let event = Event::Mouse(MouseEvent::new(MouseEventKind::Released, x, y, button));
        let _ = ctx.dispatch_to_target(target, &event);
        if let Some(captured) = ctx.captured() {
            ctx.set_pressed(captured, false);
            ctx.set_captured(None);
            self.refresh_mouse_target(ctx, x, y);
        }
    }

    fn dispatch_mouse_moved(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        let kind = if ctx.captured().is_some() {
            MouseEventKind::Dragged
        } else {
            MouseEventKind::Moved
        };
        self.dispatch_mouse_event(ctx, kind, x, y, MouseButton::None);
    }

    fn dispatch_mouse_double_clicked(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        button: MouseButton,
    ) {
        self.dispatch_mouse_event(ctx, MouseEventKind::DoubleClicked, x, y, button);
    }

    fn dispatch_mouse_hover(&mut self, ctx: &mut dyn DispatchContext, x: f64, y: f64) {
        self.refresh_mouse_target(ctx, x, y);
        let event = Event::Mouse(MouseEvent::new(
            MouseEventKind::Hover,
            x,
            y,
            MouseButton::None,
        ));
        let _ = ctx.dispatch_to_target(ctx.hover_source(), &event);
    }

    fn dispatch_mouse_wheel(
        &mut self,
        ctx: &mut dyn DispatchContext,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
    ) {
        self.refresh_mouse_target(ctx, x, y);
        let event = Event::Wheel(WheelEvent::new(x, y, delta_x, delta_y));
        let _ = ctx.dispatch_to_target(ctx.mouse_target(), &event);
    }

    fn dispatch_key_pressed(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    ) {
        let event = Event::Key(KeyEvent {
            kind: KeyEventKind::Pressed,
            key,
            modifiers,
        });
        let _ = ctx.dispatch_to_target(ctx.focus_owner(), &event);
    }

    fn dispatch_key_released(
        &mut self,
        ctx: &mut dyn DispatchContext,
        key: Key,
        modifiers: KeyModifiers,
    ) {
        let event = Event::Key(KeyEvent {
            kind: KeyEventKind::Released,
            key,
            modifiers,
        });
        let _ = ctx.dispatch_to_target(ctx.focus_owner(), &event);
    }

    fn request_focus(&mut self, ctx: &mut dyn DispatchContext, target: Option<BlockId>) {
        self.update_focus(ctx, target);
    }

    fn release_focus(&mut self, ctx: &mut dyn DispatchContext) {
        self.update_focus(ctx, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FigureGraph, RectangleFigure};

    struct MockDispatchContext {
        hit_target: Option<BlockId>,
        mouse_target: Option<BlockId>,
        cursor_target: Option<BlockId>,
        hover_source: Option<BlockId>,
        focus_owner: Option<BlockId>,
        captured: Option<BlockId>,
        dispatched: Vec<(Option<BlockId>, Event)>,
        handled: bool,
        wants_key_events: bool,
    }

    impl MockDispatchContext {
        fn new(hit_target: Option<BlockId>) -> Self {
            Self {
                hit_target,
                mouse_target: None,
                cursor_target: None,
                hover_source: None,
                focus_owner: None,
                captured: None,
                dispatched: Vec::new(),
                handled: false,
                wants_key_events: false,
            }
        }
    }

    impl DispatchContext for MockDispatchContext {
        fn find_mouse_event_target_at(&self, _x: f64, _y: f64) -> Option<BlockId> {
            self.hit_target
        }

        fn mouse_target(&self) -> Option<BlockId> {
            self.mouse_target
        }

        fn set_mouse_target(&mut self, id: Option<BlockId>) {
            self.mouse_target = id;
        }

        fn cursor_target(&self) -> Option<BlockId> {
            self.cursor_target
        }

        fn set_cursor_target(&mut self, id: Option<BlockId>) {
            self.cursor_target = id;
        }

        fn hover_source(&self) -> Option<BlockId> {
            self.hover_source
        }

        fn set_hover_source(&mut self, id: Option<BlockId>) {
            self.hover_source = id;
        }

        fn set_hovered(&mut self, _id: BlockId, _hovered: bool) {}

        fn set_pressed(&mut self, _id: BlockId, _pressed: bool) {}

        fn focus_owner(&self) -> Option<BlockId> {
            self.focus_owner
        }

        fn set_focus_owner(&mut self, id: Option<BlockId>) {
            self.focus_owner = id;
        }

        fn captured(&self) -> Option<BlockId> {
            self.captured
        }

        fn set_captured(&mut self, id: Option<BlockId>) {
            self.captured = id;
        }

        fn wants_key_events(&self, _target_id: BlockId) -> bool {
            self.wants_key_events
        }

        fn dispatch_to_target(&mut self, target_id: Option<BlockId>, event: &Event) -> bool {
            self.dispatched.push((target_id, *event));
            self.handled
        }
    }

    #[test]
    fn test_receive_updates_mouse_target() {
        let mut dispatcher = BasicEventDispatcher;
        let mut ctx = MockDispatchContext::new(None);
        let mut scene = FigureGraph::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        ctx.hit_target = Some(target);

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
    fn test_captured_target_overrides_hit_target() {
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
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
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
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
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(None);
        ctx.mouse_target = Some(target);
        ctx.hover_source = Some(target);
        ctx.captured = Some(target);
        ctx.handled = true;

        dispatcher.dispatch_mouse_released(&mut ctx, 40.0, 40.0, MouseButton::Left);

        assert_eq!(ctx.captured(), None);
        assert_eq!(ctx.mouse_target(), None);
        assert_eq!(ctx.dispatched[1].0, Some(target));
        assert_eq!(
            ctx.dispatched[1].1,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::Released,
                40.0,
                40.0,
                MouseButton::Left,
            ))
        );
        assert_eq!(
            ctx.dispatched[0].1,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::Exited,
                40.0,
                40.0,
                MouseButton::None,
            ))
        );
    }

    #[test]
    fn test_drag_uses_capture_while_hover_tracks_hit_target() {
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
        let root = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let captured = scene.add_child_to(root, Box::new(RectangleFigure::new(1.0, 1.0, 4.0, 4.0)));
        let mut ctx = MockDispatchContext::new(Some(root));
        ctx.captured = Some(captured);

        dispatcher.dispatch_mouse_moved(&mut ctx, 8.0, 8.0);

        assert_eq!(ctx.hover_source(), Some(root));
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
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(Some(target));
        ctx.handled = true;
        ctx.wants_key_events = true;

        dispatcher.dispatch_mouse_pressed(&mut ctx, 4.0, 4.0, MouseButton::Left);
        dispatcher.dispatch_key_pressed(&mut ctx, Key::Character('a'), KeyModifiers::default());

        assert_eq!(ctx.focus_owner(), Some(target));
        assert!(ctx.dispatched.iter().any(|(_, event)| {
            *event
                == Event::Focus(FocusEvent {
                    kind: FocusEventKind::Gained,
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
    fn test_wheel_hover_and_double_click_use_pointer_target() {
        let mut dispatcher = BasicEventDispatcher;
        let mut scene = FigureGraph::new();
        let target = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)));
        let mut ctx = MockDispatchContext::new(Some(target));

        dispatcher.dispatch_mouse_hover(&mut ctx, 2.0, 3.0);
        dispatcher.dispatch_mouse_wheel(&mut ctx, 2.0, 3.0, 0.0, -1.0);
        dispatcher.dispatch_mouse_double_clicked(&mut ctx, 2.0, 3.0, MouseButton::Left);

        assert!(ctx.dispatched.iter().any(|(_, event)| {
            matches!(
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
}
