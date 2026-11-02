use crate::model::{
    Diagram, ShapeFactory, WorkspaceSettings, resize_handle_sites, selection_handle,
};
use novadraw::render::{RenderBackend, SurfaceInfo};
use novadraw::runtime::{FramePreparation, SubmissionAcknowledgement};
use novadraw::{
    geometry::{Point, Rectangle},
    tree::FigureId,
};
use novadraw_editor::overlay::{ConnectionEndpointAction, ResizeAction};
use novadraw_editor::{ConnectionEndpoint, EditPartId, EditorDomain, GraphicalViewer};

#[derive(Clone, Debug, PartialEq)]
enum HandleProjection {
    Shape {
        part: EditPartId,
        bounds: Rectangle,
        primary: bool,
        interactive: bool,
    },
    Connection {
        part: EditPartId,
        source: Point,
        target: Point,
        primary: bool,
    },
}

/// Editor state shared by the egui host and headless tests.
pub struct Session {
    pub viewer: GraphicalViewer<Diagram, ShapeFactory>,
    pub domain: EditorDomain<Diagram>,
    surface: Option<SurfaceInfo>,
    handles: Vec<FigureId>,
    handle_projection: Vec<HandleProjection>,
    pub(crate) workspace: WorkspaceSettings,
}

/// Stable application projection; no model or host-framework types escape here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CanvasAction {
    pub can_undo: bool,
    pub can_redo: bool,
    pub error: Option<String>,
    pub dirty: bool,
    pub model_revision: u64,
    pub active_tool: &'static str,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub zoom_percent: u16,
}

impl Session {
    pub fn new(
        viewer: GraphicalViewer<Diagram, ShapeFactory>,
        workspace: WorkspaceSettings,
    ) -> Self {
        Self {
            viewer,
            domain: EditorDomain::new(),
            surface: None,
            handles: Vec::new(),
            handle_projection: Vec::new(),
            workspace,
        }
    }

    pub fn snapshot(&self, error: Option<String>) -> CanvasAction {
        let settings = self.workspace.get();
        CanvasAction {
            can_undo: self.domain.has_undo(),
            can_redo: self.domain.has_redo(),
            error,
            dirty: self.domain.is_dirty(),
            model_revision: self.viewer.applied_revision().value(),
            active_tool: self.domain.active_tool_key().name(),
            grid_visible: settings.show_grid,
            snap_enabled: settings.snap_to_grid,
            zoom_percent: self.viewer.viewport_scale().map_or(100, |scale| {
                (scale * 100.0).round().clamp(1.0, 1000.0) as u16
            }),
        }
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        let result = self.domain.cancel_tool(&mut self.viewer);
        self.viewer.drive().cancel_gestures();
        result.map_err(|error| error.to_string())
    }

    pub(crate) fn sync_selection_handles(&mut self) -> Result<bool, String> {
        if self.domain.has_active_gesture() {
            return self.clear_selection_handles();
        }
        let projection = self.current_handle_projection();
        if projection == self.handle_projection
            && (!self.handles.is_empty() || projection.is_empty())
        {
            return Ok(false);
        }
        self.clear_selection_handles()?;
        for item in &projection {
            match *item {
                HandleProjection::Shape {
                    part,
                    bounds,
                    primary,
                    interactive,
                } => self.add_shape_handles(part, bounds, primary && interactive)?,
                HandleProjection::Connection {
                    part,
                    source,
                    target,
                    primary,
                } => self.add_connection_handles(part, source, target, primary)?,
            }
        }
        self.handle_projection = projection;
        Ok(true)
    }

    fn clear_selection_handles(&mut self) -> Result<bool, String> {
        let changed = !self.handles.is_empty();
        for figure in self.handles.drain(..) {
            self.viewer
                .remove_overlay_visual(figure)
                .map_err(|error| error.to_string())?;
        }
        Ok(changed)
    }

    #[cfg(test)]
    pub(crate) fn handle_visual_count(&self) -> usize {
        self.handles.len()
    }

    fn current_handle_projection(&self) -> Vec<HandleProjection> {
        let primary = self.viewer.selection().primary();
        self.viewer
            .selection()
            .items()
            .iter()
            .filter_map(|part| {
                if let Some(connection) = self.viewer.as_connection_part(*part) {
                    let (source, target) = self
                        .viewer
                        .connection_route_endpoints_in_surface(connection)?;
                    Some(HandleProjection::Connection {
                        part: *part,
                        source,
                        target,
                        primary: Some(*part) == primary,
                    })
                } else {
                    let model_id = self
                        .viewer
                        .parts()
                        .get(*part)
                        .and_then(|part| part.model_id())?;
                    let presentation = self.viewer.model().presentation(model_id);
                    if !presentation.visible {
                        return None;
                    }
                    self.viewer.part_bounds_in_surface(*part).map(|bounds| {
                        HandleProjection::Shape {
                            part: *part,
                            bounds,
                            primary: Some(*part) == primary,
                            interactive: !presentation.locked,
                        }
                    })
                }
            })
            .collect()
    }

    fn add_shape_handles(
        &mut self,
        part: EditPartId,
        bounds: Rectangle,
        primary: bool,
    ) -> Result<(), String> {
        for (point, direction) in resize_handle_sites(bounds) {
            let figure = selection_handle(point);
            let (_, figure) = if primary {
                self.viewer.add_handle_visual_with_action::<ResizeAction>(
                    part,
                    direction,
                    Box::new(figure),
                )
            } else {
                self.viewer.add_handle_visual(part, Box::new(figure))
            }
            .map_err(|error| error.to_string())?;
            self.handles.push(figure);
        }
        Ok(())
    }

    fn add_connection_handles(
        &mut self,
        part: EditPartId,
        source: Point,
        target: Point,
        primary: bool,
    ) -> Result<(), String> {
        for (point, endpoint) in [
            (source, ConnectionEndpoint::Source),
            (target, ConnectionEndpoint::Target),
        ] {
            let figure = selection_handle(point);
            let (_, figure) = if primary {
                self.viewer
                    .add_handle_visual_with_action::<ConnectionEndpointAction>(
                        part,
                        endpoint,
                        Box::new(figure),
                    )
            } else {
                self.viewer.add_handle_visual(part, Box::new(figure))
            }
            .map_err(|error| error.to_string())?;
            self.handles.push(figure);
        }
        Ok(())
    }

    pub fn set_grid_visible(&mut self, visible: bool) {
        self.workspace
            .update(|settings| settings.show_grid = visible);
        self.viewer.drive().request_full_redraw();
    }

    pub fn set_snap_enabled(&self, enabled: bool) {
        self.workspace
            .update(|settings| settings.snap_to_grid = enabled);
    }

    pub fn set_zoom(&mut self, scale: f64) -> Result<(), String> {
        let viewport = self
            .viewer
            .viewport_bounds_in_surface()
            .map_err(|error| error.to_string())?;
        self.domain
            .set_viewport_scale_at(
                &mut self.viewer,
                scale,
                Some(Point::new(
                    viewport.x + viewport.width / 2.0,
                    viewport.y + viewport.height / 2.0,
                )),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn zoom_by_step(&mut self, direction: i32) -> Result<(), String> {
        let current = self
            .viewer
            .viewport_scale()
            .map_err(|error| error.to_string())?;
        let levels = self.viewer.zoom_manager().zoom_levels();
        let next = if direction < 0 {
            levels
                .iter()
                .rev()
                .copied()
                .find(|level| *level < current - f64::EPSILON)
        } else {
            levels
                .iter()
                .copied()
                .find(|level| *level > current + f64::EPSILON)
        }
        .unwrap_or(current);
        self.set_zoom(next)
    }

    pub fn fit_visible_shapes(&mut self) -> Result<(), String> {
        const FIT_PADDING: f64 = 40.0;
        let model = self.viewer.model();
        let mut visible = model.order.iter().filter_map(|id| {
            model
                .presentation(*id)
                .visible
                .then_some(model.shapes[id].bounds)
        });
        let Some(mut extent) = visible.next() else {
            return Ok(());
        };
        for bounds in visible {
            extent = extent.union(bounds);
        }
        let viewport = self
            .viewer
            .viewport_bounds_in_surface()
            .map_err(|error| error.to_string())?;
        let available_width = (viewport.width - FIT_PADDING * 2.0).max(1.0);
        let available_height = (viewport.height - FIT_PADDING * 2.0).max(1.0);
        let scale = (available_width / extent.width)
            .min(available_height / extent.height)
            .clamp(0.1, 4.0);
        self.domain
            .set_viewport_scale_at(&mut self.viewer, scale, None)
            .map_err(|error| error.to_string())?;
        self.domain
            .set_viewport_origin(
                &mut self.viewer,
                Point::new(
                    extent.x + extent.width / 2.0 - viewport.width / (2.0 * scale),
                    extent.y + extent.height / 2.0 - viewport.height / (2.0 * scale),
                ),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Publishes one frame through an application-owned backend.
    pub fn render(
        &mut self,
        logical_size: [f64; 2],
        scale_factor: f64,
        backend: &mut impl RenderBackend,
    ) -> Result<bool, String> {
        let pixel_width = (logical_size[0] * scale_factor).ceil();
        let pixel_height = (logical_size[1] * scale_factor).ceil();
        if !scale_factor.is_finite()
            || scale_factor <= 0.0
            || logical_size
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || !pixel_width.is_finite()
            || !pixel_height.is_finite()
            || pixel_width > u32::MAX as f64
            || pixel_height > u32::MAX as f64
        {
            return Err("invalid canvas extent or DPI".into());
        }
        let surface = SurfaceInfo {
            logical_width: logical_size[0],
            logical_height: logical_size[1],
            pixel_width: pixel_width as u32,
            pixel_height: pixel_height as u32,
            scale_factor,
        };
        if self.surface != Some(surface) {
            self.viewer
                .drive()
                .resize_logical_viewport(logical_size[0], logical_size[1])
                .map_err(|error| error.to_string())?;
            self.viewer.drive().request_full_redraw();
            self.surface = Some(surface);
        }
        match self
            .viewer
            .drive()
            .prepare_submission(surface, backend.capabilities())
        {
            FramePreparation::Ready(submission) => {
                let result = backend.submit(&submission);
                match self
                    .viewer
                    .drive()
                    .complete_submission(submission.session_id, submission.frame_id, result)
                    .map_err(|error| error.to_string())?
                {
                    SubmissionAcknowledgement::Presented | SubmissionAcknowledgement::Skipped => {
                        Ok(false)
                    }
                    SubmissionAcknowledgement::Retry => Ok(true),
                    SubmissionAcknowledgement::Rejected(error)
                    | SubmissionAcknowledgement::Terminated(error) => Err(error.to_string()),
                    other => Err(format!("unexpected renderer recovery: {other:?}")),
                }
            }
            FramePreparation::Idle | FramePreparation::Suspended => Ok(false),
            FramePreparation::AwaitingCompletion => {
                Err("canvas submission awaits completion".into())
            }
            FramePreparation::Error(error) => Err(format!("{error:?}")),
        }
    }
}
