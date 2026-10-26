//! Composable Draw2D ScrollBar with an independent range model.
use super::{DefaultRangeModel, RangeModel, RangeModelSnapshot, ScrollOrientation};
use crate::figure::{
    ACTIVITY, ActivityCapability, CONTAINER, ContainerCapability, Figure, FigureActivity,
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
    FigureEventHandler, INPUT, InputCapability, RectangleFigure,
};
use crate::geometry::{Dimension, Point, Rectangle};
use crate::graphics::{Color, GraphicsError, PaintContext, Path};
use crate::layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery};
use crate::runtime::{MonotonicTime, PreparedFigureUpdate};
use crate::tree::{FigureSubtree, SubtreeNodeId};
use crate::{
    EventContext, FigureComponentContext, FigureComponentUpdate, FigureId, FigureMeasurement,
    MeasureConstraints, MouseButton, MouseEvent,
};
use std::{
    convert::Infallible,
    sync::{Arc, Mutex, MutexGuard},
};

const THICKNESS: f64 = 14.0;
const MIN_THUMB: f64 = 12.0;
const INITIAL_DELAY_US: u64 = 250_000;
const REPEAT_INTERVAL_US: u64 = 40_000;
const TRACK: Color = Color::rgba(0.86, 0.87, 0.89, 1.0);
const BUTTON: Color = Color::rgba(0.68, 0.70, 0.74, 1.0);
const THUMB: Color = Color::rgba(0.38, 0.42, 0.48, 1.0);

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Layout roles for the five ordinary child Figures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollBarPart {
    Decrement,
    PageDecrement,
    Thumb,
    PageIncrement,
    Increment,
}

const PARTS: [ScrollBarPart; 5] = [
    ScrollBarPart::Decrement,
    ScrollBarPart::PageDecrement,
    ScrollBarPart::Thumb,
    ScrollBarPart::PageIncrement,
    ScrollBarPart::Increment,
];

/// Value consumption runs with the same checked effects as any Figure callback.
pub trait ScrollBarValueConsumer: Send + Sync {
    fn value_changed(&self, old: f64, new: f64, context: &mut EventContext<'_>);
    /// Returns true when the owner accepts a deferred absolute or incremental request.
    /// The range model has not been written when this callback runs.
    fn request_value(&self, _value: f64, _relative: bool, _context: &mut EventContext<'_>) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScrollBarConfig {
    pub orientation: ScrollOrientation,
    pub thickness: f64,
    pub button_length: f64,
    pub minimum_thumb_length: f64,
    pub step_increment: f64,
    pub page_increment: f64,
}

impl Default for ScrollBarConfig {
    fn default() -> Self {
        Self {
            orientation: ScrollOrientation::Vertical,
            thickness: THICKNESS,
            button_length: THICKNESS,
            minimum_thumb_length: MIN_THUMB,
            step_increment: 10.0,
            page_increment: 50.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidScrollBarConfig;

impl std::fmt::Display for InvalidScrollBarConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scrollbar dimensions and increments must be finite and positive")
    }
}
impl std::error::Error for InvalidScrollBarConfig {}

impl ScrollBarConfig {
    fn validate(self) -> Result<Self, InvalidScrollBarConfig> {
        if [
            self.thickness,
            self.button_length,
            self.minimum_thumb_length,
            self.step_increment,
            self.page_increment,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
        {
            Ok(self)
        } else {
            Err(InvalidScrollBarConfig)
        }
    }

    fn axis(self, point: Point) -> f64 {
        match self.orientation {
            ScrollOrientation::Horizontal => point.x(),
            ScrollOrientation::Vertical => point.y(),
        }
    }
    fn length(self, bounds: Rectangle) -> f64 {
        match self.orientation {
            ScrollOrientation::Horizontal => bounds.width,
            ScrollOrientation::Vertical => bounds.height,
        }
    }
    fn rect(self, area: Rectangle, start: f64, length: f64) -> Rectangle {
        match self.orientation {
            ScrollOrientation::Horizontal => {
                Rectangle::new(area.x + start, area.y, length, area.height)
            }
            ScrollOrientation::Vertical => {
                Rectangle::new(area.x, area.y + start, area.width, length)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    button: f64,
    length: f64,
    thumb_start: f64,
    thumb_length: f64,
    travel: f64,
    show_thumb: bool,
}

impl Geometry {
    fn new(config: ScrollBarConfig, bounds: Rectangle, model: RangeModelSnapshot) -> Self {
        let length = config.length(bounds).max(0.0);
        let button = config.button_length.min(length / 2.0);
        let track = (length - 2.0 * button).max(0.0);
        let span = model.maximum - model.minimum;
        let thumb_length = if span > 0.0 {
            (track * model.extent / span)
                .max(config.minimum_thumb_length)
                .min(track)
        } else {
            track
        };
        let travel = (track - thumb_length).max(0.0);
        let value_range = span - model.extent;
        let offset = if value_range > 0.0 {
            travel * (model.value - model.minimum) / value_range
        } else {
            0.0
        };
        Self {
            button,
            length,
            thumb_start: button + offset,
            thumb_length,
            travel,
            show_thumb: model.is_enabled() && track >= config.minimum_thumb_length && travel > 0.0,
        }
    }
    fn role(self, pointer: f64) -> ScrollBarPart {
        if pointer < self.button {
            ScrollBarPart::Decrement
        } else if pointer >= self.length - self.button {
            ScrollBarPart::Increment
        } else if pointer < self.thumb_start {
            ScrollBarPart::PageDecrement
        } else if pointer > self.thumb_start + self.thumb_length {
            ScrollBarPart::PageIncrement
        } else {
            ScrollBarPart::Thumb
        }
    }
    fn placement(self, role: ScrollBarPart) -> (f64, f64) {
        let track_end = self.length - self.button;
        let split = if self.show_thumb {
            self.thumb_start
        } else {
            self.length / 2.0
        };
        match role {
            ScrollBarPart::Decrement => (0.0, self.button),
            ScrollBarPart::Increment => (track_end, self.button),
            ScrollBarPart::Thumb => (self.thumb_start, self.thumb_length),
            ScrollBarPart::PageDecrement => (self.button, (split - self.button).max(0.0)),
            ScrollBarPart::PageIncrement => {
                let start = if self.show_thumb {
                    self.thumb_start + self.thumb_length
                } else {
                    split
                };
                (start, (track_end - start).max(0.0))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Session {
    Drag {
        pointer: f64,
        value: f64,
        ratio: f64,
    },
    Repeat {
        role: ScrollBarPart,
        pointer: f64,
        deadline: MonotonicTime,
    },
}

struct State {
    model: Arc<dyn RangeModel>,
    observed: RangeModelSnapshot,
    minimum_thumb_length: f64,
    session: Option<Session>,
    now: MonotonicTime,
}

/// Detached Figure configuration. Use `into_subtree` to admit all five parts atomically.
pub struct ScrollBarFigure {
    bounds: Rectangle,
    config: ScrollBarConfig,
    state: Arc<Mutex<State>>,
    consumer: Option<Arc<dyn ScrollBarValueConsumer>>,
    parts: Vec<(ScrollBarPart, Box<dyn Figure>)>,
}

impl Default for ScrollBarFigure {
    fn default() -> Self {
        Self::new(Arc::new(DefaultRangeModel::default()))
    }
}

impl ScrollBarFigure {
    pub fn new(model: Arc<dyn RangeModel>) -> Self {
        let observed = model.snapshot();
        Self {
            bounds: Rectangle::ZERO,
            config: ScrollBarConfig::default(),
            state: Arc::new(Mutex::new(State {
                model,
                observed,
                minimum_thumb_length: MIN_THUMB,
                session: None,
                now: MonotonicTime::ZERO,
            })),
            consumer: None,
            parts: Vec::new(),
        }
    }
    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }
    pub fn with_config(mut self, config: ScrollBarConfig) -> Result<Self, InvalidScrollBarConfig> {
        self.config = config.validate()?;
        Ok(self)
    }
    pub fn with_consumer(mut self, consumer: Arc<dyn ScrollBarValueConsumer>) -> Self {
        self.consumer = Some(consumer);
        self
    }
    pub fn with_part(mut self, role: ScrollBarPart, figure: Box<dyn Figure>) -> Self {
        self.parts.retain(|(part, _)| *part != role);
        self.parts.push((role, figure));
        self
    }
    pub fn range_model(&self) -> Arc<dyn RangeModel> {
        Arc::clone(&lock(&self.state).model)
    }

    fn geometry(&self, bounds: Rectangle) -> Geometry {
        let state = lock(&self.state);
        let config = ScrollBarConfig {
            minimum_thumb_length: self
                .config
                .minimum_thumb_length
                .max(state.minimum_thumb_length),
            ..self.config
        };
        Geometry::new(config, bounds, state.model.snapshot())
    }

    pub fn into_subtree(mut self) -> ScrollBarSubtree {
        let config = self.config;
        let state = Arc::clone(&self.state);
        let mut parts = std::mem::take(&mut self.parts);
        let mut subtree = FigureSubtree::new(Box::new(self));
        let root = subtree.root();
        let mut nodes = Vec::new();
        for role in PARTS {
            let child = if let Some(index) = parts.iter().position(|(part, _)| *part == role) {
                parts.remove(index).1
            } else {
                default_part(role, config)
            };
            let child = subtree.add(root, child).expect("own root");
            subtree
                .set_constraint(child, Box::new(role))
                .expect("own child");
            nodes.push((role, child));
        }
        subtree
            .set_layout_manager(root, Box::new(ScrollBarLayout { config, state }))
            .expect("own root");
        ScrollBarSubtree {
            subtree,
            root,
            parts: nodes,
        }
    }

    fn change(&self, role: ScrollBarPart, context: &mut EventContext<'_>) {
        let model = self.range_model();
        let increment = match role {
            ScrollBarPart::Decrement => -self.config.step_increment,
            ScrollBarPart::Increment => self.config.step_increment,
            ScrollBarPart::PageDecrement => -self.config.page_increment,
            ScrollBarPart::PageIncrement => self.config.page_increment,
            ScrollBarPart::Thumb => return,
        };
        if !self
            .consumer
            .as_ref()
            .is_some_and(|consumer| consumer.request_value(increment, true, context))
        {
            let _ = model.set_value(model.value() + increment);
        }
    }
    fn sync(&self, context: &mut EventContext<'_>) {
        let change = {
            let mut state = lock(&self.state);
            let current = state.model.snapshot();
            if current == state.observed {
                return;
            }
            let old = state.observed;
            state.observed = current;
            (old, current)
        };
        context.effects().invalidate();
        context.effects().repaint(None);
        if change.0.value != change.1.value {
            let id = context.target_id();
            context.effects().emit_property_change(
                id,
                SCROLL_VALUE,
                change.0.value,
                change.1.value,
            );
            if let Some(consumer) = &self.consumer {
                consumer.value_changed(change.0.value, change.1.value, context);
            }
        }
    }
}

/// A public typed identity; strings are diagnostic names only.
pub const SCROLL_VALUE: crate::PropertyKey<f64> =
    crate::PropertyKey::new("novadraw.scrollbar", "value");

pub struct ScrollBarSubtree {
    pub subtree: FigureSubtree,
    pub root: SubtreeNodeId,
    pub parts: Vec<(ScrollBarPart, SubtreeNodeId)>,
}

/// Model replacement goes through the same typed component transaction as external Figures.
pub struct SetScrollBarModel(pub Arc<dyn RangeModel>);
impl FigureComponentUpdate for SetScrollBarModel {
    type Figure = ScrollBarFigure;
    type Prepared = Arc<dyn RangeModel>;
    type Error = Infallible;
    fn prepare(
        self,
        _: &Self::Figure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        Ok(PreparedFigureUpdate::new(self.0))
    }
    fn commit(model: Self::Prepared, target: &mut Self::Figure) {
        let mut state = lock(&target.state);
        state.model = model;
        state.session = None;
    }
}

impl Figure for ScrollBarFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "ScrollBar"
    }
    fn intrinsic_size(&self) -> Dimension {
        match self.config.orientation {
            ScrollOrientation::Horizontal => {
                Dimension::new(self.bounds.width, self.config.thickness)
            }
            ScrollOrientation::Vertical => {
                Dimension::new(self.config.thickness, self.bounds.height)
            }
        }
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(TRACK);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(ACTIVITY, ActivityCapability::of::<Self>())
    }
}
impl FigureContainer for ScrollBarFigure {}

impl FigureEventHandler for ScrollBarFigure {
    fn on_mouse_pressed(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let model = self.range_model();
        if event.button != MouseButton::Left || !model.is_enabled() {
            return false;
        }
        let pointer = self.config.axis(Point::new(event.x, event.y));
        let geometry = self.geometry(context.target_bounds());
        let role = geometry.role(pointer);
        let mut state = lock(&self.state);
        state.session = if role == ScrollBarPart::Thumb {
            Some(Session::Drag {
                pointer,
                value: model.value(),
                ratio: if geometry.travel > 0.0 {
                    (model.maximum() - model.minimum() - model.extent()) / geometry.travel
                } else {
                    0.0
                },
            })
        } else {
            state
                .now
                .as_micros()
                .checked_add(INITIAL_DELAY_US)
                .map(|deadline| Session::Repeat {
                    role,
                    pointer,
                    deadline: MonotonicTime::from_micros(deadline),
                })
        };
        drop(state);
        context.effects().set_pressed(true);
        self.change(role, context);
        self.sync(context);
        true
    }
    fn on_mouse_dragged(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let session = lock(&self.state).session;
        let current = self.config.axis(Point::new(event.x, event.y));
        match session {
            Some(Session::Drag {
                pointer,
                value,
                ratio,
            }) => {
                let requested = value + (current - pointer) * ratio;
                if !self
                    .consumer
                    .as_ref()
                    .is_some_and(|consumer| consumer.request_value(requested, false, context))
                {
                    let _ = self.range_model().set_value(requested);
                }
                self.sync(context);
            }
            Some(Session::Repeat { role, deadline, .. }) => {
                let geometry = self.geometry(context.target_bounds());
                lock(&self.state).session = (geometry.role(current) == role
                    && current >= 0.0
                    && current <= geometry.length)
                    .then_some(Session::Repeat {
                        role,
                        pointer: current,
                        deadline,
                    });
            }
            None => return false,
        }
        true
    }
    fn on_mouse_released(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        lock(&self.state).session = None;
        context.effects().set_pressed(false);
        true
    }
    fn on_mouse_exited(&self, _: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        if matches!(lock(&self.state).session, Some(Session::Repeat { .. })) {
            lock(&self.state).session = None;
        }
        false
    }
}

impl FigureActivity for ScrollBarFigure {
    fn poll(&self, now: MonotonicTime, active: bool, context: &mut EventContext<'_>) {
        let repeat = {
            let mut state = lock(&self.state);
            state.now = now;
            if !active || !context.is_pointer_pressed() || !state.model.is_enabled() {
                state.session = None;
            }
            match state.session {
                Some(Session::Repeat {
                    role,
                    pointer,
                    deadline,
                }) if now >= deadline => {
                    let config = ScrollBarConfig {
                        minimum_thumb_length: self
                            .config
                            .minimum_thumb_length
                            .max(state.minimum_thumb_length),
                        ..self.config
                    };
                    let geometry =
                        Geometry::new(config, context.target_bounds(), state.model.snapshot());
                    if geometry.role(pointer) == role {
                        state.session =
                            now.as_micros().checked_add(REPEAT_INTERVAL_US).map(|time| {
                                Session::Repeat {
                                    role,
                                    pointer,
                                    deadline: MonotonicTime::from_micros(time),
                                }
                            });
                        Some(role)
                    } else {
                        state.session = None;
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(role) = repeat {
            self.change(role, context);
        }
        self.sync(context);
    }
    fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        match lock(&self.state).session {
            Some(Session::Repeat { deadline, .. }) => Some(deadline),
            _ => None,
        }
    }
}

pub struct ScrollBarLayout {
    config: ScrollBarConfig,
    state: Arc<Mutex<State>>,
}
impl LayoutManager for ScrollBarLayout {
    fn validate_constraint(
        &self,
        container: FigureId,
        child: FigureId,
        constraint: &dyn crate::layout::LayoutConstraint,
    ) -> Result<(), LayoutError> {
        if constraint.as_any().is::<ScrollBarPart>() {
            Ok(())
        } else {
            Err(LayoutError::UnsupportedConstraint {
                container,
                child,
                actual: constraint.type_name(),
            })
        }
    }
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let size = match self.config.orientation {
            ScrollOrientation::Horizontal => {
                Dimension::new(self.config.thickness * 4.0, self.config.thickness)
            }
            ScrollOrientation::Vertical => {
                Dimension::new(self.config.thickness, self.config.thickness * 4.0)
            }
        };
        Ok(FigureMeasurement::new(size.width, size.height, None))
    }
    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        // Draw2D ScrollBarLayout inherits AbstractLayout's minimum = preferred contract.
        Ok(self
            .preferred_measurement(container, constraints, scene)?
            .size())
    }
    fn layout(
        &mut self,
        container: FigureId,
        scene: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let area = scene.container_bounds(container)?;
        let mut config = self.config;
        for (id, _) in scene.children(container)? {
            if scene.constraint_as::<ScrollBarPart>(container, id)? == Some(&ScrollBarPart::Thumb) {
                let minimum = scene.minimum_size(id, MeasureConstraints::UNBOUNDED)?;
                let length = match config.orientation {
                    ScrollOrientation::Horizontal => minimum.width,
                    ScrollOrientation::Vertical => minimum.height,
                };
                config.minimum_thumb_length = config.minimum_thumb_length.max(length);
            }
        }
        let geometry = Geometry::new(config, area, lock(&self.state).model.snapshot());
        for (id, _) in scene.children(container)? {
            if let Some(role) = scene.constraint_as::<ScrollBarPart>(container, id)? {
                let (start, length) = geometry.placement(*role);
                out.set_child_bounds(id, self.config.rect(area, start, length));
                out.set_child_visible(id, *role != ScrollBarPart::Thumb || geometry.show_thumb);
            }
        }
        lock(&self.state).minimum_thumb_length = config.minimum_thumb_length;
        Ok(())
    }
}

struct Arrow {
    config: ScrollBarConfig,
    direction: f64,
}
impl Figure for Arrow {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ScrollBarArrow"
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(BUTTON);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) / 4.0;
        let mut path = Path::new();
        let points = [
            (self.direction * radius, 0.0),
            (-self.direction * radius, -radius),
            (-self.direction * radius, radius),
        ];
        for (index, (axis, cross)) in points.into_iter().enumerate() {
            let point = match self.config.orientation {
                ScrollOrientation::Horizontal => Point::new(center.x() + axis, center.y() + cross),
                ScrollOrientation::Vertical => Point::new(center.x() + cross, center.y() + axis),
            };
            if index == 0 {
                path.move_to(point.x(), point.y());
            } else {
                path.line_to(point.x(), point.y());
            }
        }
        path.close();
        context.set_fill_paint(Color::BLACK);
        context.fill_path(&path)
    }
}
fn default_part(role: ScrollBarPart, config: ScrollBarConfig) -> Box<dyn Figure> {
    match role {
        ScrollBarPart::Decrement => Box::new(Arrow {
            config,
            direction: -1.0,
        }),
        ScrollBarPart::Increment => Box::new(Arrow {
            config,
            direction: 1.0,
        }),
        _ => Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            0.0,
            0.0,
            if role == ScrollBarPart::Thumb {
                THUMB
            } else {
                TRACK
            },
        )),
    }
}
