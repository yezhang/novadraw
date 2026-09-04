use novadraw::{
    EventDispatcher, FigureTree, FocusEventKind, InteractionState, Key, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind, PendingMutations, SceneDispatchContext, UpdateManager,
};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};
use novadraw_demo_scenes::event::{ProbeEvent, probe_scene};

fn with_context(
    graph: &mut FigureTree,
    interaction: &mut InteractionState,
    manager: &mut UpdateManager,
    pending: &mut PendingMutations,
    action: impl FnOnce(&mut EventDispatcher, &mut SceneDispatchContext<'_>),
) {
    let mut dispatcher = EventDispatcher;
    let mut context = SceneDispatchContext::new(graph, interaction, manager, pending);
    action(&mut dispatcher, &mut context);
}

fn verify_pointer_capture() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = UpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 300.0, 220.0, MouseButton::Left);
            dispatcher.dispatch_mouse_moved(ctx, 700.0, 500.0);
            dispatcher.dispatch_mouse_released(ctx, 700.0, 500.0, MouseButton::Left);
        },
    );
    let events = &state.lock().unwrap().events;
    assert_event_order(
        events,
        &[
            MouseEventKind::Entered,
            MouseEventKind::Pressed,
            MouseEventKind::Exited,
            MouseEventKind::Dragged,
            MouseEventKind::Released,
        ],
    )?;
    if interaction.captured().is_some() {
        return Err("capture was not released".to_string());
    }
    Ok(metrics([("events", events.len().to_string())]))
}

fn verify_focus_keyboard() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = UpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 300.0, 220.0, MouseButton::Left);
            dispatcher.dispatch_key_pressed(
                ctx,
                Key::Character('a'),
                KeyModifiers {
                    control: true,
                    ..KeyModifiers::default()
                },
            );
            dispatcher.dispatch_key_released(ctx, Key::Character('a'), KeyModifiers::default());
            dispatcher.release_focus(ctx);
        },
    );
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
        format!("{:?}", interaction.focus_owner()),
    )]))
}

fn verify_wheel_hover_double() -> Result<VerificationMetrics, String> {
    let (mut graph, state) = probe_scene(false);
    let mut interaction = InteractionState::default();
    let mut manager = UpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_hover(ctx, 300.0, 220.0);
            dispatcher.dispatch_mouse_wheel(ctx, 300.0, 220.0, 1.0, -2.0);
            dispatcher.dispatch_mouse_double_clicked(ctx, 300.0, 220.0, MouseButton::Left);
        },
    );
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
    let (mut graph, state) = probe_scene(true);
    let mut interaction = InteractionState::default();
    let mut manager = UpdateManager::new();
    let mut pending = PendingMutations::new();
    with_context(
        &mut graph,
        &mut interaction,
        &mut manager,
        &mut pending,
        |dispatcher, ctx| {
            dispatcher.dispatch_mouse_pressed(ctx, 160.0, 150.0, MouseButton::Left);
        },
    );
    let events = &state.lock().unwrap().events;
    if !events.contains(&ProbeEvent::Mouse(MouseEventKind::Pressed, 20.0, 20.0)) {
        return Err(format!("target-domain point was not reduced: {events:?}"));
    }
    Ok(metrics([
        ("entry_x", "160".to_string()),
        ("target_x", "20".to_string()),
    ]))
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

fn verification_cases() -> [VerificationCase; 4] {
    [
        VerificationCase {
            name: "pointer_capture",
            run: verify_pointer_capture,
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

    let suite = novadraw_demo_scenes::event::suite();
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
