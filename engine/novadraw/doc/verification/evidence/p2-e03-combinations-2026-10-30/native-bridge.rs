use editor_container_input::InputProbe;
use eframe::egui;

struct App {
    probe: InputProbe,
    error: Option<String>,
    captured: bool,
    input_position: Option<[f64; 2]>,
}

impl App {
    fn record(&mut self, operation: &str, result: Result<(), String>) {
        self.error = result.err();
        match self.probe.snapshot() {
            Ok(snapshot) => println!(
                "E03_NATIVE {}",
                serde_json::json!({
                    "operation": operation,
                    "error": self.error,
                    "input_position": self.input_position,
                    "state": snapshot,
                })
            ),
            Err(error) => self.error = Some(error),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        self.input_position = None;
        egui::Panel::top("controls").show(root, |ui| {
            ui.horizontal(|ui| {
                for (label, creating) in [("Reset move", false), ("Reset create", true)] {
                    if ui.button(label).clicked() {
                        self.probe = InputProbe::new(creating);
                        self.captured = false;
                        self.record(label, Ok(()));
                    }
                }
                if ui.button("Undo").clicked() {
                    let r = self.probe.undo();
                    self.record("undo", r);
                }
                if ui.button("Redo").clicked() {
                    let r = self.probe.redo();
                    self.record("redo", r);
                }
                if ui.button("Zoom").clicked() {
                    let r = self.probe.zoom(1.25);
                    self.record("zoom", r);
                }
                if ui.button("Scroll").clicked() {
                    let r = self.probe.scroll(20.0, 15.0);
                    self.record("scroll", r);
                }
                ui.label("Editor E03 input probe · Esc cancels");
            });
        });
        egui::CentralPanel::default().show(root, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            let r = self
                .probe
                .resize(f64::from(rect.width()), f64::from(rect.height()));
            if let Err(error) = r {
                self.error = Some(error);
            }
            if let Ok(snapshot) = self.probe.snapshot() {
                for visual in snapshot.visuals {
                    if visual.model == 1 {
                        continue;
                    }
                    let [x, y, w, h] = visual.bounds;
                    let b = egui::Rect::from_min_size(
                        rect.min + egui::vec2(x as f32, y as f32),
                        egui::vec2(w as f32, h as f32),
                    );
                    let container = matches!(visual.model, 2 | 3 | 6);
                    ui.painter().rect_stroke(
                        b,
                        0.0,
                        egui::Stroke::new(
                            2.0,
                            if container {
                                egui::Color32::GRAY
                            } else {
                                egui::Color32::BLUE
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        b.left_top(),
                        egui::Align2::LEFT_TOP,
                        visual.model.to_string(),
                        egui::FontId::monospace(12.0),
                        egui::Color32::BLACK,
                    );
                }
                ui.painter().text(
                    rect.min + egui::vec2(8.0, 420.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "active={} history={} builds={} feedback={} DPI={}",
                        snapshot.active,
                        snapshot.history,
                        snapshot.builds,
                        snapshot.feedback,
                        ui.ctx().pixels_per_point()
                    ),
                    egui::FontId::monospace(14.0),
                    egui::Color32::BLACK,
                );
            }
            let local =
                |pos: egui::Pos2| (f64::from(pos.x - rect.min.x), f64::from(pos.y - rect.min.y));
            let dropped = ui.ctx().input(|i| i.raw.dropped_files.clone());
            if !dropped.is_empty() {
                let position = ui.ctx().input(|i| i.pointer.latest_pos());
                for file in dropped {
                    if file.path.as_ref().and_then(|path| path.file_name())
                        == Some(std::ffi::OsStr::new("e03-template.txt"))
                        && let Some(position) = position
                        && rect.contains(position)
                    {
                        let (x, y) = local(position);
                        self.input_position = Some([x, y]);
                        let r = self.probe.drop_template(x, y);
                        self.record("system-drop", r);
                    } else {
                        self.record("unsupported-drop", Ok(()));
                    }
                }
            }
            for event in ui.ctx().input(|i| i.events.clone()) {
                match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        ..
                    } if rect.contains(pos) || self.captured => {
                        let (x, y) = local(pos);
                        self.input_position = Some([x, y]);
                        if pressed {
                            response.request_focus();
                            self.captured = true;
                            let r = self.probe.press(x, y);
                            self.record("press", r);
                        } else {
                            let r = self.probe.release(x, y);
                            self.record("release", r);
                            self.captured = false;
                        }
                    }
                    egui::Event::PointerMoved(pos) if self.captured => {
                        let (x, y) = local(pos);
                        self.input_position = Some([x, y]);
                        let r = self.probe.move_pointer(x, y);
                        self.record("move", r);
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        ..
                    } => {
                        let operation = match key {
                            egui::Key::M | egui::Key::C if !self.captured => {
                                self.probe = InputProbe::new(key == egui::Key::C);
                                Some(("reset", Ok(())))
                            }
                            egui::Key::U if !self.captured => Some(("undo", self.probe.undo())),
                            egui::Key::R if !self.captured => Some(("redo", self.probe.redo())),
                            egui::Key::N => {
                                self.probe.select_batch();
                                Some(("batch", Ok(())))
                            }
                            egui::Key::O => {
                                self.probe.ordered_target();
                                Some(("ordered", Ok(())))
                            }
                            egui::Key::Z => Some(("zoom", self.probe.zoom(1.25))),
                            egui::Key::S => Some(("scroll", self.probe.scroll(20.0, 15.0))),
                            egui::Key::E => Some(("external", self.probe.external_revision())),
                            egui::Key::I => Some(("invalidate", self.probe.invalidate_target())),
                            egui::Key::T => Some(("switch", self.probe.switch_tool())),
                            egui::Key::Escape if self.captured => {
                                self.captured = false;
                                Some(("cancel", self.probe.cancel()))
                            }
                            _ => None,
                        };
                        if let Some((name, result)) = operation {
                            self.record(name, result);
                        }
                    }
                    egui::Event::WindowFocused(false) if self.captured => {
                        let r = self.probe.cancel();
                        self.record("cancel", r);
                        self.captured = false;
                    }
                    _ => {}
                }
            }
            if let Some(error) = &self.error {
                ui.label(error);
            }
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Novadraw E03 input verification",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([860.0, 640.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|context| {
            context.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(App {
                probe: InputProbe::new(false),
                error: None,
                captured: false,
                input_position: None,
            }))
        }),
    )
}
