use std::convert::Infallible;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use std::thread;

use novadraw::runtime::PreparedFigureUpdate;
use novadraw::{
    Color, FigureComponentContext, FigureComponentUpdate, FigureId, FigureTree, FramePreparation,
    RectangleFigure, RenderBackend, RenderOutcome, Runtime, SurfaceInfo,
};
use novadraw_backend_vello::{VelloAdapterInfo, VelloRenderer};
use serde::Serialize;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

#[cfg(target_os = "macos")]
mod macos_probe;

const REPORT_SCHEMA_VERSION: u32 = 3;
const DEFAULT_WARMUP_ITERATIONS: usize = 5;
const DEFAULT_SAMPLE_ITERATIONS: usize = 30;
const DEFAULT_REPORT: &str = "target/performance/ga2-native-vello.json";
const FIGURE_COUNT: usize = 4_096;
const COLUMNS: usize = 64;
const LOGICAL_WIDTH: f64 = 1_024.0;
const LOGICAL_HEIGHT: f64 = 768.0;
const GPU_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const SURFACE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const INPUT_EVENT_TIMEOUT: Duration = Duration::from_secs(2);
const WINDOW_SERVER_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(2);
const WINDOW_SERVER_POLL_INTERVAL: Duration = Duration::from_micros(250);
const INPUT_SAMPLE_INTERVAL: Duration = Duration::from_millis(50);
const REDRAW_INTERVAL: Duration = Duration::from_millis(16);
const MARKER_SIZE: f64 = 128.0;

#[derive(Debug)]
struct Cli {
    warmup_iterations: usize,
    sample_iterations: usize,
    report: PathBuf,
    workspace: Option<PathBuf>,
    require_surface_present: bool,
    require_window_server_present: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            warmup_iterations: DEFAULT_WARMUP_ITERATIONS,
            sample_iterations: DEFAULT_SAMPLE_ITERATIONS,
            report: PathBuf::from(DEFAULT_REPORT),
            workspace: None,
            require_surface_present: false,
            require_window_server_present: false,
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
            } else if argument == "--require-window-server-present" {
                cli.require_surface_present = true;
                cli.require_window_server_present = true;
            } else if argument == "--help" {
                println!(
                    "native-vello-perf [--warmup=<count>] [--samples=<count>] \
                     [--report=<path>] [--workspace=<path>] [--require-surface-present] \
                     [--require-window-server-present]"
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

#[derive(Clone, Copy, Serialize)]
struct InputPresentationSample {
    input_post_to_event_ns: u64,
    event_to_window_server_visible_ns: u64,
    input_post_to_window_server_visible_ns: u64,
    submit_return_to_window_server_visible_ns: u64,
    capture_attempts: usize,
    baseline_signature: [u8; 4],
    visible_signature: [u8; 4],
}

#[cfg(target_os = "macos")]
enum InputProbePhase {
    Idle,
    AwaitingInput {
        posted_at: Instant,
        deadline: Instant,
        baseline: macos_probe::CapturedPixel,
    },
    AwaitingRender {
        posted_at: Instant,
        event_received_at: Instant,
        deadline: Instant,
        baseline: macos_probe::CapturedPixel,
    },
}

#[cfg(target_os = "macos")]
struct InputProbeState {
    window_server: macos_probe::WindowServerProbe,
    phase: InputProbePhase,
    warmups_completed: usize,
    samples: Vec<InputPresentationSample>,
    next_input_at: Instant,
    marker_is_green: bool,
    image_width: usize,
    image_height: usize,
    bits_per_pixel: usize,
}

struct SetRectangleFill(Color);

impl FigureComponentUpdate for SetRectangleFill {
    type Figure = RectangleFigure;
    type Prepared = Color;
    type Error = Infallible;

    fn prepare(
        self,
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        Ok(PreparedFigureUpdate::paint(self.0))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.fill_color = prepared;
    }
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
    marker: Option<FigureId>,
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
    #[cfg(target_os = "macos")]
    input_probe: Option<InputProbeState>,
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
            marker: None,
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
            #[cfg(target_os = "macos")]
            input_probe: None,
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
        let submission = match runtime.prepare_submission(surface, renderer.capabilities()) {
            FramePreparation::Ready(submission) => submission,
            FramePreparation::Error(error) => panic!("benchmark frame preparation failed: {error}"),
            FramePreparation::Idle
            | FramePreparation::Suspended
            | FramePreparation::AwaitingCompletion => {
                window.request_redraw();
                return;
            }
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
                RenderOutcome::InvalidGraphicsInput(error) => {
                    panic!("native Vello surface probe received invalid graphics: {error}");
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
            if !self.cli.require_window_server_present {
                self.finish(event_loop);
            }
        } else {
            window.request_redraw();
        }
    }

    #[cfg(target_os = "macos")]
    fn post_input_probe(&mut self) {
        assert!(
            self.focused,
            "input-to-present probe requires a focused window"
        );
        assert_eq!(
            self.occluded,
            Some(false),
            "input-to-present probe requires an unoccluded window"
        );
        let input_probe = self
            .input_probe
            .as_mut()
            .expect("initialized WindowServer probe");
        assert!(
            matches!(input_probe.phase, InputProbePhase::Idle),
            "input probe must be idle before posting input"
        );
        let baseline = input_probe
            .window_server
            .capture_center_pixel()
            .unwrap_or_else(|error| panic!("capture input baseline: {error}"));
        if input_probe.image_width == 0 {
            input_probe.image_width = baseline.image_width;
            input_probe.image_height = baseline.image_height;
            input_probe.bits_per_pixel = baseline.bits_per_pixel;
        } else {
            assert_eq!(input_probe.image_width, baseline.image_width);
            assert_eq!(input_probe.image_height, baseline.image_height);
            assert_eq!(input_probe.bits_per_pixel, baseline.bits_per_pixel);
        }
        let posted_at = Instant::now();
        input_probe
            .window_server
            .post_space_to_self()
            .unwrap_or_else(|error| panic!("post synthetic input: {error}"));
        input_probe.phase = InputProbePhase::AwaitingInput {
            posted_at,
            deadline: posted_at + INPUT_EVENT_TIMEOUT,
            baseline,
        };
    }

    #[cfg(target_os = "macos")]
    fn receive_input_probe(&mut self) {
        let (posted_at, baseline, marker_color) = {
            let input_probe = self
                .input_probe
                .as_mut()
                .expect("initialized WindowServer probe");
            let phase = std::mem::replace(&mut input_probe.phase, InputProbePhase::Idle);
            let InputProbePhase::AwaitingInput {
                posted_at,
                baseline,
                ..
            } = phase
            else {
                return;
            };
            input_probe.marker_is_green = !input_probe.marker_is_green;
            let marker_color = if input_probe.marker_is_green {
                Color::GREEN
            } else {
                Color::RED
            };
            (posted_at, baseline, marker_color)
        };

        let event_received_at = Instant::now();
        let marker = self.marker.expect("input marker Figure");
        let mut editor = self
            .runtime
            .as_mut()
            .expect("initialized Runtime")
            .figure(marker)
            .unwrap_or_else(|error| panic!("edit input marker: {error}"));
        editor
            .update_component(SetRectangleFill(marker_color))
            .unwrap_or_else(|error| panic!("update input marker: {error}"));
        self.input_probe
            .as_mut()
            .expect("initialized WindowServer probe")
            .phase = InputProbePhase::AwaitingRender {
            posted_at,
            event_received_at,
            deadline: event_received_at + WINDOW_SERVER_VISIBILITY_TIMEOUT,
            baseline,
        };
        self.window
            .as_ref()
            .expect("initialized window")
            .request_redraw();
    }

    #[cfg(target_os = "macos")]
    fn render_input_probe(&mut self, event_loop: &ActiveEventLoop) {
        let (posted_at, event_received_at, deadline, baseline) = {
            let input_probe = self
                .input_probe
                .as_mut()
                .expect("initialized WindowServer probe");
            let phase = std::mem::replace(&mut input_probe.phase, InputProbePhase::Idle);
            let InputProbePhase::AwaitingRender {
                posted_at,
                event_received_at,
                deadline,
                baseline,
            } = phase
            else {
                return;
            };
            (posted_at, event_received_at, deadline, baseline)
        };

        let window = self.window.as_ref().expect("initialized window");
        let renderer = self.renderer.as_mut().expect("initialized renderer");
        let runtime = self.runtime.as_mut().expect("initialized Runtime");
        let surface = surface_info(window);
        let submission = runtime
            .prepare_submission(surface, renderer.capabilities())
            .into_ready()
            .expect("input marker update must prepare a submission");
        let session_id = submission.session_id;
        let frame_id = submission.frame_id;
        let outcome = renderer.submit(&submission);
        let submit_returned_at = Instant::now();
        assert_eq!(
            outcome,
            RenderOutcome::Presented,
            "input marker surface submission must present"
        );

        let mut capture_attempts = 0usize;
        let visible = loop {
            capture_attempts += 1;
            let captured = self
                .input_probe
                .as_ref()
                .expect("initialized WindowServer probe")
                .window_server
                .capture_center_pixel()
                .unwrap_or_else(|error| panic!("capture WindowServer marker: {error}"));
            if captured.signature != baseline.signature {
                break captured;
            }
            assert!(
                Instant::now() < deadline,
                "input marker did not become WindowServer-visible within \
                 {WINDOW_SERVER_VISIBILITY_TIMEOUT:?}"
            );
            thread::sleep(WINDOW_SERVER_POLL_INTERVAL);
        };
        let visible_at = Instant::now();
        renderer
            .wait_for_gpu_idle(GPU_WAIT_TIMEOUT)
            .unwrap_or_else(|error| panic!("wait for input frame GPU completion: {error}"));
        runtime.complete_submission(session_id, frame_id, outcome);

        let sample = InputPresentationSample {
            input_post_to_event_ns: duration_ns(event_received_at.duration_since(posted_at)),
            event_to_window_server_visible_ns: duration_ns(
                visible_at.duration_since(event_received_at),
            ),
            input_post_to_window_server_visible_ns: duration_ns(
                visible_at.duration_since(posted_at),
            ),
            submit_return_to_window_server_visible_ns: duration_ns(
                visible_at.duration_since(submit_returned_at),
            ),
            capture_attempts,
            baseline_signature: baseline.signature.bytes(),
            visible_signature: visible.signature.bytes(),
        };

        let input_probe = self
            .input_probe
            .as_mut()
            .expect("initialized WindowServer probe");
        if input_probe.warmups_completed < self.cli.warmup_iterations {
            input_probe.warmups_completed += 1;
        } else {
            input_probe.samples.push(sample);
        }
        input_probe.next_input_at = Instant::now() + INPUT_SAMPLE_INTERVAL;
        input_probe.phase = InputProbePhase::Idle;
        if input_probe.samples.len() == self.cli.sample_iterations {
            self.finish(event_loop);
        }
    }

    #[cfg(target_os = "macos")]
    fn window_server_report(&self) -> Option<WindowServerVisibilityReport> {
        let input_probe = self.input_probe.as_ref()?;
        assert_eq!(
            input_probe.samples.len(),
            self.cli.sample_iterations,
            "WindowServer report requires all input samples"
        );
        Some(WindowServerVisibilityReport {
            method: "CGWindowListCreateImage self-window center-pixel transition",
            input_source: "CGEventPostToPid synthetic Space key",
            window_id: input_probe.window_server.window_id(),
            image_width: input_probe.image_width,
            image_height: input_probe.image_height,
            bits_per_pixel: input_probe.bits_per_pixel,
            warmup_iterations: input_probe.warmups_completed,
            sample_iterations: input_probe.samples.len(),
            input_post_to_event: StageReport::from_samples(
                input_probe
                    .samples
                    .iter()
                    .map(|sample| sample.input_post_to_event_ns)
                    .collect(),
            ),
            event_to_window_server_visible: StageReport::from_samples(
                input_probe
                    .samples
                    .iter()
                    .map(|sample| sample.event_to_window_server_visible_ns)
                    .collect(),
            ),
            input_post_to_window_server_visible: StageReport::from_samples(
                input_probe
                    .samples
                    .iter()
                    .map(|sample| sample.input_post_to_window_server_visible_ns)
                    .collect(),
            ),
            submit_return_to_window_server_visible: StageReport::from_samples(
                input_probe
                    .samples
                    .iter()
                    .map(|sample| sample.submit_return_to_window_server_visible_ns)
                    .collect(),
            ),
            capture_attempts: CountReport::from_samples(
                input_probe
                    .samples
                    .iter()
                    .map(|sample| sample.capture_attempts as u64)
                    .collect(),
            ),
            samples: input_probe.samples.clone(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn window_server_report(&self) -> Option<WindowServerVisibilityReport> {
        None
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        self.finished = true;
        let window_server_visibility = self.window_server_report();
        let window_server_present = window_server_visibility.is_some();
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
                window_server_visible: window_server_present,
                compositor_present: window_server_present,
                display_scanout: false,
                synthetic_input: window_server_present,
                physical_input: false,
                input_to_present: window_server_present,
            },
            scenario: ScenarioReport {
                name: match (self.render_mode, window_server_present) {
                    (RenderMode::Surface, true) => {
                        "wide_tree_full_surface_4096_window_server_input"
                    }
                    (RenderMode::Surface, false) => "wide_tree_full_surface_4096",
                    (RenderMode::Offscreen, _) => "wide_tree_full_offscreen_gpu_4096",
                    (RenderMode::SurfaceProbe, _) => {
                        unreachable!("probe must resolve before samples")
                    }
                }
                .to_owned(),
                figure_count: FIGURE_COUNT + 1 + usize::from(self.marker.is_some()),
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
                window_server_visibility,
                memory: ProcessMemoryReport {
                    method: peak_rss_method(),
                    before_setup_peak_rss_bytes: self.before_setup_peak_rss_bytes,
                    after_samples_peak_rss_bytes: peak_rss_bytes(),
                },
                notes: match (self.render_mode, window_server_present) {
                    (RenderMode::Surface, true) => {
                        "Surface submit includes SurfaceTexture::present(). A synthetic \
                         process-targeted input changes a marker, and WindowServer capture \
                         observes the new pixel. This is compositor-visible evidence, not \
                         physical display scanout."
                    }
                    (RenderMode::Surface, false) => {
                        "Surface submit includes Vello lowering, command encoding, internal queue \
                         submissions, retained-texture blit submission, and \
                         SurfaceTexture::present(). Queue completion is not a compositor \
                         presentation signal."
                    }
                    (RenderMode::Offscreen, _) => {
                        "The surface present probe was skipped, so samples use \
                         render_for_screenshot plus queue completion. No surface present call or \
                         compositor presentation is included."
                    }
                    (RenderMode::SurfaceProbe, _) => {
                        unreachable!("probe must resolve before samples")
                    }
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
        let renderer = VelloRenderer::new(Arc::clone(&window), surface)
            .unwrap_or_else(|error| panic!("initialize Vello renderer: {error}"));
        let (runtime, marker) = build_runtime(self.cli.require_window_server_present);
        #[cfg(target_os = "macos")]
        if self.cli.require_window_server_present {
            let window_server = macos_probe::WindowServerProbe::new(&window)
                .unwrap_or_else(|error| panic!("initialize WindowServer probe: {error}"));
            self.input_probe = Some(InputProbeState {
                window_server,
                phase: InputProbePhase::Idle,
                warmups_completed: 0,
                samples: Vec::with_capacity(self.cli.sample_iterations),
                next_input_at: Instant::now(),
                marker_is_green: false,
                image_width: 0,
                image_height: 0,
                bits_per_pixel: 0,
            });
        }
        #[cfg(not(target_os = "macos"))]
        assert!(
            !self.cli.require_window_server_present,
            "WindowServer input-to-present probe requires macOS"
        );
        self.adapter = Some(renderer.adapter_info());
        self.surface = Some(surface);
        self.runtime = Some(runtime);
        self.marker = marker;
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.physical_key == PhysicalKey::Code(KeyCode::Space) =>
            {
                #[cfg(target_os = "macos")]
                self.receive_input_probe();
            }
            WindowEvent::RedrawRequested => {
                #[cfg(target_os = "macos")]
                if self.input_probe.as_ref().is_some_and(|probe| {
                    matches!(probe.phase, InputProbePhase::AwaitingRender { .. })
                }) {
                    self.render_input_probe(event_loop);
                    return;
                }
                if self.samples.len() < self.cli.sample_iterations {
                    self.render_sample(event_loop);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        if self.samples.len() < self.cli.sample_iterations {
            if now >= self.next_redraw_at {
                self.next_redraw_at = now + REDRAW_INTERVAL;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw_at));
            return;
        }

        if self.cli.require_window_server_present {
            #[cfg(target_os = "macos")]
            {
                let should_post = self.input_probe.as_ref().is_some_and(|probe| {
                    matches!(probe.phase, InputProbePhase::Idle) && now >= probe.next_input_at
                });
                if should_post {
                    self.post_input_probe();
                }
                let wake_at = match &self
                    .input_probe
                    .as_ref()
                    .expect("initialized WindowServer probe")
                    .phase
                {
                    InputProbePhase::Idle => {
                        self.input_probe
                            .as_ref()
                            .expect("initialized WindowServer probe")
                            .next_input_at
                    }
                    InputProbePhase::AwaitingInput { deadline, .. } => {
                        assert!(
                            now < *deadline,
                            "synthetic input was not received within {INPUT_EVENT_TIMEOUT:?}"
                        );
                        *deadline
                    }
                    InputProbePhase::AwaitingRender { deadline, .. } => {
                        assert!(
                            now < *deadline,
                            "input update was not redrawn within \
                             {WINDOW_SERVER_VISIBILITY_TIMEOUT:?}"
                        );
                        *deadline
                    }
                };
                event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
                return;
            }
        }

        event_loop.set_control_flow(ControlFlow::Wait);
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
    window_server_visible: bool,
    compositor_present: bool,
    display_scanout: bool,
    synthetic_input: bool,
    physical_input: bool,
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
    window_server_visibility: Option<WindowServerVisibilityReport>,
    memory: ProcessMemoryReport,
    notes: &'static str,
}

#[derive(Serialize)]
struct WindowServerVisibilityReport {
    method: &'static str,
    input_source: &'static str,
    window_id: u32,
    image_width: usize,
    image_height: usize,
    bits_per_pixel: usize,
    warmup_iterations: usize,
    sample_iterations: usize,
    input_post_to_event: StageReport,
    event_to_window_server_visible: StageReport,
    input_post_to_window_server_visible: StageReport,
    submit_return_to_window_server_visible: StageReport,
    capture_attempts: CountReport,
    samples: Vec<InputPresentationSample>,
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
struct CountReport {
    min: u64,
    p50: u64,
    p95: u64,
    samples: Vec<u64>,
}

impl CountReport {
    fn from_samples(samples: Vec<u64>) -> Self {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        Self {
            min: sorted[0],
            p50: percentile(&sorted, 50),
            p95: percentile(&sorted, 95),
            samples,
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

fn build_runtime(include_input_marker: bool) -> (Runtime, Option<FigureId>) {
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
    let marker = include_input_marker.then(|| {
        tree.builder()
            .add_child(
                root,
                Box::new(RectangleFigure::new_with_color(
                    (LOGICAL_WIDTH - MARKER_SIZE) / 2.0,
                    (LOGICAL_HEIGHT - MARKER_SIZE) / 2.0,
                    MARKER_SIZE,
                    MARKER_SIZE,
                    Color::RED,
                )),
            )
            .expect("input marker Figure")
    });
    (Runtime::new(tree), marker)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_scenario_has_no_input_marker() {
        let (_, marker) = build_runtime(false);
        assert!(marker.is_none());
    }

    #[test]
    fn input_marker_color_change_uses_paint_only_component_update() {
        let (mut runtime, marker) = build_runtime(true);
        let marker = marker.expect("input marker");
        let receipt = runtime
            .figure(marker)
            .expect("edit marker")
            .update_component(SetRectangleFill(Color::GREEN))
            .expect("update marker");

        assert_eq!(receipt.figure, marker);
        assert_eq!(receipt.invalidation, novadraw::ComponentInvalidation::Paint);
    }
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
