use novadraw::event::{
    FocusEventKind, Key, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind, WheelEvent,
};
use novadraw::render::command::RenderCommandKind;
use novadraw::render::{BackendCapabilities, RenderDisposition, SurfaceInfo};
use novadraw::runtime::Runtime;
use novadraw_example_scenes::event::{ProbeEvent, hover_probe_scene, probe_scene};
use novadraw_example_support::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};

fn verify_pointer_capture() -> Result<VerificationMetrics, String> {
    let (graph, state) = probe_scene(false);
    let mut runtime = Runtime::new(graph);
    runtime
        .input()
        .dispatch_mouse_pressed(300.0, 220.0, MouseButton::Left);
    runtime.input().dispatch_mouse_moved(700.0, 500.0);
    runtime
        .input()
        .dispatch_mouse_released(700.0, 500.0, MouseButton::Left);
    let events = &state.lock().unwrap().events;
    assert_event_order(
        events,
        &[
            MouseEventKind::Entered,
            MouseEventKind::Pressed,
            MouseEventKind::Dragged,
            MouseEventKind::Released,
            MouseEventKind::Exited,
        ],
    )?;
    if runtime.input().state().captured().is_some() {
        return Err("capture was not released".to_string());
    }
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_pointer_leave_cleanup() -> Result<VerificationMetrics, String> {
    let (graph, state) = probe_scene(false);
    let mut runtime = Runtime::new(graph);
    runtime
        .input()
        .dispatch_mouse_pressed(300.0, 220.0, MouseButton::Left);
    runtime.input().pointer_exited();
    runtime.input().dispatch_mouse_moved(700.0, 500.0);
    let events = &state.lock().unwrap().events;
    if runtime.input().state().captured().is_some() {
        return Err("pointer leave retained capture".to_string());
    }
    if events
        .iter()
        .any(|event| matches!(event, ProbeEvent::Mouse(MouseEventKind::Dragged, ..)))
    {
        return Err(format!("pointer leave allowed a stale drag: {events:?}"));
    }
    assert_event_order(
        events,
        &[
            MouseEventKind::Entered,
            MouseEventKind::Pressed,
            MouseEventKind::Exited,
        ],
    )?;
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_focus_keyboard() -> Result<VerificationMetrics, String> {
    let (graph, state) = probe_scene(false);
    let mut runtime = Runtime::new(graph);
    runtime
        .input()
        .dispatch_mouse_pressed(300.0, 220.0, MouseButton::Left);
    runtime.input().dispatch_key_pressed(
        Key::Character('a'),
        KeyModifiers {
            control: true,
            ..KeyModifiers::default()
        },
    );
    runtime
        .input()
        .dispatch_key_released(Key::Character('a'), KeyModifiers::default());
    runtime.focus().release();
    let events = &state.lock().unwrap().events;
    for expected in [
        ProbeEvent::Focus(FocusEventKind::Gained),
        ProbeEvent::Key(
            KeyEventKind::Pressed,
            Key::Character('a'),
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        ),
        ProbeEvent::Key(
            KeyEventKind::Released,
            Key::Character('a'),
            KeyModifiers::default(),
        ),
        ProbeEvent::Focus(FocusEventKind::Lost),
    ] {
        if !events.contains(&expected) {
            return Err(format!("missing event: {expected:?}"));
        }
    }
    Ok(metrics([(
        "focus_owner",
        format!("{:?}", runtime.input().state().focus_owner()),
    )]))
}

fn verify_wheel_hover_double() -> Result<VerificationMetrics, String> {
    let (graph, state) = probe_scene(false);
    let mut runtime = Runtime::new(graph);
    runtime.input().dispatch_mouse_hover(300.0, 220.0);
    runtime
        .input()
        .dispatch_scroll(WheelEvent::new(300.0, 220.0, 1.0, -2.0));
    runtime
        .input()
        .dispatch_mouse_double_clicked(300.0, 220.0, MouseButton::Left);
    let events = &state.lock().unwrap().events;
    if !events
        .iter()
        .any(|event| matches!(event, ProbeEvent::Mouse(MouseEventKind::Hover, ..)))
        || !events
            .iter()
            .any(|event| matches!(event, ProbeEvent::Wheel(_, _, 1.0, -2.0)))
        || !events
            .iter()
            .any(|event| matches!(event, ProbeEvent::Mouse(MouseEventKind::DoubleClicked, ..)))
    {
        return Err("hover/wheel/double-click sequence incomplete".to_string());
    }
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_coordinate_reduction() -> Result<VerificationMetrics, String> {
    let (graph, state) = probe_scene(true);
    let mut runtime = Runtime::new(graph);
    runtime
        .input()
        .dispatch_mouse_pressed(160.0, 150.0, MouseButton::Left);
    let events = &state.lock().unwrap().events;
    if !events.contains(&ProbeEvent::Mouse(MouseEventKind::Pressed, 20.0, 20.0)) {
        return Err(format!("target-domain point was not reduced: {events:?}"));
    }
    Ok(metrics([
        ("entry_x", "160".to_string()),
        ("target_x", "20".to_string()),
    ]))
}

fn verify_hover_highlight() -> Result<VerificationMetrics, String> {
    let (graph, state) = hover_probe_scene();
    let mut runtime = Runtime::new(graph);
    let surface = SurfaceInfo {
        logical_width: 800.0,
        logical_height: 600.0,
        pixel_width: 800,
        pixel_height: 600,
        scale_factor: 1.0,
    };
    let check_border = |runtime: &mut Runtime, expected: bool| -> Result<(), String> {
        let submission = runtime
            .frames()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .map_err(|state| format!("hover repaint was not ready: {state:?}"))?;
        let has_border = submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::StrokeRect { .. }));
        let _ = runtime
            .frames()
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .map_err(|error| error.to_string())?;
        if has_border != expected {
            return Err(format!(
                "hover border mismatch: expected {expected}, got {has_border}"
            ));
        }
        Ok(())
    };

    check_border(&mut runtime, false)?;
    runtime.input().dispatch_mouse_moved(300.0, 220.0);
    check_border(&mut runtime, true)?;
    runtime.input().dispatch_mouse_moved(700.0, 500.0);
    check_border(&mut runtime, false)?;
    runtime.input().dispatch_mouse_moved(300.0, 220.0);
    check_border(&mut runtime, true)?;
    runtime.input().pointer_exited();
    check_border(&mut runtime, false)?;
    let events = &state.lock().unwrap().events;
    assert_event_order(
        events,
        &[
            MouseEventKind::Entered,
            MouseEventKind::Exited,
            MouseEventKind::Entered,
            MouseEventKind::Exited,
        ],
    )?;
    Ok(metrics([("border_transitions", "4".to_string())]))
}

fn assert_event_order(events: &[ProbeEvent], kinds: &[MouseEventKind]) -> Result<(), String> {
    let actual = events
        .iter()
        .filter_map(|event| match event {
            ProbeEvent::Mouse(kind, _, _) => Some(*kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut cursor = 0;
    for expected in kinds {
        let Some(offset) = actual[cursor..].iter().position(|kind| kind == expected) else {
            return Err(format!(
                "missing ordered mouse event {expected:?}: {actual:?}"
            ));
        };
        cursor += offset + 1;
    }
    Ok(())
}

fn metrics<const N: usize>(entries: [(&str, String); N]) -> VerificationMetrics {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn verification_cases() -> [VerificationCase; 6] {
    [
        VerificationCase {
            name: "pointer_capture",
            run: verify_pointer_capture,
        },
        VerificationCase {
            name: "pointer_leave_cleanup",
            run: verify_pointer_leave_cleanup,
        },
        VerificationCase {
            name: "focus_keyboard",
            run: verify_focus_keyboard,
        },
        VerificationCase {
            name: "wheel_hover_double",
            run: verify_wheel_hover_double,
        },
        VerificationCase {
            name: "coordinate_root",
            run: verify_coordinate_reduction,
        },
        VerificationCase {
            name: "hover_highlight",
            run: verify_hover_highlight,
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
            "event-app",
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

    let suite = novadraw_example_scenes::event::suite();
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
        run_demo_app_with_screenshot("Event Pipeline Verification", "event-app", scene_list, true)
            .expect("run event-app screenshots");
    } else if let Some(index) = screenshot_index {
        run_demo_app_with_scene_screenshot(
            "Event Pipeline Verification",
            "event-app",
            scene_list,
            index,
        )
        .expect("run event-app screenshot");
    } else {
        run_demo_app("Event Pipeline Verification", "event-app", scene_list)
            .expect("run event-app");
    }
}
