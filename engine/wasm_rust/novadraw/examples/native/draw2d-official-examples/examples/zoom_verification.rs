//! Replays repeated scrollbar input for native scale/clipping evidence.
use draw2d_official_examples::runnable_entries;
use novadraw::{event::MouseButton, runtime::Runtime};
use novadraw_example_support::AppBuilder;

const REPETITIONS: usize = 20;
const ARROW_INSET: f64 = 8.0;

fn scene(stage: usize) -> Runtime {
    let mut entries = runnable_entries();
    let (_, factory) = entries
        .iter_mut()
        .find(|(name, _)| *name == "ZoomExample")
        .expect("Zoom");
    let mut runtime = factory();
    runtime
        .frames()
        .stabilize_for_query()
        .expect("initial layout");
    let root = runtime.tree().contents().expect("root");
    let bar = runtime
        .tree()
        .child_order(root)
        .expect("children")
        .into_iter()
        .find(|id| runtime.tree().node(*id).expect("child").figure_name() == "ScrollBar")
        .expect("bar");
    for increase in [false, true, false].into_iter().take(stage) {
        for _ in 0..REPETITIONS {
            let bounds = runtime.tree().figure_bounds(bar).expect("bar bounds");
            let x = if increase {
                bounds.width - ARROW_INSET
            } else {
                ARROW_INSET
            };
            let y = bounds.height / 2.0;
            runtime
                .input()
                .dispatch_mouse_pressed(x, y, MouseButton::Left);
            runtime
                .input()
                .dispatch_mouse_released(x, y, MouseButton::Left);
            runtime
                .frames()
                .stabilize_for_query()
                .expect("button layout");
        }
    }
    runtime
}

fn main() {
    let scenes = [
        "ZoomInitial",
        "ZoomMinimum",
        "ZoomMaximum",
        "ZoomMinimumAgain",
    ]
    .into_iter()
    .enumerate()
    .map(|(stage, name)| {
        (
            name,
            Box::new(move || scene(stage)) as Box<dyn FnMut() -> Runtime>,
        )
    })
    .collect();
    AppBuilder::new("Zoom verification")
        .with_app_name("draw2d-official-examples")
        .with_size(800.0, 800.0)
        .with_runtime_scenes_boxed(scenes)
        .with_screenshot(true)
        .run()
        .expect("native Zoom verification");
}
