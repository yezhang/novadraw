mod editor;
mod embedded;
mod native;
mod performance;

use novadraw_example_support::AppBuilder;
use std::{error::Error, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WIDTH: f64 = 1120.0;
const HEIGHT: f64 = 760.0;

#[derive(Clone)]
struct Config {
    load: String,
    output: PathBuf,
    trace: bool,
    replay: bool,
}
impl Config {
    fn parse(args: &[String]) -> Result<Self> {
        let mut config = Self {
            load: "mixed".into(),
            output: "target/verification/text-richtext-edit/mixed".into(),
            trace: false,
            replay: false,
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--text-load" => {
                    config.load = iter.next().ok_or("missing --text-load value")?.clone()
                }
                "--output" => config.output = iter.next().ok_or("missing --output value")?.into(),
                "--trace" => config.trace = true,
                "--replay-edit" => config.replay = true,
                "--headless" | "--benchmark-edit" | "--benchmark-session" | "--screenshot" => {}
                _ => return Err(format!("unknown argument {arg}").into()),
            }
        }
        if !["mixed", "large", "giant"].contains(&config.load.as_str()) {
            return Err("--text-load expects mixed, large or giant".into());
        }
        Ok(config)
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let config = Config::parse(&args)?;
    if args.iter().any(|arg| arg == "--benchmark-session") {
        return performance::run_session(&config);
    }
    if args.iter().any(|arg| arg == "--benchmark-edit") {
        return performance::run(&config);
    }
    if args.iter().any(|arg| arg == "--headless") {
        editor::verify(&config)?;
        return embedded::verify(&config);
    }
    let scene_config = config.clone();
    let mut app = AppBuilder::new("Richtext Figure Edit")
        .with_app_name("richtext-figure-edit-app")
        .with_size(WIDTH, HEIGHT)
        .with_runtime_scenes_boxed(vec![(
            "Editing performance",
            Box::new(move || {
                editor::Editor::new(&scene_config)
                    .expect("sample scene")
                    .runtime
            }),
        )])
        .with_window_scene("Editing performance", move |window| {
            native::Native::new(window, &config)
                .map(|scene| Box::new(scene) as Box<dyn novadraw_example_support::app::WindowScene>)
                .map_err(|error| std::io::Error::other(error.to_string()))
        });
    if args.iter().any(|arg| arg == "--screenshot") {
        app = app.with_screenshot(true);
    }
    app.run()
}
