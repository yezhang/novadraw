use crate::{
    Alignment, AppState, OutlineItem, OutlineItemKind, Session, ToolMode, UiCommand,
    model::{EdgeStyle, MIN_SIZE, ShapeKind},
};
use egui::{
    Align, Align2, Color32, DragValue, Event, Frame, Image, Key, Layout, Margin, PointerButton,
    Rect, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, TextureId, Vec2,
    load::SizedTexture,
};
use novadraw::event::{Key as EditorKey, KeyEvent, KeyEventKind, KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};

const LEFT_PANEL_WIDTH: f32 = 232.0;
const RIGHT_PANEL_WIDTH: f32 = 264.0;
const COMMAND_BUTTON_WIDTH: f32 = 64.0;
const TOOL_BUTTON_WIDTH: f32 = 72.0;
const FILE_BUTTON_WIDTH: f32 = 30.0;
const FILE_BUTTON_HEIGHT: f32 = 26.0;
const FILE_ICON_SIZE: f32 = 16.0;
const FILE_ICON_STROKE: f32 = 1.5;
const ALIGN_BUTTON_WIDTH: f32 = 28.0;
const ALIGN_BUTTON_HEIGHT: f32 = 26.0;
const ALIGN_ICON_SIZE: f32 = 16.0;
const ALIGN_ICON_STROKE: f32 = 1.5;
const PANEL_MARGIN: i8 = 8;
const PANEL_SECTION_GAP: f32 = 12.0;
const OUTLINE_ROW_HEIGHT: f32 = 28.0;
const PALETTE_ROW_HEIGHT: f32 = 30.0;
const PALETTE_ICON_SIZE: f32 = 16.0;
const PALETTE_ROW_PADDING_X: f32 = 4.0;
const PALETTE_ICON_LABEL_GAP: f32 = 10.0;
const FILTER_FIELD_HEIGHT: f32 = 30.0;
const PROPERTY_ROW_HEIGHT: f32 = 26.0;
const PROPERTY_LABEL_WIDTH: f32 = 48.0;
const PROPERTY_COLUMN_GAP: f32 = 8.0;
const PROPERTY_ROW_GAP: f32 = 4.0;
const PROPERTY_GROUP_GAP: f32 = 8.0;
const PROPERTY_FIELD_MAX_WIDTH: f32 = 152.0;
const INSPECTOR_DIVIDER_GAP: f32 = 8.0;
const PANEL_TITLE_SIZE: f32 = 17.0;
const SECTION_TITLE_SIZE: f32 = 11.0;
const SECONDARY_TEXT: Color32 = Color32::from_gray(96);
const PANEL_BORDER: Color32 = Color32::from_gray(214);
const PROPERTY_FIELD_FILL: Color32 = Color32::from_gray(247);
const PROPERTY_FIELD_HOVER_FILL: Color32 = Color32::from_gray(242);
const PROPERTY_FIELD_ACTIVE_FILL: Color32 = Color32::from_gray(238);
const PROPERTY_FIELD_BORDER: Color32 = Color32::from_gray(216);
const PRIMARY_BLUE: Color32 = Color32::from_rgb(45, 105, 145);
pub(crate) const SELECTION_BLUE: Color32 = Color32::from_rgb(224, 236, 244);
pub(crate) const SELECTION_TEXT: Color32 = Color32::from_rgb(35, 73, 96);
const ERROR_RED: Color32 = Color32::from_rgb(176, 40, 40);
const SCROLL_LINE_POINTS: f32 = 32.0;

#[derive(Default)]
pub struct EditorUi {
    canvas_input: CanvasInput,
    outline_filter: String,
}

pub struct UiOutput {
    pub canvas_size: [f64; 2],
    pub interaction: Result<bool, String>,
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
                    .inner_margin(Margin::symmetric(8, 4))
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
                            .hint_text("Document path")
                            .vertical_align(Align::Center)
                            .margin(Margin::symmetric(6, 3)),
                    );
                    file_action_group(ui, app);
                    ui.separator();
                    file_action_button(
                        ui,
                        FileActionIcon::DiscardAndOpen,
                        "Discard unsaved changes and open document",
                        app.canvas.dirty && !app.disabled,
                        app,
                        UiCommand::DiscardAndOpen,
                        egui::CornerRadius::same(4),
                    );
                });
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    tool_button(
                        ui,
                        ToolMode::Selection,
                        !app.disabled,
                        app,
                        UiCommand::SelectTool,
                    );
                    tool_button(
                        ui,
                        ToolMode::Marquee,
                        !app.disabled,
                        app,
                        UiCommand::Marquee,
                    );
                    tool_button(
                        ui,
                        ToolMode::SolidConnection,
                        !app.disabled,
                        app,
                        UiCommand::Connect(EdgeStyle::Solid),
                    );
                    tool_button(
                        ui,
                        ToolMode::DashedConnection,
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
                    alignment_group(
                        ui,
                        app,
                        [
                            Alignment::Left,
                            Alignment::HorizontalCenter,
                            Alignment::Right,
                        ],
                    );
                    ui.add_space(4.0);
                    alignment_group(
                        ui,
                        app,
                        [Alignment::Top, Alignment::VerticalCenter, Alignment::Bottom],
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
                    ui.label(format!("Tool: {}", app.tool_mode.label()));
                });
            });
    }

    fn show_status(&self, root: &mut egui::Ui, app: &AppState) {
        egui::Panel::bottom("editor_status")
            .default_size(26.0)
            .size_range(26.0..=26.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(246))
                    .inner_margin(Margin::symmetric(8, 4))
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

    fn show_palette_and_outline(&mut self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::left("palette_outline")
            .default_size(LEFT_PANEL_WIDTH)
            .size_range(208.0..=340.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(247))
                    .inner_margin(Margin::same(PANEL_MARGIN))
                    .stroke(Stroke::new(1.0, PANEL_BORDER)),
            )
            .show(root, |ui| {
                panel_title(ui, "Create", None);
                ui.add_space(6.0);
                palette_item(
                    ui,
                    app,
                    ShapeKind::Rectangle,
                    Color32::from_rgb(70, 118, 160),
                );
                palette_item(ui, app, ShapeKind::Ellipse, Color32::from_rgb(195, 138, 34));
                ui.add_space(PANEL_SECTION_GAP);
                ui.separator();
                ui.add_space(PANEL_SECTION_GAP);
                panel_title(ui, "Outline", Some(app.outline.len()));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let width = if self.outline_filter.is_empty() {
                        ui.available_width()
                    } else {
                        (ui.available_width() - 28.0).max(80.0)
                    };
                    ui.add_sized(
                        [width, FILTER_FIELD_HEIGHT],
                        TextEdit::singleline(&mut self.outline_filter)
                            .hint_text("Filter name or id")
                            .vertical_align(Align::Center)
                            .margin(Margin::symmetric(8, 4)),
                    );
                    if !self.outline_filter.is_empty()
                        && ui
                            .add_sized([24.0, FILTER_FIELD_HEIGHT], egui::Button::new("×"))
                            .on_hover_text("Clear filter")
                            .clicked()
                    {
                        self.outline_filter.clear();
                    }
                });
                ui.add_space(6.0);

                let query = self.outline_filter.trim().to_ascii_lowercase();
                let visible = app
                    .outline
                    .iter()
                    .filter(|item| {
                        query.is_empty() || item.title.to_ascii_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let shape_count = visible
                    .iter()
                    .filter(|item| !matches!(item.kind, OutlineItemKind::Connection(_)))
                    .count();
                let connection_count = visible.len() - shape_count;

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if visible.is_empty() {
                            outline_empty_state(ui, self.outline_filter.is_empty());
                            return;
                        }
                        egui::CollapsingHeader::new(format!("Shapes  {shape_count}"))
                            .id_salt("outline_shapes")
                            .default_open(true)
                            .show(ui, |ui| {
                                for item in visible.iter().filter(|item| {
                                    !matches!(item.kind, OutlineItemKind::Connection(_))
                                }) {
                                    outline_row(ui, app, item);
                                }
                            });
                        if connection_count > 0 {
                            ui.add_space(4.0);
                            egui::CollapsingHeader::new(format!("Connections  {connection_count}"))
                                .id_salt("outline_connections")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for item in visible.iter().filter(|item| {
                                        matches!(item.kind, OutlineItemKind::Connection(_))
                                    }) {
                                        outline_row(ui, app, item);
                                    }
                                });
                        }
                    });
            });
    }

    fn show_inspector(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::right("properties")
            .default_size(RIGHT_PANEL_WIDTH)
            .size_range(236.0..=380.0)
            .resizable(true)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(250))
                    .inner_margin(Margin::same(PANEL_MARGIN))
                    .stroke(Stroke::new(1.0, PANEL_BORDER)),
            )
            .show(root, |ui| {
                panel_title(ui, "Inspector", Some(app.selected.len()));
                ui.add_space(8.0);
                if let Some((id, _)) = app.property_subject {
                    let item = app.outline.iter().find(|item| item.id == id);
                    object_summary(ui, id, item.map(|item| item.kind), app.selected.len());
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    ui.separator();
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    geometry_editor(ui, app);
                } else {
                    inspector_empty_state(ui, app);
                }
            });
    }
}

fn panel_title(ui: &mut egui::Ui, title: &str, count: Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(PANEL_TITLE_SIZE).strong());
        if let Some(count) = count {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(count.to_string())
                        .small()
                        .color(SECONDARY_TEXT),
                );
            });
        }
    });
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        RichText::new(title.to_uppercase())
            .size(SECTION_TITLE_SIZE)
            .strong()
            .color(SECONDARY_TEXT),
    );
}

fn outline_empty_state(ui: &mut egui::Ui, unfiltered: bool) {
    ui.add_space(16.0);
    ui.label(
        RichText::new(if unfiltered {
            "No diagram items"
        } else {
            "No matching items"
        })
        .color(SECONDARY_TEXT),
    );
}

fn outline_row(ui: &mut egui::Ui, app: &mut AppState, item: &OutlineItem) {
    let selected = app.selected.contains(&item.id);
    let icon = match item.kind {
        OutlineItemKind::Rectangle => "□",
        OutlineItemKind::Ellipse => "○",
        OutlineItemKind::Connection(EdgeStyle::Solid) => "→",
        OutlineItemKind::Connection(EdgeStyle::Dashed) => "⇢",
    };
    let response = ui.add_enabled(
        !app.disabled,
        egui::Button::selectable(selected, format!("{icon}  {}", item.title))
            .min_size(Vec2::new(ui.available_width(), OUTLINE_ROW_HEIGHT))
            .truncate(),
    );
    if response.on_hover_text(item.title.clone()).clicked() {
        app.pending.push_back(UiCommand::Select(item.id));
    }
}

fn object_summary(
    ui: &mut egui::Ui,
    id: u64,
    kind: Option<OutlineItemKind>,
    selected_count: usize,
) {
    let (icon, title, color) = match kind {
        Some(OutlineItemKind::Rectangle) => ("□", "Rectangle", Color32::from_rgb(70, 118, 160)),
        Some(OutlineItemKind::Ellipse) => ("○", "Ellipse", Color32::from_rgb(195, 138, 34)),
        _ => ("◇", "Shape", SECONDARY_TEXT),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(20.0).color(color));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(title).strong());
            let detail = if selected_count > 1 {
                format!("#{id} · primary of {selected_count}")
            } else {
                format!("#{id}")
            };
            ui.label(RichText::new(detail).small().color(SECONDARY_TEXT));
        });
    });
}

fn inspector_empty_state(ui: &mut egui::Ui, app: &AppState) {
    ui.add_space(8.0);
    let (title, detail) = if app.selected.is_empty() {
        ("No selection", "No editable properties")
    } else {
        ("Connection selected", "No editable properties")
    };
    ui.label(RichText::new(title).strong());
    ui.add_space(4.0);
    ui.label(RichText::new(detail).color(SECONDARY_TEXT));
}

fn geometry_editor(ui: &mut egui::Ui, app: &mut AppState) {
    let Some((_, before)) = app.property_subject else {
        return;
    };
    let finite = app.properties.iter().all(|value| value.is_finite());
    let valid_size = app.properties[2] >= MIN_SIZE && app.properties[3] >= MIN_SIZE;
    let draft = Rectangle::new(
        app.properties[0],
        app.properties[1],
        app.properties[2],
        app.properties[3],
    );
    let changed = draft != before;

    ui.add_enabled_ui(!app.disabled, |ui| {
        section_title(ui, "Position");
        ui.add_space(4.0);
        property_rows(ui, &mut app.properties[..2], ["X", "Y"]);
        ui.add_space(PROPERTY_GROUP_GAP);
        section_title(ui, "Size");
        ui.add_space(4.0);
        property_rows(ui, &mut app.properties[2..], ["Width", "Height"]);
    });

    if !finite {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Values must be finite")
                .small()
                .color(ERROR_RED),
        );
    } else if !valid_size {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("Width and height must be at least {MIN_SIZE}"))
                .small()
                .color(ERROR_RED),
        );
    }

    ui.add_space(PANEL_SECTION_GAP);
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), PROPERTY_ROW_HEIGHT),
        Layout::right_to_left(Align::Center),
        |ui| {
            let apply = egui::Button::new(RichText::new("Apply").color(Color32::WHITE))
                .fill(PRIMARY_BLUE)
                .min_size(Vec2::new(80.0, PROPERTY_ROW_HEIGHT));
            if ui
                .add_enabled(!app.disabled && finite && valid_size && changed, apply)
                .clicked()
            {
                app.pending.push_back(UiCommand::ApplyProperties);
            }
            if ui
                .add_enabled(
                    !app.disabled && changed,
                    egui::Button::new("Reset").min_size(Vec2::new(64.0, PROPERTY_ROW_HEIGHT)),
                )
                .clicked()
            {
                app.properties = [before.x, before.y, before.width, before.height];
            }
        },
    );
}

fn property_rows<const N: usize>(ui: &mut egui::Ui, values: &mut [f64], labels: [&'static str; N]) {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = PROPERTY_FIELD_FILL;
        widgets.inactive.bg_fill = PROPERTY_FIELD_FILL;
        widgets.inactive.bg_stroke = Stroke::new(1.0, PROPERTY_FIELD_BORDER);
        widgets.hovered.weak_bg_fill = PROPERTY_FIELD_HOVER_FILL;
        widgets.hovered.bg_fill = PROPERTY_FIELD_HOVER_FILL;
        widgets.hovered.bg_stroke = Stroke::new(1.0, PROPERTY_FIELD_BORDER);
        widgets.active.weak_bg_fill = PROPERTY_FIELD_ACTIVE_FILL;
        widgets.active.bg_fill = PROPERTY_FIELD_ACTIVE_FILL;
        widgets.active.bg_stroke = Stroke::new(1.0, SELECTION_TEXT);
        widgets.open.weak_bg_fill = PROPERTY_FIELD_FILL;
        widgets.open.bg_fill = PROPERTY_FIELD_FILL;
        widgets.open.bg_stroke = Stroke::new(1.0, SELECTION_TEXT);

        for (index, (label, value)) in labels.into_iter().zip(values).enumerate() {
            if index > 0 {
                ui.add_space(PROPERTY_ROW_GAP);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = PROPERTY_COLUMN_GAP;
                ui.allocate_ui_with_layout(
                    Vec2::new(PROPERTY_LABEL_WIDTH, PROPERTY_ROW_HEIGHT),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.label(RichText::new(label).small().color(SECONDARY_TEXT));
                    },
                );
                let field_width = ui.available_width().min(PROPERTY_FIELD_MAX_WIDTH);
                ui.allocate_ui_with_layout(
                    Vec2::new(field_width, PROPERTY_ROW_HEIGHT),
                    Layout::left_to_right(Align::Center).with_main_align(Align::Min),
                    |ui| {
                        ui.spacing_mut().interact_size =
                            Vec2::new(field_width, PROPERTY_ROW_HEIGHT);
                        ui.add(
                            DragValue::new(value)
                                .speed(1.0)
                                .min_decimals(0)
                                .max_decimals(2)
                                .suffix(" px"),
                        );
                    },
                );
            });
        }
    });
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
            egui::Button::new(label).min_size(Vec2::new(COMMAND_BUTTON_WIDTH, 26.0)),
        )
        .clicked()
    {
        app.pending.push_back(command);
    }
}

#[derive(Clone, Copy)]
enum FileActionIcon {
    Open,
    Save,
    SaveAs,
    DiscardAndOpen,
}

fn file_action_group(ui: &mut egui::Ui, app: &mut AppState) -> [Response; 3] {
    let actions = [
        (FileActionIcon::Open, "Open document", UiCommand::Open),
        (FileActionIcon::Save, "Save document", UiCommand::Save),
        (
            FileActionIcon::SaveAs,
            "Save document as",
            UiCommand::SaveAs,
        ),
    ];
    ui.allocate_ui_with_layout(
        Vec2::new(FILE_BUTTON_WIDTH * actions.len() as f32, FILE_BUTTON_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            std::array::from_fn(|index| {
                let (icon, label, command) = actions[index];
                file_action_button(
                    ui,
                    icon,
                    label,
                    !app.disabled,
                    app,
                    command,
                    grouped_corner_radius(index, actions.len()),
                )
            })
        },
    )
    .inner
}

fn file_action_button(
    ui: &mut egui::Ui,
    icon: FileActionIcon,
    label: &'static str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
    corner_radius: egui::CornerRadius,
) -> Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(Vec2::new(FILE_BUTTON_WIDTH, FILE_BUTTON_HEIGHT))
            .corner_radius(corner_radius),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    let color = if response.enabled() {
        ui.style().interact(&response).text_color()
    } else {
        SECONDARY_TEXT.gamma_multiply(0.6)
    };
    paint_file_action_icon(ui.painter(), response.rect, icon, color);
    let response = response.on_hover_text(label);
    if response.clicked() {
        app.pending.push_back(command);
    }
    response
}

fn tool_button(
    ui: &mut egui::Ui,
    mode: ToolMode,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    if ui
        .add_enabled(
            enabled,
            egui::Button::selectable(app.tool_mode == mode, mode.label())
                .min_size(Vec2::new(TOOL_BUTTON_WIDTH, 26.0)),
        )
        .clicked()
    {
        app.pending.push_back(command);
    }
}

fn alignment_group(ui: &mut egui::Ui, app: &mut AppState, modes: [Alignment; 3]) -> [Response; 3] {
    ui.allocate_ui_with_layout(
        Vec2::new(ALIGN_BUTTON_WIDTH * modes.len() as f32, ALIGN_BUTTON_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            std::array::from_fn(|index| {
                let alignment = modes[index];
                let response = ui.add_enabled(
                    !app.disabled && app.can_align,
                    egui::Button::new("")
                        .min_size(Vec2::new(ALIGN_BUTTON_WIDTH, ALIGN_BUTTON_HEIGHT))
                        .corner_radius(grouped_corner_radius(index, modes.len())),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        response.enabled(),
                        alignment.label(),
                    )
                });
                let color = if response.enabled() {
                    ui.style().interact(&response).text_color()
                } else {
                    SECONDARY_TEXT.gamma_multiply(0.6)
                };
                paint_alignment_icon(ui.painter(), response.rect, alignment, color);
                let response = response
                    .on_hover_text(format!("{} to the last selected shape", alignment.label()))
                    .on_disabled_hover_text(
                        "Select at least two shapes; the last selected shape is the reference",
                    );
                if response.clicked() {
                    app.pending.push_back(UiCommand::Align(alignment));
                }
                response
            })
        },
    )
    .inner
}

fn grouped_corner_radius(index: usize, len: usize) -> egui::CornerRadius {
    egui::CornerRadius {
        nw: if index == 0 { 4 } else { 0 },
        sw: if index == 0 { 4 } else { 0 },
        ne: if index == len - 1 { 4 } else { 0 },
        se: if index == len - 1 { 4 } else { 0 },
    }
}

#[cfg(test)]
mod file_action_button_tests {
    use super::*;

    #[test]
    fn grouped_file_buttons_dispatch_commands_and_share_edges() {
        for index in 0..3 {
            let context = egui::Context::default();
            let mut app = AppState::default();
            let mut buttons = None;
            let _ = context.run_ui(egui::RawInput::default(), |ui| {
                buttons = Some(file_action_group(ui, &mut app));
            });
            let buttons = buttons.unwrap();
            assert_eq!(buttons[0].rect.right(), buttons[1].rect.left());
            assert_eq!(buttons[1].rect.right(), buttons[2].rect.left());
            let position = buttons[index].rect.center();
            for pressed in [true, false] {
                let _ = context.run_ui(
                    egui::RawInput {
                        events: vec![
                            Event::PointerMoved(position),
                            Event::PointerButton {
                                pos: position,
                                button: PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::default(),
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        file_action_group(ui, &mut app);
                    },
                );
            }
            assert!(matches!(
                (index, app.pending.pop_front()),
                (0, Some(UiCommand::Open))
                    | (1, Some(UiCommand::Save))
                    | (2, Some(UiCommand::SaveAs))
            ));
            assert!(app.pending.is_empty());
            app.disabled = true;
            let _ = context.run_ui(egui::RawInput::default(), |ui| {
                for button in file_action_group(ui, &mut app) {
                    assert!(!button.enabled());
                }
            });
        }
    }

    #[test]
    fn discard_and_open_requires_dirty_enabled_document() {
        let context = egui::Context::default();
        let mut app = AppState::default();
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            let response = file_action_button(
                ui,
                FileActionIcon::DiscardAndOpen,
                "Discard unsaved changes and open document",
                app.canvas.dirty && !app.disabled,
                &mut app,
                UiCommand::DiscardAndOpen,
                egui::CornerRadius::same(4),
            );
            assert!(!response.enabled());
        });

        app.canvas.dirty = true;
        let mut response = None;
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            response = Some(file_action_button(
                ui,
                FileActionIcon::DiscardAndOpen,
                "Discard unsaved changes and open document",
                app.canvas.dirty && !app.disabled,
                &mut app,
                UiCommand::DiscardAndOpen,
                egui::CornerRadius::same(4),
            ));
        });
        let position = response.unwrap().rect.center();
        for pressed in [true, false] {
            let _ = context.run_ui(
                egui::RawInput {
                    events: vec![
                        Event::PointerMoved(position),
                        Event::PointerButton {
                            pos: position,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    file_action_button(
                        ui,
                        FileActionIcon::DiscardAndOpen,
                        "Discard unsaved changes and open document",
                        app.canvas.dirty && !app.disabled,
                        &mut app,
                        UiCommand::DiscardAndOpen,
                        egui::CornerRadius::same(4),
                    );
                },
            );
        }
        assert!(matches!(
            app.pending.pop_front(),
            Some(UiCommand::DiscardAndOpen)
        ));
        assert!(app.pending.is_empty());
    }
}

#[cfg(test)]
mod alignment_button_tests {
    use super::*;

    #[test]
    fn grouped_buttons_dispatch_each_alignment_and_disable_without_selection() {
        for modes in [
            [
                Alignment::Left,
                Alignment::HorizontalCenter,
                Alignment::Right,
            ],
            [Alignment::Top, Alignment::VerticalCenter, Alignment::Bottom],
        ] {
            for index in 0..3 {
                let context = egui::Context::default();
                let mut app = AppState {
                    can_align: true,
                    ..Default::default()
                };
                let mut buttons = None;
                let _ = context.run_ui(egui::RawInput::default(), |ui| {
                    buttons = Some(alignment_group(ui, &mut app, modes));
                });
                let buttons = buttons.unwrap();
                assert_eq!(buttons[0].rect.center().y, buttons[2].rect.center().y);
                assert_eq!(buttons[0].rect.right(), buttons[1].rect.left());
                let position = buttons[index].rect.center();
                for pressed in [true, false] {
                    let _ = context.run_ui(
                        egui::RawInput {
                            events: vec![
                                Event::PointerMoved(position),
                                Event::PointerButton {
                                    pos: position,
                                    button: PointerButton::Primary,
                                    pressed,
                                    modifiers: egui::Modifiers::default(),
                                },
                            ],
                            ..Default::default()
                        },
                        |ui| {
                            alignment_group(ui, &mut app, modes);
                        },
                    );
                }
                assert!(
                    matches!(app.pending.pop_front(), Some(UiCommand::Align(mode)) if mode == modes[index])
                );
                assert!(app.pending.is_empty());
                app.can_align = false;
                let _ = context.run_ui(egui::RawInput::default(), |ui| {
                    for button in alignment_group(ui, &mut app, modes) {
                        assert!(!button.enabled());
                    }
                });
            }
        }
    }
}

fn paint_file_action_icon(
    painter: &egui::Painter,
    rect: Rect,
    icon: FileActionIcon,
    color: Color32,
) {
    let origin = rect.center() - Vec2::splat(FILE_ICON_SIZE / 2.0);
    let point = |x: f32, y: f32| {
        egui::pos2(
            origin.x + x * FILE_ICON_SIZE / 24.0,
            origin.y + y * FILE_ICON_SIZE / 24.0,
        )
    };
    let stroke = Stroke::new(FILE_ICON_STROKE, color);
    match icon {
        FileActionIcon::Open => {
            painter.add(egui::Shape::line(
                vec![
                    point(2.0, 18.0),
                    point(2.0, 5.0),
                    point(4.0, 3.0),
                    point(8.0, 3.0),
                    point(10.5, 6.0),
                    point(18.0, 6.0),
                    point(20.0, 8.0),
                    point(20.0, 10.0),
                ],
                stroke,
            ));
            painter.add(egui::Shape::line(
                vec![
                    point(2.0, 18.0),
                    point(4.0, 20.0),
                    point(18.5, 20.0),
                    point(20.5, 18.5),
                    point(22.0, 12.5),
                    point(20.0, 10.0),
                    point(9.25, 10.0),
                    point(7.5, 11.0),
                    point(6.0, 14.0),
                ],
                stroke,
            ));
        }
        FileActionIcon::Save => paint_save_icon(painter, &point, stroke, false),
        FileActionIcon::SaveAs => paint_save_icon(painter, &point, stroke, true),
        FileActionIcon::DiscardAndOpen => {
            painter.add(egui::Shape::closed_line(
                vec![
                    point(2.0, 18.0),
                    point(2.0, 5.0),
                    point(4.0, 3.0),
                    point(8.0, 3.0),
                    point(10.5, 6.0),
                    point(20.0, 6.0),
                    point(22.0, 8.0),
                    point(22.0, 18.0),
                    point(20.0, 20.0),
                    point(4.0, 20.0),
                ],
                stroke,
            ));
            painter.line_segment([point(9.5, 10.5), point(14.5, 15.5)], stroke);
            painter.line_segment([point(14.5, 10.5), point(9.5, 15.5)], stroke);
        }
    }
}

fn paint_save_icon(
    painter: &egui::Painter,
    point: &impl Fn(f32, f32) -> egui::Pos2,
    stroke: Stroke,
    save_as: bool,
) {
    painter.add(egui::Shape::closed_line(
        vec![
            point(5.0, 3.0),
            point(15.0, 3.0),
            point(21.0, 9.0),
            point(21.0, 19.0),
            point(19.0, 21.0),
            point(5.0, 21.0),
            point(3.0, 19.0),
            point(3.0, 5.0),
        ],
        stroke,
    ));
    painter.add(egui::Shape::line(
        vec![point(7.0, 3.0), point(7.0, 8.0), point(15.0, 8.0)],
        stroke,
    ));
    painter.add(egui::Shape::line(
        vec![
            point(7.0, 21.0),
            point(7.0, 13.0),
            point(17.0, 13.0),
            point(17.0, 21.0),
        ],
        stroke,
    ));
    if save_as {
        painter.line_segment([point(13.5, 17.0), point(22.5, 17.0)], stroke);
        painter.add(egui::Shape::line(
            vec![point(19.5, 14.0), point(22.5, 17.0), point(19.5, 20.0)],
            stroke,
        ));
    }
}

fn paint_alignment_icon(painter: &egui::Painter, rect: Rect, alignment: Alignment, color: Color32) {
    let icon = Rect::from_center_size(rect.center(), Vec2::splat(ALIGN_ICON_SIZE));
    let horizontal = matches!(
        alignment,
        Alignment::Left | Alignment::HorizontalCenter | Alignment::Right
    );
    let reference = match alignment {
        Alignment::Left | Alignment::Top => 0.0,
        Alignment::HorizontalCenter | Alignment::VerticalCenter => 0.5,
        Alignment::Right | Alignment::Bottom => 1.0,
    };
    let point = |along: f32, across: f32| {
        if horizontal {
            egui::pos2(icon.left() + along, icon.top() + across)
        } else {
            egui::pos2(icon.left() + across, icon.top() + along)
        }
    };
    let line = reference * ALIGN_ICON_SIZE;
    painter.line_segment(
        [point(line, 0.0), point(line, ALIGN_ICON_SIZE)],
        Stroke::new(ALIGN_ICON_STROKE, color),
    );
    // Two unequal bars distinguish edge alignment from center alignment.
    for (length, across) in [(10.0, 3.0), (6.0, 10.0)] {
        let start = reference * (ALIGN_ICON_SIZE - length);
        let bar = Rect::from_two_pos(point(start, across), point(start + length, across + 3.0));
        painter.rect_filled(bar, 0.0, color);
    }
}

fn palette_item(ui: &mut egui::Ui, app: &mut AppState, kind: ShapeKind, color: Color32) {
    let mode = match kind {
        ShapeKind::Rectangle => ToolMode::Rectangle,
        ShapeKind::Ellipse => ToolMode::Ellipse,
    };
    let selected = app.tool_mode == mode;
    ui.add_enabled_ui(!app.disabled, |ui| {
        let (row, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), PALETTE_ROW_HEIGHT),
            Sense::click_and_drag(),
        );
        let visuals = ui.style().interact(&response);
        if selected || response.hovered() || response.has_focus() {
            ui.painter().rect_filled(
                row,
                3.0,
                if selected {
                    SELECTION_BLUE
                } else {
                    visuals.weak_bg_fill
                },
            );
        }
        let icon = Rect::from_center_size(
            egui::pos2(
                row.left() + PALETTE_ROW_PADDING_X + PALETTE_ICON_SIZE / 2.0,
                row.center().y,
            ),
            Vec2::splat(PALETTE_ICON_SIZE),
        );
        let icon_color = if response.enabled() {
            color
        } else {
            color.gamma_multiply(0.45)
        };
        match kind {
            ShapeKind::Rectangle => {
                ui.painter().rect_filled(icon, 1.0, icon_color);
            }
            ShapeKind::Ellipse => {
                ui.painter()
                    .circle_filled(icon.center(), PALETTE_ICON_SIZE / 2.0, icon_color);
            }
        }
        ui.painter().text(
            egui::pos2(icon.right() + PALETTE_ICON_LABEL_GAP, row.center().y),
            Align2::LEFT_CENTER,
            mode.label(),
            TextStyle::Button.resolve(ui.style()),
            if selected {
                SELECTION_TEXT
            } else {
                visuals.text_color()
            },
        );
        response.dnd_set_drag_payload(kind);
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
