use novadraw_apps::{
    VerificationCli, run_runtime_demo_app, run_runtime_demo_app_with_scene_screenshot,
    run_runtime_demo_app_with_screenshot,
};

fn main() {
    let title = "Widgets App - Clickable, Button, and Toggle";
    let app_name = "widgets-app";
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let scenes = novadraw_demo_scenes::widget::entries();

    let result = if cli.screenshot_all {
        run_runtime_demo_app_with_screenshot(title, app_name, scenes, true)
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scenes.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
        run_runtime_demo_app_with_scene_screenshot(title, app_name, scenes, index)
    } else {
        run_runtime_demo_app(title, app_name, scenes)
    };

    result.expect("Failed to run widgets demo");
}
