use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use novadraw::{
    Color, FigureTree, RectangleFigure, RenderBackend, RenderOutcome, Runtime, SurfaceInfo,
};
use novadraw_backend_vello::{VelloAdapterInfo, VelloRenderer};
use serde::Serialize;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

const REPORT_SCHEMA_VERSION: u32 = 2;
const DEFAULT_WARMUP_ITERATIONS: usize = 5;
const DEFAULT_SAMPLE_ITERATIONS: usize = 30;
const DEFAULT_REPORT: &str = "target/performance/ga2-native-vello.json";
const FIGURE_COUNT: usize = 4_096;
const COLUMNS: usize = 64;
const LOGICAL_WIDTH: f64 = 1_024.0;
const LOGICAL_HEIGHT: f64 = 768.0;
const GPU_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const SURFACE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const REDRAW_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug)]
struct Cli {
    warmup_iterations: usize,
    sample_iterations: usize,
    report: PathBuf,
    workspace: Option<PathBuf>,
    require_surface_present: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            warmup_iterations: DEFAULT_WARMUP_ITERATIONS,
            sample_iterations: DEFAULT_SAMPLE_ITERATIONS,
            report: PathBuf::from(DEFAULT_REPORT),
            workspace: None,
            require_surface_present: false,
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
            } else if let Some(value) = argument.strip_prefix("--workspace=") {
                cli.workspace = Some(PathBuf::from(value));
            } else if argument == "--require-surface-present" {
                cli.require_surface_present = true;
            } else if argument == "--help" {
                println!(
                    "native-vello-perf [--warmup=<count>] [--samples=<count>] \
                     [--report=<path>] [--workspace=<path>] [--require-surface-present]"
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

#[derive(Clone, Copy)]
struct FrameSample {
    prepare_submission_ns: u64,
    backend_submit_cpu_ns: u64,
    gpu_completion_wait_ns: u64,
    submit_to_gpu_complete_ns: u64,
    frame_to_gpu_complete_ns: u64,
    command_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderMode {
    SurfaceProbe,
    Surface,
    Offscreen,
}

struct NativeVelloBenchmark {
    cli: Cli,
    window: Option<Arc<Window>>,
    renderer: Option<VelloRenderer>,
    runtime: Option<Runtime>,
    adapter: Option<VelloAdapterInfo>,
    surface: Option<SurfaceInfo>,
    warmups_completed: usize,
    samples: Vec<FrameSample>,
    before_setup_peak_rss_bytes: Option<u64>,
    surface_probe_attempts: usize,
    render_mode: RenderMode,
    surface_probe_outcome: Option<&'static str>,
    surface_probe_deadline: Option<Instant>,
    next_redraw_at: Instant,
    focused: bool,
    occluded: Option<bool>,
    finished: bool,
}

impl NativeVelloBenchmark {
    fn new(cli: Cli) -> Self {
        let sample_capacity = cli.sample_iterations;
        Self {
            cli,
            window: None,
            renderer: None,
            runtime: None,
            adapter: None,
            surface: None,
            warmups_completed: 0,
            samples: Vec::with_capacity(sample_capacity),
            before_setup_peak_rss_bytes: peak_rss_bytes(),
            surface_probe_attempts: 0,
            render_mode: RenderMode::SurfaceProbe,
            surface_probe_outcome: None,
            surface_probe_deadline: None,
            next_redraw_at: Instant::now(),
            focused: false,
            occluded: None,
            finished: false,
        }
    }

    fn render_sample(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(renderer), Some(runtime)) = (
            self.window.as_ref(),
            self.renderer.as_mut(),
            self.runtime.as_mut(),
        ) else {
            return;
        };
        let surface = surface_info(window);
        runtime.request_full_redraw();
        let frame_start = Instant::now();
        let prepare_start = Instant::now();
        let Some(submission) = runtime.prepare_submission(surface, renderer.capabilities()) else {
            window.request_redraw();
            return;
        };
        let prepare_submission_ns = duration_ns(prepare_start.elapsed());
        let command_count = submission.commands.len();
        let session_id = submission.session_id;
        let frame_id = submission.frame_id;
        if self.render_mode == RenderMode::SurfaceProbe {
            self.surface_probe_attempts += 1;
        }

        let submit_start = Instant::now();
        let outcome = match self.render_mode {
            RenderMode::SurfaceProbe | RenderMode::Surface => renderer.submit(&submission),
            RenderMode::Offscreen => renderer.render_for_screenshot(&submission),
        };
        let backend_submit_cpu_ns = duration_ns(submit_start.elapsed());

        if self.render_mode == RenderMode::SurfaceProbe {
            match outcome {
                RenderOutcome::Presented => {
                    self.render_mode = RenderMode::Surface;
                    self.surface_probe_outcome = Some("presented");
                }
                RenderOutcome::Skipped => {
                    runtime.complete_submission(session_id, frame_id, outcome);
                    let deadline = self
                        .surface_probe_deadline
                        .expect("surface probe deadline initialized");
                    if Instant::now() >= deadline {
                        assert!(
                            !self.cli.require_surface_present,
                            "surface present was required but remained unavailable after \
                             {SURFACE_PROBE_TIMEOUT:?}; focused={}, occluded={:?}",
                            self.focused, self.occluded
                        );
                        self.render_mode = RenderMode::Offscreen;
                        self.surface_probe_outcome = Some("skipped_after_timeout");
                    }
                    return;
                }
                RenderOutcome::Retry => {
                    runtime.complete_submission(session_id, frame_id, outcome);
                    let deadline = self
                        .surface_probe_deadline
                        .expect("surface probe deadline initialized");
                    assert!(
                        Instant::now() < deadline,
                        "surface probe kept returning Retry for {SURFACE_PROBE_TIMEOUT:?}"
                    );
                    return;
                }
                RenderOutcome::Unsupported(capability) => {
                    panic!("native Vello surface probe lacks capability {capability:?}");
                }
            }
        }

        if outcome != RenderOutcome::Presented {
            runtime.complete_submission(session_id, frame_id, outcome);
            panic!(
                "native Vello {:?} submission returned {outcome:?}",
                self.render_mode
            );
        }

        let wait_start = Instant::now();
        renderer
            .wait_for_gpu_idle(GPU_WAIT_TIMEOUT)
            .unwrap_or_else(|error| panic!("wait for GPU completion: {error}"));
        let gpu_completion_wait_ns = duration_ns(wait_start.elapsed());
        let submit_to_gpu_complete_ns = duration_ns(submit_start.elapsed());
        let frame_to_gpu_complete_ns = duration_ns(frame_start.elapsed());
        runtime.complete_submission(session_id, frame_id, outcome);

        let sample = FrameSample {
            prepare_submission_ns,
            backend_submit_cpu_ns,
            gpu_completion_wait_ns,
            submit_to_gpu_complete_ns,
            frame_to_gpu_complete_ns,
            command_count,
        };
        if self.warmups_completed < self.cli.warmup_iterations {
            self.warmups_completed += 1;
        } else {
            if let Some(previous) = self.samples.last() {
                assert_eq!(
                    previous.command_count, sample.command_count,
                    "render command count changed between samples"
                );
            }
            self.samples.push(sample);
        }

        if self.samples.len() == self.cli.sample_iterations {
            self.finish(event_loop);
        } else {
            window.request_redraw();
        }
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        self.finished = true;
        let report = BenchmarkReport {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            harness: "novadraw-native-vello-gpu",
            environment: EnvironmentReport {
                git_revision: command_output("git", &["rev-parse", "HEAD"]),
                git_dirty: git_dirty(),
                rustc_verbose: command_output("rustc", &["-vV"]),
                operating_system: std::env::consts::OS,
                architecture: std::env::consts::ARCH,
                cpu_model: cpu_model(),
                adapter: self
                    .adapter
                    .clone()
                    .expect("initialized GPU adapter")
                    .into(),
            },
            sampling: SamplingReport {
                warmup_iterations: self.cli.warmup_iterations,
                sample_iterations: self.cli.sample_iterations,
            },
            measurement_scope: MeasurementScope {
                cpu_prepare_submission: true,
                backend_submit_cpu: true,
                gpu_timestamp: false,
                gpu_queue_completion_wait: true,
                surface_present_probe: true,
                offscreen_vello_submit: self.render_mode == RenderMode::Offscreen,
                surface_present_call: self.render_mode == RenderMode::Surface,
                surface_present_required: self.cli.require_surface_present,
                compositor_present: false,
                input_to_present: false,
            },
            scenario: ScenarioReport {
                name: match self.render_mode {
                    RenderMode::Surface => "wide_tree_full_surface_4096",
                    RenderMode::Offscreen => "wide_tree_full_offscreen_gpu_4096",
                    RenderMode::SurfaceProbe => unreachable!("probe must resolve before samples"),
                }
                .to_owned(),
                figure_count: FIGURE_COUNT + 1,
                logical_viewport: [LOGICAL_WIDTH, LOGICAL_HEIGHT],
                surface: self.surface.expect("initialized surface").into(),
                present_mode: "AutoVsync",
                render_mode: format!("{:?}", self.render_mode),
                surface_present_probe_outcome: self
                    .surface_probe_outcome
                    .expect("surface probe outcome"),
                surface_present_probe_attempts: self.surface_probe_attempts,
                focused_at_finish: self.focused,
                occluded_at_finish: self.occluded,
                command_count: self.samples[0].command_count,
                prepare_submission: StageReport::from_samples(
                    self.samples
                        .iter()
                        .map(|sample| sample.prepare_submission_ns)
                        .collect(),
                ),
                backend_submit_cpu: StageReport::from_samples(
                    self.samples
                        .iter()
                        .map(|sample| sample.backend_submit_cpu_ns)
                        .collect(),
                ),
                gpu_completion_wait: StageReport::from_samples(
                    self.samples
                        .iter()
                        .map(|sample| sample.gpu_completion_wait_ns)
                        .collect(),
                ),
                submit_to_gpu_complete: StageReport::from_samples(
                    self.samples
                        .iter()
                        .map(|sample| sample.submit_to_gpu_complete_ns)
                        .collect(),
                ),
                frame_to_gpu_complete: StageReport::from_samples(
                    self.samples
                        .iter()
                        .map(|sample| sample.frame_to_gpu_complete_ns)
                        .collect(),
                ),
                memory: ProcessMemoryReport {
                    method: peak_rss_method(),
                    before_setup_peak_rss_bytes: self.before_setup_peak_rss_bytes,
                    after_samples_peak_rss_bytes: peak_rss_bytes(),
                },
                notes: match self.render_mode {
                    RenderMode::Surface => {
                        "Surface submit includes Vello lowering, command encoding, internal queue \
                         submissions, retained-texture blit submission, and \
                         SurfaceTexture::present(). Queue completion is not a compositor \
                         presentation signal."
                    }
                    RenderMode::Offscreen => {
                        "The surface present probe was skipped, so samples use \
                         render_for_screenshot plus queue completion. No surface present call or \
                         compositor presentation is included."
                    }
                    RenderMode::SurfaceProbe => unreachable!("probe must resolve before samples"),
                },
            },
        };
        write_report(&self.cli.report, &report);
        println!("REPORT {}", self.cli.report.display());
        event_loop.exit();
    }
}

impl ApplicationHandler<()> for NativeVelloBenchmark {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Novadraw GA-2 Native Vello Performance")
                        .with_inner_size(LogicalSize::new(LOGICAL_WIDTH, LOGICAL_HEIGHT))
                        .with_resizable(false)
                        .with_window_level(WindowLevel::AlwaysOnTop),
                )
                .expect("create performance window"),
        );
        window.set_visible(true);
        window.focus_window();
        let surface = surface_info(&window);
        let renderer = VelloRenderer::new(Arc::clone(&window), surface);
        self.adapter = Some(renderer.adapter_info());
        self.surface = Some(surface);
        self.runtime = Some(build_runtime());
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.surface_probe_deadline = Some(Instant::now() + SURFACE_PROBE_TIMEOUT);
        self.window
            .as_ref()
            .expect("initialized window")
            .request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(focused) => self.focused = focused,
            WindowEvent::Occluded(occluded) => self.occluded = Some(occluded),
            WindowEvent::RedrawRequested => self.render_sample(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        if now >= self.next_redraw_at {
            self.next_redraw_at = now + REDRAW_INTERVAL;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw_at));
    }
}

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    generated_at_unix_seconds: u64,
    harness: &'static str,
    environment: EnvironmentReport,
    sampling: SamplingReport,
    measurement_scope: MeasurementScope,
    scenario: ScenarioReport,
}

#[derive(Serialize)]
struct EnvironmentReport {
    git_revision: Option<String>,
    git_dirty: Option<bool>,
    rustc_verbose: Option<String>,
    operating_system: &'static str,
    architecture: &'static str,
    cpu_model: Option<String>,
    adapter: VelloAdapterInfoReport,
}

impl From<VelloAdapterInfo> for VelloAdapterInfoReport {
    fn from(info: VelloAdapterInfo) -> Self {
        Self {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            device_type: info.device_type,
            backend: info.backend,
            driver: info.driver,
            driver_info: info.driver_info,
        }
    }
}

#[derive(Serialize)]
struct VelloAdapterInfoReport {
    name: String,
    vendor: u32,
    device: u32,
    device_type: String,
    backend: String,
    driver: String,
    driver_info: String,
}

#[derive(Serialize)]
struct SamplingReport {
    warmup_iterations: usize,
    sample_iterations: usize,
}

#[derive(Serialize)]
struct MeasurementScope {
    cpu_prepare_submission: bool,
    backend_submit_cpu: bool,
    gpu_timestamp: bool,
    gpu_queue_completion_wait: bool,
    surface_present_probe: bool,
    offscreen_vello_submit: bool,
    surface_present_call: bool,
    surface_present_required: bool,
    compositor_present: bool,
    input_to_present: bool,
}

#[derive(Serialize)]
struct ScenarioReport {
    name: String,
    figure_count: usize,
    logical_viewport: [f64; 2],
    surface: SurfaceReport,
    present_mode: &'static str,
    render_mode: String,
    surface_present_probe_outcome: &'static str,
    surface_present_probe_attempts: usize,
    focused_at_finish: bool,
    occluded_at_finish: Option<bool>,
    command_count: usize,
    prepare_submission: StageReport,
    backend_submit_cpu: StageReport,
    gpu_completion_wait: StageReport,
    submit_to_gpu_complete: StageReport,
    frame_to_gpu_complete: StageReport,
    memory: ProcessMemoryReport,
    notes: &'static str,
}

#[derive(Clone, Copy, Serialize)]
struct SurfaceReport {
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f64,
}

#[derive(Serialize)]
struct StageReport {
    min_ns: u64,
    p50_ns: u64,
    p95_ns: u64,
    samples_ns: Vec<u64>,
}

impl StageReport {
    fn from_samples(samples: Vec<u64>) -> Self {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        Self {
            min_ns: sorted[0],
            p50_ns: percentile(&sorted, 50),
            p95_ns: percentile(&sorted, 95),
            samples_ns: samples,
        }
    }
}

#[derive(Serialize)]
struct ProcessMemoryReport {
    method: &'static str,
    before_setup_peak_rss_bytes: Option<u64>,
    after_samples_peak_rss_bytes: Option<u64>,
}

fn surface_info(window: &Window) -> SurfaceInfo {
    let scale_factor = window.scale_factor();
    let size = window.inner_size();
    SurfaceInfo {
        logical_width: f64::from(size.width) / scale_factor,
        logical_height: f64::from(size.height) / scale_factor,
        pixel_width: size.width,
        pixel_height: size.height,
        scale_factor,
    }
}

impl From<SurfaceInfo> for SurfaceReport {
    fn from(surface: SurfaceInfo) -> Self {
        Self {
            pixel_width: surface.pixel_width,
            pixel_height: surface.pixel_height,
            scale_factor: surface.scale_factor,
        }
    }
}

fn build_runtime() -> Runtime {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            LOGICAL_WIDTH,
            LOGICAL_HEIGHT,
            Color::WHITE,
        )));
    for index in 0..FIGURE_COUNT {
        let column = index % COLUMNS;
        let row = index / COLUMNS;
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
            .expect("performance Figure");
    }
    Runtime::new(tree)
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let rank = samples.len().saturating_mul(percentile).div_ceil(100);
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn write_report(path: &PathBuf, report: &BenchmarkReport) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!("create report directory {}: {error}", parent.display())
        });
    }
    let json = serde_json::to_string_pretty(report).expect("serialize native Vello report");
    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("write report {}: {error}", path.display()));
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

#[cfg(target_os = "macos")]
fn peak_rss_method() -> &'static str {
    "getrusage-ru_maxrss-bytes"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peak_rss_method() -> &'static str {
    "getrusage-ru_maxrss-kib"
}

#[cfg(not(unix))]
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

#[cfg(not(unix))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

fn main() {
    let cli = Cli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if let Some(workspace) = &cli.workspace {
        std::env::set_current_dir(workspace).unwrap_or_else(|error| {
            panic!(
                "set native Vello performance workspace {}: {error}",
                workspace.display()
            )
        });
    }
    if cfg!(debug_assertions) {
        eprintln!("native-vello-perf must run with --release");
        std::process::exit(2);
    }
    let mut event_loop_builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        event_loop_builder
            .with_activation_policy(ActivationPolicy::Regular)
            .with_activate_ignoring_other_apps(true);
    }
    let event_loop = event_loop_builder
        .build()
        .expect("create native performance event loop");
    let mut benchmark = NativeVelloBenchmark::new(cli);
    event_loop
        .run_app(&mut benchmark)
        .expect("run native Vello performance benchmark");
}
