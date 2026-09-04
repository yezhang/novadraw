use novadraw_apps::{run_demo_app, run_demo_app_with_scene_screenshot};

fn main() {
    let suite = novadraw_demo_scenes::ndcanvas::suite();
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--screenshot="));
    let index = screenshot.map(|scenario| {
        scenario
            .parse::<usize>()
            .ok()
            .or_else(|| suite.scenes.iter().position(|scene| scene.id == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            })
    });
    let scenes = suite.into_entries();

    let result = if let Some(index) = index {
        run_demo_app_with_scene_screenshot("NdCanvas API Test", "ndcanvas-app", scenes, index)
    } else {
        run_demo_app("NdCanvas API Test", "ndcanvas-app", scenes)
    };

    result.expect("Failed to run app");
}
