//! Native evidence scenes driven through the public input facade.
use draw2d_official_examples::runnable_entries;
use novadraw::{event::MouseButton, geometry::Point, runtime::Runtime};
use novadraw_example_support::AppBuilder;

fn click(runtime: &mut Runtime, point: Point, button: MouseButton) {
    runtime
        .input()
        .dispatch_mouse_pressed(point.x(), point.y(), button);
    runtime
        .input()
        .dispatch_mouse_released(point.x(), point.y(), button);
}

fn scene(stage: usize) -> Runtime {
    let mut entries = runnable_entries();
    let (_, factory) = entries
        .iter_mut()
        .find(|(name, _)| *name == "ObstructionExample")
        .expect("Obstruction scene");
    let mut runtime = factory();
    if stage == 0 {
        return runtime;
    }
    for y in [50.0, 85.0, 120.0] {
        click(&mut runtime, Point::new(20.0, y), MouseButton::Right);
        click(&mut runtime, Point::new(500.0, y), MouseButton::Right);
    }
    if stage >= 2 {
        runtime
            .input()
            .dispatch_mouse_pressed(125.0, 40.0, MouseButton::Left);
        runtime.input().dispatch_mouse_moved(155.0, 180.0);
        runtime
            .input()
            .dispatch_mouse_released(155.0, 180.0, MouseButton::Left);
    }
    if stage >= 3 {
        click(&mut runtime, Point::new(155.0, 180.0), MouseButton::Right);
        click(&mut runtime, Point::new(20.0, 85.0), MouseButton::Middle);
    }
    runtime
        .frames()
        .stabilize_for_query()
        .expect("stable evidence scene");
    runtime
}

fn main() {
    let scenes = [
        "ObstructionInitial",
        "ObstructionShared",
        "ObstructionMoved",
        "ObstructionRemoved",
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
    AppBuilder::new("Obstruction verification")
        .with_app_name("draw2d-official-examples")
        .with_size(900.0, 700.0)
        .with_runtime_scenes_boxed(scenes)
        .with_screenshot(true)
        .run()
        .expect("native evidence");
}
