use crate::Config;
use anyhow::{Context, Result, bail};
use novadraw::{
    event::{Key, KeyModifiers},
    figure::flow::FlowSnapshot,
    render::{RenderBackend, RenderCommandKind, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    tree::FigureId,
};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const INITIAL_WIDTH: f64 = 600.0;
pub const NARROW_WIDTH: f64 = 400.0;
pub const HEIGHT: f64 = 500.0;
pub const SCROLL_STEP: f64 = 100.0;
const RESIZE_STEP: f64 = 50.0;
const FRAME_RETRIES: usize = 8;
const JOURNAL_ENTRIES: usize = 8192;
const JOURNAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Reset,
    Append,
    Scroll(f64),
    Resize(f64),
    Stable,
}

pub fn replay(cycles: usize) -> Vec<Operation> {
    let mut result = Vec::new();
    for _ in 0..cycles {
        result.extend([
            Operation::Reset,
            Operation::Append,
            Operation::Scroll(SCROLL_STEP),
            Operation::Scroll(-SCROLL_STEP),
            Operation::Stable,
        ]);
        let mut width = INITIAL_WIDTH - RESIZE_STEP;
        while width >= NARROW_WIDTH {
            result.push(Operation::Resize(width));
            width -= RESIZE_STEP;
        }
        width = NARROW_WIDTH + RESIZE_STEP;
        while width <= INITIAL_WIDTH {
            result.push(Operation::Resize(width));
            width += RESIZE_STEP;
        }
    }
    result
}

pub fn surface(width: f64) -> SurfaceInfo {
    SurfaceInfo {
        logical_width: width,
        logical_height: HEIGHT,
        pixel_width: width as u32,
        pixel_height: HEIGHT as u32,
        scale_factor: 1.0,
    }
}

/// App-owned bounded journal. Serialization happens after each measured interval.
pub struct Journal {
    header: Value,
    records: Vec<Value>,
    bytes: usize,
    dropped: usize,
}

impl Journal {
    pub fn has_records(&self) -> bool {
        !self.records.is_empty() || self.dropped > 0
    }

    pub fn new(config: &Config) -> Self {
        Self {
            header: json!({
                "kind":"config", "schema":1, "fixture":"text_flow_large_example",
                "run_id":format!("{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d|d.as_nanos())),
                "package_version":env!("CARGO_PKG_VERSION"), "rustc":env!("RESEARCH_RUSTC"),
                "profile":env!("RESEARCH_PROFILE"), "build_git_head":env!("RESEARCH_GIT"),
                "build_worktree_dirty":env!("RESEARCH_DIRTY") == "true",
                "source_identity":"build-time observation; not a source-content hash",
                "headless":config.headless, "timing":config.timing, "requests":config.requests,
                "gpu_wait":config.gpu_wait, "cycles":config.cycles, "replay":config.replay,
                "screenshot":config.screenshot, "exit_after_replay":config.exit_after_replay,
                "initial_width":INITIAL_WIDTH, "height":HEIGHT, "append":"Z",
                "scroll_step":SCROLL_STEP, "narrow_width":NARROW_WIDTH, "resize_step":RESIZE_STEP,
                "fonts":"original fixture: bundled Inter, JetBrains Mono, Hebrew and Japanese",
                "journal_entries":JOURNAL_ENTRIES, "journal_bytes":JOURNAL_BYTES,
                "requested_damage":"full",
                "glyph_measure":"recorded glyphs, not visible glyphs",
                "presentation_measure":"backend acknowledgement, not compositor or scanout",
            }),
            records: Vec::new(),
            bytes: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, record: Value) {
        let bytes = record.to_string().len();
        if self.records.len() >= JOURNAL_ENTRIES || self.bytes.saturating_add(bytes) > JOURNAL_BYTES
        {
            self.dropped += 1;
        } else {
            self.bytes += bytes;
            self.records.push(record);
        }
    }

    pub fn export(&mut self, directory: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(directory)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for suffix in 0..FRAME_RETRIES {
            let path = directory.join(format!(
                "textflow-{timestamp}-{}-{suffix}.jsonl",
                std::process::id()
            ));
            let file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            let mut writer = BufWriter::new(file);
            for record in std::iter::once(&self.header).chain(&self.records) {
                serde_json::to_writer(&mut writer, record)?;
                writeln!(writer)?;
            }
            serde_json::to_writer(
                &mut writer,
                &json!({"kind":"journal_end","records":self.records.len(),"dropped":self.dropped}),
            )?;
            writeln!(writer)?;
            writer.flush()?;
            self.records.clear();
            self.bytes = 0;
            self.dropped = 0;
            return Ok(path);
        }
        bail!("cannot allocate a fresh export filename")
    }
}

pub struct Research {
    pub runtime: Runtime,
    pub width: f64,
    pub timing: bool,
    requests: bool,
    operation_id: u64,
    cycle: u64,
    viewport: Option<FigureId>,
}

impl Research {
    pub fn has_operations(&self) -> bool {
        self.operation_id > 0
    }

    pub fn new(timing: bool, requests: bool) -> Self {
        let mut runtime = draw2d_official_examples::text_flow_large_example();
        runtime.frames().set_timing_enabled(timing);
        Self {
            runtime,
            width: INITIAL_WIDTH,
            timing,
            requests,
            operation_id: 0,
            cycle: 0,
            viewport: None,
        }
    }

    pub fn apply(&mut self, operation: Operation, journal: &mut Journal) -> Result<()> {
        self.operation_id += 1;
        let started = Instant::now();
        let result = self.mutate(operation);
        let mutation_ns = started.elapsed().as_nanos();
        let started = Instant::now();
        let mut result = result.and_then(|()| {
            self.runtime
                .frames()
                .stabilize_for_query()
                .map_err(anyhow::Error::from)
        });
        let stabilization_ns = started.elapsed().as_nanos();
        // Never expose a stale snapshot as the outcome of a failed attempt.
        let snapshot = if result.is_ok() {
            match self.snapshot_summary() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    result = Err(error);
                    Value::Null
                }
            }
        } else {
            Value::Null
        };
        journal.push(
            json!({"kind":"operation","operation_id":self.operation_id,"cycle":self.cycle,
            "operation":format!("{operation:?}"),"mutation_ns":mutation_ns,
            "stabilization_ns":stabilization_ns,"succeeded":result.is_ok(),
            "error":result.as_ref().err().map(|error|format!("{error:#}")),"snapshot":snapshot}),
        );
        self.diagnostics("operation", journal);
        result
    }

    fn mutate(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::Reset => {
                self.runtime = draw2d_official_examples::text_flow_large_example();
                self.runtime.frames().set_timing_enabled(self.timing);
                self.width = INITIAL_WIDTH;
                self.viewport = None;
                self.cycle += 1;
            }
            Operation::Append => {
                anyhow::ensure!(
                    self.runtime
                        .input()
                        .dispatch_key_pressed(Key::Character('Z'), KeyModifiers::default())
                        .is_handled(),
                    "fixture did not handle append Z"
                );
            }
            Operation::Scroll(delta) => {
                let viewport = self.viewport.context("fixture viewport not prepared")?;
                self.runtime.viewports().scroll_by(viewport, 0.0, delta)?;
            }
            Operation::Resize(width) => {
                self.runtime
                    .frames()
                    .resize_logical_viewport(width, HEIGHT)?;
                self.width = width;
            }
            Operation::Stable => {}
        }
        Ok(())
    }

    fn find_flow(&self) -> Result<&FlowSnapshot> {
        let mut pending = vec![
            self.runtime
                .tree()
                .contents()
                .context("fixture root missing")?,
        ];
        while let Some(id) = pending.pop() {
            if let Some(presentation) = self.runtime.tree().prepared_presentation(id)?
                && let Some(snapshot) = presentation.data::<FlowSnapshot>()
            {
                return Ok(snapshot);
            }
            pending.extend(self.runtime.tree().child_order(id)?);
        }
        bail!("fixture FlowSnapshot missing")
    }

    fn snapshot_summary(&mut self) -> Result<Value> {
        if self.viewport.is_none() {
            let root = self
                .runtime
                .tree()
                .contents()
                .context("fixture root missing")?;
            let pane = self.runtime.tree().child_order(root)?[0];
            self.viewport = self
                .runtime
                .tree()
                .child_order(pane)?
                .into_iter()
                .find(|id| {
                    self.runtime
                        .tree()
                        .node(*id)
                        .is_ok_and(|node| node.figure_name() == "ViewportFigure")
                });
            anyhow::ensure!(self.viewport.is_some(), "fixture viewport missing");
        }
        let snapshot = self.find_flow()?;
        let work = snapshot.statistics;
        Ok(json!({
            "paragraphs":snapshot.paragraphs.len(),
            "bytes":snapshot.paragraphs.iter().map(|p|p.layout.request().text().len()).sum::<usize>(),
            "lines":snapshot.paragraphs.iter().map(|p|p.layout.layout().line_metrics().len()).sum::<usize>(),
            "runs":snapshot.paragraphs.iter().map(|p|p.layout.runs().len()).sum::<usize>(),
            "fragments":snapshot.fragments.len(), "revision":snapshot.revision,
            "effective_width":snapshot.paragraphs[0].layout.request().constraints().max_width,
            "first_source_text":&snapshot.paragraphs[0].layout.request().text()[snapshot.paragraphs[0].sources[0].range.clone()],
            "cumulative_flow_work":{"builds":work.builds,"styled_layout_calls":work.styled_layout_calls,
                "paragraph_hits":work.paragraph_hits,"source_geometry_calls":work.source_geometry_calls,
                "contribution_ns":work.contribution_ns,"styled_layout_ns":work.styled_layout_ns,
                "source_geometry_ns":work.source_geometry_ns,"build_ns":work.build_ns},
        }))
    }

    pub fn diagnostics(&mut self, phase: &str, journal: &mut Journal) {
        for timing in self.runtime.frames().take_timings() {
            journal.push(
                json!({"kind":"timing","operation_id":self.operation_id,"phase":phase,
                "stage":timing.stage,"calls":timing.calls,
                "inclusive_ns":timing.inclusive_ns,"exclusive_ns":timing.exclusive_ns}),
            );
        }
        let observations = self.runtime.frames().take_flow_layout_observations();
        if !observations.records.is_empty() || observations.dropped > 0 {
            journal.push(
                json!({"kind":"layout_denominator","operation_id":self.operation_id,"phase":phase,
                "attempts":observations.records.len(),"dropped":observations.dropped}),
            );
        }
        for observation in observations.records {
            journal.push(json!({"kind":"layout_attempt","operation_id":self.operation_id,"phase":phase,
                "root":format!("{:?}",observation.root),"owner":format!("{:?}",observation.owner),
                "build":observation.build,"scene_revision":observation.scene_revision,
                "sources":format!("{:?}",observation.sources),"layout_phase":observation.phase,
                "inline_prefix":observation.inline_prefix,"engine_identity":format!("{:?}",observation.engine_identity),
                "engine_revision":observation.engine_revision,"cache_hit":observation.cache_hit,
                "succeeded":observation.succeeded,"elapsed_ns":observation.elapsed_ns,
                "complete_request_debug":self.requests.then(||format!("{:?}",observation.request))}));
        }
    }

    pub fn render(
        &mut self,
        backend: &mut impl RenderBackend,
        surface: SurfaceInfo,
        outcome: &str,
        journal: &mut Journal,
    ) -> Result<()> {
        for attempt in 0..FRAME_RETRIES {
            if self.render_attempt(backend, surface, outcome, attempt, journal)? {
                return Ok(());
            }
        }
        bail!("frame retry limit reached")
    }

    /// One attempt allows Native hosts to return to the window loop between retries.
    pub fn render_attempt(
        &mut self,
        backend: &mut impl RenderBackend,
        surface: SurfaceInfo,
        outcome: &str,
        attempt: usize,
        journal: &mut Journal,
    ) -> Result<bool> {
        self.runtime.frames().request_full_redraw();
        let start = Instant::now();
        let prepared = self
            .runtime
            .frames()
            .prepare_submission(surface, backend.capabilities());
        let record_ns = start.elapsed().as_nanos();
        let FramePreparation::Ready(frame) = prepared else {
            journal.push(
                json!({"kind":"frame_failure","operation_id":self.operation_id,
                    "attempt":attempt,"cpu_ir_ns":record_ns,"error":format!("{prepared:?}")}),
            );
            self.diagnostics("record_failure", journal);
            bail!("frame preparation did not produce a frame: {prepared:?}");
        };
        let start = Instant::now();
        let result = backend.submit(&frame);
        let submit_ns = start.elapsed().as_nanos();
        let completion = self.runtime.frames().complete_submission(
            frame.session_id,
            frame.frame_id,
            result.clone(),
        );
        let glyphs: usize = frame
            .commands
            .iter()
            .map(|command| match &command.kind {
                RenderCommandKind::DrawGlyphRun { run, .. } => run.glyphs.len(),
                _ => 0,
            })
            .sum();
        journal.push(json!({"kind":"frame","operation_id":self.operation_id,"attempt":attempt,
                "session_id":format!("{:?}",frame.session_id),"frame_id":format!("{:?}",frame.frame_id),
                "cpu_ir_ns":record_ns,"backend_submit_ns":submit_ns,"commands":frame.commands.len(),"requested_damage":"full",
                "recorded_glyphs":glyphs,"outcome":if matches!(&completion, Ok(SubmissionAcknowledgement::Presented)) { outcome } else { "not_presented" },
                "result":format!("{result:?}"),
                "acknowledgement":completion.as_ref().ok().map(|ack|format!("{ack:?}")),
                "completion_error":completion.as_ref().err().map(ToString::to_string),"surface":{
                    "logical_width":surface.logical_width,"logical_height":surface.logical_height,
                    "pixel_width":surface.pixel_width,"pixel_height":surface.pixel_height,"scale_factor":surface.scale_factor}}));
        self.diagnostics("record_and_submit", journal);
        let acknowledgement = completion?;
        match acknowledgement {
            SubmissionAcknowledgement::Presented => Ok(true),
            SubmissionAcknowledgement::Retry
            | SubmissionAcknowledgement::Skipped
            | SubmissionAcknowledgement::SurfaceInvalidated { .. } => Ok(false),
            _ => bail!("backend completion: {acknowledgement:?}"),
        }
    }

    pub fn note(&self, phase: &str, value: Value, journal: &mut Journal) {
        journal.push(json!({"kind":phase,"operation_id":self.operation_id,"data":value}));
    }

    pub fn note_pending(&self, phase: &str, value: Value, journal: &mut Journal) {
        journal.push(json!({"kind":phase,"operation_id":self.operation_id + 1,"data":value}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            headless: true,
            replay: true,
            cycles: 1,
            timing: false,
            requests: false,
            gpu_wait: false,
            exit_after_replay: false,
            screenshot: false,
            output: PathBuf::new(),
        }
    }

    #[test]
    fn original_fixture_operations_and_reset() -> Result<()> {
        let mut journal = Journal::new(&config());
        let mut research = Research::new(false, false);
        research.apply(Operation::Reset, &mut journal)?;
        let initial = research.snapshot_summary()?;
        assert_eq!(initial["bytes"], 51232);
        assert_eq!(initial["first_source_text"], "Normal text.");
        assert_eq!(initial["effective_width"], 586.0);
        research.apply(Operation::Append, &mut journal)?;
        assert_eq!(research.snapshot_summary()?["bytes"], 51233);
        let calls = research.find_flow()?.statistics.styled_layout_calls;
        research.apply(Operation::Scroll(SCROLL_STEP), &mut journal)?;
        assert_eq!(research.find_flow()?.statistics.styled_layout_calls, calls);
        research.apply(Operation::Resize(NARROW_WIDTH), &mut journal)?;
        assert_eq!(research.snapshot_summary()?["effective_width"], 386.0);
        research.apply(Operation::Reset, &mut journal)?;
        assert_eq!(research.snapshot_summary()?["bytes"], initial["bytes"]);
        assert_eq!(
            research.snapshot_summary()?["first_source_text"],
            initial["first_source_text"]
        );
        Ok(())
    }

    #[test]
    fn failed_budget_is_recorded_and_reset_recovers() -> Result<()> {
        let mut journal = Journal::new(&config());
        let mut research = Research::new(true, false);
        let mut budget = research.runtime.frames().preparation_budget();
        budget.work = 0;
        research.runtime.frames().set_preparation_budget(budget);
        assert!(research.apply(Operation::Stable, &mut journal).is_err());
        assert!(
            journal
                .records
                .iter()
                .any(|record| record["kind"] == "operation"
                    && record["succeeded"] == false
                    && record["snapshot"].is_null())
        );
        research.apply(Operation::Reset, &mut journal)?;
        assert_eq!(research.snapshot_summary()?["bytes"], 51232);
        Ok(())
    }

    #[test]
    fn backend_retry_and_failure_remain_in_journal() -> Result<()> {
        use novadraw::render::{
            BackendCapabilities, RenderDisposition, RenderFailure, RenderFailureDiagnostic,
            RenderResult, RenderSubmission,
        };
        struct Backend {
            retries: usize,
            fail: bool,
        }
        impl RenderBackend for Backend {
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::FULL_FRAME_ONLY
                    .with_glyph_runs()
                    .with_image_resources()
                    .with_custom_strokes()
                    .with_path_clips()
                    .with_linear_gradients()
                    .with_host_background()
                    .with_per_operation_antialias()
            }
            fn submit(&mut self, _: &RenderSubmission) -> RenderResult {
                if self.fail {
                    Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                        "test failure",
                    )))
                } else if self.retries > 0 {
                    self.retries -= 1;
                    Ok(RenderDisposition::Retry)
                } else {
                    Ok(RenderDisposition::Presented)
                }
            }
            fn resize(&mut self, _: u32, _: u32, _: f64) {}
        }
        let mut journal = Journal::new(&config());
        let mut research = Research::new(false, false);
        research.apply(Operation::Reset, &mut journal)?;
        research.render(
            &mut Backend {
                retries: 1,
                fail: false,
            },
            surface(INITIAL_WIDTH),
            "test_only",
            &mut journal,
        )?;
        assert_eq!(
            journal
                .records
                .iter()
                .filter(|v| v["kind"] == "frame")
                .count(),
            2
        );
        assert!(
            journal
                .records
                .iter()
                .any(|v| v["acknowledgement"] == "Retry")
        );
        assert!(
            research
                .render(
                    &mut Backend {
                        retries: 0,
                        fail: true
                    },
                    surface(INITIAL_WIDTH),
                    "test_only",
                    &mut journal
                )
                .is_err()
        );
        assert!(journal.records.iter().any(|v| {
            v["result"]
                .as_str()
                .is_some_and(|s| s.contains("test failure"))
        }));
        Ok(())
    }

    #[test]
    fn export_is_parseable_bounded_and_never_overwrites() -> Result<()> {
        let directory = std::env::current_dir()?
            .join("../../../../../target/verification/textflow-large-app/export-test");
        let mut journal = Journal::new(&config());
        for _ in 0..=JOURNAL_ENTRIES {
            journal.push(json!({"kind":"test"}));
        }
        assert_eq!(journal.dropped, 1);
        let first = journal.export(&directory)?;
        let bytes = std::fs::read(&first)?;
        let records: Vec<Value> = String::from_utf8(bytes.clone())?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<_, _>>()?;
        assert_eq!(records.last().unwrap()["dropped"], 1);
        let second = journal.export(&directory)?;
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first)?, bytes);
        Ok(())
    }
}
