use novadraw_apps::{
    VerificationCli, run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};

const CLIP_TO_VIEWPORT_SCENE: usize = 0;

fn main() {
    let title = "Viewport App - Figure 树视口验证 (按数字键 0-3 切换场景)";
    let app_name = "viewport-app";
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let scenes = novadraw_demo_scenes::viewport::scenes();

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
    } else if std::env::args().any(|arg| arg == "--screenshot-clip") {
        run_demo_app_with_scene_screenshot(title, app_name, scenes, CLIP_TO_VIEWPORT_SCENE)
    } else {
        run_demo_app(title, app_name, scenes)
    };

    result.expect("Failed to run app");
}
