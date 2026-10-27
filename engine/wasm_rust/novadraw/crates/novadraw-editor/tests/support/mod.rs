use novadraw::render::{BackendCapabilities, RenderDisposition, RenderSubmission, SurfaceInfo};
use novadraw::runtime::FramePreparation;
use novadraw_editor::{EditPartFactory, GraphicalViewer, ModelAdapter};

pub fn prepare_next_submission<A, F>(viewer: &mut GraphicalViewer<A, F>) -> Option<RenderSubmission>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    let FramePreparation::Ready(submission) = viewer.drive().prepare_submission(
        SurfaceInfo {
            logical_width: 2_048.0,
            logical_height: 2_048.0,
            pixel_width: 2_048,
            pixel_height: 2_048,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients(),
    ) else {
        return None;
    };
    let _ = viewer
        .drive()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("test submission completion");
    Some(submission)
}
