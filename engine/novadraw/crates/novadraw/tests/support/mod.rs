use novadraw::render::{BackendCapabilities, RenderDisposition, RenderSubmission, SurfaceInfo};
use novadraw::{runtime::FramePreparation, runtime::Runtime};

pub fn prepare_next_submission(runtime: &mut Runtime) -> Option<RenderSubmission> {
    let FramePreparation::Ready(submission) = runtime.frames().prepare_submission(
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
    let _ = runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("test submission completion");
    Some(submission)
}

pub fn record_full_submission(runtime: &mut Runtime) -> RenderSubmission {
    runtime.frames().request_full_redraw();
    prepare_next_submission(runtime).expect("full redraw must produce a submission")
}
