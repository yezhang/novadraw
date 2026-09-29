use novadraw::render::{BackendCapabilities, DamageMode, RenderOutcome, SurfaceInfo};
use novadraw::{FigureTree, LogicalViewportResizeError, Rectangle, RectangleFigure, Runtime};

fn surface(width: u32, height: u32) -> SurfaceInfo {
    SurfaceInfo {
        logical_width: f64::from(width),
        logical_height: f64::from(height),
        pixel_width: width,
        pixel_height: height,
        scale_factor: 1.0,
    }
}

fn complete_frame(runtime: &mut Runtime, surface: SurfaceInfo) {
    let submission = runtime
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        .expect("resize must produce a frame");
    assert_eq!(submission.damage.mode(), DamageMode::Full);
    assert!(runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Presented
    ));
}

#[test]
fn logical_viewport_resizes_contents_without_rewriting_child_world_coordinates() {
    let mut tree = FigureTree::new();
    let contents = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 240.0)));
    let child = tree
        .builder()
        .add_child(
            contents,
            Box::new(RectangleFigure::new(30.0, 40.0, 80.0, 60.0)),
        )
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);

    assert!(runtime.resize_logical_viewport(640.0, 480.0).unwrap());
    complete_frame(&mut runtime, surface(640, 480));

    assert_eq!(
        runtime.logical_viewport(),
        Some(Rectangle::new(0.0, 0.0, 640.0, 480.0))
    );
    assert_eq!(
        runtime.tree().figure_bounds(contents),
        Some(Rectangle::new(0.0, 0.0, 640.0, 480.0))
    );
    assert_eq!(
        runtime.tree().figure_bounds(child),
        Some(Rectangle::new(30.0, 40.0, 80.0, 60.0))
    );

    assert!(!runtime.resize_logical_viewport(640.0, 480.0).unwrap());
    assert!(
        runtime
            .prepare_submission(surface(640, 480), BackendCapabilities::RETAINED_PARTIAL)
            .is_none()
    );
}

#[test]
fn replacing_contents_after_resize_uses_the_current_logical_viewport() {
    let mut runtime = Runtime::empty();
    assert!(runtime.resize_logical_viewport(500.0, 360.0).unwrap());
    let contents = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .expect("valid Runtime mutation");

    complete_frame(&mut runtime, surface(500, 360));

    assert_eq!(
        runtime.tree().figure_bounds(contents),
        Some(Rectangle::new(0.0, 0.0, 500.0, 360.0))
    );
}

#[test]
fn logical_viewport_rejects_non_finite_and_negative_sizes() {
    let mut runtime = Runtime::empty();

    assert_eq!(
        runtime.resize_logical_viewport(-1.0, 100.0),
        Err(LogicalViewportResizeError::InvalidSize {
            width: -1.0,
            height: 100.0,
        })
    );
    assert!(matches!(
        runtime.resize_logical_viewport(f64::NAN, 100.0),
        Err(LogicalViewportResizeError::InvalidSize { .. })
    ));
    assert_eq!(runtime.logical_viewport(), None);
}
