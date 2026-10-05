use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use novadraw::render::submission::{BackendSessionId, FrameId};
use novadraw::{
    Color, FigureTree, FramePreparation, RectangleFigure, RenderBackend, RenderOutcome, Runtime,
    SurfaceInfo,
};
use novadraw_backend_vello::{VelloAdapterInfo, VelloRenderer};
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{Document, HtmlCanvasElement, Window};

use super::ValidationBackend;

const REPORT_SCHEMA_VERSION: u32 = 1;
const RECTANGLE_COUNT: usize = 4_096;
const FIGURE_COUNT: usize = RECTANGLE_COUNT + 1;
const COLUMNS: usize = 64;
const LOGICAL_WIDTH: f64 = 1_024.0;
const LOGICAL_HEIGHT: f64 = 768.0;
const WARMUP_ITERATIONS: usize = 5;
const SAMPLE_ITERATIONS: usize = 30;
const STARTUP_RAFS: usize = 2;
const GPU_COMPLETION_TIMEOUT_MS: f64 = 5_000.0;
const RESULT_ELEMENT_ID: &str = "novadraw-performance-result";
const RESULT_GLOBAL: &str = "__NOVADRAW_PERFORMANCE_RESULT__";

pub(super) const fn surface_info() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: LOGICAL_WIDTH,
        logical_height: LOGICAL_HEIGHT,
        pixel_width: LOGICAL_WIDTH as u32,
        pixel_height: LOGICAL_HEIGHT as u32,
        scale_factor: 1.0,
    }
}

#[derive(Serialize)]
struct AdapterReport {
    name: String,
    vendor: u32,
    device: u32,
    device_type: String,
    backend: String,
    driver: String,
    driver_info: String,
}

impl From<VelloAdapterInfo> for AdapterReport {
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
struct BuildReport {
    git_revision: Option<&'static str>,
    git_dirty: Option<bool>,
}

#[derive(Serialize)]
struct BrowserReport {
    user_agent: String,
    platform: String,
    document_hidden: bool,
    document_has_focus: bool,
}

#[derive(Serialize)]
struct SamplingReport {
    warmup_iterations: usize,
    sample_iterations: usize,
    rectangle_count: usize,
    figure_count: usize,
}

#[derive(Serialize)]
struct MeasurementScope {
    cpu_prepare_submission: bool,
    backend_submit_cpu: bool,
    gpu_queue_completion_callback: bool,
    first_raf_after_gpu_completion: bool,
    browser_compositor_present: bool,
    physical_display_scanout: bool,
}

#[derive(Clone, Copy, Serialize)]
struct FrameSample {
    prepare_submission_ns: u64,
    backend_submit_cpu_ns: u64,
    submit_return_to_gpu_completion_callback_ns: u64,
    gpu_completion_callback_to_next_raf_ns: u64,
    frame_start_to_next_raf_ns: u64,
    command_count: usize,
}

#[derive(Serialize)]
struct StageSummary {
    minimum_ns: u64,
    median_ns: u64,
    p95_ns: u64,
    maximum_ns: u64,
}

#[derive(Serialize)]
struct SummaryReport {
    prepare_submission: StageSummary,
    backend_submit_cpu: StageSummary,
    submit_return_to_gpu_completion_callback: StageSummary,
    gpu_completion_callback_to_next_raf: StageSummary,
    frame_start_to_next_raf: StageSummary,
}

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    benchmark: &'static str,
    status: &'static str,
    build: BuildReport,
    browser: BrowserReport,
    adapter: AdapterReport,
    surface: SurfaceInfoReport,
    sampling: SamplingReport,
    measurement_scope: MeasurementScope,
    samples: Vec<FrameSample>,
    summary: SummaryReport,
    notes: &'static str,
}

#[derive(Serialize)]
struct SurfaceInfoReport {
    logical_width: f64,
    logical_height: f64,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f64,
}

impl From<SurfaceInfo> for SurfaceInfoReport {
    fn from(surface: SurfaceInfo) -> Self {
        Self {
            logical_width: surface.logical_width,
            logical_height: surface.logical_height,
            pixel_width: surface.pixel_width,
            pixel_height: surface.pixel_height,
            scale_factor: surface.scale_factor,
        }
    }
}

struct CompletionSignal {
    done: AtomicBool,
    completed_at_bits: AtomicU64,
}

impl CompletionSignal {
    fn new() -> Self {
        Self {
            done: AtomicBool::new(false),
            completed_at_bits: AtomicU64::new(f64::NAN.to_bits()),
        }
    }
}

struct PendingFrame {
    session_id: BackendSessionId,
    frame_id: FrameId,
    frame_started_at_ms: f64,
    submit_returned_at_ms: f64,
    prepare_submission_ns: u64,
    backend_submit_cpu_ns: u64,
    command_count: usize,
    completion: Arc<CompletionSignal>,
}

struct PerformanceBenchmark {
    window: Window,
    document: Document,
    runtime: Runtime,
    backend: VelloRenderer,
    adapter: Option<AdapterReport>,
    startup_rafs_remaining: usize,
    warmups_completed: usize,
    samples: Vec<FrameSample>,
    pending: Option<PendingFrame>,
    finished: bool,
}

impl PerformanceBenchmark {
    fn begin_frame(&mut self) -> Result<(), JsValue> {
        self.runtime.request_full_redraw();
        let frame_started_at_ms = browser_now(&self.window)?;
        let prepare_started_at_ms = browser_now(&self.window)?;
        let preparation = self
            .runtime
            .prepare_submission(surface_info(), self.backend.capabilities());
        let submission = match preparation {
            FramePreparation::Ready(submission) => submission,
            FramePreparation::Error(error) => {
                return Err(JsValue::from_str(&format!(
                    "performance frame preparation failed: {error}"
                )));
            }
            FramePreparation::Idle => {
                return Err(JsValue::from_str("performance frame was idle"));
            }
            FramePreparation::Suspended => {
                return Err(JsValue::from_str("performance surface was suspended"));
            }
            FramePreparation::AwaitingCompletion => {
                return Err(JsValue::from_str(
                    "performance frame awaited prior completion",
                ));
            }
        };
        let prepare_finished_at_ms = browser_now(&self.window)?;
        let prepare_submission_ns = duration_ns(prepare_started_at_ms, prepare_finished_at_ms)?;
        let session_id = submission.session_id;
        let frame_id = submission.frame_id;
        let command_count = submission.commands.len();

        let submit_started_at_ms = browser_now(&self.window)?;
        let outcome = self.backend.submit(&submission);
        let submit_returned_at_ms = browser_now(&self.window)?;
        let backend_submit_cpu_ns = duration_ns(submit_started_at_ms, submit_returned_at_ms)?;
        if outcome != RenderOutcome::Presented {
            self.runtime
                .complete_submission(session_id, frame_id, outcome);
            return Err(JsValue::from_str(&format!(
                "performance Vello submission returned {outcome:?}"
            )));
        }

        let completion = Arc::new(CompletionSignal::new());
        let callback_signal = Arc::clone(&completion);
        self.backend.on_submitted_work_done(move || {
            callback_signal
                .completed_at_bits
                .store(callback_browser_now().to_bits(), Ordering::Release);
            callback_signal.done.store(true, Ordering::Release);
        });
        self.pending = Some(PendingFrame {
            session_id,
            frame_id,
            frame_started_at_ms,
            submit_returned_at_ms,
            prepare_submission_ns,
            backend_submit_cpu_ns,
            command_count,
            completion,
        });
        Ok(())
    }

    fn finish_frame(&mut self, raf_timestamp_ms: f64) -> Result<(), JsValue> {
        let pending = self
            .pending
            .take()
            .ok_or_else(|| JsValue::from_str("missing pending performance frame"))?;
        let gpu_completed_at_ms =
            f64::from_bits(pending.completion.completed_at_bits.load(Ordering::Acquire));
        if !gpu_completed_at_ms.is_finite() {
            return Err(JsValue::from_str(
                "GPU completion callback did not provide a browser timestamp",
            ));
        }
        self.runtime.complete_submission(
            pending.session_id,
            pending.frame_id,
            RenderOutcome::Presented,
        );
        let sample = FrameSample {
            prepare_submission_ns: pending.prepare_submission_ns,
            backend_submit_cpu_ns: pending.backend_submit_cpu_ns,
            submit_return_to_gpu_completion_callback_ns: duration_ns(
                pending.submit_returned_at_ms,
                gpu_completed_at_ms,
            )?,
            gpu_completion_callback_to_next_raf_ns: duration_ns(
                gpu_completed_at_ms,
                raf_timestamp_ms,
            )?,
            frame_start_to_next_raf_ns: duration_ns(pending.frame_started_at_ms, raf_timestamp_ms)?,
            command_count: pending.command_count,
        };

        if self.warmups_completed < WARMUP_ITERATIONS {
            self.warmups_completed += 1;
        } else {
            if let Some(previous) = self.samples.last()
                && previous.command_count != sample.command_count
            {
                return Err(JsValue::from_str(
                    "render command count changed between performance samples",
                ));
            }
            self.samples.push(sample);
        }
        self.update_progress()?;
        if self.samples.len() == SAMPLE_ITERATIONS {
            self.publish_report()?;
            self.finished = true;
        }
        Ok(())
    }

    fn advance(&mut self, raf_timestamp_ms: f64) -> Result<bool, JsValue> {
        self.require_visible_page()?;
        if self.startup_rafs_remaining > 0 {
            self.startup_rafs_remaining -= 1;
            return Ok(true);
        }

        if let Some(pending) = &self.pending {
            if pending.completion.done.load(Ordering::Acquire) {
                let completed_at_ms =
                    f64::from_bits(pending.completion.completed_at_bits.load(Ordering::Acquire));
                if completed_at_ms.is_finite() && raf_timestamp_ms < completed_at_ms {
                    return Ok(true);
                }
                self.finish_frame(raf_timestamp_ms)?;
            } else if raf_timestamp_ms - pending.submit_returned_at_ms > GPU_COMPLETION_TIMEOUT_MS {
                return Err(JsValue::from_str(
                    "timed out waiting for WebGPU queue completion",
                ));
            }
        } else {
            self.begin_frame()?;
        }
        Ok(!self.finished)
    }

    fn require_visible_page(&self) -> Result<(), JsValue> {
        if self.document.hidden() {
            return Err(JsValue::from_str(
                "performance evidence requires a visible browser page",
            ));
        }
        if !self.document.has_focus()? {
            return Err(JsValue::from_str(
                "performance evidence requires the browser page to have focus",
            ));
        }
        Ok(())
    }

    fn update_progress(&self) -> Result<(), JsValue> {
        let body = self
            .document
            .body()
            .ok_or_else(|| JsValue::from_str("missing document body"))?;
        body.set_attribute(
            "data-performance-warmups",
            &self.warmups_completed.to_string(),
        )?;
        body.set_attribute("data-performance-samples", &self.samples.len().to_string())?;
        if let Some(status) = self.document.get_element_by_id("runtime-status") {
            status.set_text_content(Some(&format!(
                "WebGPU performance · warmup {}/{} · sample {}/{}",
                self.warmups_completed,
                WARMUP_ITERATIONS,
                self.samples.len(),
                SAMPLE_ITERATIONS
            )));
        }
        Ok(())
    }

    fn publish_report(&mut self) -> Result<(), JsValue> {
        let report = BenchmarkReport {
            schema_version: REPORT_SCHEMA_VERSION,
            benchmark: "ga2-webgpu-browser",
            status: "pass",
            build: BuildReport {
                git_revision: option_env!("NOVADRAW_GIT_REVISION"),
                git_dirty: option_env!("NOVADRAW_GIT_DIRTY")
                    .and_then(|value| value.parse::<bool>().ok()),
            },
            browser: BrowserReport {
                user_agent: self.window.navigator().user_agent()?,
                platform: self.window.navigator().platform()?,
                document_hidden: self.document.hidden(),
                document_has_focus: self.document.has_focus()?,
            },
            adapter: self
                .adapter
                .take()
                .ok_or_else(|| JsValue::from_str("missing Vello adapter report"))?,
            surface: surface_info().into(),
            sampling: SamplingReport {
                warmup_iterations: WARMUP_ITERATIONS,
                sample_iterations: SAMPLE_ITERATIONS,
                rectangle_count: RECTANGLE_COUNT,
                figure_count: FIGURE_COUNT,
            },
            measurement_scope: MeasurementScope {
                cpu_prepare_submission: true,
                backend_submit_cpu: true,
                gpu_queue_completion_callback: true,
                first_raf_after_gpu_completion: true,
                browser_compositor_present: false,
                physical_display_scanout: false,
            },
            summary: SummaryReport::from_samples(&self.samples),
            samples: self.samples.clone(),
            notes: "The queue callback proves completion of WebGPU work submitted before the \
                    callback registration. The following requestAnimationFrame is a conservative \
                    browser presentation opportunity; it is not exact browser compositor \
                    presentation and does not prove physical display scanout.",
        };
        let json = serde_json::to_string_pretty(&report).map_err(|error| {
            JsValue::from_str(&format!("serialize performance report: {error}"))
        })?;
        let result = self.document.create_element("script")?;
        result.set_id(RESULT_ELEMENT_ID);
        result.set_attribute("type", "application/json")?;
        result.set_text_content(Some(&json));
        self.document
            .body()
            .ok_or_else(|| JsValue::from_str("missing document body"))?
            .append_child(&result)?;

        let object = js_sys::JSON::parse(&json)?;
        js_sys::Reflect::set(
            self.window.as_ref(),
            &JsValue::from_str(RESULT_GLOBAL),
            &object,
        )?;
        let body = self
            .document
            .body()
            .ok_or_else(|| JsValue::from_str("missing document body"))?;
        body.set_attribute("data-performance-state", "complete")?;
        body.set_attribute("data-ready", "true")?;
        if let Some(status) = self.document.get_element_by_id("runtime-status") {
            status.set_text_content(Some("WebGPU performance · complete"));
        }
        Ok(())
    }

    fn fail(&mut self, error: JsValue) {
        self.finished = true;
        let message = error.as_string().unwrap_or_else(|| format!("{error:?}"));
        if let Some(body) = self.document.body() {
            let _ = body.set_attribute("data-performance-state", "error");
            let _ = body.set_attribute("data-performance-error", &message);
            let _ = body.set_attribute("data-ready", "error");
        }
        if let Some(status) = self.document.get_element_by_id("runtime-status") {
            status.set_text_content(Some(&format!("ERROR · {message}")));
        }
        web_sys::console::error_1(&error);
    }

    fn schedule(app: &Rc<RefCell<Self>>) -> Result<(), JsValue> {
        let window = app.borrow().window.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::once_into_js(move |timestamp: f64| {
            let result = callback_app.borrow_mut().advance(timestamp);
            match result {
                Ok(true) => {
                    if let Err(error) = Self::schedule(&callback_app) {
                        callback_app.borrow_mut().fail(error);
                    }
                }
                Ok(false) => {}
                Err(error) => callback_app.borrow_mut().fail(error),
            }
        });
        window.request_animation_frame(callback.unchecked_ref())?;
        Ok(())
    }
}

impl SummaryReport {
    fn from_samples(samples: &[FrameSample]) -> Self {
        Self {
            prepare_submission: StageSummary::new(
                samples
                    .iter()
                    .map(|sample| sample.prepare_submission_ns)
                    .collect(),
            ),
            backend_submit_cpu: StageSummary::new(
                samples
                    .iter()
                    .map(|sample| sample.backend_submit_cpu_ns)
                    .collect(),
            ),
            submit_return_to_gpu_completion_callback: StageSummary::new(
                samples
                    .iter()
                    .map(|sample| sample.submit_return_to_gpu_completion_callback_ns)
                    .collect(),
            ),
            gpu_completion_callback_to_next_raf: StageSummary::new(
                samples
                    .iter()
                    .map(|sample| sample.gpu_completion_callback_to_next_raf_ns)
                    .collect(),
            ),
            frame_start_to_next_raf: StageSummary::new(
                samples
                    .iter()
                    .map(|sample| sample.frame_start_to_next_raf_ns)
                    .collect(),
            ),
        }
    }
}

impl StageSummary {
    fn new(mut values: Vec<u64>) -> Self {
        values.sort_unstable();
        Self {
            minimum_ns: values[0],
            median_ns: percentile(&values, 50),
            p95_ns: percentile(&values, 95),
            maximum_ns: values[values.len() - 1],
        }
    }
}

pub(super) fn start(
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    backend: ValidationBackend,
) -> Result<(), JsValue> {
    let ValidationBackend::Vello(backend) = backend else {
        return Err(JsValue::from_str("performance mode requires backend=vello"));
    };
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("missing document body"))?;
    body.set_attribute("data-mode", "performance")?;
    body.set_attribute("data-ready", "false")?;
    body.set_attribute("data-performance-state", "running")?;
    canvas.set_width(LOGICAL_WIDTH as u32);
    canvas.set_height(LOGICAL_HEIGHT as u32);
    canvas
        .style()
        .set_property("width", &format!("{LOGICAL_WIDTH}px"))?;
    canvas
        .style()
        .set_property("height", &format!("{LOGICAL_HEIGHT}px"))?;
    canvas.style().set_property("aspect-ratio", "1024 / 768")?;

    let adapter = backend.adapter_info().into();
    let app = Rc::new(RefCell::new(PerformanceBenchmark {
        window,
        document,
        runtime: build_runtime(),
        backend: *backend,
        adapter: Some(adapter),
        startup_rafs_remaining: STARTUP_RAFS,
        warmups_completed: 0,
        samples: Vec::with_capacity(SAMPLE_ITERATIONS),
        pending: None,
        finished: false,
    }));
    app.borrow().require_visible_page()?;
    app.borrow().update_progress()?;
    PerformanceBenchmark::schedule(&app)?;
    Ok(())
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
    for index in 0..RECTANGLE_COUNT {
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
            .expect("performance FigureTree construction must remain valid");
    }
    Runtime::new(tree)
}

fn browser_now(window: &Window) -> Result<f64, JsValue> {
    window
        .performance()
        .map(|clock| clock.now())
        .ok_or_else(|| JsValue::from_str("browser performance clock unavailable"))
}

fn callback_browser_now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(f64::NAN, |clock| clock.now())
}

fn duration_ns(start_ms: f64, end_ms: f64) -> Result<u64, JsValue> {
    let duration_ms = end_ms - start_ms;
    if !duration_ms.is_finite() || duration_ms < 0.0 {
        return Err(JsValue::from_str(
            "browser performance clock moved backwards",
        ));
    }
    Ok((duration_ms * 1_000_000.0).round().min(u64::MAX as f64) as u64)
}

fn percentile(values: &[u64], percentile: usize) -> u64 {
    let rank = values.len().saturating_mul(percentile).div_ceil(100);
    values[rank.saturating_sub(1).min(values.len() - 1)]
}
