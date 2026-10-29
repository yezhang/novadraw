use crate::{
    model::{Circuit, Id, Kind},
    session::{Mode, Session},
};
use egui::{Color32, Event, PointerButton, Response, TextureId};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::Point;

const ACCENT: Color32 = Color32::from_rgb(58, 110, 184);
const SIGNAL: Color32 = Color32::from_rgb(32, 143, 107);
const SIDEBAR_WIDTH: f32 = 204.0;
const CONTROL_HEIGHT: f32 = 26.0;
const SCROLL_LINE: f32 = 24.0;

pub struct Ui {
    pub error: Option<String>,
    pub path: String,
    captured: bool,
    pan: Option<(Point, Point)>,
}
impl Default for Ui {
    fn default() -> Self {
        Self {
            error: None,
            path: "logic.json".into(),
            captured: false,
            pan: None,
        }
    }
}
impl Ui {
    fn apply(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
    pub fn show(
        &mut self,
        root: &mut egui::Ui,
        session: &mut Session,
        texture: TextureId,
    ) -> [f64; 2] {
        egui::Panel::top("document").show(root, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new("Logic").strong().size(18.0));
                ui.label(egui::RichText::new("Half-adder").color(Color32::GRAY));
                ui.separator();
                if ui
                    .add_enabled(
                        session.domain.has_undo(),
                        egui::Button::new("Undo").min_size(egui::vec2(50.0, CONTROL_HEIGHT)),
                    )
                    .clicked()
                {
                    self.apply(
                        session
                            .domain
                            .undo(&mut session.viewer)
                            .map_err(|e| e.to_string()),
                    );
                }
                if ui
                    .add_enabled(
                        session.domain.has_redo(),
                        egui::Button::new("Redo").min_size(egui::vec2(50.0, CONTROL_HEIGHT)),
                    )
                    .clicked()
                {
                    self.apply(
                        session
                            .domain
                            .redo(&mut session.viewer)
                            .map_err(|e| e.to_string()),
                    );
                }
                ui.separator();
                ui.add_sized(
                    [150.0, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut self.path),
                );
                if ui
                    .add_sized([48.0, CONTROL_HEIGHT], egui::Button::new("Save"))
                    .clicked()
                {
                    self.error = None;
                    self.apply(save(session, &self.path));
                }
                if ui
                    .add_enabled(
                        !session.domain.is_dirty(),
                        egui::Button::new("Open").min_size(egui::vec2(48.0, CONTROL_HEIGHT)),
                    )
                    .on_hover_text("Save changes before opening another circuit")
                    .clicked()
                {
                    self.error = None;
                    self.apply(open(session, &self.path));
                }
                if ui.button("Delete").clicked() {
                    self.apply(session.delete());
                }
            });
        });
        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(180, 52, 52), error);
                } else {
                    ui.label(match session.mode {
                        Mode::Select => {
                            "Drag nodes · select wires to reconnect · middle drag to pan"
                        }
                        Mode::Connect => "Click an output, then an unused input · Esc cancels",
                        Mode::Create(_) => "Click the canvas to place a component · Esc cancels",
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let zoom = session.viewer.viewport_scale().unwrap_or(1.0);
                    ui.label(format!("{:.0}%", zoom * 100.0));
                    for (label, factor) in [("+", 1.25), ("−", 0.8)] {
                        if ui.small_button(label).clicked() {
                            self.apply(
                                session
                                    .domain
                                    .set_viewport_scale_at(
                                        &mut session.viewer,
                                        (zoom * factor).clamp(0.25, 4.0),
                                        None,
                                    )
                                    .map(|_| ())
                                    .map_err(|e| e.to_string()),
                            );
                        }
                    }
                    ui.label(if session.domain.is_dirty() {
                        "Modified"
                    } else {
                        "Saved"
                    });
                });
            });
        });
        egui::Panel::left("tools")
            .exact_size(SIDEBAR_WIDTH)
            .resizable(false)
            .show(root, |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("TOOLS").small().color(Color32::GRAY));
                for (name, mode) in [("Select / move", Mode::Select), ("Wire", Mode::Connect)] {
                    if ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(name).selected(session.mode == mode),
                        )
                        .clicked()
                    {
                        self.apply(session.set_mode(mode));
                    }
                }
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new("COMPONENTS")
                        .small()
                        .color(Color32::GRAY),
                );
                egui::Grid::new("components")
                    .num_columns(2)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for (i, kind) in Kind::ALL.into_iter().enumerate() {
                            if ui
                                .add_sized(
                                    [88.0, 34.0],
                                    egui::Button::new(kind.name())
                                        .selected(session.mode == Mode::Create(kind)),
                                )
                                .clicked()
                            {
                                self.apply(session.set_mode(Mode::Create(kind)));
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("INPUT VALUES")
                        .small()
                        .color(Color32::GRAY),
                );
                let inputs = session
                    .viewer
                    .model()
                    .document
                    .nodes
                    .iter()
                    .filter(|(id, node)| {
                        node.kind == Kind::Led && !session.viewer.model().driven(**id)
                    })
                    .map(|(id, node)| (*id, node.name.clone(), node.value))
                    .collect::<Vec<_>>();
                for (id, name, value) in inputs {
                    ui.label(egui::RichText::new(name).strong());
                    ui.horizontal(|ui| {
                        let mut next = value;
                        for bit in (0..4).rev() {
                            let on = value & (1 << bit) != 0;
                            if ui
                                .add_sized(
                                    [33.0, 28.0],
                                    egui::Button::new(if on { "1" } else { "0" }).selected(on),
                                )
                                .on_hover_text(format!("Toggle bit {bit}"))
                                .clicked()
                            {
                                next ^= 1 << bit;
                            }
                        }
                        ui.label(value.to_string());
                        if next != value {
                            self.apply(session.set_input(id, next));
                        }
                    });
                    ui.add_space(6.0);
                }
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("LIVE SIGNALS")
                        .small()
                        .color(Color32::GRAY),
                );
                let outputs = session
                    .viewer
                    .model()
                    .document
                    .nodes
                    .iter()
                    .filter(|(id, node)| {
                        node.kind == Kind::Led && session.viewer.model().driven(**id)
                    })
                    .map(|(id, node)| (node.name.clone(), session.viewer.model().signals[id]))
                    .collect::<Vec<_>>();
                for (name, value) in outputs {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.colored_label(
                                if value != 0 { SIGNAL } else { Color32::GRAY },
                                format!("{value:04b} · {value}"),
                            );
                        });
                    });
                }
                ui.add_space(20.0);
                ui.label(egui::RichText::new("CIRCUIT").small().color(Color32::GRAY));
                let nodes = session
                    .viewer
                    .model()
                    .document
                    .order
                    .iter()
                    .map(|id| (*id, session.viewer.model().document.nodes[id].name.clone()))
                    .collect::<Vec<_>>();
                for (id, name) in nodes {
                    if let Some(part) = session.viewer.part_for_model(Id::Node(id))
                        && ui
                            .selectable_label(
                                session.viewer.selection().items().contains(&part),
                                name,
                            )
                            .clicked()
                    {
                        self.apply(
                            session
                                .viewer
                                .replace_selection(part)
                                .map(|_| ())
                                .map_err(|e| e.to_string()),
                        );
                    }
                }
            });
        let mut size = [1.0, 1.0];
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(247, 249, 251)))
            .show(root, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                size = [f64::from(rect.width()), f64::from(rect.height())];
                ui.painter().image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                self.apply_input(&response, session);
            });
        self.apply(session.sync());
        size
    }
    fn apply_input(&mut self, response: &Response, session: &mut Session) {
        let result = self.input(response, session);
        self.apply(result);
    }
    fn input(&mut self, response: &Response, session: &mut Session) -> Result<(), String> {
        let events = response.ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } if response.rect.contains(pos) || self.captured => {
                    let point = local(response, pos);
                    let Some(button) = mouse_button(button) else {
                        continue;
                    };
                    if pressed {
                        response.request_focus();
                        if button == MouseButton::Middle {
                            self.pan = Some((
                                point,
                                session
                                    .viewer
                                    .viewport_origin()
                                    .map_err(|e| e.to_string())?,
                            ));
                        } else {
                            session
                                .domain
                                .pointer_pressed(
                                    &mut session.viewer,
                                    point,
                                    button,
                                    key_modifiers(modifiers),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        self.captured = true;
                    } else {
                        if button == MouseButton::Middle {
                            self.pan = None;
                        } else {
                            session
                                .domain
                                .pointer_released(&mut session.viewer, point, button)
                                .map_err(|e| e.to_string())?;
                        }
                        self.captured = false;
                    }
                }
                Event::PointerMoved(pos) if response.rect.contains(pos) || self.captured => {
                    let point = local(response, pos);
                    if let Some((start, origin)) = self.pan {
                        let zoom = session.viewer.viewport_scale().map_err(|e| e.to_string())?;
                        session
                            .domain
                            .set_viewport_origin(
                                &mut session.viewer,
                                Point::new(
                                    origin.x() + (start.x() - point.x()) / zoom,
                                    origin.y() + (start.y() - point.y()) / zoom,
                                ),
                            )
                            .map_err(|e| e.to_string())?;
                    } else {
                        session
                            .domain
                            .pointer_moved(&mut session.viewer, point)
                            .map_err(|e| e.to_string())?;
                    }
                }
                Event::MouseWheel { unit, delta, .. } if response.hovered() => {
                    let origin = session
                        .viewer
                        .viewport_origin()
                        .map_err(|e| e.to_string())?;
                    let zoom = session.viewer.viewport_scale().map_err(|e| e.to_string())?;
                    let multiplier = match unit {
                        egui::MouseWheelUnit::Point => 1.0,
                        egui::MouseWheelUnit::Line => SCROLL_LINE,
                        egui::MouseWheelUnit::Page => response.rect.height(),
                    };
                    session
                        .domain
                        .set_viewport_origin(
                            &mut session.viewer,
                            Point::new(
                                origin.x() - f64::from(delta.x * multiplier) / zoom,
                                origin.y() - f64::from(delta.y * multiplier) / zoom,
                            ),
                        )
                        .map_err(|e| e.to_string())?;
                }
                Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if response.has_focus() => match key {
                    egui::Key::Escape => {
                        session.cancel()?;
                        self.captured = false;
                        self.pan = None;
                    }
                    egui::Key::Delete | egui::Key::Backspace => session.delete()?,
                    egui::Key::Z if modifiers.command && modifiers.shift => session
                        .domain
                        .redo(&mut session.viewer)
                        .map_err(|e| e.to_string())?,
                    egui::Key::Z if modifiers.command => session
                        .domain
                        .undo(&mut session.viewer)
                        .map_err(|e| e.to_string())?,
                    egui::Key::Y if modifiers.command => session
                        .domain
                        .redo(&mut session.viewer)
                        .map_err(|e| e.to_string())?,
                    _ => {}
                },
                Event::WindowFocused(false) | Event::PointerGone
                    if self.captured || session.domain.has_active_gesture() =>
                {
                    session.cancel()?;
                    self.captured = false;
                    self.pan = None;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
fn local(response: &Response, pos: egui::Pos2) -> Point {
    Point::new(
        f64::from(pos.x - response.rect.min.x),
        f64::from(pos.y - response.rect.min.y),
    )
}
fn mouse_button(button: PointerButton) -> Option<MouseButton> {
    match button {
        PointerButton::Primary => Some(MouseButton::Left),
        PointerButton::Secondary => Some(MouseButton::Right),
        PointerButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}
fn key_modifiers(m: egui::Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: m.shift,
        control: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd,
    }
}

fn save(session: &mut Session, path: &str) -> Result<(), String> {
    use std::io::Write;
    let path = std::path::Path::new(path);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let name = path
        .file_name()
        .ok_or("Choose a circuit file")?
        .to_string_lossy();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".{name}.{}.{nonce}.tmp", std::process::id()));
    let data = session.viewer.model().to_json()?;
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())?;
    session
        .domain
        .mark_save_location()
        .map_err(|e| e.to_string())
}
fn open(session: &mut Session, path: &str) -> Result<(), String> {
    if session.domain.is_dirty() {
        return Err("Save changes before opening another circuit".into());
    }
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let replacement = Session::new(Circuit::from_json(&data)?)?;
    session.cancel()?;
    *session = replacement;
    Ok(())
}
pub fn configure_style(context: &egui::Context) {
    context.set_visuals(egui::Visuals::light());
    context.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.item_spacing = egui::vec2(6.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 5.0);
        style.visuals.selection.bg_fill = Color32::from_rgb(229, 239, 252);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
        style.visuals.widgets.hovered.bg_stroke = style.visuals.widgets.inactive.bg_stroke;
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(239);
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
        style.animation_time = 0.0;
    });
}
