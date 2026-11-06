use gef_editor_demo::model::{Diagram, ShapeFactory, ShapeKind, ShapeTemplate, WorkspaceSettings};
use novadraw::geometry::Rectangle;
use novadraw_editor::{
    GraphicalViewer,
    tool::{CreationTool, MarqueeTool},
};
use novadraw_ui::{
    docking::{DockArea, DockContent, DockStyle, Layout, Node, Permissions, Workspace},
    editor_canvas::EditorCanvas,
    layout::{SplitAxis, SplitSize},
};

const WINDOW_SIZE: [f32; 2] = [1280.0, 800.0];
const MIN_WINDOW_SIZE: [f32; 2] = [800.0, 500.0];
const SIDE_WIDTH: f32 = 240.0;
const TOOL_WIDTH: f32 = 100.0;
const CAPTURE_SETTLE_FRAMES: usize = 5;
const CAPTURE_MAX_BYTES: usize = 64 * 1024 * 1024;
const CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

struct SmokeCapture {
    path: std::path::PathBuf,
    frames: usize,
    deadline: std::time::Instant,
    slot: Option<novadraw_ui::capture::CaptureSlot>,
}

impl SmokeCapture {
    fn from_args() -> Option<Self> {
        let mut args = std::env::args().skip(1);
        if args.next().as_deref() != Some("--smoke-screenshot") {
            return None;
        }
        Some(Self {
            path: args.next()?.into(),
            frames: 0,
            deadline: std::time::Instant::now() + CAPTURE_TIMEOUT,
            slot: None,
        })
    }

    fn step(&mut self, context: &egui::Context) -> Result<bool, String> {
        self.frames += 1;
        if self.frames == CAPTURE_SETTLE_FRAMES {
            let (slot, callback) = novadraw_ui::capture::CaptureSlot::new(
                context,
                egui::ViewportId::ROOT,
                CAPTURE_MAX_BYTES,
            );
            self.slot = Some(slot);
            context.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
        }
        if let Some(result) = self.slot.as_ref().and_then(|slot| slot.take()) {
            let image = result.map_err(|e| e.to_string())?;
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            image::save_buffer(
                &self.path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            return Ok(true);
        }
        if std::time::Instant::now() >= self.deadline {
            return Err("Native screenshot timed out".into());
        }
        context.request_repaint();
        Ok(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum Panel {
    Tools,
    Canvas,
    Layers,
    Inspector,
    Notes,
}

impl Panel {
    fn label(self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Canvas => "Untitled · Canvas",
            Self::Layers => "Layers",
            Self::Inspector => "Inspector",
            Self::Notes => "Notes",
        }
    }

    fn permissions(self) -> Permissions {
        if self == Self::Canvas {
            Permissions::PROTECTED
        } else {
            Permissions::default()
        }
    }
}

fn default_layout() -> Layout<Panel> {
    let tabs = |panels: Vec<Panel>| Node::Tabs { panels, active: 0 };
    Layout {
        root: Some(Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::FixedFirst(TOOL_WIDTH),
            first: Box::new(tabs(vec![Panel::Tools])),
            second: Box::new(Node::Split {
                axis: SplitAxis::Horizontal,
                size: SplitSize::FixedSecond(SIDE_WIDTH),
                first: Box::new(tabs(vec![Panel::Canvas])),
                second: Box::new(Node::Split {
                    axis: SplitAxis::Vertical,
                    size: SplitSize::Ratio(0.45),
                    first: Box::new(tabs(vec![Panel::Layers])),
                    second: Box::new(tabs(vec![Panel::Inspector, Panel::Notes])),
                }),
            }),
        }),
        floating: Vec::new(),
    }
}

struct Content {
    canvas: EditorCanvas<Diagram, ShapeFactory>,
    canvas_size: egui::Vec2,
    notes: String,
    selected_tool: &'static str,
    error: Option<String>,
    canvas_shown: bool,
}

impl DockContent<Panel> for Content {
    fn logical_label(&self, panel: &Panel) -> String {
        panel.label().into()
    }

    fn permissions(&self, panel: &Panel) -> Permissions {
        panel.permissions()
    }

    fn body(&mut self, ui: &mut egui::Ui, panel: &Panel) {
        match panel {
            Panel::Canvas => {
                self.canvas_shown = true;
                let output = self.canvas.show(ui);
                self.canvas_size = output.response.rect.size();
                if output.error.is_some() {
                    self.error = output.error;
                }
            }
            Panel::Tools => {
                for tool in ["Select", "Marquee", "Rectangle", "Ellipse"] {
                    if ui
                        .selectable_value(&mut self.selected_tool, tool, tool)
                        .clicked()
                    {
                        let result = match tool {
                            "Select" => self.canvas.domain.cancel_tool(&mut self.canvas.viewer),
                            "Marquee" => self
                                .canvas
                                .domain
                                .set_active_tool(&mut self.canvas.viewer, MarqueeTool::new()),
                            _ => self.canvas.domain.set_active_tool(
                                &mut self.canvas.viewer,
                                CreationTool::new(ShapeTemplate(if tool == "Rectangle" {
                                    ShapeKind::Rectangle
                                } else {
                                    ShapeKind::Ellipse
                                })),
                            ),
                        };
                        if let Err(error) = result {
                            self.error = Some(error.to_string());
                        }
                    }
                }
            }
            Panel::Layers => {
                ui.strong("Document");
                let model = self.canvas.viewer.model();
                let layers: Vec<_> = model
                    .order
                    .iter()
                    .map(|id| (*id, model.name(*id)))
                    .collect();
                for (id, name) in layers {
                    if let Some(part) = self.canvas.viewer.part_for_model(id) {
                        let selected = self.canvas.viewer.selection().items().contains(&part);
                        if ui.selectable_label(selected, name).clicked()
                            && let Err(error) = self.canvas.viewer.replace_selection(part)
                        {
                            self.error = Some(error.to_string());
                        }
                    }
                }
            }
            Panel::Inspector => {
                ui.strong("Canvas");
                ui.label(format!(
                    "{:.0} × {:.0} pt",
                    self.canvas_size.x, self.canvas_size.y
                ));
                ui.separator();
                ui.label("Drag panel headers to dock, split or float.");
                ui.label("The document canvas stays open.");
                ui.label(format!(
                    "Selected: {}",
                    self.canvas.viewer.selection().items().len()
                ));
                if let Ok(zoom) = self.canvas.viewer.viewport_scale() {
                    ui.label(format!("Zoom: {:.0}%", zoom * 100.0));
                }
            }
            Panel::Notes => {
                ui.add(egui::TextEdit::multiline(&mut self.notes).desired_width(f32::INFINITY));
            }
        }
    }
}

struct App {
    workspace: Workspace<Panel>,
    saved: Option<String>,
    error: Option<String>,
    content: Content,
    smoke: Option<SmokeCapture>,
}

impl App {
    fn new(context: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let gpu = context
            .wgpu_render_state
            .clone()
            .ok_or("WGPU renderer unavailable")?;
        let viewer = GraphicalViewer::new(
            Diagram::default(),
            ShapeFactory::new(WorkspaceSettings::default()),
            Rectangle::new(0.0, 0.0, 640.0, 480.0),
        )
        .map_err(|e| e.to_string())?;
        let canvas = EditorCanvas::new(egui::Id::unique("document-canvas"), gpu, viewer)?;
        Ok(Self {
            workspace: Workspace::new(default_layout()),
            saved: None,
            error: None,
            content: Content {
                canvas,
                canvas_size: egui::Vec2::splat(1.0),
                notes: String::new(),
                selected_tool: "Select",
                error: None,
                canvas_shown: false,
            },
            smoke: SmokeCapture::from_args(),
        })
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
        ui.horizontal(|ui| {
            ui.strong("Novadraw");
            ui.separator();
            for (label, enabled, redo) in [
                ("Undo", self.content.canvas.domain.has_undo(), false),
                ("Redo", self.content.canvas.domain.has_redo(), true),
            ] {
                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                    let canvas = &mut self.content.canvas;
                    let result = if redo {
                        canvas.domain.redo(&mut canvas.viewer)
                    } else {
                        canvas.domain.undo(&mut canvas.viewer)
                    };
                    if let Err(error) = result {
                        self.error = Some(error.to_string());
                    }
                }
            }
            ui.separator();
            if ui.button("Reset layout").clicked() {
                self.workspace = Workspace::new(default_layout());
            }
            if ui.button("Remember layout").clicked() {
                match serde_json::to_string(&self.workspace) {
                    Ok(saved) => self.saved = Some(saved),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
            if ui
                .add_enabled(self.saved.is_some(), egui::Button::new("Restore layout"))
                .clicked()
            {
                let restored = self
                    .saved
                    .as_deref()
                    .ok_or_else(|| "No saved layout".to_owned())
                    .and_then(|saved| {
                        serde_json::from_str::<Workspace<Panel>>(saved).map_err(|e| e.to_string())
                    })
                    .and_then(|workspace| {
                        workspace
                            .validate()
                            .map_err(|e| e.to_string())
                            .map(|_| workspace)
                    });
                match restored {
                    Ok(workspace) => self.workspace = workspace,
                    Err(error) => self.error = Some(error),
                }
            }
            for panel in [Panel::Tools, Panel::Layers, Panel::Inspector, Panel::Notes] {
                let mut visible = self.workspace.layout.panels().contains(&&panel);
                if ui.checkbox(&mut visible, panel.label()).changed()
                    && let Err(error) = self
                        .workspace
                        .set_visible(panel, visible, None, |p| p.permissions())
                {
                    self.error = Some(error.to_string());
                }
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.separator();
        let style = DockStyle::from_ui(ui);
        self.content.canvas_shown = false;
        let output = DockArea::new(egui::Id::unique("novadraw-workbench")).show_content(
            ui,
            &self.workspace.layout,
            &style,
            &mut self.content,
        );
        if let Some(error) = output.error {
            self.error = Some(error.to_string());
        }
        if let Err(error) = self
            .workspace
            .apply_all(output.actions, |p| p.permissions())
        {
            self.error = Some(error.to_string());
        }
        if !self.content.canvas_shown
            && let Err(error) = self.content.canvas.deactivate(ui.ctx())
        {
            self.error = Some(error);
        }
        if let Some(error) = self.content.error.take() {
            self.error = Some(error);
        }
        if let Some(smoke) = &mut self.smoke {
            match smoke.step(ui.ctx()) {
                Ok(true) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                Ok(false) => {}
                Err(error) => {
                    self.error = Some(error);
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Novadraw Workbench",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(WINDOW_SIZE)
                .with_min_inner_size(MIN_WINDOW_SIZE),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|context| Ok(Box::new(App::new(context)?))),
    )
}

#[cfg(test)]
mod canvas_tests;
#[cfg(test)]
mod text_canvas_tests;
