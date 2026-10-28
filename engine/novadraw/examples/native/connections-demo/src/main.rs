use novadraw_example_support::{
    VerificationCli, run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

fn main() {
    let title = "Connections Demo - Anchor / Router matrix";
    let app_name = "connections-demo";
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let scenes = novadraw_example_scenes::connection::suite().into_entries();

    let result = if cli.screenshot_all {
        run_demo_app_with_screenshot(title, app_name, scenes, true)
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scenes.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
        run_demo_app_with_scene_screenshot(title, app_name, scenes, index)
    } else {
        run_demo_app(title, app_name, scenes)
    };

    result.expect("Failed to run connections demo");
}
