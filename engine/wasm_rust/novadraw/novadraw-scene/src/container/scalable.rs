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
        let old_bounds = graph
            .figure_bounds(self.block_id)
            .ok_or(ScaleError::MissingFigure)?;
        let old_scale = {
            let mut runtime = lock_unpoisoned(&self.runtime);
            if runtime.scale == scale {
                return Ok(false);
            }
            let old_scale = runtime.scale;
            runtime.scale = scale;
            old_scale
        };
        let scale_ratio = scale / old_scale;
        graph.set_bounds_with_update(
            update_manager,
            self.block_id,
            old_bounds.x,
            old_bounds.y,
            old_bounds.width * scale_ratio,
            old_bounds.height * scale_ratio,
        );

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
            Arc::new(Mutex::new(ScaleRuntime { scale: 1.0 })),
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

    pub fn with_scale(mut self, scale: f64) -> Self {
        if valid_scale(scale) {
            let old_scale = self.scale();
            let scale_ratio = scale / old_scale;
            self.bounds.width *= scale_ratio;
            self.bounds.height *= scale_ratio;
            lock_unpoisoned(&self.runtime).scale = scale;
        }
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

    fn use_local_coordinates(&self) -> bool {
        true
    }

    fn child_transform(&self) -> ChildTransform {
        let scale = self.scale();
        let (top, left, _, _) = self.insets();
        ChildTransform::uniform(scale, self.bounds.x + left, self.bounds.y + top)
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
        let scale = self.scale();
        let (top, left, bottom, right) = self.insets();
        Rectangle::new(
            0.0,
            0.0,
            (self.bounds.width - left - right).max(0.0) / scale,
            (self.bounds.height - top - bottom).max(0.0) / scale,
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
    pub fn add_scalable_layered_pane_to(
        &mut self,
        parent: BlockId,
        bounds: Rectangle,
    ) -> Result<ScaleHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ScaleRuntime { scale: 1.0 }));
        let figure = ScalableLayeredPaneFigure::with_runtime(bounds, Arc::clone(&runtime));
        let block_id = self.try_add_child_to(parent, Box::new(figure))?;
        Ok(ScaleHandle { block_id, runtime })
    }
}
