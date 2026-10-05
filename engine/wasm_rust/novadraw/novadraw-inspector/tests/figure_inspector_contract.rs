use std::num::NonZeroUsize;

use novadraw::render::{BackendCapabilities, RenderOutcome, SurfaceInfo};
use novadraw::{
    FigureEvent, FigureTree, FramePreparation, NotificationEffect, Rectangle, RectangleFigure,
    Runtime, StableQueryError,
};
use novadraw_inspector::FigureInspector;

const EVENT_CAPACITY: usize = 32;
const NARROW_EVENT_CAPACITY: usize = 1;

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 160.0,
        logical_height: 120.0,
        pixel_width: 160,
        pixel_height: 120,
        scale_factor: 1.0,
    }
}

fn stabilize(runtime: &mut Runtime) {
    let preparation = runtime.prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL);
    let FramePreparation::Ready(submission) = preparation else {
        panic!("fixture preparation: {preparation:?}");
    };
    assert!(runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Presented,
    ));
    assert!(runtime.stable_query().is_ok());
}

fn runtime_with_contents() -> (Runtime, novadraw::FigureId, novadraw::FigureId) {
    let mut tree = FigureTree::new();
    let mut builder = tree.builder();
    let contents = builder.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 120.0, 80.0)));
    let child = builder
        .add_child(
            contents,
            Box::new(RectangleFigure::new(10.0, 12.0, 20.0, 24.0)),
        )
        .expect("valid FigureTree construction");
    (Runtime::new(tree), contents, child)
}

#[test]
fn captures_parent_first_tree_and_committed_effect_order() {
    let (mut runtime, contents, child) = runtime_with_contents();
    stabilize(&mut runtime);

    let inspector =
        FigureInspector::new(NonZeroUsize::new(EVENT_CAPACITY).expect("non-zero capacity"));
    inspector.attach(&mut runtime);
    let changed_bounds = Rectangle::new(16.0, 18.0, 32.0, 36.0);
    assert!(
        runtime
            .figure(child)
            .expect("child must remain attached")
            .set_bounds(changed_bounds)
            .expect("valid Runtime mutation")
    );
    stabilize(&mut runtime);

    let snapshot = inspector.capture(&runtime).expect("runtime is stable");
    assert_eq!(snapshot.nodes().len(), 3);
    assert_eq!(snapshot.nodes()[0].id, runtime.tree().root_id());
    assert_eq!(snapshot.nodes()[1].id, contents);
    assert_eq!(snapshot.nodes()[2].id, child);
    assert_eq!(snapshot.nodes()[2].parent, Some(contents));
    assert_eq!(snapshot.nodes()[2].bounds, changed_bounds);
    assert_eq!(snapshot.nodes()[2].figure_name, "RectangleFigure");

    let events = inspector.events();
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(events.iter().any(|event| {
        matches!(
            event.effect,
            NotificationEffect::EmitFigure(FigureEvent::FigureMoved {
                figure_id,
                new_bounds,
                ..
            }) if figure_id == child && new_bounds == changed_bounds
        )
    }));
}

#[test]
fn rejects_unstable_capture_and_bounds_its_event_history() {
    let (mut runtime, contents, _) = runtime_with_contents();
    let inspector =
        FigureInspector::new(NonZeroUsize::new(NARROW_EVENT_CAPACITY).expect("non-zero capacity"));
    inspector.attach(&mut runtime);

    assert!(matches!(
        inspector.capture(&runtime),
        Err(StableQueryError::NotStable { .. })
    ));

    stabilize(&mut runtime);
    assert!(
        runtime
            .figure(contents)
            .expect("contents must remain attached")
            .set_bounds(Rectangle::new(1.0, 2.0, 120.0, 80.0))
            .expect("valid Runtime mutation")
    );
    stabilize(&mut runtime);

    assert!(inspector.events().len() <= NARROW_EVENT_CAPACITY);
    inspector.clear_events();
    assert!(inspector.events().is_empty());
}
