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
use novadraw_editor::{EditPartFactory, EditPartId, EditorNamespace, ModelAdapter};
use std::{sync::Arc, time::Duration};

use crate::session::{CanvasAction, EditorSession};

const PREFERRED_WIDTH: f64 = 640.0;
const PREFERRED_HEIGHT: f64 = 480.0;
const SCROLL_LINE_PIXELS: f64 = 40.0;
pub(crate) type KeyHandler<A, F> = Arc<
    dyn Fn(
            &mut EditorSession<A, F>,
            &masonry::core::keyboard::KeyboardEvent,
        ) -> Result<bool, String>
        + Send
        + Sync,
>;

/// Owns the persistent editor controller and cached Scene.
pub struct EditorCanvas<A: ModelAdapter + 'static, F: EditPartFactory<A>> {
    pub(crate) session: EditorSession<A, F>,
    pub(crate) size: Size,
    captured: bool,
    pan: Option<(Point, Point)>,
    last_error: Option<String>,
    render_error: bool,
    last_action: Option<CanvasAction>,
    last_namespace: Option<EditorNamespace>,
    last_selection: Vec<EditPartId>,
    projection_changed: bool,
    pub(crate) key_handler: Option<KeyHandler<A, F>>,
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> EditorCanvas<A, F> {
    pub fn new(session: EditorSession<A, F>) -> Self {
        Self {
            session,
            size: Size::ZERO,
            captured: false,
            pan: None,
            last_error: None,
            render_error: false,
            last_action: None,
            last_namespace: None,
            last_selection: Vec::new(),
            projection_changed: true,
            key_handler: None,
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
        self.pan = None;
    }

    pub(crate) fn take_action(&mut self) -> Option<CanvasAction> {
        let action = self.session.snapshot(self.last_error.clone());
        let selection = self.session.viewer.selection().items();
        if self.last_action.as_ref() == Some(&action)
            && self.last_namespace == Some(self.session.viewer.namespace())
            && self.last_selection == selection
            && !self.projection_changed
        {
            return None;
        }
        self.last_selection = selection.to_vec();
        self.last_action = Some(action.clone());
        self.last_namespace = Some(self.session.viewer.namespace());
        self.projection_changed = false;
        Some(action)
    }

    pub(crate) fn set_error(&mut self, error: Option<String>) {
        self.last_error = error;
        self.render_error = false;
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
                if button == MouseButton::Middle {
                    if self.captured {
                        return;
                    }
                    match self.session.viewer.viewport_origin() {
                        Ok(origin) => {
                            self.pan = Some((Point::new(local.x, local.y), origin));
                            self.captured = true;
                            ctx.capture_pointer();
                            ctx.request_focus();
                            ctx.set_handled();
                        }
                        Err(error) => self.last_error = Some(error.to_string()),
                    }
                    return;
                }
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
                result
                    .map(|outcome| {
                        let dispatch = outcome.dispatch();
                        if self.session.domain.has_active_gesture() || dispatch.capture().is_some()
                        {
                            self.captured = true;
                            ctx.capture_pointer();
                        }
                        if self.captured || dispatch.is_handled() || outcome.selection().is_some() {
                            ctx.set_handled();
                            ctx.request_focus();
                        }
                    })
                    .map_err(|e| e.to_string())
            }
            PointerEvent::Move(event) => {
                let local = ctx.local_position(event.current.position);
                if let Some((start, origin)) = self.pan {
                    (|| -> Result<(), String> {
                        let zoom = self
                            .session
                            .viewer
                            .viewport_scale()
                            .map_err(|e| e.to_string())?;
                        self.session
                            .domain
                            .set_viewport_origin(
                                &mut self.session.viewer,
                                Point::new(
                                    origin.x() + (start.x() - local.x) / zoom,
                                    origin.y() + (start.y() - local.y) / zoom,
                                ),
                            )
                            .map_err(|e| e.to_string())?;
                        ctx.set_handled();
                        Ok(())
                    })()
                } else {
                    self.session
                        .domain
                        .pointer_moved(&mut self.session.viewer, Point::new(local.x, local.y))
                        .map(|dispatch| {
                            if self.captured || dispatch.is_handled() {
                                ctx.set_handled();
                            }
                        })
                        .map_err(|e| e.to_string())
                }
            }
            PointerEvent::Up(event) => {
                let Some(button) = mouse_button(event.button) else {
                    return;
                };
                let local = ctx.local_position(event.state.position);
                if button == MouseButton::Middle && self.pan.take().is_some() {
                    self.captured = false;
                    ctx.release_pointer();
                    ctx.set_handled();
                    return;
                }
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
                result
                    .map(|outcome| {
                        if was_captured
                            || outcome.dispatch().is_handled()
                            || outcome.command_executed()
                        {
                            ctx.set_handled();
                        }
                    })
                    .map_err(|e| e.to_string())
            }
            PointerEvent::Cancel(_) => {
                let captured = self.captured;
                self.cancel();
                if captured && ctx.is_pointer_capture_target() {
                    ctx.release_pointer();
                }
                if let Some(action) = self.take_action() {
                    ctx.submit_action::<CanvasAction>(action);
                }
                ctx.request_anim_frame();
                return;
            }
            PointerEvent::Scroll(event) => {
                let scale = ctx.scale_factor();
                let delta = event
                    .delta
                    .to_pixel_delta(
                        masonry::dpi::PhysicalPosition::new(
                            SCROLL_LINE_PIXELS * scale,
                            SCROLL_LINE_PIXELS * scale,
                        ),
                        masonry::dpi::PhysicalPosition::new(
                            self.size.width * scale,
                            self.size.height * scale,
                        ),
                    )
                    .to_logical::<f64>(scale);
                (|| -> Result<(), String> {
                    let origin = self
                        .session
                        .viewer
                        .viewport_origin()
                        .map_err(|e| e.to_string())?;
                    let zoom = self
                        .session
                        .viewer
                        .viewport_scale()
                        .map_err(|e| e.to_string())?;
                    if self
                        .session
                        .domain
                        .set_viewport_origin(
                            &mut self.session.viewer,
                            Point::new(origin.x() - delta.x / zoom, origin.y() - delta.y / zoom),
                        )
                        .map_err(|e| e.to_string())?
                    {
                        ctx.set_handled();
                    }
                    Ok(())
                })()
            }
            _ => return,
        };
        self.set_error(result.err());
        if self.last_error.is_some() {
            let error = self.last_error.clone();
            let captured = self.captured;
            self.cancel();
            self.last_error = error;
            if captured && ctx.is_pointer_capture_target() {
                ctx.release_pointer();
            }
        }
        if let Some(action) = self.take_action() {
            ctx.submit_action::<CanvasAction>(action);
        }
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
            if captured && ctx.is_pointer_capture_target() {
                ctx.release_pointer();
            }
            ctx.set_handled();
            if let Some(action) = self.take_action() {
                ctx.submit_action::<CanvasAction>(action);
            }
            ctx.request_anim_frame();
        } else if let TextEvent::Keyboard(event) = event {
            if ctx.is_disabled() {
                return;
            }
            let mut handled = false;
            if let Some(handler) = self.key_handler.clone() {
                match handler(&mut self.session, event) {
                    Ok(true) => {
                        self.set_error(None);
                        handled = true;
                        ctx.set_handled();
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.set_error(Some(error));
                        handled = true;
                        ctx.set_handled();
                    }
                }
            }
            if !handled {
                use novadraw::event::{Key as EditorKey, KeyEvent, KeyEventKind};
                let key = match &event.key {
                    Key::Character(text) if text.chars().count() == 1 => {
                        EditorKey::Character(text.chars().next().unwrap())
                    }
                    Key::Named(NamedKey::Enter) => EditorKey::Enter,
                    Key::Named(NamedKey::Tab) => EditorKey::Tab,
                    Key::Named(NamedKey::ArrowUp) => EditorKey::ArrowUp,
                    Key::Named(NamedKey::ArrowDown) => EditorKey::ArrowDown,
                    Key::Named(NamedKey::ArrowLeft) => EditorKey::ArrowLeft,
                    Key::Named(NamedKey::ArrowRight) => EditorKey::ArrowRight,
                    _ => return,
                };
                let modifiers = event.modifiers;
                let normalized = KeyEvent {
                    kind: if event.state.is_down() {
                        KeyEventKind::Pressed
                    } else {
                        KeyEventKind::Released
                    },
                    key,
                    modifiers: KeyModifiers {
                        shift: modifiers.shift(),
                        control: modifiers.ctrl(),
                        alt: modifiers.alt(),
                        meta: modifiers.meta(),
                    },
                };
                match self
                    .session
                    .domain
                    .key_event(&mut self.session.viewer, normalized)
                {
                    Ok(novadraw_editor::tool::ToolDirective::Continue) => {}
                    Ok(_) => {
                        self.captured = false;
                        if ctx.is_pointer_capture_target() {
                            ctx.release_pointer();
                        }
                        ctx.set_handled();
                    }
                    Err(error) => {
                        self.set_error(Some(error.to_string()));
                        ctx.set_handled();
                    }
                }
            }
            if let Some(action) = self.take_action() {
                ctx.submit_action::<CanvasAction>(action);
            }
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
            if let Some(action) = self.take_action() {
                ctx.submit_action::<CanvasAction>(action);
            }
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
            self.projection_changed = true;
            ctx.mutate_self_later(|mut widget| widget.ctx.request_anim_frame());
        }
        ctx.set_clip_path(size.to_rect());
    }
    fn on_anim_frame(&mut self, ctx: &mut UpdateCtx<'_>, _: &mut PropertiesMut<'_>, interval: u64) {
        if self.session.domain.autoexpose_requested() {
            match self
                .session
                .domain
                .autoexpose_tick(&mut self.session.viewer, Duration::from_nanos(interval))
            {
                Ok(tick) if tick.continue_requested() => ctx.request_anim_frame(),
                Ok(_) => {}
                Err(error) => self.last_error = Some(error.to_string()),
            }
        }
        match self.session.render(self.size, ctx.scale_factor()) {
            Ok(retry) => {
                if self.render_error {
                    self.last_error = None;
                    self.render_error = false;
                }
                if retry {
                    ctx.request_anim_frame();
                }
            }
            Err(error) => {
                self.render_error = true;
                if self.last_error.as_ref() != Some(&error) {
                    self.last_error = Some(error);
                }
            }
        }
        if let Some(action) = self.take_action() {
            ctx.submit_action::<CanvasAction>(action);
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
