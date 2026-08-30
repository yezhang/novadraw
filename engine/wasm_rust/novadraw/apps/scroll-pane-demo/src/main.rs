use novadraw::{
    BasicEventDispatcher, Color, EventDispatcher, FigureGraph, GesturePhase, GestureSessionId,
    KeyModifiers, PendingMutations, Rectangle, RectangleFigure, SceneDispatchContext,
    SceneUpdateManager, ScrollBarVisibility, ScrollDeltaKind, WheelEvent, ZoomEvent, ZoomManager,
};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const PANE_X: f64 = 120.0;
const PANE_Y: f64 = 90.0;
const PANE_WIDTH: f64 = 480.0;
const PANE_HEIGHT: f64 = 340.0;
const LARGE_CONTENT_WIDTH: f64 = 860.0;
const LARGE_CONTENT_HEIGHT: f64 = 640.0;
const SMALL_CONTENT_WIDTH: f64 = 240.0;
const SMALL_CONTENT_HEIGHT: f64 = 160.0;
const GRID_COLUMNS: usize = 8;
const GRID_ROWS: usize = 6;
const TILE_WIDTH: f64 = 82.0;
const TILE_HEIGHT: f64 = 68.0;
const TILE_GAP: f64 = 12.0;
const INITIAL_SCROLL_X: f64 = 90.0;
const INITIAL_SCROLL_Y: f64 = 70.0;
const DEMO_SCALE: f64 = 1.5;

type SceneEntry = (&'static str, Box<dyn FnMut() -> FigureGraph>);

fn color(hex: &str) -> Color {
    Color::hex(hex)
}

fn base_scene() -> (FigureGraph, novadraw::BlockId) {
    let mut graph = FigureGraph::new();
    let root = graph.set_contents(Box::new(RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        color("#eeeeee"),
    )));
    (graph, root)
}

fn add_grid(graph: &mut FigureGraph, parent: novadraw::BlockId) {
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            let fill = match (row + column) % 4 {
                0 => color("#2f80ed"),
                1 => color("#27ae60"),
                2 => color("#f2994a"),
                _ => color("#9b51e0"),
            };
            graph.add_child_to(
                parent,
                Box::new(RectangleFigure::new_with_color(
                    TILE_GAP + column as f64 * (TILE_WIDTH + TILE_GAP),
                    TILE_GAP + row as f64 * (TILE_HEIGHT + TILE_GAP),
                    TILE_WIDTH,
                    TILE_HEIGHT,
                    fill,
                )),
            );
        }
    }
}

fn scene_with_policy(
    content_width: f64,
    content_height: f64,
    horizontal: ScrollBarVisibility,
    vertical: ScrollBarVisibility,
    initial_scroll: Option<(f64, f64)>,
) -> FigureGraph {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .expect("attach scroll pane");
    let mut update_manager = SceneUpdateManager::new();
    pane.set_scroll_bar_visibility(&mut graph, &mut update_manager, horizontal, vertical)
        .expect("set scrollbar visibility");
    let contents = pane
        .set_contents(
            &mut graph,
            &mut update_manager,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                content_width,
                content_height,
                Color::WHITE,
            )),
        )
        .expect("set scroll pane contents");
    add_grid(&mut graph, contents);
    graph.revalidate(pane.pane_id());
    if let Some((x, y)) = initial_scroll {
        pane.scroll_to(&mut graph, &mut update_manager, x, y)
            .expect("set initial scroll");
    }
    graph
}

fn automatic_scene() -> FigureGraph {
    scene_with_policy(
        LARGE_CONTENT_WIDTH,
        LARGE_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        None,
    )
}

fn scrolled_scene() -> FigureGraph {
    scene_with_policy(
        LARGE_CONTENT_WIDTH,
        LARGE_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        Some((INITIAL_SCROLL_X, INITIAL_SCROLL_Y)),
    )
}

fn hidden_bars_scene() -> FigureGraph {
    scene_with_policy(
        SMALL_CONTENT_WIDTH,
        SMALL_CONTENT_HEIGHT,
        ScrollBarVisibility::Automatic,
        ScrollBarVisibility::Automatic,
        None,
    )
}

fn scalable_scene() -> FigureGraph {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .expect("attach scroll pane");
    let scalable = graph
        .add_scalable_layered_pane_to(
            pane.viewport().block_id(),
            Rectangle::new(0.0, 0.0, LARGE_CONTENT_WIDTH, LARGE_CONTENT_HEIGHT),
        )
        .expect("attach scalable pane");
    let mut update_manager = SceneUpdateManager::new();
    add_grid(&mut graph, scalable.block_id());
    graph.revalidate(pane.pane_id());
    ZoomManager::new(scalable, pane.viewport().clone())
        .set_zoom(&mut graph, &mut update_manager, DEMO_SCALE)
        .expect("set demo zoom");
    pane.viewport()
        .set_view_location(&mut graph, &mut update_manager, 0.0, 0.0)
        .expect("reset demo view location");
    graph
}

fn scenes() -> Vec<SceneEntry> {
    vec![
        ("automatic_scrollbars", Box::new(automatic_scene)),
        ("scrolled_content", Box::new(scrolled_scene)),
        ("automatic_hidden", Box::new(hidden_bars_scene)),
        ("scalable_content", Box::new(scalable_scene)),
    ]
}

fn verify_auto_visibility() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let mut update_manager = SceneUpdateManager::new();
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
    let mut update_manager = SceneUpdateManager::new();
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
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    {
        let mut context = SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
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
    let mut update_manager = SceneUpdateManager::new();
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
    let mut point = novadraw::Point::new(20.0, 30.0);
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
    let mut update_manager = SceneUpdateManager::new();
    let mut pending = PendingMutations::new();
    let mut dispatcher = BasicEventDispatcher;
    let anchor = novadraw::Point::new(PANE_X + 50.0, PANE_Y + 40.0);
    {
        let mut context = SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
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
        let mut context = SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
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
            let mut context =
                SceneDispatchContext::new(&mut graph, &mut update_manager, &mut pending);
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

    let scene_list = scenes();
    if cli.screenshot_all {
        run_demo_app_with_screenshot(
            "M8 Scroll Pane Verification",
            "scroll-pane-demo",
            scene_list,
            true,
        )
        .expect("run scroll-pane screenshots");
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
