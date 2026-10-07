use novadraw::render::{BackendCapabilities, RenderCommandKind, SurfaceInfo};
use novadraw::{MouseButton, Point, Rectangle};
use novadraw_example_support::{
    VerificationCase, VerificationCli, VerificationMetrics, run_runtime_demo_app,
    run_runtime_demo_app_with_scene_screenshot, run_runtime_demo_app_with_screenshot,
    run_verification,
};

const APP_NAME: &str = "uml-demo";
const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;

fn main() {
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if cli.verify {
        run_verification(
            APP_NAME,
            &[VerificationCase {
                name: "order-domain-extension",
                run: verify_order_domain,
            }],
            cli.scenario.as_deref(),
            cli.report.as_deref(),
        )
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(1);
        });
        return;
    }

    let suite = novadraw_example_scenes::uml::suite();
    let screenshot_index = cli.screenshot.as_deref().map(|scenario| {
        scenario
            .parse::<usize>()
            .ok()
            .or_else(|| {
                suite
                    .scenes
                    .iter()
                    .position(|scene| scene.id == scenario || scene.title == scenario)
            })
            .unwrap_or_else(|| {
                eprintln!("unknown screenshot scenario: {scenario}");
                std::process::exit(2);
            })
    });
    let scenes = suite.into_entries();
    let result = if cli.screenshot_all {
        run_runtime_demo_app_with_screenshot("Novadraw UML", APP_NAME, scenes, true)
    } else if let Some(index) = screenshot_index {
        run_runtime_demo_app_with_scene_screenshot("Novadraw UML", APP_NAME, scenes, index)
    } else {
        run_runtime_demo_app("Novadraw UML", APP_NAME, scenes)
    };
    result.expect("failed to run UML demo");
}

fn verify_order_domain() -> Result<VerificationMetrics, String> {
    let mut example = novadraw_example_scenes::uml::build_example();
    if example.class_figures.len() != 6 {
        return Err(format!(
            "expected 6 class figures, got {}",
            example.class_figures.len()
        ));
    }
    if example.connection_figures.len() != 6 || example.relation_labels.len() != 6 {
        return Err("relation projection is incomplete".to_owned());
    }

    for class in &example.class_figures {
        let node = example
            .runtime
            .tree()
            .node(*class)
            .ok_or_else(|| format!("class {class:?} is detached"))?;
        if node.figure_name() != "ExampleUmlClassFigure" || node.children_count() != 3 {
            return Err(format!(
                "class {class:?} did not preserve the custom compound Figure contract"
            ));
        }
    }

    let mut total_route_points = 0;
    for connection in &example.connection_figures {
        let points = example
            .runtime
            .tree()
            .connection_route_points(*connection)
            .ok_or_else(|| format!("connection {connection:?} has no committed route"))?;
        if points.len() < 2 {
            return Err(format!("connection {connection:?} has a degenerate route"));
        }
        total_route_points += points.len();
    }

    let dragged = example.class_figures[1];
    let before_bounds = example
        .runtime
        .tree()
        .figure_bounds(dragged)
        .ok_or_else(|| "draggable class has no bounds".to_owned())?;
    let before_route = example
        .runtime
        .tree()
        .connection_route_points(example.connection_figures[0])
        .ok_or_else(|| "draggable class relation has no route".to_owned())?
        .clone();
    let start = Point::new(
        before_bounds.x + before_bounds.width / 2.0,
        before_bounds.y + 18.0,
    );
    let press = example
        .runtime
        .dispatch_mouse_pressed(start.x(), start.y(), MouseButton::Left);
    if press.target() != Some(dragged) || press.capture() != Some(dragged) {
        return Err("class press did not establish pointer capture".to_owned());
    }
    let delta = Point::new(48.0, 36.0);
    example
        .runtime
        .dispatch_mouse_moved(start.x() + delta.x(), start.y() + delta.y());
    let after_bounds = example
        .runtime
        .tree()
        .figure_bounds(dragged)
        .ok_or_else(|| "dragged class became detached".to_owned())?;
    let expected_bounds = Rectangle::new(
        before_bounds.x + delta.x(),
        before_bounds.y + delta.y(),
        before_bounds.width,
        before_bounds.height,
    );
    if after_bounds != expected_bounds {
        return Err(format!(
            "drag committed {after_bounds:?}, expected {expected_bounds:?}"
        ));
    }
    let rerouted_connections = example.runtime.dirty_connections().len();
    if rerouted_connections < 4 {
        return Err(format!(
            "drag invalidated only {rerouted_connections} connected relations"
        ));
    }
    let release = example.runtime.dispatch_mouse_released(
        start.x() + delta.x(),
        start.y() + delta.y(),
        MouseButton::Left,
    );
    if !release.is_handled() || release.capture().is_some() {
        return Err("class release did not close pointer capture".to_owned());
    }
    if !example.runtime.take_deferred_mutation_errors().is_empty() {
        return Err("class drag produced deferred Runtime mutation errors".to_owned());
    }

    let submission = example
        .runtime
        .prepare_submission(
            SurfaceInfo {
                logical_width: WIDTH,
                logical_height: HEIGHT,
                pixel_width: WIDTH as u32,
                pixel_height: HEIGHT as u32,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .map_err(|state| format!("UML scene did not produce a render submission: {state:?}"))?;
    if example
        .runtime
        .tree()
        .connection_route_points(example.connection_figures[0])
        == Some(&before_route)
    {
        return Err("connected relation did not reroute after class drag".to_owned());
    }
    let glyph_runs = submission
        .commands
        .iter()
        .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
        .count();
    let polyline_commands = submission
        .commands
        .iter()
        .filter(|command| matches!(command.kind, RenderCommandKind::Polyline { .. }))
        .count();
    if glyph_runs != 41 || polyline_commands < 6 {
        return Err(format!(
            "render projection is incomplete: {glyph_runs} glyph runs, \
             {polyline_commands} polylines"
        ));
    }

    Ok(VerificationMetrics::from([
        (
            "class_figures".to_owned(),
            example.class_figures.len().to_string(),
        ),
        (
            "connection_figures".to_owned(),
            example.connection_figures.len().to_string(),
        ),
        ("glyph_runs".to_owned(), glyph_runs.to_string()),
        (
            "polyline_commands".to_owned(),
            polyline_commands.to_string(),
        ),
        (
            "rerouted_connections".to_owned(),
            rerouted_connections.to_string(),
        ),
        ("route_points".to_owned(), total_route_points.to_string()),
    ]))
}
