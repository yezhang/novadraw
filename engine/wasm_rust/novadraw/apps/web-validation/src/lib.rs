use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;
use std::rc::Rc;

use novadraw::{
    BackendCapabilities, Bounded, Color, CursorIcon, DamageMode, Figure, FigureEventHandler, Key,
    KeyModifiers, MouseButton, NdCanvas, NovadrawContext, PlatformHost, Rectangle, RectangleFigure,
    RenderBackend, RenderCommandKind, RenderOutcome, RenderSubmission, Runtime, Shape, SurfaceInfo,
    Updatable,
    command::{LineCap, LineJoin},
};
use novadraw_apps::{WebInputAdapter, WebPlatformHost, WebPointerInput, WebWheelDeltaMode};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlButtonElement, HtmlCanvasElement, KeyboardEvent,
    PointerEvent, WheelEvent as DomWheelEvent, Window,
};

const LOGICAL_WIDTH: f64 = 800.0;
const LOGICAL_HEIGHT: f64 = 500.0;
const PROBE_BOUNDS: Rectangle = Rectangle {
    x: 250.0,
    y: 150.0,
    width: 300.0,
    height: 200.0,
};

#[derive(Default)]
struct ProbeState {
    hovered: Cell<bool>,
    pressed: Cell<bool>,
    focused: Cell<bool>,
    pointer_events: Cell<u32>,
    key_events: Cell<u32>,
    wheel_events: Cell<u32>,
    last_key: RefCell<String>,
}

struct WebProbeFigure {
    bounds: Rectangle,
    state: Rc<ProbeState>,
}

impl WebProbeFigure {
    fn new(bounds: Rectangle, state: Rc<ProbeState>) -> Self {
        Self { bounds, state }
    }

    fn record_pointer(&self, ctx: &mut dyn NovadrawContext) {
        self.state
            .pointer_events
            .set(self.state.pointer_events.get() + 1);
        ctx.repaint(None);
    }
}

impl Bounded for WebProbeFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "WebProbeFigure"
    }
}

impl Updatable for WebProbeFigure {
    fn validate(&mut self) {}
}

impl Shape for WebProbeFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(Color::hex("#17202a"))
    }

    fn stroke_width(&self) -> f64 {
        2.0
    }

    fn fill_color(&self) -> Option<Color> {
        let color = if self.state.pressed.get() {
            "#e74c3c"
        } else if self.state.focused.get() {
            "#8e44ad"
        } else if self.state.hovered.get() {
            "#16a085"
        } else {
            "#2980b9"
        };
        Some(Color::hex(color))
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, canvas: &mut NdCanvas) {
        if let Some(color) = self.fill_color() {
            canvas.fill_rect(0.0, 0.0, self.bounds.width, self.bounds.height, color);
        }
    }

    fn outline_shape(&self, canvas: &mut NdCanvas) {
        if let Some(color) = self.stroke_color() {
            canvas.stroke_rect(
                0.0,
                0.0,
                self.bounds.width,
                self.bounds.height,
                color,
                self.stroke_width(),
                self.line_cap(),
                self.line_join(),
            );
        }
    }
}

impl Figure for WebProbeFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure(&self, canvas: &mut NdCanvas) {
        Shape::paint_figure(self, canvas);
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for WebProbeFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn wants_key_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(
        &self,
        _event: &novadraw::MouseEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.state.pressed.set(true);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_released(
        &self,
        _event: &novadraw::MouseEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.state.pressed.set(false);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_moved(&self, _event: &novadraw::MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_dragged(
        &self,
        _event: &novadraw::MouseEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_entered(
        &self,
        _event: &novadraw::MouseEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.state.hovered.set(true);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_exited(
        &self,
        _event: &novadraw::MouseEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.state.hovered.set(false);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_wheel(&self, _event: &novadraw::WheelEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.state
            .wheel_events
            .set(self.state.wheel_events.get() + 1);
        ctx.repaint(None);
        true
    }

    fn on_key_pressed(&self, event: &novadraw::KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.state.key_events.set(self.state.key_events.get() + 1);
        *self.state.last_key.borrow_mut() = format!("{:?}", event.key);
        ctx.repaint(None);
        true
    }

    fn on_key_released(&self, event: &novadraw::KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.on_key_pressed(event, ctx)
    }

    fn on_focus_gained(
        &self,
        _event: &novadraw::FocusEvent,
        ctx: &mut dyn NovadrawContext,
    ) -> bool {
        self.state.focused.set(true);
        ctx.repaint(None);
        true
    }

    fn on_focus_lost(&self, _event: &novadraw::FocusEvent, ctx: &mut dyn NovadrawContext) -> bool {
        self.state.focused.set(false);
        ctx.repaint(None);
        true
    }
}

struct Canvas2dBackend {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    scale_factor: f64,
}

impl Canvas2dBackend {
    fn new(canvas: HtmlCanvasElement, context: CanvasRenderingContext2d) -> Self {
        Self {
            canvas,
            context,
            scale_factor: 1.0,
        }
    }

    fn color(color: Color) -> String {
        format!(
            "rgba({},{},{},{})",
            (color.r * 255.0).round() as u8,
            (color.g * 255.0).round() as u8,
            (color.b * 255.0).round() as u8,
            color.a
        )
    }

    fn set_fill(&self, color: Color) {
        self.context.set_fill_style_str(Self::color(color).as_str());
    }

    fn set_stroke(&self, color: Color, width: f64) {
        self.context
            .set_stroke_style_str(Self::color(color).as_str());
        self.context.set_line_width(width);
    }

    fn reset_transform(&self) {
        let scale = self.scale_factor;
        let _ = self.context.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0);
    }
}

impl RenderBackend for Canvas2dBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderOutcome {
        self.resize(
            submission.surface.pixel_width,
            submission.surface.pixel_height,
            submission.surface.scale_factor,
        );
        self.reset_transform();
        self.context.clear_rect(
            0.0,
            0.0,
            submission.surface.logical_width,
            submission.surface.logical_height,
        );
        self.set_fill(Color::hex("#eef1f4"));
        self.context.fill_rect(
            0.0,
            0.0,
            submission.surface.logical_width,
            submission.surface.logical_height,
        );

        for command in &submission.commands {
            match &command.kind {
                RenderCommandKind::PushState => self.context.save(),
                RenderCommandKind::RestoreState => {
                    self.context.restore();
                    self.context.save();
                }
                RenderCommandKind::PopState => self.context.restore(),
                RenderCommandKind::ConcatTransform { matrix } => {
                    let [a, b, c, d, e, f] = matrix.coeffs();
                    let _ = self.context.transform(a, b, c, d, e, f);
                }
                RenderCommandKind::SetTransform { matrix } => {
                    let [a, b, c, d, e, f] = matrix.coeffs();
                    let scale = self.scale_factor;
                    let _ = self.context.set_transform(
                        a * scale,
                        b * scale,
                        c * scale,
                        d * scale,
                        e * scale,
                        f * scale,
                    );
                }
                RenderCommandKind::ResetTransform => self.reset_transform(),
                RenderCommandKind::Clip { rect } => {
                    self.context.begin_path();
                    self.context.rect(
                        rect[0].x,
                        rect[0].y,
                        rect[1].x - rect[0].x,
                        rect[1].y - rect[0].y,
                    );
                    self.context.clip();
                }
                RenderCommandKind::SetGlobalAlpha { alpha } => {
                    self.context.set_global_alpha(*alpha);
                }
                RenderCommandKind::ClearRect { rect, color }
                | RenderCommandKind::FillRect { rect, color } => {
                    self.set_fill(*color);
                    self.context.fill_rect(
                        rect[0].x,
                        rect[0].y,
                        rect[1].x - rect[0].x,
                        rect[1].y - rect[0].y,
                    );
                }
                RenderCommandKind::StrokeRect {
                    rect, color, width, ..
                } => {
                    self.set_stroke(*color, *width);
                    self.context.stroke_rect(
                        rect[0].x,
                        rect[0].y,
                        rect[1].x - rect[0].x,
                        rect[1].y - rect[0].y,
                    );
                }
                RenderCommandKind::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    fill_color,
                    stroke_color,
                    stroke_width,
                    ..
                } => {
                    self.context.begin_path();
                    let _ = self.context.ellipse(*cx, *cy, *rx, *ry, 0.0, 0.0, TAU);
                    if let Some(color) = fill_color {
                        self.set_fill(*color);
                        self.context.fill();
                    }
                    if let Some(color) = stroke_color {
                        self.set_stroke(*color, *stroke_width);
                        self.context.stroke();
                    }
                }
                RenderCommandKind::Clear { color } => {
                    self.set_fill(*color);
                    self.context.fill_rect(
                        0.0,
                        0.0,
                        submission.surface.logical_width,
                        submission.surface.logical_height,
                    );
                }
                _ => {}
            }
        }
        RenderOutcome::Presented
    }

    fn resize(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f64) {
        self.scale_factor = scale_factor;
        if self.canvas.width() != pixel_width {
            self.canvas.set_width(pixel_width);
        }
        if self.canvas.height() != pixel_height {
            self.canvas.set_height(pixel_height);
        }
    }
}

struct WebValidationApp {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    runtime: Runtime,
    host: WebPlatformHost,
    backend: Canvas2dBackend,
    input: WebInputAdapter,
    probe: Rc<ProbeState>,
    redraw_pending: Rc<Cell<bool>>,
    frame_count: u64,
    scale_override: Option<f64>,
}

impl WebValidationApp {
    fn new(
        window: Window,
        document: Document,
        canvas: HtmlCanvasElement,
        context: CanvasRenderingContext2d,
    ) -> Self {
        let probe = Rc::new(ProbeState::default());
        let mut graph = novadraw::FigureGraph::new();
        let root = graph.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            LOGICAL_WIDTH,
            LOGICAL_HEIGHT,
            Color::hex("#eef1f4"),
        )));
        graph.add_child_to(
            root,
            Box::new(WebProbeFigure::new(PROBE_BOUNDS, probe.clone())),
        );

        let redraw_pending = Rc::new(Cell::new(false));
        let host = WebPlatformHost::new(
            SurfaceInfo::default(),
            {
                let pending = redraw_pending.clone();
                move || pending.set(true)
            },
            {
                let canvas = canvas.clone();
                move |cursor| {
                    let value = match cursor {
                        CursorIcon::Pointer => "pointer",
                        CursorIcon::Crosshair => "crosshair",
                        CursorIcon::Text => "text",
                        CursorIcon::Move => "move",
                        CursorIcon::NotAllowed => "not-allowed",
                        CursorIcon::EastWestResize => "ew-resize",
                        CursorIcon::NorthSouthResize => "ns-resize",
                        CursorIcon::NorthEastSouthWestResize => "nesw-resize",
                        CursorIcon::NorthWestSouthEastResize => "nwse-resize",
                        CursorIcon::Default => "default",
                    };
                    let _ = canvas.style().set_property("cursor", value);
                }
            },
            |_| {},
            |_| {},
        );

        Self {
            window,
            document,
            canvas: canvas.clone(),
            runtime: Runtime::new(graph),
            host,
            backend: Canvas2dBackend::new(canvas, context),
            input: WebInputAdapter,
            probe,
            redraw_pending,
            frame_count: 0,
            scale_override: None,
        }
    }

    fn sync_surface(&mut self) {
        let rect = self.canvas.get_bounding_client_rect();
        let logical_width = rect.width().max(1.0);
        let logical_height = rect.height().max(1.0);
        let scale_factor = self
            .scale_override
            .unwrap_or_else(|| self.window.device_pixel_ratio().max(1.0));
        self.host.set_surface_info(SurfaceInfo {
            logical_width,
            logical_height,
            pixel_width: (logical_width * scale_factor).round() as u32,
            pixel_height: (logical_height * scale_factor).round() as u32,
            scale_factor,
        });
        self.runtime.request_full_redraw();
    }

    fn pointer_input(&self, event: &PointerEvent) -> WebPointerInput {
        let rect = self.canvas.get_bounding_client_rect();
        WebPointerInput {
            pointer_id: event.pointer_id().max(0) as u64,
            client_x: f64::from(event.client_x()),
            client_y: f64::from(event.client_y()),
            canvas_left: rect.left(),
            canvas_top: rect.top(),
        }
    }

    fn request_and_render(&mut self) {
        self.host.request_redraw();
        if !self.redraw_pending.replace(false) {
            return;
        }
        let Some(submission) = self
            .runtime
            .prepare_submission(self.host.surface_info(), self.backend.capabilities())
        else {
            self.update_status(DamageMode::None);
            return;
        };
        let frame_id = submission.frame_id;
        let damage = submission.damage.mode();
        let outcome = self.backend.submit(&submission);
        self.runtime.complete_submission(frame_id, outcome);
        self.frame_count += 1;
        self.update_status(damage);
    }

    fn update_status(&self, damage: DamageMode) {
        let status = format!(
            "READY · frame {} · {:?} · pointer {} · wheel {} · key {}",
            self.frame_count,
            damage,
            self.probe.pointer_events.get(),
            self.probe.wheel_events.get(),
            self.probe.key_events.get()
        );
        set_text(&self.document, "runtime-status", &status);
        set_text(
            &self.document,
            "pointer-status",
            if self.probe.focused.get() {
                "Focused"
            } else if self.probe.hovered.get() {
                "Hovered"
            } else {
                "Idle"
            },
        );
        let last_key = self.probe.last_key.borrow();
        set_text(
            &self.document,
            "keyboard-status",
            if last_key.is_empty() {
                "No key received"
            } else {
                last_key.as_str()
            },
        );
        let surface = self.host.surface_info();
        set_text(
            &self.document,
            "surface-status",
            &format!(
                "{:.0}×{:.0} logical · {}×{} px · {:.1}x DPR",
                surface.logical_width,
                surface.logical_height,
                surface.pixel_width,
                surface.pixel_height,
                surface.scale_factor
            ),
        );
        if let Some(body) = self.document.body() {
            let _ = body.set_attribute("data-ready", "true");
            let _ = body.set_attribute("data-frame-count", &self.frame_count.to_string());
            let _ = body.set_attribute(
                "data-pointer-events",
                &self.probe.pointer_events.get().to_string(),
            );
            let _ = body.set_attribute("data-key-events", &self.probe.key_events.get().to_string());
            let _ = body.set_attribute(
                "data-wheel-events",
                &self.probe.wheel_events.get().to_string(),
            );
        }
    }

    fn on_pointer_move(&mut self, event: PointerEvent) {
        if let Some(point) = self.pointer_input(&event).logical_position() {
            self.runtime.dispatch_mouse_moved(point.x(), point.y());
            self.request_and_render();
        }
    }

    fn on_pointer_down(&mut self, event: PointerEvent) {
        let _ = self.canvas.focus();
        let _ = self.canvas.set_pointer_capture(event.pointer_id());
        if let Some(point) = self.pointer_input(&event).logical_position() {
            self.runtime.dispatch_mouse_moved(point.x(), point.y());
            self.runtime
                .dispatch_mouse_pressed(point.x(), point.y(), MouseButton::Left);
            self.request_and_render();
        }
    }

    fn on_pointer_up(&mut self, event: PointerEvent) {
        if let Some(point) = self.pointer_input(&event).logical_position() {
            self.runtime
                .dispatch_mouse_released(point.x(), point.y(), MouseButton::Left);
        }
        let _ = self.canvas.release_pointer_capture(event.pointer_id());
        self.request_and_render();
    }

    fn on_wheel(&mut self, event: DomWheelEvent) {
        event.prevent_default();
        let pointer = WebPointerInput {
            pointer_id: 0,
            client_x: f64::from(event.client_x()),
            client_y: f64::from(event.client_y()),
            canvas_left: self.canvas.get_bounding_client_rect().left(),
            canvas_top: self.canvas.get_bounding_client_rect().top(),
        };
        let mode = match event.delta_mode() {
            1 => WebWheelDeltaMode::Line,
            2 => WebWheelDeltaMode::Page,
            _ => WebWheelDeltaMode::Pixel,
        };
        if let Some(wheel) = self.input.adapt_wheel(
            pointer,
            event.delta_x(),
            event.delta_y(),
            mode,
            self.host.surface_info().logical_width,
            self.host.surface_info().logical_height,
            modifiers(
                event.ctrl_key(),
                event.shift_key(),
                event.alt_key(),
                event.meta_key(),
            ),
        ) {
            self.runtime.dispatch_scroll(wheel);
            self.request_and_render();
        }
    }

    fn on_key(&mut self, event: KeyboardEvent, pressed: bool) {
        let Some(key) = map_key(&event.key()) else {
            return;
        };
        let modifiers = modifiers(
            event.ctrl_key(),
            event.shift_key(),
            event.alt_key(),
            event.meta_key(),
        );
        if pressed {
            self.runtime.dispatch_key_pressed(key, modifiers);
        } else {
            self.runtime.dispatch_key_released(key, modifiers);
        }
        self.request_and_render();
    }

    fn toggle_scale(&mut self) {
        self.scale_override = Some(if self.host.surface_info().scale_factor < 1.5 {
            2.0
        } else {
            1.0
        });
        self.sync_surface();
        self.request_and_render();
    }
}

fn modifiers(control: bool, shift: bool, alt: bool, meta: bool) -> KeyModifiers {
    KeyModifiers {
        control,
        shift,
        alt,
        meta,
    }
}

fn map_key(value: &str) -> Option<Key> {
    match value {
        "Enter" => Some(Key::Enter),
        "Escape" => Some(Key::Escape),
        "Tab" => Some(Key::Tab),
        "ArrowUp" => Some(Key::ArrowUp),
        "ArrowDown" => Some(Key::ArrowDown),
        "ArrowLeft" => Some(Key::ArrowLeft),
        "ArrowRight" => Some(Key::ArrowRight),
        value if value.chars().count() == 1 => value
            .chars()
            .next()
            .map(|key| Key::Character(key.to_ascii_lowercase())),
        _ => None,
    }
}

fn set_text(document: &Document, id: &str, value: &str) {
    if let Some(element) = document.get_element_by_id(id) {
        element.set_text_content(Some(value));
    }
}

fn register_event(
    target: &web_sys::EventTarget,
    name: &str,
    app: &Rc<RefCell<WebValidationApp>>,
    mut handler: impl FnMut(&mut WebValidationApp, Event) + 'static,
) -> Result<(), JsValue> {
    let app = app.clone();
    let closure = Closure::<dyn FnMut(Event)>::new(move |event| {
        handler(&mut app.borrow_mut(), event);
    });
    target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

fn element<T: JsCast>(document: &Document, id: &str) -> Result<T, JsValue> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("missing element #{id}")))?
        .dyn_into::<T>()
        .map_err(|_| JsValue::from_str(&format!("invalid element type for #{id}")))
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("missing document"))?;
    let canvas: HtmlCanvasElement = element(&document, "novadraw-canvas")?;
    let context = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("Canvas2D unavailable"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let app = Rc::new(RefCell::new(WebValidationApp::new(
        window.clone(),
        document.clone(),
        canvas.clone(),
        context,
    )));
    let target: &web_sys::EventTarget = canvas.as_ref();
    register_event(target, "pointermove", &app, |app, event| {
        app.on_pointer_move(event.unchecked_into::<PointerEvent>())
    })?;
    register_event(target, "pointerdown", &app, |app, event| {
        app.on_pointer_down(event.unchecked_into::<PointerEvent>())
    })?;
    register_event(target, "pointerup", &app, |app, event| {
        app.on_pointer_up(event.unchecked_into::<PointerEvent>())
    })?;
    register_event(target, "wheel", &app, |app, event| {
        app.on_wheel(event.unchecked_into::<DomWheelEvent>())
    })?;
    register_event(target, "keydown", &app, |app, event| {
        app.on_key(event.unchecked_into::<KeyboardEvent>(), true)
    })?;
    register_event(target, "keyup", &app, |app, event| {
        app.on_key(event.unchecked_into::<KeyboardEvent>(), false)
    })?;
    register_event(target, "blur", &app, |app, _event| {
        app.runtime.release_focus();
        app.request_and_render();
    })?;

    let scale_button: HtmlButtonElement = element(&document, "toggle-scale")?;
    register_event(scale_button.as_ref(), "click", &app, |app, _event| {
        app.toggle_scale()
    })?;
    register_event(window.as_ref(), "resize", &app, |app, _event| {
        app.sync_surface();
        app.request_and_render();
    })?;

    {
        let mut app = app.borrow_mut();
        app.host.set_cursor(CursorIcon::Crosshair);
        app.sync_surface();
        app.request_and_render();
    }

    // Event closures own an Rc; retaining this reference documents the app lifetime.
    std::mem::forget(app);
    Ok(())
}
