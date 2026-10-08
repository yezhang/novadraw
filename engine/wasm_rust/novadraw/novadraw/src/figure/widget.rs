use std::sync::Arc;

use crate::Color;
use crate::geometry::{Dimension, Insets, Point, Rectangle};
use crate::render::{
    NdCanvas,
    command::{LineCap, LineJoin},
};

use super::border::{BevelBorder, BevelStyle, Border};
use super::{
    AccessibleFigure, Bounded, ChildPolicy, Figure, FigureContainer, FigureEventHandler,
    LabelFigure,
};
use crate::{
    AccessibilityAction, AccessibilityRole, EventContext, FigureStyle, Key, KeyEvent, MouseButton,
    MouseEvent,
};

const BUTTON_PADDING: f64 = 6.0;
const BUTTON_BEVEL_WIDTH: u32 = 2;
const PRESSED_CONTENT_OFFSET: f64 = 1.0;
const FOCUS_INSET: f64 = 4.0;
const FOCUS_STROKE_WIDTH: f64 = 1.0;

const BUTTON_BACKGROUND: Color = Color::rgba(0.91, 0.92, 0.94, 1.0);
const BUTTON_ROLLOVER_BACKGROUND: Color = Color::rgba(0.84, 0.9, 0.98, 1.0);
const BUTTON_PRESSED_BACKGROUND: Color = Color::rgba(0.72, 0.82, 0.94, 1.0);
const TOGGLE_SELECTED_BACKGROUND: Color = Color::rgba(0.67, 0.82, 0.76, 1.0);
const DISABLED_BACKGROUND: Color = Color::rgba(0.86, 0.86, 0.86, 1.0);
const BORDER_HIGHLIGHT: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const BORDER_SHADOW: Color = Color::rgba(0.32, 0.35, 0.4, 1.0);
const FOCUS_COLOR: Color = Color::rgba(0.08, 0.3, 0.62, 1.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickableKind {
    Push,
    Toggle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClickableVisualState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClickableSnapshot {
    pub kind: ClickableKind,
    pub selected: bool,
    pub rollover_enabled: bool,
    pub action_revision: u64,
    pub visual: ClickableVisualState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetError {
    Faulted,
    UnknownFigure(crate::FigureId),
    WrongCapability(crate::FigureId),
}

impl std::fmt::Display for WidgetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure: {id:?}"),
            Self::WrongCapability(id) => {
                write!(formatter, "Figure does not support widget behavior: {id:?}")
            }
        }
    }
}

impl std::error::Error for WidgetError {}

#[derive(Clone, Debug)]
pub struct ClickableModel {
    kind: ClickableKind,
    selected: bool,
    rollover_enabled: bool,
    action_revision: u64,
    visual: ClickableVisualState,
}

impl ClickableModel {
    pub fn new(kind: ClickableKind) -> Self {
        Self {
            kind,
            selected: false,
            rollover_enabled: true,
            action_revision: 0,
            visual: ClickableVisualState {
                enabled: true,
                ..ClickableVisualState::default()
            },
        }
    }

    pub fn snapshot(&self) -> ClickableSnapshot {
        ClickableSnapshot {
            kind: self.kind,
            selected: self.selected,
            rollover_enabled: self.rollover_enabled,
            action_revision: self.action_revision,
            visual: self.visual,
        }
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn rollover_enabled(&self) -> bool {
        self.rollover_enabled
    }

    fn set_selected(&mut self, selected: bool) -> bool {
        if self.selected == selected {
            return false;
        }
        self.selected = selected;
        true
    }

    fn set_rollover_enabled(&mut self, enabled: bool) -> bool {
        if self.rollover_enabled == enabled {
            return false;
        }
        self.rollover_enabled = enabled;
        true
    }

    fn activate(&mut self) -> (Option<(bool, bool)>, u64) {
        let selected = if self.kind == ClickableKind::Toggle {
            let old = self.selected;
            self.selected = !self.selected;
            Some((old, self.selected))
        } else {
            None
        };
        self.action_revision = self.action_revision.wrapping_add(1);
        (selected, self.action_revision)
    }

    fn sync_visual(&mut self, visual: ClickableVisualState) -> bool {
        if self.visual == visual {
            return false;
        }
        self.visual = visual;
        true
    }
}

pub trait ClickableBehavior {
    fn clickable_model(&self) -> &ClickableModel;
    fn clickable_model_mut(&mut self) -> &mut ClickableModel;
}

fn handles_activation_key(key: Key) -> bool {
    matches!(key, Key::Character(' ') | Key::Enter)
}

fn handle_mouse_pressed(event: &MouseEvent) -> bool {
    event.button == MouseButton::Left
}

fn handle_mouse_released(event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
    if event.button != MouseButton::Left || !ctx.is_pointer_pressed() {
        return false;
    }
    if ctx.target_bounds().contains(Point::new(event.x, event.y)) {
        ctx.activate_clickable();
    }
    true
}

fn handle_key_pressed(event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
    if !handles_activation_key(event.key) {
        return false;
    }
    if !ctx.is_keyboard_pressed() {
        ctx.set_keyboard_pressed(Some(event.key));
    }
    true
}

fn handle_key_released(event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
    if !handles_activation_key(event.key) || ctx.keyboard_pressed_key() != Some(event.key) {
        return false;
    }
    ctx.activate_clickable();
    ctx.set_keyboard_pressed(None);
    true
}

fn handle_focus_lost(ctx: &mut EventContext<'_>) -> bool {
    if ctx.is_keyboard_pressed() {
        ctx.set_keyboard_pressed(None);
    }
    if ctx.is_pointer_pressed() {
        ctx.set_pressed(false);
    }
    true
}

fn paint_focus(gc: &mut NdCanvas, bounds: Rectangle, focused: bool) {
    if !focused {
        return;
    }
    let width = (bounds.width - FOCUS_INSET * 2.0).max(0.0);
    let height = (bounds.height - FOCUS_INSET * 2.0).max(0.0);
    gc.stroke_rect_with_style(
        FOCUS_INSET,
        FOCUS_INSET,
        width,
        height,
        FOCUS_COLOR,
        crate::graphics::StrokeStyle::default()
            .with_width(FOCUS_STROKE_WIDTH)
            .expect("valid focus stroke")
            .with_cap(LineCap::Butt)
            .with_join(LineJoin::Miter),
    );
}

fn default_button_border(style: BevelStyle) -> BevelBorder {
    BevelBorder::new(style, BORDER_HIGHLIGHT, BORDER_SHADOW, BUTTON_BEVEL_WIDTH)
}

fn paint_widget(
    gc: &mut NdCanvas,
    bounds: Rectangle,
    model: &ClickableModel,
    label: Option<&LabelFigure>,
    toggle: bool,
) {
    let snapshot = model.snapshot();
    let push_selected = !toggle && snapshot.selected;
    let background = if !snapshot.visual.enabled {
        DISABLED_BACKGROUND
    } else if snapshot.visual.pressed || push_selected {
        BUTTON_PRESSED_BACKGROUND
    } else if toggle && snapshot.selected {
        TOGGLE_SELECTED_BACKGROUND
    } else if snapshot.rollover_enabled && snapshot.visual.hovered {
        BUTTON_ROLLOVER_BACKGROUND
    } else {
        BUTTON_BACKGROUND
    };
    gc.fill_rect_with_color(0.0, 0.0, bounds.width, bounds.height, background);
    if let Some(label) = label {
        let offset = if snapshot.visual.pressed || push_selected {
            PRESSED_CONTENT_OFFSET
        } else {
            0.0
        };
        gc.push_state();
        gc.translate(offset, offset);
        label.paint_figure_in_bounds(gc, bounds);
        gc.pop_state();
    }
}

fn paint_widget_border(gc: &mut NdCanvas, bounds: Rectangle, model: &ClickableModel) {
    let snapshot = model.snapshot();
    let lowered = snapshot.visual.pressed || snapshot.selected;
    default_button_border(if lowered {
        BevelStyle::Lowered
    } else {
        BevelStyle::Raised
    })
    .paint(bounds, gc);
    paint_focus(gc, bounds, snapshot.visual.focused);
}

#[derive(Clone)]
pub struct ClickableFigure {
    bounds: Rectangle,
    model: ClickableModel,
}

impl ClickableFigure {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            model: ClickableModel::new(ClickableKind::Push),
        }
    }

    pub fn snapshot(&self) -> ClickableSnapshot {
        self.model.snapshot()
    }
}

impl Figure for ClickableFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ClickableFigure"
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint_border_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        paint_focus(gc, bounds, self.model.visual.focused);
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::INPUT, crate::InputCapability::of::<Self>())?;
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(crate::CLICKABLE, crate::ClickableCapability::of::<Self>())
    }
}

impl Bounded for ClickableFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ClickableFigure"
    }
}

impl FigureContainer for ClickableFigure {
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }
}

impl ClickableBehavior for ClickableFigure {
    fn clickable_model(&self) -> &ClickableModel {
        &self.model
    }

    fn clickable_model_mut(&mut self) -> &mut ClickableModel {
        &mut self.model
    }
}

macro_rules! impl_clickable_events {
    ($figure:ty) => {
        impl FigureEventHandler for $figure {
            fn on_mouse_pressed(&self, event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
                handle_mouse_pressed(event)
            }

            fn on_mouse_released(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
                handle_mouse_released(event, ctx)
            }

            fn on_mouse_dragged(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
                true
            }

            fn on_key_pressed(&self, event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
                handle_key_pressed(event, ctx)
            }

            fn on_key_released(&self, event: &KeyEvent, ctx: &mut EventContext<'_>) -> bool {
                handle_key_released(event, ctx)
            }

            fn on_focus_lost(
                &self,
                _event: &crate::FocusEvent,
                ctx: &mut EventContext<'_>,
            ) -> bool {
                handle_focus_lost(ctx)
            }
        }
    };
}

impl_clickable_events!(ClickableFigure);

#[derive(Clone)]
pub struct ButtonFigure {
    bounds: Rectangle,
    model: ClickableModel,
    label: LabelFigure,
    border: Arc<dyn Border>,
}

impl ButtonFigure {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            model: ClickableModel::new(ClickableKind::Push),
            label: LabelFigure::new(text),
            border: Arc::new(default_button_border(BevelStyle::Raised)),
        }
    }

    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_icon(mut self, icon: crate::ImageId) -> Self {
        self.label.set_icon(Some(icon));
        self
    }

    pub fn snapshot(&self) -> ClickableSnapshot {
        self.model.snapshot()
    }

    pub(crate) fn label_component(&self) -> &LabelFigure {
        &self.label
    }

    pub(crate) fn label_component_mut(&mut self) -> &mut LabelFigure {
        &mut self.label
    }
}

impl Figure for ButtonFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ButtonFigure"
    }

    fn initial_insets(&self) -> Insets {
        let width = f64::from(BUTTON_BEVEL_WIDTH) + BUTTON_PADDING;
        Insets::uniform(width)
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(BUTTON_BACKGROUND),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn intrinsic_size(&self) -> Dimension {
        let (width, height) = self.label.preferred_size().unwrap_or_default();
        let inset = (f64::from(BUTTON_BEVEL_WIDTH) + BUTTON_PADDING) * 2.0;
        Dimension::new(width + inset, height + inset)
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        paint_widget(gc, bounds, &self.model, Some(&self.label), false);
    }

    fn paint_border_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        paint_widget_border(gc, bounds, &self.model);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Some(self.border.as_ref())
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::INPUT, crate::InputCapability::of::<Self>())?;
        out.register(
            crate::ACCESSIBILITY,
            crate::AccessibilityCapability::of::<Self>(),
        )?;
        out.register(crate::CLICKABLE, crate::ClickableCapability::of::<Self>())
    }
}

impl Bounded for ButtonFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ButtonFigure"
    }
}

impl ClickableBehavior for ButtonFigure {
    fn clickable_model(&self) -> &ClickableModel {
        &self.model
    }

    fn clickable_model_mut(&mut self) -> &mut ClickableModel {
        &mut self.model
    }
}

impl AccessibleFigure for ButtonFigure {
    fn accessible_name(&self) -> Option<&str> {
        Some(self.label.text())
    }

    fn accessible_role(&self) -> AccessibilityRole {
        AccessibilityRole::Button
    }

    fn accessible_default_action(&self) -> Option<AccessibilityAction> {
        Some(AccessibilityAction::Default)
    }
}

impl_clickable_events!(ButtonFigure);

#[derive(Clone)]
pub struct ToggleFigure {
    bounds: Rectangle,
    model: ClickableModel,
    label: LabelFigure,
    border: Arc<dyn Border>,
}

impl ToggleFigure {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            model: ClickableModel::new(ClickableKind::Toggle),
            label: LabelFigure::new(text),
            border: Arc::new(default_button_border(BevelStyle::Raised)),
        }
    }

    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_icon(mut self, icon: crate::ImageId) -> Self {
        self.label.set_icon(Some(icon));
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.model.selected = selected;
        self
    }

    pub fn snapshot(&self) -> ClickableSnapshot {
        self.model.snapshot()
    }

    pub(crate) fn label_component(&self) -> &LabelFigure {
        &self.label
    }

    pub(crate) fn label_component_mut(&mut self) -> &mut LabelFigure {
        &mut self.label
    }
}

impl Figure for ToggleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ToggleFigure"
    }

    fn initial_insets(&self) -> Insets {
        let width = f64::from(BUTTON_BEVEL_WIDTH) + BUTTON_PADDING;
        Insets::uniform(width)
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(BUTTON_BACKGROUND),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn intrinsic_size(&self) -> Dimension {
        let (width, height) = self.label.preferred_size().unwrap_or_default();
        let inset = (f64::from(BUTTON_BEVEL_WIDTH) + BUTTON_PADDING) * 2.0;
        Dimension::new(width + inset, height + inset)
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        paint_widget(gc, bounds, &self.model, Some(&self.label), true);
    }

    fn paint_border_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        paint_widget_border(gc, bounds, &self.model);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Some(self.border.as_ref())
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::INPUT, crate::InputCapability::of::<Self>())?;
        out.register(
            crate::ACCESSIBILITY,
            crate::AccessibilityCapability::of::<Self>(),
        )?;
        out.register(crate::CLICKABLE, crate::ClickableCapability::of::<Self>())
    }
}

impl Bounded for ToggleFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ToggleFigure"
    }
}

impl ClickableBehavior for ToggleFigure {
    fn clickable_model(&self) -> &ClickableModel {
        &self.model
    }

    fn clickable_model_mut(&mut self) -> &mut ClickableModel {
        &mut self.model
    }
}

impl AccessibleFigure for ToggleFigure {
    fn accessible_name(&self) -> Option<&str> {
        Some(self.label.text())
    }

    fn accessible_role(&self) -> AccessibilityRole {
        AccessibilityRole::ToggleButton
    }

    fn accessible_default_action(&self) -> Option<AccessibilityAction> {
        Some(AccessibilityAction::Default)
    }
}

impl_clickable_events!(ToggleFigure);

pub(crate) fn activate(model: &mut ClickableModel) -> (Option<(bool, bool)>, u64) {
    model.activate()
}

pub(crate) fn set_selected(model: &mut ClickableModel, selected: bool) -> bool {
    model.set_selected(selected)
}

pub(crate) fn set_rollover_enabled(model: &mut ClickableModel, enabled: bool) -> bool {
    model.set_rollover_enabled(enabled)
}

pub(crate) fn sync_visual(model: &mut ClickableModel, visual: ClickableVisualState) -> bool {
    model.sync_visual(visual)
}
