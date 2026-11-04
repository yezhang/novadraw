pub use crate::trace::TraceSession;
use anyhow::{Context, Result, bail};
use novadraw::{
    event::{Key, KeyModifiers},
    figure::flow::FlowSnapshot,
    render::{RenderBackend, RenderCommandKind, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    tree::FigureId,
};
use serde_json::{Value, json};

pub const INITIAL_WIDTH: f64 = 600.0;
pub const NARROW_WIDTH: f64 = 400.0;
pub const HEIGHT: f64 = 500.0;
pub const SCROLL_STEP: f64 = 100.0;
const RESIZE_STEP: f64 = 50.0;
const FRAME_RETRIES: usize = 8;

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

pub struct Research {
    pub runtime: Runtime,
    pub width: f64,
    pub capture_enabled: bool,
    operation_id: u64,
    cycle: u64,
    viewport: Option<FigureId>,
}

impl Research {
    pub fn has_operations(&self) -> bool {
        self.operation_id > 0
    }

    pub fn new(capture_enabled: bool) -> Self {
        Self {
            runtime: draw2d_official_examples::text_flow_large_example(),
            width: INITIAL_WIDTH,
            capture_enabled,
            operation_id: 0,
            cycle: 0,
            viewport: None,
        }
    }

    pub fn apply(&mut self, operation: Operation, trace: &mut TraceSession) -> Result<()> {
        self.operation_id += 1;
        let _operation = trace.scope("host-operation");
        let mut result = {
            let _mutation = trace.scope("host-mutation");
            self.mutate(operation)
        }
        .and_then(|()| {
            self.runtime
                .frames()
                .stabilize_for_query()
                .map_err(anyhow::Error::from)
        });
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
        trace.push(
            json!({"kind":"operation","operation_id":self.operation_id,"cycle":self.cycle,
            "operation":format!("{operation:?}"),"succeeded":result.is_ok(),
            "error":result.as_ref().err().map(|e|format!("{e:#}")),"snapshot":snapshot}),
        );
        result
    }

    fn mutate(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::Reset => {
                self.runtime = draw2d_official_examples::text_flow_large_example();
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
                self.runtime.viewports().scroll_by(
                    self.viewport.context("fixture viewport not prepared")?,
                    0.0,
                    delta,
                )?;
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
            "fragments":snapshot.fragments.len(),"revision":snapshot.revision,
            "effective_width":snapshot.paragraphs[0].layout.request().constraints().max_width,
            "first_source_text":&snapshot.paragraphs[0].layout.request().text()[snapshot.paragraphs[0].sources[0].range.clone()],
            "cumulative_flow_work":{"builds":work.builds,"styled_layout_calls":work.styled_layout_calls,
                "paragraph_hits":work.paragraph_hits,"source_geometry_calls":work.source_geometry_calls}
        }))
    }

    pub fn render(
        &mut self,
        backend: &mut impl RenderBackend,
        surface: SurfaceInfo,
        outcome: &str,
        trace: &mut TraceSession,
    ) -> Result<()> {
        for attempt in 0..FRAME_RETRIES {
            if self.render_attempt(backend, surface, outcome, attempt, trace)? {
                return Ok(());
            }
        }
        bail!("frame retry limit reached")
    }

    pub fn render_attempt(
        &mut self,
        backend: &mut impl RenderBackend,
        surface: SurfaceInfo,
        outcome: &str,
        attempt: usize,
        trace: &mut TraceSession,
    ) -> Result<bool> {
        let _frame = trace.scope("host-frame");
        self.runtime.frames().request_full_redraw();
        let prepared = self
            .runtime
            .frames()
            .prepare_submission(surface, backend.capabilities());
        let FramePreparation::Ready(frame) = prepared else {
            trace.push(
                json!({"kind":"frame_failure","operation_id":self.operation_id,
                "attempt":attempt,"error":format!("{prepared:?}")}),
            );
            bail!("frame preparation did not produce a frame: {prepared:?}");
        };
        let result = {
            let _submit = trace.scope("host-backend-submit");
            backend.submit(&frame)
        };
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
        trace.push(json!({"kind":"frame","operation_id":self.operation_id,"attempt":attempt,
            "session_id":format!("{:?}",frame.session_id),"frame_id":format!("{:?}",frame.frame_id),
            "commands":frame.commands.len(),"recorded_glyphs":glyphs,
            "outcome":if matches!(&completion, Ok(SubmissionAcknowledgement::Presented)) { outcome } else { "not_presented" },
            "result":format!("{result:?}"),"acknowledgement":completion.as_ref().ok().map(|a|format!("{a:?}")),
            "completion_error":completion.as_ref().err().map(ToString::to_string)}));
        match completion? {
            SubmissionAcknowledgement::Presented => Ok(true),
            SubmissionAcknowledgement::Retry
            | SubmissionAcknowledgement::Skipped
            | SubmissionAcknowledgement::SurfaceInvalidated { .. } => Ok(false),
            other => bail!("backend completion: {other:?}"),
        }
    }

    pub fn note(&self, phase: &str, value: Value, trace: &mut TraceSession) {
        trace.push(json!({"kind":phase,"operation_id":self.operation_id,"data":value}));
    }

    pub fn note_pending(&self, phase: &str, value: Value, trace: &mut TraceSession) {
        trace.push(json!({"kind":phase,"operation_id":self.operation_id + 1,"data":value}));
    }
}
