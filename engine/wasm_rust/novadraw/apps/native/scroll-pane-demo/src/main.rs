use novadraw::{
    EventDispatcher, GesturePhase, GestureSessionId, InteractionState, KeyModifiers,
    PendingMutations, Rectangle, RectangleFigure, SceneDispatchContext, ScrollDeltaKind,
    UpdateManager, WheelEvent, ZoomEvent, ZoomManager,
};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};
use novadraw_demo_scenes::scroll_pane::{
    DEMO_SCALE, LARGE_CONTENT_HEIGHT, LARGE_CONTENT_WIDTH, PANE_HEIGHT, PANE_WIDTH, PANE_X, PANE_Y,
    base_scene,
};

fn verify_auto_visibility() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let mut update_manager = UpdateManager::new();
    pane.set_contents(
        &mut graph,
        &mut update_manager,
        Box::new(RectangleFigure::new(
            0.0,
            0.0,
            LARGE_CONTENT_WIDTH,
            LARGE_CONTENT_HEIGHT,
        )),
    )
    .map_err(|error| error.to_string())?;
    graph.revalidate(pane.pane_id());
    if !graph.is_visible(pane.horizontal_scroll_bar())
        || !graph.is_visible(pane.vertical_scroll_bar())
    {
        return Err("automatic policy did not expose both scrollbars".to_string());
    }
    Ok(metrics([
        (
            "horizontal_extent",
            pane.viewport().horizontal_range().extent.to_string(),
        ),
        (
            "vertical_extent",
            pane.viewport().vertical_range().extent.to_string(),
        ),
    ]))
}

fn verify_wheel_scroll() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let mut update_manager = UpdateManager::new();
    pane.set_contents(
        &mut graph,
        &mut update_manager,
        Box::new(RectangleFigure::new(
            0.0,
            0.0,
            LARGE_CONTENT_WIDTH,
            LARGE_CONTENT_HEIGHT,
        )),
    )
    .map_err(|error| error.to_string())?;
    graph.revalidate(pane.pane_id());
    let mut interaction = InteractionState::default();
    let mut pending = PendingMutations::new();
    let mut dispatcher = EventDispatcher;
    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_mouse_wheel(&mut context, PANE_X + 20.0, PANE_Y + 20.0, 0.0, -1.0);
    }
    let location = pane.viewport().view_location();
    if location.y() <= 0.0 {
        return Err("wheel did not change vertical view location".to_string());
    }
    Ok(metrics([("view_y", location.y().to_string())]))
}

fn verify_scale_chain() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, 400.0, 300.0),
        )
        .map_err(|error| error.to_string())?;
    let mut update_manager = UpdateManager::new();
    let child = graph.add_child_to(
        scalable.block_id(),
        Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 20.0)),
    );
    graph.revalidate(pane.pane_id());
    ZoomManager::new(scalable.clone(), pane.viewport().clone())
        .set_zoom(&mut graph, &mut update_manager, DEMO_SCALE)
        .map_err(|error| error.to_string())?;
    pane.viewport()
        .set_view_location(&mut graph, &mut update_manager, 0.0, 0.0)
        .map_err(|error| error.to_string())?;
    let mut point = novadraw::Point::new(0.0, 0.0);
    graph.translate_to_absolute_mut(child, &mut point);
    let expected_x = PANE_X + 20.0 * DEMO_SCALE;
    let expected_y = PANE_Y + 30.0 * DEMO_SCALE;
    if point != novadraw::Point::new(expected_x, expected_y) {
        return Err(format!("unexpected scaled point: {point:?}"));
    }
    Ok(metrics([
        ("absolute_x", point.x().to_string()),
        ("absolute_y", point.y().to_string()),
    ]))
}

fn verify_pinch_anchor() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, LARGE_CONTENT_WIDTH, LARGE_CONTENT_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let child = graph.add_child_to(
        scalable.block_id(),
        Box::new(RectangleFigure::new(
            0.0,
            0.0,
            LARGE_CONTENT_WIDTH,
            LARGE_CONTENT_HEIGHT,
        )),
    );
    graph.revalidate(pane.pane_id());
    let mut interaction = InteractionState::default();
    let mut update_manager = UpdateManager::new();
    let mut pending = PendingMutations::new();
    let mut dispatcher = EventDispatcher;
    let anchor = novadraw::Point::new(PANE_X + 50.0, PANE_Y + 40.0);
    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_zoom(
            &mut context,
            ZoomEvent::new(
                anchor.x(),
                anchor.y(),
                DEMO_SCALE,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }
    let mut content_point = novadraw::Point::new(50.0, 40.0);
    graph.translate_to_absolute_mut(child, &mut content_point);
    if content_point != anchor {
        return Err(format!(
            "pinch anchor moved from {anchor:?} to {content_point:?}"
        ));
    }
    let expected_width = LARGE_CONTENT_WIDTH * DEMO_SCALE;
    let expected_height = LARGE_CONTENT_HEIGHT * DEMO_SCALE;
    if pane.viewport().horizontal_range().maximum != expected_width
        || pane.viewport().vertical_range().maximum != expected_height
    {
        return Err("pinch did not synchronize the scaled scroll range".to_string());
    }
    {
        let mut context = SceneDispatchContext::new(
            &mut graph,
            &mut interaction,
            &mut update_manager,
            &mut pending,
        );
        dispatcher.dispatch_scroll(
            &mut context,
            WheelEvent::with_details(
                anchor.x(),
                anchor.y(),
                10_000.0,
                10_000.0,
                ScrollDeltaKind::LogicalPixels,
                GesturePhase::Impulse,
                KeyModifiers::default(),
                GestureSessionId::IMPULSE,
            ),
        );
    }
    if pane.viewport().view_location() != novadraw::Point::new(0.0, 0.0) {
        return Err("touchpad pan could not reach the scaled canvas origin".to_string());
    }
    let shrunk_scale = 0.5;
    let expanded_scale = 2.0;
    for target_scale in [shrunk_scale, expanded_scale] {
        {
            let mut context = SceneDispatchContext::new(
                &mut graph,
                &mut interaction,
                &mut update_manager,
                &mut pending,
            );
            dispatcher.dispatch_zoom(
                &mut context,
                ZoomEvent::new(
                    anchor.x(),
                    anchor.y(),
                    target_scale / scalable.scale(),
                    GesturePhase::Impulse,
                    KeyModifiers::default(),
                    GestureSessionId::IMPULSE,
                ),
            );
        }
        let _ = graph.perform_update(&mut update_manager);
    }
    let expected_width = LARGE_CONTENT_WIDTH * expanded_scale;
    let expected_height = LARGE_CONTENT_HEIGHT * expanded_scale;
    if pane.viewport().horizontal_range().maximum != expected_width
        || pane.viewport().vertical_range().maximum != expected_height
    {
        return Err("zoom roundtrip corrupted the unscaled preferred extent".to_string());
    }
    Ok(metrics([
        ("scale", scalable.scale().to_string()),
        ("range_width", expected_width.to_string()),
        ("range_height", expected_height.to_string()),
        (
            "view_location",
            format!("{:?}", pane.viewport().view_location()),
        ),
    ]))
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
            name: "auto_visibility",
            run: verify_auto_visibility,
        },
        VerificationCase {
            name: "wheel_scroll",
            run: verify_wheel_scroll,
        },
        VerificationCase {
            name: "scale_chain",
            run: verify_scale_chain,
        },
        VerificationCase {
            name: "pinch_anchor",
            run: verify_pinch_anchor,
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
            "scroll-pane-demo",
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

    let suite = novadraw_demo_scenes::scroll_pane::suite();
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
            "M8 Scroll Pane Verification",
            "scroll-pane-demo",
            scene_list,
            true,
        )
        .expect("run scroll-pane screenshots");
    } else if let Some(index) = screenshot_index {
        run_demo_app_with_scene_screenshot(
            "M8 Scroll Pane Verification",
            "scroll-pane-demo",
            scene_list,
            index,
        )
        .expect("run scroll-pane screenshot");
    } else {
        run_demo_app(
            "M8 Scroll Pane Verification",
            "scroll-pane-demo",
            scene_list,
        )
        .expect("run scroll-pane demo");
    }
}
