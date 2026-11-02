use draw2d_official_examples::{catalog, official_scene_title, panel_entries};
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
    let scenes = if std::env::var_os("NOVADRAW_THUMBNAIL_EVIDENCE").is_some() {
        draw2d_official_examples::thumbnail_verification_entries()
    } else {
        panel_entries()
    };
    let verify = cli.verify;
    let mut builder = AppBuilder::new(TITLE)
        .with_app_name(APP_NAME)
        .with_scene_title_formatter(|name| {
            let title = official_scene_title(name);
            if name == "InheritBackgroundExample" {
                format!("{title} | Left: orange / Right: inherited gradient | arrows: scene | Alt+arrows: move Canvas | Tab: focus | B: background | X: remove | F5: repair")
            } else {
                title
            }
        })
        .with_window_scene("InheritBackgroundExample", move |window| {
            draw2d_official_examples::inherit_background_host::create(window, verify)
        })
        .with_size(1200.0, 800.0);
    builder = if cli.screenshot_all {
        builder.with_screenshot(true)
    } else if let Some(scenario) = cli.screenshot.or_else(|| {
        verify.then(|| {
            cli.scenario
                .unwrap_or_else(|| "InheritBackgroundExample".to_string())
        })
    }) {
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
    for (index, entry) in catalog().into_iter().enumerate() {
        println!(
            "{:02}\t{}\t{}\t{}\t{}",
            index + 1,
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
