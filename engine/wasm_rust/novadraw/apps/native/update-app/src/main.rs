use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use novadraw::advanced::UpdateManager;
use novadraw::event::{
    FigureEvent, ListenerDirective, NotificationEffect, UpdateEvent, UpdateListener,
};
use novadraw::layout::XYConstraint;
use novadraw::render::command::ImageData;
use novadraw::render::submission::ResourceSync;
use novadraw::render::{BackendCapabilities, DamageMode, RenderOutcome, SurfaceInfo};
use novadraw::{FigureId, Rectangle, Runtime};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};
use novadraw_demo_scenes::update::{
    STRESS_FIGURE_COUNT, WINDOW_HEIGHT, WINDOW_WIDTH, baseline_scene, stress_scene,
    validation_scene,
};

struct CaptureListener {
    effects: Arc<Mutex<Vec<NotificationEffect>>>,
}

impl UpdateListener for CaptureListener {
    fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::EmitUpdate(event));
        ListenerDirective::Keep
    }

    fn on_figure_event(&self, event: FigureEvent) -> ListenerDirective {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::EmitFigure(event));
        ListenerDirective::Keep
    }

    fn on_notify(&self, figure_id: FigureId) -> ListenerDirective {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::Notify { figure_id });
        ListenerDirective::Keep
    }
}

fn verify_damage_modes() -> Result<VerificationMetrics, String> {
    let mut runtime = Runtime::new(baseline_scene());
    let root = runtime.tree().contents().ok_or("missing root")?;
    let child = runtime
        .tree()
        .child_order(root)
        .ok_or("missing root children")?[0];

    let full = runtime
        .prepare_frame()
        .ok_or("initial frame was not prepared")?;
    if !full.damage().is_full() {
        return Err("initial Runtime frame is not full damage".to_string());
    }
    if runtime.prepare_frame().is_some() {
        return Err("no-op update produced render work".to_string());
    }
    runtime
        .figure(child)
        .unwrap()
        .repaint(None)
        .map_err(|error| error.to_string())?;
    let partial = runtime
        .prepare_frame()
        .ok_or("repaint frame was not prepared")?;
    if partial.damage().is_empty() || partial.damage().is_full() {
        return Err("repaint did not produce partial damage".to_string());
    }

    Ok(metrics([
        ("noop_commands", "0".to_string()),
        (
            "partial_regions",
            partial.damage().regions().len().to_string(),
        ),
    ]))
}

fn verify_notification_order() -> Result<VerificationMetrics, String> {
    let mut runtime = Runtime::new(validation_scene());
    let root = runtime.tree().contents().ok_or("missing root")?;
    let child = runtime
        .tree()
        .child_order(root)
        .ok_or("missing root children")?[0];
    runtime.prepare_frame();
    let effects = Arc::new(Mutex::new(Vec::new()));
    runtime.add_update_listener(Box::new(CaptureListener {
        effects: effects.clone(),
    }));
    runtime
        .figure(child)
        .unwrap()
        .set_layout_constraint(XYConstraint::at_size(180.0, 260.0, 140.0, 90.0))
        .map_err(|error| error.to_string())?;
    runtime
        .prepare_frame()
        .ok_or("layout frame was not prepared")?;
    let effects = effects.lock().unwrap();
    let validating = position(&effects, |effect| {
        matches!(
            effect,
            NotificationEffect::EmitUpdate(UpdateEvent::Validating)
        )
    })?;
    let moved = position(&effects, |effect| {
        matches!(
            effect,
            NotificationEffect::EmitFigure(FigureEvent::FigureMoved { figure_id, .. })
                if *figure_id == child
        )
    })?;
    let validated = position(&effects, |effect| {
        matches!(
            effect,
            NotificationEffect::EmitUpdate(UpdateEvent::Validated)
        )
    })?;
    if !(validating < moved && moved < validated) {
        return Err("notification order is not causal".to_string());
    }
    Ok(metrics([("effect_count", effects.len().to_string())]))
}

fn verify_dirty_coalescing() -> Result<VerificationMetrics, String> {
    let graph = baseline_scene();
    let root = graph.contents().ok_or("missing root")?;
    let child = graph.child_order(root).ok_or("missing root children")?[0];
    let mut manager = UpdateManager::new();
    manager.add_dirty_region(child, Rectangle::new(0.0, 0.0, 20.0, 20.0));
    manager.add_dirty_region(child, Rectangle::new(10.0, 10.0, 30.0, 30.0));
    if manager.dirty_count() != 1 {
        return Err("dirty regions were not coalesced per block".to_string());
    }
    let damage = manager.compute_damage();
    if damage != Rectangle::new(0.0, 0.0, 40.0, 40.0) {
        return Err(format!("unexpected coalesced damage: {damage:?}"));
    }
    Ok(metrics([("dirty_blocks", "1".to_string())]))
}

struct PanicOnceListener {
    did_panic: AtomicBool,
}

impl UpdateListener for PanicOnceListener {
    fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
        if matches!(event, UpdateEvent::Painting { .. })
            && !self.did_panic.swap(true, Ordering::SeqCst)
        {
            panic!("intentional verification panic");
        }
        ListenerDirective::Keep
    }

    fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
        ListenerDirective::Keep
    }

    fn on_notify(&self, _block_id: FigureId) -> ListenerDirective {
        ListenerDirective::Keep
    }
}

fn verify_runtime_fault_boundary() -> Result<VerificationMetrics, String> {
    let mut runtime = Runtime::new(baseline_scene());
    let root = runtime.tree().contents().ok_or("missing root")?;
    let child = runtime
        .tree()
        .child_order(root)
        .ok_or("missing root children")?[0];
    runtime.prepare_frame();
    runtime.add_update_listener(Box::new(PanicOnceListener {
        did_panic: AtomicBool::new(false),
    }));
    runtime
        .figure(child)
        .unwrap()
        .repaint(None)
        .map_err(|error| error.to_string())?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runtime.prepare_frame();
    }));
    std::panic::set_hook(previous_hook);
    if first.is_ok() || !runtime.is_faulted() {
        return Err("Runtime did not enter the fault boundary after a listener panic".to_string());
    }
    if runtime.prepare_frame().is_some() {
        return Err("faulted Runtime prepared another frame".to_string());
    }
    Ok(metrics([("faulted", "true".to_string())]))
}

fn verify_stress_1024() -> Result<VerificationMetrics, String> {
    let mut runtime = Runtime::new(stress_scene());
    let root = runtime.tree().contents().ok_or("missing root")?;
    runtime
        .figure(root)
        .unwrap()
        .revalidate()
        .map_err(|error| error.to_string())?;
    runtime
        .figure(root)
        .unwrap()
        .repaint(None)
        .map_err(|error| error.to_string())?;
    let start = Instant::now();
    let canvas = runtime
        .prepare_frame()
        .ok_or("stress frame was not prepared")?;
    let elapsed = start.elapsed();
    if runtime.prepare_frame().is_some() || canvas.commands().is_empty() {
        return Err("stress transaction did not converge".to_string());
    }
    Ok(metrics([
        ("figures", STRESS_FIGURE_COUNT.to_string()),
        ("elapsed_us", elapsed.as_micros().to_string()),
        ("commands", canvas.commands().len().to_string()),
    ]))
}

fn verify_submission_lifecycle() -> Result<VerificationMetrics, String> {
    let surface = SurfaceInfo {
        logical_width: WINDOW_WIDTH,
        logical_height: WINDOW_HEIGHT,
        pixel_width: WINDOW_WIDTH as u32,
        pixel_height: WINDOW_HEIGHT as u32,
        scale_factor: 1.0,
    };
    let mut runtime = Runtime::new(baseline_scene());
    let image = runtime.register_image();
    runtime
        .complete_image(
            image,
            ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
        )
        .map_err(|error| error.to_string())?;

    let first = runtime
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        .ok_or("initial submission was not prepared")?;
    if first.damage.mode() != DamageMode::Full
        || !matches!(
            &first.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        )
    {
        return Err("initial submission did not carry full damage and resources".to_string());
    }
    if !runtime.complete_submission(first.session_id, first.frame_id, RenderOutcome::Retry) {
        return Err("retry result was not accepted".to_string());
    }

    let retry = runtime
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        .ok_or("retry submission was not prepared")?;
    if retry.damage.mode() != DamageMode::Full
        || !matches!(
            &retry.resources,
            ResourceSync::Snapshot(snapshot)
                if snapshot.ready.len() == 1
                    && snapshot.ready[0].id == image.resource_id()
        )
    {
        return Err("retry did not restore full damage and resources".to_string());
    }
    if !runtime.complete_submission(retry.session_id, retry.frame_id, RenderOutcome::Presented) {
        return Err("presented result was not accepted".to_string());
    }

    Ok(metrics([
        ("first_frame_id", first.frame_id.get().to_string()),
        ("retry_frame_id", retry.frame_id.get().to_string()),
    ]))
}

fn metrics<const N: usize>(entries: [(&str, String); N]) -> VerificationMetrics {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn position(
    effects: &[NotificationEffect],
    predicate: impl Fn(&NotificationEffect) -> bool,
) -> Result<usize, String> {
    effects
        .iter()
        .position(predicate)
        .ok_or_else(|| "expected notification was not emitted".to_string())
}

fn verification_cases() -> [VerificationCase; 6] {
    [
        VerificationCase {
            name: "damage_modes",
            run: verify_damage_modes,
        },
        VerificationCase {
            name: "notification_order",
            run: verify_notification_order,
        },
        VerificationCase {
            name: "dirty_coalescing",
            run: verify_dirty_coalescing,
        },
        VerificationCase {
            name: "runtime_fault_boundary",
            run: verify_runtime_fault_boundary,
        },
        VerificationCase {
            name: "stress_1024",
            run: verify_stress_1024,
        },
        VerificationCase {
            name: "submission_lifecycle",
            run: verify_submission_lifecycle,
        },
    ]
}

fn main() {
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if cli.verify {
        run_verification(
            "update-app",
            &verification_cases(),
            cli.scenario.as_deref(),
            cli.report.as_deref(),
        )
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(1);
        });
        return;
    }

    let suite = novadraw_demo_scenes::update::suite();
    let screenshot_index = cli.screenshot.as_deref().map(|scenario| {
        let normalized = scenario.replace('_', "-");
        scenario
            .parse::<usize>()
            .ok()
            .or_else(|| suite.scenes.iter().position(|scene| scene.id == normalized))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            })
    });
    let scene_list = suite.into_entries();
    if cli.screenshot_all {
        run_demo_app_with_screenshot(
            "Update Pipeline Verification",
            "update-app",
            scene_list,
            true,
        )
        .expect("run update-app screenshots");
    } else if let Some(index) = screenshot_index {
        run_demo_app_with_scene_screenshot(
            "Update Pipeline Verification",
            "update-app",
            scene_list,
            index,
        )
        .expect("run update-app screenshot");
    } else {
        run_demo_app("Update Pipeline Verification", "update-app", scene_list)
            .expect("run update-app");
    }
}
