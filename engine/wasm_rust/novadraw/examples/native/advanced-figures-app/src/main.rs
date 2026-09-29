use novadraw_example_support::{
    VerificationCli, run_runtime_demo_app, run_runtime_demo_app_with_scene_screenshot,
    run_runtime_demo_app_with_screenshot,
};

fn main() {
    let title = "Novadraw Advanced Figures";
    let app_name = "advanced-figures-app";
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let suite = novadraw_example_scenes::advanced_figures::suite();
    let screenshot_index = cli.screenshot.as_deref().map(|scenario| {
        scenario
            .parse::<usize>()
            .ok()
            .or_else(|| {
                suite
                    .scenes
                    .iter()
                    .position(|scene| scene.id == scenario || scene.title == scenario)
            })
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            })
    });
    let scenes = suite.into_entries();

    let result = if cli.screenshot_all {
        run_runtime_demo_app_with_screenshot(title, app_name, scenes, true)
    } else if let Some(index) = screenshot_index {
        run_runtime_demo_app_with_scene_screenshot(title, app_name, scenes, index)
    } else {
        run_runtime_demo_app(title, app_name, scenes)
    };

    result.expect("Failed to run advanced figures app");
}
