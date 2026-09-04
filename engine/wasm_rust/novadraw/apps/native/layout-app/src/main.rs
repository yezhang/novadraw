use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

fn main() {
    let suite = novadraw_demo_scenes::layout::suite();
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

    let result = if args.iter().any(|arg| arg == "--screenshot-all") {
        run_demo_app_with_screenshot("Layout App", "layout-app", scenes, true)
    } else if let Some(index) = index {
        run_demo_app_with_scene_screenshot("Layout App", "layout-app", scenes, index)
    } else {
        run_demo_app("Layout App", "layout-app", scenes)
    };

    result.expect("Failed to run app");
}
