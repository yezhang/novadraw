use crate::{
    AppState, Session, UiCommand,
    model::{EdgeStyle, ShapeKind},
};
use egui::{
    Color32, Event, Frame, Grid, Image, Key, Margin, PointerButton, Response, RichText, ScrollArea,
    Sense, Stroke, TextEdit, TextureId, Vec2, load::SizedTexture,
};
use novadraw::event::{Key as EditorKey, KeyEvent, KeyEventKind, KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};

const LEFT_PANEL_WIDTH: f32 = 220.0;
const RIGHT_PANEL_WIDTH: f32 = 240.0;
const TOOL_BUTTON_WIDTH: f32 = 74.0;
const PANEL_MARGIN: i8 = 8;
const SCROLL_LINE_POINTS: f32 = 32.0;

pub struct EditorUi {
    canvas_input: CanvasInput,
}

pub struct UiOutput {
    pub canvas_size: [f64; 2],
    pub interaction: Result<bool, String>,
}

impl Default for EditorUi {
    fn default() -> Self {
        Self {
            canvas_input: CanvasInput::default(),
        }
    }
}

impl EditorUi {
    pub fn show(
        &mut self,
        root: &mut egui::Ui,
        app: &mut AppState,
        session: &mut Session,
        texture_id: TextureId,
    ) -> UiOutput {
        self.show_header(root, app);
        self.show_status(root, app);
        self.show_palette_and_outline(root, app);
        self.show_inspector(root, app);

        let mut output = UiOutput {
            canvas_size: [0.0, 0.0],
            interaction: Ok(false),
        };
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(224))
                    .inner_margin(Margin::same(PANEL_MARGIN)),
            )
            .show(root, |ui| {
                let size = ui.available_size().max(Vec2::splat(1.0));
                let image = Image::new(SizedTexture::new(texture_id, size))
                    .fit_to_exact_size(size)
                    .sense(Sense::click_and_drag());
                let response = Frame::new()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0, Color32::from_gray(148)))
                    .show(ui, |ui| ui.add(image))
                    .inner;
                let rect = response.rect;
                app.canvas_bounds = Rectangle::new(
                    f64::from(rect.min.x),
                    f64::from(rect.min.y),
                    f64::from(rect.width()),
                    f64::from(rect.height()),
                );
                output.canvas_size = [f64::from(rect.width()), f64::from(rect.height())];

                if let Some(kind) = response.dnd_release_payload::<ShapeKind>()
                    && let Some(position) = response.ctx.pointer_latest_pos()
                {
                    app.pending.push_back(UiCommand::Drop(
                        *kind,
                        Point::new(
                            f64::from(position.x - rect.min.x),
                            f64::from(position.y - rect.min.y),
                        ),
                    ));
                }
                output.interaction = self.canvas_input.handle(&response, app.disabled, session);
            });
        output
    }

    fn show_header(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::top("editor_header")
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(248))
                    .inner_margin(Margin::symmetric(8, 6))
                    .stroke(Stroke::new(1.0, Color32::from_gray(210))),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Shapes");
                    if app.canvas.dirty {
                        ui.label(RichText::new("Modified").color(Color32::from_rgb(166, 83, 18)));
                    }
                    ui.separator();
                    ui.add(
                        TextEdit::singleline(&mut app.path_input)
                            .desired_width(240.0)
                            .hint_text("Document path"),
                    );
                    command_button(ui, "Open", !app.disabled, app, UiCommand::Open);
                    command_button(ui, "Save", !app.disabled, app, UiCommand::Save);
                    command_button(ui, "Save as", !app.disabled, app, UiCommand::SaveAs);
                    command_button(
                        ui,
                        "Discard + open",
                        app.canvas.dirty && !app.disabled,
                        app,
                        UiCommand::DiscardAndOpen,
                    );
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    command_button(ui, "Select", !app.disabled, app, UiCommand::SelectTool);
                    command_button(ui, "Marquee", !app.disabled, app, UiCommand::Marquee);
                    command_button(
                        ui,
                        "Solid link",
                        !app.disabled,
                        app,
                        UiCommand::Connect(EdgeStyle::Solid),
                    );
                    command_button(
                        ui,
                        "Dashed link",
                        !app.disabled,
                        app,
                        UiCommand::Connect(EdgeStyle::Dashed),
                    );
                    ui.separator();
                    command_button(
                        ui,
                        "Delete",
                        !app.disabled && !app.selected.is_empty(),
                        app,
                        UiCommand::Delete,
                    );
                    command_button(
                        ui,
                        "Undo",
                        app.canvas.can_undo && !app.disabled,
                        app,
                        UiCommand::Undo,
                    );
                    command_button(
                        ui,
                        "Redo",
                        app.canvas.can_redo && !app.disabled,
                        app,
                        UiCommand::Redo,
                    );
                    ui.separator();
                    if ui
                        .selectable_label(app.disabled, "Lock canvas")
                        .on_hover_text("Temporarily disable canvas editing")
                        .clicked()
                    {
                        app.disabled = !app.disabled;
                    }
                    ui.separator();
                    ui.label(format!("Tool: {}", app.canvas.active_tool));
                });
            });
    }

    fn show_status(&self, root: &mut egui::Ui, app: &AppState) {
        egui::Panel::bottom("editor_status")
            .default_size(28.0)
            .size_range(28.0..=28.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(246))
                    .inner_margin(Margin::symmetric(8, 5))
                    .stroke(Stroke::new(1.0, Color32::from_gray(210))),
            )
            .show(root, |ui| {
                let text = app
                    .canvas
                    .error
                    .as_deref()
                    .or(app.message.as_deref())
                    .unwrap_or("Ready");
                let color = if app.canvas.error.is_some() {
                    Color32::from_rgb(176, 40, 40)
                } else {
                    Color32::from_gray(78)
                };
                ui.label(RichText::new(text).color(color));
            });
    }

    fn show_palette_and_outline(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::left("palette_outline")
            .default_size(LEFT_PANEL_WIDTH)
            .size_range(180.0..=320.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(244))
                    .inner_margin(Margin::same(PANEL_MARGIN))
                    .stroke(Stroke::new(1.0, Color32::from_gray(210))),
            )
            .show(root, |ui| {
                ui.heading("Palette");
                ui.add_space(4.0);
                palette_item(
                    ui,
                    app,
                    ShapeKind::Rectangle,
                    Color32::from_rgb(70, 118, 160),
                );
                palette_item(ui, app, ShapeKind::Ellipse, Color32::from_rgb(195, 138, 34));
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.heading("Outline");
                ui.add_space(4.0);
                ScrollArea::vertical().show(ui, |ui| {
                    if app.outline.is_empty() {
                        ui.weak("No diagram items");
                    }
                    for (id, title) in app.outline.clone() {
                        let selected = app.selected.contains(&id);
                        if ui
                            .add_enabled(
                                !app.disabled,
                                egui::Button::selectable(selected, title).wrap(),
                            )
                            .clicked()
                        {
                            app.pending.push_back(UiCommand::Select(id));
                        }
                    }
                });
            });
    }

    fn show_inspector(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::right("properties")
            .default_size(RIGHT_PANEL_WIDTH)
            .size_range(200.0..=360.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(248))
                    .inner_margin(Margin::same(PANEL_MARGIN))
                    .stroke(Stroke::new(1.0, Color32::from_gray(210))),
            )
            .show(root, |ui| {
                ui.heading("Properties");
                ui.add_space(6.0);
                if let Some((id, _)) = app.property_subject {
                    ui.label(RichText::new(format!("Shape #{id}")).strong());
                    ui.add_space(8.0);
                    Grid::new("geometry_grid")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (label, value) in ["X", "Y", "Width", "Height"]
                                .into_iter()
                                .zip(&mut app.properties)
                            {
                                ui.label(label);
                                ui.add_enabled(
                                    !app.disabled,
                                    TextEdit::singleline(value).desired_width(112.0),
                                );
                                ui.end_row();
                            }
                        });
                    ui.add_space(10.0);
                    if ui
                        .add_enabled(
                            !app.disabled,
                            egui::Button::new("Apply geometry").min_size(Vec2::new(120.0, 28.0)),
                        )
                        .clicked()
                    {
                        app.pending.push_back(UiCommand::ApplyProperties);
                    }
                } else {
                    ui.weak("Select a shape to edit its geometry.");
                }
            });
    }
}

fn command_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    if ui
        .add_enabled(
            enabled,
            egui::Button::new(label).min_size(Vec2::new(TOOL_BUTTON_WIDTH, 26.0)),
        )
        .clicked()
    {
        app.pending.push_back(command);
    }
}

fn palette_item(ui: &mut egui::Ui, app: &mut AppState, kind: ShapeKind, color: Color32) {
    let title = match kind {
        ShapeKind::Rectangle => "Rectangle",
        ShapeKind::Ellipse => "Ellipse",
    };
    ui.add_enabled_ui(!app.disabled, |ui| {
        let response = ui
            .dnd_drag_source(
                ui.make_persistent_id(("palette", kind.name())),
                kind,
                |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                        match kind {
                            ShapeKind::Rectangle => {
                                ui.painter().rect_filled(rect.shrink(3.0), 1.0, color);
                            }
                            ShapeKind::Ellipse => {
                                ui.painter().circle_filled(rect.center(), 9.0, color);
                            }
                        }
                        ui.label(title);
                    });
                },
            )
            .response;
        if response.clicked() {
            app.pending.push_back(match kind {
                ShapeKind::Rectangle => UiCommand::Rectangle,
                ShapeKind::Ellipse => UiCommand::Ellipse,
            });
        }
    });
}

#[derive(Default)]
struct CanvasInput {
    captured: bool,
    pan: Option<(Point, Point)>,
}

impl CanvasInput {
    fn handle(
        &mut self,
        response: &Response,
        disabled: bool,
        session: &mut Session,
    ) -> Result<bool, String> {
        if disabled {
            if self.captured || session.domain.has_active_gesture() {
                self.cancel(session)?;
                return Ok(true);
            }
            return Ok(false);
        }

        let mut changed = false;
        let events = response.ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                Event::PointerMoved(position) => {
                    if self.captured || response.rect.contains(position) {
                        self.pointer_moved(
                            session,
                            Point::new(
                                f64::from(position.x - response.rect.min.x),
                                f64::from(position.y - response.rect.min.y),
                            ),
                        )?;
                        changed = true;
                    }
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    let inside = response.rect.contains(pos);
                    if pressed && inside {
                        response.request_focus();
                        self.pointer_pressed(
                            session,
                            Point::new(
                                f64::from(pos.x - response.rect.min.x),
                                f64::from(pos.y - response.rect.min.y),
                            ),
                            button,
                            modifiers,
                        )?;
                        changed = true;
                    } else if !pressed && (inside || self.captured) {
                        self.pointer_released(
                            session,
                            Point::new(
                                f64::from(pos.x - response.rect.min.x),
                                f64::from(pos.y - response.rect.min.y),
                            ),
                            button,
                        )?;
                        changed = true;
                    }
                }
                Event::MouseWheel { unit, delta, .. }
                    if response
                        .ctx
                        .pointer_latest_pos()
                        .is_some_and(|position| response.rect.contains(position)) =>
                {
                    let multiplier = match unit {
                        egui::MouseWheelUnit::Point => 1.0,
                        egui::MouseWheelUnit::Line => SCROLL_LINE_POINTS,
                        egui::MouseWheelUnit::Page => response.rect.height(),
                    };
                    self.scroll(
                        session,
                        Vec2::new(delta.x * multiplier, delta.y * multiplier),
                    )?;
                    changed = true;
                }
                Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } if response.has_focus() => {
                    changed |= self.key_event(session, key, pressed, repeat, modifiers)?;
                }
                Event::PointerGone | Event::WindowFocused(false) if self.captured => {
                    self.cancel(session)?;
                    changed = true;
                }
                _ => {}
            }
        }
        Ok(changed)
    }

    fn pointer_pressed(
        &mut self,
        session: &mut Session,
        point: Point,
        button: PointerButton,
        modifiers: egui::Modifiers,
    ) -> Result<(), String> {
        let Some(button) = mouse_button(button) else {
            return Ok(());
        };
        if button == MouseButton::Middle {
            self.pan = Some((
                point,
                session
                    .viewer
                    .viewport_origin()
                    .map_err(|error| error.to_string())?,
            ));
            self.captured = true;
            return Ok(());
        }
        let outcome = session
            .domain
            .pointer_pressed(&mut session.viewer, point, button, key_modifiers(modifiers))
            .map_err(|error| error.to_string())?;
        self.captured = session.domain.has_active_gesture()
            || outcome.dispatch().capture().is_some()
            || outcome.dispatch().is_handled();
        Ok(())
    }

    fn pointer_moved(&mut self, session: &mut Session, point: Point) -> Result<(), String> {
        if let Some((start, origin)) = self.pan {
            let zoom = session
                .viewer
                .viewport_scale()
                .map_err(|error| error.to_string())?;
            session
                .domain
                .set_viewport_origin(
                    &mut session.viewer,
                    Point::new(
                        origin.x() + (start.x() - point.x()) / zoom,
                        origin.y() + (start.y() - point.y()) / zoom,
                    ),
                )
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        session
            .domain
            .pointer_moved(&mut session.viewer, point)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn pointer_released(
        &mut self,
        session: &mut Session,
        point: Point,
        button: PointerButton,
    ) -> Result<(), String> {
        let Some(button) = mouse_button(button) else {
            return Ok(());
        };
        if button == MouseButton::Middle && self.pan.take().is_some() {
            self.captured = false;
            return Ok(());
        }
        session
            .domain
            .pointer_released(&mut session.viewer, point, button)
            .map_err(|error| error.to_string())?;
        self.captured = false;
        Ok(())
    }

    fn scroll(&self, session: &mut Session, delta: Vec2) -> Result<(), String> {
        let origin = session
            .viewer
            .viewport_origin()
            .map_err(|error| error.to_string())?;
        let zoom = session
            .viewer
            .viewport_scale()
            .map_err(|error| error.to_string())?;
        session
            .domain
            .set_viewport_origin(
                &mut session.viewer,
                Point::new(
                    origin.x() - f64::from(delta.x) / zoom,
                    origin.y() - f64::from(delta.y) / zoom,
                ),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn key_event(
        &mut self,
        session: &mut Session,
        key: Key,
        pressed: bool,
        repeat: bool,
        modifiers: egui::Modifiers,
    ) -> Result<bool, String> {
        if pressed && !repeat {
            if key == Key::Escape {
                self.cancel(session)?;
                return Ok(true);
            }
            if matches!(key, Key::Delete | Key::Backspace) {
                let mut app = AppState::default();
                app.pending.push_back(UiCommand::Delete);
                crate::sync(&mut app, session)?;
                return Ok(true);
            }
            if modifiers.command {
                let command = match key {
                    Key::Z if modifiers.shift => Some(UiCommand::Redo),
                    Key::Z => Some(UiCommand::Undo),
                    Key::Y => Some(UiCommand::Redo),
                    _ => None,
                };
                if let Some(command) = command {
                    let mut app = AppState::default();
                    app.pending.push_back(command);
                    crate::sync(&mut app, session)?;
                    return Ok(true);
                }
            }
        }
        let Some(key) = editor_key(key) else {
            return Ok(false);
        };
        let directive = session
            .domain
            .key_event(
                &mut session.viewer,
                KeyEvent {
                    kind: if pressed {
                        KeyEventKind::Pressed
                    } else {
                        KeyEventKind::Released
                    },
                    key,
                    modifiers: key_modifiers(modifiers),
                },
            )
            .map_err(|error| error.to_string())?;
        if !matches!(directive, novadraw_editor::tool::ToolDirective::Continue) {
            self.captured = false;
        }
        Ok(!matches!(
            directive,
            novadraw_editor::tool::ToolDirective::Continue
        ))
    }

    fn cancel(&mut self, session: &mut Session) -> Result<(), String> {
        self.captured = false;
        self.pan = None;
        session.cancel()
    }
}

fn mouse_button(button: PointerButton) -> Option<MouseButton> {
    match button {
        PointerButton::Primary => Some(MouseButton::Left),
        PointerButton::Secondary => Some(MouseButton::Right),
        PointerButton::Middle => Some(MouseButton::Middle),
        PointerButton::Extra1 | PointerButton::Extra2 => None,
    }
}

fn key_modifiers(modifiers: egui::Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: modifiers.shift,
        control: modifiers.ctrl,
        alt: modifiers.alt,
        meta: modifiers.mac_cmd,
    }
}

fn editor_key(key: Key) -> Option<EditorKey> {
    match key {
        Key::Enter => Some(EditorKey::Enter),
        Key::Tab => Some(EditorKey::Tab),
        Key::ArrowUp => Some(EditorKey::ArrowUp),
        Key::ArrowDown => Some(EditorKey::ArrowDown),
        Key::ArrowLeft => Some(EditorKey::ArrowLeft),
        Key::ArrowRight => Some(EditorKey::ArrowRight),
        _ => None,
    }
}
