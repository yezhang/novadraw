//! Shared presentation for the GEF example applications.
//!
//! Owns window chrome and visual tokens only. Commands, selection and canvas
//! input remain with each example and its Editor session.

use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, RichText, Sense, Stroke, TextStyle, Vec2,
    ViewportBuilder, ViewportCommand,
};

pub use egui_phosphor::regular as icons;

pub const CHROME: Color32 = Color32::from_rgb(239, 240, 243);
pub const SIDEBAR: Color32 = Color32::from_rgb(247, 248, 250);
pub const SURFACE: Color32 = Color32::from_rgb(252, 252, 253);
pub const PASTEBOARD: Color32 = Color32::from_rgb(229, 232, 237);
pub const BORDER: Color32 = Color32::from_rgb(214, 218, 224);
pub const TEXT: Color32 = Color32::from_rgb(37, 42, 51);
pub const SECONDARY: Color32 = Color32::from_rgb(98, 106, 119);
pub const ACCENT: Color32 = Color32::from_rgb(0, 99, 218);
pub const SELECTED: Color32 = Color32::from_rgb(222, 234, 252);
pub const SELECTED_TEXT: Color32 = Color32::from_rgb(23, 72, 138);
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const TOOL_SIZE: f32 = 32.0;
pub const TOOL_RAIL_WIDTH: f32 = 48.0;
pub const SIDEBAR_WIDTH: f32 = 232.0;
pub const INSPECTOR_WIDTH: f32 = 264.0;
// AppKit's standard buttons are centered 14 pt below the window's top edge.
// Match that native row instead of centering controls in a taller custom bar.
pub const TITLEBAR_HEIGHT: f32 = if cfg!(target_os = "macos") {
    28.0
} else {
    48.0
};
pub const TITLEBAR_CONTROL_HEIGHT: f32 = if cfg!(target_os = "macos") {
    24.0
} else {
    CONTROL_HEIGHT
};
pub const TOOLBAR_HEIGHT: f32 = 48.0;
pub const STATUS_HEIGHT: f32 = 32.0;
const MACOS_BUTTON_INSET: f32 = 72.0;
const CORNER_RADIUS: u8 = 6;
const BAR_BORDER_WIDTH: f32 = 1.0;
const TITLEBAR_BUTTON_PADDING_Y: f32 = 2.0;
const TITLEBAR_ICON_SIZE: f32 = 16.0;
const TOOL_ICON_SIZE: f32 = 18.0;

pub fn viewport(title: &str, size: [f32; 2], minimum: [f32; 2]) -> ViewportBuilder {
    let viewport = ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(size)
        .with_min_inner_size(minimum);
    if cfg!(target_os = "macos") {
        viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
    } else {
        viewport
    }
}

pub fn configure_style(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(family.len().min(1), "phosphor".into());
    }
    context.set_fonts(fonts);
    context.set_visuals(egui::Visuals::light());
    context.style_mut_of(egui::Theme::Light, |style| {
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(18.0));
        style.spacing.item_spacing = Vec2::new(8.0, 4.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.panel_fill = SIDEBAR;
        style.visuals.window_fill = SURFACE;
        style.visuals.faint_bg_color = SIDEBAR;
        style.visuals.extreme_bg_color = SURFACE;
        style.visuals.window_corner_radius = CornerRadius::same(8);
        style.visuals.selection.bg_fill = SELECTED;
        style.visuals.selection.stroke = Stroke::new(1.0, SELECTED_TEXT);
        let widgets = &mut style.visuals.widgets;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.inactive.corner_radius = CornerRadius::same(CORNER_RADIUS);
        widgets.inactive.weak_bg_fill = Color32::from_rgb(236, 238, 242);
        widgets.inactive.bg_fill = SURFACE;
        widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.hovered = widgets.inactive;
        widgets.hovered.weak_bg_fill = Color32::from_rgb(224, 228, 234);
        widgets.hovered.bg_fill = widgets.hovered.weak_bg_fill;
        widgets.active = widgets.hovered;
        widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        widgets.active.weak_bg_fill = SELECTED;
        widgets.active.bg_fill = SELECTED;
        widgets.open = widgets.active;
        style.animation_time = 0.0;
    });
}

pub fn panel_frame(fill: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .inner_margin(Margin::same(8))
        .stroke(Stroke::new(1.0, BORDER))
}

fn bar_frame(fill: Color32, height: f32, control_height: f32) -> Frame {
    // The stroke is part of the frame's allocation, in addition to its margin.
    let vertical_margin = ((height - control_height) / 2.0 - BAR_BORDER_WIDTH) as i8;
    Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(8, vertical_margin))
        .stroke(Stroke::new(BAR_BORDER_WIDTH, BORDER))
}

/// Reserve space for the real macOS window buttons, and drag only the unused
/// portion of the title bar, so buttons and text fields keep their own input.
pub fn titlebar(root: &mut egui::Ui, id: &'static str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Panel::top(id)
        .exact_size(TITLEBAR_HEIGHT)
        .frame(bar_frame(CHROME, TITLEBAR_HEIGHT, TITLEBAR_CONTROL_HEIGHT))
        .show(root, |ui| {
            ui.spacing_mut().interact_size.y = TITLEBAR_CONTROL_HEIGHT;
            ui.spacing_mut().button_padding.y = TITLEBAR_BUTTON_PADDING_Y;
            ui.horizontal_centered(|ui| {
                if cfg!(target_os = "macos") {
                    ui.add_space(MACOS_BUTTON_INSET);
                }
                contents(ui);
                let (_, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().max(0.0), TITLEBAR_CONTROL_HEIGHT),
                    Sense::click_and_drag(),
                );
                if cfg!(target_os = "macos") && response.drag_started() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                }
            });
        });
}

pub fn toolbar(root: &mut egui::Ui, id: &'static str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Panel::top(id)
        .exact_size(TOOLBAR_HEIGHT)
        .frame(bar_frame(SURFACE, TOOLBAR_HEIGHT, CONTROL_HEIGHT))
        .show(root, |ui| {
            ui.spacing_mut().interact_size.y = CONTROL_HEIGHT;
            ui.horizontal_centered(contents);
        });
}

pub fn document_label(ui: &mut egui::Ui, title: &str, dirty: bool) -> egui::Response {
    ui.label(RichText::new(icons::FILE).size(18.0).color(SECONDARY));
    let response = ui.add_sized(
        Vec2::new(0.0, ui.spacing().interact_size.y),
        egui::Label::new(RichText::new(title).strong()).extend(),
    );
    ui.label(
        RichText::new(if dirty { "Edited" } else { "Saved" })
            .size(12.0)
            .color(SECONDARY),
    );
    response
}

pub fn section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(16.0);
    ui.label(RichText::new(label).size(12.0).strong().color(SECONDARY));
    ui.add_space(8.0);
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    let height = ui.spacing().interact_size.y;
    let icon_size = if height < CONTROL_HEIGHT {
        TITLEBAR_ICON_SIZE
    } else {
        TOOL_ICON_SIZE
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(icon).size(icon_size))
            .min_size(Vec2::new(TOOL_SIZE, height))
            .selected(selected),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    response.on_hover_text(label)
}

pub fn panel_heading(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(18.0).color(SECONDARY));
        ui.label(RichText::new(title).strong());
    });
    ui.add_space(8.0);
    ui.separator();
}
