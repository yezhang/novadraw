use draw2d_official_examples::{catalog, panel_entries};
use novadraw_example_support::{AppBuilder, VerificationCli};

const APP_NAME: &str = "draw2d-official-examples";
const TITLE: &str = "Draw2D Official Examples";

fn main() {
    if std::env::args().any(|argument| argument == "--list") {
        print_catalog();
        return;
    }

    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let scenes = panel_entries();
    let mut builder = AppBuilder::new(TITLE)
        .with_app_name(APP_NAME)
        .with_size(1200.0, 800.0);
    builder = if cli.screenshot_all {
        builder.with_screenshot(true)
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scenes.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
        builder.with_screenshot_scene(index)
    } else {
        builder
    };

    builder
        .with_panel_scenes(scenes)
        .run()
        .expect("failed to run Draw2D official examples");
}

fn print_catalog() {
    for entry in catalog() {
        println!(
            "{}\t{}\t{}\t{}",
            entry.status.as_str(),
            entry.id,
            entry.name,
            entry.upstream_path
        );
        for gap in entry.gaps {
            println!("  gap: {gap}");
        }
    }
}
