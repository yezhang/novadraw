use crate::render::ImagingSceneRenderer;
use masonry::imaging::record::Scene;
use masonry::kurbo::Size;
use novadraw::render::{RenderBackend, SurfaceInfo};
use novadraw::runtime::{FramePreparation, SubmissionAcknowledgement};
use novadraw_editor::{EditPartFactory, EditorDomain, GraphicalViewer, ModelAdapter};

/// Session controller shared by headless consumers and the native widget.
pub struct EditorSession<A: ModelAdapter + 'static, F: EditPartFactory<A>> {
    pub viewer: GraphicalViewer<A, F>,
    pub domain: EditorDomain<A>,
    renderer: ImagingSceneRenderer,
    scene: Scene,
    surface: Option<SurfaceInfo>,
}

/// UI projection emitted after input, without exporting the business model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CanvasAction {
    pub can_undo: bool,
    pub can_redo: bool,
    pub error: Option<String>,
    pub dirty: bool,
    pub model_revision: u64,
    pub active_tool: &'static str,
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> EditorSession<A, F> {
    pub fn new(viewer: GraphicalViewer<A, F>) -> Self {
        Self {
            viewer,
            domain: EditorDomain::new(),
            renderer: ImagingSceneRenderer::default(),
            scene: Scene::new(),
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

    /// Cancel transient tool and Figure gestures without committing them.
    pub fn cancel(&mut self) -> Result<(), String> {
        let result = self.domain.cancel_tool(&mut self.viewer);
        self.viewer.drive().cancel_gestures();
        result.map_err(|error| error.to_string())
    }

    /// Publish a full logical Scene. Returns true when another attempt is needed.
    /// A failed publication preserves the last successfully encoded Scene.
    pub fn render(&mut self, size: Size, scale: f64) -> Result<bool, String> {
        let pixel_width = (size.width * scale).ceil();
        let pixel_height = (size.height * scale).ceil();
        if !scale.is_finite()
            || scale <= 0.0
            || !pixel_width.is_finite()
            || !pixel_height.is_finite()
            || pixel_width < 0.0
            || pixel_height < 0.0
            || pixel_width > u32::MAX as f64
            || pixel_height > u32::MAX as f64
        {
            return Err("invalid canvas extent or DPI".into());
        }
        let surface = SurfaceInfo {
            logical_width: size.width,
            logical_height: size.height,
            pixel_width: pixel_width as u32,
            pixel_height: pixel_height as u32,
            scale_factor: scale,
        };
        if self.surface != Some(surface) {
            self.viewer
                .drive()
                .resize_logical_viewport(size.width, size.height)
                .map_err(|error| error.to_string())?;
            self.viewer.drive().request_full_redraw();
            self.surface = Some(surface);
        }
        let capabilities = self.renderer.bind(&mut self.scene).capabilities();
        match self
            .viewer
            .drive()
            .prepare_submission(surface, capabilities)
        {
            FramePreparation::Ready(submission) => {
                let result = self.renderer.bind(&mut self.scene).submit(&submission);
                let acknowledgement = self
                    .viewer
                    .drive()
                    .complete_submission(submission.session_id, submission.frame_id, result)
                    .map_err(|error| error.to_string())?;
                match acknowledgement {
                    SubmissionAcknowledgement::Presented | SubmissionAcknowledgement::Skipped => {
                        Ok(false)
                    }
                    SubmissionAcknowledgement::Retry => Ok(true),
                    SubmissionAcknowledgement::Rejected(error)
                    | SubmissionAcknowledgement::Terminated(error) => Err(error.to_string()),
                    other => Err(format!("unexpected Scene backend recovery: {other:?}")),
                }
            }
            FramePreparation::Idle | FramePreparation::Suspended => Ok(false),
            FramePreparation::AwaitingCompletion => {
                Err("canvas submission awaits completion".into())
            }
            FramePreparation::Error(error) => Err(format!("{error:?}")),
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}
