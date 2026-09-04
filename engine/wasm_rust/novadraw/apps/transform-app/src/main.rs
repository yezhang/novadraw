use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

fn main() {
    let suite = novadraw_demo_scenes::transform::suite();
    let screenshot =
        std::env::args().find_map(|arg| arg.strip_prefix("--screenshot=").map(str::to_owned));
    let screenshot_all = std::env::args().any(|arg| arg == "--screenshot-all");
    let index = screenshot.as_deref().map(|scenario| {
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

    let result = if screenshot_all {
        run_demo_app_with_screenshot("Transform App", "transform-app", scenes, true)
    } else if let Some(index) = index {
        run_demo_app_with_scene_screenshot("Transform App", "transform-app", scenes, index)
    } else {
        run_demo_app("Transform App", "transform-app", scenes)
    };

    result.expect("failed to run transform-app");
}
