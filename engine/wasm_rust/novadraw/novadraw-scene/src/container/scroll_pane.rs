use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use novadraw_core::Color;
use novadraw_geometry::{Point, Rectangle};
use novadraw_render::NdCanvas;

use crate::figure::{Bounded, Figure, FigureEventHandler, Updatable};
use crate::layout::{LayoutContext, LayoutManager};
use crate::{
    BlockId, FigureGraph, GraphMutationError, MouseEvent, NovadrawContext, PropertyValue,
    RangeModel, ScrollDeltaKind, UpdateManager, ViewportError, ViewportHandle, WheelEvent,
};

const DEFAULT_SCROLL_BAR_THICKNESS: f64 = 14.0;
const DEFAULT_STEP_INCREMENT: f64 = 24.0;
const MINIMUM_THUMB_LENGTH: f64 = 12.0;
const PANE_BACKGROUND: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const TRACK_COLOR: Color = Color {
    r: 0.86,
    g: 0.87,
    b: 0.89,
    a: 1.0,
};
const THUMB_COLOR: Color = Color {
    r: 0.38,
    g: 0.42,
    b: 0.48,
    a: 1.0,
};
const BUTTON_COLOR: Color = Color {
    r: 0.68,
    g: 0.70,
    b: 0.74,
    a: 1.0,
};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollPaneError {
    Graph(GraphMutationError),
    Viewport(ViewportError),
    MissingPane,
}

impl fmt::Display for ScrollPaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(f),
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

struct ScrollPaneRuntime {
    pane_bounds: Rectangle,
    viewport_id: Option<BlockId>,
    viewport_bounds: Rectangle,
    horizontal: Arc<dyn RangeModel>,
    vertical: Arc<dyn RangeModel>,
    horizontal_visibility: ScrollBarVisibility,
    vertical_visibility: ScrollBarVisibility,
    scroll_bar_thickness: f64,
}

impl ScrollPaneRuntime {
    fn new(
        bounds: Rectangle,
        horizontal: Arc<dyn RangeModel>,
        vertical: Arc<dyn RangeModel>,
    ) -> Self {
        Self {
            pane_bounds: bounds,
            viewport_id: None,
            viewport_bounds: bounds,
            horizontal,
            vertical,
            horizontal_visibility: ScrollBarVisibility::Automatic,
            vertical_visibility: ScrollBarVisibility::Automatic,
            scroll_bar_thickness: DEFAULT_SCROLL_BAR_THICKNESS,
        }
    }
}

#[derive(Clone)]
pub struct ScrollPaneHandle {
    pane_id: BlockId,
    viewport: ViewportHandle,
    horizontal_scroll_bar: BlockId,
    vertical_scroll_bar: BlockId,
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
}

impl ScrollPaneHandle {
    pub fn pane_id(&self) -> BlockId {
        self.pane_id
    }

    pub fn viewport(&self) -> &ViewportHandle {
        &self.viewport
    }

    pub fn horizontal_scroll_bar(&self) -> BlockId {
        self.horizontal_scroll_bar
    }

    pub fn vertical_scroll_bar(&self) -> BlockId {
        self.vertical_scroll_bar
    }

    pub fn set_scroll_bar_visibility(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        horizontal: ScrollBarVisibility,
        vertical: ScrollBarVisibility,
    ) -> Result<bool, ScrollPaneError> {
        if graph.get_block(self.pane_id).is_none() {
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

    pub fn scroll_to(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        x: f64,
        y: f64,
    ) -> Result<bool, ScrollPaneError> {
        Ok(self
            .viewport
            .set_view_location(graph, update_manager, x, y)?)
    }

    pub fn set_contents(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        figure: Box<dyn Figure>,
    ) -> Result<BlockId, ScrollPaneError> {
        Ok(self.viewport.set_contents(graph, update_manager, figure)?)
    }
}

#[derive(Clone)]
pub struct ScrollPaneFigure {
    bounds: Rectangle,
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
}

impl ScrollPaneFigure {
    fn new(bounds: Rectangle, runtime: Arc<Mutex<ScrollPaneRuntime>>) -> Self {
        Self { bounds, runtime }
    }

    fn scroll_model(model: &dyn RangeModel, delta: f64, delta_kind: ScrollDeltaKind) -> bool {
        if !delta.is_finite() || delta == 0.0 || !model.is_enabled() {
            return false;
        }
        let distance = match delta_kind {
            ScrollDeltaKind::Lines => delta * DEFAULT_STEP_INCREMENT,
            ScrollDeltaKind::LogicalPixels => delta,
        };
        let old = model.value();
        let _ = model.set_value(old - distance);
        model.value() != old
    }

    fn notify_viewport_change(
        runtime: &ScrollPaneRuntime,
        old_location: Point,
        ctx: &mut dyn NovadrawContext,
    ) {
        let Some(viewport_id) = runtime.viewport_id else {
            return;
        };
        let new_location = Point::new(runtime.horizontal.value(), runtime.vertical.value());
        if old_location == new_location {
            return;
        }
        ctx.emit_property_change(
            viewport_id,
            "viewLocation",
            PropertyValue::Point(old_location),
            PropertyValue::Point(new_location),
        );
        ctx.coordinate_system_changed(viewport_id, runtime.viewport_bounds);
        ctx.repaint_figure(
            viewport_id,
            Rectangle::new(
                0.0,
                0.0,
                runtime.viewport_bounds.width,
                runtime.viewport_bounds.height,
            ),
        );
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

impl Updatable for ScrollPaneFigure {
    fn validate(&mut self) {}
}

impl Figure for ScrollPaneFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        gc.fill_rect(
            0.0,
            0.0,
            self.bounds.width,
            self.bounds.height,
            PANE_BACKGROUND,
        );
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for ScrollPaneFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let runtime = lock_unpoisoned(&self.runtime);
        let old_location = Point::new(runtime.horizontal.value(), runtime.vertical.value());
        let vertical_changed =
            Self::scroll_model(runtime.vertical.as_ref(), event.delta_y, event.delta_kind);
        let horizontal_changed =
            Self::scroll_model(runtime.horizontal.as_ref(), event.delta_x, event.delta_kind);
        if !vertical_changed && !horizontal_changed {
            return false;
        }
        Self::notify_viewport_change(&runtime, old_location, ctx);
        ctx.repaint_figure(
            ctx.target_id(),
            Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height),
        );
        true
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct DragState {
    armed: bool,
    pointer_start: f64,
    value_start: f64,
}

#[derive(Clone)]
pub struct ScrollBarFigure {
    bounds: Rectangle,
    orientation: ScrollOrientation,
    model: Arc<dyn RangeModel>,
    pane_id: BlockId,
    pane_runtime: Arc<Mutex<ScrollPaneRuntime>>,
    drag: Arc<Mutex<DragState>>,
}

impl ScrollBarFigure {
    fn new(
        orientation: ScrollOrientation,
        model: Arc<dyn RangeModel>,
        pane_id: BlockId,
        pane_runtime: Arc<Mutex<ScrollPaneRuntime>>,
    ) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            orientation,
            model,
            pane_id,
            pane_runtime,
            drag: Arc::new(Mutex::new(DragState::default())),
        }
    }

    fn axis_position(&self, x: f64, y: f64) -> f64 {
        match self.orientation {
            ScrollOrientation::Horizontal => x,
            ScrollOrientation::Vertical => y,
        }
    }

    fn axis_start(&self) -> f64 {
        0.0
    }

    fn axis_length(&self) -> f64 {
        match self.orientation {
            ScrollOrientation::Horizontal => self.bounds.width,
            ScrollOrientation::Vertical => self.bounds.height,
        }
    }

    fn geometry(&self) -> ScrollBarGeometry {
        ScrollBarGeometry::new(self.axis_start(), self.axis_length(), self.model.snapshot())
    }

    fn repaint_pane(&self, ctx: &mut dyn NovadrawContext) {
        let runtime = lock_unpoisoned(&self.pane_runtime);
        let pane_bounds = runtime.pane_bounds;
        ctx.repaint_figure(
            self.pane_id,
            Rectangle::new(0.0, 0.0, pane_bounds.width, pane_bounds.height),
        );
    }

    fn notify_model_change(&self, old_value: f64, ctx: &mut dyn NovadrawContext) {
        let runtime = lock_unpoisoned(&self.pane_runtime);
        let Some(viewport_id) = runtime.viewport_id else {
            return;
        };
        let property = match self.orientation {
            ScrollOrientation::Horizontal => "horizontalViewLocation",
            ScrollOrientation::Vertical => "verticalViewLocation",
        };
        let new_value = self.model.value();
        if old_value == new_value {
            return;
        }
        ctx.emit_property_change(
            viewport_id,
            property,
            PropertyValue::Number(old_value),
            PropertyValue::Number(new_value),
        );
        ctx.coordinate_system_changed(viewport_id, runtime.viewport_bounds);
        ctx.repaint_figure(
            viewport_id,
            Rectangle::new(
                0.0,
                0.0,
                runtime.viewport_bounds.width,
                runtime.viewport_bounds.height,
            ),
        );
    }
}

impl Bounded for ScrollBarFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ScrollBarFigure"
    }

    fn preferred_size(&self) -> (f64, f64) {
        match self.orientation {
            ScrollOrientation::Horizontal => (0.0, DEFAULT_SCROLL_BAR_THICKNESS),
            ScrollOrientation::Vertical => (DEFAULT_SCROLL_BAR_THICKNESS, 0.0),
        }
    }
}

impl Updatable for ScrollBarFigure {
    fn validate(&mut self) {}
}

impl Figure for ScrollBarFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        gc.fill_rect(0.0, 0.0, self.bounds.width, self.bounds.height, TRACK_COLOR);
        let geometry = self.geometry();
        let thickness = match self.orientation {
            ScrollOrientation::Horizontal => self.bounds.height,
            ScrollOrientation::Vertical => self.bounds.width,
        };
        match self.orientation {
            ScrollOrientation::Horizontal => {
                gc.fill_rect(
                    geometry.axis_start,
                    0.0,
                    geometry.button_length,
                    thickness,
                    BUTTON_COLOR,
                );
                gc.fill_rect(
                    geometry.axis_end - geometry.button_length,
                    0.0,
                    geometry.button_length,
                    thickness,
                    BUTTON_COLOR,
                );
                gc.fill_rect(
                    geometry.thumb_start,
                    0.0,
                    geometry.thumb_length,
                    thickness,
                    THUMB_COLOR,
                );
            }
            ScrollOrientation::Vertical => {
                gc.fill_rect(
                    0.0,
                    geometry.axis_start,
                    thickness,
                    geometry.button_length,
                    BUTTON_COLOR,
                );
                gc.fill_rect(
                    0.0,
                    geometry.axis_end - geometry.button_length,
                    thickness,
                    geometry.button_length,
                    BUTTON_COLOR,
                );
                gc.fill_rect(
                    0.0,
                    geometry.thumb_start,
                    thickness,
                    geometry.thumb_length,
                    THUMB_COLOR,
                );
            }
        }
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for ScrollBarFigure {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let pointer = self.axis_position(event.x, event.y);
        let geometry = self.geometry();
        let snapshot = self.model.snapshot();
        let next = if pointer < geometry.track_start {
            Some(snapshot.value - DEFAULT_STEP_INCREMENT)
        } else if pointer > geometry.track_end {
            Some(snapshot.value + DEFAULT_STEP_INCREMENT)
        } else if pointer < geometry.thumb_start {
            Some(snapshot.value - snapshot.extent.max(DEFAULT_STEP_INCREMENT))
        } else if pointer > geometry.thumb_end() {
            Some(snapshot.value + snapshot.extent.max(DEFAULT_STEP_INCREMENT))
        } else {
            let mut drag = lock_unpoisoned(&self.drag);
            *drag = DragState {
                armed: true,
                pointer_start: pointer,
                value_start: snapshot.value,
            };
            None
        };
        if let Some(value) = next {
            let old_value = self.model.value();
            let _ = self.model.set_value(value);
            self.notify_model_change(old_value, ctx);
            self.repaint_pane(ctx);
        }
        true
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        let drag = *lock_unpoisoned(&self.drag);
        if !drag.armed {
            return false;
        }
        let geometry = self.geometry();
        let draggable = (geometry.track_length - geometry.thumb_length).max(0.0);
        let value_range =
            (self.model.maximum() - self.model.extent() - self.model.minimum()).max(0.0);
        if draggable > 0.0 && value_range > 0.0 {
            let pointer_delta = self.axis_position(event.x, event.y) - drag.pointer_start;
            let old_value = self.model.value();
            let _ = self
                .model
                .set_value(drag.value_start + pointer_delta * value_range / draggable);
            self.notify_model_change(old_value, ctx);
            self.repaint_pane(ctx);
        }
        true
    }

    fn on_mouse_released(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        lock_unpoisoned(&self.drag).armed = false;
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct ScrollBarGeometry {
    axis_start: f64,
    axis_end: f64,
    button_length: f64,
    track_start: f64,
    track_end: f64,
    track_length: f64,
    thumb_start: f64,
    thumb_length: f64,
}

impl ScrollBarGeometry {
    fn new(axis_start: f64, axis_length: f64, model: crate::RangeModelSnapshot) -> Self {
        let axis_length = axis_length.max(0.0);
        let button_length = DEFAULT_SCROLL_BAR_THICKNESS.min(axis_length / 3.0);
        let axis_end = axis_start + axis_length;
        let track_start = axis_start + button_length;
        let track_end = axis_end - button_length;
        let track_length = (track_end - track_start).max(0.0);
        let span = (model.maximum - model.minimum).max(0.0);
        let thumb_length = if span <= 0.0 {
            track_length
        } else {
            (track_length * model.extent / span)
                .max(MINIMUM_THUMB_LENGTH.min(track_length))
                .min(track_length)
        };
        let value_range = (span - model.extent).max(0.0);
        let thumb_offset = if value_range <= 0.0 {
            0.0
        } else {
            (track_length - thumb_length) * (model.value - model.minimum) / value_range
        };
        Self {
            axis_start,
            axis_end,
            button_length,
            track_start,
            track_end,
            track_length,
            thumb_start: track_start + thumb_offset,
            thumb_length,
        }
    }

    fn thumb_end(self) -> f64 {
        self.thumb_start + self.thumb_length
    }
}

#[derive(Clone)]
pub struct ScrollPaneLayout {
    runtime: Arc<Mutex<ScrollPaneRuntime>>,
    viewport: BlockId,
    horizontal_scroll_bar: BlockId,
    vertical_scroll_bar: BlockId,
}

impl LayoutManager for ScrollPaneLayout {
    fn get_preferred_size(
        &self,
        _container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        let viewport_size = ctx.get_preferred_size(self.viewport, w_hint, h_hint);
        let runtime = lock_unpoisoned(&self.runtime);
        let width = viewport_size.0
            + if runtime.vertical_visibility == ScrollBarVisibility::Never {
                0.0
            } else {
                runtime.scroll_bar_thickness
            };
        let height = viewport_size.1
            + if runtime.horizontal_visibility == ScrollBarVisibility::Never {
                0.0
            } else {
                runtime.scroll_bar_thickness
            };
        (width, height)
    }

    fn get_minimum_size(
        &self,
        _container: BlockId,
        _w_hint: f64,
        _h_hint: f64,
        _ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let area = ctx.get_container_bounds(container);
        let preferred = ctx.get_preferred_size(self.viewport, area.width, area.height);
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
                && (h_policy == ScrollBarVisibility::Always || preferred.0 > viewport_width);
            show_v = v_policy != ScrollBarVisibility::Never
                && (v_policy == ScrollBarVisibility::Always || preferred.1 > viewport_height);
        }

        let viewport_width = (area.width - if show_v { thickness } else { 0.0 }).max(0.0);
        let viewport_height = (area.height - if show_h { thickness } else { 0.0 }).max(0.0);
        let viewport_bounds = Rectangle::new(area.x, area.y, viewport_width, viewport_height);
        ctx.set_child_bounds(self.viewport, viewport_bounds);
        ctx.set_child_visible(self.horizontal_scroll_bar, show_h);
        ctx.set_child_visible(self.vertical_scroll_bar, show_v);
        if show_h {
            ctx.set_child_bounds(
                self.horizontal_scroll_bar,
                Rectangle::new(area.x, area.y + viewport_height, viewport_width, thickness),
            );
        }
        if show_v {
            ctx.set_child_bounds(
                self.vertical_scroll_bar,
                Rectangle::new(area.x + viewport_width, area.y, thickness, viewport_height),
            );
        }
        let mut runtime = lock_unpoisoned(&self.runtime);
        runtime.pane_bounds = area;
        runtime.viewport_bounds = viewport_bounds;
    }
}

impl FigureGraph {
    pub fn add_scroll_pane_to(
        &mut self,
        parent: BlockId,
        bounds: Rectangle,
    ) -> Result<ScrollPaneHandle, ScrollPaneError> {
        let horizontal_model: Arc<dyn RangeModel> = Arc::new(crate::DefaultRangeModel::default());
        let vertical_model: Arc<dyn RangeModel> = Arc::new(crate::DefaultRangeModel::default());
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
        lock_unpoisoned(&runtime).viewport_id = Some(viewport.block_id());

        let horizontal_scroll_bar = self.try_add_child_to(
            pane_id,
            Box::new(ScrollBarFigure::new(
                ScrollOrientation::Horizontal,
                horizontal_model,
                pane_id,
                Arc::clone(&runtime),
            )),
        )?;
        let vertical_scroll_bar = self.try_add_child_to(
            pane_id,
            Box::new(ScrollBarFigure::new(
                ScrollOrientation::Vertical,
                vertical_model,
                pane_id,
                Arc::clone(&runtime),
            )),
        )?;
        self.set_block_layout_manager(
            pane_id,
            Box::new(ScrollPaneLayout {
                runtime: Arc::clone(&runtime),
                viewport: viewport.block_id(),
                horizontal_scroll_bar,
                vertical_scroll_bar,
            }),
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
