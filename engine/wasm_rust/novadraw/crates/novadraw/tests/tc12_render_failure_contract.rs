use novadraw::render::{
    BackendCapabilities, DamageMode, RenderDisposition, RenderFailure, RenderFailureDiagnostic,
    ResourceSync, SurfaceInfo,
};
use novadraw::runtime::{FramePreparation, SubmissionAcknowledgement, SubmissionCompletionError};
use novadraw::{figure::RectangleFigure, runtime::Runtime};

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 160.0,
        logical_height: 120.0,
        pixel_width: 160,
        pixel_height: 120,
        scale_factor: 1.0,
    }
}

fn runtime_with_frame() -> (Runtime, novadraw::render::RenderSubmission) {
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 80.0, 60.0)))
        .expect("valid fixture");
    let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("fixture must prepare a frame");
    };
    (runtime, submission)
}

#[test]
fn dispositions_preserve_their_distinct_acknowledgements() {
    for (disposition, expected) in [
        (
            RenderDisposition::Presented,
            SubmissionAcknowledgement::Presented,
        ),
        (
            RenderDisposition::Skipped,
            SubmissionAcknowledgement::Skipped,
        ),
        (RenderDisposition::Retry, SubmissionAcknowledgement::Retry),
    ] {
        let (mut runtime, submission) = runtime_with_frame();
        assert_eq!(
            runtime.frames().complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(disposition),
            ),
            Ok(expected)
        );

        if disposition != RenderDisposition::Presented {
            let retry = runtime
                .frames()
                .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
                .into_ready()
                .expect("non-presented frame must remain pending");
            assert_eq!(retry.session_id, submission.session_id);
            assert_eq!(retry.damage.mode(), DamageMode::Full);
        }
    }
}

#[test]
fn surface_loss_retries_full_frame_without_invalidating_backend_session() {
    let (mut runtime, submission) = runtime_with_frame();
    let diagnostic = RenderFailureDiagnostic::new("surface drawable was lost");

    assert_eq!(
        runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id,
            Err(RenderFailure::SurfaceLost(diagnostic)),
        ),
        Ok(SubmissionAcknowledgement::SurfaceInvalidated {
            diagnostic: RenderFailureDiagnostic::new("surface drawable was lost"),
        })
    );

    let retry = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("surface recovery must retry");
    assert_eq!(retry.session_id, submission.session_id);
    assert_eq!(retry.damage.mode(), DamageMode::Full);
}

#[test]
fn device_loss_invalidates_session_and_refreezes_snapshot() {
    let (mut runtime, submission) = runtime_with_frame();
    let previous = submission.session_id;

    let acknowledgement = runtime
        .frames()
        .complete_submission(
            previous,
            submission.frame_id,
            Err(RenderFailure::DeviceLost(RenderFailureDiagnostic::new(
                "adapter reset",
            ))),
        )
        .expect("device loss must be acknowledged");
    let SubmissionAcknowledgement::BackendSessionInvalidated {
        previous_session,
        next_session,
        diagnostic,
    } = acknowledgement
    else {
        panic!("device loss must invalidate the backend session");
    };
    assert_eq!(previous_session, previous);
    assert_ne!(next_session, previous);
    assert_eq!(diagnostic.summary(), "adapter reset");

    let replacement = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("replacement backend must receive a recovery frame");
    assert_eq!(replacement.session_id, next_session);
    assert_eq!(replacement.damage.mode(), DamageMode::Full);
    assert!(matches!(replacement.resources, ResourceSync::Snapshot(_)));
}

#[test]
fn deterministic_and_terminal_failures_do_not_collapse_into_retry() {
    for (failure, expected) in [
        (
            RenderFailure::InvalidInput(novadraw::render::InvalidGraphicsInput {
                command_index: 4,
                reason: novadraw::graphics::GraphicsInputError::InvalidDeviceScale,
            }),
            SubmissionAcknowledgement::Rejected(RenderFailure::InvalidInput(
                novadraw::render::InvalidGraphicsInput {
                    command_index: 4,
                    reason: novadraw::graphics::GraphicsInputError::InvalidDeviceScale,
                },
            )),
        ),
        (
            RenderFailure::OutOfMemory(RenderFailureDiagnostic::new("GPU allocation failed")),
            SubmissionAcknowledgement::Terminated(RenderFailure::OutOfMemory(
                RenderFailureDiagnostic::new("GPU allocation failed"),
            )),
        ),
        (
            RenderFailure::Fatal(RenderFailureDiagnostic::new("encoder failed")),
            SubmissionAcknowledgement::Terminated(RenderFailure::Fatal(
                RenderFailureDiagnostic::new("encoder failed"),
            )),
        ),
    ] {
        let (mut runtime, submission) = runtime_with_frame();
        assert_eq!(
            runtime.frames().complete_submission(
                submission.session_id,
                submission.frame_id,
                Err(failure),
            ),
            Ok(expected)
        );
    }
}

#[test]
fn stale_or_duplicate_completion_is_structurally_rejected() {
    let (mut runtime, submission) = runtime_with_frame();
    assert_eq!(
        runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id.next(),
            Ok(RenderDisposition::Presented),
        ),
        Err(SubmissionCompletionError::MismatchedFrame {
            expected: submission.frame_id,
            actual: submission.frame_id.next(),
        })
    );
    assert_eq!(
        runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        ),
        Ok(SubmissionAcknowledgement::Presented)
    );
    assert_eq!(
        runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        ),
        Err(SubmissionCompletionError::NoInFlightFrame)
    );
}
