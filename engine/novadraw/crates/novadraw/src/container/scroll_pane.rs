use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};

use crate::figure::{Border, BorderedFigure, Bounded, Figure, FigureEventHandler};
use crate::layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery};
use crate::runtime::update::property::standard as property;
use crate::{
    EventContext, FigureId, FigureMeasurement, FigureTree, FigureTreeBuilder, GraphMutationError,
    MeasureConstraints, RangeModel, ScrollDeltaKind, UpdateManager, ViewportError, ViewportHandle,
    WheelEvent,
};

const DEFAULT_SCROLL_BAR_THICKNESS: f64 = 14.0;
pub(crate) const DEFAULT_STEP_INCREMENT: f64 = 24.0;
const PANE_BACKGROUND: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollBarVisibility {
    Never,
    Automatic,
    Always,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScrollPaneError {
    Graph(GraphMutationError),
    Layout(LayoutError),
    Viewport(ViewportError),
    MissingPane,
}

impl fmt::Display for ScrollPaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(f),
            Self::Layout(error) => error.fmt(f),
            Self::Viewport(error) => error.fmt(f),
            Self::MissingPane => write!(f, "scroll pane block does not exist"),
        }
    }
}

impl Error for ScrollPaneError {}

impl From<GraphMutationError> for ScrollPaneError {
    fn from(value: GraphMutationError) -> Self {
        Self::Graph(value)
    }
}

impl From<ViewportError> for ScrollPaneError {
    fn from(value: ViewportError) -> Self {
        Self::Viewport(value)
    }
}

impl From<LayoutError> for ScrollPaneError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

struct ScrollPaneRuntime {
    pane_bounds: Rectangle,
    viewport_id: Option<FigureId>,
    viewport_bounds: Rectangle,
    horizontal_visibility: ScrollBarVisibility,
    vertical_visibility: ScrollBarVisibility,
    scroll_bar_thickness: f64,
}

impl ScrollPaneRuntime {
    fn new(
        bounds: Rectangle,
        _horizontal: Arc<dyn RangeModel>,
        _vertical: Arc<dyn RangeModel>,
    ) -> Self {
        Self {
            pane_bounds: bounds,
            viewport_id: None,
            viewport_bounds: bounds,
            horizontal_visibility: ScrollBarVisibility::Automatic,
            vertical_visibility: ScrollBarVisibility::Automatic,
            scroll_bar_thickness: DEFAULT_SCROLL_BAR_THICKNESS,
        }
    }
}

#[derive(Clone)]
pub struct ScrollPaneHandle {
    pane_id: FigureId,
    viewport: ViewportHandle,
    horizontal_scroll_bar: FigureId,
    vertical_scroll_bar: FigureId,
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
}

impl ScrollPaneHandle {
    pub fn pane_id(&self) -> FigureId {
        self.pane_id
    }

    pub fn viewport(&self) -> &ViewportHandle {
        &self.viewport
    }

    pub fn horizontal_scroll_bar(&self) -> FigureId {
        self.horizontal_scroll_bar
    }

    pub fn vertical_scroll_bar(&self) -> FigureId {
        self.vertical_scroll_bar
    }

    pub(crate) fn set_scroll_bar_visibility(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        horizontal: ScrollBarVisibility,
        vertical: ScrollBarVisibility,
    ) -> Result<bool, ScrollPaneError> {
        if !graph.contains(self.pane_id) {
            return Err(ScrollPaneError::MissingPane);
        }
        let changed = {
            let mut runtime = lock_unpoisoned(&self.runtime);
            if runtime.horizontal_visibility == horizontal
                && runtime.vertical_visibility == vertical
            {
                false
            } else {
                runtime.horizontal_visibility = horizontal;
                runtime.vertical_visibility = vertical;
                true
            }
        };
        if changed {
            graph.mark_invalid(update_manager, self.pane_id);
            graph.repaint(update_manager, self.pane_id, None);
        }
        Ok(changed)
    }

    pub(crate) fn scroll_to(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        x: f64,
        y: f64,
    ) -> Result<bool, ScrollPaneError> {
        Ok(self
            .viewport
            .set_view_location(graph, update_manager, x, y)?)
    }

    pub(crate) fn set_contents(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, ScrollPaneError> {
        Ok(self.viewport.set_contents(graph, update_manager, figure)?)
    }
}

#[derive(Clone)]
pub struct ScrollPaneFigure {
    bounds: Rectangle,
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
    border: Option<Arc<dyn Border>>,
}

impl ScrollPaneFigure {
    fn new(bounds: Rectangle, runtime: Arc<Mutex<ScrollPaneRuntime>>) -> Self {
        Self {
            bounds,
            runtime,
            border: None,
        }
    }
}

impl Bounded for ScrollPaneFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
        lock_unpoisoned(&self.runtime).pane_bounds = self.bounds;
    }

    fn name(&self) -> &'static str {
        "ScrollPaneFigure"
    }
}

impl Figure for ScrollPaneFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ScrollPaneFigure"
    }

    fn initial_insets(&self) -> Insets {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or_default()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(PANE_BACKGROUND);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::INPUT, crate::InputCapability::of::<Self>())?;
        out.register(crate::BORDER, crate::BorderCapability::of::<Self>())
    }
}

impl BorderedFigure for ScrollPaneFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

impl FigureEventHandler for ScrollPaneFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }
    fn on_key_pressed(&self, event: &crate::KeyEvent, ctx: &mut EventContext<'_>) -> bool {
        let mirrored = ctx
            .scene()
            .layout_direction(ctx.target_id())
            .is_ok_and(crate::figure::LayoutDirection::is_mirrored);
        let horizontal = match event.key {
            crate::Key::ArrowLeft => Some(-DEFAULT_STEP_INCREMENT),
            crate::Key::ArrowRight => Some(DEFAULT_STEP_INCREMENT),
            _ => None,
        };
        let (x, y) = if let Some(delta) = horizontal {
            (if mirrored { -delta } else { delta }, 0.0)
        } else {
            match event.key {
                crate::Key::ArrowUp => (0.0, -DEFAULT_STEP_INCREMENT),
                crate::Key::ArrowDown => (0.0, DEFAULT_STEP_INCREMENT),
                _ => return false,
            }
        };
        let Some(viewport) = lock_unpoisoned(&self.runtime).viewport_id else {
            return false;
        };
        ctx.effects().scroll_viewport_by_later(viewport, x, y);
        true
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, ctx: &mut EventContext<'_>) -> bool {
        let runtime = lock_unpoisoned(&self.runtime);
        let Some(viewport) = runtime.viewport_id else {
            return false;
        };
        let factor = match event.delta_kind {
            ScrollDeltaKind::Lines => DEFAULT_STEP_INCREMENT,
            ScrollDeltaKind::LogicalPixels => 1.0,
        };
        let horizontal_sign = if ctx
            .scene()
            .layout_direction(ctx.target_id())
            .is_ok_and(crate::figure::LayoutDirection::is_mirrored)
        {
            -1.0
        } else {
            1.0
        };
        ctx.effects().scroll_viewport_by_later(
            viewport,
            -event.delta_x * factor * horizontal_sign,
            -event.delta_y * factor,
        );
        let target_id = ctx.target_id();
        ctx.effects().repaint_figure(
            target_id,
            Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height),
        );
        true
    }
}

struct PaneScrollValue {
    pane_id: FigureId,
    orientation: ScrollOrientation,
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
}

impl super::ScrollBarValueConsumer for PaneScrollValue {
    fn request_value(&self, value: f64, relative: bool, context: &mut EventContext<'_>) -> bool {
        let runtime = lock_unpoisoned(&self.runtime);
        let Some(viewport) = runtime.viewport_id else {
            return false;
        };
        context.effects().set_viewport_axis_later(
            viewport,
            self.orientation == ScrollOrientation::Horizontal,
            value,
            relative,
        );
        true
    }

    fn value_changed(&self, old: f64, new: f64, context: &mut EventContext<'_>) {
        let runtime = lock_unpoisoned(&self.runtime);
        if let Some(viewport) = runtime.viewport_id {
            let key = match self.orientation {
                ScrollOrientation::Horizontal => property::HORIZONTAL_VIEW_LOCATION,
                ScrollOrientation::Vertical => property::VERTICAL_VIEW_LOCATION,
            };
            context
                .effects()
                .emit_property_change(viewport, key, old, new);
            context
                .effects()
                .coordinate_system_changed(viewport, runtime.viewport_bounds);
            context.effects().repaint_figure(
                viewport,
                Rectangle::new(
                    0.0,
                    0.0,
                    runtime.viewport_bounds.width,
                    runtime.viewport_bounds.height,
                ),
            );
        }
        context.effects().repaint_figure(
            self.pane_id,
            Rectangle::new(
                0.0,
                0.0,
                runtime.pane_bounds.width,
                runtime.pane_bounds.height,
            ),
        );
    }
}

#[derive(Clone)]
pub struct ScrollPaneLayout {
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
    viewport: FigureId,
    horizontal_scroll_bar: FigureId,
    vertical_scroll_bar: FigureId,
}

impl LayoutManager for ScrollPaneLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let viewport_size = snapshot
            .preferred_measurement(self.viewport, constraints)?
            .size();
        let runtime = lock_unpoisoned(&self.runtime);
        let width = viewport_size.width
            + if runtime.vertical_visibility == ScrollBarVisibility::Never {
                0.0
            } else {
                runtime.scroll_bar_thickness
            };
        let height = viewport_size.height
            + if runtime.horizontal_visibility == ScrollBarVisibility::Never {
                0.0
            } else {
                runtime.scroll_bar_thickness
            };
        Ok(FigureMeasurement::new(width, height, None))
    }

    fn minimum_size(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let area = snapshot.container_bounds(container)?;
        let preferred = snapshot
            .preferred_measurement(
                self.viewport,
                MeasureConstraints::bounded(area.width, area.height)
                    .expect("scroll pane client area is valid layout geometry"),
            )?
            .size();
        let (h_policy, v_policy, thickness) = {
            let runtime = lock_unpoisoned(&self.runtime);
            (
                runtime.horizontal_visibility,
                runtime.vertical_visibility,
                runtime.scroll_bar_thickness,
            )
        };

        let mut show_h = h_policy == ScrollBarVisibility::Always;
        let mut show_v = v_policy == ScrollBarVisibility::Always;
        for _ in 0..2 {
            let viewport_width = (area.width - if show_v { thickness } else { 0.0 }).max(0.0);
            let viewport_height = (area.height - if show_h { thickness } else { 0.0 }).max(0.0);
            show_h = h_policy != ScrollBarVisibility::Never
                && (h_policy == ScrollBarVisibility::Always || preferred.width > viewport_width);
            show_v = v_policy != ScrollBarVisibility::Never
                && (v_policy == ScrollBarVisibility::Always || preferred.height > viewport_height);
        }

        let viewport_width = (area.width - if show_v { thickness } else { 0.0 }).max(0.0);
        let viewport_height = (area.height - if show_h { thickness } else { 0.0 }).max(0.0);
        let mirrored = snapshot.layout_direction(container)?.is_mirrored();
        let viewport_x = area.x + if show_v && mirrored { thickness } else { 0.0 };
        let viewport_bounds = Rectangle::new(viewport_x, area.y, viewport_width, viewport_height);
        out.set_child_bounds(self.viewport, viewport_bounds);
        out.set_child_visible(self.horizontal_scroll_bar, show_h);
        out.set_child_visible(self.vertical_scroll_bar, show_v);
        if show_h {
            out.set_child_bounds(
                self.horizontal_scroll_bar,
                Rectangle::new(
                    viewport_x,
                    area.y + viewport_height,
                    viewport_width,
                    thickness,
                ),
            );
        }
        if show_v {
            out.set_child_bounds(
                self.vertical_scroll_bar,
                Rectangle::new(
                    if mirrored {
                        area.x
                    } else {
                        area.x + viewport_width
                    },
                    area.y,
                    thickness,
                    viewport_height,
                ),
            );
        }
        let mut runtime = lock_unpoisoned(&self.runtime);
        runtime.pane_bounds = area;
        runtime.viewport_bounds = viewport_bounds;
        Ok(())
    }
}

impl FigureTree {
    pub(crate) fn add_scroll_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScrollPaneHandle, ScrollPaneError> {
        let horizontal_model: Arc<dyn RangeModel> = Arc::new(crate::DefaultRangeModel::default());
        let vertical_model: Arc<dyn RangeModel> = Arc::new(crate::DefaultRangeModel::default());
        self.add_scroll_pane_with_models_to(parent, bounds, horizontal_model, vertical_model)
    }

    pub(crate) fn add_scroll_pane_with_models_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
        horizontal_model: Arc<dyn RangeModel>,
        vertical_model: Arc<dyn RangeModel>,
    ) -> Result<ScrollPaneHandle, ScrollPaneError> {
        let runtime = Arc::new(Mutex::new(ScrollPaneRuntime::new(
            bounds,
            Arc::clone(&horizontal_model),
            Arc::clone(&vertical_model),
        )));
        let pane = ScrollPaneFigure::new(bounds, Arc::clone(&runtime));
        let pane_id = self.try_add_child_to(parent, Box::new(pane))?;
        let viewport = self.add_viewport_with_models_to(
            pane_id,
            bounds,
            Arc::clone(&horizontal_model),
            Arc::clone(&vertical_model),
        )?;
        lock_unpoisoned(&runtime).viewport_id = Some(viewport.figure_id());

        let mut add_bar = |orientation, model| -> Result<FigureId, ScrollPaneError> {
            let description = super::ScrollBarFigure::new(model)
                .with_config(super::ScrollBarConfig {
                    orientation,
                    step_increment: DEFAULT_STEP_INCREMENT,
                    ..super::ScrollBarConfig::default()
                })
                .expect("pane ScrollBar config")
                .with_consumer(Arc::new(PaneScrollValue {
                    pane_id,
                    orientation,
                    runtime: Arc::clone(&runtime),
                }))
                .into_subtree();
            let (inserted, _) = self.insert_subtree(
                pane_id,
                description.subtree.nodes,
                None,
                |_, _, _| Ok::<_, ScrollPaneError>(()),
                |_, error| error,
                |_, _| Ok(()),
            )?;
            Ok(inserted[0].0)
        };
        let horizontal_scroll_bar = add_bar(ScrollOrientation::Horizontal, horizontal_model)?;
        let vertical_scroll_bar = add_bar(ScrollOrientation::Vertical, vertical_model)?;
        self.replace_layout_manager(
            pane_id,
            Some(Box::new(ScrollPaneLayout {
                runtime: Arc::clone(&runtime),
                viewport: viewport.figure_id(),
                horizontal_scroll_bar,
                vertical_scroll_bar,
            })),
        );
        Ok(ScrollPaneHandle {
            pane_id,
            viewport,
            horizontal_scroll_bar,
            vertical_scroll_bar,
            runtime,
        })
    }
}

impl FigureTreeBuilder<'_> {
    /// Composes ordinary ScrollBar and Viewport consumers around caller-owned ranges.
    pub fn add_scroll_pane_with_models_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
        horizontal: Arc<dyn RangeModel>,
        vertical: Arc<dyn RangeModel>,
    ) -> Result<ScrollPaneHandle, ScrollPaneError> {
        self.tree_mut()
            .add_scroll_pane_with_models_to(parent, bounds, horizontal, vertical)
    }

    pub fn add_scroll_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScrollPaneHandle, ScrollPaneError> {
        self.tree_mut().add_scroll_pane_to(parent, bounds)
    }

    pub fn set_scroll_bar_visibility(
        &mut self,
        pane: &ScrollPaneHandle,
        horizontal: ScrollBarVisibility,
        vertical: ScrollBarVisibility,
    ) -> Result<bool, ScrollPaneError> {
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        pane.set_scroll_bar_visibility(self.tree_mut(), &mut updates, horizontal, vertical)
    }

    pub fn scroll_pane_to(
        &mut self,
        pane: &ScrollPaneHandle,
        x: f64,
        y: f64,
    ) -> Result<bool, ScrollPaneError> {
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        pane.scroll_to(self.tree_mut(), &mut updates, x, y)
    }

    pub fn set_scroll_pane_contents(
        &mut self,
        pane: &ScrollPaneHandle,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, ScrollPaneError> {
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        pane.set_contents(self.tree_mut(), &mut updates, figure)
    }
}
