use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use novadraw::{
    BlockId, Color, FigureEvent, FigureGraph, GridLayout, NotificationEffect, Rectangle,
    RectangleFigure, SceneUpdateManager, UpdateEvent, UpdateListener, UpdateManager, XYConstraint,
    XYLayout,
};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const STRESS_FIGURE_COUNT: usize = 1024;

type SceneEntry = (&'static str, Box<dyn FnMut() -> FigureGraph>);

fn gray_background() -> RectangleFigure {
    RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::hex("#eeeeee"))
}

fn baseline_scene() -> FigureGraph {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(gray_background()));
    for (x, color) in [(100.0, "#e74c3c"), (325.0, "#2ecc71"), (550.0, "#3498db")] {
        graph.add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                x,
                200.0,
                150.0,
                100.0,
                Color::hex(color),
            )),
        );
    }
    graph
}

fn partial_damage_scene() -> FigureGraph {
    let mut graph = baseline_scene();
    let root = graph.get_contents().expect("contents");
    let target = graph.child_order(root).expect("root children")[1];
    let mut manager = SceneUpdateManager::new();
    let old_bounds = graph.figure_bounds(target).expect("target bounds");
    graph.set_bounds_with_update(
        &mut manager,
        target,
        old_bounds.x + 40.0,
        old_bounds.y + 30.0,
        old_bounds.width,
        old_bounds.height,
    );
    let _ = graph.perform_update(&mut manager);
    graph
}

fn validation_scene() -> FigureGraph {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(gray_background()));
    graph.set_block_layout_manager(root, Arc::new(XYLayout::new()));
    for (index, color) in ["#9b59b6", "#f39c12", "#1abc9c"].iter().enumerate() {
        let child = graph.add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                140.0,
                90.0,
                Color::hex(color),
            )),
        );
        graph.set_constraint(
            child,
            XYConstraint::at_size(100.0 + index as f64 * 220.0, 220.0, 140.0, 90.0),
        );
    }
    graph.revalidate(root);
    graph
}

fn stress_scene() -> FigureGraph {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(gray_background()));
    graph.set_block_layout_manager(
        root,
        Arc::new(
            GridLayout::new(32)
                .with_margins(8.0, 8.0)
                .with_spacing(2.0, 2.0),
        ),
    );
    for index in 0..STRESS_FIGURE_COUNT {
        let channel = (index % 32) as f64 / 31.0;
        graph.add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                20.0,
                14.0,
                Color::rgba(channel, 0.55, 1.0 - channel, 1.0),
            )),
        );
    }
    graph.revalidate(root);
    graph
}

fn scenes() -> Vec<SceneEntry> {
    vec![
        ("baseline", Box::new(baseline_scene)),
        ("partial_damage", Box::new(partial_damage_scene)),
        ("validation", Box::new(validation_scene)),
        ("stress_1024", Box::new(stress_scene)),
    ]
}

struct CaptureListener {
    effects: Arc<Mutex<Vec<NotificationEffect>>>,
}

impl UpdateListener for CaptureListener {
    fn on_update_event(&self, event: UpdateEvent) {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::EmitUpdate(event));
    }

    fn on_figure_event(&self, event: FigureEvent) {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::EmitFigure(event));
    }

    fn on_notify(&self, block_id: BlockId) {
        self.effects
            .lock()
            .unwrap()
            .push(NotificationEffect::Notify { block_id });
    }
}

fn verify_damage_modes() -> Result<VerificationMetrics, String> {
    let mut graph = baseline_scene();
    let root = graph.get_contents().ok_or("missing root")?;
    let child = graph.child_order(root).ok_or("missing root children")?[0];
    let mut manager = SceneUpdateManager::new();

    let noop = graph.perform_update(&mut manager);
    if !noop.damage().is_empty() || !noop.commands().is_empty() {
        return Err("no-op update produced render work".to_string());
    }
    if !graph.render().damage().is_full() {
        return Err("direct render is not full damage".to_string());
    }
    graph.repaint(&mut manager, child, None);
    let partial = graph.perform_update(&mut manager);
    if partial.damage().is_empty() || partial.damage().is_full() {
        return Err("repaint did not produce partial damage".to_string());
    }

    Ok(metrics([
        ("noop_commands", noop.commands().len().to_string()),
        (
            "partial_regions",
            partial.damage().regions().len().to_string(),
        ),
    ]))
}

fn verify_notification_order() -> Result<VerificationMetrics, String> {
    let mut graph = validation_scene();
    let root = graph.get_contents().ok_or("missing root")?;
    let child = graph.child_order(root).ok_or("missing root children")?[0];
    graph.drain_notification_effects();
    graph.set_constraint(child, XYConstraint::at_size(180.0, 260.0, 140.0, 90.0));
    let mut manager = SceneUpdateManager::new();
    graph.mark_invalid(&mut manager, child);
    let effects = Arc::new(Mutex::new(Vec::new()));
    manager.add_listener(Box::new(CaptureListener {
        effects: effects.clone(),
    }));
    let _ = graph.perform_update(&mut manager);
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
            NotificationEffect::EmitFigure(FigureEvent::FigureMoved { block_id, .. })
                if *block_id == child
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
    let mut graph = baseline_scene();
    let root = graph.get_contents().ok_or("missing root")?;
    let child = graph.child_order(root).ok_or("missing root children")?[0];
    let mut manager = SceneUpdateManager::new();
    manager.add_dirty_region(child, Rectangle::new(0.0, 0.0, 20.0, 20.0));
    manager.add_dirty_region(child, Rectangle::new(10.0, 10.0, 30.0, 30.0));
    if manager.dirty_count() != 1 {
        return Err("dirty regions were not coalesced per block".to_string());
    }
    let damage = manager.compute_damage();
    if damage != Rectangle::new(0.0, 0.0, 40.0, 40.0) {
        return Err(format!("unexpected coalesced damage: {damage:?}"));
    }
    let _ = graph.perform_update(&mut manager);
    Ok(metrics([("dirty_blocks", "1".to_string())]))
}

struct PanicOnceListener {
    did_panic: AtomicBool,
}

impl UpdateListener for PanicOnceListener {
    fn on_update_event(&self, event: UpdateEvent) {
        if matches!(event, UpdateEvent::Painting { .. })
            && !self.did_panic.swap(true, Ordering::SeqCst)
        {
            panic!("intentional verification panic");
        }
    }

    fn on_figure_event(&self, _event: FigureEvent) {}
    fn on_notify(&self, _block_id: BlockId) {}
}

fn verify_panic_recovery() -> Result<VerificationMetrics, String> {
    let mut graph = baseline_scene();
    let root = graph.get_contents().ok_or("missing root")?;
    let child = graph.child_order(root).ok_or("missing root children")?[0];
    let mut manager = SceneUpdateManager::new();
    manager.add_listener(Box::new(PanicOnceListener {
        did_panic: AtomicBool::new(false),
    }));
    graph.repaint(&mut manager, child, None);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = graph.perform_update(&mut manager);
    }));
    std::panic::set_hook(previous_hook);
    if first.is_ok() || manager.is_updating() || !manager.is_update_queued() {
        return Err("manager did not recover from listener panic".to_string());
    }
    let _ = graph.perform_update(&mut manager);
    if manager.is_update_queued() {
        return Err("recovered update did not drain work".to_string());
    }
    Ok(metrics([("recovered", "true".to_string())]))
}

fn verify_stress_1024() -> Result<VerificationMetrics, String> {
    let mut graph = stress_scene();
    let root = graph.get_contents().ok_or("missing root")?;
    let mut manager = SceneUpdateManager::new();
    graph.mark_invalid(&mut manager, root);
    graph.repaint(&mut manager, root, None);
    let start = Instant::now();
    let canvas = graph.perform_update(&mut manager);
    let elapsed = start.elapsed();
    if manager.is_update_queued() || canvas.commands().is_empty() {
        return Err("stress transaction did not converge".to_string());
    }
    Ok(metrics([
        ("figures", STRESS_FIGURE_COUNT.to_string()),
        ("elapsed_us", elapsed.as_micros().to_string()),
        ("commands", canvas.commands().len().to_string()),
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

fn verification_cases() -> [VerificationCase; 5] {
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
            name: "panic_recovery",
            run: verify_panic_recovery,
        },
        VerificationCase {
            name: "stress_1024",
            run: verify_stress_1024,
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

    let scene_list = scenes();
    if cli.screenshot_all {
        run_demo_app_with_screenshot(
            "Update Pipeline Verification",
            "update-app",
            scene_list,
            true,
        )
        .expect("run update-app screenshots");
    } else if let Some(scenario) = cli.screenshot {
        let index = scenario
            .parse::<usize>()
            .ok()
            .or_else(|| scene_list.iter().position(|(name, _)| *name == scenario))
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            });
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
