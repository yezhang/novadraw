use std::error::Error;
use std::fmt;
use std::sync::Arc;

use novadraw_geometry::Point;

use crate::{
    FigureGraph, RangeModelSnapshot, ScaleError, ScaleHandle, UpdateManager, ViewportError,
    ViewportHandle,
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

    pub fn set_zoom(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        zoom: f64,
    ) -> Result<bool, ZoomError> {
        self.set_zoom_at(graph, update_manager, zoom, None)
    }

    pub fn set_zoom_at(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
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
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        new_zoom: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        let old_zoom = self.zoom();
        if old_zoom == new_zoom {
            return Ok(false);
        }

        let block = graph
            .block(self.viewport.block_id())
            .ok_or(ZoomError::MissingViewport)?;
        let client_area = block.client_area();
        let old_location = self.viewport.view_location();
        let new_location = self.scroll_policy.calc_new_view_location(
            ZoomViewportState {
                view_location: old_location,
                width: client_area.width,
                height: client_area.height,
                anchor,
            },
            old_zoom,
            new_zoom,
        );
        let old_horizontal = self.viewport.horizontal_range();
        let old_vertical = self.viewport.vertical_range();

        self.scalable.set_scale(graph, update_manager, new_zoom)?;
        graph.validate_with_update(update_manager, self.viewport.block_id());
        self.viewport.set_view_location(
            graph,
            update_manager,
            new_location.x(),
            new_location.y(),
        )?;
        self.repaint_range_changes(graph, update_manager, old_horizontal, old_vertical);
        Ok(true)
    }

    pub fn zoom_by_at(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        factor: f64,
        anchor: Option<Point>,
    ) -> Result<bool, ZoomError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(ZoomError::InvalidZoom);
        }
        self.set_zoom_at(graph, update_manager, self.zoom() * factor, anchor)
    }

    pub fn zoom_in(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
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

    pub fn zoom_out(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
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

    pub fn fit_all(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, true, true)
    }

    pub fn fit_width(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, true, false)
    }

    pub fn fit_height(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
    ) -> Result<bool, ZoomError> {
        self.fit(graph, update_manager, false, true)
    }

    fn fit(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        fit_width: bool,
        fit_height: bool,
    ) -> Result<bool, ZoomError> {
        let old_zoom = self.zoom();
        let preferred = graph
            .preferred_size(self.scalable.block_id(), -1.0, -1.0)
            .ok_or(ZoomError::Scale(ScaleError::MissingFigure))?;
        let scalable_block = graph
            .block(self.scalable.block_id())
            .ok_or(ZoomError::Scale(ScaleError::MissingFigure))?;
        let (top, left, bottom, right) = scalable_block.state().insets();
        let viewport = graph
            .block(self.viewport.block_id())
            .ok_or(ZoomError::MissingViewport)?
            .client_area();
        let scaled_content_width = preferred.0 - left - right;
        let scaled_content_height = preferred.1 - top - bottom;
        if scaled_content_width <= 0.0
            || scaled_content_height <= 0.0
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err(ZoomError::InvalidZoom);
        }
        let width_zoom = viewport.width * old_zoom / scaled_content_width;
        let height_zoom = viewport.height * old_zoom / scaled_content_height;
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

    fn min_zoom(&self) -> f64 {
        self.zoom_levels[0]
    }

    fn max_zoom(&self) -> f64 {
        self.zoom_levels[self.zoom_levels.len() - 1]
    }

    fn repaint_range_changes(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        old_horizontal: RangeModelSnapshot,
        old_vertical: RangeModelSnapshot,
    ) {
        if old_horizontal == self.viewport.horizontal_range()
            && old_vertical == self.viewport.vertical_range()
        {
            return;
        }
        graph.repaint(update_manager, self.viewport.block_id(), None);
        if let Some(parent) = graph.parent_id(self.viewport.block_id()) {
            graph.repaint(update_manager, parent, None);
        }
    }
}
