use novadraw_example_support::{run_demo_app, run_demo_app_with_screenshot};

fn main() {
    let scenes = novadraw_example_scenes::border::suite().into_entries();
    let result = if std::env::args().any(|arg| arg == "--screenshot") {
        run_demo_app_with_screenshot("Border App", "border-app", scenes, true)
    } else {
        run_demo_app("Border App", "border-app", scenes)
    };

    result.expect("Failed to run app");
}
