use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use crate::figure::{
    Bounded, ChildClippingStrategy, ChildTransform, Figure, Updatable, border::Border,
};
use crate::{BlockId, FigureGraph, GraphMutationError, PropertyValue, UpdateManager};

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

    fn unscaled_preferred_size(&self) -> (f64, f64) {
        (
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
        let (width, height) = self.unscaled_preferred_size();
        let (width, height) = (width * scale, height * scale);
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
    block_id: BlockId,
    runtime: Arc<Mutex<ScaleRuntime>>,
}

impl ScaleHandle {
    pub fn block_id(&self) -> BlockId {
        self.block_id
    }

    pub fn scale(&self) -> f64 {
        lock_unpoisoned(&self.runtime).scale
    }

    pub fn set_scale(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        scale: f64,
    ) -> Result<bool, ScaleError> {
        if !valid_scale(scale) {
            return Err(ScaleError::InvalidScale);
        }
        if graph.get_block(self.block_id).is_none() {
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
            self.block_id,
            "scale",
            PropertyValue::Number(old_scale),
            PropertyValue::Number(scale),
        );
        graph.record_coordinate_system_changed(self.block_id);
        graph.mark_invalid(update_manager, self.block_id);
        graph.repaint(update_manager, self.block_id, None);
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

    fn project_layout_size(&self, size: (f64, f64)) -> (f64, f64) {
        let scale = self.scale();
        let (top, left, bottom, right) = self.insets();
        (
            (size.0 - left - right).max(0.0) * scale + left + right,
            (size.1 - top - bottom).max(0.0) * scale + top + bottom,
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

    fn preferred_size(&self) -> (f64, f64) {
        let size = lock_unpoisoned(&self.runtime).unscaled_preferred_size();
        self.project_layout_size(size)
    }

    fn layout_size_hints(&self, w_hint: f64, h_hint: f64) -> (f64, f64) {
        let scale = self.scale();
        let scale_hint = |hint: f64| if hint >= 0.0 { hint / scale } else { hint };
        (scale_hint(w_hint), scale_hint(h_hint))
    }

    fn project_preferred_size(&self, size: (f64, f64)) -> (f64, f64) {
        self.project_layout_size(size)
    }

    fn project_minimum_size(&self, size: (f64, f64)) -> (f64, f64) {
        self.project_layout_size(size)
    }

    fn child_transform(&self) -> ChildTransform {
        let scale = self.scale();
        let (top, left, _, _) = self.insets();
        ChildTransform::uniform(scale, left, top)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    fn client_area(&self) -> Rectangle {
        let (top, left, bottom, right) = self.insets();
        Rectangle::new(
            left,
            top,
            (self.bounds.width - left - right).max(0.0),
            (self.bounds.height - top - bottom).max(0.0),
        )
    }
}

impl Updatable for ScalableLayeredPaneFigure {
    fn validate(&mut self) {}
}

impl Figure for ScalableLayeredPaneFigure {
    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }
}

impl ScalableFigure for ScalableLayeredPaneFigure {
    fn scale(&self) -> f64 {
        lock_unpoisoned(&self.runtime).scale
    }
}

impl FigureGraph {
    pub fn scale_handle(&self, block_id: BlockId) -> Option<ScaleHandle> {
        let scalable = self
            .block(block_id)?
            .figure
            .as_any()
            .downcast_ref::<ScalableLayeredPaneFigure>()?;
        Some(ScaleHandle {
            block_id,
            runtime: Arc::clone(&scalable.runtime),
        })
    }

    pub fn add_scalable_layered_pane_to(
        &mut self,
        parent: BlockId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ScaleRuntime::new(bounds.width, bounds.height)));
        let figure = ScalableLayeredPaneFigure::with_runtime(bounds, Arc::clone(&runtime));
        let block_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { block_id, runtime })
    }
}
