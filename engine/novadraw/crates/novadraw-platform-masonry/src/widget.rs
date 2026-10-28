use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, PaintCtx, PointerButton, PointerEvent,
    PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Size};
use masonry::layout::{LenReq, Length};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::Point;
use novadraw_editor::{EditPartFactory, ModelAdapter};

use crate::session::{CanvasAction, EditorSession};

const PREFERRED_WIDTH: f64 = 640.0;
const PREFERRED_HEIGHT: f64 = 480.0;

/// Owns the persistent editor controller and cached Scene.
pub struct EditorCanvas<A: ModelAdapter + 'static, F: EditPartFactory<A>> {
    pub(crate) session: EditorSession<A, F>,
    size: Size,
    captured: bool,
    last_error: Option<String>,
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> EditorCanvas<A, F> {
    pub fn new(session: EditorSession<A, F>) -> Self {
        Self {
            session,
            size: Size::ZERO,
            captured: false,
            last_error: None,
        }
    }

    pub fn session(&self) -> &EditorSession<A, F> {
        &self.session
    }

    pub fn error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn cancel(&mut self) {
        self.last_error = self.session.cancel().err();
        self.captured = false;
    }
}

fn mouse_button(button: Option<PointerButton>) -> Option<MouseButton> {
    match button {
        Some(PointerButton::Primary) | None => Some(MouseButton::Left),
        Some(PointerButton::Secondary) => Some(MouseButton::Right),
        Some(PointerButton::Auxiliary) => Some(MouseButton::Middle),
        _ => None,
    }
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A> + 'static> Widget for EditorCanvas<A, F> {
    type Action = CanvasAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        let result = match event {
            PointerEvent::Down(event) => {
                let Some(button) = mouse_button(event.button) else {
                    return;
                };
                let local = ctx.local_position(event.state.position);
                let modifiers = event.state.modifiers;
                let result = self.session.domain.pointer_pressed(
                    &mut self.session.viewer,
                    Point::new(local.x, local.y),
                    button,
                    KeyModifiers {
                        shift: modifiers.shift(),
                        control: modifiers.ctrl(),
                        alt: modifiers.alt(),
                        meta: modifiers.meta(),
                    },
                );
                result.map(|outcome| {
                    let dispatch = outcome.dispatch();
                    if self.session.domain.has_active_gesture() || dispatch.capture().is_some() {
                        self.captured = true;
                        ctx.capture_pointer();
                    }
                    if self.captured || dispatch.is_handled() || outcome.selection().is_some() {
                        ctx.set_handled();
                        ctx.request_focus();
                    }
                })
            }
            PointerEvent::Move(event) => {
                let local = ctx.local_position(event.current.position);
                self.session
                    .domain
                    .pointer_moved(&mut self.session.viewer, Point::new(local.x, local.y))
                    .map(|dispatch| {
                        if self.captured || dispatch.is_handled() {
                            ctx.set_handled();
                        }
                    })
            }
            PointerEvent::Up(event) => {
                let Some(button) = mouse_button(event.button) else {
                    return;
                };
                let local = ctx.local_position(event.state.position);
                let was_captured = self.captured;
                let result = self.session.domain.pointer_released(
                    &mut self.session.viewer,
                    Point::new(local.x, local.y),
                    button,
                );
                if was_captured {
                    ctx.release_pointer();
                }
                self.captured = false;
                result.map(|outcome| {
                    if was_captured || outcome.dispatch().is_handled() || outcome.command_executed()
                    {
                        ctx.set_handled();
                    }
                })
            }
            PointerEvent::Cancel(_) => {
                let captured = self.captured;
                self.cancel();
                if captured && ctx.is_active() {
                    ctx.release_pointer();
                }
                ctx.request_anim_frame();
                return;
            }
            _ => return,
        };
        self.last_error = result.err().map(|error| error.to_string());
        if self.last_error.is_some() {
            let error = self.last_error.clone();
            let captured = self.captured;
            self.cancel();
            self.last_error = error;
            if captured && ctx.is_active() {
                ctx.release_pointer();
            }
        }
        ctx.submit_action::<CanvasAction>(self.session.snapshot(self.last_error.clone()));
        ctx.request_anim_frame();
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        use masonry::core::keyboard::{Key, NamedKey};
        let cancel = matches!(event, TextEvent::WindowFocusChange(false))
            || matches!(event, TextEvent::Keyboard(event) if event.state.is_down()
                && event.key == Key::Named(NamedKey::Escape));
        if cancel {
            let captured = self.captured;
            self.cancel();
            if captured && ctx.is_active() {
                ctx.release_pointer();
            }
            ctx.set_handled();
            ctx.submit_action::<CanvasAction>(self.session.snapshot(self.last_error.clone()));
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(
            event,
            Update::DisabledChanged(true)
                | Update::StashedChanged(true)
                | Update::FocusChanged(false)
        ) {
            self.cancel();
            ctx.submit_action::<CanvasAction>(self.session.snapshot(self.last_error.clone()));
            ctx.request_anim_frame();
        }
    }

    fn register_children(&mut self, _: &mut RegisterCtx<'_>) {}
    fn measure(
        &mut self,
        _: &mut MeasureCtx<'_>,
        _: &PropertiesRef<'_>,
        axis: Axis,
        req: LenReq,
        _: Option<Length>,
    ) -> Length {
        match req {
            LenReq::FitContent(space) => space,
            _ => Length::px(match axis {
                Axis::Horizontal => PREFERRED_WIDTH,
                Axis::Vertical => PREFERRED_HEIGHT,
            }),
        }
    }
    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _: &PropertiesRef<'_>, size: Size) {
        if self.size != size {
            self.size = size;
            ctx.mutate_self_later(|mut widget| widget.ctx.request_anim_frame());
        }
        ctx.set_clip_path(size.to_rect());
    }
    fn on_anim_frame(&mut self, ctx: &mut UpdateCtx<'_>, _: &mut PropertiesMut<'_>, _: u64) {
        match self.session.render(self.size, ctx.scale_factor()) {
            Ok(retry) => {
                self.last_error = None;
                if retry {
                    ctx.request_anim_frame();
                }
            }
            Err(error) => {
                if self.last_error.as_ref() != Some(&error) {
                    self.last_error = Some(error);
                    ctx.submit_action::<CanvasAction>(
                        self.session.snapshot(self.last_error.clone()),
                    );
                }
            }
        }
        ctx.request_render();
    }
    fn paint(&mut self, _: &mut PaintCtx<'_>, _: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        painter.replay(self.session.scene());
    }
    fn accessibility_role(&self) -> Role {
        Role::Canvas
    }
    fn accessibility(&mut self, _: &mut AccessCtx<'_>, _: &PropertiesRef<'_>, node: &mut Node) {
        node.set_description("Diagram editor canvas");
    }
    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
    fn accepts_pointer_interaction(&self) -> bool {
        true
    }
    fn accepts_focus(&self) -> bool {
        true
    }
    fn accepts_text_input(&self) -> bool {
        false
    }
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> Drop for EditorCanvas<A, F> {
    fn drop(&mut self) {
        self.cancel();
    }
}
