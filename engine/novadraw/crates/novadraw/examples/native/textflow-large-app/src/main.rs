mod native;
mod research;
mod trace;

use anyhow::{Context, Result, bail};
use research::{Operation, Research, TraceSession, surface};
use serde_json::json;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    headless: bool,
    replay: bool,
    cycles: usize,
    trace: bool,
    requests: bool,
    gpu_wait: bool,
    exit_after_replay: bool,
    screenshot: bool,
    scroll_replay: bool,
    natural_damage: bool,
    text_raster_cache: bool,
    raster_validation: bool,
    coarse_trace: bool,
    offscreen_gpu: bool,
    output: PathBuf,
}

impl Config {
    fn parse() -> Result<Option<Self>> {
        let mut config = Self {
            headless: false,
            replay: false,
            cycles: 1,
            trace: false,
            requests: false,
            gpu_wait: false,
            exit_after_replay: false,
            screenshot: false,
            scroll_replay: false,
            natural_damage: false,
            text_raster_cache: false,
            raster_validation: false,
            coarse_trace: false,
            offscreen_gpu: false,
            output: "target/verification/textflow-large-app".into(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--headless" => config.headless = true,
                "--replay" => config.replay = true,
                "--trace" => config.trace = true,
                "--scroll-replay" => {
                    config.scroll_replay = true;
                    config.replay = true;
                }
                "--natural-damage" => config.natural_damage = true,
                "--text-raster-cache" => config.text_raster_cache = true,
                "--offscreen-gpu" => config.offscreen_gpu = true,
                "--raster-validation" => {
                    config.raster_validation = true;
                    config.replay = true;
                    config.screenshot = true;
                }
                "--coarse-trace" => {
                    config.coarse_trace = true;
                    config.trace = true;
                }
                "--requests" => {
                    config.requests = true;
                    config.trace = true;
                }
                "--gpu-wait" => config.gpu_wait = true,
                "--exit-after-replay" => {
                    config.exit_after_replay = true;
                    config.replay = true;
                }
                "--screenshot" => config.screenshot = true,
                "--cycles" => {
                    config.cycles = args.next().context("--cycles needs a count")?.parse()?;
                    config.replay = true;
                }
                "--output" => {
                    config.output = args.next().context("--output needs a directory")?.into()
                }
                "--help" | "-h" => {
                    println!(
                        "TextFlowLarge research\n\
                        --headless --replay --cycles N --trace --requests\n\
                        --gpu-wait --exit-after-replay --screenshot --output DIRECTORY\n\
                        --scroll-replay --natural-damage --text-raster-cache\n\
                        --raster-validation --coarse-trace --offscreen-gpu\n\
                        F1 reset / F2 append Z / F3 scroll 100 / F4 resize / F5 stabilize\n\
                        F6 replay / F7 export / F8 capture / Escape close"
                    );
                    return Ok(None);
                }
                _ => bail!("unknown argument: {arg}"),
            }
        }
        anyhow::ensure!(
            !config.headless
                || (!config.offscreen_gpu
                    && !config.scroll_replay
                    && !config.text_raster_cache
                    && !config.raster_validation),
            "scroll and raster experiments require a native window"
        );
        anyhow::ensure!(
            (1..=10_000).contains(&config.cycles),
            "cycles must be between 1 and 10000"
        );
        anyhow::ensure!(
            !config.headless || (!config.gpu_wait && !config.screenshot),
            "--gpu-wait and --screenshot require a native window"
        );
        Ok(Some(config))
    }
}

fn main() -> Result<()> {
    let Some(config) = Config::parse()? else {
        return Ok(());
    };
    if config.headless {
        run_headless(config)
    } else {
        native::run(config)
    }
}

fn run_headless(config: Config) -> Result<()> {
    let mut trace = TraceSession::new(&config)?;
    let mut research = Research::new(config.trace);
    research.full_redraw = !config.natural_damage;
    let mut renderer = novadraw_backend_vello::scene::VelloSceneRenderer::new();
    let mut scene = novadraw_backend_vello::scene::Scene::new();
    let result = (|| {
        let operations = if config.replay {
            research::replay(config.cycles)
        } else {
            vec![Operation::Reset]
        };
        for operation in operations {
            if matches!(operation, Operation::Reset) {
                if research.has_operations() && trace.has_records() {
                    let path = trace.export(&config.output)?;
                    println!("TextFlowLarge export: {}", path.display());
                }
                renderer = novadraw_backend_vello::scene::VelloSceneRenderer::new();
            }
            research.apply(operation, &mut trace)?;
            let mut backend = renderer.bind(&mut scene);
            research.render(
                &mut backend,
                surface(research.width),
                "cpu_recorded",
                &mut trace,
            )?;
        }
        Ok(())
    })();
    if let Err(error) = &result {
        trace.push(json!({"kind":"failure", "phase":"headless", "error":format!("{error:#}")}));
    }
    if trace.has_records() {
        let path = trace.export(&config.output)?;
        println!("TextFlowLarge export: {}", path.display());
    }
    result
}
