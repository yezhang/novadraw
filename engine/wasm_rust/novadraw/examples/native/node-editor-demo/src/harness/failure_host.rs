//! Real Native host verification of ADR-037 fault exit, using public extension points.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use novadraw::figure::{
    Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureLifecycle,
    FigureLifecycleContext, LIFECYCLE, LifecycleCapability,
};
use novadraw::host::PlatformHost;
use novadraw::render::{DamageMode, RenderBackend, ResourceSync};
use novadraw::runtime::FramePreparationError;
use novadraw_backend_vello::VelloRenderer;
use novadraw_platform_winit::WinitPlatformHost;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

const GPU_EXIT_TIMEOUT: Duration = Duration::from_secs(10);

struct AttachPanic(Arc<AtomicUsize>);

impl Figure for AttachPanic {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 1.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "S06 Native attach panic"
    }

    fn register_capabilities(
        &self,
        builder: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        builder.register(LIFECYCLE, LifecycleCapability::of::<Self>())
    }
}

impl FigureLifecycle for AttachPanic {
    fn on_attached(&mut self, _: FigureLifecycleContext) {
        panic!("S06 Native fixture: attached callback failed");
    }

    fn on_detached(&mut self, _: FigureLifecycleContext) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn check(condition: bool, message: &str) -> HarnessResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}

fn verify(window: Arc<Window>, renderer: &mut VelloRenderer) -> HarnessResult<()> {
    let host = WinitPlatformHost::new(Arc::clone(&window));
    let surface = host.surface_info();
    println!("S06 surface: {surface:?}");
    println!("S06 adapter: {:?}", renderer.adapter_info());

    let mut old = EditorHarness::new()?;
    old.create_node()?;
    let revision = old.model_revision();
    let history = old.history_lengths();
    let old_part = old.viewer.contents();
    let old_figure = old.viewer.parts().get(old_part).unwrap().primary_figure();
    let submission = old
        .prepare_submission(surface, renderer.capabilities())
        .into_ready()
        .map_err(|error| format!("old preparation: {error:?}"))?;
    // Real GPU queue work without depending on WindowServer drawable availability.
    // This proves producer exit and resource takeover, not surface presentation.
    let result = renderer.render_for_screenshot(&submission);
    println!("S06 old GPU result: {result:?}");
    check(
        matches!(result, Ok(RenderDisposition::Presented)),
        "old real GPU retained-texture submission failed",
    )?;
    // The GPU has received the frame; the Runtime completion is deliberately pending.
    let detached = Arc::new(AtomicUsize::new(0));
    let failure = catch_unwind(AssertUnwindSafe(|| {
        old.viewer
            .add_feedback_visual(None, false, Box::new(AttachPanic(Arc::clone(&detached))))
    }));
    check(failure.is_err(), "lifecycle fixture did not unwind")?;
    check(old.viewer.is_faulted(), "Viewer must be faulted")?;
    check(old.viewer.runtime().is_faulted(), "Core must be faulted")?;
    check(
        old.viewer
            .last_projection_cleanup()
            .is_some_and(|cleanup| cleanup.runtime_faulted && !cleanup.live_figures.is_empty()),
        "fault cleanup must report attached resources",
    )?;
    check(
        matches!(
            old.prepare_submission(surface, renderer.capabilities()),
            FramePreparation::Error(FramePreparationError::Faulted)
        ),
        "faulted owner prepared new GPU work",
    )?;
    check(old.create_node().is_err(), "faulted owner accepted editing")?;
    check(
        matches!(
            old.complete_submission(submission.session_id, submission.frame_id, result),
            Err(SubmissionCompletionError::Faulted)
        ),
        "Core fault completion unlocked the old owner",
    )?;
    println!("S06 F03b: stopped new work; Core fault rejected pending completion");

    // Preserve committed model and history, independent of the disposable projection.
    let model = old.viewer.model();
    let snapshot = DemoModel {
        revision: model.revision,
        nodes: model.nodes.clone(),
        labels: model.labels.clone(),
        children: model.children.clone(),
        connections: model.connections.clone(),
        bendpoints: model.bendpoints.clone(),
        events: Vec::new(),
        next_id: model.next_id,
        next_connection_id: model.next_connection_id,
    };
    renderer
        .wait_for_gpu_idle(GPU_EXIT_TIMEOUT)
        .map_err(|error| error.to_string())?;
    println!("S06 F03c: old GPU queue drained before releasing its producer");
    let EditorHarness { viewer, domain, .. } = old;
    drop(viewer);
    check(
        detached.load(Ordering::SeqCst) == 0,
        "owner exit retried the faulted Figure lifecycle",
    )?;
    println!("S06 F03c: old Viewer/Runtime released; no repeated callback");

    let mut fresh = EditorHarness::with_model(snapshot)?;
    fresh.domain = domain;
    check(
        fresh.model_revision() == revision,
        "model revision changed during reconstruction",
    )?;
    check(
        fresh.history_lengths() == history,
        "command history changed during reconstruction",
    )?;
    check(
        fresh.viewer.contents() != old_part,
        "rebuilt Part identity was reused",
    )?;
    check(
        fresh.viewer.runtime().tree().is_attached(old_figure)
            == Err(novadraw::tree::TreeQueryError::ForeignFigure(old_figure)),
        "old Figure identity is valid in the replacement Runtime",
    )?;
    let baseline = fresh
        .prepare_submission(surface, renderer.capabilities())
        .into_ready()
        .map_err(|error| format!("replacement preparation: {error:?}"))?;
    check(
        matches!(baseline.resources, ResourceSync::Snapshot(_))
            && baseline.damage.mode() == DamageMode::Full,
        "replacement did not establish Snapshot + Full",
    )?;
    let result = renderer.render_for_screenshot(&baseline);
    println!("S06 replacement GPU result: {result:?}");
    check(
        matches!(result, Ok(RenderDisposition::Presented)),
        "replacement real GPU retained-texture baseline failed",
    )?;
    check(
        matches!(
            fresh.complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            ),
            Err(SubmissionCompletionError::MismatchedSession { expected, actual })
                if expected == baseline.session_id && actual == submission.session_id
        ),
        "replacement accepted a late old completion",
    )?;
    let acknowledgement = fresh
        .complete_submission(baseline.session_id, baseline.frame_id, result)
        .map_err(|error| error.to_string())?;
    check(
        matches!(acknowledgement, SubmissionAcknowledgement::Presented),
        "replacement acknowledgement did not accept its baseline",
    )?;
    fresh.create_node()?;
    check(
        fresh.model_revision() > revision && fresh.history_lengths().0 == history.0 + 1,
        "editing did not resume with preserved model and history",
    )?;
    renderer
        .wait_for_gpu_idle(GPU_EXIT_TIMEOUT)
        .map_err(|error| error.to_string())?;
    println!(
        "S06 F03c: Snapshot + Full rendered to retained texture; stale completion rejected; editing resumed"
    );
    Ok(())
}

#[derive(Default)]
struct VerificationApp {
    window: Option<Arc<Window>>,
    renderer: Option<VelloRenderer>,
    result: Option<HarnessResult<()>>,
}

impl ApplicationHandler for VerificationApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match event_loop.create_window(
            WindowAttributes::default()
                .with_title("S06 Native failure exit verification")
                .with_inner_size(LogicalSize::new(WIDTH, HEIGHT)),
        ) {
            Ok(window) => {
                let window = Arc::new(window);
                let surface = WinitPlatformHost::new(Arc::clone(&window)).surface_info();
                match VelloRenderer::new(Arc::clone(&window), surface) {
                    Ok(renderer) => self.renderer = Some(renderer),
                    Err(error) => {
                        self.result = Some(Err(error.to_string()));
                        event_loop.exit();
                        return;
                    }
                }
                window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                self.result = Some(Err(error.to_string()));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) && self.result.is_none() {
            self.result = Some(verify(
                Arc::clone(self.window.as_ref().unwrap()),
                self.renderer.as_mut().unwrap(),
            ));
            event_loop.exit();
        }
    }
}

pub(crate) fn run() -> HarnessResult<()> {
    let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = VerificationApp::default();
    event_loop
        .run_app(&mut app)
        .map_err(|error| error.to_string())?;
    app.result.ok_or("Native event loop did not start")??;
    println!("S06 Native host failure-exit verification passed");
    Ok(())
}
