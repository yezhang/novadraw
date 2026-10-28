use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use novadraw::{
    event::{EventContext, FigureEventHandler, Key, KeyEvent, MouseButton, MouseEvent},
    figure::{
        CONTAINER, ChildClippingStrategy, ContainerCapability, CursorIcon, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer, FigureStyle,
        INPUT, InputCapability, LabelFigure,
    },
    geometry::{Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    runtime::Runtime,
    text::{FontDescriptor, TextConstraints, TextLayout},
    tree::FigureId,
};

type BoolAction = Arc<dyn for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync>;
type ChoiceAction = Arc<dyn for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync>;
type ScaleAction = Arc<dyn for<'a> Fn(f64, &mut EventContext<'a>) + Send + Sync>;
type PushAction = Arc<dyn for<'a> Fn(&mut EventContext<'a>) + Send + Sync>;
type TextAction = Arc<dyn for<'a> Fn(String, &mut EventContext<'a>) + Send + Sync>;

const LABEL_X: f64 = 22.0;
const INDICATOR_SIZE: f64 = 14.0;
const CONTROL_HEIGHT: f64 = 22.0;

pub struct PushButtonFigure {
    bounds: Rectangle,
    action: PushAction,
}

impl PushButtonFigure {
    pub fn new(
        bounds: Rectangle,
        action: impl for<'a> Fn(&mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            action: Arc::new(action),
        }
    }
}

impl Figure for PushButtonFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Button"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Pointer),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::rgba(0.9, 0.9, 0.9, 1.0));
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for PushButtonFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for PushButtonFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        (self.action)(context);
        true
    }
}

pub struct ComboBoxFigure {
    bounds: Rectangle,
    options: Arc<Vec<String>>,
    selected: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
    popup: Arc<Mutex<Option<FigureId>>>,
    open: Arc<AtomicBool>,
    action: ChoiceAction,
}

impl ComboBoxFigure {
    pub fn new_with_enabled(
        bounds: Rectangle,
        options: Arc<Vec<String>>,
        selected: Arc<AtomicUsize>,
        enabled: Arc<AtomicBool>,
        popup: Arc<Mutex<Option<FigureId>>>,
        open: Arc<AtomicBool>,
        action: impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            options,
            selected,
            enabled,
            popup,
            open,
            action: Arc::new(action),
        }
    }
}

impl Figure for ComboBoxFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Combo"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Pointer),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(if self.enabled.load(Ordering::Relaxed) {
            Color::WHITE
        } else {
            Color::rgba(0.88, 0.88, 0.88, 1.0)
        });
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))?;
        let right = bounds.width - 8.0;
        context.set_fill_paint(Color::BLACK);
        let mut arrow = novadraw::graphics::Path::new();
        arrow.move_to(right - 4.0, bounds.height / 2.0 - 2.0);
        arrow.line_to(right + 4.0, bounds.height / 2.0 - 2.0);
        arrow.line_to(right, bounds.height / 2.0 + 3.0);
        arrow.close();
        context.fill_path(&arrow)
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for ComboBoxFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for ComboBoxFigure {
    fn wants_mouse_events(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left
            || self.options.is_empty()
            || !self.enabled.load(Ordering::Relaxed)
        {
            return false;
        }
        let open = !self.open.fetch_xor(true, Ordering::Relaxed);
        if let Some(popup) = *self.popup.lock().expect("combo popup")
            && let Ok(Some(parent)) = context.scene().parent_id(popup)
        {
            if open {
                context.effects().bring_child_to_front_later(parent, popup);
            }
            repaint_parent(context, parent);
        }
        true
    }

    fn on_key_pressed(&self, event: &KeyEvent, context: &mut EventContext<'_>) -> bool {
        if !self.enabled.load(Ordering::Relaxed) || self.options.is_empty() {
            return false;
        }
        let selected = self.selected.load(Ordering::Relaxed);
        let next = match event.key {
            Key::ArrowDown => (selected + 1).min(self.options.len() - 1),
            Key::ArrowUp => selected.saturating_sub(1),
            Key::Escape => {
                self.open.store(false, Ordering::Relaxed);
                if let Some(popup) = *self.popup.lock().expect("combo popup")
                    && let Ok(Some(parent)) = context.scene().parent_id(popup)
                {
                    repaint_parent(context, parent);
                }
                return true;
            }
            _ => return false,
        };
        self.selected.store(next, Ordering::Relaxed);
        replace_control_label(
            context,
            self.options[next].clone(),
            self.bounds.width - 22.0,
        );
        (self.action)(next, context);
        true
    }
}

struct ComboPopupFigure {
    bounds: Rectangle,
    control: FigureId,
    options: Arc<Vec<String>>,
    layouts: Vec<TextLayout>,
    selected: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
    open: Arc<AtomicBool>,
    action: ChoiceAction,
}

impl Figure for ComboPopupFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "ComboPopup"
    }
    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        self.open.load(Ordering::Relaxed)
            && self.enabled.load(Ordering::Relaxed)
            && x >= 0.0
            && y >= 0.0
            && x < bounds.width
            && y < bounds.height
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        if !self.open.load(Ordering::Relaxed) || !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }
        context.set_fill_paint(Color::WHITE);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        for (index, layout) in self.layouts.iter().enumerate() {
            let y = index as f64 * CONTROL_HEIGHT;
            if self.selected.load(Ordering::Relaxed) == index {
                context.set_fill_paint(Color::rgba(0.75, 0.85, 1.0, 1.0));
                context.fill_rect(Rectangle::new(0.0, y, bounds.width, CONTROL_HEIGHT))?;
            }
            context.set_fill_paint(Color::BLACK);
            context.fill_text(layout, Point::new(4.0, y + 2.0))?;
        }
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for ComboPopupFigure {
    fn wants_mouse_events(&self) -> bool {
        self.open.load(Ordering::Relaxed) && self.enabled.load(Ordering::Relaxed)
    }
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let index = (event.y / CONTROL_HEIGHT) as usize;
        if let Some(value) = self.options.get(index) {
            self.selected.store(index, Ordering::Relaxed);
            self.open.store(false, Ordering::Relaxed);
            replace_text_value_later(
                context,
                self.control,
                value.clone(),
                self.bounds.width - 22.0,
            );
            (self.action)(index, context);
            if let Ok(Some(parent)) = context.scene().parent_id(context.target_id()) {
                repaint_parent(context, parent);
            }
        }
        true
    }
}

fn repaint_parent(context: &mut EventContext<'_>, parent: FigureId) {
    if let Ok(bounds) = context.scene().figure_bounds(parent) {
        context.effects().repaint_figure(
            parent,
            Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
        );
    }
}

pub struct TextFieldFigure {
    bounds: Rectangle,
    text: Arc<Mutex<String>>,
    enabled: Arc<AtomicBool>,
    replace_on_first_character: AtomicBool,
    action: TextAction,
}

impl TextFieldFigure {
    pub fn new(
        bounds: Rectangle,
        text: Arc<Mutex<String>>,
        enabled: Arc<AtomicBool>,
        action: impl for<'a> Fn(String, &mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            text,
            enabled,
            replace_on_first_character: AtomicBool::new(true),
            action: Arc::new(action),
        }
    }

    fn commit(&self, context: &mut EventContext<'_>) {
        (self.action)(self.text.lock().expect("text field").clone(), context);
    }
}

impl Figure for TextFieldFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Text"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Text),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(if self.enabled.load(Ordering::Relaxed) {
            Color::WHITE
        } else {
            Color::rgba(0.88, 0.88, 0.88, 1.0)
        });
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for TextFieldFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for TextFieldFigure {
    fn wants_mouse_events(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, _context: &mut EventContext<'_>) -> bool {
        event.button == MouseButton::Left && self.enabled.load(Ordering::Relaxed)
    }

    fn on_key_pressed(&self, event: &KeyEvent, context: &mut EventContext<'_>) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return false;
        }
        match event.key {
            Key::Character(character) => {
                let mut text = self.text.lock().expect("text field");
                if self
                    .replace_on_first_character
                    .swap(false, Ordering::Relaxed)
                {
                    text.clear();
                }
                text.push(character);
                replace_control_label(context, text.clone(), self.bounds.width - 4.0);
                true
            }
            Key::Enter => {
                self.commit(context);
                self.replace_on_first_character
                    .store(true, Ordering::Relaxed);
                true
            }
            Key::Escape => {
                self.replace_on_first_character
                    .store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    fn on_focus_lost(
        &self,
        _event: &novadraw::event::FocusEvent,
        context: &mut EventContext<'_>,
    ) -> bool {
        if self.enabled.load(Ordering::Relaxed) {
            self.commit(context);
        }
        self.replace_on_first_character
            .store(true, Ordering::Relaxed);
        true
    }
}

pub struct CheckBoxFigure {
    bounds: Rectangle,
    selected: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    action: BoolAction,
}

impl CheckBoxFigure {
    pub fn new_with_enabled(
        bounds: Rectangle,
        selected: Arc<AtomicBool>,
        enabled: Arc<AtomicBool>,
        action: impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            selected,
            enabled,
            action: Arc::new(action),
        }
    }
}

impl Figure for CheckBoxFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "CheckBox"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Pointer),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        _bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let indicator = Rectangle::new(
            1.0,
            (CONTROL_HEIGHT - INDICATOR_SIZE) / 2.0,
            INDICATOR_SIZE,
            INDICATOR_SIZE,
        );
        let enabled = self.enabled.load(Ordering::Relaxed);
        context.set_fill_paint(if enabled {
            Color::WHITE
        } else {
            Color::rgba(0.88, 0.88, 0.88, 1.0)
        });
        context.fill_rect(indicator)?;
        context.set_stroke_paint(if enabled {
            Color::BLACK
        } else {
            Color::rgba(0.5, 0.5, 0.5, 1.0)
        });
        context.set_stroke_width(1.0)?;
        context.stroke_rect(indicator)?;
        if self.selected.load(Ordering::Relaxed) {
            context.set_stroke_width(2.0)?;
            context.stroke_polyline(&[
                Point::new(3.0, 11.0),
                Point::new(7.0, 15.0),
                Point::new(14.0, 6.0),
            ])?;
        }
        Ok(())
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for CheckBoxFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for CheckBoxFigure {
    fn wants_mouse_events(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left || !self.enabled.load(Ordering::Relaxed) {
            return false;
        }
        let selected = !self.selected.fetch_xor(true, Ordering::Relaxed);
        (self.action)(selected, context);
        context.effects().repaint(None);
        true
    }
}

pub struct RadioButtonFigure {
    bounds: Rectangle,
    group: Arc<AtomicUsize>,
    value: usize,
    action: ChoiceAction,
}

impl RadioButtonFigure {
    pub fn new(
        bounds: Rectangle,
        group: Arc<AtomicUsize>,
        value: usize,
        action: impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            group,
            value,
            action: Arc::new(action),
        }
    }
}

impl Figure for RadioButtonFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RadioButton"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Pointer),
            ..FigureStyle::default()
        }
    }

    fn initial_focusable(&self) -> bool {
        true
    }

    fn initial_focus_traversable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        _bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let indicator = Rectangle::new(
            1.0,
            (CONTROL_HEIGHT - INDICATOR_SIZE) / 2.0,
            INDICATOR_SIZE,
            INDICATOR_SIZE,
        );
        context.set_fill_paint(Color::WHITE);
        context.fill_ellipse(indicator)?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_ellipse(indicator)?;
        if self.group.load(Ordering::Relaxed) == self.value {
            context.set_fill_paint(Color::BLACK);
            context.fill_ellipse(indicator.inflate(-4.0, -4.0))?;
        }
        Ok(())
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for RadioButtonFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for RadioButtonFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        self.group.store(self.value, Ordering::Relaxed);
        (self.action)(self.value, context);
        let bounds = context.target_bounds();
        if let Ok(Some(parent)) = context.scene().parent_id(context.target_id()) {
            context.effects().repaint_figure(
                parent,
                Rectangle::new(0.0, 0.0, bounds.width.max(180.0), 120.0),
            );
        }
        true
    }
}

pub struct ScaleFigure {
    bounds: Rectangle,
    minimum: f64,
    maximum: f64,
    value: Arc<Mutex<f64>>,
    action: ScaleAction,
    increment: Option<f64>,
}

impl ScaleFigure {
    pub fn new(
        bounds: Rectangle,
        minimum: f64,
        maximum: f64,
        value: Arc<Mutex<f64>>,
        action: impl for<'a> Fn(f64, &mut EventContext<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            bounds,
            minimum,
            maximum,
            value,
            action: Arc::new(action),
            increment: None,
        }
    }

    pub fn with_increment(mut self, increment: f64) -> Self {
        assert!(increment.is_finite() && increment > 0.0);
        self.increment = Some(increment);
        self
    }

    fn set_value(&self, value: f64, context: &mut EventContext<'_>) {
        let value = if self.increment.is_some() {
            value.round()
        } else {
            value
        };
        let value = value.clamp(self.minimum, self.maximum);
        *self.value.lock().expect("scale value") = value;
        (self.action)(value, context);
        context.effects().repaint(None);
    }

    fn update(&self, x: f64, context: &mut EventContext<'_>) {
        let usable = (context.target_bounds().width - 12.0).max(1.0);
        let ratio = ((x - 6.0) / usable).clamp(0.0, 1.0);
        let value = self.minimum + ratio * (self.maximum - self.minimum);
        self.set_value(value, context);
    }
}

impl Figure for ScaleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Scale"
    }

    fn initial_focusable(&self) -> bool {
        self.increment.is_some()
    }

    fn initial_focus_traversable(&self) -> bool {
        self.increment.is_some()
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            cursor: Some(CursorIcon::Crosshair),
            ..FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let center_y = bounds.height / 2.0;
        context.set_stroke_paint(Color::rgba(0.35, 0.35, 0.35, 1.0));
        context.set_stroke_width(2.0)?;
        context.stroke_line(
            Point::new(6.0, center_y),
            Point::new(bounds.width - 6.0, center_y),
        )?;
        let value = *self.value.lock().expect("scale value");
        let ratio = if self.maximum > self.minimum {
            (value - self.minimum) / (self.maximum - self.minimum)
        } else {
            0.0
        };
        let x = 6.0 + ratio.clamp(0.0, 1.0) * (bounds.width - 12.0);
        context.set_fill_paint(Color::rgba(0.82, 0.82, 0.82, 1.0));
        context.fill_rect(Rectangle::new(x - 4.0, 2.0, 8.0, bounds.height - 4.0))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(x - 4.0, 2.0, 8.0, bounds.height - 4.0))
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for ScaleFigure {
    fn on_key_pressed(&self, event: &KeyEvent, context: &mut EventContext<'_>) -> bool {
        let Some(increment) = self.increment else {
            return false;
        };
        let delta = match event.key {
            Key::ArrowRight | Key::ArrowDown => increment,
            Key::ArrowLeft | Key::ArrowUp => -increment,
            _ => return false,
        };
        let value = *self.value.lock().expect("scale value");
        self.set_value(value + delta, context);
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        self.update(event.x, context);
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        self.update(event.x, context);
        true
    }
}

pub fn add_checkbox(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    text: &str,
    selected: Arc<AtomicBool>,
    action: impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    add_checkbox_enabled(
        runtime,
        parent,
        bounds,
        text,
        selected,
        Arc::new(AtomicBool::new(true)),
        action,
    )
}

pub fn add_checkbox_enabled(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    text: &str,
    selected: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    action: impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    let control = runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(CheckBoxFigure::new_with_enabled(
            bounds, selected, enabled, action,
        )))
        .expect("CheckBox");
    add_control_label(runtime, control, text, bounds.width);
    control
}

pub fn add_radio(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    text: &str,
    group: Arc<AtomicUsize>,
    value: usize,
    action: impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    let control = runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(RadioButtonFigure::new(
            bounds, group, value, action,
        )))
        .expect("RadioButton");
    add_control_label(runtime, control, text, bounds.width);
    control
}

pub fn add_scale(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    minimum: f64,
    maximum: f64,
    value: Arc<Mutex<f64>>,
    action: impl for<'a> Fn(f64, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(ScaleFigure::new(
            bounds, minimum, maximum, value, action,
        )))
        .expect("Scale")
}

pub fn add_button(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    text: &str,
    action: impl for<'a> Fn(&mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    let control = runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(PushButtonFigure::new(bounds, action)))
        .expect("Button");
    add_control_label(runtime, control, text, bounds.width);
    control
}

pub fn add_combo(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    options: impl IntoIterator<Item = impl Into<String>>,
    selected: Arc<AtomicUsize>,
    action: impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    add_combo_enabled(
        runtime,
        parent,
        bounds,
        options,
        selected,
        Arc::new(AtomicBool::new(true)),
        action,
    )
}

pub fn add_combo_enabled(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    options: impl IntoIterator<Item = impl Into<String>>,
    selected: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
    action: impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    let options = Arc::new(options.into_iter().map(Into::into).collect::<Vec<_>>());
    let popup = Arc::new(Mutex::new(None));
    let open = Arc::new(AtomicBool::new(false));
    let action: ChoiceAction = Arc::new(action);
    let combo_action = action.clone();
    let initial = options
        .get(selected.load(Ordering::Relaxed))
        .cloned()
        .unwrap_or_default();
    let control = runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(ComboBoxFigure::new_with_enabled(
            bounds,
            options.clone(),
            selected.clone(),
            enabled.clone(),
            popup.clone(),
            open.clone(),
            move |index, context| (combo_action)(index, context),
        )))
        .expect("Combo");
    add_text_value(runtime, control, &initial, bounds.width - 22.0);
    let font = FontDescriptor::parse("13px Inter Variable").expect("combo font");
    let layouts = options
        .iter()
        .map(|option| {
            runtime
                .text()
                .layout(option, &font, TextConstraints::UNBOUNDED)
                .expect("combo option text")
        })
        .collect();
    let popup_id = runtime
        .figure(parent)
        .expect("combo parent")
        .add(Box::new(ComboPopupFigure {
            bounds: Rectangle::new(
                bounds.x,
                bounds.y + bounds.height,
                bounds.width,
                options.len() as f64 * CONTROL_HEIGHT,
            ),
            control,
            options,
            layouts,
            selected,
            enabled,
            open,
            action,
        }))
        .expect("combo popup");
    *popup.lock().expect("combo popup") = Some(popup_id);
    control
}

pub fn add_text_field(
    runtime: &mut Runtime,
    parent: FigureId,
    bounds: Rectangle,
    text: Arc<Mutex<String>>,
    enabled: Arc<AtomicBool>,
    action: impl for<'a> Fn(String, &mut EventContext<'a>) + Send + Sync + 'static,
) -> FigureId {
    let initial = text.lock().expect("text field").clone();
    let control = runtime
        .figure(parent)
        .expect("control parent")
        .add(Box::new(TextFieldFigure::new(
            bounds, text, enabled, action,
        )))
        .expect("Text");
    add_text_value(runtime, control, &initial, bounds.width - 4.0);
    control
}

fn add_control_label(runtime: &mut Runtime, control: FigureId, text: &str, width: f64) {
    let label = runtime
        .figure(control)
        .expect("control")
        .add(Box::new(LabelFigure::new(text).with_bounds(
            Rectangle::new(LABEL_X, 0.0, (width - LABEL_X).max(0.0), CONTROL_HEIGHT),
        )))
        .expect("control label");
    runtime
        .figure(label)
        .expect("control label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("control label style");
}

fn add_text_value(runtime: &mut Runtime, control: FigureId, text: &str, width: f64) {
    let label = runtime
        .figure(control)
        .expect("control")
        .add(Box::new(LabelFigure::new(text).with_bounds(
            Rectangle::new(2.0, 0.0, width.max(0.0), CONTROL_HEIGHT),
        )))
        .expect("control value");
    runtime
        .figure(label)
        .expect("control value")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("control value style");
}

fn replace_control_label(context: &mut EventContext<'_>, text: String, width: f64) {
    replace_text_value_later(context, context.target_id(), text, width);
}

pub fn replace_text_value_later(
    context: &mut EventContext<'_>,
    control: FigureId,
    text: String,
    width: f64,
) {
    if let Ok(children) = context.scene().children(control) {
        for child in children {
            context.effects().remove_child_later(control, child);
        }
    }
    context.effects().add_child_later(
        control,
        Box::new(LabelFigure::new(text).with_bounds(Rectangle::new(
            2.0,
            0.0,
            width.max(0.0),
            CONTROL_HEIGHT,
        ))),
    );
}
