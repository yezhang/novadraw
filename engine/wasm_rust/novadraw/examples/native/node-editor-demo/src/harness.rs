use std::time::Duration;

use novadraw::event::{KeyModifiers, MonotonicTime, MouseButton};
use novadraw::figure::border::{BorderStyle, LineBorder};
use novadraw::render::text::BuiltinFont;
use novadraw::render::{
    BackendCapabilities, BackendSessionId, FrameId, RenderDisposition, RenderResult,
    RenderSubmission, SurfaceInfo,
};
use novadraw::runtime::{SubmissionAcknowledgement, SubmissionCompletionError};
use novadraw::{
    figure::FigureStyle, figure::RectangleFigure, geometry::Point, geometry::Rectangle,
    graphics::Color, runtime::FramePreparation, tree::FigureId,
};
use novadraw_editor::overlay::{
    BendpointCreateAction, BendpointHandleSite, BendpointMoveAction, ConnectionEndpointAction,
    HandleActionProtocol, ResizeAction,
};
use novadraw_editor::{
    AutoexposeTick, ConnectionEndpoint, ConnectionPartId, CreateChild, CreateRequest, CreationType,
    DeleteParts, DeleteRequest, DirectTextFeature, EditorDomain, RequestMetadata, RequestModifiers,
    ResizeDirection, SessionTextInputEvent, TextInputEffect,
};

use super::{
    BENDPOINT_CREATE_HANDLE_COLOR, BENDPOINT_CREATE_HANDLE_SIZE, BENDPOINT_HANDLE_COLOR,
    CREATED_NODE_HEIGHT, CREATED_NODE_OFFSET, CREATED_NODE_WIDTH, DemoFactory, DemoModel,
    DemoViewer, HANDLE_SIZE, HEIGHT, NodeId, PRIMARY_HANDLE_COLOR, SECONDARY_HANDLE_COLOR,
    VIEWPORT_BACKGROUND_COLOR, VIEWPORT_BORDER_COLOR, VIEWPORT_BORDER_WIDTH, WIDTH,
};

pub(crate) type HarnessResult<T> = Result<T, String>;

/// Platform-neutral owner of one complete demo editor session.
pub(crate) struct EditorHarness {
    viewer: DemoViewer,
    domain: EditorDomain<DemoModel>,
    handles: Vec<FigureId>,
}

impl EditorHarness {
    pub(crate) fn new() -> HarnessResult<Self> {
        let mut viewer = DemoViewer::new(
            DemoModel::new(),
            DemoFactory,
            Rectangle::new(0.0, 0.0, WIDTH, HEIGHT),
        )
        .map_err(|error| error.to_string())?;
        viewer
            .drive()
            .register_builtin_font(BuiltinFont::Inter)
            .map_err(|error| error.to_string())?;
        viewer
            .drive()
            .register_builtin_font(BuiltinFont::NotoSansSc)
            .map_err(|error| error.to_string())?;
        let style_changed = viewer
            .set_root_viewport_style(FigureStyle {
                background: Some(
                    Color::from_hex(VIEWPORT_BACKGROUND_COLOR).expect("valid color literal"),
                ),
                ..FigureStyle::default()
            })
            .map_err(|error| error.to_string())?;
        if !style_changed {
            return Err("failed to apply viewport background".to_string());
        }
        viewer
            .set_root_viewport_border(
                LineBorder::new(
                    Color::from_hex(VIEWPORT_BORDER_COLOR).expect("valid color literal"),
                    VIEWPORT_BORDER_WIDTH,
                )
                .with_style(BorderStyle::Dash)
                .with_insets(0.0, 0.0, 0.0, 0.0),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            viewer,
            domain: EditorDomain::new(),
            handles: Vec::new(),
        })
    }

    pub(crate) fn prepare_frame(&mut self) -> Option<RenderSubmission> {
        let submission = self
            .viewer
            .drive()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: WIDTH,
                    logical_height: HEIGHT,
                    pixel_width: WIDTH as u32,
                    pixel_height: HEIGHT as u32,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL,
            )
            .into_ready()
            .ok()?;
        let _ = self
            .viewer
            .drive()
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .ok()?;
        Some(submission)
    }

    pub(crate) fn prepare_submission(
        &mut self,
        surface: SurfaceInfo,
        capabilities: BackendCapabilities,
    ) -> FramePreparation {
        self.viewer
            .drive()
            .prepare_submission(surface, capabilities)
    }

    pub(crate) fn complete_submission(
        &mut self,
        session_id: BackendSessionId,
        frame_id: FrameId,
        result: RenderResult,
    ) -> Result<SubmissionAcknowledgement, SubmissionCompletionError> {
        self.viewer
            .drive()
            .complete_submission(session_id, frame_id, result)
    }

    pub(crate) fn has_pending_update(&mut self) -> bool {
        self.viewer.drive().has_pending_update()
    }

    pub(crate) fn reset_backend_session(&mut self) -> HarnessResult<BackendSessionId> {
        self.viewer
            .drive()
            .reset_backend_session()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> HarnessResult<bool> {
        self.viewer
            .drive()
            .advance_time(now)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn next_wake_deadline(&mut self) -> Option<MonotonicTime> {
        self.viewer.drive().next_wake_deadline()
    }

    pub(crate) fn title_status(&self) -> String {
        format!(
            "{} selected | undo {} redo {}{}",
            self.viewer.selection().items().len(),
            self.domain.command_stack().undo_len(),
            self.domain.command_stack().redo_len(),
            if self.direct_text_edit_active() {
                " | TEXT EDIT"
            } else if self.domain.is_connection_creation_active() {
                " | CONNECTION"
            } else {
                ""
            },
        )
    }

    pub(crate) fn pointer_pressed(
        &mut self,
        location: Point,
        button: MouseButton,
        modifiers: KeyModifiers,
    ) -> HarnessResult<bool> {
        let outcome = self
            .domain
            .pointer_pressed(&mut self.viewer, location, button, modifiers)
            .map_err(|error| error.to_string())?;
        let selection_changed = outcome.selection().is_some();
        if selection_changed {
            self.sync_selection_handles()?;
        }
        Ok(selection_changed)
    }

    pub(crate) fn pointer_moved(&mut self, location: Point) -> HarnessResult<()> {
        let origin_before = self
            .viewer
            .viewport_origin()
            .map_err(|error| error.to_string())?;
        self.domain
            .pointer_moved(&mut self.viewer, location)
            .map_err(|error| error.to_string())?;
        let origin_after = self
            .viewer
            .viewport_origin()
            .map_err(|error| error.to_string())?;
        if origin_before != origin_after {
            self.sync_selection_handles()?;
        }
        Ok(())
    }

    pub(crate) fn pointer_released(
        &mut self,
        location: Point,
        button: MouseButton,
    ) -> HarnessResult<bool> {
        let release = self
            .domain
            .pointer_released(&mut self.viewer, location, button)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()?;
        Ok(release.command_executed())
    }

    pub(crate) fn click(&mut self, location: Point, modifiers: KeyModifiers) -> HarnessResult<()> {
        self.pointer_pressed(location, MouseButton::Left, modifiers)?;
        self.pointer_released(location, MouseButton::Left)?;
        Ok(())
    }

    pub(crate) fn drag(
        &mut self,
        start: Point,
        end: Point,
        modifiers: KeyModifiers,
    ) -> HarnessResult<bool> {
        self.pointer_pressed(start, MouseButton::Left, modifiers)?;
        self.pointer_moved(end)?;
        self.pointer_released(end, MouseButton::Left)
    }

    pub(crate) fn pointer_exited(&mut self) -> HarnessResult<()> {
        self.cancel_tool()?;
        self.domain.pointer_exited(&mut self.viewer);
        Ok(())
    }

    pub(crate) fn autoexpose_requested(&self) -> bool {
        self.domain.autoexpose_requested()
    }

    pub(crate) fn autoexpose_tick(&mut self, elapsed: Duration) -> HarnessResult<AutoexposeTick> {
        let outcome = self
            .domain
            .autoexpose_tick(&mut self.viewer, elapsed)
            .map_err(|error| error.to_string())?;
        if outcome.scrolled() {
            self.sync_selection_handles()?;
        }
        Ok(outcome)
    }

    pub(crate) fn scroll_by(&mut self, dx: f64, dy: f64) -> HarnessResult<bool> {
        let origin = self
            .viewer
            .viewport_origin()
            .map_err(|error| error.to_string())?;
        let changed = self
            .domain
            .set_viewport_origin(
                &mut self.viewer,
                Point::new(origin.x() + dx, origin.y() + dy),
            )
            .map_err(|error| error.to_string())?;
        if changed {
            self.sync_selection_handles()?;
        }
        Ok(changed)
    }

    pub(crate) fn zoom_by(&mut self, factor: f64, anchor: Point) -> HarnessResult<bool> {
        let current = self
            .viewer
            .viewport_scale()
            .map_err(|error| error.to_string())?;
        let changed = self
            .domain
            .set_viewport_scale_at(&mut self.viewer, current * factor, Some(anchor))
            .map_err(|error| error.to_string())?;
        if changed {
            self.sync_selection_handles()?;
        }
        Ok(changed)
    }

    pub(crate) fn resize_logical_viewport(
        &mut self,
        width: f64,
        height: f64,
    ) -> HarnessResult<bool> {
        let changed = self
            .viewer
            .drive()
            .resize_logical_viewport(width, height)
            .map_err(|error| error.to_string())?;
        if changed {
            self.refresh_overlay_positions()?;
        }
        Ok(changed)
    }

    pub(crate) fn refresh_overlay_positions(&mut self) -> HarnessResult<()> {
        self.domain
            .refresh_after_viewport_change(&mut self.viewer)
            .map_err(|error| error.to_string())?;
        self.viewer
            .synchronize_direct_text_input_area()
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()
    }

    pub(crate) fn cancel_tool(&mut self) -> HarnessResult<()> {
        self.domain
            .cancel_tool(&mut self.viewer)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()
    }

    pub(crate) fn focus_lost(&mut self) -> HarnessResult<()> {
        if self.viewer.direct_text_edit().is_some() {
            self.domain
                .direct_text_focus_lost(&mut self.viewer)
                .map_err(|error| error.to_string())?;
        } else {
            self.cancel_tool()?;
        }
        self.viewer.pointer_exited();
        self.viewer.drive().cancel_gestures();
        self.viewer.drive().release_focus();
        Ok(())
    }

    pub(crate) fn start_rename_selected(&mut self) -> HarnessResult<bool> {
        let Some(source) = self.viewer.selection().primary() else {
            return Ok(false);
        };
        self.domain
            .start_direct_text_edit(
                &mut self.viewer,
                source,
                DirectTextFeature::new("label").map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()?;
        Ok(true)
    }

    pub(crate) fn handle_text_input(&mut self, event: SessionTextInputEvent) -> HarnessResult<()> {
        self.domain
            .handle_text_input_event(&mut self.viewer, event)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn take_text_input_effects(&mut self) -> Vec<TextInputEffect> {
        self.viewer.take_text_input_effects()
    }

    pub(crate) fn direct_text_edit_active(&self) -> bool {
        self.viewer.direct_text_edit().is_some()
    }

    pub(crate) fn activate_connection_creation(&mut self) -> HarnessResult<()> {
        self.domain
            .activate_connection_creation(
                &mut self.viewer,
                CreationType::new("connection").map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
    }

    pub(crate) fn create_node(&mut self) -> HarnessResult<()> {
        let sequence = self.viewer.model().next_id.saturating_sub(5) as f64;
        let offset = sequence * CREATED_NODE_OFFSET;
        let request = CreateRequest::new(
            self.viewer.contents(),
            CreationType::new("shape").map_err(|error| error.to_string())?,
            Rectangle::new(
                420.0 + offset,
                330.0 + offset,
                CREATED_NODE_WIDTH,
                CREATED_NODE_HEIGHT,
            ),
            RequestModifiers::default(),
            self.domain
                .next_interaction_revision()
                .map_err(|error| error.to_string())?,
        );
        let metadata = RequestMetadata::new(request.revision(), vec![request.parent()]);
        self.domain
            .execute::<CreateChild, _>(&mut self.viewer, &metadata, &request)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()
    }

    pub(crate) fn delete_selection(&mut self) -> HarnessResult<bool> {
        let parts = self.viewer.selection().items().to_vec();
        if parts.is_empty() {
            return Ok(false);
        }
        let request = DeleteRequest::new(
            parts,
            self.domain
                .next_interaction_revision()
                .map_err(|error| error.to_string())?,
        );
        let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
        self.domain
            .execute::<DeleteParts, _>(&mut self.viewer, &metadata, &request)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()?;
        Ok(true)
    }

    pub(crate) fn undo(&mut self) -> HarnessResult<bool> {
        if self.domain.command_stack().undo_len() == 0 {
            return Ok(false);
        }
        self.domain
            .undo(&mut self.viewer)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()?;
        Ok(true)
    }

    pub(crate) fn redo(&mut self) -> HarnessResult<bool> {
        if self.domain.command_stack().redo_len() == 0 {
            return Ok(false);
        }
        self.domain
            .redo(&mut self.viewer)
            .map_err(|error| error.to_string())?;
        self.sync_selection_handles()?;
        Ok(true)
    }

    pub(crate) fn selected_model_ids(&self) -> Vec<u64> {
        self.viewer
            .selection()
            .items()
            .iter()
            .filter_map(|part| self.viewer.parts().get(*part)?.model_id())
            .map(|id| id.0)
            .collect()
    }

    pub(crate) fn node_bounds(&self, id: u64) -> Option<Rectangle> {
        self.viewer
            .model()
            .nodes
            .get(&NodeId(id))
            .map(|node| node.bounds)
    }

    #[cfg(test)]
    pub(crate) fn node_label(&self, id: u64) -> Option<&str> {
        self.viewer
            .model()
            .labels
            .get(&NodeId(id))
            .map(String::as_str)
    }

    pub(crate) fn node_bounds_in_surface(&self, id: u64) -> Option<Rectangle> {
        self.viewer
            .part_for_model(NodeId(id))
            .and_then(|part| self.viewer.part_bounds_in_surface(part))
    }

    pub(crate) fn viewport_origin(&self) -> HarnessResult<Point> {
        self.viewer
            .viewport_origin()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn viewport_scale(&self) -> HarnessResult<f64> {
        self.viewer
            .viewport_scale()
            .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    pub(crate) fn handle_centers_in_surface(&self) -> Vec<Point> {
        self.handles
            .iter()
            .map(|figure| {
                let tree = self.viewer.runtime().tree();
                let bounds = tree.figure_bounds(*figure).expect("live handle");
                let transform = tree
                    .local_to_surface_transform(*figure)
                    .expect("valid handle transform");
                transform.transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0))
            })
            .collect()
    }

    pub(crate) fn node_count(&self) -> usize {
        self.viewer.model().nodes.len()
    }

    pub(crate) fn model_revision(&self) -> u64 {
        self.viewer.model().revision.value()
    }

    pub(crate) fn history_lengths(&self) -> (usize, usize) {
        (
            self.domain.command_stack().undo_len(),
            self.domain.command_stack().redo_len(),
        )
    }

    pub(crate) fn connection_ids(&self) -> Vec<u64> {
        self.viewer
            .model()
            .connections
            .iter()
            .map(|connection| connection.id().0)
            .collect()
    }

    pub(crate) fn connection_endpoints(&self, id: u64) -> Option<(u64, u64)> {
        self.viewer
            .model()
            .connections
            .iter()
            .find(|connection| connection.id() == NodeId(id))
            .map(|connection| (connection.source().0, connection.target().0))
    }

    pub(crate) fn connection_bendpoints(&self, id: u64) -> Vec<Point> {
        self.viewer
            .model()
            .bendpoints
            .get(&NodeId(id))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn connection_route(&self, id: u64) -> Option<Vec<Point>> {
        let connection = self.viewer.connection_part_for_model(NodeId(id))?;
        self.viewer.connection_route_points_in_surface(connection)
    }

    pub(crate) fn scaled_feedback_point_lists(&self) -> Vec<Vec<Point>> {
        let tree = self.viewer.runtime().tree();
        tree.child_order(self.viewer.root_layers().scaled_feedback())
            .expect("live feedback layer")
            .into_iter()
            .filter_map(|figure| tree.point_list_points(figure).expect("live feedback child"))
            .collect()
    }

    pub(crate) fn bendpoint_handle_sites(
        &mut self,
        id: u64,
    ) -> HarnessResult<Vec<BendpointHandleSite>> {
        let connection = self.connection_part(id)?;
        self.viewer
            .connection_bendpoint_handle_sites(connection)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn has_active_gesture(&self) -> bool {
        self.domain.has_active_gesture()
    }

    pub(crate) fn is_connection_creation_active(&self) -> bool {
        self.domain.is_connection_creation_active()
    }

    fn connection_part(&self, id: u64) -> HarnessResult<ConnectionPartId> {
        self.viewer
            .connection_part_for_model(NodeId(id))
            .ok_or_else(|| format!("connection model {id} has no live ConnectionPart"))
    }

    fn sync_selection_handles(&mut self) -> HarnessResult<()> {
        for figure in self.handles.drain(..) {
            let _ = self.viewer.remove_overlay_visual(figure);
        }
        let selected = self.viewer.selection().items().to_vec();
        let primary = self.viewer.selection().primary();
        for part in selected {
            if let Some(connection) = self.viewer.as_connection_part(part) {
                self.add_connection_handles(part, connection)?;
                continue;
            }
            let Some(bounds) = self.viewer.part_bounds_in_surface(part) else {
                continue;
            };
            let color = if Some(part) == primary {
                PRIMARY_HANDLE_COLOR
            } else {
                SECONDARY_HANDLE_COLOR
            };
            for (x, y, direction) in [
                (bounds.x, bounds.y, ResizeDirection::NorthWest),
                (
                    bounds.x + bounds.width,
                    bounds.y,
                    ResizeDirection::NorthEast,
                ),
                (
                    bounds.x,
                    bounds.y + bounds.height,
                    ResizeDirection::SouthWest,
                ),
                (
                    bounds.x + bounds.width,
                    bounds.y + bounds.height,
                    ResizeDirection::SouthEast,
                ),
            ] {
                let handle = RectangleFigure::new_with_color(
                    x - HANDLE_SIZE / 2.0,
                    y - HANDLE_SIZE / 2.0,
                    HANDLE_SIZE,
                    HANDLE_SIZE,
                    color,
                )
                .with_stroke(Color::BLACK, 1.0);
                let result = if Some(part) == primary {
                    self.viewer.add_handle_visual_with_action::<ResizeAction>(
                        part,
                        direction,
                        Box::new(handle),
                    )
                } else {
                    self.viewer.add_handle_visual(part, Box::new(handle))
                };
                let (_, figure) = result.map_err(|error| error.to_string())?;
                self.handles.push(figure);
            }
        }
        Ok(())
    }

    fn add_connection_handles(
        &mut self,
        owner: novadraw_editor::EditPartId,
        connection: ConnectionPartId,
    ) -> HarnessResult<()> {
        let Some(route) = self.viewer.connection_route_points_in_surface(connection) else {
            return Ok(());
        };
        let Some((&source, &target)) = route.first().zip(route.last()) else {
            return Ok(());
        };
        for (point, endpoint) in [
            (source, ConnectionEndpoint::Source),
            (target, ConnectionEndpoint::Target),
        ] {
            self.add_handle::<ConnectionEndpointAction>(
                owner,
                point,
                endpoint,
                HANDLE_SIZE,
                PRIMARY_HANDLE_COLOR,
            )?;
        }
        let sites = self
            .viewer
            .connection_bendpoint_handle_sites(connection)
            .map_err(|error| error.to_string())?;
        for site in sites {
            if let Some(index) = site.action().get::<BendpointCreateAction>().copied() {
                self.add_handle::<BendpointCreateAction>(
                    owner,
                    site.location(),
                    index,
                    BENDPOINT_CREATE_HANDLE_SIZE,
                    BENDPOINT_CREATE_HANDLE_COLOR,
                )?;
            } else if let Some(index) = site.action().get::<BendpointMoveAction>().copied() {
                self.add_handle::<BendpointMoveAction>(
                    owner,
                    site.location(),
                    index,
                    HANDLE_SIZE,
                    BENDPOINT_HANDLE_COLOR,
                )?;
            }
        }
        Ok(())
    }

    fn add_handle<R: HandleActionProtocol>(
        &mut self,
        owner: novadraw_editor::EditPartId,
        point: Point,
        payload: R::Payload,
        size: f64,
        color: Color,
    ) -> HarnessResult<()> {
        let handle = RectangleFigure::new_with_color(
            point.x() - size / 2.0,
            point.y() - size / 2.0,
            size,
            size,
            color,
        )
        .with_stroke(Color::BLACK, 1.0);
        let (_, figure) = self
            .viewer
            .add_handle_visual_with_action::<R>(owner, payload, Box::new(handle))
            .map_err(|error| error.to_string())?;
        self.handles.push(figure);
        Ok(())
    }
}
