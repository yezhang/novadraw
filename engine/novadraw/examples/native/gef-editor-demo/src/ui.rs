use crate::{
    Alignment, AppState, OutlineItem, OutlineItemKind, Session, ToolMode, UiCommand,
    model::{EdgeStyle, MIN_SIZE, ShapeKind},
};
use egui::{
    Align, Align2, Color32, DragValue, Event, Frame, Image, Key, Layout, Margin, PointerButton,
    Rect, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, TextureId, Vec2,
    load::SizedTexture,
};
use egui_phosphor::regular as icons;
use novadraw::event::{Key as EditorKey, KeyEvent, KeyEventKind, KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};

const LEFT_PANEL_WIDTH: f32 = 232.0;
const RIGHT_PANEL_WIDTH: f32 = 264.0;
const TOOL_RAIL_WIDTH: f32 = 44.0;
const COLLAPSED_PANEL_WIDTH: f32 = 28.0;
const COMMAND_BUTTON_WIDTH: f32 = 36.0;
const DOCUMENT_CONTROL_HEIGHT: f32 = 26.0;
const FILE_BUTTON_WIDTH: f32 = 30.0;
const FILE_BUTTON_HEIGHT: f32 = DOCUMENT_CONTROL_HEIGHT;
const FILE_ICON_SIZE: f32 = 16.0;
const ALIGN_BUTTON_WIDTH: f32 = 28.0;
const ALIGN_BUTTON_HEIGHT: f32 = 26.0;
const ALIGN_ICON_SIZE: f32 = 16.0;
const ALIGN_ICON_STROKE: f32 = 1.5;
const PANEL_MARGIN: i8 = 8;
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
const GRID_DOT_RADIUS: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LeftTab {
    #[default]
    Layers,
    Assets,
}

#[derive(Default)]
pub struct EditorUi {
    canvas_input: CanvasInput,
    outline_filter: String,
    left_tab: LeftTab,
    left_collapsed: bool,
    right_collapsed: bool,
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
        self.show_document_bar(root, app);
        self.show_status(root, app);
        self.show_tool_rail(root, app);
        self.show_left_panel(root, app);
        self.show_inspector(root, app);
        self.show_context_bar(root, app);

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
                if app.canvas.grid_visible {
                    paint_grid(ui.painter(), rect, session);
                }

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

    fn show_document_bar(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::top("document_bar")
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(248))
                    .inner_margin(Margin::symmetric(8, 4))
                    .stroke(Stroke::new(1.0, Color32::from_gray(210))),
            )
            .show(root, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), DOCUMENT_CONTROL_HEIGHT),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        document_title(ui, app.canvas.dirty);
                        ui.separator();
                        document_path_field(ui, &mut app.path_input);
                        file_action_group(ui, app);
                        ui.separator();
                        file_action_button(
                            ui,
                            icons::FOLDER_SIMPLE_MINUS,
                            "Discard unsaved changes and open document",
                            app.canvas.dirty && !app.disabled,
                            app,
                            UiCommand::DiscardAndOpen,
                            egui::CornerRadius::same(4),
                        );
                        ui.separator();
                        command_button(
                            ui,
                            icons::ARROW_COUNTER_CLOCKWISE,
                            "Undo",
                            app.canvas.can_undo && !app.disabled,
                            app,
                            UiCommand::Undo,
                        );
                        command_button(
                            ui,
                            icons::ARROW_CLOCKWISE,
                            "Redo",
                            app.canvas.can_redo && !app.disabled,
                            app,
                            UiCommand::Redo,
                        );
                    },
                );
            });
    }

    fn show_status(&self, root: &mut egui::Ui, app: &mut AppState) {
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
                ui.horizontal(|ui| {
                    let text = app
                        .canvas
                        .error
                        .as_deref()
                        .or(app.message.as_deref())
                        .unwrap_or_else(|| app.tool_mode.label());
                    let color = if app.canvas.error.is_some() {
                        ERROR_RED
                    } else {
                        Color32::from_gray(78)
                    };
                    ui.label(RichText::new(text).color(color));
                    ui.separator();
                    ui.label(match app.selected.len() {
                        0 => "No selection".to_owned(),
                        1 => "1 selected".to_owned(),
                        count => format!("{count} selected"),
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        status_command(
                            ui,
                            icons::CORNERS_OUT,
                            "Fit visible shapes",
                            app,
                            UiCommand::ZoomFit,
                        );
                        status_command(
                            ui,
                            icons::MAGNIFYING_GLASS_PLUS,
                            "Zoom in",
                            app,
                            UiCommand::ZoomIn,
                        );
                        status_command(
                            ui,
                            &format!("{}%", app.canvas.zoom_percent),
                            "Reset zoom to 100%",
                            app,
                            UiCommand::ZoomReset,
                        );
                        status_command(
                            ui,
                            icons::MAGNIFYING_GLASS_MINUS,
                            "Zoom out",
                            app,
                            UiCommand::ZoomOut,
                        );
                        ui.separator();
                        let snap_label = format!("{} Snap", icons::MAGNET);
                        status_toggle(
                            ui,
                            &snap_label,
                            app.canvas.snap_enabled,
                            app,
                            UiCommand::ToggleSnap,
                        );
                        let grid_label = format!("{} Grid", icons::GRID_FOUR);
                        status_toggle(
                            ui,
                            &grid_label,
                            app.canvas.grid_visible,
                            app,
                            UiCommand::ToggleGrid,
                        );
                        ui.separator();
                        let canvas_lock = format!(
                            "{} Canvas lock",
                            if app.disabled {
                                icons::LOCK
                            } else {
                                icons::LOCK_OPEN
                            }
                        );
                        if ui
                            .selectable_label(app.disabled, canvas_lock)
                            .on_hover_text("Temporarily disable canvas editing")
                            .clicked()
                        {
                            app.disabled = !app.disabled;
                        }
                    });
                });
            });
    }

    fn show_tool_rail(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::left("tool_rail")
            .exact_size(TOOL_RAIL_WIDTH)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(242))
                    .inner_margin(Margin::symmetric(5, 8))
                    .stroke(Stroke::new(1.0, PANEL_BORDER)),
            )
            .show(root, |ui| {
                ui.vertical_centered(|ui| {
                    rail_button(
                        ui,
                        icons::CURSOR,
                        "Select",
                        app.tool_mode == ToolMode::Selection,
                        !app.disabled,
                        app,
                        UiCommand::SelectTool,
                    );
                    rail_button(
                        ui,
                        icons::SELECTION,
                        "Marquee",
                        app.tool_mode == ToolMode::Marquee,
                        !app.disabled,
                        app,
                        UiCommand::Marquee,
                    );
                    ui.separator();
                    rail_button(
                        ui,
                        icons::RECTANGLE,
                        "Rectangle",
                        app.tool_mode == ToolMode::Rectangle,
                        !app.disabled,
                        app,
                        UiCommand::Rectangle,
                    );
                    rail_button(
                        ui,
                        icons::CIRCLE,
                        "Ellipse",
                        app.tool_mode == ToolMode::Ellipse,
                        !app.disabled,
                        app,
                        UiCommand::Ellipse,
                    );
                    rail_button(
                        ui,
                        icons::FLOW_ARROW,
                        "Connection",
                        matches!(
                            app.tool_mode,
                            ToolMode::SolidConnection | ToolMode::DashedConnection
                        ),
                        !app.disabled,
                        app,
                        UiCommand::Connect(EdgeStyle::Solid),
                    );
                });
            });
    }

    fn show_left_panel(&mut self, root: &mut egui::Ui, app: &mut AppState) {
        if self.left_collapsed {
            egui::Panel::left("left_sidebar_collapsed")
                .exact_size(COLLAPSED_PANEL_WIDTH)
                .resizable(false)
                .show(root, |ui| {
                    if ui
                        .button(RichText::new(icons::CARET_RIGHT).size(16.0))
                        .on_hover_text("Expand left sidebar")
                        .clicked()
                    {
                        self.left_collapsed = false;
                    }
                });
            return;
        }
        egui::Panel::left("layers_assets")
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
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.left_tab == LeftTab::Layers, "Layers")
                        .clicked()
                    {
                        self.left_tab = LeftTab::Layers;
                    }
                    if ui
                        .selectable_label(self.left_tab == LeftTab::Assets, "Assets")
                        .clicked()
                    {
                        self.left_tab = LeftTab::Assets;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new(icons::CARET_LEFT).size(16.0))
                            .on_hover_text("Collapse left sidebar")
                            .clicked()
                        {
                            self.left_collapsed = true;
                        }
                    });
                });
                ui.separator();
                ui.add_space(6.0);
                if self.left_tab == LeftTab::Assets {
                    panel_title(ui, "Create", None);
                    ui.add_space(6.0);
                    palette_item(
                        ui,
                        app,
                        ShapeKind::Rectangle,
                        Color32::from_rgb(70, 118, 160),
                    );
                    palette_item(ui, app, ShapeKind::Ellipse, Color32::from_rgb(195, 138, 34));
                    return;
                }
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
                            .add_sized(
                                [24.0, FILTER_FIELD_HEIGHT],
                                egui::Button::new(RichText::new(icons::X).size(15.0)),
                            )
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
                let connection_count = visible
                    .iter()
                    .filter(|item| matches!(item.kind, OutlineItemKind::Connection { .. }))
                    .count();

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if visible.is_empty() {
                            outline_empty_state(ui, self.outline_filter.is_empty());
                            return;
                        }
                        for group in app.groups.clone() {
                            let members = visible
                                .iter()
                                .filter(|item| group.members.contains(&item.id))
                                .cloned()
                                .collect::<Vec<_>>();
                            if members.is_empty() {
                                continue;
                            }
                            let selected = group
                                .members
                                .iter()
                                .all(|member| app.selected.contains(member));
                            egui::CollapsingHeader::new(format!(
                                "{}  {}",
                                group.title,
                                members.len()
                            ))
                            .id_salt(("layer_group", group.id))
                            .default_open(true)
                            .show(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!("{} Select group", icons::SQUARES_FOUR),
                                    )
                                    .clicked()
                                {
                                    app.pending.push_back(UiCommand::SelectGroup(group.id));
                                }
                                for item in &members {
                                    layer_row(ui, app, item);
                                }
                            });
                        }
                        let ungrouped = visible
                            .iter()
                            .filter(|item| {
                                !matches!(item.kind, OutlineItemKind::Connection { .. })
                                    && item.group.is_none()
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if !ungrouped.is_empty() {
                            egui::CollapsingHeader::new(format!("Ungrouped  {}", ungrouped.len()))
                                .id_salt("outline_shapes")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for item in &ungrouped {
                                        layer_row(ui, app, item);
                                    }
                                });
                        }
                        if connection_count > 0 {
                            ui.add_space(4.0);
                            egui::CollapsingHeader::new(format!("Connections  {connection_count}"))
                                .id_salt("outline_connections")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for item in visible.iter().filter(|item| {
                                        matches!(item.kind, OutlineItemKind::Connection { .. })
                                    }) {
                                        layer_row(ui, app, item);
                                    }
                                });
                        }
                    });
            });
    }

    fn show_inspector(&mut self, root: &mut egui::Ui, app: &mut AppState) {
        if self.right_collapsed {
            egui::Panel::right("right_sidebar_collapsed")
                .exact_size(COLLAPSED_PANEL_WIDTH)
                .resizable(false)
                .show(root, |ui| {
                    if ui
                        .button(RichText::new(icons::CARET_LEFT).size(16.0))
                        .on_hover_text("Expand Inspector")
                        .clicked()
                    {
                        self.right_collapsed = false;
                    }
                });
            return;
        }
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
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Inspector").size(PANEL_TITLE_SIZE).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new(icons::CARET_RIGHT).size(16.0))
                            .on_hover_text("Collapse Inspector")
                            .clicked()
                        {
                            self.right_collapsed = true;
                        }
                        ui.label(
                            RichText::new(app.selected.len().to_string())
                                .small()
                                .color(SECONDARY_TEXT),
                        );
                    });
                });
                ui.add_space(8.0);
                if let Some((id, _)) = app.property_subject {
                    let item = app.outline.iter().find(|item| item.id == id);
                    object_summary(ui, id, item.map(|item| item.kind), app.selected.len());
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    ui.separator();
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    name_editor(ui, app);
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    ui.separator();
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    geometry_editor(ui, app);
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    ui.separator();
                    ui.add_space(INSPECTOR_DIVIDER_GAP);
                    appearance_editor(ui, app);
                } else {
                    inspector_empty_state(ui, app);
                }
            });
    }

    fn show_context_bar(&self, root: &mut egui::Ui, app: &mut AppState) {
        egui::Panel::top("selection_context_bar")
            .exact_size(34.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(252))
                    .inner_margin(Margin::symmetric(8, 4))
                    .stroke(Stroke::new(1.0, PANEL_BORDER)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    context_icon(
                        ui,
                        icons::TRASH,
                        "Delete selection",
                        app.can_delete && !app.disabled,
                        app,
                        UiCommand::Delete,
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
                    alignment_group(
                        ui,
                        app,
                        [Alignment::Top, Alignment::VerticalCenter, Alignment::Bottom],
                    );
                    ui.separator();
                    if ui.available_width() >= 150.0 {
                        context_icon(
                            ui,
                            icons::SELECTION_BACKGROUND,
                            "Send backward",
                            app.can_send_backward && !app.disabled,
                            app,
                            UiCommand::SendBackward,
                        );
                        context_icon(
                            ui,
                            icons::SELECTION_FOREGROUND,
                            "Bring forward",
                            app.can_bring_forward && !app.disabled,
                            app,
                            UiCommand::BringForward,
                        );
                        context_icon(
                            ui,
                            icons::SQUARES_FOUR,
                            "Group selection",
                            app.can_group && !app.disabled,
                            app,
                            UiCommand::Group,
                        );
                        context_icon(
                            ui,
                            icons::SELECTION_SLASH,
                            "Ungroup selection",
                            app.can_ungroup && !app.disabled,
                            app,
                            UiCommand::Ungroup,
                        );
                    } else {
                        ui.menu_button(RichText::new(icons::DOTS_THREE).size(16.0), |ui| {
                            context_command(
                                ui,
                                &format!("{}  Send backward", icons::SELECTION_BACKGROUND),
                                app.can_send_backward && !app.disabled,
                                app,
                                UiCommand::SendBackward,
                            );
                            context_command(
                                ui,
                                &format!("{}  Bring forward", icons::SELECTION_FOREGROUND),
                                app.can_bring_forward && !app.disabled,
                                app,
                                UiCommand::BringForward,
                            );
                            context_command(
                                ui,
                                &format!("{}  Group", icons::SQUARES_FOUR),
                                app.can_group && !app.disabled,
                                app,
                                UiCommand::Group,
                            );
                            context_command(
                                ui,
                                &format!("{}  Ungroup", icons::SELECTION_SLASH),
                                app.can_ungroup && !app.disabled,
                                app,
                                UiCommand::Ungroup,
                            );
                        });
                    }
                    if matches!(
                        app.tool_mode,
                        ToolMode::SolidConnection | ToolMode::DashedConnection
                    ) {
                        ui.separator();
                        let (icon, tooltip, style) = if app.tool_mode == ToolMode::SolidConnection {
                            (
                                icons::LINE_SEGMENT,
                                "Solid connection; click for dashed",
                                EdgeStyle::Dashed,
                            )
                        } else {
                            (
                                icons::LINE_SEGMENTS,
                                "Dashed connection; click for solid",
                                EdgeStyle::Solid,
                            )
                        };
                        context_icon(
                            ui,
                            icon,
                            tooltip,
                            !app.disabled,
                            app,
                            UiCommand::Connect(style),
                        );
                    }
                });
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

fn layer_row(ui: &mut egui::Ui, app: &mut AppState, item: &OutlineItem) {
    let selected = app.selected.contains(&item.id);
    let icon = match item.kind {
        OutlineItemKind::Rectangle => icons::RECTANGLE,
        OutlineItemKind::Ellipse => icons::CIRCLE,
        OutlineItemKind::Connection {
            style,
            source,
            target,
        } => {
            connection_row(ui, app, item, style, source, target);
            return;
        }
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if ui
            .add_enabled(
                !app.disabled,
                egui::Button::new(RichText::new(if item.visible {
                    icons::EYE
                } else {
                    icons::EYE_SLASH
                }))
                .min_size(Vec2::new(22.0, OUTLINE_ROW_HEIGHT)),
            )
            .on_hover_text(if item.visible {
                "Hide layer"
            } else {
                "Show layer"
            })
            .clicked()
        {
            app.pending.push_back(UiCommand::ToggleVisibility(item.id));
        }
        if ui
            .add_enabled(
                !app.disabled,
                egui::Button::new(RichText::new(if item.locked {
                    icons::LOCK
                } else {
                    icons::LOCK_OPEN
                }))
                .min_size(Vec2::new(22.0, OUTLINE_ROW_HEIGHT)),
            )
            .on_hover_text(if item.locked {
                "Unlock layer"
            } else {
                "Lock layer"
            })
            .clicked()
        {
            app.pending.push_back(UiCommand::ToggleLock(item.id));
        }
        let response = ui.add(
            egui::Button::selectable(selected, format!("{icon}  {}", item.title))
                .min_size(Vec2::new(ui.available_width(), OUTLINE_ROW_HEIGHT))
                .truncate(),
        );
        if response.on_hover_text(item.title.clone()).clicked() {
            app.pending.push_back(UiCommand::Select(item.id));
        }
    });
}

fn connection_row(
    ui: &mut egui::Ui,
    app: &mut AppState,
    item: &OutlineItem,
    style: EdgeStyle,
    source: u64,
    target: u64,
) {
    let icon = match style {
        EdgeStyle::Solid => icons::LINE_SEGMENT,
        EdgeStyle::Dashed => icons::LINE_SEGMENTS,
    };
    let selected = app.selected.contains(&item.id);
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), OUTLINE_ROW_HEIGHT),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(format!("{source}  {}  {target}", icons::ARROW_RIGHT))
                        .small()
                        .color(SECONDARY_TEXT),
                );
                ui.add(
                    egui::Button::selectable(selected, format!("{icon}  {}", item.title))
                        .min_size(Vec2::new(ui.available_width(), OUTLINE_ROW_HEIGHT))
                        .truncate(),
                )
            },
        )
        .inner;
    if response
        .on_hover_text(format!("{}  {source} -> {target}", item.title))
        .clicked()
    {
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
        Some(OutlineItemKind::Rectangle) => (
            icons::RECTANGLE,
            "Rectangle",
            Color32::from_rgb(70, 118, 160),
        ),
        Some(OutlineItemKind::Ellipse) => {
            (icons::CIRCLE, "Ellipse", Color32::from_rgb(195, 138, 34))
        }
        _ => (icons::BOUNDING_BOX, "Shape", SECONDARY_TEXT),
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

fn name_editor(ui: &mut egui::Ui, app: &mut AppState) {
    section_title(ui, "Identity");
    ui.add_space(4.0);
    let editable = !app.disabled
        && app
            .presentation_draft
            .is_none_or(|presentation| !presentation.locked);
    let response = ui.add_enabled(
        editable,
        TextEdit::singleline(&mut app.name_draft)
            .desired_width(ui.available_width())
            .margin(Margin::symmetric(6, 4)),
    );
    let current = app
        .property_subject
        .and_then(|(id, _)| app.outline.iter().find(|item| item.id == id))
        .map(|item| item.title.as_str())
        .unwrap_or_default();
    let enter = response.has_focus() && ui.input(|input| input.key_pressed(Key::Enter));
    if (response.lost_focus() || enter)
        && !app.name_draft.trim().is_empty()
        && app.name_draft.trim() != current
    {
        app.pending
            .push_back(UiCommand::Rename(app.name_draft.trim().to_owned()));
    }
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

    let editable = !app.disabled
        && app
            .presentation_draft
            .is_none_or(|presentation| !presentation.locked);
    ui.add_enabled_ui(editable, |ui| {
        section_title(ui, "Position");
        ui.add_space(4.0);
        let (position_changed, position_commit) =
            property_rows(ui, &mut app.properties[..2], ["X", "Y"]);
        ui.add_space(PROPERTY_GROUP_GAP);
        section_title(ui, "Size");
        ui.add_space(4.0);
        let (size_changed, size_commit) =
            property_rows(ui, &mut app.properties[2..], ["Width", "Height"]);
        app.geometry_dirty |= position_changed || size_changed;
        if (position_commit || size_commit) && app.geometry_dirty && finite && valid_size && changed
        {
            app.pending.push_back(UiCommand::ApplyProperties);
            app.geometry_dirty = false;
        }
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
}

fn appearance_editor(ui: &mut egui::Ui, app: &mut AppState) {
    const FILL_SWATCHES: [[u8; 4]; 6] = [
        [75, 145, 200, 255],
        [229, 173, 82, 255],
        [87, 166, 120, 255],
        [194, 91, 103, 255],
        [151, 112, 190, 255],
        [232, 232, 232, 255],
    ];
    const STROKE_SWATCHES: [[u8; 4]; 4] = [
        [37, 52, 72, 255],
        [45, 105, 145, 255],
        [176, 40, 40, 255],
        [255, 255, 255, 255],
    ];
    let Some(mut draft) = app.presentation_draft else {
        return;
    };
    let editable = !app.disabled && !draft.locked;
    section_title(ui, "Appearance");
    ui.add_space(5.0);
    ui.label(RichText::new("Fill").small().color(SECONDARY_TEXT));
    ui.horizontal_wrapped(|ui| {
        for swatch in FILL_SWATCHES {
            if color_swatch(ui, swatch, draft.fill == swatch, editable, "Set fill color").clicked()
            {
                draft.fill = swatch;
                app.presentation_draft = Some(draft);
                app.pending.push_back(UiCommand::SetAppearance(draft));
            }
        }
    });
    ui.add_space(6.0);
    ui.label(RichText::new("Stroke").small().color(SECONDARY_TEXT));
    ui.horizontal_wrapped(|ui| {
        for swatch in STROKE_SWATCHES {
            if color_swatch(
                ui,
                swatch,
                draft.stroke == swatch,
                editable,
                "Set stroke color",
            )
            .clicked()
            {
                draft.stroke = swatch;
                app.presentation_draft = Some(draft);
                app.pending.push_back(UiCommand::SetAppearance(draft));
            }
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Opacity").small().color(SECONDARY_TEXT));
        let response = ui.add_enabled(
            editable,
            egui::Slider::new(&mut draft.opacity, 0..=100)
                .show_value(true)
                .suffix("%"),
        );
        app.appearance_dirty |= response.changed();
        if app.appearance_dirty && (response.drag_stopped() || response.lost_focus()) {
            app.presentation_draft = Some(draft);
            app.pending.push_back(UiCommand::SetAppearance(draft));
            app.appearance_dirty = false;
        } else {
            app.presentation_draft = Some(draft);
        }
    });
}

fn property_rows<const N: usize>(
    ui: &mut egui::Ui,
    values: &mut [f64],
    labels: [&'static str; N],
) -> (bool, bool) {
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

        let mut changed = false;
        let mut commit = false;
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
                        let response = ui.add(
                            DragValue::new(value)
                                .speed(1.0)
                                .min_decimals(0)
                                .max_decimals(2)
                                .suffix(" px"),
                        );
                        changed |= response.changed();
                        commit |= response.drag_stopped() || response.lost_focus();
                    },
                );
            });
        }
        (changed, commit)
    })
    .inner
}

fn rail_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    selected: bool,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    let response = ui.add_enabled(
        enabled,
        egui::Button::selectable(selected, RichText::new(icon).size(18.0))
            .min_size(Vec2::splat(32.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), tooltip)
    });
    if response.on_hover_text(tooltip).clicked() {
        app.pending.push_back(command);
    }
}

fn context_command(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    if ui
        .add_enabled(
            enabled,
            egui::Button::new(label).min_size(Vec2::new(58.0, 24.0)),
        )
        .clicked()
    {
        app.pending.push_back(command);
    }
}

fn context_icon(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(icon).size(16.0)).min_size(Vec2::new(28.0, 24.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), tooltip)
    });
    if response.on_hover_text(tooltip).clicked() {
        app.pending.push_back(command);
    }
}

fn status_command(
    ui: &mut egui::Ui,
    label: &str,
    tooltip: &str,
    app: &mut AppState,
    command: UiCommand,
) {
    let response = ui.button(label);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), tooltip)
    });
    if response.on_hover_text(tooltip).clicked() {
        app.pending.push_back(command);
    }
}

fn status_toggle(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    if ui.selectable_label(selected, label).clicked() {
        app.pending.push_back(command);
    }
}

fn color_swatch(
    ui: &mut egui::Ui,
    rgba: [u8; 4],
    selected: bool,
    enabled: bool,
    tooltip: &str,
) -> Response {
    let [r, g, b, a] = rgba;
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .fill(Color32::from_rgba_unmultiplied(r, g, b, a))
            .stroke(Stroke::new(
                if selected { 2.0 } else { 1.0 },
                if selected {
                    PRIMARY_BLUE
                } else {
                    PROPERTY_FIELD_BORDER
                },
            ))
            .min_size(Vec2::splat(24.0)),
    );
    response.on_hover_text(tooltip)
}

fn paint_grid(painter: &egui::Painter, rect: Rect, session: &Session) {
    let settings = session.workspace.get();
    let Ok(scale) = session.viewer.viewport_scale() else {
        return;
    };
    let Ok(origin) = session.viewer.viewport_origin() else {
        return;
    };
    let mut model_step = settings.grid_size;
    while model_step * scale < 10.0 {
        model_step *= 2.0;
    }
    let step = (model_step * scale) as f32;
    if !step.is_finite() || step <= 0.0 {
        return;
    }
    let start_x =
        rect.left() + (((origin.x() / model_step).ceil() * model_step - origin.x()) * scale) as f32;
    let start_y =
        rect.top() + (((origin.y() / model_step).ceil() * model_step - origin.y()) * scale) as f32;
    let color = Color32::from_rgba_unmultiplied(70, 82, 92, 42);
    let mut x = start_x;
    let mut count = 0usize;
    while x <= rect.right() && count < 10_000 {
        let mut y = start_y;
        while y <= rect.bottom() && count < 10_000 {
            painter.circle_filled(egui::pos2(x, y), GRID_DOT_RADIUS, color);
            y += step;
            count += 1;
        }
        x += step;
    }
}

fn command_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
) {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(icon).size(FILE_ICON_SIZE))
            .min_size(Vec2::new(COMMAND_BUTTON_WIDTH, DOCUMENT_CONTROL_HEIGHT)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), tooltip)
    });
    if response.on_hover_text(tooltip).clicked() {
        app.pending.push_back(command);
    }
}

fn document_path_field(ui: &mut egui::Ui, path: &mut String) -> Response {
    ui.add_sized(
        [240.0, DOCUMENT_CONTROL_HEIGHT],
        TextEdit::singleline(path)
            .hint_text("Document path")
            .vertical_align(Align::Center)
            .margin(Margin::symmetric(6, 3)),
    )
}

fn document_title(ui: &mut egui::Ui, dirty: bool) -> Response {
    let response = ui.strong("Shapes");
    if dirty {
        ui.label(RichText::new("Modified").color(Color32::from_rgb(166, 83, 18)));
    }
    response
}

fn file_action_group(ui: &mut egui::Ui, app: &mut AppState) -> [Response; 3] {
    let actions = [
        (icons::FOLDER_OPEN, "Open document", UiCommand::Open),
        (icons::FLOPPY_DISK, "Save document", UiCommand::Save),
        (
            icons::FLOPPY_DISK_BACK,
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
                let (icon, label, command) = actions[index].clone();
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
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    app: &mut AppState,
    command: UiCommand,
    corner_radius: egui::CornerRadius,
) -> Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(icon).size(FILE_ICON_SIZE))
            .min_size(Vec2::new(FILE_BUTTON_WIDTH, FILE_BUTTON_HEIGHT))
            .corner_radius(corner_radius),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    let response = response.on_hover_text(label);
    if response.clicked() {
        app.pending.push_back(command);
    }
    response
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
    fn document_path_and_file_buttons_share_height_and_centerline() {
        let context = egui::Context::default();
        let mut app = AppState::default();
        let mut title = None;
        let mut field = None;
        let mut buttons = None;
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(600.0, DOCUMENT_CONTROL_HEIGHT),
                Layout::left_to_right(Align::Center),
                |ui| {
                    title = Some(document_title(ui, false));
                    field = Some(document_path_field(ui, &mut app.path_input));
                    buttons = Some(file_action_group(ui, &mut app));
                },
            );
        });
        let title = title.unwrap();
        let field = field.unwrap();
        assert!((title.rect.center().y - field.rect.center().y).abs() < f32::EPSILON);
        for button in buttons.unwrap() {
            assert_eq!(field.rect.height(), button.rect.height());
            assert!((field.rect.center().y - button.rect.center().y).abs() < f32::EPSILON);
        }
    }

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
                icons::FOLDER_SIMPLE_MINUS,
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
                icons::FOLDER_SIMPLE_MINUS,
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
                        icons::FOLDER_SIMPLE_MINUS,
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
