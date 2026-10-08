use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use novadraw::connection::{
    ChopboxAnchor, ConnectionFigure, ConnectionId, CoordinateSpace, DirectRouter, FanRouter,
    RouterBinding,
};
use novadraw::container::ZoomManager;
use novadraw::figure::FlowPage;
use novadraw::render::BuiltinFont;
use novadraw::{
    Color, Dimension, FigureId, FigureStyle, FigureTree, LabelFigure, Rectangle, RectangleFigure,
    Runtime, TextFlowFigure,
};
use serde::{Deserialize, Serialize};

const REPORT_SCHEMA_VERSION: u32 = 2;
const DEFAULT_WARMUP_ITERATIONS: usize = 5;
const DEFAULT_SAMPLE_ITERATIONS: usize = 30;
const LARGE_TREE_FIGURES: usize = 4_096;
const LARGE_TREE_COLUMNS: usize = 64;
const DEEP_TREE_SHALLOW_DEPTH: usize = 1_000;
const DEEP_TREE_MAX_DEPTH: usize = 10_000;
const TEXT_FIGURES: usize = 1_000;
const TEXT_FLOW_FIGURES: usize = 512;
const INDEPENDENT_CONNECTIONS: usize = 1_000;
const GROUPED_CONNECTIONS: usize = 256;
const VIEWPORT_FIGURES: usize = 1_024;
const VIEWPORT_COLUMNS: usize = 32;
const ROOT_WIDTH: f64 = 1_024.0;
const ROOT_HEIGHT: f64 = 768.0;
const LOGICAL_DPI: f64 = 96.0;
const DEFAULT_REPORT: &str = "target/performance/ga2-novadraw.json";
const SCENARIO_NAMES: &[&str] = &[
    "wide_tree_full_record_4096",
    "deep_tree_full_record_1000",
    "deep_tree_full_record_10000",
    "deep_tree_validate_1000",
    "deep_tree_validate_10000",
    "label_refresh_wide_1000",
    "label_refresh_deep_1000",
    "text_flow_full_record_wide_512",
    "text_flow_full_record_deep_512",
    "independent_routing_1000",
    "grouped_routing_256",
    "local_update_record_1pct_4096",
    "full_update_record_100pct_4096",
    "viewport_full_record_1024",
];

#[derive(Debug)]
struct Cli {
    warmup_iterations: usize,
    sample_iterations: usize,
    report: PathBuf,
    scenario: Option<String>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            warmup_iterations: DEFAULT_WARMUP_ITERATIONS,
            sample_iterations: DEFAULT_SAMPLE_ITERATIONS,
            report: PathBuf::from(DEFAULT_REPORT),
            scenario: None,
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
            } else if let Some(value) = argument.strip_prefix("--scenario=") {
                cli.scenario = Some(value.to_owned());
            } else if argument == "--help" {
                println!(
                    "r8-perf [--warmup=<count>] [--samples=<count>] \
                     [--scenario=<name>] [--report=<path>]"
                );
                std::process::exit(0);
            } else {
                return Err(format!(
                    "unknown argument: {argument}; run with --help for supported arguments"
                ));
            }
        }
        Ok(cli)
    }

    fn includes(&self, name: &str) -> bool {
        self.scenario
            .as_deref()
            .is_none_or(|requested| requested == name)
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

#[derive(Deserialize, Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    generated_at_unix_seconds: u64,
    harness: String,
    environment: EnvironmentReport,
    sampling: SamplingReport,
    measurement_scope: MeasurementScope,
    scenarios: Vec<ScenarioReport>,
}

#[derive(Deserialize, Serialize)]
struct EnvironmentReport {
    git_revision: Option<String>,
    git_dirty: Option<bool>,
    rustc_verbose: Option<String>,
    operating_system: String,
    architecture: String,
    cpu_model: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct SamplingReport {
    warmup_iterations: usize,
    sample_iterations: usize,
}

#[derive(Deserialize, Serialize)]
struct MeasurementScope {
    cpu_setup: bool,
    cpu_operation: bool,
    gpu_submission: bool,
    gpu_execution: bool,
    present: bool,
    input_to_present: bool,
    process_memory: bool,
}

#[derive(Clone, Deserialize, Serialize)]
struct ScenarioConfig {
    figure_count: usize,
    maximum_depth: usize,
    viewport_logical: [f64; 2],
    logical_dpi: f64,
    visible_ratio: f64,
    update_ratio: f64,
    font: Option<String>,
    input_trajectory: String,
}

#[derive(Deserialize, Serialize)]
struct ScenarioReport {
    name: String,
    config: ScenarioConfig,
    setup_ns: u64,
    min_ns: u64,
    p50_ns: u64,
    p95_ns: u64,
    samples_ns: Vec<u64>,
    memory: ProcessMemoryReport,
    work: BTreeMap<String, u64>,
    notes: String,
}

#[derive(Deserialize, Serialize)]
struct ProcessMemoryReport {
    method: String,
    process_isolated: bool,
    before_setup_peak_rss_bytes: Option<u64>,
    after_setup_peak_rss_bytes: Option<u64>,
    after_samples_peak_rss_bytes: Option<u64>,
    setup_peak_growth_bytes: Option<u64>,
    sample_peak_growth_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScenarioOutput {
    work: BTreeMap<&'static str, u64>,
}

impl ScenarioOutput {
    fn one(name: &'static str, value: usize) -> Self {
        Self::from_pairs([(name, usize_to_u64(value))])
    }

    fn from_pairs<const N: usize>(pairs: [(&'static str, u64); N]) -> Self {
        Self {
            work: pairs.into_iter().collect(),
        }
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

    let scenarios = if cli.scenario.is_none() {
        run_isolated_scenarios(&cli)
    } else {
        run_selected_scenarios(&cli)
    };

    if scenarios.is_empty() {
        eprintln!(
            "unknown scenario `{}`",
            cli.scenario.as_deref().unwrap_or_default()
        );
        std::process::exit(2);
    }

    let report = BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        harness: "novadraw-headless-cpu".to_owned(),
        environment: EnvironmentReport {
            git_revision: command_output("git", &["rev-parse", "HEAD"]),
            git_dirty: git_dirty(),
            rustc_verbose: command_output("rustc", &["-vV"]),
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            cpu_model: cpu_model(),
        },
        sampling: SamplingReport {
            warmup_iterations: cli.warmup_iterations,
            sample_iterations: cli.sample_iterations,
        },
        measurement_scope: MeasurementScope {
            cpu_setup: true,
            cpu_operation: true,
            gpu_submission: false,
            gpu_execution: false,
            present: false,
            input_to_present: false,
            process_memory: peak_rss_bytes().is_some(),
        },
        scenarios,
    };
    write_report(&cli.report, &report);
}

fn run_selected_scenarios(cli: &Cli) -> Vec<ScenarioReport> {
    let mut scenarios = Vec::new();
    push_if_selected(
        &mut scenarios,
        cli,
        "wide_tree_full_record_4096",
        benchmark_large_tree_render,
    );
    push_if_selected(&mut scenarios, cli, "deep_tree_full_record_1000", |cli| {
        benchmark_deep_tree_render(cli, DEEP_TREE_SHALLOW_DEPTH)
    });
    push_if_selected(&mut scenarios, cli, "deep_tree_full_record_10000", |cli| {
        benchmark_deep_tree_render(cli, DEEP_TREE_MAX_DEPTH)
    });
    push_if_selected(&mut scenarios, cli, "deep_tree_validate_1000", |cli| {
        benchmark_deep_tree_validate(cli, DEEP_TREE_SHALLOW_DEPTH)
    });
    push_if_selected(&mut scenarios, cli, "deep_tree_validate_10000", |cli| {
        benchmark_deep_tree_validate(cli, DEEP_TREE_MAX_DEPTH)
    });
    push_if_selected(&mut scenarios, cli, "label_refresh_wide_1000", |cli| {
        benchmark_label_refresh(cli, false)
    });
    push_if_selected(&mut scenarios, cli, "label_refresh_deep_1000", |cli| {
        benchmark_label_refresh(cli, true)
    });
    push_if_selected(
        &mut scenarios,
        cli,
        "text_flow_full_record_wide_512",
        |cli| benchmark_text_flow(cli, false),
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "text_flow_full_record_deep_512",
        |cli| benchmark_text_flow(cli, true),
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "independent_routing_1000",
        benchmark_independent_routing,
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "grouped_routing_256",
        benchmark_grouped_routing,
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "local_update_record_1pct_4096",
        |cli| benchmark_tree_update(cli, LARGE_TREE_FIGURES / 100),
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "full_update_record_100pct_4096",
        |cli| benchmark_tree_update(cli, LARGE_TREE_FIGURES),
    );
    push_if_selected(
        &mut scenarios,
        cli,
        "viewport_full_record_1024",
        benchmark_viewport_render,
    );
    scenarios
}

fn run_isolated_scenarios(cli: &Cli) -> Vec<ScenarioReport> {
    let executable = std::env::current_exe().expect("resolve r8-perf executable");
    let mut scenarios = Vec::with_capacity(SCENARIO_NAMES.len());
    for name in SCENARIO_NAMES {
        let child_report =
            std::env::temp_dir().join(format!("r8-perf-{}-{name}.json", std::process::id()));
        let status = Command::new(&executable)
            .arg(format!("--warmup={}", cli.warmup_iterations))
            .arg(format!("--samples={}", cli.sample_iterations))
            .arg(format!("--scenario={name}"))
            .arg(format!("--report={}", child_report.display()))
            .status()
            .unwrap_or_else(|error| panic!("run isolated scenario {name}: {error}"));
        assert!(
            status.success(),
            "isolated scenario {name} failed: {status}"
        );
        let child_json = std::fs::read(&child_report).unwrap_or_else(|error| {
            panic!("read child report {}: {error}", child_report.display())
        });
        let mut child: BenchmarkReport =
            serde_json::from_slice(&child_json).unwrap_or_else(|error| {
                panic!("parse child report {}: {error}", child_report.display())
            });
        assert_eq!(
            child.scenarios.len(),
            1,
            "isolated scenario {name} must emit exactly one result"
        );
        scenarios.push(child.scenarios.remove(0));
        std::fs::remove_file(&child_report).unwrap_or_else(|error| {
            panic!("remove child report {}: {error}", child_report.display())
        });
    }
    scenarios
}

fn write_report(path: &PathBuf, report: &BenchmarkReport) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!("create report directory {}: {error}", parent.display())
        });
    }
    let json = serde_json::to_string_pretty(&report).expect("serialize benchmark report");
    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("write report {}: {error}", path.display()));
    println!("REPORT {}", path.display());
}

fn push_if_selected(
    reports: &mut Vec<ScenarioReport>,
    cli: &Cli,
    name: &'static str,
    benchmark: impl FnOnce(&Cli) -> ScenarioReport,
) {
    if cli.includes(name) {
        reports.push(benchmark(cli));
    }
}

fn benchmark_large_tree_render(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "wide_tree_full_record_4096",
        cli,
        scenario_config(LARGE_TREE_FIGURES + 1, 2, 1.0, 1.0, None, "none"),
        || Runtime::new(build_large_tree().0),
        |runtime| {
            ScenarioOutput::one(
                "render_commands",
                runtime.record_full_frame().commands().len(),
            )
        },
        "CPU stabilization and full command recording only.",
    )
}

fn benchmark_deep_tree_render(cli: &Cli, depth: usize) -> ScenarioReport {
    benchmark_prepared(
        if depth == DEEP_TREE_MAX_DEPTH {
            "deep_tree_full_record_10000"
        } else {
            "deep_tree_full_record_1000"
        },
        cli,
        scenario_config(depth, depth, 1.0, 1.0, None, "none"),
        || Runtime::new(build_deep_tree_with_leaf(depth).0),
        |runtime| {
            ScenarioOutput::one(
                "render_commands",
                runtime.record_full_frame().commands().len(),
            )
        },
        "CPU stabilization and the supported recursive recording path.",
    )
}

fn benchmark_deep_tree_validate(cli: &Cli, depth: usize) -> ScenarioReport {
    let mut alternate = false;
    benchmark_prepared(
        if depth == DEEP_TREE_MAX_DEPTH {
            "deep_tree_validate_10000"
        } else {
            "deep_tree_validate_1000"
        },
        cli,
        scenario_config(
            depth,
            depth,
            1.0,
            1.0 / depth as f64,
            None,
            "deepest leaf size toggle",
        ),
        || build_deep_tree_with_leaf(depth),
        |(tree, root, leaf)| {
            alternate = !alternate;
            let size = if alternate { 2.0 } else { 3.0 };
            tree.builder()
                .set_preferred_size(*leaf, Some(Dimension::new(size, size)))
                .expect("valid FigureTree construction");
            tree.builder()
                .validate_subtree(*root)
                .expect("validate deep tree");
            ScenarioOutput::one("validated_leaf", usize::from(tree.is_valid(*leaf)))
        },
        "Invalidates the deepest leaf and validates the complete path.",
    )
}

fn benchmark_label_refresh(cli: &Cli, deep: bool) -> ScenarioReport {
    benchmark_prepared(
        if deep {
            "label_refresh_deep_1000"
        } else {
            "label_refresh_wide_1000"
        },
        cli,
        scenario_config(
            TEXT_FIGURES + 1,
            if deep { TEXT_FIGURES + 1 } else { 2 },
            1.0,
            0.0,
            Some("Inter"),
            "cached intrinsic refresh",
        ),
        || build_label_runtime(deep),
        |runtime| {
            let before = runtime.text_layout_stats();
            runtime.refresh_label_layouts().expect("refresh labels");
            let after = runtime.text_layout_stats();
            ScenarioOutput::from_pairs([
                (
                    "label_style_nodes_visited",
                    after
                        .label_style_nodes_visited
                        .saturating_sub(before.label_style_nodes_visited),
                ),
                (
                    "label_figures_refreshed",
                    after
                        .label_figures_refreshed
                        .saturating_sub(before.label_figures_refreshed),
                ),
            ])
        },
        "Uses real LabelFigure shaping caches; work counters expose style-resolution complexity.",
    )
}

fn benchmark_text_flow(cli: &Cli, deep: bool) -> ScenarioReport {
    benchmark_prepared(
        if deep {
            "text_flow_full_record_deep_512"
        } else {
            "text_flow_full_record_wide_512"
        },
        cli,
        scenario_config(
            TEXT_FLOW_FIGURES + 1,
            if deep { TEXT_FLOW_FIGURES + 1 } else { 2 },
            1.0,
            1.0,
            Some("Inter"),
            "none",
        ),
        || build_text_flow_runtime(deep),
        |runtime| {
            let before = runtime.text_layout_stats();
            let commands = runtime.record_full_frame().commands().len();
            let after = runtime.text_layout_stats();
            ScenarioOutput::from_pairs([
                ("render_commands", usize_to_u64(commands)),
                (
                    "text_flow_style_nodes_visited",
                    after
                        .text_flow_style_nodes_visited
                        .saturating_sub(before.text_flow_style_nodes_visited),
                ),
                (
                    "text_flow_figures_refreshed",
                    after
                        .text_flow_figures_refreshed
                        .saturating_sub(before.text_flow_figures_refreshed),
                ),
            ])
        },
        "Uses real TextFlowFigure layout and full CPU command recording.",
    )
}

fn benchmark_independent_routing(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "independent_routing_1000",
        cli,
        scenario_config(
            INDEPENDENT_CONNECTIONS + 3,
            2,
            1.0,
            1.0,
            None,
            "resolve every DirectRouter connection in stable order",
        ),
        || build_connection_runtime(INDEPENDENT_CONNECTIONS, false),
        |workload| {
            let before = workload.runtime.connection_routing_stats();
            for connection in &workload.connections {
                workload
                    .runtime
                    .resolve_connection_route(*connection, workload.space)
                    .expect("resolve independent connection");
            }
            let after = workload.runtime.connection_routing_stats();
            ScenarioOutput::from_pairs([
                (
                    "route_calculations",
                    after
                        .route_calculations
                        .saturating_sub(before.route_calculations),
                ),
                (
                    "routing_order_entries",
                    after
                        .routing_order_entries
                        .saturating_sub(before.routing_order_entries),
                ),
            ])
        },
        "DirectRouter routes are independent; stable-order work must grow linearly.",
    )
}

fn benchmark_grouped_routing(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "grouped_routing_256",
        cli,
        scenario_config(
            GROUPED_CONNECTIONS + 3,
            2,
            1.0,
            1.0,
            None,
            "resolve one FanRouter anchor-pair batch",
        ),
        || build_connection_runtime(GROUPED_CONNECTIONS, true),
        |workload| {
            let before = workload.runtime.connection_routing_stats();
            workload
                .runtime
                .resolve_connection_route(workload.connections[0], workload.space)
                .expect("resolve grouped connections");
            let after = workload.runtime.connection_routing_stats();
            ScenarioOutput::from_pairs([
                (
                    "route_calculations",
                    after
                        .route_calculations
                        .saturating_sub(before.route_calculations),
                ),
                (
                    "routing_order_entries",
                    after
                        .routing_order_entries
                        .saturating_sub(before.routing_order_entries),
                ),
            ])
        },
        "FanRouter preserves the stable anchor-pair group contract.",
    )
}

fn benchmark_tree_update(cli: &Cli, changed: usize) -> ScenarioReport {
    benchmark_prepared(
        if changed == LARGE_TREE_FIGURES {
            "full_update_record_100pct_4096"
        } else {
            "local_update_record_1pct_4096"
        },
        cli,
        scenario_config(
            LARGE_TREE_FIGURES + 1,
            2,
            1.0,
            changed as f64 / LARGE_TREE_FIGURES as f64,
            None,
            "alternate selected Figure x coordinate by one logical pixel",
        ),
        || {
            let (tree, figures) = build_large_tree();
            let mut runtime = Runtime::new(tree);
            runtime.prepare_frame();
            UpdateWorkload {
                runtime,
                figures,
                changed,
                alternate: false,
            }
        },
        |workload| {
            workload.alternate = !workload.alternate;
            let offset = if workload.alternate { 1.0 } else { 0.0 };
            for (index, figure) in workload
                .figures
                .iter()
                .copied()
                .take(workload.changed)
                .enumerate()
            {
                let column = index % LARGE_TREE_COLUMNS;
                let row = index / LARGE_TREE_COLUMNS;
                workload
                    .runtime
                    .figure(figure)
                    .expect("attached benchmark Figure")
                    .set_bounds(Rectangle::new(
                        column as f64 * 8.0 + offset,
                        row as f64 * 8.0,
                        7.0,
                        7.0,
                    ))
                    .expect("valid bounds update");
            }
            let commands = workload
                .runtime
                .prepare_frame()
                .expect("mutation queues an incremental frame")
                .commands()
                .len();
            ScenarioOutput::from_pairs([
                ("mutated_figures", usize_to_u64(workload.changed)),
                ("render_commands", usize_to_u64(commands)),
            ])
        },
        "Measures Runtime mutation, validation, damage traversal and CPU command recording.",
    )
}

fn benchmark_viewport_render(cli: &Cli) -> ScenarioReport {
    benchmark_prepared(
        "viewport_full_record_1024",
        cli,
        scenario_config(
            VIEWPORT_FIGURES + 3,
            4,
            0.25,
            1.0,
            None,
            "fixed scroll and 1.5x zoom",
        ),
        || Runtime::new(build_viewport_tree()),
        |runtime| {
            ScenarioOutput::one(
                "render_commands",
                runtime.record_full_frame().commands().len(),
            )
        },
        "CPU recording includes viewport clip, translation and scale; no GPU or present timing.",
    )
}

fn benchmark_prepared<T>(
    name: &'static str,
    cli: &Cli,
    config: ScenarioConfig,
    prepare: impl FnOnce() -> T,
    mut operation: impl FnMut(&mut T) -> ScenarioOutput,
    notes: &'static str,
) -> ScenarioReport {
    let before_setup_peak_rss_bytes = peak_rss_bytes();
    let setup_start = Instant::now();
    let mut value = prepare();
    let setup_ns = duration_ns(setup_start.elapsed());
    let after_setup_peak_rss_bytes = peak_rss_bytes();

    for _ in 0..cli.warmup_iterations {
        black_box(operation(black_box(&mut value)));
    }

    let mut samples = Vec::with_capacity(cli.sample_iterations);
    let mut output = None;
    for _ in 0..cli.sample_iterations {
        let start = Instant::now();
        let current = black_box(operation(black_box(&mut value)));
        samples.push(duration_ns(start.elapsed()));
        if let Some(previous) = &output {
            assert_eq!(
                previous, &current,
                "scenario work counters must remain stable across samples"
            );
        }
        output = Some(current);
    }
    let after_samples_peak_rss_bytes = peak_rss_bytes();
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let report = ScenarioReport {
        name: name.to_owned(),
        config,
        setup_ns,
        min_ns: sorted[0],
        p50_ns: percentile(&sorted, 50),
        p95_ns: percentile(&sorted, 95),
        samples_ns: samples,
        memory: ProcessMemoryReport {
            method: peak_rss_method().to_owned(),
            process_isolated: cli.scenario.is_some(),
            before_setup_peak_rss_bytes,
            after_setup_peak_rss_bytes,
            after_samples_peak_rss_bytes,
            setup_peak_growth_bytes: memory_growth(
                before_setup_peak_rss_bytes,
                after_setup_peak_rss_bytes,
            ),
            sample_peak_growth_bytes: memory_growth(
                after_setup_peak_rss_bytes,
                after_samples_peak_rss_bytes,
            ),
        },
        work: output
            .expect("at least one sample")
            .work
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
        notes: notes.to_owned(),
    };
    println!(
        "BENCH {} setup={}ns min={}ns p50={}ns p95={}ns work={:?}",
        report.name, report.setup_ns, report.min_ns, report.p50_ns, report.p95_ns, report.work
    );
    report
}

fn scenario_config(
    figure_count: usize,
    maximum_depth: usize,
    visible_ratio: f64,
    update_ratio: f64,
    font: Option<&'static str>,
    input_trajectory: &'static str,
) -> ScenarioConfig {
    ScenarioConfig {
        figure_count,
        maximum_depth,
        viewport_logical: [ROOT_WIDTH, ROOT_HEIGHT],
        logical_dpi: LOGICAL_DPI,
        visible_ratio,
        update_ratio,
        font: font.map(str::to_owned),
        input_trajectory: input_trajectory.to_owned(),
    }
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let rank = samples.len().saturating_mul(percentile).div_ceil(100);
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn memory_growth(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    before
        .zip(after)
        .map(|(before, after)| after.saturating_sub(before))
}

#[cfg(target_os = "macos")]
fn peak_rss_method() -> &'static str {
    "getrusage-ru_maxrss-bytes"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peak_rss_method() -> &'static str {
    "getrusage-ru_maxrss-kib"
}

#[cfg(windows)]
fn peak_rss_method() -> &'static str {
    "GetProcessMemoryInfo-PeakWorkingSetSize"
}

#[cfg(not(any(unix, windows)))]
fn peak_rss_method() -> &'static str {
    "unsupported"
}

#[cfg(unix)]
fn peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the supplied rusage when it returns zero.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    // SAFETY: a successful getrusage call initialized usage.
    let usage = unsafe { usage.assume_init() };
    let peak = u64::try_from(usage.ru_maxrss).ok()?;
    if cfg!(target_os = "macos") {
        Some(peak)
    } else {
        peak.checked_mul(1_024)
    }
}

#[cfg(windows)]
fn peak_rss_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    // SAFETY: counters points to writable storage of the size passed to the Windows API.
    let status = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), counters.as_mut_ptr(), size) };
    if status == 0 {
        return None;
    }
    // SAFETY: a successful GetProcessMemoryInfo call initialized counters.
    let counters = unsafe { counters.assume_init() };
    u64::try_from(counters.PeakWorkingSetSize).ok()
}

#[cfg(not(any(unix, windows)))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git_dirty() -> Option<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()?;
    output.status.success().then_some(!output.stdout.is_empty())
}

fn cpu_model() -> Option<String> {
    if cfg!(target_os = "macos") {
        command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|source| {
                source.lines().find_map(|line| {
                    line.strip_prefix("model name")
                        .and_then(|line| line.split_once(':'))
                        .map(|(_, value)| value.trim().to_owned())
                })
            })
    } else {
        std::env::var("PROCESSOR_IDENTIFIER").ok()
    }
}

fn root_tree() -> (FigureTree, FigureId) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            ROOT_WIDTH,
            ROOT_HEIGHT,
            Color::WHITE,
        )))
        .expect("valid FigureTree construction");
    (tree, root)
}

fn build_large_tree() -> (FigureTree, Vec<FigureId>) {
    let (mut tree, root) = root_tree();
    let mut figures = Vec::with_capacity(LARGE_TREE_FIGURES);
    for index in 0..LARGE_TREE_FIGURES {
        let column = index % LARGE_TREE_COLUMNS;
        let row = index / LARGE_TREE_COLUMNS;
        figures.push(
            tree.builder()
                .add_child(
                    root,
                    Box::new(RectangleFigure::new(
                        column as f64 * 8.0,
                        row as f64 * 8.0,
                        7.0,
                        7.0,
                    )),
                )
                .expect("valid FigureTree construction"),
        );
    }
    (tree, figures)
}

fn build_deep_tree_with_leaf(depth: usize) -> (FigureTree, FigureId, FigureId) {
    let (mut tree, mut parent) = root_tree();
    let root = parent;
    for _ in 1..depth {
        parent = tree
            .builder()
            .add_child(parent, Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)))
            .expect("valid FigureTree construction");
    }
    (tree, root, parent)
}

fn build_label_runtime(deep: bool) -> Runtime {
    let (mut tree, root) = root_tree();
    tree.builder()
        .set_figure_style(
            root,
            FigureStyle {
                font: Some("12px Inter".to_owned()),
                ..FigureStyle::default()
            },
        )
        .expect("valid root style");
    let mut parent = root;
    for index in 0..TEXT_FIGURES {
        let row = index / 20;
        let column = index % 20;
        let label = tree
            .builder()
            .add_child(
                parent,
                Box::new(LabelFigure::new(format!("label-{index:04}")).with_bounds(
                    Rectangle::new(column as f64 * 48.0, row as f64 * 15.0, 46.0, 14.0),
                )),
            )
            .expect("valid Label tree");
        if deep {
            parent = label;
        }
    }
    let mut runtime = Runtime::new(tree);
    runtime
        .register_builtin_font(BuiltinFont::Inter)
        .expect("register benchmark font");
    runtime
        .refresh_label_layouts()
        .expect("prime Label layouts");
    runtime
}

fn build_text_flow_runtime(deep: bool) -> Runtime {
    let (mut tree, root) = root_tree();
    tree.builder()
        .set_figure_style(
            root,
            FigureStyle {
                font: Some("12px Inter".to_owned()),
                ..FigureStyle::default()
            },
        )
        .expect("valid root style");
    let mut parent = root;
    for index in 0..TEXT_FLOW_FIGURES {
        let flow = tree
            .builder()
            .add_child(
                parent,
                Box::new(TextFlowFigure::new(
                    Rectangle::new(0.0, index as f64 * 18.0, 160.0, 18.0),
                    FlowPage::from_text(format!("flow paragraph {index:04}")),
                )),
            )
            .expect("valid TextFlow tree");
        if deep {
            parent = flow;
        }
    }
    let mut runtime = Runtime::new(tree);
    runtime
        .register_builtin_font(BuiltinFont::Inter)
        .expect("register benchmark font");
    runtime.record_full_frame();
    runtime
}

struct ConnectionWorkload {
    runtime: Runtime,
    connections: Vec<ConnectionId>,
    space: CoordinateSpace,
}

fn build_connection_runtime(count: usize, grouped: bool) -> ConnectionWorkload {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(
            0.0,
            0.0,
            ROOT_WIDTH,
            ROOT_HEIGHT,
        )))
        .expect("benchmark root");
    let source = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(RectangleFigure::new(20.0, 40.0, 80.0, 40.0)))
        .expect("source Figure");
    let target = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(RectangleFigure::new(900.0, 600.0, 80.0, 40.0)))
        .expect("target Figure");
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let router = if grouped {
        runtime.register_connection_router(Box::new(
            FanRouter::new(Box::new(DirectRouter), 16.0).expect("valid fan separation"),
        ))
    } else {
        runtime.direct_connection_router()
    };
    let mut connections = Vec::with_capacity(count);
    for _ in 0..count {
        let figure = runtime
            .container(root)
            .expect("root container")
            .add(Box::new(ConnectionFigure::new()))
            .expect("connection Figure");
        connections.push(
            runtime
                .register_connection_state(
                    figure,
                    Some(source_anchor),
                    Some(target_anchor),
                    RouterBinding::Explicit { router },
                    None,
                )
                .expect("connection state"),
        );
    }
    ConnectionWorkload {
        runtime,
        connections,
        space: CoordinateSpace::ChildContent(root),
    }
}

struct UpdateWorkload {
    runtime: Runtime,
    figures: Vec<FigureId>,
    changed: usize,
    alternate: bool,
}

fn build_viewport_tree() -> FigureTree {
    let (mut tree, root) = root_tree();
    let viewport = tree
        .builder()
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 800.0, 560.0))
        .expect("attach viewport");
    let scalable = tree
        .builder()
        .add_scalable_layered_pane_to(
            viewport.figure_id(),
            Rectangle::new(0.0, 0.0, 2_048.0, 2_048.0),
        )
        .expect("attach scalable pane");
    for index in 0..VIEWPORT_FIGURES {
        let column = index % VIEWPORT_COLUMNS;
        let row = index / VIEWPORT_COLUMNS;
        tree.builder()
            .add_child(
                scalable.figure_id(),
                Box::new(RectangleFigure::new(
                    column as f64 * 56.0,
                    row as f64 * 48.0,
                    48.0,
                    40.0,
                )),
            )
            .expect("valid FigureTree construction");
    }
    let zoom = ZoomManager::new(scalable, viewport.clone());
    tree.builder()
        .set_zoom(&zoom, 1.5)
        .expect("set viewport zoom");
    tree.builder()
        .set_view_location(viewport.figure_id(), 160.0, 120.0)
        .expect("set viewport location");
    tree
}
