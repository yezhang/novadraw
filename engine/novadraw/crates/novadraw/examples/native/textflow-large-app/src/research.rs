pub use crate::trace::TraceSession;
use anyhow::{Context, Result, bail};
use novadraw::{
    event::{Key, KeyModifiers},
    figure::flow::{FlowLayoutService, FlowSnapshot},
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
const SCROLL_REPLAY_STEP: f64 = 40.0;
const SCROLL_REPLAY_PAIRS: usize = 12;
const RASTER_VALIDATION_STEPS: usize = 16;
const RASTER_VALIDATION_STEP: f64 = 200.0;
const ASSEMBLY_CACHE_UNITS: usize = 16 * 1024 * 1024;
const ASSEMBLY_RETAINED_UNITS: usize = 64 * 1024 * 1024;
const PROJECTION_RETAINED_BYTES: usize = 64 * 1024 * 1024;
// This research fixture performs repeated whole-document stabilization. Both
// control and reuse runs use this explicit bounded session policy.
pub const PREPARATION_WORK_UNITS: u64 = 128 * 1024 * 1024;
pub const PREPARATION_ALLOCATION_UNITS: u64 = 64 * 1024 * 1024;

fn flow_runtime(service: &FlowLayoutService) -> Runtime {
    let budget = novadraw::figure::PreparationBudget::new(
        PREPARATION_WORK_UNITS,
        PREPARATION_ALLOCATION_UNITS,
    );
    // Projection scopes have a separate policy from Runtime preparation.
    // Configure it before fixture construction and keep it on every reset.
    let tree =
        novadraw::tree::FigureTree::with_projection_budget(budget, PROJECTION_RETAINED_BYTES);
    let mut runtime = draw2d_official_examples::text_flow_large_example_with_service_and_tree(
        service.clone(),
        tree,
    );
    runtime
        .frames()
        .set_preparation_budget(novadraw::figure::PreparationBudget::new(
            PREPARATION_WORK_UNITS,
            PREPARATION_ALLOCATION_UNITS,
        ));
    runtime
}

fn flow_service(enabled: bool) -> FlowLayoutService {
    let service = FlowLayoutService::default().with_assembly_reuse(enabled);
    service
        .set_assembly_limits(ASSEMBLY_CACHE_UNITS, ASSEMBLY_RETAINED_UNITS)
        .expect("new empty flow service");
    service
}

pub fn raster_validation() -> Vec<Operation> {
    let mut result = vec![
        Operation::Reset,
        Operation::Stable,
        Operation::FullDamage(false),
    ];
    for delta in [RASTER_VALIDATION_STEP, -RASTER_VALIDATION_STEP] {
        result.extend((0..RASTER_VALIDATION_STEPS).map(|_| Operation::Scroll(delta)));
    }
    result.extend([
        Operation::Append,
        Operation::Scroll(SCROLL_REPLAY_STEP),
        Operation::Scroll(-SCROLL_REPLAY_STEP),
        Operation::Resize(NARROW_WIDTH),
        Operation::Scroll(SCROLL_REPLAY_STEP),
        Operation::Resize(INITIAL_WIDTH),
        Operation::Reset,
        Operation::Stable,
    ]);
    result
}

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Reset,
    Append,
    Scroll(f64),
    Resize(f64),
    Stable,
    Wheel(f64),
    Pointer,
    FullDamage(bool),
    DragStart,
    DragMove(f64),
    DragEnd,
}

const INTERACTION_STEPS: usize = 20;
const INTERACTION_DRAG_STEP: f64 = 3.0;

pub fn interaction_replay(cycles: usize) -> Vec<Operation> {
    let mut result = vec![
        Operation::Reset,
        Operation::Stable,
        Operation::FullDamage(false),
    ];
    for _ in 0..cycles {
        result.push(Operation::DragStart);
        for direction in [1.0, -1.0] {
            result.extend(
                (0..INTERACTION_STEPS)
                    .map(|_| Operation::DragMove(direction * INTERACTION_DRAG_STEP)),
            );
        }
        result.push(Operation::DragEnd);
        for direction in [-1.0, 1.0] {
            result.extend(
                (0..INTERACTION_STEPS).map(|_| Operation::Wheel(direction * SCROLL_REPLAY_STEP)),
            );
        }
        for step in (0..INTERACTION_STEPS).rev().chain(1..=INTERACTION_STEPS) {
            result.push(Operation::Resize(
                NARROW_WIDTH
                    + (INITIAL_WIDTH - NARROW_WIDTH) * step as f64 / INTERACTION_STEPS as f64,
            ));
        }
        result.extend([Operation::Append, Operation::Stable, Operation::Pointer]);
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameAttempt {
    Complete,
    Retry,
    SurfaceUnavailable,
}

pub fn scroll_replay(cycles: usize, natural_only: bool) -> Vec<Operation> {
    let mut result = vec![Operation::Reset, Operation::Stable];
    let modes: &[bool] = if natural_only {
        &[false]
    } else {
        &[true, false]
    };
    for &full in modes {
        result.push(Operation::FullDamage(full));
        for _ in 0..cycles {
            for _ in 0..SCROLL_REPLAY_PAIRS {
                result.extend([
                    Operation::Scroll(SCROLL_REPLAY_STEP),
                    Operation::Scroll(-SCROLL_REPLAY_STEP),
                ]);
            }
            for _ in 0..SCROLL_REPLAY_PAIRS {
                result.extend([
                    Operation::Wheel(-SCROLL_REPLAY_STEP),
                    Operation::Wheel(SCROLL_REPLAY_STEP),
                ]);
            }
            for _ in 0..SCROLL_REPLAY_PAIRS {
                result.extend([Operation::Pointer, Operation::Stable]);
            }
        }
    }
    result
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
    pub full_redraw: bool,
    operation_id: u64,
    cycle: u64,
    viewport: Option<FigureId>,
    typing_page: FigureId,
    flow_assembly_reuse: bool,
    flow_service: FlowLayoutService,
}

impl Research {
    pub fn has_operations(&self) -> bool {
        self.operation_id > 0
    }

    pub fn new(capture_enabled: bool, flow_assembly_reuse: bool) -> Self {
        let service = flow_service(flow_assembly_reuse);
        let mut runtime = flow_runtime(&service);
        let typing_page = runtime
            .input()
            .state()
            .focus_owner()
            .expect("fixture typing page");
        Self {
            runtime,
            typing_page,
            width: INITIAL_WIDTH,
            capture_enabled,
            full_redraw: true,
            operation_id: 0,
            cycle: 0,
            viewport: None,
            flow_assembly_reuse,
            flow_service: service,
        }
    }

    pub fn apply(&mut self, operation: Operation, trace: &mut TraceSession) -> Result<()> {
        self.operation_id += 1;
        let _operation = trace.scope("host-operation");
        let before_y = self.viewport_y();
        let mut result = {
            let _mutation = trace.scope("host-mutation");
            self.mutate(operation)
        }
        .and_then(|()| {
            let _query = trace.scope("host-stabilize-query");
            self.runtime
                .frames()
                .stabilize_for_query()
                .map_err(anyhow::Error::from)
        });
        let snapshot = if result.is_ok() {
            let _snapshot = trace.scope("host-snapshot-summary");
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
            "error":result.as_ref().err().map(|e|format!("{e:#}")),"snapshot":snapshot,
            "viewport_before_y":before_y,"viewport_after_y":self.viewport_y(),"full_redraw":self.full_redraw}),
        );
        result
    }

    fn mutate(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::Reset => {
                self.flow_service = flow_service(self.flow_assembly_reuse);
                self.runtime = flow_runtime(&self.flow_service);
                self.typing_page = self
                    .runtime
                    .input()
                    .state()
                    .focus_owner()
                    .context("reset fixture typing page missing")?;
                self.width = INITIAL_WIDTH;
                self.viewport = None;
                self.cycle += 1;
            }
            Operation::Append => {
                self.focus_typing_page()?;
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
            Operation::Stable
            | Operation::Wheel(_)
            | Operation::Pointer
            | Operation::DragStart
            | Operation::DragMove(_)
            | Operation::DragEnd => {}
            Operation::FullDamage(full) => self.full_redraw = full,
        }
        Ok(())
    }

    pub fn focus_typing_page(&mut self) -> Result<()> {
        self.runtime.focus().request(self.typing_page)?;
        Ok(())
    }

    pub fn viewport_y(&mut self) -> Option<f64> {
        let id = self.viewport?;
        self.runtime
            .viewports()
            .handle(id)
            .ok()
            .map(|h| h.view_location().y())
    }

    pub fn scroll_thumb_center(&self) -> Result<novadraw::geometry::Point> {
        use novadraw::{container::ScrollBarPart, geometry::Point};
        let tree = self.runtime.tree();
        let mut pending = vec![tree.contents().context("fixture root missing")?];
        while let Some(id) = pending.pop() {
            if tree.layout_constraint::<ScrollBarPart>(id).ok().flatten()
                == Some(&ScrollBarPart::Thumb)
            {
                let bounds = tree.figure_bounds(id)?;
                // The fixture has only a vertical scrollbar. Use its actual
                // laid-out thumb and current surface projection.
                if bounds.height > 0.0 && bounds.width > 0.0 {
                    return Ok(tree
                        .local_to_surface_transform(id)?
                        .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0)));
                }
            }
            pending.extend(tree.child_order(id)?);
        }
        bail!("visible fixture scrollbar thumb missing")
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
        let work = self.flow_service.work_statistics()?;
        let usage = self.flow_service.assembly_usage()?;
        Ok(json!({
            "paragraphs":snapshot.paragraphs.len(),
            "bytes":snapshot.paragraphs.iter().map(|p|p.layout.request().text().len()).sum::<usize>(),
            "lines":snapshot.paragraphs.iter().map(|p|p.layout.layout().line_metrics().len()).sum::<usize>(),
            "runs":snapshot.paragraphs.iter().map(|p|p.layout.runs().len()).sum::<usize>(),
            "fragments":snapshot.fragments.len(),"revision":snapshot.revision,
            "effective_width":snapshot.paragraphs[0].layout.request().constraints().max_width,
            "first_source_text":&snapshot.paragraphs[0].layout.request().text()[snapshot.paragraphs[0].sources[0].range.clone()],
            "cumulative_flow_work":{"builds":work.builds,"styled_layout_calls":work.styled_layout_calls,
                "paragraph_hits":work.paragraph_hits,"source_geometry_calls":work.source_geometry_calls,
                "assembly_builds":work.assembly_builds,"assembly_hits":work.assembly_hits,
                "assembly_ineligible":work.assembly_ineligible},
            "assembly_usage":{"retained_units":usage.retained_units,"limit":usage.limit,
                "content_count":usage.content_count,"cache_limit":ASSEMBLY_CACHE_UNITS}
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
            if self.render_attempt(backend, surface, outcome, attempt, trace)?
                == FrameAttempt::Complete
            {
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
    ) -> Result<FrameAttempt> {
        let _frame = trace.scope("host-frame");
        if self.full_redraw {
            self.runtime.frames().request_full_redraw();
        }
        let prepared = self
            .runtime
            .frames()
            .prepare_submission(surface, backend.capabilities());
        let FramePreparation::Ready(frame) = prepared else {
            if !self.full_redraw && matches!(prepared, FramePreparation::Idle) {
                trace.push(json!({"kind":"frame","operation_id":self.operation_id,
                    "attempt":attempt,"outcome":"no_damage","commands":0,"recorded_glyphs":0}));
                return Ok(FrameAttempt::Complete);
            }
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
            "damage":format!("{:?}",frame.damage.mode()),"damage_regions":frame.damage.regions().len(),
            "damage_union":format!("{:?}",frame.damage.union()),"surface":format!("{surface:?}"),
            "outcome":if matches!(&completion, Ok(SubmissionAcknowledgement::Presented)) { outcome } else { "not_presented" },
            "result":format!("{result:?}"),"acknowledgement":completion.as_ref().ok().map(|a|format!("{a:?}")),
            "completion_error":completion.as_ref().err().map(ToString::to_string)}));
        match completion? {
            SubmissionAcknowledgement::Presented => Ok(FrameAttempt::Complete),
            SubmissionAcknowledgement::Skipped => Ok(FrameAttempt::SurfaceUnavailable),
            SubmissionAcknowledgement::Retry
            | SubmissionAcknowledgement::SurfaceInvalidated { .. } => Ok(FrameAttempt::Retry),
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

#[cfg(test)]
mod replay_tests {
    use super::*;

    #[test]
    fn large_scene_projection_policy_survives_reset_without_assembly_reuse() {
        let mut research = Research::new(false, false);
        research.full_redraw = false;
        for operation in [
            Operation::Stable,
            Operation::Reset,
            Operation::Scroll(SCROLL_STEP),
            Operation::Resize(NARROW_WIDTH),
            Operation::Append,
            Operation::Reset,
        ] {
            research.mutate(operation).unwrap();
            research.runtime.frames().stabilize_for_query().unwrap();
            let snapshot = research.snapshot_summary().unwrap();
            assert!(snapshot["bytes"].as_u64().unwrap() >= 51_232);
            let point = research.scroll_thumb_center().unwrap();
            assert!(point.x() > 0.0 && point.x() < research.width);
        }
    }

    #[test]
    fn interaction_replay_uses_a_visible_thumb_and_balanced_input() {
        let mut runtime = Research::new(false, true);
        runtime.runtime.frames().stabilize_for_query().unwrap();
        let point = runtime.scroll_thumb_center().unwrap();
        assert!(point.x() > INITIAL_WIDTH / 2.0 && point.x() < INITIAL_WIDTH);
        assert!(point.y() > 0.0 && point.y() < HEIGHT);
        let operations = interaction_replay(2);
        assert_eq!(
            operations
                .iter()
                .filter(|op| matches!(op, Operation::DragStart))
                .count(),
            2
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| matches!(op, Operation::DragEnd))
                .count(),
            2
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| matches!(op, Operation::Resize(_)))
                .count(),
            80
        );
    }
}
