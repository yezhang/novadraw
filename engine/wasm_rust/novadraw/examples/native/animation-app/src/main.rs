use novadraw::{
    Affine2D, MonotonicTime, Rectangle,
    animation::{
        AnimationError, AnimationMode, AnimationStart, AnimationState, ConnectionRouteTransition,
    },
};
use novadraw_example_scenes::animation::{
    BOUNDS_DURATION, MECHANISM_DURATION, REPEAT_CYCLES, TEMPORARY_DURATION, build_example,
    build_mechanisms_example,
};
use novadraw_example_support::{
    VerificationCase, VerificationCli, VerificationMetrics, run_runtime_demo_app,
    run_runtime_demo_app_with_scene_screenshot, run_runtime_demo_app_with_screenshot,
    run_verification,
};

const APP_NAME: &str = "animation-app";
const TARGET_SAMPLE_MILLIS: u64 = 600;
const MOTION_SAMPLE_MILLIS: u64 = 700;
const TEMPORARY_SAMPLE_MILLIS: u64 = 900;
const CANCEL_SAMPLE_MILLIS: u64 = 500;
const EPSILON: f64 = 1.0e-6;

fn main() {
    let cli = VerificationCli::parse().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if cli.verify {
        run_verification(
            APP_NAME,
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

    let suite = novadraw_example_scenes::animation::suite();
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
        run_runtime_demo_app_with_screenshot("Novadraw Animation", APP_NAME, scenes, true)
    } else if let Some(index) = screenshot_index {
        run_runtime_demo_app_with_scene_screenshot("Novadraw Animation", APP_NAME, scenes, index)
    } else {
        run_runtime_demo_app("Novadraw Animation", APP_NAME, scenes)
    };
    result.expect("failed to run animation example");
}

fn verification_cases() -> [VerificationCase; 7] {
    [
        VerificationCase {
            name: "target-motion-composition",
            run: verify_target_motion_composition,
        },
        VerificationCase {
            name: "motion-substitution",
            run: verify_motion_substitution,
        },
        VerificationCase {
            name: "source-presentation-separation",
            run: verify_source_presentation_separation,
        },
        VerificationCase {
            name: "temporary-lifecycle-and-cancel",
            run: verify_temporary_lifecycle_and_cancel,
        },
        VerificationCase {
            name: "trigger-and-layout-transaction",
            run: verify_trigger_and_layout_transaction,
        },
        VerificationCase {
            name: "viewport-and-route-transition",
            run: verify_viewport_and_route_transition,
        },
        VerificationCase {
            name: "continuous-effects-and-reduced-motion",
            run: verify_continuous_effects_and_reduced_motion,
        },
    ]
}

fn verify_target_motion_composition() -> Result<VerificationMetrics, String> {
    let mut example = build_example();
    let committed = example
        .target_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure));
    example
        .runtime
        .advance_time(time(TARGET_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;

    let translations = example.target_channels.map(|channels| {
        example
            .runtime
            .animations()
            .value(channels.transform())
            .map(|transform| transform.coeffs()[4])
    });
    let [first, second, third] = translations.map(|value| value.map_err(|error| error.to_string()));
    let first = first?;
    let second = second?;
    let third = third?;
    if !(first > second && second > third && third < 0.0) {
        return Err(format!(
            "stable stagger order was not observable: [{first}, {second}, {third}]"
        ));
    }
    for channels in example.target_channels {
        let opacity = example
            .runtime
            .animations()
            .value(channels.opacity())
            .map_err(|error| error.to_string())?
            .get();
        if !(0.25..1.0).contains(&opacity) {
            return Err(format!(
                "arrival opacity left its expected interval: {opacity}"
            ));
        }
    }
    if example
        .target_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure))
        != committed
    {
        return Err("presentation motion mutated committed Figure bounds".to_string());
    }
    if example
        .runtime
        .animations()
        .state(example.target_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("repeated target composition is not running".to_string());
    }

    Ok(metrics([
        ("target_count", example.target_figures.len().to_string()),
        (
            "staggered_translation_x",
            format!("{first:.2},{second:.2},{third:.2}"),
        ),
    ]))
}

fn verify_motion_substitution() -> Result<VerificationMetrics, String> {
    let mut example = build_example();
    let committed = example
        .motion_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure));
    example
        .runtime
        .advance_time(time(MOTION_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    let values = example.motion_channels.map(|channel| {
        example
            .runtime
            .animations()
            .value(channel)
            .map(|transform| transform.coeffs()[5])
    });
    let [tween, keyframes, spring] = values.map(|value| value.map_err(|error| error.to_string()));
    let tween = tween?;
    let keyframes = keyframes?;
    let spring = spring?;
    if approx_eq(tween, keyframes) || approx_eq(tween, spring) || approx_eq(keyframes, spring) {
        return Err(format!(
            "motion samplers did not diverge at the same time: [{tween}, {keyframes}, {spring}]"
        ));
    }
    if example
        .motion_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure))
        != committed
    {
        return Err("motion substitution mutated committed Figure bounds".to_string());
    }
    if example
        .runtime
        .animations()
        .state(example.motion_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("parallel motion composition is not running".to_string());
    }

    Ok(metrics([(
        "translation_y",
        format!("{tween:.2},{keyframes:.2},{spring:.2}"),
    )]))
}

fn verify_source_presentation_separation() -> Result<VerificationMetrics, String> {
    let mut example = build_example();
    let committed = example
        .runtime
        .tree()
        .figure_bounds(example.bounds_figure)
        .ok_or_else(|| "bounds Figure is detached".to_string())?;
    let initial_presentation = example
        .runtime
        .animations()
        .value(example.bounds_channel)
        .map_err(|error| error.to_string())?;
    if initial_presentation == Affine2D::IDENTITY {
        return Err("bounds transition did not preserve the captured presentation".to_string());
    }

    example
        .runtime
        .advance_time(duration_time(BOUNDS_DURATION))
        .map_err(|error| error.to_string())?;
    if example.runtime.tree().figure_bounds(example.bounds_figure) != Some(committed) {
        return Err("animation changed the committed final bounds".to_string());
    }
    if example
        .runtime
        .animations()
        .value(example.bounds_channel)
        .map_err(|error| error.to_string())?
        != Affine2D::IDENTITY
    {
        return Err("completed transition did not clear its presentation transform".to_string());
    }
    if example
        .runtime
        .animations()
        .state(example.bounds_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Completed
    {
        return Err("bounds transition did not complete deterministically".to_string());
    }

    Ok(metrics([
        ("committed_x", format!("{:.1}", committed.x)),
        ("committed_width", format!("{:.1}", committed.width)),
    ]))
}

fn verify_temporary_lifecycle_and_cancel() -> Result<VerificationMetrics, String> {
    let mut temporary_example = build_example();
    temporary_example
        .runtime
        .advance_time(time(TEMPORARY_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    if temporary_example
        .runtime
        .animations()
        .temporary_visual_count()
        != 1
    {
        return Err("active temporary visual is not Runtime-owned".to_string());
    }
    let temporary_opacity = temporary_example
        .runtime
        .animations()
        .value(temporary_example.temporary.opacity())
        .map_err(|error| error.to_string())?
        .get();
    if temporary_opacity < 0.99 {
        return Err(format!(
            "temporary visual did not sample its independent opacity motion: {temporary_opacity}"
        ));
    }
    let temporary_total = TEMPORARY_DURATION
        .checked_mul(REPEAT_CYCLES)
        .ok_or_else(|| "temporary duration overflow".to_string())?;
    temporary_example
        .runtime
        .advance_time(duration_time(temporary_total))
        .map_err(|error| error.to_string())?;
    if temporary_example
        .runtime
        .animations()
        .temporary_visual_count()
        != 0
    {
        return Err("completed temporary visual was not removed".to_string());
    }
    if temporary_example
        .runtime
        .animations()
        .state(temporary_example.temporary_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Completed
    {
        return Err("temporary visual owner did not complete".to_string());
    }
    if !matches!(
        temporary_example
            .runtime
            .animations()
            .value(temporary_example.temporary.opacity()),
        Err(AnimationError::UnknownChannel)
    ) {
        return Err("temporary visual channels outlived their owner".to_string());
    }

    let mut cancelled = build_example();
    cancelled
        .runtime
        .advance_time(time(CANCEL_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    cancelled
        .runtime
        .animations()
        .cancel(cancelled.target_animation)
        .map_err(|error| error.to_string())?;
    for channels in cancelled.target_channels {
        if cancelled
            .runtime
            .animations()
            .has_override(channels.opacity())
            .map_err(|error| error.to_string())?
            || cancelled
                .runtime
                .animations()
                .has_override(channels.transform())
                .map_err(|error| error.to_string())?
        {
            return Err("group cancellation left a presentation override".to_string());
        }
    }

    Ok(metrics([
        ("temporary_peak_opacity", format!("{temporary_opacity:.2}")),
        ("cancelled_track_count", "6".to_string()),
    ]))
}

fn verify_trigger_and_layout_transaction() -> Result<VerificationMetrics, String> {
    let mut example = build_mechanisms_example();
    let layout_bounds = example
        .runtime
        .tree()
        .figure_bounds(example.layout_child)
        .ok_or_else(|| "layout child is detached".to_string())?;
    if layout_bounds.width != 146.0 || layout_bounds.height != 92.0 {
        return Err(format!(
            "layout transaction did not commit final bounds: {layout_bounds:?}"
        ));
    }
    if example.runtime.animations().behavior_count() != 1 {
        return Err("trigger behavior was not installed".to_string());
    }
    let trigger_start = example
        .runtime
        .animations()
        .value(example.trigger_channel)
        .map_err(|error| error.to_string())?;
    let layout_start = example
        .runtime
        .animations()
        .value(example.layout_channel)
        .map_err(|error| error.to_string())?;
    if trigger_start == Affine2D::IDENTITY || layout_start == Affine2D::IDENTITY {
        return Err("trigger or layout transition did not install presentation state".to_string());
    }

    example
        .runtime
        .advance_time(duration_time(MECHANISM_DURATION))
        .map_err(|error| error.to_string())?;
    if example
        .runtime
        .animations()
        .value(example.trigger_channel)
        .map_err(|error| error.to_string())?
        != Affine2D::IDENTITY
        || example
            .runtime
            .animations()
            .value(example.layout_channel)
            .map_err(|error| error.to_string())?
            != Affine2D::IDENTITY
    {
        return Err("trigger or layout presentation did not converge".to_string());
    }

    Ok(metrics([
        ("behavior_count", "1".to_string()),
        (
            "layout_final_size",
            format!("{:.0}x{:.0}", layout_bounds.width, layout_bounds.height),
        ),
    ]))
}

fn verify_viewport_and_route_transition() -> Result<VerificationMetrics, String> {
    let mut example = build_mechanisms_example();
    if example.viewport.view_location().x() != 118.0 {
        return Err("viewport origin was not committed before presentation".to_string());
    }
    let route_points = example
        .runtime
        .tree()
        .connection_route_points(example.connection_figure)
        .ok_or_else(|| "connection route is missing".to_string())?
        .len();
    if route_points != 4 {
        return Err(format!(
            "bendpoint route was not committed before animation: {route_points}"
        ));
    }
    if example.runtime.animations().temporary_visual_count() != 2 {
        return Err("route crossfade and pulse temporaries were not both active".to_string());
    }

    example
        .runtime
        .advance_time(duration_time(MECHANISM_DURATION))
        .map_err(|error| error.to_string())?;
    for animation in [
        example.layout_animation,
        example.viewport_animation,
        example.route_animation,
        example.pulse_animation,
    ] {
        if example
            .runtime
            .animations()
            .state(animation)
            .map_err(|error| error.to_string())?
            != AnimationState::Completed
        {
            return Err(format!("finite mechanism did not complete: {animation:?}"));
        }
    }
    if example.runtime.animations().temporary_visual_count() != 0 {
        return Err("route or pulse temporary visual leaked after completion".to_string());
    }
    if example
        .runtime
        .animations()
        .value(example.viewport_channel)
        .map_err(|error| error.to_string())?
        != Affine2D::IDENTITY
    {
        return Err("viewport presentation did not converge to committed geometry".to_string());
    }

    let before_route = example
        .runtime
        .tree()
        .connection_route_points(example.connection_figure)
        .cloned()
        .ok_or_else(|| "connection route disappeared".to_string())?;
    let (_, compatible_start) = example
        .runtime
        .transition_connection_routes_transaction(
            [example.connection_figure],
            ConnectionRouteTransition::new(std::time::Duration::from_millis(500)),
            |runtime| {
                runtime
                    .figure(example.connection_target)?
                    .set_bounds(Rectangle::new(390.0, 130.0, 88.0, 54.0))
            },
        )
        .map_err(|error| error.to_string())?;
    let AnimationStart::Running(compatible_animation) = compatible_start else {
        return Err(format!(
            "compatible route did not start: {compatible_start:?}"
        ));
    };
    let committed_route = example
        .runtime
        .tree()
        .connection_route_points(example.connection_figure)
        .cloned()
        .ok_or_else(|| "compatible route was not committed".to_string())?;
    if committed_route == before_route
        || example
            .runtime
            .animations()
            .value(example.route_channel)
            .map_err(|error| error.to_string())?
            == committed_route
    {
        return Err("compatible route did not install an old-path presentation".to_string());
    }
    example
        .runtime
        .advance_time(time(1_900))
        .map_err(|error| error.to_string())?;
    if example
        .runtime
        .animations()
        .state(compatible_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Completed
        || example
            .runtime
            .animations()
            .value(example.route_channel)
            .map_err(|error| error.to_string())?
            != committed_route
    {
        return Err("compatible route did not converge to committed geometry".to_string());
    }

    Ok(metrics([
        ("route_points", route_points.to_string()),
        ("compatible_route", "completed".to_string()),
        ("viewport_origin_x", "118".to_string()),
        ("temporary_visuals_after", "0".to_string()),
    ]))
}

fn verify_continuous_effects_and_reduced_motion() -> Result<VerificationMetrics, String> {
    let mut example = build_mechanisms_example();
    let initial_phase = example
        .runtime
        .animations()
        .value(example.dash_channel)
        .map_err(|error| error.to_string())?;
    example
        .runtime
        .advance_time(time(500))
        .map_err(|error| error.to_string())?;
    let sampled_phase = example
        .runtime
        .animations()
        .value(example.dash_channel)
        .map_err(|error| error.to_string())?;
    if approx_eq(initial_phase, sampled_phase) {
        return Err("continuous dash phase did not advance".to_string());
    }
    if example
        .runtime
        .animations()
        .state(example.dash_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("continuous dash animation terminated".to_string());
    }

    example
        .runtime
        .animations()
        .set_mode(AnimationMode::ReducedMotion);
    if example.runtime.animations().active_animation_count() != 0
        || example.runtime.animations().temporary_visual_count() != 0
    {
        return Err("reduced motion did not clear active presentation work".to_string());
    }

    Ok(metrics([
        ("dash_phase", format!("{sampled_phase:.2}")),
        ("active_after_reduced_motion", "0".to_string()),
    ]))
}

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds.saturating_mul(1_000))
}

fn duration_time(duration: std::time::Duration) -> MonotonicTime {
    MonotonicTime::from_micros(u64::try_from(duration.as_micros()).unwrap_or(u64::MAX))
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= EPSILON
}

fn metrics<const N: usize>(entries: [(&str, String); N]) -> VerificationMetrics {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}
