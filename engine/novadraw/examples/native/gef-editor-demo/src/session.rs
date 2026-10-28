use crate::model::{Diagram, ShapeFactory};
use novadraw::render::{RenderBackend, SurfaceInfo};
use novadraw::runtime::{FramePreparation, SubmissionAcknowledgement};
use novadraw_editor::{EditorDomain, GraphicalViewer};

/// Editor state shared by the egui host and headless tests.
pub struct Session {
    pub viewer: GraphicalViewer<Diagram, ShapeFactory>,
    pub domain: EditorDomain<Diagram>,
    surface: Option<SurfaceInfo>,
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
}

impl Session {
    pub fn new(viewer: GraphicalViewer<Diagram, ShapeFactory>) -> Self {
        Self {
            viewer,
            domain: EditorDomain::new(),
            surface: None,
        }
    }

    pub fn snapshot(&self, error: Option<String>) -> CanvasAction {
        CanvasAction {
            can_undo: self.domain.has_undo(),
            can_redo: self.domain.has_redo(),
            error,
            dirty: self.domain.is_dirty(),
            model_revision: self.viewer.applied_revision().value(),
            active_tool: self.domain.active_tool_key().name(),
        }
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        let result = self.domain.cancel_tool(&mut self.viewer);
        self.viewer.drive().cancel_gestures();
        result.map_err(|error| error.to_string())
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
