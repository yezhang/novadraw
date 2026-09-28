use std::sync::Arc;

use novadraw::container::{MouseLocationZoomScrollPolicy, ZoomManager};
use novadraw::event::{
    GesturePhase, GestureSessionId, KeyModifiers, ScrollDeltaKind, WheelEvent, ZoomEvent,
};
use novadraw::{Point, Rectangle, RectangleFigure, Runtime};
use novadraw_apps::{
    VerificationCase, VerificationCli, VerificationMetrics, run_demo_app,
    run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot, run_verification,
};
use novadraw_demo_scenes::freeform::{
    CONTENT_MAX_X, CONTENT_MAX_Y, CONTENT_MIN_X, CONTENT_MIN_Y, build_demo as build_freeform_demo,
};
use novadraw_demo_scenes::scroll_pane::{
    DEMO_SCALE, LARGE_CONTENT_HEIGHT, LARGE_CONTENT_WIDTH, PANE_HEIGHT, PANE_WIDTH, PANE_X, PANE_Y,
    base_scene,
};

fn verify_auto_visibility() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    graph
        .builder()
        .set_scroll_pane_contents(
            &pane,
            Box::new(RectangleFigure::new(
                0.0,
                0.0,
                LARGE_CONTENT_WIDTH,
                LARGE_CONTENT_HEIGHT,
            )),
        )
        .map_err(|error| error.to_string())?;
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    let runtime = Runtime::new(graph);
    if !runtime.tree().is_visible(pane.horizontal_scroll_bar())
        || !runtime.tree().is_visible(pane.vertical_scroll_bar())
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
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    graph
        .builder()
        .set_scroll_pane_contents(
            &pane,
            Box::new(RectangleFigure::new(
                0.0,
                0.0,
                LARGE_CONTENT_WIDTH,
                LARGE_CONTENT_HEIGHT,
            )),
        )
        .map_err(|error| error.to_string())?;
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(graph);
    runtime.dispatch_scroll(WheelEvent::new(PANE_X + 20.0, PANE_Y + 20.0, 0.0, -1.0));
    let location = pane.viewport().view_location();
    if location.y() <= 0.0 {
        return Err("wheel did not change vertical view location".to_string());
    }
    Ok(metrics([("view_y", location.y().to_string())]))
}

fn verify_scale_chain() -> Result<VerificationMetrics, String> {
    let (mut graph, root) = base_scene();
    let pane = graph
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let scalable = graph
        .builder()
        .add_scalable_layered_pane_to(
            pane.viewport().figure_id(),
            Rectangle::new(0.0, 0.0, 400.0, 300.0),
        )
        .map_err(|error| error.to_string())?;
    let child = graph
        .builder()
        .add_child(
            scalable.figure_id(),
            Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 20.0)),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    let zoom = ZoomManager::new(scalable.clone(), pane.viewport().clone());
    let mut runtime = Runtime::new(graph);
    runtime
        .zoom(&zoom)
        .map_err(|error| error.to_string())?
        .set_zoom_at(DEMO_SCALE, None)
        .map_err(|error| error.to_string())?;
    runtime
        .viewport(pane.viewport().figure_id())
        .map_err(|error| error.to_string())?
        .set_view_location(0.0, 0.0)
        .map_err(|error| error.to_string())?;
    let point = runtime
        .tree()
        .local_to_surface_transform(child)
        .ok_or("missing child transform")?
        .transform_point(novadraw::Point::new(0.0, 0.0));
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
        .builder()
        .add_scroll_pane_to(
            root,
            Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let scalable = graph
        .builder()
        .add_scalable_layered_pane_to(
            pane.viewport().figure_id(),
            Rectangle::new(0.0, 0.0, LARGE_CONTENT_WIDTH, LARGE_CONTENT_HEIGHT),
        )
        .map_err(|error| error.to_string())?;
    let child = graph
        .builder()
        .add_child(
            scalable.figure_id(),
            Box::new(RectangleFigure::new(
                0.0,
                0.0,
                LARGE_CONTENT_WIDTH,
                LARGE_CONTENT_HEIGHT,
            )),
        )
        .expect("valid FigureTree construction");
    graph
        .builder()
        .validate_subtree(pane.pane_id())
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(graph);
    let anchor = novadraw::Point::new(PANE_X + 50.0, PANE_Y + 40.0);
    runtime.dispatch_zoom(ZoomEvent::new(
        anchor.x(),
        anchor.y(),
        DEMO_SCALE,
        GesturePhase::Impulse,
        KeyModifiers::default(),
        GestureSessionId::IMPULSE,
    ));
    let content_point = runtime
        .tree()
        .local_to_surface_transform(child)
        .ok_or("missing child transform")?
        .transform_point(novadraw::Point::new(50.0, 40.0));
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
    runtime.dispatch_scroll(WheelEvent::with_details(
        anchor.x(),
        anchor.y(),
        10_000.0,
        10_000.0,
        ScrollDeltaKind::LogicalPixels,
        GesturePhase::Impulse,
        KeyModifiers::default(),
        GestureSessionId::IMPULSE,
    ));
    if pane.viewport().view_location() != novadraw::Point::new(0.0, 0.0) {
        return Err("touchpad pan could not reach the scaled canvas origin".to_string());
    }
    let shrunk_scale = 0.5;
    let expanded_scale = 2.0;
    for target_scale in [shrunk_scale, expanded_scale] {
        runtime.dispatch_zoom(ZoomEvent::new(
            anchor.x(),
            anchor.y(),
            target_scale / scalable.scale(),
            GesturePhase::Impulse,
            KeyModifiers::default(),
            GestureSessionId::IMPULSE,
        ));
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

fn verify_freeform_range() -> Result<VerificationMetrics, String> {
    let demo = build_freeform_demo(1.0, (0.0, 0.0));
    let extent = demo
        .runtime
        .tree()
        .freeform_extent(demo.scalable.figure_id())
        .map_err(|error| error.to_string())?;
    let horizontal = demo.pane.viewport().horizontal_range();
    let vertical = demo.pane.viewport().vertical_range();
    let expected = Rectangle::new(
        CONTENT_MIN_X,
        CONTENT_MIN_Y,
        CONTENT_MAX_X - CONTENT_MIN_X,
        CONTENT_MAX_Y - CONTENT_MIN_Y,
    );
    if extent != expected
        || horizontal.minimum != CONTENT_MIN_X
        || horizontal.maximum != CONTENT_MAX_X
        || vertical.minimum != CONTENT_MIN_Y
        || vertical.maximum != CONTENT_MAX_Y
    {
        return Err(format!(
            "unexpected freeform extent/range: extent={extent:?}, horizontal={horizontal:?}, vertical={vertical:?}"
        ));
    }
    Ok(metrics([
        ("extent", format!("{extent:?}")),
        ("horizontal", format!("{horizontal:?}")),
        ("vertical", format!("{vertical:?}")),
    ]))
}

fn verify_freeform_layer_hit_order() -> Result<VerificationMetrics, String> {
    let demo = build_freeform_demo(1.0, (0.0, 0.0));
    let overlap = Point::new(PANE_X + 150.0, PANE_Y + 120.0);
    let target = demo
        .runtime
        .tree()
        .hit_test_simple((overlap.x(), overlap.y()));
    if target != Some(demo.upper_overlap) {
        return Err(format!(
            "top layer did not win reverse-Z hit test: target={target:?}"
        ));
    }
    let empty = Point::new(PANE_X + 20.0, PANE_Y + 20.0);
    let empty_target = demo.runtime.tree().hit_test_simple((empty.x(), empty.y()));
    if empty_target == Some(demo.content_layer) || empty_target == Some(demo.overlay_layer) {
        return Err("transparent layer became a hit target".to_string());
    }
    Ok(metrics([
        ("overlap_target", format!("{target:?}")),
        ("upper_overlap", format!("{:?}", demo.upper_overlap)),
        ("empty_target", format!("{empty_target:?}")),
    ]))
}

fn verify_freeform_scroll_and_zoom() -> Result<VerificationMetrics, String> {
    let mut demo = build_freeform_demo(1.0, (0.0, 0.0));
    let viewport = demo.pane.viewport().clone();
    demo.runtime
        .viewport(viewport.figure_id())
        .map_err(|error| error.to_string())?
        .set_view_location(CONTENT_MIN_X, CONTENT_MIN_Y)
        .map_err(|error| error.to_string())?;
    if viewport.view_location() != Point::new(CONTENT_MIN_X, CONTENT_MIN_Y) {
        return Err("freeform viewport could not reach negative range edge".to_string());
    }
    demo.runtime
        .viewport(viewport.figure_id())
        .map_err(|error| error.to_string())?
        .set_view_location(f64::MAX, f64::MAX)
        .map_err(|error| error.to_string())?;
    let horizontal = viewport.horizontal_range();
    let vertical = viewport.vertical_range();
    let expected_max = Point::new(
        horizontal.maximum - horizontal.extent,
        vertical.maximum - vertical.extent,
    );
    if viewport.view_location() != expected_max {
        return Err("freeform viewport did not clamp at positive range edge".to_string());
    }
    demo.runtime
        .viewport(viewport.figure_id())
        .map_err(|error| error.to_string())?
        .set_view_location(0.0, 0.0)
        .map_err(|error| error.to_string())?;
    let mut zoom = ZoomManager::new(demo.scalable, viewport.clone());
    zoom.set_scroll_policy(Arc::new(MouseLocationZoomScrollPolicy));
    demo.runtime
        .zoom(&zoom)
        .map_err(|error| error.to_string())?
        .set_zoom_at(2.0, Some(Point::new(60.0, 40.0)))
        .map_err(|error| error.to_string())?;
    if viewport.view_location() != Point::new(30.0, 20.0) {
        return Err("freeform anchor zoom changed the anchored content point".to_string());
    }
    if viewport.horizontal_range().maximum != CONTENT_MAX_X
        || viewport.vertical_range().maximum != CONTENT_MAX_Y
    {
        return Err("freeform zoom changed content-domain range bounds".to_string());
    }
    Ok(metrics([
        ("positive_edge", format!("{expected_max:?}")),
        ("zoom_origin", format!("{:?}", viewport.view_location())),
        ("zoom", zoom.zoom().to_string()),
    ]))
}

fn metrics<const N: usize>(entries: [(&str, String); N]) -> VerificationMetrics {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn verification_cases() -> [VerificationCase; 7] {
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
        VerificationCase {
            name: "freeform_range",
            run: verify_freeform_range,
        },
        VerificationCase {
            name: "freeform_layer_hit_order",
            run: verify_freeform_layer_hit_order,
        },
        VerificationCase {
            name: "freeform_scroll_and_zoom",
            run: verify_freeform_scroll_and_zoom,
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

    let mut suite = novadraw_demo_scenes::scroll_pane::suite();
    suite
        .scenes
        .extend(novadraw_demo_scenes::freeform::suite().scenes);
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
