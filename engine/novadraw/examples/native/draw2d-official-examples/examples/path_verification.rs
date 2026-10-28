//! Native replay of the official Path ScrollBar at its two limits.
use draw2d_official_examples::runnable_entries;
use novadraw::{event::MouseButton, runtime::Runtime};
use novadraw_example_support::AppBuilder;

fn scene(maximum: bool) -> Runtime {
    let mut entries = runnable_entries();
    let (_, factory) = entries
        .iter_mut()
        .find(|(name, _)| *name == "PathExample")
        .unwrap();
    let mut runtime = factory();
    runtime.frames().stabilize_for_query().unwrap();
    if maximum {
        let root = runtime.tree().contents().unwrap();
        let bar = runtime.tree().child_order(root).unwrap()[0];
        for _ in 0..20 {
            let bounds = runtime.tree().figure_bounds(bar).unwrap();
            runtime.input().dispatch_mouse_pressed(
                bounds.width - 8.0,
                bounds.height / 2.0,
                MouseButton::Left,
            );
            runtime.input().dispatch_mouse_released(
                bounds.width - 8.0,
                bounds.height / 2.0,
                MouseButton::Left,
            );
            runtime.frames().stabilize_for_query().unwrap();
        }
    }
    runtime
}
fn main() {
    AppBuilder::new("Path verification")
        .with_app_name("draw2d-official-examples")
        .with_size(800.0, 800.0)
        .with_runtime_scenes_boxed(vec![
            ("PathMinimum", Box::new(|| scene(false))),
            ("PathMaximum", Box::new(|| scene(true))),
        ])
        .with_screenshot(true)
        .run()
        .expect("Path verification");
}
