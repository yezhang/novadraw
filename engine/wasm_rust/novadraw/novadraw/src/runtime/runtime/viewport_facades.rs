use super::Runtime;
use crate::container::{
    LayerError, LayerKey, LayerPlacement, ScaleHandle, ScrollBarVisibility, ScrollPaneHandle,
    ViewportHandle, ZoomManager,
};
use crate::geometry::{Point, Rectangle};
use crate::{Figure, FigureId, FreeformError, RuntimeMutationError, UpdateManager, ZoomError};

pub struct RuntimeViewports<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeViewports<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn add(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ViewportHandle, RuntimeMutationError> {
        self.runtime.add_viewport(parent, bounds)
    }

    pub fn handle(&self, viewport: FigureId) -> Result<ViewportHandle, RuntimeMutationError> {
        self.runtime.validate_attached_figure(viewport)?;
        self.runtime
            .tree
            .viewport_handle(viewport)
            .ok_or(RuntimeMutationError::WrongCapability {
                figure: viewport,
                capability: "viewport mutation",
            })
    }

    pub fn scale_handle(&self, figure: FigureId) -> Result<ScaleHandle, RuntimeMutationError> {
        self.runtime.validate_attached_figure(figure)?;
        self.runtime
            .tree
            .scale_handle(figure)
            .ok_or(RuntimeMutationError::WrongCapability {
                figure,
                capability: "scale mutation",
            })
    }

    pub fn set_view_location(
        &mut self,
        viewport: FigureId,
        x: f64,
        y: f64,
    ) -> Result<bool, RuntimeMutationError> {
        let handle = self.handle(viewport)?;
        self.runtime.set_view_location(&handle, x, y)
    }

    pub fn set_horizontal_location(
        &mut self,
        viewport: FigureId,
        x: f64,
    ) -> Result<bool, RuntimeMutationError> {
        let current = self.handle(viewport)?.view_location();
        self.set_view_location(viewport, x, current.y())
    }

    pub fn set_vertical_location(
        &mut self,
        viewport: FigureId,
        y: f64,
    ) -> Result<bool, RuntimeMutationError> {
        let current = self.handle(viewport)?.view_location();
        self.set_view_location(viewport, current.x(), y)
    }

    pub fn scroll_by(
        &mut self,
        viewport: FigureId,
        dx: f64,
        dy: f64,
    ) -> Result<bool, RuntimeMutationError> {
        let current = self.handle(viewport)?.view_location();
        self.set_view_location(viewport, current.x() + dx, current.y() + dy)
    }

    pub fn set_contents(
        &mut self,
        viewport: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        let handle = self.handle(viewport)?;
        self.runtime.guarded_runtime_mutation(move |runtime| {
            if let Some(previous) = handle.contents(&runtime.tree) {
                runtime.dispose_subtree(previous)?;
            }
            runtime.add_figure_inner(viewport, figure)
        })
    }

    pub fn set_tracks_width(
        &mut self,
        viewport: FigureId,
        tracks: bool,
    ) -> Result<bool, RuntimeMutationError> {
        let handle = self.handle(viewport)?;
        self.runtime.guarded_runtime_mutation(|runtime| {
            handle
                .set_tracks_width(&mut runtime.tree, &mut runtime.updates, tracks)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn set_tracks_height(
        &mut self,
        viewport: FigureId,
        tracks: bool,
    ) -> Result<bool, RuntimeMutationError> {
        let handle = self.handle(viewport)?;
        self.runtime.guarded_runtime_mutation(|runtime| {
            handle
                .set_tracks_height(&mut runtime.tree, &mut runtime.updates, tracks)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn set_scale(
        &mut self,
        figure: FigureId,
        scale: f64,
    ) -> Result<bool, RuntimeMutationError> {
        let handle = self.scale_handle(figure)?;
        self.runtime.guarded_runtime_mutation(|runtime| {
            handle
                .set_scale(&mut runtime.tree, &mut runtime.updates, scale)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn set_scroll_bar_visibility(
        &mut self,
        pane: &ScrollPaneHandle,
        horizontal: ScrollBarVisibility,
        vertical: ScrollBarVisibility,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.validate_attached_figure(pane.pane_id())?;
        let pane = pane.clone();
        self.runtime.guarded_runtime_mutation(|runtime| {
            pane.set_scroll_bar_visibility(
                &mut runtime.tree,
                &mut runtime.updates,
                horizontal,
                vertical,
            )
            .map_err(|_| RuntimeMutationError::Rejected)
        })
    }

    pub fn scroll_to(
        &mut self,
        pane: &ScrollPaneHandle,
        x: f64,
        y: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.set_view_location(pane.viewport().figure_id(), x, y)
    }

    pub fn set_scroll_contents(
        &mut self,
        pane: &ScrollPaneHandle,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, RuntimeMutationError> {
        self.set_contents(pane.viewport().figure_id(), figure)
    }

    pub fn set_zoom(
        &mut self,
        zoom: &ZoomManager,
        scale: f64,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_zoom_at(zoom, scale, None)
    }

    pub fn set_zoom_at(
        &mut self,
        zoom: &ZoomManager,
        scale: f64,
        anchor: Option<Point>,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime.set_zoom_at(zoom, scale, anchor)
    }

    pub fn zoom_in(&mut self, zoom: &ZoomManager) -> Result<bool, RuntimeMutationError> {
        self.mutate_zoom(zoom, |zoom, tree, updates| zoom.zoom_in(tree, updates))
    }

    pub fn zoom_out(&mut self, zoom: &ZoomManager) -> Result<bool, RuntimeMutationError> {
        self.mutate_zoom(zoom, |zoom, tree, updates| zoom.zoom_out(tree, updates))
    }

    pub fn fit_all(&mut self, zoom: &ZoomManager) -> Result<bool, RuntimeMutationError> {
        self.runtime.fit_zoom_to_contents(zoom)
    }

    pub fn fit_width(&mut self, zoom: &ZoomManager) -> Result<bool, RuntimeMutationError> {
        self.mutate_zoom(zoom, |zoom, tree, updates| zoom.fit_width(tree, updates))
    }

    pub fn fit_height(&mut self, zoom: &ZoomManager) -> Result<bool, RuntimeMutationError> {
        self.mutate_zoom(zoom, |zoom, tree, updates| zoom.fit_height(tree, updates))
    }

    pub fn freeform_extent(&self, figure: FigureId) -> Result<Rectangle, FreeformError> {
        self.runtime.tree.freeform_extent(figure)
    }

    fn mutate_zoom(
        &mut self,
        zoom: &ZoomManager,
        operation: impl FnOnce(
            &ZoomManager,
            &mut crate::FigureTree,
            &mut UpdateManager,
        ) -> Result<bool, ZoomError>,
    ) -> Result<bool, RuntimeMutationError> {
        self.runtime
            .validate_attached_figure(zoom.viewport().figure_id())?;
        self.runtime
            .validate_attached_figure(zoom.scalable().figure_id())?;
        self.runtime.guarded_runtime_mutation(|runtime| {
            operation(zoom, &mut runtime.tree, &mut runtime.updates)
                .map_err(|_| RuntimeMutationError::Rejected)
        })
    }
}

pub struct RuntimeLayers<'a> {
    runtime: &'a mut Runtime,
}

impl<'a> RuntimeLayers<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        Self { runtime }
    }

    pub fn add_pane(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<FigureId, LayerError> {
        if self.runtime.faulted {
            return Err(LayerError::Faulted);
        }
        self.runtime
            .guarded(|runtime| runtime.add_layered_pane_inner(parent, bounds))
    }

    pub fn add(
        &mut self,
        pane: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        self.runtime.add_layer(pane, figure, key, placement)
    }

    pub fn remove(&mut self, pane: FigureId, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.runtime.remove_layer(pane, key)
    }

    pub fn move_to(
        &mut self,
        pane: FigureId,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.runtime.move_layer(pane, key, placement)
    }

    pub fn reparent(
        &mut self,
        child: FigureId,
        pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.runtime.reparent_layer(child, pane, key, placement)
    }

    pub fn layer(&mut self, pane: FigureId, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.runtime.validate_layered_pane_handle(pane)?;
        self.runtime.layer(pane, key)
    }

    pub fn key(&mut self, pane: FigureId, child: FigureId) -> Result<LayerKey, LayerError> {
        self.runtime.validate_layered_pane_handle(pane)?;
        self.runtime.layer_key(pane, child)
    }

    pub fn ids(&mut self, pane: FigureId) -> Result<Vec<FigureId>, LayerError> {
        self.runtime.validate_layered_pane_handle(pane)?;
        self.runtime.layer_ids(pane)
    }
}
