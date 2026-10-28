//! Native drag sources carrying application-defined typed templates.
use masonry::{
    accesskit::{Node, Role},
    core::{
        AccessCtx, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, PaintCtx, PointerButton,
        PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx,
        Widget,
    },
    imaging::Painter,
    kurbo::{Axis, Rect, Size},
    layout::{LenReq, Length},
};
use novadraw::geometry::Point;
use std::marker::PhantomData;
use std::sync::Arc;
use xilem_masonry::{
    Pod, ViewCtx,
    core::{MessageCtx, MessageResult, Mut, View, ViewMarker},
};

const SOURCE_WIDTH: f64 = 80.0;
const SOURCE_HEIGHT: f64 = 36.0;
const DRAG_DISTANCE: f64 = 3.0;
type Preview = Arc<dyn Fn(&mut Painter<'_>, Size) + Send + Sync>;

/// Click activation or drop at a logical window point; the payload remains typed.
pub struct TemplateAction<T> {
    pub template: T,
    pub window_position: Point,
    pub is_drop: bool,
}
impl<T> std::fmt::Debug for TemplateAction<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateAction")
            .field("type", &std::any::type_name::<T>())
            .field("window_position", &self.window_position)
            .field("is_drop", &self.is_drop)
            .finish()
    }
}

/// A focusable drag source. Capture keeps release delivery when crossing into another widget.
pub struct TemplateSource<T> {
    template: T,
    name: &'static str,
    size: Size,
    start: Option<Point>,
    preview: Option<Preview>,
}
impl<T> TemplateSource<T> {
    pub fn new(template: T, name: &'static str) -> Self {
        Self {
            template,
            name,
            size: Size::ZERO,
            start: None,
            preview: None,
        }
    }
}
impl<T: Clone + Send + 'static> Widget for TemplateSource<T> {
    type Action = TemplateAction<T>;
    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(e) if e.button == Some(PointerButton::Primary) => {
                let local = ctx.local_position(e.state.position);
                let p = ctx.to_window(local);
                self.start = Some(Point::new(p.x, p.y));
                ctx.capture_pointer();
                ctx.request_focus();
                ctx.set_handled();
            }
            PointerEvent::Move(_) if self.start.is_some() => {
                ctx.set_handled();
            }
            PointerEvent::Up(e) if e.button == Some(PointerButton::Primary) => {
                if let Some(start) = self.start.take() {
                    let local = ctx.local_position(e.state.position);
                    let p = ctx.to_window(local);
                    let window_position = Point::new(p.x, p.y);
                    ctx.submit_action::<TemplateAction<T>>(TemplateAction {
                        template: self.template.clone(),
                        window_position,
                        is_drop: (window_position - start).length() >= DRAG_DISTANCE,
                    });
                    if ctx.is_pointer_capture_target() {
                        ctx.release_pointer();
                    }
                    ctx.set_handled();
                }
            }
            PointerEvent::Cancel(_) => {
                self.start = None;
            }
            _ => {}
        }
    }
    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        use masonry::core::keyboard::{Key, NamedKey};
        match event {
            TextEvent::Keyboard(e)
                if e.state.is_down() && e.key == Key::Named(NamedKey::Escape) =>
            {
                self.start = None;
                if ctx.is_pointer_capture_target() {
                    ctx.release_pointer();
                }
                ctx.set_handled();
            }
            TextEvent::WindowFocusChange(false) => {
                self.start = None;
                if ctx.is_pointer_capture_target() {
                    ctx.release_pointer();
                }
            }
            TextEvent::Keyboard(e)
                if e.state.is_up()
                    && (e.key == Key::Named(NamedKey::Enter)
                        || matches!(&e.key, Key::Character(c) if c == " ")) =>
            {
                let p = ctx.to_window(masonry::kurbo::Point::ZERO);
                ctx.submit_action::<TemplateAction<T>>(TemplateAction {
                    template: self.template.clone(),
                    window_position: Point::new(p.x, p.y),
                    is_drop: false,
                });
                ctx.set_handled();
            }
            _ => {}
        }
    }
    fn update(&mut self, _: &mut UpdateCtx<'_>, _: &mut PropertiesMut<'_>, e: &Update) {
        if matches!(
            e,
            Update::DisabledChanged(true)
                | Update::StashedChanged(true)
                | Update::FocusChanged(false)
        ) {
            self.start = None;
        }
    }
    fn register_children(&mut self, _: &mut RegisterCtx<'_>) {}
    fn measure(
        &mut self,
        _: &mut MeasureCtx<'_>,
        _: &PropertiesRef<'_>,
        axis: Axis,
        _: LenReq,
        _: Option<Length>,
    ) -> Length {
        Length::px(if axis == Axis::Horizontal {
            SOURCE_WIDTH
        } else {
            SOURCE_HEIGHT
        })
    }
    fn layout(&mut self, _: &mut LayoutCtx<'_>, _: &PropertiesRef<'_>, size: Size) {
        self.size = size;
    }
    fn paint(&mut self, _: &mut PaintCtx<'_>, _: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        painter
            .fill(
                Rect::new(0.0, 0.0, self.size.width, self.size.height),
                masonry::palette::css::LIGHT_STEEL_BLUE,
            )
            .draw();
        if let Some(preview) = &self.preview {
            preview(painter, self.size);
        }
        // Text is supplied by the adjoining ordinary Xilem label and the accessibility name.
    }
    fn accessibility_role(&self) -> Role {
        Role::Button
    }
    fn accessibility(&mut self, _: &mut AccessCtx<'_>, _: &PropertiesRef<'_>, node: &mut Node) {
        node.set_label(self.name);
        node.set_description("Click to activate; drag to drop a template onto the canvas");
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
    fn get_debug_text(&self) -> Option<String> {
        Some(self.name.into())
    }
}

pub struct TemplateView<State, T, Callback> {
    template: T,
    name: &'static str,
    callback: Callback,
    disabled: bool,
    preview: Option<Preview>,
    marker: PhantomData<fn(State)>,
}
pub fn template_source<State, T, Callback>(
    template: T,
    name: &'static str,
    callback: Callback,
) -> TemplateView<State, T, Callback> {
    TemplateView {
        template,
        name,
        callback,
        disabled: false,
        preview: None,
        marker: PhantomData,
    }
}
impl<State, T, C> TemplateView<State, T, C> {
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }

    /// Draws the application's template icon without interpreting its payload.
    pub fn preview(
        mut self,
        paint: impl Fn(&mut Painter<'_>, Size) + Send + Sync + 'static,
    ) -> Self {
        self.preview = Some(Arc::new(paint));
        self
    }
}
impl<State, T, C> ViewMarker for TemplateView<State, T, C> {}
impl<State, Action, T, C> View<State, Action, ViewCtx> for TemplateView<State, T, C>
where
    State: 'static,
    Action: 'static,
    T: Clone + Send + Sync + 'static,
    C: Fn(&mut State, TemplateAction<T>) -> Action + Send + Sync + 'static,
{
    type Element = Pod<TemplateSource<T>>;
    type ViewState = ();
    fn build(&self, ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, ()) {
        (
            ctx.with_action_widget(|ctx| {
                let mut widget = TemplateSource::new(self.template.clone(), self.name);
                widget.preview = self.preview.clone();
                let mut pod = ctx.create_pod(widget);
                pod.new_widget.options.disabled = self.disabled;
                pod
            }),
            (),
        )
    }
    fn rebuild(
        &self,
        _: &Self,
        _: &mut (),
        _: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut State,
    ) {
        element.ctx.set_disabled(self.disabled);
        element.widget.template = self.template.clone();
        element.widget.name = self.name;
        element.widget.preview = self.preview.clone();
        element.ctx.request_paint_only();
    }
    fn teardown(&self, _: &mut (), ctx: &mut ViewCtx, element: Mut<'_, Self::Element>) {
        element.widget.start = None;
        ctx.teardown_action_source(element);
    }
    fn message(
        &self,
        _: &mut (),
        ctx: &mut MessageCtx,
        _: Mut<'_, Self::Element>,
        state: &mut State,
    ) -> MessageResult<Action> {
        if ctx.take_first().is_some() {
            return MessageResult::Stale;
        }
        match ctx.take_message::<TemplateAction<T>>() {
            Some(action) => MessageResult::Action((self.callback)(state, *action)),
            None => MessageResult::Stale,
        }
    }
}
