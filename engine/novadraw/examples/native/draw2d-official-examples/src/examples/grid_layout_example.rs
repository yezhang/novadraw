#[cfg(test)]
use novadraw::{
    figure::LabelFigure,
    geometry::Point,
    text::{FontDescriptor, TextConstraints},
};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        CONTAINER, ChildClippingStrategy, ContainerCapability, CursorIcon, EllipseFigure, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
        FigureLifecycle, FigureLifecycleContext, FigureStyle, INPUT, InputCapability, LIFECYCLE,
        LifecycleCapability, RectangleFigure, RoundedRectangleFigure, TriangleFigure,
    },
    geometry::{Dimension, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::{FillLayout, GridAlignment, GridConstraint, GridLayout},
    runtime::Runtime,
    tree::{FigureId, FigureSubtree},
};

#[cfg(test)]
use crate::controls::{
    add_button, add_checkbox, add_checkbox_enabled, add_combo, add_combo_enabled, add_text_field,
    replace_text_value_later,
};

use super::runtime_with_fonts;
use novadraw_example_support::panel::{Panel, PanelScene, egui};

pub fn panel_scene() -> PanelScene {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::WHITE)
                .with_border(canvas_margins()),
        ))
        .expect("Grid root");
    configure_canvas_layout(&mut runtime, root);
    let state = Arc::new(GridExampleState::default());
    let content = runtime
        .figure(root)
        .expect("Grid root")
        .add(Box::new(GridContainerFigure {
            bounds: Rectangle::new(CONTENT_X, CONTENT_Y, CONTENT_WIDTH, CONTENT_HEIGHT),
            state: state.clone(),
        }))
        .expect("Grid container");
    let mut settings = GridSettings::default();
    runtime
        .figure(content)
        .expect("Grid container")
        .set_layout_manager(Box::new(settings.layout()))
        .expect("Grid layout");
    let mut sequence = ShapeSequence::default();
    for _ in 0..2 {
        let id = runtime
            .figure(content)
            .expect("Grid container")
            .add(Box::new(sequence.next(state.clone())))
            .expect("Grid shape");
        runtime
            .figure(id)
            .expect("Grid shape")
            .set_preferred_size(Dimension::new(SHAPE_SIZE, SHAPE_SIZE))
            .expect("shape size");
        state
            .constraints
            .lock()
            .expect("constraints")
            .insert(id, GridConstraint::default());
    }
    (
        runtime,
        Some(Panel::new([CONTROL_X as f32, 20.0], move |ui, runtime| {
            ui.heading("GridLayout");
            let before = settings;
            ui.horizontal(|ui| {
                ui.label("numColumns");
                let response =
                    ui.add(egui::DragValue::new(&mut settings.columns).range(1..=usize::MAX));
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("grid-columns-editor"), response.rect)
                });
                let _ = response;
            });
            ui.checkbox(&mut settings.equal_column_widths, "makeColumnsEqualWidth");
            for (label, value) in [
                ("marginWidth", &mut settings.margin_width),
                ("marginHeight", &mut settings.margin_height),
                ("horizontalSpacing", &mut settings.horizontal_spacing),
                ("verticalSpacing", &mut settings.vertical_spacing),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).range(0.0..=f64::MAX));
                });
            }
            if settings != before {
                runtime
                    .figure(content)
                    .expect("Grid container")
                    .set_layout_manager(Box::new(settings.layout()))
                    .expect("Grid layout");
            }
            ui.horizontal(|ui| {
                let add = ui.button("Add");
                #[cfg(test)]
                super::panel_test::record(ui, "Add", &add);
                if add.clicked() {
                    let figure = sequence.next(state.clone());
                    let receipt = runtime
                        .figure(content)
                        .expect("Grid container")
                        .add_subtree(FigureSubtree::new(Box::new(figure)))
                        .expect("Grid insertion");
                    state
                        .constraints
                        .lock()
                        .expect("constraints")
                        .insert(receipt.root(), GridConstraint::default());
                }
                let clear = ui.button("Clear All");
                #[cfg(test)]
                super::panel_test::record(ui, "Clear All", &clear);
                if clear.clicked() {
                    if let Some(previous) = state.selected.lock().expect("selection").take() {
                        previous.highlighted.store(false, Ordering::Relaxed);
                    }
                    let children = runtime
                        .tree()
                        .child_order(content)
                        .expect("Grid children")
                        .to_vec();
                    for child in children {
                        runtime
                            .figure(content)
                            .expect("Grid container")
                            .remove(child)
                            .expect("Grid removal");
                    }
                }
            });
            ui.separator();
            ui.label("Selected Figure's GridData");
            let selected = state
                .selected
                .lock()
                .expect("selection")
                .as_ref()
                .map(|shape| shape.id);
            ui.add_enabled_ui(selected.is_some(), |ui| {
                let mut constraint = selected
                    .and_then(|id| {
                        state
                            .constraints
                            .lock()
                            .expect("constraints")
                            .get(&id)
                            .copied()
                    })
                    .unwrap_or_default();
                let before = constraint;
                for (label, hint) in [
                    ("widthHint", &mut constraint.width_hint),
                    ("heightHint", &mut constraint.height_hint),
                ] {
                    let mut value = hint.unwrap_or(-1.0);
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let response =
                            ui.add(egui::DragValue::new(&mut value).range(-1.0..=f64::MAX));
                        #[cfg(test)]
                        super::panel_test::record(ui, label, &response);
                        let _ = response;
                    });
                    *hint = (value >= 0.0).then_some(value);
                }
                for (label, alignment) in [
                    ("horizontalAlignment", &mut constraint.horizontal_alignment),
                    ("verticalAlignment", &mut constraint.vertical_alignment),
                ] {
                    egui::ComboBox::from_label(label)
                        .selected_text(ALIGNMENT_NAMES[alignment_index(*alignment)])
                        .show_ui(ui, |ui| {
                            for (index, label) in ALIGNMENT_NAMES.iter().enumerate() {
                                ui.selectable_value(alignment, alignment_at(index), *label);
                            }
                        });
                }
                ui.horizontal(|ui| {
                    ui.label("horizontalIndent");
                    let response = ui.add(
                        egui::DragValue::new(&mut constraint.horizontal_indent)
                            .range(0.0..=f64::MAX),
                    );
                    #[cfg(test)]
                    super::panel_test::record(ui, "horizontalIndent", &response);
                    let _ = response;
                });
                for (label, span) in [
                    ("horizontalSpan", &mut constraint.horizontal_span),
                    ("verticalSpan", &mut constraint.vertical_span),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let response = ui.add(egui::DragValue::new(span).range(1..=usize::MAX));
                        #[cfg(test)]
                        super::panel_test::record(ui, label, &response);
                        let _ = response;
                    });
                }
                for (label, value) in [
                    ("grabExcessHorizontalSpace", &mut constraint.grab_horizontal),
                    ("grabExcessVerticalSpace", &mut constraint.grab_vertical),
                ] {
                    let response = ui.checkbox(value, label);
                    #[cfg(test)]
                    super::panel_test::record(ui, label, &response);
                    let _ = response;
                }
                if constraint != before
                    && let Some(id) = selected
                {
                    runtime
                        .figure(id)
                        .expect("selected Grid shape")
                        .set_layout_constraint(constraint)
                        .expect("Grid constraint");
                    state
                        .constraints
                        .lock()
                        .expect("constraints")
                        .insert(id, constraint);
                }
            });
        })),
    )
}

const WIDTH: f64 = 1200.0;
const HEIGHT: f64 = 760.0;
const CONTENT_X: f64 = 390.0;
const CONTENT_Y: f64 = 20.0;
const CONTENT_WIDTH: f64 = 790.0;
const CONTENT_HEIGHT: f64 = 720.0;
const CONTROL_X: f64 = 20.0;
#[cfg(test)]
const CONTROL_WIDTH: f64 = 350.0;
const SHAPE_SIZE: f64 = 70.0;
#[cfg(test)]
const TEXT_WIDTH: f64 = 62.0;
#[cfg(test)]
const COMBO_WIDTH: f64 = 118.0;
#[cfg(test)]
const GROUP_BACKGROUND: Color = Color::rgba(0.96, 0.96, 0.96, 1.0);
#[cfg(test)]
const GROUP_TITLE_HORIZONTAL_PADDING: f64 = 4.0;
#[cfg(test)]
const GROUP_TITLE_X: f64 = 10.0;
#[cfg(test)]
const GROUP_BORDER_INSET: f64 = 0.5;
#[cfg(test)]
const GROUP_BORDER_TOP: f64 = 8.5;
#[cfg(test)]
const CONTROL_FONT: &str = "13px Inter Variable";

fn canvas_margins() -> novadraw::figure::border::MarginBorder {
    use novadraw::figure::border::MarginBorder;

    MarginBorder::default_border().with_margins(
        CONTENT_Y,
        CONTENT_X,
        HEIGHT - CONTENT_Y - CONTENT_HEIGHT,
        WIDTH - CONTENT_X - CONTENT_WIDTH,
    )
}

fn configure_canvas_layout(runtime: &mut Runtime, root: FigureId) {
    // The first child is the Canvas; the remaining children are host test controls.
    runtime
        .figure(root)
        .expect("Grid root")
        .set_layout_manager(Box::new(FillLayout::new()))
        .expect("Grid Canvas layout");
}

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

#[cfg(test)]
#[derive(Clone)]
struct TextBinding {
    id: FigureId,
    value: Arc<Mutex<String>>,
    enabled: Arc<AtomicBool>,
}

#[cfg(test)]
#[derive(Clone)]
struct ChoiceBinding {
    id: FigureId,
    selected: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
}

#[cfg(test)]
#[derive(Clone)]
struct BoolBinding {
    id: FigureId,
    selected: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
}

#[cfg(test)]
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

#[cfg(test)]
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
    #[cfg(test)]
    controls: Mutex<Option<Arc<GridDataControls>>>,
}

impl GridExampleState {
    #[cfg(test)]
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

        #[cfg(test)]
        let constraint = *self
            .constraints
            .lock()
            .expect("GridData constraints")
            .entry(id)
            .or_default();
        #[cfg(test)]
        if let Some(controls) = self.controls.lock().expect("GridData controls").as_ref() {
            controls.populate(constraint, context);
        }
    }

    fn deselect(&self, context: &mut EventContext<'_>) {
        if let Some(previous) = self.selected.lock().expect("selected shape").take() {
            previous.highlighted.store(false, Ordering::Relaxed);
            repaint_figure(context, previous.id);
        }
        #[cfg(test)]
        if let Some(controls) = self.controls.lock().expect("GridData controls").as_ref() {
            controls.set_enabled(false, context);
        }
    }

    #[cfg(test)]
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

#[cfg(test)]
struct GroupFigure {
    bounds: Rectangle,
    title_width: f64,
}

#[cfg(test)]
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
        context.set_fill_paint(GROUP_BACKGROUND);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::rgba(0.45, 0.45, 0.45, 1.0));
        context.set_stroke_width(1.0)?;
        let right = bounds.width - GROUP_BORDER_INSET;
        let bottom = bounds.height - GROUP_BORDER_INSET;
        let gap_start = GROUP_TITLE_X - GROUP_TITLE_HORIZONTAL_PADDING;
        let gap_end = (GROUP_TITLE_X + self.title_width).min(right);
        context.stroke_line(
            Point::new(GROUP_BORDER_INSET, GROUP_BORDER_TOP),
            Point::new(gap_start, GROUP_BORDER_TOP),
        )?;
        context.stroke_line(
            Point::new(gap_end, GROUP_BORDER_TOP),
            Point::new(right, GROUP_BORDER_TOP),
        )?;
        context.stroke_line(
            Point::new(GROUP_BORDER_INSET, GROUP_BORDER_TOP),
            Point::new(GROUP_BORDER_INSET, bottom),
        )?;
        context.stroke_line(
            Point::new(right, GROUP_BORDER_TOP),
            Point::new(right, bottom),
        )?;
        context.stroke_line(
            Point::new(GROUP_BORDER_INSET, bottom),
            Point::new(right, bottom),
        )
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[cfg(test)]
struct Fixture {
    runtime: Runtime,
    content: FigureId,
    state: Arc<GridExampleState>,
    settings: Arc<Mutex<GridSettings>>,
    num_columns: TextBinding,
    group_titles: [FigureId; 4],
    controls: Arc<GridDataControls>,
    add_button: FigureId,
    clear_button: FigureId,
}

pub fn grid_layout_example() -> Runtime {
    panel_scene().0
}

#[cfg(test)]
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

    let group_titles = [
        add_group(
            &mut runtime,
            root,
            Rectangle::new(CONTROL_X, 15.0, CONTROL_WIDTH, 100.0),
            "Columns",
        ),
        add_group(
            &mut runtime,
            root,
            Rectangle::new(CONTROL_X, 120.0, CONTROL_WIDTH, 145.0),
            "Margins and Spacing",
        ),
        add_group(
            &mut runtime,
            root,
            Rectangle::new(CONTROL_X, 270.0, CONTROL_WIDTH, 70.0),
            "Children",
        ),
        add_group(
            &mut runtime,
            root,
            Rectangle::new(CONTROL_X, 345.0, CONTROL_WIDTH, 395.0),
            "Selected Figure's GridData",
        ),
    ];

    let num_columns_value = Arc::new(Mutex::new("2".to_owned()));
    let num_columns_enabled = Arc::new(AtomicBool::new(true));
    let num_columns_id = add_text_field(
        &mut runtime,
        root,
        Rectangle::new(42.0, 48.0, TEXT_WIDTH, 22.0),
        num_columns_value.clone(),
        num_columns_enabled.clone(),
        layout_text_action(settings.clone(), content, |settings, value| {
            settings.columns = parse_integer(&value, 1).max(1) as usize;
        }),
    );
    let num_columns = TextBinding {
        id: num_columns_id,
        value: num_columns_value,
        enabled: num_columns_enabled,
    };
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
        num_columns,
        group_titles,
        controls,
        add_button: add_button_id,
        clear_button: clear_button_id,
    }
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn add_group(
    runtime: &mut Runtime,
    root: FigureId,
    bounds: Rectangle,
    title: &'static str,
) -> FigureId {
    let title_width = runtime
        .text()
        .layout(
            title,
            &FontDescriptor::parse(CONTROL_FONT).expect("control font"),
            TextConstraints::UNBOUNDED,
        )
        .expect("group title layout")
        .size()
        .width
        + 2.0 * GROUP_TITLE_HORIZONTAL_PADDING;
    runtime
        .figure(root)
        .expect("root")
        .add(Box::new(GroupFigure {
            bounds,
            title_width,
        }))
        .expect("Group");
    add_caption(
        runtime,
        root,
        title,
        bounds.x + GROUP_TITLE_X,
        bounds.y,
        title_width.min(bounds.width - 20.0),
    )
}

#[cfg(test)]
fn add_caption(
    runtime: &mut Runtime,
    root: FigureId,
    text: &str,
    x: f64,
    y: f64,
    width: f64,
) -> FigureId {
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
            background: Some(GROUP_BACKGROUND),
            font: Some(novadraw::text::FontDescriptor::parse(CONTROL_FONT).unwrap()),
            ..FigureStyle::default()
        })
        .expect("Label style");
    label
}

#[cfg(test)]
fn set_text(binding: &TextBinding, value: String, context: &mut EventContext<'_>) {
    *binding.value.lock().expect("GridData text") = value.clone();
    replace_text_value_later(context, binding.id, value, TEXT_WIDTH - 4.0);
}

#[cfg(test)]
fn set_choice(binding: &ChoiceBinding, index: usize, context: &mut EventContext<'_>) {
    binding.selected.store(index, Ordering::Relaxed);
    replace_text_value_later(
        context,
        binding.id,
        ALIGNMENT_NAMES[index].to_owned(),
        COMBO_WIDTH - 22.0,
    );
}

#[cfg(test)]
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

#[cfg(test)]
fn parse_integer(value: &str, fallback: i64) -> i64 {
    value.parse().unwrap_or(fallback)
}

#[cfg(test)]
fn parse_hint(value: &str) -> Option<f64> {
    let value = parse_integer(value, 1);
    (value >= 0).then_some(value as f64)
}

#[cfg(test)]
fn hint_text(value: Option<f64>) -> String {
    value.map_or_else(|| "-1".to_owned(), numeric_text)
}

#[cfg(test)]
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
    fn native_panel_grid_data_and_clear_follow_real_selection_lifecycle() {
        let mut driver = super::super::panel_test::Driver::new("GridLayoutExample");
        let root = driver.runtime.tree().contents().unwrap();
        let content = driver.runtime.tree().child_order(root).unwrap()[0];
        let selected = driver.runtime.tree().child_order(content).unwrap()[0];
        assert!(!driver.enabled("widthHint"));
        click_at(&mut driver.runtime, CONTENT_X + 35.0, CONTENT_Y + 35.0);
        driver.draw(Vec::new());
        assert!(driver.enabled("widthHint"));
        driver.edit("widthHint", "91");
        driver.edit("heightHint", "43");
        driver.edit("horizontalIndent", "12");
        driver.edit("horizontalSpan", "2");
        driver.edit("verticalSpan", "2");
        driver.click("grabExcessHorizontalSpace");
        driver.click("grabExcessVerticalSpace");
        let constraint = *driver
            .runtime
            .tree()
            .layout_constraint::<GridConstraint>(selected)
            .unwrap()
            .unwrap();
        assert_eq!(constraint.width_hint, Some(91.0));
        assert_eq!(constraint.height_hint, Some(43.0));
        assert_eq!(constraint.horizontal_indent, 12.0);
        assert_eq!(constraint.horizontal_span, 2);
        assert_eq!(constraint.vertical_span, 2);
        assert!(constraint.grab_horizontal && constraint.grab_vertical);
        driver.click("Add");
        assert_eq!(driver.runtime.tree().child_order(content).unwrap().len(), 3);
        driver.click("Clear All");
        assert!(
            driver
                .runtime
                .tree()
                .child_order(content)
                .unwrap()
                .is_empty()
        );
        assert!(!driver.enabled("widthHint"));
        driver.click("Add");
        let added = driver.runtime.tree().child_order(content).unwrap()[0];
        assert_ne!(
            added, selected,
            "removed selection cannot alias the new figure"
        );
        let bounds = driver.runtime.tree().figure_bounds(added).unwrap();
        assert_eq!((bounds.width, bounds.height), (SHAPE_SIZE, SHAPE_SIZE));
        assert!(!driver.enabled("widthHint"));
        assert!(driver.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn viewport_resize_reflows_spanning_grabbing_shapes_and_restores_geometry() {
        let (mut runtime, _) = panel_scene();
        let root = runtime.tree().contents().unwrap();
        let content = runtime.tree().child_order(root).unwrap()[0];
        let shapes = runtime.tree().child_order(content).unwrap();
        for shape in &shapes {
            runtime
                .figure(*shape)
                .unwrap()
                .set_layout_constraint(GridConstraint {
                    horizontal_span: 2,
                    horizontal_alignment: GridAlignment::Fill,
                    grab_horizontal: true,
                    width_hint: Some(SHAPE_SIZE),
                    height_hint: Some(SHAPE_SIZE),
                    ..GridConstraint::default()
                })
                .unwrap();
        }
        let mut original = None;
        for (width, height) in [
            (WIDTH, HEIGHT),
            (800.0, 400.0),
            (1600.0, 1000.0),
            (WIDTH, HEIGHT),
        ] {
            runtime
                .frames()
                .resize_logical_viewport(width, height)
                .unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let canvas = runtime.tree().figure_bounds(content).unwrap();
            assert_eq!(
                canvas,
                Rectangle::new(
                    0.0,
                    0.0,
                    CONTENT_WIDTH + width - WIDTH,
                    CONTENT_HEIGHT + height - HEIGHT,
                )
            );
            let origin = runtime
                .tree()
                .local_to_surface_transform(content)
                .unwrap()
                .transform_point(Point::new(0.0, 0.0));
            assert_eq!(origin, Point::new(CONTENT_X, CONTENT_Y));
            let bounds: Vec<_> = shapes
                .iter()
                .map(|id| runtime.tree().figure_bounds(*id).unwrap())
                .collect();
            assert_eq!(bounds[0].width, canvas.width - 10.0);
            assert_eq!(bounds[1].width, bounds[0].width);
            assert_eq!(bounds[0].height, SHAPE_SIZE);
            assert_eq!(bounds[1].y, bounds[0].y + bounds[0].height + 5.0);
            if let Some(initial) = &original {
                if width == WIDTH {
                    assert_eq!(&bounds, initial, "resize must restore committed geometry");
                }
            } else {
                original = Some(bounds);
            }
            assert!(runtime.take_deferred_mutation_errors().is_empty());
        }
    }

    #[test]
    fn egui_numeric_edit_commits_columns_without_canvas_key_dispatch() {
        let (mut runtime, mut panel) = panel_scene();
        let panel = panel.as_mut().expect("Grid panel");
        let root = runtime.tree().contents().expect("Grid root");
        let content = runtime.tree().child_order(root).unwrap()[0];
        for _ in 0..2 {
            runtime
                .figure(content)
                .unwrap()
                .add(Box::new(RectangleFigure::new(
                    0.0, 0.0, SHAPE_SIZE, SHAPE_SIZE,
                )))
                .unwrap();
        }
        let context = egui::Context::default();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            ..Default::default()
        };
        // Build the panel, then find and focus its numeric editor through real pointer input.
        let draw = |context: &egui::Context, panel: &mut Panel, runtime: &mut Runtime, input| {
            context.run_ui(input, |ui| panel.show(ui, runtime))
        };
        draw(&context, panel, &mut runtime, input.clone());
        let position = context
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("grid-columns-editor")))
            .unwrap()
            .center();
        input.events = vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ];
        draw(&context, panel, &mut runtime, input.clone());
        input.events = vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }];
        draw(&context, panel, &mut runtime, input.clone());
        input.events = vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    command: true,
                    ctrl: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("4".to_owned()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            },
        ];
        draw(&context, panel, &mut runtime, input);
        runtime.frames().stabilize_for_query().unwrap();
        let children = runtime.tree().child_order(content).unwrap();
        let first = runtime.tree().figure_bounds(children[0]).unwrap();
        let fourth = runtime.tree().figure_bounds(children[3]).unwrap();
        assert_eq!(
            first.y, fourth.y,
            "four columns must place all four figures on one row"
        );
    }

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
    fn num_columns_accepts_digits_and_commits_the_layout_on_enter() {
        let mut fixture = fixture();
        click(&mut fixture.runtime, fixture.num_columns.id);
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Character('3'), KeyModifiers::default());
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::Enter, KeyModifiers::default());

        assert_eq!(
            fixture
                .num_columns
                .value
                .lock()
                .expect("numColumns value")
                .as_str(),
            "3"
        );
        assert_eq!(fixture.settings.lock().expect("Grid settings").columns, 3);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn group_titles_have_opaque_region_backgrounds() {
        let fixture = fixture();
        for title in fixture.group_titles {
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .resolved_style(title)
                    .expect("group title style")
                    .background,
                GROUP_BACKGROUND
            );
        }
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
