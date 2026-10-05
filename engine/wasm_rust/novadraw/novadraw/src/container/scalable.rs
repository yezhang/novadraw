use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::geometry::{Dimension, Insets, Rectangle};
use crate::render::NdCanvas;

use crate::figure::{
    Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform, Figure, FigureContainer,
    FigureMeasurement, Freeform, HitParticipation, Layer, MeasureConstraints, border::Border,
};
use crate::{
    FigureId, FigureTree, FigureTreeBuilder, GraphMutationError, PropertyValue, UpdateManager,
};

fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleError {
    Graph(GraphMutationError),
    MissingFigure,
    InvalidScale,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(f),
            Self::MissingFigure => write!(f, "scalable figure block does not exist"),
            Self::InvalidScale => write!(f, "scale must be finite and greater than zero"),
        }
    }
}

impl Error for ScaleError {}

impl From<GraphMutationError> for ScaleError {
    fn from(value: GraphMutationError) -> Self {
        Self::Graph(value)
    }
}

#[derive(Debug)]
struct ScaleRuntime {
    scale: f64,
    unscaled_preferred_width: f64,
    unscaled_preferred_height: f64,
}

impl ScaleRuntime {
    fn new(width: f64, height: f64) -> Self {
        Self {
            scale: 1.0,
            unscaled_preferred_width: width,
            unscaled_preferred_height: height,
        }
    }

    fn unscaled_preferred_size(&self) -> Dimension {
        Dimension::new(
            self.unscaled_preferred_width,
            self.unscaled_preferred_height,
        )
    }

    fn update_scale(&mut self, scale: f64) -> Result<Option<(f64, f64, f64)>, ScaleError> {
        if !valid_scale(scale) {
            return Err(ScaleError::InvalidScale);
        }
        if self.scale == scale {
            return Ok(None);
        }
        let old_scale = self.scale;
        let size = self.unscaled_preferred_size();
        let (width, height) = (size.width * scale, size.height * scale);
        if !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 {
            return Err(ScaleError::InvalidScale);
        }
        self.scale = scale;
        Ok(Some((old_scale, width, height)))
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub trait ScalableFigure: Figure {
    fn scale(&self) -> f64;
}

#[derive(Clone)]
pub struct ScaleHandle {
    figure_id: FigureId,
    runtime: Arc<Mutex<ScaleRuntime>>,
}

impl ScaleHandle {
    pub fn figure_id(&self) -> FigureId {
        self.figure_id
    }

    pub fn scale(&self) -> f64 {
        lock_unpoisoned(&self.runtime).scale
    }

    pub(crate) fn set_scale(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        scale: f64,
    ) -> Result<bool, ScaleError> {
        if !valid_scale(scale) {
            return Err(ScaleError::InvalidScale);
        }
        if graph.node(self.figure_id).is_none() {
            return Err(ScaleError::MissingFigure);
        }
        let old_scale = {
            let mut runtime = lock_unpoisoned(&self.runtime);
            let Some(update) = runtime.update_scale(scale)? else {
                return Ok(false);
            };
            update.0
        };

        graph.record_property_change(
            self.figure_id,
            "scale",
            PropertyValue::Number(old_scale),
            PropertyValue::Number(scale),
        );
        graph.record_coordinate_system_changed(self.figure_id);
        graph.mark_invalid(update_manager, self.figure_id);
        graph.repaint(update_manager, self.figure_id, None);
        Ok(true)
    }
}

#[derive(Clone)]
pub struct ScalableLayeredPaneFigure {
    bounds: Rectangle,
    runtime: Arc<Mutex<ScaleRuntime>>,
    child_clipping_strategy: ChildClippingStrategy,
    border: Option<Arc<dyn Border>>,
}

impl ScalableLayeredPaneFigure {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::with_runtime(
            Rectangle::new(x, y, width, height),
            Arc::new(Mutex::new(ScaleRuntime::new(width, height))),
        )
    }

    fn with_runtime(bounds: Rectangle, runtime: Arc<Mutex<ScaleRuntime>>) -> Self {
        Self {
            bounds,
            runtime,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    pub fn with_scale(self, scale: f64) -> Self {
        let _ = lock_unpoisoned(&self.runtime).update_scale(scale);
        self
    }

    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    fn project_layout_size(&self, size: Dimension) -> Dimension {
        let scale = self.scale();
        let insets = self.insets();
        Dimension::new(
            (size.width - insets.width()).max(0.0) * scale + insets.width(),
            (size.height - insets.height()).max(0.0) * scale + insets.height(),
        )
    }
}

impl Bounded for ScalableLayeredPaneFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ScalableLayeredPaneFigure"
    }

    fn preferred_size(&self) -> Dimension {
        let size = lock_unpoisoned(&self.runtime).unscaled_preferred_size();
        self.project_layout_size(size)
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        let scale = self.scale();
        MeasureConstraints::new(
            constraints.max_width().map(|width| width / scale),
            constraints.max_height().map(|height| height / scale),
        )
        .expect("valid constraints remain valid after positive scaling")
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        let size = self.project_layout_size(measurement.size());
        let top = self.insets().top;
        FigureMeasurement::new(
            size.width,
            size.height,
            measurement
                .baseline
                .map(|baseline| (baseline - top).max(0.0) * self.scale() + top),
        )
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        self.project_layout_size(size)
    }

    fn child_transform(&self) -> ChildTransform {
        let scale = self.scale();
        ChildTransform::uniform(scale, 0.0, 0.0)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> Insets {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or(Insets::ZERO)
    }

    fn client_area(&self) -> Rectangle {
        let insets = self.insets();
        Rectangle::new(
            insets.left,
            insets.top,
            (self.bounds.width - insets.width()).max(0.0),
            (self.bounds.height - insets.height()).max(0.0),
        )
    }
}

impl Figure for ScalableLayeredPaneFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ScalableLayeredPaneFigure"
    }

    fn initial_insets(&self) -> Insets {
        Bounded::insets(self)
    }

    fn intrinsic_size(&self) -> Dimension {
        Bounded::preferred_size(self)
    }

    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn content_scale(&self) -> Option<f64> {
        Some(self.scale())
    }
}

impl FigureContainer for ScalableLayeredPaneFigure {
    fn child_transform(&self) -> ChildTransform {
        Bounded::child_transform(self)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        Bounded::layout_constraints(self, constraints)
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        Bounded::project_preferred_measurement(self, measurement)
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        Bounded::project_minimum_size(self, size)
    }
}

impl ScalableFigure for ScalableLayeredPaneFigure {
    fn scale(&self) -> f64 {
        lock_unpoisoned(&self.runtime).scale
    }
}

#[derive(Clone)]
pub struct ScalableFreeformLayeredPane {
    bounds: Rectangle,
    runtime: Arc<Mutex<ScaleRuntime>>,
    border: Option<Arc<dyn Border>>,
}

impl ScalableFreeformLayeredPane {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::with_runtime(
            Rectangle::new(x, y, width, height),
            Arc::new(Mutex::new(ScaleRuntime::new(width, height))),
        )
    }

    fn with_runtime(bounds: Rectangle, runtime: Arc<Mutex<ScaleRuntime>>) -> Self {
        Self {
            bounds,
            runtime,
            border: None,
        }
    }

    pub fn with_scale(self, scale: f64) -> Self {
        let _ = lock_unpoisoned(&self.runtime).update_scale(scale);
        self
    }

    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    fn project_layout_size(&self, size: Dimension) -> Dimension {
        let scale = self.scale();
        let insets = self.insets();
        Dimension::new(
            (size.width - insets.width()).max(0.0) * scale + insets.width(),
            (size.height - insets.height()).max(0.0) * scale + insets.height(),
        )
    }
}

impl Bounded for ScalableFreeformLayeredPane {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ScalableFreeformLayeredPane"
    }

    fn preferred_size(&self) -> Dimension {
        let size = lock_unpoisoned(&self.runtime).unscaled_preferred_size();
        self.project_layout_size(size)
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        let scale = self.scale();
        MeasureConstraints::new(
            constraints.max_width().map(|width| width / scale),
            constraints.max_height().map(|height| height / scale),
        )
        .expect("valid constraints remain valid after positive scaling")
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        let size = self.project_layout_size(measurement.size());
        let top = self.insets().top;
        FigureMeasurement::new(
            size.width,
            size.height,
            measurement
                .baseline
                .map(|baseline| (baseline - top).max(0.0) * self.scale() + top),
        )
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        self.project_layout_size(size)
    }

    fn child_transform(&self) -> ChildTransform {
        ChildTransform::uniform(self.scale(), 0.0, 0.0)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }

    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Layered
    }

    fn insets(&self) -> Insets {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or(Insets::ZERO)
    }

    fn client_area(&self) -> Rectangle {
        let insets = self.insets();
        Rectangle::new(
            insets.left,
            insets.top,
            (self.bounds.width - insets.width()).max(0.0),
            (self.bounds.height - insets.height()).max(0.0),
        )
    }
}

impl Figure for ScalableFreeformLayeredPane {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ScalableFreeformLayeredPane"
    }

    fn initial_insets(&self) -> Insets {
        Bounded::insets(self)
    }

    fn intrinsic_size(&self) -> Dimension {
        Bounded::preferred_size(self)
    }

    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn layer(&self) -> Option<&dyn Layer> {
        Some(self)
    }

    fn freeform(&self) -> Option<&dyn Freeform> {
        Some(self)
    }

    fn content_scale(&self) -> Option<f64> {
        Some(self.scale())
    }
}

impl FigureContainer for ScalableFreeformLayeredPane {
    fn child_transform(&self) -> ChildTransform {
        Bounded::child_transform(self)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }

    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Layered
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        Bounded::layout_constraints(self, constraints)
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        Bounded::project_preferred_measurement(self, measurement)
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        Bounded::project_minimum_size(self, size)
    }
}

impl Layer for ScalableFreeformLayeredPane {}
impl Freeform for ScalableFreeformLayeredPane {}

impl ScalableFigure for ScalableFreeformLayeredPane {
    fn scale(&self) -> f64 {
        lock_unpoisoned(&self.runtime).scale
    }
}

impl FigureTree {
    pub fn scale_handle(&self, figure_id: FigureId) -> Option<ScaleHandle> {
        let figure = &self.node(figure_id)?.figure;
        let runtime =
            if let Some(scalable) = figure.as_any().downcast_ref::<ScalableLayeredPaneFigure>() {
                Arc::clone(&scalable.runtime)
            } else {
                Arc::clone(
                    &figure
                        .as_any()
                        .downcast_ref::<ScalableFreeformLayeredPane>()?
                        .runtime,
                )
            };
        Some(ScaleHandle { figure_id, runtime })
    }

    pub(crate) fn add_scalable_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ScaleRuntime::new(bounds.width, bounds.height)));
        let figure = ScalableLayeredPaneFigure::with_runtime(bounds, Arc::clone(&runtime));
        let figure_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { figure_id, runtime })
    }

    pub(crate) fn add_scalable_freeform_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ScaleRuntime::new(bounds.width, bounds.height)));
        let figure = ScalableFreeformLayeredPane::with_runtime(bounds, Arc::clone(&runtime));
        let figure_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { figure_id, runtime })
    }
}

impl FigureTreeBuilder<'_> {
    pub fn add_scalable_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        self.tree_mut().add_scalable_layered_pane_to(parent, bounds)
    }

    pub fn add_scalable_freeform_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        self.tree_mut()
            .add_scalable_freeform_layered_pane_to(parent, bounds)
    }

    pub fn set_scale(&mut self, scalable: FigureId, scale: f64) -> Result<bool, ScaleError> {
        let handle = self
            .tree_mut()
            .scale_handle(scalable)
            .ok_or(ScaleError::MissingFigure)?;
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        handle.set_scale(self.tree_mut(), &mut updates, scale)
    }
}
