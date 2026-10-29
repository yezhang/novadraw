use crate::{
    model::*,
    parts::{Factory, NodeTemplate, selection_color},
};
use novadraw::connection::ManhattanConnectionRouter;
use novadraw::figure::RectangleFigure;
use novadraw::geometry::{Point, Rectangle};
use novadraw::graphics::Color;
use novadraw::render::{RenderBackend, SurfaceInfo};
use novadraw::runtime::{FramePreparation, SubmissionAcknowledgement};
use novadraw::tree::FigureId;
use novadraw_editor::overlay::ConnectionEndpointAction;
use novadraw_editor::tool::CreationTool;
use novadraw_editor::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Select,
    Connect,
    Create(Kind),
}
pub struct Session {
    pub viewer: GraphicalViewer<Circuit, Factory>,
    pub domain: EditorDomain<Circuit>,
    pub mode: Mode,
    surface: Option<SurfaceInfo>,
    overlays: Vec<FigureId>,
    overlay_revision: Option<(u64, Vec<EditPartId>, f64, Point)>,
}
impl Session {
    pub fn new(circuit: Circuit) -> Result<Self, String> {
        let mut viewer =
            GraphicalViewer::new(circuit, Factory, Rectangle::new(0.0, 0.0, 880.0, 500.0))
                .map_err(|e| e.to_string())?;
        viewer
            .drive()
            .register_builtin_font(novadraw::render::BuiltinFont::Inter)
            .map_err(|e| e.to_string())?;
        let mut result = Self {
            viewer,
            domain: EditorDomain::new(),
            mode: Mode::Select,
            surface: None,
            overlays: Vec::new(),
            overlay_revision: None,
        };
        result
            .viewer
            .replace_connection_layer_router(Box::new(ManhattanConnectionRouter))
            .map_err(|e| e.to_string())?;
        result.sync()?;
        Ok(result)
    }
    pub fn set_mode(&mut self, mode: Mode) -> Result<(), String> {
        match mode {
            Mode::Select => self
                .domain
                .cancel_tool(&mut self.viewer)
                .map_err(|e| e.to_string())?,
            Mode::Connect => self
                .domain
                .activate_connection_creation(
                    &mut self.viewer,
                    CreationType::new("wire").expect("wire type"),
                )
                .map_err(|e| e.to_string())?,
            Mode::Create(kind) => self
                .domain
                .set_active_tool(&mut self.viewer, CreationTool::new(NodeTemplate(kind)))
                .map_err(|e| e.to_string())?,
        }
        self.mode = mode;
        Ok(())
    }
    pub fn set_input(&mut self, id: u64, value: u8) -> Result<(), String> {
        let before = self
            .viewer
            .model()
            .document
            .nodes
            .get(&id)
            .ok_or("Node no longer exists")?
            .clone();
        if before.kind != Kind::Led || self.viewer.model().driven(id) {
            return Err("This LED is driven by its input wires".into());
        }
        if value > LED_MASK {
            return Err("LED value must be between 0 and 15".into());
        }
        if before.value == value {
            return Ok(());
        }
        let after = Node {
            value,
            ..before.clone()
        };
        self.domain
            .execute_command(
                &mut self.viewer,
                Box::new(ChangeNode {
                    id,
                    before: Some(before),
                    after,
                }),
            )
            .map_err(|e| e.to_string())
    }
    pub fn delete(&mut self) -> Result<(), String> {
        let parts = self
            .viewer
            .selection()
            .items()
            .iter()
            .copied()
            .filter(|part| {
                self.viewer
                    .parts()
                    .get(*part)
                    .and_then(|p| p.model_id())
                    .is_some_and(|id| matches!(id, Id::Node(_) | Id::Wire(_)))
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return Ok(());
        }
        let request = DeleteRequest::new(
            parts,
            self.domain
                .next_interaction_revision()
                .map_err(|e| e.to_string())?,
        );
        let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
        self.domain
            .execute::<DeleteParts, _>(&mut self.viewer, &metadata, &request)
            .map_err(|e| e.to_string())
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.domain
            .cancel_tool(&mut self.viewer)
            .map_err(|e| e.to_string())?;
        self.viewer.drive().cancel_gestures();
        self.mode = Mode::Select;
        Ok(())
    }
    pub fn sync(&mut self) -> Result<(), String> {
        self.viewer
            .stabilize_for_query()
            .map_err(|e| e.to_string())?;
        if self.domain.active_tool_key().name() == "selection" {
            self.mode = Mode::Select;
        }
        let projection = (
            self.viewer.applied_revision().value(),
            self.viewer.selection().items().to_vec(),
            self.viewer.viewport_scale().map_err(|e| e.to_string())?,
            self.viewer.viewport_origin().map_err(|e| e.to_string())?,
        );
        if !self.domain.has_active_gesture() && self.overlay_revision.as_ref() == Some(&projection)
        {
            return Ok(());
        }
        for id in self.overlays.drain(..) {
            self.viewer
                .remove_overlay_visual(id)
                .map_err(|e| e.to_string())?;
        }
        self.overlay_revision = None;
        if self.domain.has_active_gesture() {
            return Ok(());
        }
        for part in &projection.1 {
            if let Some(connection) = self.viewer.as_connection_part(*part) {
                if let Some((source, target)) = self
                    .viewer
                    .connection_route_endpoints_in_surface(connection)
                {
                    for (point, endpoint) in [
                        (source, ConnectionEndpoint::Source),
                        (target, ConnectionEndpoint::Target),
                    ] {
                        const HANDLE_SIZE: f64 = 8.0;
                        let figure = RectangleFigure::new_with_color(
                            point.x() - HANDLE_SIZE / 2.0,
                            point.y() - HANDLE_SIZE / 2.0,
                            HANDLE_SIZE,
                            HANDLE_SIZE,
                            Color::WHITE,
                        )
                        .map_err(|e| e.to_string())?
                        .with_stroke(selection_color(), 1.0)
                        .map_err(|e| e.to_string())?;
                        let (_, id) = self
                            .viewer
                            .add_handle_visual_with_action::<ConnectionEndpointAction>(
                                *part,
                                endpoint,
                                Box::new(figure),
                            )
                            .map_err(|e| e.to_string())?;
                        self.overlays.push(id);
                    }
                }
            } else if let Some(b) = self.viewer.part_bounds_in_surface(*part) {
                let figure = RectangleFigure::new_with_color(
                    b.x,
                    b.y,
                    b.width,
                    b.height,
                    Color::TRANSPARENT,
                )
                .map_err(|e| e.to_string())?
                .with_stroke(selection_color(), 2.0)
                .map_err(|e| e.to_string())?;
                self.overlays.push(
                    self.viewer
                        .add_feedback_visual(Some(*part), false, Box::new(figure))
                        .map_err(|e| e.to_string())?
                        .1,
                );
            }
        }
        self.overlay_revision = Some(projection);
        Ok(())
    }
    pub fn render(
        &mut self,
        size: [f64; 2],
        scale: f64,
        backend: &mut impl RenderBackend,
    ) -> Result<bool, String> {
        let surface = SurfaceInfo {
            logical_width: size[0],
            logical_height: size[1],
            pixel_width: (size[0] * scale).ceil() as u32,
            pixel_height: (size[1] * scale).ceil() as u32,
            scale_factor: scale,
        };
        if self.surface != Some(surface) {
            self.viewer
                .drive()
                .resize_logical_viewport(size[0], size[1])
                .map_err(|e| e.to_string())?;
            self.viewer.drive().request_full_redraw();
            self.surface = Some(surface);
            self.overlay_revision = None;
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
                    .map_err(|e| e.to_string())?
                {
                    SubmissionAcknowledgement::Presented | SubmissionAcknowledgement::Skipped => {
                        Ok(false)
                    }
                    SubmissionAcknowledgement::Retry => Ok(true),
                    other => Err(format!("Renderer: {other:?}")),
                }
            }
            FramePreparation::Idle | FramePreparation::Suspended => Ok(false),
            other => Err(format!("Frame preparation: {other:?}")),
        }
    }
}
