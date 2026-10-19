use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        CONTAINER, ChildClippingStrategy, ContainerCapability, CursorIcon, EllipseFigure, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
        FigureLifecycle, FigureLifecycleContext, FigureStyle, INPUT, InputCapability, LIFECYCLE,
        LabelFigure, LifecycleCapability, RectangleFigure, RoundedRectangleFigure, TriangleFigure,
    },
    geometry::{Dimension, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::{GridAlignment, GridConstraint, GridLayout},
    runtime::Runtime,
    tree::{FigureId, FigureSubtree},
};

use crate::controls::{
    add_button, add_checkbox, add_checkbox_enabled, add_combo, add_combo_enabled, add_text_field,
    replace_text_value_later,
};

use super::runtime_with_fonts;

const WIDTH: f64 = 1200.0;
const HEIGHT: f64 = 760.0;
const CONTENT_X: f64 = 390.0;
const CONTENT_Y: f64 = 20.0;
const CONTENT_WIDTH: f64 = 790.0;
const CONTENT_HEIGHT: f64 = 720.0;
const CONTROL_X: f64 = 20.0;
const CONTROL_WIDTH: f64 = 350.0;
const SHAPE_SIZE: f64 = 70.0;
const TEXT_WIDTH: f64 = 62.0;
const COMBO_WIDTH: f64 = 118.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct GridSettings {
    columns: usize,
    equal_column_widths: bool,
    margin_width: f64,
    margin_height: f64,
    horizontal_spacing: f64,
    vertical_spacing: f64,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            columns: 2,
            equal_column_widths: false,
            margin_width: 5.0,
            margin_height: 5.0,
            horizontal_spacing: 5.0,
            vertical_spacing: 5.0,
        }
    }
}

impl GridSettings {
    fn layout(self) -> GridLayout {
        GridLayout::new(self.columns)
            .with_equal_column_widths(self.equal_column_widths)
            .with_margins(self.margin_width, self.margin_height)
            .with_spacing(self.horizontal_spacing, self.vertical_spacing)
    }
}

#[derive(Clone)]
struct TextBinding {
    id: FigureId,
    value: Arc<Mutex<String>>,
    enabled: Arc<AtomicBool>,
}

#[derive(Clone)]
struct ChoiceBinding {
    id: FigureId,
    selected: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
}

#[derive(Clone)]
struct BoolBinding {
    id: FigureId,
    selected: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
}

struct GridDataControls {
    width_hint: TextBinding,
    height_hint: TextBinding,
    horizontal_alignment: ChoiceBinding,
    vertical_alignment: ChoiceBinding,
    horizontal_indent: TextBinding,
    horizontal_span: TextBinding,
    vertical_span: TextBinding,
    grab_horizontal: BoolBinding,
    grab_vertical: BoolBinding,
}

impl GridDataControls {
    fn all_ids(&self) -> [FigureId; 9] {
        [
            self.width_hint.id,
            self.height_hint.id,
            self.horizontal_alignment.id,
            self.vertical_alignment.id,
            self.horizontal_indent.id,
            self.horizontal_span.id,
            self.vertical_span.id,
            self.grab_horizontal.id,
            self.grab_vertical.id,
        ]
    }

    fn set_enabled(&self, enabled: bool, context: &mut EventContext<'_>) {
        for state in [
            &self.width_hint.enabled,
            &self.height_hint.enabled,
            &self.horizontal_alignment.enabled,
            &self.vertical_alignment.enabled,
            &self.horizontal_indent.enabled,
            &self.horizontal_span.enabled,
            &self.vertical_span.enabled,
            &self.grab_horizontal.enabled,
            &self.grab_vertical.enabled,
        ] {
            state.store(enabled, Ordering::Relaxed);
        }
        for id in self.all_ids() {
            repaint_control(context, id);
        }
    }

    fn populate(&self, constraint: GridConstraint, context: &mut EventContext<'_>) {
        self.set_enabled(true, context);
        set_text(&self.width_hint, hint_text(constraint.width_hint), context);
        set_text(
            &self.height_hint,
            hint_text(constraint.height_hint),
            context,
        );
        set_text(
            &self.horizontal_indent,
            numeric_text(constraint.horizontal_indent),
            context,
        );
        set_text(
            &self.horizontal_span,
            constraint.horizontal_span.to_string(),
            context,
        );
        set_text(
            &self.vertical_span,
            constraint.vertical_span.to_string(),
            context,
        );
        set_choice(
            &self.horizontal_alignment,
            alignment_index(constraint.horizontal_alignment),
            context,
        );
        set_choice(
            &self.vertical_alignment,
            alignment_index(constraint.vertical_alignment),
            context,
        );
        self.grab_horizontal
            .selected
            .store(constraint.grab_horizontal, Ordering::Relaxed);
        self.grab_vertical
            .selected
            .store(constraint.grab_vertical, Ordering::Relaxed);
        repaint_control(context, self.grab_horizontal.id);
        repaint_control(context, self.grab_vertical.id);
    }
}

#[derive(Clone)]
struct SelectedShape {
    id: FigureId,
    highlighted: Arc<AtomicBool>,
}

#[derive(Default)]
struct GridExampleState {
    selected: Mutex<Option<SelectedShape>>,
    constraints: Mutex<HashMap<FigureId, GridConstraint>>,
    controls: Mutex<Option<Arc<GridDataControls>>>,
}

impl GridExampleState {
    fn set_controls(&self, controls: Arc<GridDataControls>) {
        *self.controls.lock().expect("GridData controls") = Some(controls);
    }

    fn select(&self, id: FigureId, highlighted: Arc<AtomicBool>, context: &mut EventContext<'_>) {
        let previous = self
            .selected
            .lock()
            .expect("selected shape")
            .replace(SelectedShape {
                id,
                highlighted: highlighted.clone(),
            });
        if let Some(previous) = previous
            && previous.id != id
        {
            previous.highlighted.store(false, Ordering::Relaxed);
            repaint_figure(context, previous.id);
        }
        highlighted.store(true, Ordering::Relaxed);
        repaint_figure(context, id);

        let constraint = *self
            .constraints
            .lock()
            .expect("GridData constraints")
            .entry(id)
            .or_default();
        if let Some(controls) = self.controls.lock().expect("GridData controls").as_ref() {
            controls.populate(constraint, context);
        }
    }

    fn deselect(&self, context: &mut EventContext<'_>) {
        if let Some(previous) = self.selected.lock().expect("selected shape").take() {
            previous.highlighted.store(false, Ordering::Relaxed);
            repaint_figure(context, previous.id);
        }
        if let Some(controls) = self.controls.lock().expect("GridData controls").as_ref() {
            controls.set_enabled(false, context);
        }
    }

    fn update_constraint(
        &self,
        context: &mut EventContext<'_>,
        update: impl FnOnce(&mut GridConstraint),
    ) {
        let Some(id) = self
            .selected
            .lock()
            .expect("selected shape")
            .as_ref()
            .map(|selected| selected.id)
        else {
            return;
        };
        let constraint = {
            let mut constraints = self.constraints.lock().expect("GridData constraints");
            let constraint = constraints.entry(id).or_default();
            update(constraint);
            *constraint
        };
        context
            .effects()
            .set_layout_constraint_later(id, constraint);
    }
}

struct GridContainerFigure {
    bounds: Rectangle,
    state: Arc<GridExampleState>,
}

impl Figure for GridContainerFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Figure"
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::WHITE);
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

impl FigureContainer for GridContainerFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }
}

impl FigureEventHandler for GridContainerFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        self.state.deselect(context);
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OfficialShapeKind {
    Ellipse,
    Rectangle,
    RoundedRectangle,
    Triangle,
}

struct SelectableShapeFigure {
    bounds: Rectangle,
    kind: OfficialShapeKind,
    color: Color,
    highlighted: Arc<AtomicBool>,
    state: Arc<GridExampleState>,
}

impl SelectableShapeFigure {
    fn rendered(&self, bounds: Rectangle) -> Box<dyn Figure> {
        let stroke_width = if self.highlighted.load(Ordering::Relaxed) {
            4.0
        } else {
            1.0
        };
        match self.kind {
            OfficialShapeKind::Ellipse => Box::new(
                EllipseFigure::new_with_color(0.0, 0.0, bounds.width, bounds.height, self.color)
                    .with_stroke(Color::BLACK, stroke_width),
            ),
            OfficialShapeKind::Rectangle => Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, bounds.width, bounds.height, self.color)
                    .with_stroke(Color::BLACK, stroke_width),
            ),
            OfficialShapeKind::RoundedRectangle => Box::new(
                RoundedRectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    bounds.width,
                    bounds.height,
                    8.0,
                    self.color,
                )
                .with_stroke(Color::BLACK, stroke_width),
            ),
            OfficialShapeKind::Triangle => Box::new(
                TriangleFigure::new(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
                    .with_fill_color(self.color)
                    .with_stroke_color(Color::BLACK)
                    .with_stroke_width(stroke_width)
                    .expect("valid official line width"),
            ),
        }
    }
}

impl Figure for SelectableShapeFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        match self.kind {
            OfficialShapeKind::Ellipse => "Ellipse",
            OfficialShapeKind::Rectangle => "RectangleFigure",
            OfficialShapeKind::RoundedRectangle => "RoundedRectangle",
            OfficialShapeKind::Triangle => "Triangle",
        }
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(self.color),
            foreground: Some(Color::BLACK),
            cursor: Some(CursorIcon::Pointer),
            ..FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.rendered(bounds).paint(context, bounds)
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        self.rendered(bounds).precise_hit(x, y, bounds)
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(LIFECYCLE, LifecycleCapability::of::<Self>())
    }
}

impl FigureLifecycle for SelectableShapeFigure {
    fn on_detached(&mut self, context: FigureLifecycleContext) {
        self.state
            .constraints
            .lock()
            .expect("GridData constraints")
            .remove(&context.figure_id);
    }
}

impl FigureEventHandler for SelectableShapeFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        self.state
            .select(context.target_id(), self.highlighted.clone(), context);
        true
    }
}

#[derive(Default)]
struct ShapeSequence {
    index: usize,
}

impl ShapeSequence {
    fn next(&mut self, state: Arc<GridExampleState>) -> SelectableShapeFigure {
        let index = self.index;
        self.index = (self.index + 1) % OFFICIAL_SHAPE_KINDS.len();
        SelectableShapeFigure {
            bounds: Rectangle::new(0.0, 0.0, SHAPE_SIZE, SHAPE_SIZE),
            kind: OFFICIAL_SHAPE_KINDS[index],
            color: OFFICIAL_COLORS[index],
            highlighted: Arc::new(AtomicBool::new(false)),
            state,
        }
    }
}

const OFFICIAL_SHAPE_KINDS: [OfficialShapeKind; 9] = [
    OfficialShapeKind::Ellipse,
    OfficialShapeKind::Rectangle,
    OfficialShapeKind::RoundedRectangle,
    OfficialShapeKind::Rectangle,
    OfficialShapeKind::Ellipse,
    OfficialShapeKind::Triangle,
    OfficialShapeKind::RoundedRectangle,
    OfficialShapeKind::Triangle,
    OfficialShapeKind::RoundedRectangle,
];

const OFFICIAL_COLORS: [Color; 9] = [
    Color::rgba(0.0, 0.0, 1.0, 1.0),
    Color::rgba(1.0, 0.0, 0.0, 1.0),
    Color::rgba(1.0, 1.0, 0.0, 1.0),
    Color::rgba(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0),
    Color::rgba(0.0, 1.0, 0.0, 1.0),
    Color::rgba(127.0 / 255.0, 127.0 / 255.0, 1.0, 1.0),
    Color::rgba(0.0, 1.0, 1.0, 1.0),
    Color::rgba(0.0, 127.0 / 255.0, 0.0, 1.0),
    Color::rgba(1.0, 196.0 / 255.0, 0.0, 1.0),
];

struct GroupFigure {
    bounds: Rectangle,
}

impl Figure for GroupFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Group"
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::rgba(0.96, 0.96, 0.96, 1.0));
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::rgba(0.45, 0.45, 0.45, 1.0));
        context.set_stroke_width(1.0)?;
        context.stroke_rect(Rectangle::new(
            0.5,
            8.5,
            bounds.width - 1.0,
            bounds.height - 9.0,
        ))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    content: FigureId,
    state: Arc<GridExampleState>,
    settings: Arc<Mutex<GridSettings>>,
    controls: Arc<GridDataControls>,
    add_button: FigureId,
    clear_button: FigureId,
}

pub fn grid_layout_example() -> Runtime {
    fixture().runtime
}

fn fixture() -> Fixture {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("GridLayoutExample root");
    let state = Arc::new(GridExampleState::default());
    let settings = Arc::new(Mutex::new(GridSettings::default()));
    let content = runtime
        .figure(root)
        .expect("root")
        .add(Box::new(GridContainerFigure {
            bounds: Rectangle::new(CONTENT_X, CONTENT_Y, CONTENT_WIDTH, CONTENT_HEIGHT),
            state: state.clone(),
        }))
        .expect("GridLayout container");
    runtime
        .figure(content)
        .expect("GridLayout container")
        .set_layout_manager(Box::new(settings.lock().expect("Grid settings").layout()))
        .expect("GridLayout");

    add_group(
        &mut runtime,
        root,
        Rectangle::new(CONTROL_X, 15.0, CONTROL_WIDTH, 100.0),
        "Columns",
    );
    add_group(
        &mut runtime,
        root,
        Rectangle::new(CONTROL_X, 120.0, CONTROL_WIDTH, 145.0),
        "Margins and Spacing",
    );
    add_group(
        &mut runtime,
        root,
        Rectangle::new(CONTROL_X, 270.0, CONTROL_WIDTH, 70.0),
        "Children",
    );
    add_group(
        &mut runtime,
        root,
        Rectangle::new(CONTROL_X, 345.0, CONTROL_WIDTH, 395.0),
        "Selected Figure's GridData",
    );

    let num_columns = Arc::new(Mutex::new("2".to_owned()));
    add_text_field(
        &mut runtime,
        root,
        Rectangle::new(42.0, 48.0, TEXT_WIDTH, 22.0),
        num_columns,
        Arc::new(AtomicBool::new(true)),
        layout_text_action(settings.clone(), content, |settings, value| {
            settings.columns = parse_integer(&value, 1).max(1) as usize;
        }),
    );
    add_caption(&mut runtime, root, "numColumns", 116.0, 48.0, 220.0);
    add_checkbox(
        &mut runtime,
        root,
        Rectangle::new(42.0, 78.0, 280.0, 22.0),
        "makeColumnsEqualWidth",
        Arc::new(AtomicBool::new(false)),
        layout_bool_action(settings.clone(), content, |settings, value| {
            settings.equal_column_widths = value;
        }),
    );

    let margin_values = ["0", "3", "5", "10"];
    add_layout_combo(
        &mut runtime,
        root,
        settings.clone(),
        content,
        "marginHeight",
        148.0,
        margin_values,
        |settings, value| settings.margin_height = value,
    );
    add_layout_combo(
        &mut runtime,
        root,
        settings.clone(),
        content,
        "marginWidth",
        176.0,
        margin_values,
        |settings, value| settings.margin_width = value,
    );
    add_layout_combo(
        &mut runtime,
        root,
        settings.clone(),
        content,
        "horizontalSpacing",
        204.0,
        margin_values,
        |settings, value| settings.horizontal_spacing = value,
    );
    add_layout_combo(
        &mut runtime,
        root,
        settings.clone(),
        content,
        "verticalSpacing",
        232.0,
        margin_values,
        |settings, value| settings.vertical_spacing = value,
    );

    let sequence = Arc::new(Mutex::new(ShapeSequence::default()));
    let add_button_id = add_button(
        &mut runtime,
        root,
        Rectangle::new(42.0, 299.0, 130.0, 26.0),
        "Add",
        {
            let sequence = sequence.clone();
            let state = state.clone();
            move |context| {
                let figure = sequence
                    .lock()
                    .expect("official shape sequence")
                    .next(state.clone());
                let state = state.clone();
                context.effects().add_subtree_later(
                    content,
                    FigureSubtree::new(Box::new(figure)),
                    move |result| {
                        if let Ok(receipt) = result {
                            state
                                .constraints
                                .lock()
                                .expect("GridData constraints")
                                .insert(receipt.root(), GridConstraint::default());
                        }
                    },
                );
            }
        },
    );
    let clear_button_id = add_button(
        &mut runtime,
        root,
        Rectangle::new(190.0, 299.0, 130.0, 26.0),
        "Clear All",
        {
            let state = state.clone();
            move |context| {
                state.deselect(context);
                if let Ok(children) = context.scene().children(content) {
                    for child in children {
                        context.effects().remove_child_later(content, child);
                    }
                }
            }
        },
    );

    let controls = Arc::new(create_grid_data_controls(&mut runtime, root, state.clone()));
    state.set_controls(controls.clone());

    for _ in 0..2 {
        let figure = sequence
            .lock()
            .expect("official shape sequence")
            .next(state.clone());
        let child = runtime
            .figure(content)
            .expect("GridLayout container")
            .add(Box::new(figure))
            .expect("official GridLayout shape");
        runtime
            .figure(child)
            .expect("official GridLayout shape")
            .set_preferred_size(Dimension::new(SHAPE_SIZE, SHAPE_SIZE))
            .expect("official shape preferred size");
    }

    Fixture {
        runtime,
        content,
        state,
        settings,
        controls,
        add_button: add_button_id,
        clear_button: clear_button_id,
    }
}

fn create_grid_data_controls(
    runtime: &mut Runtime,
    root: FigureId,
    state: Arc<GridExampleState>,
) -> GridDataControls {
    let width_hint = add_grid_text(
        runtime,
        root,
        "widthHint",
        378.0,
        "-1",
        state.clone(),
        |constraint, value| constraint.width_hint = parse_hint(&value),
    );
    let height_hint = add_grid_text(
        runtime,
        root,
        "heightHint",
        408.0,
        "-1",
        state.clone(),
        |constraint, value| constraint.height_hint = parse_hint(&value),
    );
    let horizontal_alignment = add_grid_alignment(
        runtime,
        root,
        "horizontalAlignment",
        438.0,
        state.clone(),
        |constraint, alignment| constraint.horizontal_alignment = alignment,
    );
    let vertical_alignment = add_grid_alignment(
        runtime,
        root,
        "verticalAlignment",
        468.0,
        state.clone(),
        |constraint, alignment| constraint.vertical_alignment = alignment,
    );
    let horizontal_indent = add_grid_text(
        runtime,
        root,
        "horizontalIndent",
        498.0,
        "0",
        state.clone(),
        |constraint, value| {
            constraint.horizontal_indent = parse_integer(&value, 1).max(0) as f64;
        },
    );
    let horizontal_span = add_grid_text(
        runtime,
        root,
        "horizontalSpan",
        528.0,
        "1",
        state.clone(),
        |constraint, value| {
            constraint.horizontal_span = parse_integer(&value, 1).max(1) as usize;
        },
    );
    let vertical_span = add_grid_text(
        runtime,
        root,
        "verticalSpan",
        558.0,
        "1",
        state.clone(),
        |constraint, value| {
            constraint.vertical_span = parse_integer(&value, 1).max(1) as usize;
        },
    );

    let grab_horizontal_selected = Arc::new(AtomicBool::new(false));
    let grab_horizontal_enabled = Arc::new(AtomicBool::new(false));
    let grab_horizontal_id = add_checkbox_enabled(
        runtime,
        root,
        Rectangle::new(42.0, 594.0, 290.0, 22.0),
        "grabExcessHorizontalSpace",
        grab_horizontal_selected.clone(),
        grab_horizontal_enabled.clone(),
        {
            let state = state.clone();
            move |value, context| {
                state.update_constraint(context, |constraint| {
                    constraint.grab_horizontal = value;
                });
            }
        },
    );
    let grab_vertical_selected = Arc::new(AtomicBool::new(false));
    let grab_vertical_enabled = Arc::new(AtomicBool::new(false));
    let grab_vertical_id = add_checkbox_enabled(
        runtime,
        root,
        Rectangle::new(42.0, 624.0, 290.0, 22.0),
        "grabExcessVerticalSpace",
        grab_vertical_selected.clone(),
        grab_vertical_enabled.clone(),
        move |value, context| {
            state.update_constraint(context, |constraint| {
                constraint.grab_vertical = value;
            });
        },
    );

    GridDataControls {
        width_hint,
        height_hint,
        horizontal_alignment,
        vertical_alignment,
        horizontal_indent,
        horizontal_span,
        vertical_span,
        grab_horizontal: BoolBinding {
            id: grab_horizontal_id,
            selected: grab_horizontal_selected,
            enabled: grab_horizontal_enabled,
        },
        grab_vertical: BoolBinding {
            id: grab_vertical_id,
            selected: grab_vertical_selected,
            enabled: grab_vertical_enabled,
        },
    }
}

fn add_grid_text(
    runtime: &mut Runtime,
    root: FigureId,
    label: &str,
    y: f64,
    initial: &str,
    state: Arc<GridExampleState>,
    update: fn(&mut GridConstraint, String),
) -> TextBinding {
    let value = Arc::new(Mutex::new(initial.to_owned()));
    let enabled = Arc::new(AtomicBool::new(false));
    let id = add_text_field(
        runtime,
        root,
        Rectangle::new(42.0, y, TEXT_WIDTH, 22.0),
        value.clone(),
        enabled.clone(),
        move |value, context| {
            state.update_constraint(context, |constraint| update(constraint, value));
        },
    );
    add_caption(runtime, root, label, 116.0, y, 215.0);
    TextBinding { id, value, enabled }
}

fn add_grid_alignment(
    runtime: &mut Runtime,
    root: FigureId,
    label: &str,
    y: f64,
    state: Arc<GridExampleState>,
    update: fn(&mut GridConstraint, GridAlignment),
) -> ChoiceBinding {
    let selected = Arc::new(AtomicUsize::new(0));
    let enabled = Arc::new(AtomicBool::new(false));
    let id = add_combo_enabled(
        runtime,
        root,
        Rectangle::new(42.0, y, COMBO_WIDTH, 22.0),
        ["BEGINNING", "CENTER", "END", "FILL"],
        selected.clone(),
        enabled.clone(),
        move |index, context| {
            state.update_constraint(context, |constraint| {
                update(constraint, alignment_at(index));
            });
        },
    );
    add_caption(runtime, root, label, 172.0, y, 160.0);
    ChoiceBinding {
        id,
        selected,
        enabled,
    }
}

#[allow(clippy::too_many_arguments)]
fn add_layout_combo(
    runtime: &mut Runtime,
    root: FigureId,
    settings: Arc<Mutex<GridSettings>>,
    content: FigureId,
    label: &str,
    y: f64,
    options: [&str; 4],
    update: fn(&mut GridSettings, f64),
) {
    const VALUES: [f64; 4] = [0.0, 3.0, 5.0, 10.0];
    add_caption(runtime, root, label, 42.0, y, 145.0);
    add_combo(
        runtime,
        root,
        Rectangle::new(200.0, y, 120.0, 22.0),
        options,
        Arc::new(AtomicUsize::new(2)),
        move |index, context| {
            let layout = {
                let mut settings = settings.lock().expect("Grid settings");
                update(&mut settings, VALUES[index]);
                settings.layout()
            };
            context
                .effects()
                .set_layout_manager_later(content, Box::new(layout));
        },
    );
}

fn layout_text_action(
    settings: Arc<Mutex<GridSettings>>,
    content: FigureId,
    update: fn(&mut GridSettings, String),
) -> impl for<'a> Fn(String, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut settings = settings.lock().expect("Grid settings");
            update(&mut settings, value);
            settings.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

fn layout_bool_action(
    settings: Arc<Mutex<GridSettings>>,
    content: FigureId,
    update: fn(&mut GridSettings, bool),
) -> impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut settings = settings.lock().expect("Grid settings");
            update(&mut settings, value);
            settings.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

fn add_group(runtime: &mut Runtime, root: FigureId, bounds: Rectangle, title: &'static str) {
    runtime
        .figure(root)
        .expect("root")
        .add(Box::new(GroupFigure { bounds }))
        .expect("Group");
    add_caption(
        runtime,
        root,
        title,
        bounds.x + 10.0,
        bounds.y,
        bounds.width - 20.0,
    );
}

fn add_caption(runtime: &mut Runtime, root: FigureId, text: &str, x: f64, y: f64, width: f64) {
    let label = runtime
        .figure(root)
        .expect("root")
        .add(Box::new(
            LabelFigure::new(text).with_bounds(Rectangle::new(x, y, width, 22.0)),
        ))
        .expect("Label");
    runtime
        .figure(label)
        .expect("Label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .expect("Label style");
}

fn set_text(binding: &TextBinding, value: String, context: &mut EventContext<'_>) {
    *binding.value.lock().expect("GridData text") = value.clone();
    replace_text_value_later(context, binding.id, value, TEXT_WIDTH - 4.0);
}

fn set_choice(binding: &ChoiceBinding, index: usize, context: &mut EventContext<'_>) {
    binding.selected.store(index, Ordering::Relaxed);
    replace_text_value_later(
        context,
        binding.id,
        ALIGNMENT_NAMES[index].to_owned(),
        COMBO_WIDTH - 22.0,
    );
}

fn repaint_control(context: &mut EventContext<'_>, id: FigureId) {
    if let Ok(bounds) = context.scene().figure_bounds(id) {
        context
            .effects()
            .repaint_figure(id, Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
    }
}

fn repaint_figure(context: &mut EventContext<'_>, id: FigureId) {
    if let Ok(bounds) = context.scene().figure_bounds(id) {
        context
            .effects()
            .repaint_figure(id, Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
    }
}

fn parse_integer(value: &str, fallback: i64) -> i64 {
    value.parse().unwrap_or(fallback)
}

fn parse_hint(value: &str) -> Option<f64> {
    let value = parse_integer(value, 1);
    (value >= 0).then_some(value as f64)
}

fn hint_text(value: Option<f64>) -> String {
    value.map_or_else(|| "-1".to_owned(), numeric_text)
}

fn numeric_text(value: f64) -> String {
    format!("{value:.0}")
}

const ALIGNMENT_NAMES: [&str; 4] = ["BEGINNING", "CENTER", "END", "FILL"];

fn alignment_index(alignment: GridAlignment) -> usize {
    match alignment {
        GridAlignment::Start => 0,
        GridAlignment::Center => 1,
        GridAlignment::End => 2,
        GridAlignment::Fill => 3,
    }
}

fn alignment_at(index: usize) -> GridAlignment {
    match index {
        1 => GridAlignment::Center,
        2 => GridAlignment::End,
        3 => GridAlignment::Fill,
        _ => GridAlignment::Start,
    }
}

#[cfg(test)]
mod tests {
    use novadraw::{
        event::{Key, KeyModifiers, MouseButton},
        layout::GridAlignment,
    };

    use super::*;

    #[test]
    fn fixture_matches_the_official_shape_and_control_structure() {
        let fixture = fixture();
        let children = fixture
            .runtime
            .tree()
            .child_order(fixture.content)
            .expect("GridLayout children");
        let names = children
            .iter()
            .map(|id| {
                fixture
                    .runtime
                    .tree()
                    .node(*id)
                    .expect("official shape")
                    .figure_name()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, ["Ellipse", "RectangleFigure"]);
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.controls.width_hint.id)
                .expect("widthHint")
                .figure_name(),
            "Text"
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.controls.horizontal_alignment.id)
                .expect("horizontalAlignment")
                .figure_name(),
            "Combo"
        );
        assert_eq!(
            *fixture.settings.lock().expect("Grid settings"),
            GridSettings::default()
        );
        assert!(!fixture.controls.width_hint.enabled.load(Ordering::Relaxed));
    }

    #[test]
    fn selecting_a_shape_populates_and_enables_the_grid_data_editor() {
        let mut fixture = fixture();
        click_at(&mut fixture.runtime, CONTENT_X + 35.0, CONTENT_Y + 35.0);

        let selected = fixture
            .state
            .selected
            .lock()
            .expect("selected shape")
            .clone()
            .expect("shape selected");
        assert!(selected.highlighted.load(Ordering::Relaxed));
        assert!(fixture.controls.width_hint.enabled.load(Ordering::Relaxed));
        assert_eq!(
            fixture
                .controls
                .vertical_alignment
                .selected
                .load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn text_and_combo_controls_update_the_selected_grid_constraint() {
        let mut fixture = fixture();
        click_at(&mut fixture.runtime, CONTENT_X + 35.0, CONTENT_Y + 35.0);
        let selected = fixture
            .state
            .selected
            .lock()
            .expect("selected shape")
            .as_ref()
            .expect("shape selected")
            .id;

        let bounds = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.controls.width_hint.id)
            .expect("widthHint");
        click_at(
            &mut fixture.runtime,
            bounds.center().x(),
            bounds.center().y(),
        );
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Character('9'), KeyModifiers::default());
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Character('0'), KeyModifiers::default());
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Enter, KeyModifiers::default());

        let combo_bounds = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.controls.horizontal_alignment.id)
            .expect("horizontalAlignment");
        click_at(
            &mut fixture.runtime,
            combo_bounds.center().x(),
            combo_bounds.center().y(),
        );
        click_at(
            &mut fixture.runtime,
            combo_bounds.center().x(),
            combo_bounds.y + combo_bounds.height + 33.0,
        );

        let constraint = fixture
            .state
            .constraints
            .lock()
            .expect("GridData constraints")[&selected];
        assert_eq!(constraint.width_hint, Some(90.0));
        assert_eq!(constraint.horizontal_alignment, GridAlignment::Center);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn width_hint_focus_out_and_invalid_return_match_java_callback_evidence() {
        let mut fixture = fixture();
        click_at(&mut fixture.runtime, CONTENT_X + 35.0, CONTENT_Y + 35.0);
        let selected = fixture.state.selected.lock().unwrap().as_ref().unwrap().id;
        click(&mut fixture.runtime, fixture.controls.width_hint.id);
        for character in "91".chars() {
            fixture
                .runtime
                .input()
                .dispatch_key_pressed(Key::Character(character), KeyModifiers::default());
        }
        click(&mut fixture.runtime, fixture.controls.height_hint.id);
        assert_eq!(
            fixture.state.constraints.lock().unwrap()[&selected].width_hint,
            Some(91.0)
        );
        click(&mut fixture.runtime, fixture.controls.width_hint.id);
        for character in "invalid".chars() {
            fixture
                .runtime
                .input()
                .dispatch_key_pressed(Key::Character(character), KeyModifiers::default());
        }
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Enter, KeyModifiers::default());
        assert_eq!(
            fixture.state.constraints.lock().unwrap()[&selected].width_hint,
            Some(1.0)
        );
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn add_and_clear_buttons_mutate_the_official_shape_collection() {
        let mut fixture = fixture();
        click(&mut fixture.runtime, fixture.add_button);
        assert_eq!(
            fixture
                .runtime
                .tree()
                .child_order(fixture.content)
                .expect("GridLayout children")
                .len(),
            3
        );

        click(&mut fixture.runtime, fixture.clear_button);
        assert!(
            fixture
                .runtime
                .tree()
                .child_order(fixture.content)
                .expect("GridLayout children")
                .is_empty()
        );
        assert!(!fixture.controls.width_hint.enabled.load(Ordering::Relaxed));
    }

    #[test]
    fn clear_releases_grid_data_and_preserves_the_official_creation_cycle() {
        let mut fixture = fixture();
        click_at(&mut fixture.runtime, CONTENT_X + 35.0, CONTENT_Y + 35.0);
        assert_eq!(fixture.state.constraints.lock().unwrap().len(), 1);
        click(&mut fixture.runtime, fixture.clear_button);
        assert!(fixture.state.constraints.lock().unwrap().is_empty());
        assert!(fixture.state.selected.lock().unwrap().is_none());

        let mut names = Vec::new();
        for _ in 0..OFFICIAL_SHAPE_KINDS.len() {
            click(&mut fixture.runtime, fixture.add_button);
            let children = fixture.runtime.tree().child_order(fixture.content).unwrap();
            let child = *children.last().unwrap();
            names.push(fixture.runtime.tree().node(child).unwrap().figure_name());
            assert_eq!(
                fixture.state.constraints.lock().unwrap()[&child],
                GridConstraint::default()
            );
            assert_eq!(
                fixture.state.constraints.lock().unwrap().len(),
                children.len()
            );
        }
        assert_eq!(
            names,
            [
                "RoundedRectangle",
                "RectangleFigure",
                "Ellipse",
                "Triangle",
                "RoundedRectangle",
                "Triangle",
                "RoundedRectangle",
                "Ellipse",
                "RectangleFigure",
            ]
        );
        click(&mut fixture.runtime, fixture.clear_button);
        assert!(fixture.state.constraints.lock().unwrap().is_empty());
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn rejected_add_does_not_publish_grid_data_for_an_unmounted_shape() {
        let mut fixture = fixture();
        let parent = fixture
            .runtime
            .tree()
            .parent_id(fixture.content)
            .unwrap()
            .unwrap();
        fixture
            .runtime
            .figure(parent)
            .unwrap()
            .remove(fixture.content)
            .unwrap();
        click(&mut fixture.runtime, fixture.add_button);
        assert!(fixture.state.constraints.lock().unwrap().is_empty());
        assert_eq!(fixture.runtime.take_deferred_mutation_errors().len(), 1);
    }

    fn click(runtime: &mut Runtime, figure: FigureId) {
        let bounds = runtime
            .tree()
            .figure_bounds(figure)
            .expect("control bounds");
        click_at(runtime, bounds.center().x(), bounds.center().y());
    }

    fn click_at(runtime: &mut Runtime, x: f64, y: f64) {
        runtime
            .input()
            .dispatch_mouse_pressed(x, y, MouseButton::Left);
        runtime
            .input()
            .dispatch_mouse_released(x, y, MouseButton::Left);
    }
}
