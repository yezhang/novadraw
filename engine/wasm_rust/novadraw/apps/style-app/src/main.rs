use novadraw_apps::{run_demo_app, run_demo_app_with_screenshot};

fn main() {
    let scenes = novadraw_demo_scenes::style::suite().into_entries();
    let result = if std::env::args().any(|arg| arg == "--screenshot") {
        run_demo_app_with_screenshot(
            "Style App - Visual Style Properties",
            "style-app",
            scenes,
            true,
        )
    } else {
        run_demo_app("Style App - Visual Style Properties", "style-app", scenes)
    };

    result.expect("Failed to run app");
}
