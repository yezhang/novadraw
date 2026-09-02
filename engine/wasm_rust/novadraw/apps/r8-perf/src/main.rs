use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use novadraw::{
    Bounded, Color, Figure, FigureTree, NdCanvas, Rectangle, RectangleFigure, SceneUpdateManager,
    Updatable, ZoomManager,
};
use serde::Serialize;

const DEFAULT_WARMUP_ITERATIONS: usize = 1;
const DEFAULT_SAMPLE_ITERATIONS: usize = 7;
const LARGE_TREE_FIGURES: usize = 4_096;
const LARGE_TREE_COLUMNS: usize = 64;
const DEEP_TREE_DEPTH: usize = 10_000;
const TEXT_FIGURES: usize = 1_000;
const VIEWPORT_FIGURES: usize = 1_024;
const VIEWPORT_COLUMNS: usize = 32;
const ROOT_WIDTH: f64 = 1_024.0;
const ROOT_HEIGHT: f64 = 768.0;
const DEFAULT_REPORT: &str = "target/performance/r8-baseline.json";

#[derive(Debug)]
struct Cli {
    warmup_iterations: usize,
    sample_iterations: usize,
    report: PathBuf,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            warmup_iterations: DEFAULT_WARMUP_ITERATIONS,
            sample_iterations: DEFAULT_SAMPLE_ITERATIONS,
            report: PathBuf::from(DEFAULT_REPORT),
        }
    }
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut cli = Self::default();
        for argument in std::env::args().skip(1) {
            if let Some(value) = argument.strip_prefix("--warmup=") {
                cli.warmup_iterations = parse_positive("warmup", value)?;
            } else if let Some(value) = argument.strip_prefix("--samples=") {
                cli.sample_iterations = parse_positive("samples", value)?;
            } else if let Some(value) = argument.strip_prefix("--report=") {
                cli.report = PathBuf::from(value);
            } else {
                return Err(format!(
                    "unknown argument: {argument}; expected \
                     --warmup=<count> --samples=<count> --report=<path>"
                ));
            }
        }
        Ok(cli)
    }
}

fn parse_positive(name: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(parsed)
}

#[derive(Serialize)]
struct BenchmarkReport {
    generated_at_unix_seconds: u64,
    profile: &'static str,
    operating_system: &'static str,
    architecture: &'static str,
    warmup_iterations: usize,
    sample_iterations: usize,
    scenarios: Vec<ScenarioReport>,
}

#[derive(Serialize)]
struct ScenarioReport {
    name: &'static str,
    setup_ns: u64,
    min_ns: u64,
    median_ns: u64,
    p95_ns: u64,
    output: usize,
    notes: &'static str,
}

struct TextProbeFigure {
    bounds: Rectangle,
    text: String,
}

impl TextProbeFigure {
    fn new(bounds: Rectangle, text: String) -> Self {
        Self { bounds, text }
    }
}

impl Bounded for TextProbeFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "R8TextProbe"
    }
}

impl Updatable for TextProbeFigure {
    fn validate(&mut self) {}
}

impl Figure for TextProbeFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint_figure_in_bounds(&self, canvas: &mut NdCanvas, bounds: Rectangle) {
        canvas.fill_style(Color::BLACK);
        canvas.fill_text(&self.text, 0.0, bounds.height);
    }
}

fn main() {
    let cli = Cli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if cfg!(debug_assertions) {
        eprintln!("r8-perf must run with --release");
        std::process::exit(2);
    }

    let scenarios = vec![
        benchmark_large_tree_render(&cli),
        benchmark_large_tree_hit_test(&cli),
        benchmark_deep_tree_render(&cli),
        benchmark_text_recording(&cli),
        benchmark_viewport_render(&cli),
    ];
    let report = BenchmarkReport {
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        profile: "release",
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        warmup_iterations: cli.warmup_iterations,
        sample_iterations: cli.sample_iterations,
        scenarios,
    };
    if let Some(parent) = cli.report.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!("create report directory {}: {error}", parent.display())
        });
    }
    let json = serde_json::to_string_pretty(&report).expect("serialize benchmark report");
    std::fs::write(&cli.report, json)
        .unwrap_or_else(|error| panic!("write report {}: {error}", cli.report.display()));
    println!("REPORT {}", cli.report.display());
}

fn benchmark_large_tree_render(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "large_tree_render_4096",
        cli,
        build_large_tree,
        |tree| tree.render().commands().len(),
        "Records a full frame for a flat 4,096-Figure tree.",
    )
}

fn benchmark_large_tree_hit_test(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "large_tree_hit_test_4096",
        cli,
        build_large_tree,
        |tree| {
            usize::from(
                tree.hit_test_simple((ROOT_WIDTH - 1.0, ROOT_HEIGHT - 1.0))
                    .is_some(),
            )
        },
        "Runs a worst-case child scan before hitting the flat tree root.",
    )
}

fn benchmark_deep_tree_render(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "deep_tree_render_10000",
        cli,
        build_deep_tree,
        |tree| tree.render().commands().len(),
        "Records the supported 10,000-level recursive tree boundary.",
    )
}

fn benchmark_text_recording(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "text_recording_1000",
        cli,
        build_text_tree,
        |tree| tree.render().commands().len(),
        "Measures Figure traversal and text command recording, not glyph shaping.",
    )
}

fn benchmark_viewport_render(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "viewport_render_1024",
        cli,
        build_viewport_tree,
        |tree| tree.render().commands().len(),
        "Records clipped, translated and scaled viewport content.",
    )
}

fn benchmark_prepared<T>(
    name: &'static str,
    cli: &Cli,
    prepare: impl FnOnce() -> T,
    mut operation: impl FnMut(&T) -> usize,
    notes: &'static str,
) -> ScenarioReport {
    let setup_start = Instant::now();
    let value = prepare();
    let setup_ns = duration_ns(setup_start.elapsed());

    for _ in 0..cli.warmup_iterations {
        black_box(operation(black_box(&value)));
    }

    let mut samples = Vec::with_capacity(cli.sample_iterations);
    let mut output = 0;
    for _ in 0..cli.sample_iterations {
        let start = Instant::now();
        output = black_box(operation(black_box(&value)));
        samples.push(duration_ns(start.elapsed()));
    }
    samples.sort_unstable();
    let report = ScenarioReport {
        name,
        setup_ns,
        min_ns: samples[0],
        median_ns: percentile(&samples, 50),
        p95_ns: percentile(&samples, 95),
        output,
        notes,
    };
    println!(
        "BENCH {} setup={}ns min={}ns median={}ns p95={}ns output={}",
        report.name, report.setup_ns, report.min_ns, report.median_ns, report.p95_ns, report.output
    );
    report
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let index = (samples.len() - 1) * percentile / 100;
    samples[index]
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn root_tree() -> (FigureTree, novadraw::FigureId) {
    let mut tree = FigureTree::new();
    let root = tree.set_contents(Box::new(RectangleFigure::new_with_color(
        0.0,
        0.0,
        ROOT_WIDTH,
        ROOT_HEIGHT,
        Color::WHITE,
    )));
    (tree, root)
}

fn build_large_tree() -> FigureTree {
    let (mut tree, root) = root_tree();
    for index in 0..LARGE_TREE_FIGURES {
        let column = index % LARGE_TREE_COLUMNS;
        let row = index / LARGE_TREE_COLUMNS;
        tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(
                column as f64 * 8.0,
                row as f64 * 8.0,
                7.0,
                7.0,
            )),
        );
    }
    tree
}

fn build_deep_tree() -> FigureTree {
    let (mut tree, mut parent) = root_tree();
    for _ in 1..DEEP_TREE_DEPTH {
        parent = tree.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
    }
    tree
}

fn build_text_tree() -> FigureTree {
    let (mut tree, root) = root_tree();
    for index in 0..TEXT_FIGURES {
        let row = index / 20;
        let column = index % 20;
        tree.add_child_to(
            root,
            Box::new(TextProbeFigure::new(
                Rectangle::new(column as f64 * 48.0, row as f64 * 15.0, 46.0, 14.0),
                format!("label-{index:04}"),
            )),
        );
    }
    tree
}

fn build_viewport_tree() -> FigureTree {
    let (mut tree, root) = root_tree();
    let viewport = tree
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 800.0, 560.0))
        .expect("attach viewport");
    let scalable = tree
        .add_scalable_layered_pane_to(
            viewport.block_id(),
            Rectangle::new(0.0, 0.0, 2_048.0, 2_048.0),
        )
        .expect("attach scalable pane");
    for index in 0..VIEWPORT_FIGURES {
        let column = index % VIEWPORT_COLUMNS;
        let row = index / VIEWPORT_COLUMNS;
        tree.add_child_to(
            scalable.block_id(),
            Box::new(RectangleFigure::new(
                column as f64 * 56.0,
                row as f64 * 48.0,
                48.0,
                40.0,
            )),
        );
    }
    let mut updates = SceneUpdateManager::new();
    ZoomManager::new(scalable, viewport.clone())
        .set_zoom(&mut tree, &mut updates, 1.5)
        .expect("set viewport zoom");
    viewport
        .set_view_location(&mut tree, &mut updates, 160.0, 120.0)
        .expect("set viewport location");
    tree
}
