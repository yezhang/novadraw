use novadraw::runtime::Runtime;

#[unsafe(no_mangle)]
pub extern "C" fn empty_owner() -> bool {
    Runtime::empty().is_faulted()
}

#[cfg(feature = "probe-inspector")]
#[unsafe(no_mangle)]
pub extern "C" fn inspect_frame() {
    use novadraw::{
        figure::RectangleFigure,
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::FramePreparation,
    };
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap()))
        .unwrap();
    let inspector = novadraw_inspector::FigureInspector::new(std::num::NonZeroUsize::new(64).unwrap());
    // Identical source accepts the baseline ListenerId and current fallible token.
    let subscription = std::hint::black_box(inspector.attach(&mut runtime));
    let surface = SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 100,
        pixel_height: 100,
        scale_factor: 1.0,
    };
    if let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    {
        let _ = std::hint::black_box(runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        ));
    }
    let _ = std::hint::black_box(inspector.events());
    let _ = std::hint::black_box(subscription);
}
