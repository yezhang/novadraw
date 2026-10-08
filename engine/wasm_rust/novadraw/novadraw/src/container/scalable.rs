use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::geometry::{Dimension, Insets, Rectangle};
use crate::render::NdCanvas;

use crate::figure::{
    Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform, Figure, FigureContainer,
    FigureMeasurement, HitParticipation, MeasureConstraints, border::Border,
};
use crate::runtime::update::property::standard as property;
use crate::{
    FREEFORM, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureId, FigureTree,
    FigureTreeBuilder, FreeformCapability, GraphMutationError, LAYER, LayerCapability, SCALE,
    ScaleCapability, UpdateManager,
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
struct ScaleState {
    scale: f64,
}

impl ScaleState {
    const fn new() -> Self {
        Self { scale: 1.0 }
    }

    fn update_scale(&mut self, scale: f64) -> Result<Option<f64>, ScaleError> {
        if !valid_scale(scale) {
            return Err(ScaleError::InvalidScale);
        }
        if self.scale == scale {
            return Ok(None);
        }
        let old_scale = self.scale;
        self.scale = scale;
        Ok(Some(old_scale))
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Shared scale capability owned by a Figure and cloned by runtime handles.
#[derive(Clone, Debug)]
pub struct ScaleModel {
    state: Arc<Mutex<ScaleState>>,
}

impl ScaleModel {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ScaleState::new())),
        }
    }

    pub fn with_scale(scale: f64) -> Result<Self, ScaleError> {
        let model = Self::new();
        model.update_scale(scale)?;
        Ok(model)
    }

    pub fn scale(&self) -> f64 {
        lock_unpoisoned(&self.state).scale
    }

    fn update_scale(&self, scale: f64) -> Result<Option<f64>, ScaleError> {
        lock_unpoisoned(&self.state).update_scale(scale)
    }
}

impl Default for ScaleModel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct ScaleHandle {
    figure_id: FigureId,
    model: ScaleModel,
}

impl ScaleHandle {
    pub fn figure_id(&self) -> FigureId {
        self.figure_id
    }

    pub fn scale(&self) -> f64 {
        self.model.scale()
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
            let Some(old_scale) = self.model.update_scale(scale)? else {
                return Ok(false);
            };
            old_scale
        };

        graph.record_property_change(self.figure_id, property::SCALE, old_scale, scale);
        graph.record_coordinate_system_changed(self.figure_id);
        graph.mark_invalid(update_manager, self.figure_id);
        graph.repaint(update_manager, self.figure_id, None);
        Ok(true)
    }
}

#[derive(Clone)]
pub struct ScalableLayeredPaneFigure {
    bounds: Rectangle,
    scale_model: ScaleModel,
    child_clipping_strategy: ChildClippingStrategy,
    border: Option<Arc<dyn Border>>,
}

impl ScalableLayeredPaneFigure {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::with_scale_model(Rectangle::new(x, y, width, height), ScaleModel::new())
    }

    fn with_scale_model(bounds: Rectangle, scale_model: ScaleModel) -> Self {
        Self {
            bounds,
            scale_model,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    pub fn with_scale(mut self, scale: f64) -> Result<Self, ScaleError> {
        self.scale_model = ScaleModel::with_scale(scale)?;
        Ok(self)
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
        let scale = self.scale_model.scale();
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
        let size = Dimension::new(self.bounds.width, self.bounds.height);
        self.project_layout_size(size)
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        let scale = self.scale_model.scale();
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
                .map(|baseline| (baseline - top).max(0.0) * self.scale_model.scale() + top),
        )
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        self.project_layout_size(size)
    }

    fn child_transform(&self) -> ChildTransform {
        let scale = self.scale_model.scale();
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

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(SCALE, ScaleCapability::new(self.scale_model.clone()))
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

#[derive(Clone)]
pub struct ScalableFreeformLayeredPane {
    bounds: Rectangle,
    scale_model: ScaleModel,
    border: Option<Arc<dyn Border>>,
}

impl ScalableFreeformLayeredPane {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::with_scale_model(Rectangle::new(x, y, width, height), ScaleModel::new())
    }

    fn with_scale_model(bounds: Rectangle, scale_model: ScaleModel) -> Self {
        Self {
            bounds,
            scale_model,
            border: None,
        }
    }

    pub fn with_scale(mut self, scale: f64) -> Result<Self, ScaleError> {
        self.scale_model = ScaleModel::with_scale(scale)?;
        Ok(self)
    }

    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    fn project_layout_size(&self, size: Dimension) -> Dimension {
        let scale = self.scale_model.scale();
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
        let size = Dimension::new(self.bounds.width, self.bounds.height);
        self.project_layout_size(size)
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        let scale = self.scale_model.scale();
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
                .map(|baseline| (baseline - top).max(0.0) * self.scale_model.scale() + top),
        )
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        self.project_layout_size(size)
    }

    fn child_transform(&self) -> ChildTransform {
        ChildTransform::uniform(self.scale_model.scale(), 0.0, 0.0)
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

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(LAYER, LayerCapability)?;
        out.register(FREEFORM, FreeformCapability)?;
        out.register(SCALE, ScaleCapability::new(self.scale_model.clone()))
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

impl FigureTree {
    pub fn scale_handle(&self, figure_id: FigureId) -> Option<ScaleHandle> {
        let model = self.capability(figure_id, SCALE).ok()??.model().clone();
        Some(ScaleHandle { figure_id, model })
    }

    pub(crate) fn add_scalable_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let model = ScaleModel::new();
        let figure = ScalableLayeredPaneFigure::with_scale_model(bounds, model.clone());
        let figure_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { figure_id, model })
    }

    pub(crate) fn add_scalable_freeform_layered_pane_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let model = ScaleModel::new();
        let figure = ScalableFreeformLayeredPane::with_scale_model(bounds, model.clone());
        let figure_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { figure_id, model })
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
