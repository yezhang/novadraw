#![cfg(target_arch = "wasm32")]

mod direct_edit_mode;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::rc::Rc;

use novadraw::event::{
    AccessibilityNode, AccessibilityNodeId, AccessibilityRole, AccessibilityUpdate, EventContext,
    FigureEventHandler, FocusTraversalOutcome, Key, KeyModifiers, MonotonicTime, TooltipUpdate,
    place_tooltip,
};
use novadraw::figure::{Bounded, CursorIcon, Shape};
use novadraw::graphics::{LineCap, LineJoin};
use novadraw::render::command::RenderCommandKind;
use novadraw::render::submission::{BackendSessionDecision, BackendSessionGate};
use novadraw::render::{
    BackendCapabilities, DamageMode, RenderCapability, RenderOutcome, RenderSubmission,
    SurfaceInfo, UnsupportedRenderCapability,
};
use novadraw::{Color, Figure, NdCanvas, PlatformHost, Rectangle, RenderBackend, Runtime};
use novadraw_backend_vello::VelloRenderer;
use novadraw_example_scenes::{
    DemoSuite, SceneSpec, ValidationKind, catalog,
    focus::{FOCUS_TRAVERSAL_SCENE_TITLE, FocusProbeSpec, build_focus_traversal_scene},
};
use novadraw_platform_web::{
    AdaptedGesture, AdaptedKeyInput, WebInputAdapter, WebPlatformHost, WebPointerInput,
    WebWheelDeltaMode, adapt_key_input, adapt_pointer_button,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
#[cfg(target_arch = "wasm32")]
use web_sys::UrlSearchParams;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, Event, HtmlButtonElement, HtmlCanvasElement,
    HtmlElement, KeyboardEvent, PointerEvent, WheelEvent as DomWheelEvent, Window,
};

const INPUT_THEME_ID: &str = "input";
const INPUT_THEME_TITLE: &str = "Input";
const INPUT_SCENE_WIDTH: u32 = 800;
const INPUT_SCENE_HEIGHT: u32 = 500;

#[derive(Default)]
struct ProbeState {
    hovered_label: RefCell<Option<&'static str>>,
    focused_label: RefCell<Option<&'static str>>,
    pointer_events: Cell<u32>,
    key_events: Cell<u32>,
    wheel_events: Cell<u32>,
    last_key: RefCell<String>,
}

#[derive(Default)]
struct WebAccessibilityState {
    nodes: HashMap<AccessibilityNodeId, AccessibilityNode>,
    root: Option<AccessibilityNodeId>,
    revision: u64,
}

struct WebProbeFigure {
    bounds: Rectangle,
    label: &'static str,
    disabled_visual: bool,
    hovered: Cell<bool>,
    pressed: Cell<bool>,
    focused: Cell<bool>,
    state: Rc<ProbeState>,
}

impl WebProbeFigure {
    fn from_focus_spec(spec: FocusProbeSpec, state: Rc<ProbeState>) -> Self {
        Self {
            bounds: spec.bounds,
            label: spec.label,
            disabled_visual: !spec.enabled,
            hovered: Cell::new(false),
            pressed: Cell::new(false),
            focused: Cell::new(false),
            state,
        }
    }

    fn record_pointer(&self, ctx: &mut EventContext<'_>) {
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

impl Shape for WebProbeFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(Color::from_hex("#17202a").expect("valid color literal"))
    }

    fn stroke_width(&self) -> f64 {
        2.0
    }

    fn fill_color(&self) -> Option<Color> {
        let color = if self.disabled_visual {
            "#95a5a6"
        } else if self.pressed.get() {
            "#e74c3c"
        } else if self.focused.get() {
            "#8e44ad"
        } else if self.hovered.get() {
            "#16a085"
        } else {
            "#2980b9"
        };
        Some(Color::from_hex(color).expect("valid color literal"))
    }

    fn line_cap(&self) -> LineCap {
        LineCap::default()
    }

    fn line_join(&self) -> LineJoin {
        LineJoin::default()
    }

    fn fill_shape(&self, canvas: &mut NdCanvas) {
        if let Some(color) = self.fill_color() {
            canvas.fill_rect_with_color(0.0, 0.0, self.bounds.width, self.bounds.height, color);
        }
    }

    fn outline_shape(&self, canvas: &mut NdCanvas) {
        if let Some(color) = self.stroke_color() {
            canvas.stroke_rect_with_style(
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

    fn on_mouse_pressed(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.pressed.set(true);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_released(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.pressed.set(false);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_moved(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_dragged(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_entered(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.hovered.set(true);
        *self.state.hovered_label.borrow_mut() = Some(self.label);
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_exited(
        &self,
        _event: &novadraw::event::MouseEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.hovered.set(false);
        let is_current_hover = *self.state.hovered_label.borrow() == Some(self.label);
        if is_current_hover {
            *self.state.hovered_label.borrow_mut() = None;
        }
        self.record_pointer(ctx);
        true
    }

    fn on_mouse_wheel(
        &self,
        _event: &novadraw::event::WheelEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.state
            .wheel_events
            .set(self.state.wheel_events.get() + 1);
        ctx.repaint(None);
        true
    }

    fn on_key_pressed(
        &self,
        event: &novadraw::event::KeyEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.state.key_events.set(self.state.key_events.get() + 1);
        *self.state.last_key.borrow_mut() = format!("{:?}", event.key);
        ctx.repaint(None);
        true
    }

    fn on_key_released(
        &self,
        event: &novadraw::event::KeyEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.on_key_pressed(event, ctx)
    }

    fn on_focus_gained(
        &self,
        _event: &novadraw::event::FocusEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.focused.set(true);
        *self.state.focused_label.borrow_mut() = Some(self.label);
        ctx.repaint(None);
        true
    }

    fn on_focus_lost(
        &self,
        _event: &novadraw::event::FocusEvent,
        ctx: &mut EventContext<'_>,
    ) -> bool {
        self.focused.set(false);
        let is_current_focus = *self.state.focused_label.borrow() == Some(self.label);
        if is_current_focus {
            *self.state.focused_label.borrow_mut() = None;
        }
        ctx.repaint(None);
        true
    }
}

struct Canvas2dBackend {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    scale_factor: f64,
    session_gate: BackendSessionGate,
}

impl Canvas2dBackend {
    fn new(canvas: HtmlCanvasElement, context: CanvasRenderingContext2d) -> Self {
        Self {
            canvas,
            context,
            scale_factor: 1.0,
            session_gate: BackendSessionGate::default(),
        }
    }

    fn color(color: Color) -> String {
        format!(
            "rgba({},{},{},{})",
            (color.red() * 255.0).round() as u8,
            (color.green() * 255.0).round() as u8,
            (color.blue() * 255.0).round() as u8,
            color.alpha()
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
        match self
            .session_gate
            .accept(submission.session_id, &submission.resources)
        {
            BackendSessionDecision::RejectStale | BackendSessionDecision::RejectMissingSnapshot => {
                return RenderOutcome::Skipped;
            }
            BackendSessionDecision::Initialize
            | BackendSessionDecision::Continue
            | BackendSessionDecision::Replace => {}
        }
        if submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
        {
            return RenderOutcome::Unsupported(UnsupportedRenderCapability {
                capability: RenderCapability::GlyphRuns,
            });
        }
        if submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
        {
            return RenderOutcome::Unsupported(UnsupportedRenderCapability {
                capability: RenderCapability::ImageResources,
            });
        }
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
        self.set_fill(Color::from_hex("#eef1f4").expect("valid color literal"));
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
                    self.context.rect(rect.x, rect.y, rect.width, rect.height);
                    self.context.clip();
                }
                RenderCommandKind::SetGlobalAlpha { alpha } => {
                    self.context.set_global_alpha(*alpha);
                }
                RenderCommandKind::ClearRect { rect, color }
                | RenderCommandKind::FillRect { rect, color } => {
                    self.set_fill(*color);
                    self.context
                        .fill_rect(rect.x, rect.y, rect.width, rect.height);
                }
                RenderCommandKind::StrokeRect {
                    rect, color, width, ..
                } => {
                    self.set_stroke(*color, *width);
                    self.context
                        .stroke_rect(rect.x, rect.y, rect.width, rect.height);
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

enum ValidationBackend {
    Vello(Box<VelloRenderer>),
    Canvas2d(Canvas2dBackend),
}

impl ValidationBackend {
    fn name(&self) -> &'static str {
        match self {
            Self::Vello(_) => "Vello WebGPU",
            Self::Canvas2d(_) => "Canvas2D",
        }
    }

    fn data_name(&self) -> &'static str {
        match self {
            Self::Vello(_) => "vello-webgpu",
            Self::Canvas2d(_) => "canvas2d",
        }
    }
}

impl RenderBackend for ValidationBackend {
    fn capabilities(&self) -> BackendCapabilities {
        match self {
            Self::Vello(backend) => backend.capabilities(),
            Self::Canvas2d(backend) => backend.capabilities(),
        }
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderOutcome {
        match self {
            Self::Vello(backend) => backend.submit(submission),
            Self::Canvas2d(backend) => backend.submit(submission),
        }
    }

    fn resize(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f64) {
        match self {
            Self::Vello(backend) => backend.resize(pixel_width, pixel_height, scale_factor),
            Self::Canvas2d(backend) => backend.resize(pixel_width, pixel_height, scale_factor),
        }
    }
}

fn input_theme(probe: Rc<ProbeState>) -> DemoSuite {
    let scene_probe = probe.clone();
    let scenes = vec![SceneSpec::new(
        "focus-traversal",
        FOCUS_TRAVERSAL_SCENE_TITLE,
        (INPUT_SCENE_WIDTH, INPUT_SCENE_HEIGHT),
        ValidationKind::Interactive,
        move || {
            build_focus_traversal_scene(|spec| {
                Box::new(WebProbeFigure::from_focus_spec(spec, scene_probe.clone()))
            })
            .0
        },
    )];

    DemoSuite::new(INPUT_THEME_ID, INPUT_THEME_TITLE, scenes)
}

fn theme_selection(window: &Window, themes: &[DemoSuite]) -> (usize, usize) {
    let parameters = window
        .location()
        .search()
        .ok()
        .and_then(|search| UrlSearchParams::new_with_str(&search).ok());
    let theme_id = parameters.as_ref().and_then(|query| query.get("theme"));
    let theme_index = theme_id
        .as_deref()
        .and_then(|id| themes.iter().position(|theme| theme.id == id))
        .unwrap_or_default();
    let scene_index = parameters
        .and_then(|query| query.get("scene"))
        .and_then(|value| {
            value.parse::<usize>().ok().or_else(|| {
                themes[theme_index]
                    .scenes
                    .iter()
                    .position(|scene| scene.id == value)
            })
        })
        .filter(|index| *index < themes[theme_index].scenes.len())
        .unwrap_or_default();
    (theme_index, scene_index)
}

fn create_scene(themes: &mut [DemoSuite], theme_index: usize, scene_index: usize) -> Runtime {
    themes[theme_index].scenes[scene_index].build()
}

struct WebValidationApp {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    runtime: Runtime,
    host: WebPlatformHost,
    backend: ValidationBackend,
    input: WebInputAdapter,
    probe: Rc<ProbeState>,
    themes: Vec<DemoSuite>,
    current_theme: usize,
    current_scene: usize,
    redraw_pending: Rc<Cell<bool>>,
    frame_count: u64,
    scale_override: Option<f64>,
    clock_origin_millis: f64,
}

impl WebValidationApp {
    fn new(
        window: Window,
        document: Document,
        canvas: HtmlCanvasElement,
        backend: ValidationBackend,
    ) -> Rc<RefCell<Self>> {
        let probe = Rc::new(ProbeState::default());
        let mut themes = vec![input_theme(probe.clone())];
        themes.extend(catalog());
        let (current_theme, current_scene) = theme_selection(&window, &themes);
        let (width, height) = themes[current_theme].scenes[current_scene].logical_size;
        let _ = canvas
            .style()
            .set_property("aspect-ratio", &format!("{width} / {height}"));
        let runtime = create_scene(&mut themes, current_theme, current_scene);

        let redraw_pending = Rc::new(Cell::new(false));
        let tooltip: HtmlElement =
            element(&document, "novadraw-tooltip").expect("tooltip element must exist");
        let accessibility_tree: HtmlElement = element(&document, "novadraw-accessibility-tree")
            .expect("accessibility tree element must exist");
        let accessibility_state = Rc::new(RefCell::new(WebAccessibilityState::default()));
        let wake_generation = Rc::new(Cell::new(0_u64));
        let clock_origin_millis = browser_now(&window);

        Rc::new_cyclic(|weak: &std::rc::Weak<RefCell<Self>>| {
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
                {
                    let window = window.clone();
                    let weak = weak.clone();
                    let generation = wake_generation.clone();
                    move |deadline| {
                        let next_generation = generation.get().wrapping_add(1);
                        generation.set(next_generation);
                        let Some(deadline) = deadline else {
                            return;
                        };
                        let now = web_monotonic_time(&window, clock_origin_millis);
                        let delay_micros = deadline.as_micros().saturating_sub(now.as_micros());
                        let delay_millis = delay_micros.div_ceil(1_000).min(i32::MAX as u64) as i32;
                        let weak = weak.clone();
                        let generation = generation.clone();
                        let callback = Closure::once_into_js(move || {
                            if generation.get() == next_generation
                                && let Some(app) = weak.upgrade()
                            {
                                app.borrow_mut().on_runtime_wake();
                            }
                        });
                        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                            callback.unchecked_ref(),
                            delay_millis,
                        );
                    }
                },
                {
                    let canvas = canvas.clone();
                    let tooltip = tooltip.clone();
                    move |update| update_web_tooltip(&canvas, &tooltip, update)
                },
                {
                    let document = document.clone();
                    let tree = accessibility_tree.clone();
                    let state = accessibility_state.clone();
                    move |update| {
                        update_web_accessibility(&document, &tree, &mut state.borrow_mut(), update)
                    }
                },
            );

            RefCell::new(Self {
                window,
                document,
                canvas: canvas.clone(),
                runtime,
                host,
                backend,
                input: WebInputAdapter,
                probe,
                themes,
                current_theme,
                current_scene,
                redraw_pending,
                frame_count: 0,
                scale_override: None,
                clock_origin_millis,
            })
        })
    }

    fn sync_surface(&mut self) {
        let surface = measure_surface(&self.window, &self.canvas, self.scale_override);
        self.host.set_surface_info(surface);
        self.runtime
            .resize_logical_viewport(surface.logical_width, surface.logical_height)
            .expect("browser surface must provide a valid logical viewport");
        self.runtime.request_full_redraw();
    }

    fn apply_scene_size(&self) {
        let (width, height) =
            self.themes[self.current_theme].scenes[self.current_scene].logical_size;
        let _ = self
            .canvas
            .style()
            .set_property("aspect-ratio", &format!("{width} / {height}"));
    }

    fn switch_theme(&mut self, theme_index: usize) {
        if theme_index >= self.themes.len() {
            return;
        }
        self.current_theme = theme_index;
        self.current_scene = 0;
        self.replace_scene();
    }

    fn switch_scene(&mut self, scene_index: usize) {
        if scene_index >= self.themes[self.current_theme].scenes.len() {
            return;
        }
        self.current_scene = scene_index;
        self.replace_scene();
    }

    fn previous_scene(&mut self) {
        let count = self.themes[self.current_theme].scenes.len();
        if count > 0 {
            self.switch_scene((self.current_scene + count - 1) % count);
        }
    }

    fn next_scene(&mut self) {
        let count = self.themes[self.current_theme].scenes.len();
        if count > 0 {
            self.switch_scene((self.current_scene + 1) % count);
        }
    }

    fn replace_scene(&mut self) {
        self.advance_runtime_time();
        self.runtime.pointer_exited();
        self.sync_platform_effects();
        self.runtime.release_focus();
        self.runtime = create_scene(&mut self.themes, self.current_theme, self.current_scene);
        self.advance_runtime_time();
        self.apply_scene_size();
        self.sync_surface();
        self.update_navigation();
        self.request_and_render();
    }

    fn update_navigation(&self) {
        let theme = &self.themes[self.current_theme];
        set_text(&self.document, "theme-title", theme.title);
        set_text(
            &self.document,
            "scene-title",
            theme.scenes[self.current_scene].title,
        );
        set_text(
            &self.document,
            "scene-count",
            &format!("{}/{}", self.current_scene + 1, theme.scenes.len()),
        );
        for (index, suite) in self.themes.iter().enumerate() {
            if let Some(button) = self
                .document
                .get_element_by_id(&format!("theme-{}", suite.id))
            {
                let _ = button.set_attribute(
                    "aria-selected",
                    if index == self.current_theme {
                        "true"
                    } else {
                        "false"
                    },
                );
            }
        }
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
        self.host.set_cursor(self.runtime.cursor_icon());
        self.host.request_redraw();
        if !self.redraw_pending.replace(false) {
            self.sync_platform_effects();
            return;
        }
        let Some(submission) = self
            .runtime
            .prepare_submission(self.host.surface_info(), self.backend.capabilities())
        else {
            self.sync_platform_effects();
            self.update_status(DamageMode::None);
            return;
        };
        let damage = submission.damage.mode();
        let outcome = self.backend.submit(&submission);
        self.runtime
            .complete_submission(submission.session_id, submission.frame_id, outcome);
        self.sync_platform_effects();
        self.frame_count += 1;
        self.update_status(damage);
    }

    fn advance_runtime_time(&mut self) {
        let now = web_monotonic_time(&self.window, self.clock_origin_millis);
        let _ = self.runtime.advance_time(now);
    }

    fn sync_platform_effects(&mut self) {
        self.host.set_cursor(self.runtime.cursor_icon());
        for update in self.runtime.take_tooltip_updates() {
            self.host.update_tooltip(update);
        }
        for update in self.runtime.take_accessibility_updates() {
            self.host.update_accessibility(update);
        }
        self.host.schedule_wake(self.runtime.next_wake_deadline());
    }

    fn on_runtime_wake(&mut self) {
        self.advance_runtime_time();
        self.request_and_render();
    }

    fn update_status(&self, damage: DamageMode) {
        let status = format!(
            "READY · {} · frame {} · {:?} · pointer {} · wheel {} · key {}",
            self.backend.name(),
            self.frame_count,
            damage,
            self.probe.pointer_events.get(),
            self.probe.wheel_events.get(),
            self.probe.key_events.get()
        );
        set_text(&self.document, "runtime-status", &status);
        let pointer_status = if let Some(label) = *self.probe.focused_label.borrow() {
            format!("Focused {label}")
        } else if let Some(label) = *self.probe.hovered_label.borrow() {
            format!("Hovered {label}")
        } else {
            "Idle".to_string()
        };
        set_text(&self.document, "pointer-status", &pointer_status);
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
        set_text(&self.document, "backend-status", self.backend.name());
        if let Some(body) = self.document.body() {
            let _ = body.set_attribute("data-ready", "true");
            let _ = body.set_attribute("data-backend", self.backend.data_name());
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
        self.advance_runtime_time();
        if let Some(point) = self.pointer_input(&event).logical_position() {
            self.runtime.dispatch_mouse_moved(point.x(), point.y());
            self.request_and_render();
        }
    }

    fn on_pointer_down(&mut self, event: PointerEvent) {
        self.advance_runtime_time();
        let Some(button) = adapt_pointer_button(event.button()) else {
            return;
        };
        let _ = self.canvas.focus();
        let _ = self.canvas.set_pointer_capture(event.pointer_id());
        if let Some(point) = self.pointer_input(&event).logical_position() {
            self.runtime.dispatch_mouse_moved(point.x(), point.y());
            self.runtime
                .dispatch_mouse_pressed(point.x(), point.y(), button);
            self.request_and_render();
        }
    }

    fn on_pointer_up(&mut self, event: PointerEvent) {
        self.advance_runtime_time();
        if let Some(button) = adapt_pointer_button(event.button())
            && let Some(point) = self.pointer_input(&event).logical_position()
        {
            self.runtime
                .dispatch_mouse_released(point.x(), point.y(), button);
        }
        let _ = self.canvas.release_pointer_capture(event.pointer_id());
        self.request_and_render();
    }

    fn on_pointer_leave(&mut self) {
        self.advance_runtime_time();
        self.runtime.pointer_exited();
        self.request_and_render();
    }

    fn on_wheel(&mut self, event: DomWheelEvent) {
        self.advance_runtime_time();
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
        if let Some(gesture) = self.input.adapt_wheel_gesture(
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
            match gesture {
                AdaptedGesture::Scroll(wheel) => self.runtime.dispatch_scroll(wheel),
                AdaptedGesture::Zoom(zoom) => self.runtime.dispatch_zoom(zoom),
            };
            self.request_and_render();
        }
    }

    fn on_key(&mut self, event: KeyboardEvent, pressed: bool) {
        self.advance_runtime_time();
        let Some(key) = map_key(&event.key()) else {
            return;
        };
        let modifiers = modifiers(
            event.ctrl_key(),
            event.shift_key(),
            event.alt_key(),
            event.meta_key(),
        );
        match adapt_key_input(key, pressed, modifiers) {
            AdaptedKeyInput::FocusTraversal(direction) => {
                if matches!(
                    self.runtime.traverse_focus(direction),
                    FocusTraversalOutcome::Moved(_)
                ) {
                    event.prevent_default();
                    self.request_and_render();
                }
            }
            AdaptedKeyInput::Key {
                key,
                pressed,
                modifiers,
            } => {
                if pressed {
                    self.runtime.dispatch_key_pressed(key, modifiers);
                } else {
                    self.runtime.dispatch_key_released(key, modifiers);
                }
                self.request_and_render();
            }
            AdaptedKeyInput::Ignored => {}
        }
    }

    fn toggle_scale(&mut self) {
        self.advance_runtime_time();
        self.scale_override = Some(if self.host.surface_info().scale_factor < 1.5 {
            2.0
        } else {
            1.0
        });
        self.sync_surface();
        self.request_and_render();
    }
}

fn browser_now(window: &Window) -> f64 {
    window.performance().map_or(0.0, |clock| clock.now())
}

fn web_monotonic_time(window: &Window, origin_millis: f64) -> MonotonicTime {
    let elapsed_micros = ((browser_now(window) - origin_millis).max(0.0) * 1_000.0).round();
    MonotonicTime::from_micros(elapsed_micros.min(u64::MAX as f64) as u64)
}

fn update_web_tooltip(canvas: &HtmlCanvasElement, tooltip: &HtmlElement, update: TooltipUpdate) {
    let snapshot = match update {
        TooltipUpdate::Show(snapshot) | TooltipUpdate::Replace(snapshot) => snapshot,
        TooltipUpdate::Hide { .. } => {
            let _ = tooltip.style().set_property("display", "none");
            return;
        }
    };
    tooltip.set_text_content(Some(&snapshot.text));
    let _ = tooltip.style().set_property("display", "block");
    let popup = tooltip.get_bounding_client_rect();
    let canvas_bounds = canvas.get_bounding_client_rect();
    let Some(placement) = place_tooltip(
        snapshot.anchor,
        (popup.width(), popup.height()),
        Rectangle::new(0.0, 0.0, canvas_bounds.width(), canvas_bounds.height()),
        snapshot.placement,
    ) else {
        let _ = tooltip.style().set_property("display", "none");
        return;
    };
    let _ = tooltip
        .style()
        .set_property("left", &format!("{}px", canvas_bounds.left() + placement.x));
    let _ = tooltip
        .style()
        .set_property("top", &format!("{}px", canvas_bounds.top() + placement.y));
}

fn update_web_accessibility(
    document: &Document,
    tree: &HtmlElement,
    state: &mut WebAccessibilityState,
    update: AccessibilityUpdate,
) {
    match update {
        AccessibilityUpdate::Snapshot(snapshot) => {
            state.nodes = snapshot
                .nodes
                .iter()
                .cloned()
                .map(|node| (node.id, node))
                .collect();
            state.root = Some(snapshot.root);
            state.revision = snapshot.revision;
        }
        AccessibilityUpdate::Delta(delta) => {
            if state.revision != delta.base_revision {
                return;
            }
            for removed in delta.removed {
                state.nodes.remove(&removed);
            }
            for node in delta.upserts {
                state.nodes.insert(node.id, node);
            }
            if let Some(root) = state.root.and_then(|root| state.nodes.get_mut(&root)) {
                root.children = delta.root_children;
            }
            state.revision = delta.revision;
        }
    }

    tree.set_inner_html("");
    if let Some(root) = state.root
        && let Some(root_node) = state.nodes.get(&root)
    {
        for child in &root_node.children {
            if let Some(element) = accessibility_element(document, state, *child, 1) {
                let _ = tree.append_child(&element);
            }
        }
    }
    if let Some(body) = document.body() {
        let _ = body.set_attribute("data-accessibility-revision", &state.revision.to_string());
    }
}

fn accessibility_element(
    document: &Document,
    state: &WebAccessibilityState,
    id: AccessibilityNodeId,
    depth: usize,
) -> Option<Element> {
    if depth > novadraw::tree::MAX_TREE_DEPTH {
        return None;
    }
    let node = state.nodes.get(&id)?;
    let element = document.create_element("div").ok()?;
    let role = match node.role {
        AccessibilityRole::Group => "group",
        AccessibilityRole::Text => "text",
        AccessibilityRole::Image => "img",
        AccessibilityRole::Button | AccessibilityRole::ToggleButton => "button",
    };
    let _ = element.set_attribute("role", role);
    let _ = element.set_attribute("aria-label", &node.name);
    let _ = element.set_attribute(
        "aria-disabled",
        if node.state.enabled { "false" } else { "true" },
    );
    if node.role == AccessibilityRole::ToggleButton {
        let _ = element.set_attribute(
            "aria-pressed",
            if node.state.selected { "true" } else { "false" },
        );
    }
    if node.state.focusable {
        let _ = element.set_attribute("tabindex", "-1");
    }
    let _ = element.set_attribute(
        "data-bounds",
        &format!(
            "{},{},{},{}",
            node.bounds.x, node.bounds.y, node.bounds.width, node.bounds.height
        ),
    );
    for child in &node.children {
        if let Some(child_element) = accessibility_element(document, state, *child, depth + 1) {
            let _ = element.append_child(&child_element);
        }
    }
    Some(element)
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

fn install_theme_buttons(
    document: &Document,
    themes: &[DemoSuite],
) -> Result<Vec<HtmlButtonElement>, JsValue> {
    let container = document
        .get_element_by_id("theme-tabs")
        .ok_or_else(|| JsValue::from_str("missing element #theme-tabs"))?;
    let markup = themes
        .iter()
        .enumerate()
        .map(|(index, theme)| {
            format!(
                r#"<button id="theme-{}" type="button" role="tab" aria-selected="{}">{}</button>"#,
                theme.id,
                index == 0,
                theme.title
            )
        })
        .collect::<String>();
    container.set_inner_html(&markup);
    themes
        .iter()
        .map(|theme| element(document, &format!("theme-{}", theme.id)))
        .collect()
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

fn measure_surface(
    window: &Window,
    canvas: &HtmlCanvasElement,
    scale_override: Option<f64>,
) -> SurfaceInfo {
    let rect = canvas.get_bounding_client_rect();
    let logical_width = rect.width().max(1.0);
    let logical_height = rect.height().max(1.0);
    let scale_factor = scale_override.unwrap_or_else(|| window.device_pixel_ratio().max(1.0));
    SurfaceInfo {
        logical_width,
        logical_height,
        pixel_width: (logical_width * scale_factor).round() as u32,
        pixel_height: (logical_height * scale_factor).round() as u32,
        scale_factor,
    }
}

#[cfg(target_arch = "wasm32")]
async fn create_backend(
    window: &Window,
    canvas: &HtmlCanvasElement,
    surface: SurfaceInfo,
) -> Result<ValidationBackend, JsValue> {
    let parameters = UrlSearchParams::new_with_str(&window.location().search()?)?;
    match parameters.get("backend").as_deref() {
        Some("canvas2d") => {
            let context = canvas
                .get_context("2d")?
                .ok_or_else(|| JsValue::from_str("Canvas2D unavailable"))?
                .dyn_into::<CanvasRenderingContext2d>()?;
            Ok(ValidationBackend::Canvas2d(Canvas2dBackend::new(
                canvas.clone(),
                context,
            )))
        }
        None | Some("vello") => VelloRenderer::new_web(canvas.clone(), surface)
            .await
            .map(Box::new)
            .map(ValidationBackend::Vello)
            .map_err(|error| {
                JsValue::from_str(&format!("Vello WebGPU initialization failed: {error}"))
            }),
        Some(value) => Err(JsValue::from_str(&format!(
            "unsupported backend '{value}', expected 'vello' or 'canvas2d'"
        ))),
    }
}

#[cfg(target_arch = "wasm32")]
async fn start_async() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("missing document"))?;
    let canvas: HtmlCanvasElement = element(&document, "novadraw-canvas")?;
    let surface = measure_surface(&window, &canvas, None);
    canvas.set_width(surface.pixel_width);
    canvas.set_height(surface.pixel_height);
    let backend = create_backend(&window, &canvas, surface).await?;
    let parameters = UrlSearchParams::new_with_str(&window.location().search()?)?;
    if parameters.get("mode").as_deref() == Some("direct-edit") {
        return direct_edit_mode::start(window, document, canvas, backend);
    }

    let app = WebValidationApp::new(window.clone(), document.clone(), canvas.clone(), backend);
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
    register_event(target, "pointerleave", &app, |app, _event| {
        app.on_pointer_leave()
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
        app.advance_runtime_time();
        app.runtime.pointer_exited();
        app.runtime.release_focus();
        app.request_and_render();
    })?;

    let scale_button: HtmlButtonElement = element(&document, "toggle-scale")?;
    register_event(scale_button.as_ref(), "click", &app, |app, _event| {
        app.toggle_scale()
    })?;
    let theme_buttons = install_theme_buttons(&document, &app.borrow().themes)?;
    for (theme_index, button) in theme_buttons.into_iter().enumerate() {
        register_event(button.as_ref(), "click", &app, move |app, _event| {
            app.switch_theme(theme_index)
        })?;
    }
    let previous_scene: HtmlButtonElement = element(&document, "previous-scene")?;
    register_event(previous_scene.as_ref(), "click", &app, |app, _event| {
        app.previous_scene()
    })?;
    let next_scene: HtmlButtonElement = element(&document, "next-scene")?;
    register_event(next_scene.as_ref(), "click", &app, |app, _event| {
        app.next_scene()
    })?;
    register_event(window.as_ref(), "resize", &app, |app, _event| {
        app.sync_surface();
        app.request_and_render();
    })?;

    {
        let mut app = app.borrow_mut();
        app.host.set_cursor(CursorIcon::Crosshair);
        app.update_navigation();
        app.sync_surface();
        app.request_and_render();
    }

    // Event closures own an Rc; retaining this reference documents the app lifetime.
    std::mem::forget(app);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    spawn_local(async {
        if let Err(error) = start_async().await {
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                if let Some(body) = document.body() {
                    let _ = body.set_attribute("data-ready", "error");
                }
                set_text(
                    &document,
                    "runtime-status",
                    &format!("ERROR · {}", error.as_string().unwrap_or_default()),
                );
            }
            web_sys::console::error_1(&error);
        }
    });
}
