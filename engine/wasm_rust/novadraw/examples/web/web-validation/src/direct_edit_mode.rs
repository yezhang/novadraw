use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    ops::Range,
    rc::{Rc, Weak},
};

use novadraw::figure::border::LineBorder;
use novadraw::render::{BuiltinFont, DamageMode, SurfaceInfo};
use novadraw::{
    Alignment, Color, Figure, FigureStyle, FlowPage, FlowWrapping, FreeformLayerFigure,
    LabelFigure, MouseButton, PlatformHost, Point, Rectangle, RectangleFigure, RenderBackend,
    TextFlowFigure,
};
use novadraw_editor::{
    Command, CommandError, DirectTextEdit, DirectTextEditDescriptor, DirectTextEditRequest,
    DirectTextEditState, DirectTextFeature, DirectTextFeedback, EditPartBehavior, EditPartError,
    EditPartFactory, EditPolicy, EditorDomain, EditorRequest, FeedbackVisual, FocusLossPolicy,
    GraphicalViewer, ModelAdapter, ModelEvent, ModelRevision, PartFactoryContext, PolicyError,
    PolicyHost, PolicyInstallation, PolicyRole, TextEditMode, TextInputEffect, TextInputSnapshot,
    VisualUpdateContext,
};
use novadraw_platform_web::{
    WebEditContextHost, WebPlatformHost, WebPointerInput, WebTextInputHost, adapt_pointer_button,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    Document, Event, HtmlButtonElement, HtmlCanvasElement, MouseEvent, PointerEvent,
    UrlSearchParams, Window,
};

use super::{ValidationBackend, browser_now, measure_surface, set_text, web_monotonic_time};

const ROOT: DirectEditNode = DirectEditNode(1);
const LABEL: DirectEditNode = DirectEditNode(2);
const LOGICAL_WIDTH: f64 = 800.0;
const LOGICAL_HEIGHT: f64 = 500.0;
const LABEL_X: f64 = 150.0;
const LABEL_Y: f64 = 180.0;
const LABEL_WIDTH: f64 = 500.0;
const LABEL_HEIGHT: f64 = 120.0;
const LABEL_FEATURE: &str = "label";
const INITIAL_LABEL: &str = "Click to edit this label";
const NODE_COLOR: Color = Color::rgba(0.12, 0.42, 0.70, 1.0);
const NODE_BORDER_COLOR: Color = Color::rgba(0.05, 0.16, 0.25, 1.0);
const EDIT_BORDER_COLOR: Color = Color::rgba(0.94, 0.48, 0.08, 1.0);
const NODE_BORDER_WIDTH: f64 = 2.0;
const DIRECT_TEXT_INSET: f64 = 4.0;

fn label_bounds() -> Rectangle {
    Rectangle::new(LABEL_X, LABEL_Y, LABEL_WIDTH, LABEL_HEIGHT)
}

fn label_style() -> FigureStyle {
    FigureStyle {
        foreground: Some(Color::WHITE),
        background: Some(NODE_COLOR),
        font: Some("28px Inter Variable".to_owned()),
        ..FigureStyle::default()
    }
}

fn direct_text_style() -> FigureStyle {
    FigureStyle {
        foreground: Some(Color::WHITE),
        font: Some("28px Inter Variable".to_owned()),
        ..FigureStyle::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DirectEditNode(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectEditEvent {
    LabelChanged,
}

struct DirectEditModel {
    revision: ModelRevision,
    label: String,
    events: Vec<ModelEvent<DirectEditNode, DirectEditEvent>>,
}

impl DirectEditModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            label: INITIAL_LABEL.to_owned(),
            events: Vec::new(),
        }
    }

    fn set_label(&mut self, label: String) {
        self.label = label;
        self.revision = self
            .revision
            .next()
            .expect("direct-edit validation must not exhaust model revisions");
        self.events.push(ModelEvent::new(
            self.revision,
            LABEL,
            DirectEditEvent::LabelChanged,
        ));
    }
}

impl ModelAdapter for DirectEditModel {
    type ModelId = DirectEditNode;
    type Event = DirectEditEvent;
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        ROOT
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == ROOT {
            vec![LABEL]
        } else {
            Vec::new()
        })
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct SetLabelCommand {
    before: String,
    after: String,
}

impl Command<DirectEditModel> for SetLabelCommand {
    fn label(&self) -> &str {
        "Rename label"
    }

    fn execute(&mut self, model: &mut DirectEditModel) -> Result<(), CommandError> {
        model.set_label(self.after.clone());
        Ok(())
    }

    fn undo(&mut self, model: &mut DirectEditModel) -> Result<(), CommandError> {
        model.set_label(self.before.clone());
        Ok(())
    }
}

struct LabelDirectEdit {
    descriptor: DirectTextEditDescriptor,
}

impl DirectTextEdit<DirectEditModel> for LabelDirectEdit {
    fn descriptor(&self) -> &DirectTextEditDescriptor {
        &self.descriptor
    }

    fn feedback(
        &mut self,
        state: &DirectTextEditState,
        _model: &DirectEditModel,
    ) -> Result<DirectTextFeedback, PolicyError> {
        let text_bounds = label_bounds().inflate(-DIRECT_TEXT_INSET, -DIRECT_TEXT_INSET);
        DirectTextFeedback::new(
            vec![
                FeedbackVisual::scaled(Box::new(
                    RectangleFigure::new_with_color(
                        LABEL_X,
                        LABEL_Y,
                        LABEL_WIDTH,
                        LABEL_HEIGHT,
                        NODE_COLOR,
                    )
                    .with_stroke(EDIT_BORDER_COLOR, NODE_BORDER_WIDTH),
                )),
                FeedbackVisual::scaled(Box::new(
                    TextFlowFigure::new(text_bounds, FlowPage::from_text(state.draft()))
                        .with_wrapping(FlowWrapping::NoWrap)
                        .with_alignment(Alignment::Center, Alignment::Center),
                ))
                .with_style(direct_text_style()),
            ],
            1,
        )
    }

    fn command(
        &mut self,
        state: &DirectTextEditState,
        model: &DirectEditModel,
    ) -> Result<Box<dyn Command<DirectEditModel>>, PolicyError> {
        Ok(Box::new(SetLabelCommand {
            before: model.label.clone(),
            after: state.draft().to_owned(),
        }))
    }
}

struct LabelPolicy;

impl EditPolicy<DirectEditModel> for LabelPolicy {
    fn understands(&self, _request: &EditorRequest) -> bool {
        false
    }

    fn command(
        &mut self,
        _host: PolicyHost<DirectEditNode>,
        _request: &EditorRequest,
        _model: &DirectEditModel,
    ) -> Result<Option<Box<dyn Command<DirectEditModel>>>, PolicyError> {
        Ok(None)
    }

    fn start_direct_text_edit(
        &mut self,
        host: PolicyHost<DirectEditNode>,
        request: &DirectTextEditRequest,
        model: &DirectEditModel,
    ) -> Result<Option<Box<dyn DirectTextEdit<DirectEditModel>>>, PolicyError> {
        if host.model() != LABEL || request.feature().as_str() != LABEL_FEATURE {
            return Ok(None);
        }
        Ok(Some(Box::new(LabelDirectEdit {
            descriptor: DirectTextEditDescriptor::new(
                request.feature().clone(),
                model.label.clone(),
                model.revision(),
                TextEditMode::SingleLine,
                FocusLossPolicy::Accept,
            ),
        })))
    }
}

struct DirectEditPart {
    render_text: bool,
}

impl EditPartBehavior<DirectEditModel> for DirectEditPart {
    fn create_figure(
        &mut self,
        model: &DirectEditModel,
        model_id: DirectEditNode,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(if model_id == ROOT {
            Box::new(FreeformLayerFigure::new(
                0.0,
                0.0,
                LOGICAL_WIDTH,
                LOGICAL_HEIGHT,
            ))
        } else if self.render_text {
            Box::new(
                LabelFigure::new(model.label.clone())
                    .with_bounds(label_bounds())
                    .with_border(LineBorder::new(NODE_BORDER_COLOR, NODE_BORDER_WIDTH)),
            )
        } else {
            Box::new(
                RectangleFigure::new_with_color(
                    LABEL_X,
                    LABEL_Y,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    NODE_COLOR,
                )
                .with_stroke(NODE_BORDER_COLOR, NODE_BORDER_WIDTH),
            )
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &DirectEditModel,
        model_id: DirectEditNode,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        if model_id == LABEL {
            context.set_primary_bounds(label_bounds())?;
            context.set_primary_style(label_style())?;
            context.set_primary_opaque(true)?;
            if self.render_text {
                context.set_primary_label_text(model.label.clone())?;
            }
        }
        Ok(())
    }

    fn create_policies(
        &mut self,
        _model: &DirectEditModel,
        model_id: DirectEditNode,
    ) -> Result<Vec<PolicyInstallation<DirectEditModel>>, EditPartError> {
        Ok(if model_id == LABEL {
            vec![(PolicyRole::DirectTextEdit, Box::new(LabelPolicy))]
        } else {
            Vec::new()
        })
    }
}

struct DirectEditFactory {
    render_text: bool,
}

impl EditPartFactory<DirectEditModel> for DirectEditFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<DirectEditNode>,
        _model: &DirectEditModel,
    ) -> Result<Box<dyn EditPartBehavior<DirectEditModel>>, EditPartError> {
        Ok(Box::new(DirectEditPart {
            render_text: self.render_text,
        }))
    }
}

type DirectEditViewer = GraphicalViewer<DirectEditModel, DirectEditFactory>;
type AppRef = Rc<RefCell<DirectEditWebApp>>;

enum DirectEditTextInputHost {
    Textarea(WebTextInputHost),
    EditContext(WebEditContextHost),
}

impl DirectEditTextInputHost {
    fn set_surface_origin(&self, origin: Point) -> Result<(), JsValue> {
        match self {
            Self::Textarea(host) => {
                host.set_surface_origin(origin);
                Ok(())
            }
            Self::EditContext(host) => host.set_surface_origin(origin),
        }
    }

    fn apply_effect(
        &self,
        effect: &TextInputEffect,
        snapshot: Option<&TextInputSnapshot>,
    ) -> Result<(), JsValue> {
        match self {
            Self::Textarea(host) => {
                host.apply_effect(effect);
                Ok(())
            }
            Self::EditContext(host) => host.apply_effect(effect, snapshot).map(|_| ()),
        }
    }

    fn take_events(&self) -> Result<Vec<novadraw_editor::SessionTextInputEvent>, JsValue> {
        match self {
            Self::Textarea(host) => Ok(host.take_events()),
            Self::EditContext(host) => host
                .take_events()
                .map_err(|error| JsValue::from_str(&error.to_string())),
        }
    }

    const fn name(&self) -> &'static str {
        match self {
            Self::Textarea(_) => "textarea",
            Self::EditContext(_) => "EditContext",
        }
    }

    fn active_session(&self) -> Option<novadraw_editor::DirectTextEditSessionId> {
        match self {
            Self::Textarea(host) => host.active_session(),
            Self::EditContext(host) => host.active_session(),
        }
    }
}

struct DirectEditWebApp {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    viewer: DirectEditViewer,
    domain: EditorDomain<DirectEditModel>,
    host: WebPlatformHost,
    backend: ValidationBackend,
    text_input: DirectEditTextInputHost,
    text_input_fallback: Option<String>,
    redraw_pending: Rc<Cell<bool>>,
    wake_schedule_count: Rc<Cell<u64>>,
    frame_count: u64,
    wake_count: u64,
    scale_override: Option<f64>,
    clock_origin_millis: f64,
    last_error: Option<String>,
}

impl DirectEditWebApp {
    fn new(
        window: Window,
        document: Document,
        canvas: HtmlCanvasElement,
        backend: ValidationBackend,
        prefer_edit_context: bool,
    ) -> Result<AppRef, JsValue> {
        let render_text = backend.data_name() != "canvas2d";
        let mut viewer = DirectEditViewer::new(
            DirectEditModel::new(),
            DirectEditFactory { render_text },
            Rectangle::new(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT),
        )
        .map_err(js_error)?;
        viewer
            .runtime_mut()
            .register_builtin_font(BuiltinFont::Inter)
            .map_err(js_error)?;
        viewer
            .runtime_mut()
            .register_builtin_font(BuiltinFont::NotoSansSc)
            .map_err(js_error)?;

        let redraw_pending = Rc::new(Cell::new(false));
        let wake_generation = Rc::new(Cell::new(0_u64));
        let wake_schedule_count = Rc::new(Cell::new(0_u64));
        let clock_origin_millis = browser_now(&window);
        let app_slot: Rc<RefCell<Option<Weak<RefCell<Self>>>>> = Rc::new(RefCell::new(None));
        let edit_context = if prefer_edit_context {
            Some(WebEditContextHost::new_with_callbacks(
                &canvas,
                {
                    let app_slot = Rc::clone(&app_slot);
                    move || {
                        let app = app_slot.borrow().as_ref().and_then(Weak::upgrade);
                        if let Some(app) = app {
                            app.borrow_mut().on_text_input_ready();
                        }
                    }
                },
                {
                    let app_slot = Rc::clone(&app_slot);
                    move |ranges: Vec<Range<usize>>| {
                        let app = app_slot.borrow().as_ref().and_then(Weak::upgrade)?;
                        let app = app.borrow();
                        ranges
                            .into_iter()
                            .map(|range| app.viewer.direct_text_range_bounds(range).ok())
                            .collect()
                    }
                },
            ))
        } else {
            None
        };
        let (text_input, text_input_fallback) = match edit_context {
            Some(Ok(Some(host))) => (DirectEditTextInputHost::EditContext(host), None),
            Some(Ok(None)) => (
                DirectEditTextInputHost::Textarea(WebTextInputHost::new_with_event_ready(
                    &document,
                    {
                        let app_slot = Rc::clone(&app_slot);
                        move || {
                            let app = app_slot.borrow().as_ref().and_then(Weak::upgrade);
                            if let Some(app) = app {
                                app.borrow_mut().on_text_input_ready();
                            }
                        }
                    },
                )?),
                Some("EditContext unsupported".to_owned()),
            ),
            Some(Err(error)) => (
                DirectEditTextInputHost::Textarea(WebTextInputHost::new_with_event_ready(
                    &document,
                    {
                        let app_slot = Rc::clone(&app_slot);
                        move || {
                            let app = app_slot.borrow().as_ref().and_then(Weak::upgrade);
                            if let Some(app) = app {
                                app.borrow_mut().on_text_input_ready();
                            }
                        }
                    },
                )?),
                Some(
                    error
                        .as_string()
                        .unwrap_or_else(|| "EditContext initialization failed".to_owned()),
                ),
            ),
            None => (
                DirectEditTextInputHost::Textarea(WebTextInputHost::new_with_event_ready(
                    &document,
                    {
                        let app_slot = Rc::clone(&app_slot);
                        move || {
                            let app = app_slot.borrow().as_ref().and_then(Weak::upgrade);
                            if let Some(app) = app {
                                app.borrow_mut().on_text_input_ready();
                            }
                        }
                    },
                )?),
                None,
            ),
        };
        let host = WebPlatformHost::new(
            SurfaceInfo::default(),
            {
                let redraw_pending = Rc::clone(&redraw_pending);
                move || redraw_pending.set(true)
            },
            {
                let canvas = canvas.clone();
                move |cursor| {
                    let value = if cursor == novadraw::figure::CursorIcon::Text {
                        "text"
                    } else {
                        "default"
                    };
                    let _ = canvas.style().set_property("cursor", value);
                }
            },
            |_| {},
            {
                let app_slot = Rc::clone(&app_slot);
                let generation = Rc::clone(&wake_generation);
                let schedule_count = Rc::clone(&wake_schedule_count);
                let window = window.clone();
                move |deadline| {
                    schedule_count.set(schedule_count.get() + 1);
                    let next_generation = generation.get().wrapping_add(1);
                    generation.set(next_generation);
                    let Some(deadline) = deadline else {
                        return;
                    };
                    let now = web_monotonic_time(&window, clock_origin_millis);
                    let delay_micros = deadline.as_micros().saturating_sub(now.as_micros());
                    let delay_millis = delay_micros.div_ceil(1_000).min(i32::MAX as u64) as i32;
                    let app_slot = Rc::clone(&app_slot);
                    let generation = Rc::clone(&generation);
                    let callback = Closure::once_into_js(move || {
                        if generation.get() == next_generation {
                            let app = app_slot.borrow().as_ref().and_then(Weak::upgrade);
                            if let Some(app) = app {
                                app.borrow_mut().on_runtime_wake();
                            }
                        }
                    });
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        callback.unchecked_ref(),
                        delay_millis,
                    );
                }
            },
            |_| {},
            |_| {},
        );
        let app = Rc::new(RefCell::new(Self {
            window,
            document,
            canvas,
            viewer,
            domain: EditorDomain::new(),
            host,
            backend,
            text_input,
            text_input_fallback,
            redraw_pending,
            wake_schedule_count,
            frame_count: 0,
            wake_count: 0,
            scale_override: None,
            clock_origin_millis,
            last_error: None,
        }));
        *app_slot.borrow_mut() = Some(Rc::downgrade(&app));
        Ok(app)
    }

    fn sync_surface(&mut self) {
        let surface = measure_surface(&self.window, &self.canvas, self.scale_override);
        self.host.set_surface_info(surface);
        let bounds = self.canvas.get_bounding_client_rect();
        if let Err(error) = self
            .text_input
            .set_surface_origin(Point::new(bounds.left(), bounds.top()))
        {
            self.last_error = Some(
                error
                    .as_string()
                    .unwrap_or_else(|| "text input geometry update failed".to_owned()),
            );
        }
        if let Err(error) = self
            .viewer
            .runtime_mut()
            .resize_logical_viewport(surface.logical_width, surface.logical_height)
        {
            self.last_error = Some(error.to_string());
        }
        if self.viewer.direct_text_edit().is_some()
            && let Err(error) = self.viewer.synchronize_direct_text_input_area()
        {
            self.last_error = Some(error.to_string());
        }
        self.viewer.runtime_mut().request_full_redraw();
        self.sync_text_input_effects();
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

    fn on_pointer_move(&mut self, event: PointerEvent) {
        self.advance_time();
        if let Some(point) = self.pointer_input(&event).logical_position()
            && let Err(error) = self.domain.pointer_moved(&mut self.viewer, point)
        {
            self.last_error = Some(error.to_string());
        }
        self.request_and_render();
    }

    fn on_pointer_down(&mut self, event: PointerEvent) {
        self.advance_time();
        let Some(button) = adapt_pointer_button(event.button()) else {
            return;
        };
        if self.viewer.direct_text_edit().is_some() {
            event.prevent_default();
        } else {
            let _ = self.canvas.focus();
        }
        let _ = self.canvas.set_pointer_capture(event.pointer_id());
        if let Some(point) = self.pointer_input(&event).logical_position()
            && let Err(error) = self.domain.pointer_pressed(
                &mut self.viewer,
                point,
                button,
                pointer_modifiers(&event),
            )
        {
            self.last_error = Some(error.to_string());
        }
        self.sync_text_input_effects();
        self.request_and_render();
    }

    fn on_pointer_up(&mut self, event: PointerEvent) {
        self.advance_time();
        if let Some(button) = adapt_pointer_button(event.button())
            && let Some(point) = self.pointer_input(&event).logical_position()
        {
            if let Err(error) = self
                .domain
                .pointer_released(&mut self.viewer, point, button)
            {
                self.last_error = Some(error.to_string());
            }
            if button == MouseButton::Left {
                self.start_edit_at(point);
            }
        }
        let _ = self.canvas.release_pointer_capture(event.pointer_id());
        self.sync_text_input_effects();
        self.request_and_render();
    }

    fn on_pointer_leave(&mut self) {
        self.advance_time();
        self.domain.pointer_exited(&mut self.viewer);
        self.request_and_render();
    }

    fn on_click(&mut self, event: MouseEvent) {
        self.advance_time();
        let bounds = self.canvas.get_bounding_client_rect();
        self.start_edit_at(Point::new(
            f64::from(event.client_x()) - bounds.left(),
            f64::from(event.client_y()) - bounds.top(),
        ));
        self.sync_text_input_effects();
        self.request_and_render();
    }

    fn start_edit_at(&mut self, point: Point) {
        if self.viewer.direct_text_edit().is_some() || !label_bounds().contains(point) {
            return;
        }
        let Some(source) = self.viewer.part_for_model(LABEL) else {
            return;
        };
        if let Err(error) = self.viewer.replace_selection(source) {
            self.last_error = Some(error.to_string());
            return;
        }
        let feature = DirectTextFeature::new(LABEL_FEATURE)
            .expect("static direct-edit feature must be valid");
        if let Err(error) = self
            .domain
            .start_direct_text_edit(&mut self.viewer, source, feature)
        {
            self.last_error = Some(error.to_string());
        }
    }

    fn on_text_input_ready(&mut self) {
        self.advance_time();
        let events = match self.text_input.take_events() {
            Ok(events) => events,
            Err(error) => {
                self.last_error = Some(
                    error
                        .as_string()
                        .unwrap_or_else(|| "text input event conversion failed".to_owned()),
                );
                let _ = self.domain.cancel_direct_text_edit(&mut self.viewer);
                self.sync_text_input_effects();
                self.request_and_render();
                return;
            }
        };
        for event in events {
            if let Err(error) = self.domain.handle_text_input_event(&mut self.viewer, event) {
                self.last_error = Some(error.to_string());
            }
        }
        self.sync_text_input_effects();
        self.request_and_render();
    }

    fn sync_text_input_effects(&mut self) {
        let bounds = self.canvas.get_bounding_client_rect();
        if let Err(error) = self
            .text_input
            .set_surface_origin(Point::new(bounds.left(), bounds.top()))
        {
            self.last_error = Some(
                error
                    .as_string()
                    .unwrap_or_else(|| "text input geometry update failed".to_owned()),
            );
        }
        let snapshot = match self
            .viewer
            .direct_text_edit()
            .map(DirectTextEditState::input_snapshot)
            .transpose()
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        };
        for effect in self.viewer.take_text_input_effects() {
            if let Err(error) = self.text_input.apply_effect(&effect, snapshot.as_ref()) {
                self.last_error = Some(
                    error
                        .as_string()
                        .unwrap_or_else(|| "text input host effect failed".to_owned()),
                );
            }
        }
    }

    fn toggle_scale(&mut self) {
        self.advance_time();
        self.scale_override = Some(if self.host.surface_info().scale_factor < 1.5 {
            2.0
        } else {
            1.0
        });
        self.sync_surface();
        self.request_and_render();
    }

    fn advance_time(&mut self) {
        let now = web_monotonic_time(&self.window, self.clock_origin_millis);
        if let Err(error) = self.viewer.advance_time(now) {
            self.last_error = Some(error.to_string());
        }
    }

    fn schedule_next_wake(&self) {
        self.host.schedule_wake(self.viewer.next_wake_deadline());
    }

    fn on_runtime_wake(&mut self) {
        self.wake_count += 1;
        self.request_and_render();
    }

    fn request_and_render(&mut self) {
        self.advance_time();
        self.host.request_redraw();
        if !self.redraw_pending.replace(false) {
            self.schedule_next_wake();
            self.update_status(DamageMode::None);
            return;
        }
        let Some(submission) = self
            .viewer
            .runtime_mut()
            .prepare_submission(self.host.surface_info(), self.backend.capabilities())
        else {
            self.schedule_next_wake();
            self.update_status(DamageMode::None);
            return;
        };
        let damage = submission.damage.mode();
        let outcome = self.backend.submit(&submission);
        self.viewer.runtime_mut().complete_submission(
            submission.session_id,
            submission.frame_id,
            outcome,
        );
        self.frame_count += 1;
        self.schedule_next_wake();
        self.update_status(damage);
    }

    fn update_status(&self, damage: DamageMode) {
        let editing = self.viewer.direct_text_edit();
        let visible_text = editing
            .map(DirectTextEditState::draft)
            .unwrap_or(&self.viewer.model().label);
        set_text(
            &self.document,
            "runtime-status",
            &format!(
                "READY · {} · direct edit · frame {} · {:?}",
                self.backend.name(),
                self.frame_count,
                damage
            ),
        );
        set_text(&self.document, "theme-title", "Editor");
        set_text(
            &self.document,
            "scene-title",
            &format!(
                "Direct text editing · {} input host",
                self.text_input.name()
            ),
        );
        set_text(
            &self.document,
            "pointer-status",
            if editing.is_some() {
                "Editing label"
            } else {
                "Click the label"
            },
        );
        set_text(&self.document, "keyboard-status", visible_text);
        set_text(&self.document, "backend-status", self.backend.name());
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
            let _ = body.set_attribute("data-mode", "direct-edit");
            let _ = body.set_attribute(
                "data-editor-state",
                if editing.is_some() { "editing" } else { "idle" },
            );
            let _ = body.set_attribute("data-label", &self.viewer.model().label);
            let _ = body.set_attribute("data-draft", visible_text);
            let _ = body.set_attribute("data-frame-count", &self.frame_count.to_string());
            let _ = body.set_attribute("data-wake-count", &self.wake_count.to_string());
            let _ = body.set_attribute(
                "data-wake-schedule-count",
                &self.wake_schedule_count.get().to_string(),
            );
            let _ = body.set_attribute(
                "data-next-wake",
                &self
                    .viewer
                    .next_wake_deadline()
                    .map(|deadline| deadline.as_micros().to_string())
                    .unwrap_or_default(),
            );
            let _ = body.set_attribute("data-backend", self.backend.data_name());
            let _ = body.set_attribute("data-text-input-host", self.text_input.name());
            if let Some(reason) = &self.text_input_fallback {
                let _ = body.set_attribute("data-text-input-fallback", reason);
            } else {
                let _ = body.remove_attribute("data-text-input-fallback");
            }
            let _ = body.set_attribute(
                "data-text-input-active",
                if self.text_input.active_session().is_some() {
                    "true"
                } else {
                    "false"
                },
            );
            if let Some(error) = &self.last_error {
                let _ = body.set_attribute("data-error", error);
            } else {
                let _ = body.remove_attribute("data-error");
            }
        }
    }
}

pub(super) fn start(
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    backend: ValidationBackend,
) -> Result<(), JsValue> {
    let parameters = UrlSearchParams::new_with_str(&window.location().search()?)?;
    let prefer_edit_context = parameters.get("text-input").as_deref() == Some("edit-context");
    let app = DirectEditWebApp::new(
        window.clone(),
        document.clone(),
        canvas.clone(),
        backend,
        prefer_edit_context,
    )?;
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
    register_event(target, "click", &app, |app, event| {
        app.on_click(event.unchecked_into::<MouseEvent>())
    })?;
    register_event(window.as_ref(), "resize", &app, |app, _event| {
        app.sync_surface();
        app.request_and_render();
    })?;
    register_event(window.as_ref(), "scroll", &app, |app, _event| {
        app.sync_text_input_effects();
    })?;
    let scale_button: HtmlButtonElement = super::element(&document, "toggle-scale")?;
    register_event(scale_button.as_ref(), "click", &app, |app, _event| {
        app.toggle_scale()
    })?;

    {
        let mut app = app.borrow_mut();
        app.sync_surface();
        app.request_and_render();
    }
    Ok(())
}

fn register_event(
    target: &web_sys::EventTarget,
    name: &str,
    app: &AppRef,
    mut handler: impl FnMut(&mut DirectEditWebApp, Event) + 'static,
) -> Result<(), JsValue> {
    let app = Rc::clone(app);
    let closure = Closure::<dyn FnMut(Event)>::new(move |event| {
        handler(&mut app.borrow_mut(), event);
    });
    target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

fn pointer_modifiers(event: &PointerEvent) -> novadraw::KeyModifiers {
    novadraw::KeyModifiers {
        shift: event.shift_key(),
        control: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
