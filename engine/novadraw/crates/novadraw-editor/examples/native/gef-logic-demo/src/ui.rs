use crate::{
    model::{Circuit, Id, Kind, Routing},
    presets::Preset,
    session::{Mode, Session},
};
use egui::{Color32, Event, PointerButton, Response, TextureId};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::Point;
use novadraw_example_support::workbench::{self, icons};

const SIGNAL: Color32 = Color32::from_rgb(32, 143, 107);
const SIDEBAR_WIDTH: f32 = workbench::SIDEBAR_WIDTH;
const CONTROL_HEIGHT: f32 = workbench::CONTROL_HEIGHT;
const SCROLL_LINE: f32 = 24.0;
const CONTENT_PADDING: f64 = 40.0;

pub struct Ui {
    pub error: Option<String>,
    pub path: String,
    captured: bool,
    pan: Option<(Point, Point)>,
    preset: Preset,
    pending_preset: Option<Preset>,
    canvas_size: [f64; 2],
    initial_fit_pending: bool,
}
impl Default for Ui {
    fn default() -> Self {
        Self {
            error: None,
            path: concat!(env!("CARGO_MANIFEST_DIR"), "/logic.json").into(),
            captured: false,
            pan: None,
            preset: Preset::default(),
            pending_preset: None,
            canvas_size: [880.0, 600.0],
            initial_fit_pending: true,
        }
    }
}
impl Ui {
    fn load_preset(&mut self, session: &mut Session, preset: Preset) -> Result<(), String> {
        let mut next = Session::new(preset.circuit())?;
        self.fit_canvas(&mut next)?;
        next.sync()?;
        *session = next;
        self.preset = preset;
        self.pending_preset = None;
        self.captured = false;
        self.pan = None;
        Ok(())
    }
    fn fit_canvas(&self, session: &mut Session) -> Result<(), String> {
        let (width, height) =
            session
                .viewer
                .model()
                .document
                .nodes
                .values()
                .fold((0.0_f64, 0.0_f64), |(w, h), n| {
                    let b = n.bounds();
                    (
                        w.max(b.x + b.width + CONTENT_PADDING),
                        h.max(b.y + b.height + CONTENT_PADDING),
                    )
                });
        let scale = (self.canvas_size[0] / width)
            .min(self.canvas_size[1] / height)
            .clamp(0.1, 1.0);
        session
            .domain
            .set_viewport_scale_at(&mut session.viewer, scale, Some(Point::ZERO))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
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
        workbench::titlebar(root, "document", |ui| {
            workbench::document_label(ui, "Logic", session.domain.is_dirty());
            ui.add_space(16.0);
            ui.menu_button("File", |ui| {
                workbench::section(ui, "Document location");
                ui.add_sized(
                    [280.0, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut self.path),
                );
                ui.add_space(8.0);
                if ui
                    .button(format!("{}  Save circuit", icons::FLOPPY_DISK))
                    .clicked()
                {
                    self.error = None;
                    self.apply(save(session, &self.path));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !session.domain.is_dirty(),
                        egui::Button::new(format!("{}  Open circuit", icons::FOLDER_OPEN)),
                    )
                    .on_hover_text("Save changes before opening another circuit")
                    .clicked()
                {
                    self.error = None;
                    self.apply(open(session, &self.path));
                    ui.close();
                }
            });
            ui.separator();
            egui::ComboBox::from_id_salt("logic-preset")
                .selected_text(self.preset.name())
                .show_ui(ui, |ui| {
                    for preset in Preset::ALL {
                        if ui
                            .selectable_label(self.preset == preset, preset.name())
                            .clicked()
                        {
                            if session.domain.is_dirty() {
                                self.pending_preset = Some(preset);
                            } else {
                                let result = self.load_preset(session, preset);
                                self.apply(result);
                            }
                        }
                    }
                });
            ui.separator();
            if workbench::icon_button(
                ui,
                icons::ARROW_COUNTER_CLOCKWISE,
                "Undo · ⌘Z",
                session.domain.has_undo(),
                false,
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
            if workbench::icon_button(
                ui,
                icons::ARROW_CLOCKWISE,
                "Redo · ⇧⌘Z",
                session.domain.has_redo(),
                false,
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
            if workbench::icon_button(
                ui,
                icons::TRASH,
                "Delete selection",
                !session.viewer.selection().items().is_empty(),
                false,
            )
            .clicked()
            {
                self.apply(session.delete());
            }
        });
        if let Some(preset) = self.pending_preset {
            egui::Window::new("Load preset?")
                .collapsible(false)
                .resizable(false)
                .show(root.ctx(), |ui| {
                    ui.label("The current circuit has unsaved changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.pending_preset = None;
                        }
                        if ui.button("Discard and load").clicked() {
                            let result = self.load_preset(session, preset);
                            self.apply(result);
                        }
                    });
                });
        }
        egui::Panel::bottom("status")
            .exact_size(workbench::STATUS_HEIGHT)
            .frame(
                workbench::panel_frame(workbench::CHROME)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(root, |ui| {
                ui.spacing_mut().interact_size.y = 24.0;
                ui.horizontal(|ui| {
                    if let Some(error) = &self.error {
                        ui.colored_label(Color32::from_rgb(180, 52, 52), error);
                    } else {
                        ui.label(match session.mode {
                            Mode::Select => {
                                "Drag nodes · select wires to reconnect · middle drag to pan"
                            }
                            Mode::Connect => "Click an output, then an unused input · Esc cancels",
                            Mode::Create(_) => {
                                "Click the canvas to place a component · Esc cancels"
                            }
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(icons::CORNERS_OUT)
                            .on_hover_text("Fit circuit to canvas")
                            .clicked()
                        {
                            let result = self.fit_canvas(session);
                            self.apply(result);
                        }
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
        egui::Panel::left("logic-tool-rail")
            .exact_size(workbench::TOOL_RAIL_WIDTH)
            .resizable(false)
            .frame(workbench::panel_frame(workbench::CHROME))
            .show(root, |ui| {
                ui.vertical_centered(|ui| {
                    for (icon, label, mode) in [
                        (icons::CURSOR, "Select / move", Mode::Select),
                        (icons::FLOW_ARROW, "Connect components", Mode::Connect),
                    ] {
                        if workbench::icon_button(ui, icon, label, true, session.mode == mode)
                            .clicked()
                        {
                            self.apply(session.set_mode(mode));
                        }
                    }
                });
            });
        egui::Panel::left("logic-library")
            .default_size(SIDEBAR_WIDTH)
            .size_range(208.0..=320.0)
            .resizable(true)
            .frame(workbench::panel_frame(workbench::SIDEBAR))
            .show(root, |ui| {
                workbench::panel_heading(ui, icons::SQUARES_FOUR, "Library");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    workbench::section(ui, "Components");
                    for kind in Kind::ALL {
                        if ui
                            .add_sized(
                                [ui.available_width(), CONTROL_HEIGHT],
                                egui::Button::new(format!("{}   {}", icons::CPU, kind.name()))
                                    .selected(session.mode == Mode::Create(kind)),
                            )
                            .clicked()
                        {
                            self.apply(session.set_mode(Mode::Create(kind)));
                        }
                    }
                    workbench::section(ui, "Circuit layers");
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
                                .add_sized(
                                    [ui.available_width(), CONTROL_HEIGHT],
                                    egui::Button::new(format!("{}   {name}", icons::STACK))
                                        .frame(false)
                                        .selected(
                                            session.viewer.selection().items().contains(&part),
                                        ),
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
            });
        egui::Panel::right("logic-inspector")
            .default_size(workbench::INSPECTOR_WIDTH)
            .size_range(240.0..=360.0)
            .resizable(true)
            .frame(workbench::panel_frame(workbench::SURFACE))
            .show(root, |ui| {
                workbench::panel_heading(ui, icons::SLIDERS_HORIZONTAL, "Inspector");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    workbench::section(ui, "Connection routing");
                    for routing in Routing::ALL {
                        if ui
                            .selectable_label(
                                session.viewer.model().document.routing == routing,
                                routing.name(),
                            )
                            .clicked()
                        {
                            self.apply(session.set_routing(routing));
                        }
                    }
                    if session.viewer.model().document.routing == Routing::Manual {
                        ui.label(
                            egui::RichText::new("Drag wire handles to add / move bends")
                                .small()
                                .color(Color32::GRAY),
                        );
                        let selected = session.viewer.selection().items().iter().find_map(|p| {
                            session
                                .viewer
                                .parts()
                                .get(*p)
                                .and_then(|p| p.model_id())
                                .and_then(|id| {
                                    if let Id::Wire(id) = id {
                                        Some(id)
                                    } else {
                                        None
                                    }
                                })
                        });
                        if let Some(id) = selected {
                            let count = session
                                .viewer
                                .model()
                                .document
                                .bendpoints
                                .get(&id)
                                .map_or(0, Vec::len);
                            for index in 0..count {
                                if ui
                                    .small_button(format!("Delete bend {}", index + 1))
                                    .clicked()
                                {
                                    self.apply(session.delete_bendpoint(index));
                                    break;
                                }
                            }
                        }
                    }
                    workbench::section(ui, "Input values");
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
                    workbench::section(ui, "Live signals");
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.colored_label(
                                        if value != 0 { SIGNAL } else { Color32::GRAY },
                                        format!("{value:04b} · {value}"),
                                    );
                                },
                            );
                        });
                    }
                });
            });
        let mut size = [1.0, 1.0];
        egui::CentralPanel::default()
            .frame(workbench::panel_frame(workbench::PASTEBOARD))
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
        self.canvas_size = size;
        if self.initial_fit_pending {
            self.initial_fit_pending = false;
            let result = self.fit_canvas(session);
            self.apply(result);
            root.ctx().request_repaint();
        }
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
    workbench::configure_style(context);
}
