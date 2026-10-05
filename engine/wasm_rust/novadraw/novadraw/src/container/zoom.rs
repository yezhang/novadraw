use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::geometry::Point;

use crate::{
    FigureTree, FigureTreeBuilder, FreeformError, LayoutError, MeasureConstraints,
    RangeModelSnapshot, ScaleError, ScaleHandle, UpdateManager, ViewportError, ViewportHandle,
};

pub const DEFAULT_ZOOM_LEVELS: [f64; 8] = [0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomViewportState {
    pub view_location: Point,
    pub width: f64,
    pub height: f64,
    pub anchor: Option<Point>,
}

pub trait ZoomScrollPolicy: Send + Sync {
    fn calc_new_view_location(
        &self,
        viewport: ZoomViewportState,
        old_zoom: f64,
        new_zoom: f64,
    ) -> Point;
}

#[derive(Debug, Default)]
pub struct DefaultScrollPolicy;

impl ZoomScrollPolicy for DefaultScrollPolicy {
    fn calc_new_view_location(
        &self,
        viewport: ZoomViewportState,
        old_zoom: f64,
        new_zoom: f64,
    ) -> Point {
        zoom_location_at(
            viewport.view_location,
            Point::new(viewport.width / 2.0, viewport.height / 2.0),
            old_zoom,
            new_zoom,
        )
    }
}

#[derive(Debug, Default)]
pub struct MouseLocationZoomScrollPolicy;

impl ZoomScrollPolicy for MouseLocationZoomScrollPolicy {
    fn calc_new_view_location(
        &self,
        viewport: ZoomViewportState,
        old_zoom: f64,
        new_zoom: f64,
    ) -> Point {
        let anchor = viewport.anchor.filter(|anchor| {
            anchor.x() >= 0.0
                && anchor.y() >= 0.0
                && anchor.x() <= viewport.width
                && anchor.y() <= viewport.height
        });
        zoom_location_at(
            viewport.view_location,
            anchor.unwrap_or_else(|| Point::new(viewport.width / 2.0, viewport.height / 2.0)),
            old_zoom,
            new_zoom,
        )
    }
}

fn zoom_location_at(old_location: Point, anchor: Point, old_zoom: f64, new_zoom: f64) -> Point {
    let ratio = new_zoom / old_zoom;
    Point::new(
        (anchor.x() + old_location.x()) * ratio - anchor.x(),
        (anchor.y() + old_location.y()) * ratio - anchor.y(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomError {
    InvalidZoom,
    InvalidZoomLevels,
    MissingViewport,
    Layout(LayoutError),
    Freeform(FreeformError),
    Scale(ScaleError),
    Viewport(ViewportError),
}

impl fmt::Display for ZoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidZoom => write!(f, "zoom must be finite and greater than zero"),
            Self::InvalidZoomLevels => {
                write!(
                    f,
                    "zoom levels must be finite, positive, and strictly increasing"
                )
            }
            Self::MissingViewport => write!(f, "zoom manager viewport does not exist"),
            Self::Layout(error) => error.fmt(f),
            Self::Freeform(error) => error.fmt(f),
            Self::Scale(error) => error.fmt(f),
            Self::Viewport(error) => error.fmt(f),
        }
    }
}

impl Error for ZoomError {}

impl From<ScaleError> for ZoomError {
    fn from(value: ScaleError) -> Self {
        Self::Scale(value)
    }
}

impl From<LayoutError> for ZoomError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<FreeformError> for ZoomError {
    fn from(value: FreeformError) -> Self {
        Self::Freeform(value)
    }
}

impl From<ViewportError> for ZoomError {
    fn from(value: ViewportError) -> Self {
        Self::Viewport(value)
    }
}

pub struct ZoomManager {
    scalable: ScaleHandle,
    viewport: ViewportHandle,
    scroll_policy: Arc<dyn ZoomScrollPolicy>,
    zoom_levels: Vec<f64>,
}

impl ZoomManager {
    pub fn new(scalable: ScaleHandle, viewport: ViewportHandle) -> Self {
        Self {
            scalable,
            viewport,
            scroll_policy: Arc::new(DefaultScrollPolicy),
            zoom_levels: DEFAULT_ZOOM_LEVELS.to_vec(),
        }
    }

    pub fn scalable(&self) -> &ScaleHandle {
        &self.scalable
    }

    pub fn viewport(&self) -> &ViewportHandle {
        &self.viewport
    }

    pub fn zoom(&self) -> f64 {
        self.scalable.scale()
    }

    pub fn zoom_levels(&self) -> &[f64] {
        &self.zoom_levels
    }

    pub fn set_scroll_policy(&mut self, policy: Arc<dyn ZoomScrollPolicy>) {
        self.scroll_policy = policy;
    }

    pub fn set_zoom_levels(&mut self, levels: Vec<f64>) -> Result<(), ZoomError> {
        let valid = !levels.is_empty()
            && levels.iter().all(|level| level.is_finite() && *level > 0.0)
            && levels.windows(2).all(|pair| pair[0] < pair[1]);
        if !valid {
            return Err(ZoomError::InvalidZoomLevels);
        }
        self.zoom_levels = levels;
        Ok(())
    }

    pub(crate) fn set_zoom(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        zoom: f64,
    ) -> Result<bool, ZoomError> {
        self.set_zoom_at(graph, update_manager, zoom, None)
    }

    pub(crate) fn set_zoom_at(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        zoom: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(ZoomError::InvalidZoom);
        }
        let new_zoom = zoom.clamp(self.min_zoom(), self.max_zoom());
        self.prim_set_zoom_at(graph, update_manager, new_zoom, anchor)
    }

    fn prim_set_zoom_at(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        new_zoom: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        let old_zoom = self.zoom();
        if old_zoom == new_zoom {
            return Ok(false);
        }

        let block = graph
            .node(self.viewport.figure_id())
            .ok_or(ZoomError::MissingViewport)?;
        let client_area = block.client_area();
        let old_location = self.viewport.view_location();
        let uses_content_domain = self.uses_content_domain(graph);
        let policy_location = if uses_content_domain {
            Point::new(old_location.x() * old_zoom, old_location.y() * old_zoom)
        } else {
            old_location
        };
        let policy_location = self.scroll_policy.calc_new_view_location(
            ZoomViewportState {
                view_location: policy_location,
                width: client_area.width,
                height: client_area.height,
                anchor,
            },
            old_zoom,
            new_zoom,
        );
        let new_location = if uses_content_domain {
            Point::new(
                policy_location.x() / new_zoom,
                policy_location.y() / new_zoom,
            )
        } else {
            policy_location
        };
        let old_horizontal = self.viewport.horizontal_range();
        let old_vertical = self.viewport.vertical_range();

        self.scalable.set_scale(graph, update_manager, new_zoom)?;
        graph.validate_with_update(update_manager, self.viewport.figure_id())?;
        self.viewport.set_view_location(
            graph,
            update_manager,
            new_location.x(),
            new_location.y(),
        )?;
        self.repaint_range_changes(graph, update_manager, old_horizontal, old_vertical);
        Ok(true)
    }

    pub(crate) fn zoom_by_at(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        factor: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(ZoomError::InvalidZoom);
        }
        self.set_zoom_at(graph, update_manager, self.zoom() * factor, anchor)
    }

    pub(crate) fn zoom_in(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
    ) -> Result<bool, ZoomError> {
        let current = self.zoom();
        let next = self
            .zoom_levels
            .iter()
            .copied()
            .find(|level| *level > current)
            .unwrap_or_else(|| self.max_zoom());
        self.set_zoom(graph, update_manager, next)
    }

    pub(crate) fn zoom_out(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
    ) -> Result<bool, ZoomError> {
        let current = self.zoom();
        let previous = self
            .zoom_levels
            .iter()
            .copied()
            .rev()
            .find(|level| *level < current)
            .unwrap_or_else(|| self.min_zoom());
        self.set_zoom(graph, update_manager, previous)
    }

    pub(crate) fn fit_all(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, true, true)
    }

    pub(crate) fn fit_width(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, true, false)
    }

    pub(crate) fn fit_height(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, false, true)
    }

    fn fit(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        fit_width: bool,
        fit_height: bool,
    ) -> Result<bool, ZoomError> {
        let old_zoom = self.zoom();
        graph.validate_with_update(update_manager, self.viewport.figure_id())?;
        let uses_content_domain = self.uses_content_domain(graph);
        let viewport = graph
            .node(self.viewport.figure_id())
            .ok_or(ZoomError::MissingViewport)?
            .client_area();
        let (content_width, content_height) = if uses_content_domain {
            let extent = graph.freeform_extent(self.scalable.figure_id())?;
            (extent.width, extent.height)
        } else {
            let preferred = graph
                .preferred_measurement(self.scalable.figure_id(), MeasureConstraints::UNBOUNDED)
                .ok_or(ZoomError::Scale(ScaleError::MissingFigure))?;
            let scalable_block = graph
                .node(self.scalable.figure_id())
                .ok_or(ZoomError::Scale(ScaleError::MissingFigure))?;
            let insets = scalable_block.state().insets();
            (
                (preferred.width - insets.width()) / old_zoom,
                (preferred.height - insets.height()) / old_zoom,
            )
        };
        if content_width <= 0.0
            || content_height <= 0.0
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err(ZoomError::InvalidZoom);
        }
        let width_zoom = viewport.width / content_width;
        let height_zoom = viewport.height / content_height;
        let new_zoom = match (fit_width, fit_height) {
            (true, true) => width_zoom.min(height_zoom),
            (true, false) => width_zoom,
            (false, true) => height_zoom,
            (false, false) => old_zoom,
        }
        .min(self.max_zoom());
        let zoom_changed = self.prim_set_zoom_at(graph, update_manager, new_zoom, None)?;
        let current = self.viewport.view_location();
        let horizontal = self.viewport.horizontal_range();
        let vertical = self.viewport.vertical_range();
        let location_changed = self.viewport.set_view_location(
            graph,
            update_manager,
            if fit_width {
                horizontal.minimum
            } else {
                current.x()
            },
            if fit_height {
                vertical.minimum
            } else {
                current.y()
            },
        )?;
        Ok(zoom_changed || location_changed)
    }

    fn uses_content_domain(&self, graph: &FigureTree) -> bool {
        self.viewport.contents(graph) == Some(self.scalable.figure_id())
            && graph
                .node(self.scalable.figure_id())
                .is_some_and(|block| block.figure.freeform().is_some())
    }

    fn min_zoom(&self) -> f64 {
        self.zoom_levels[0]
    }

    fn max_zoom(&self) -> f64 {
        self.zoom_levels[self.zoom_levels.len() - 1]
    }

    fn repaint_range_changes(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        old_horizontal: RangeModelSnapshot,
        old_vertical: RangeModelSnapshot,
    ) {
        if old_horizontal == self.viewport.horizontal_range()
            && old_vertical == self.viewport.vertical_range()
        {
            return;
        }
        graph.repaint(update_manager, self.viewport.figure_id(), None);
        if let Some(parent) = graph.parent_id(self.viewport.figure_id()) {
            graph.repaint(update_manager, parent, None);
        }
    }
}

impl FigureTreeBuilder<'_> {
    pub fn set_zoom(&mut self, zoom: &ZoomManager, scale: f64) -> Result<bool, ZoomError> {
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        zoom.set_zoom(self.tree_mut(), &mut updates, scale)
    }

    pub fn set_zoom_at(
        &mut self,
        zoom: &ZoomManager,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        zoom.set_zoom_at(self.tree_mut(), &mut updates, scale, anchor)
    }
}
