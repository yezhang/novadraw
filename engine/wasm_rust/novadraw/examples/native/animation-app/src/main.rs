use novadraw::event::{MonotonicTime, MouseButton};
use novadraw::{
    Affine2D, Rectangle,
    animation::{
        AnimationError, AnimationMode, AnimationPlan, AnimationStart, AnimationState,
        ConnectionRouteTransition, InterruptionPolicy, Motion, Tween,
    },
    render::{Paint, RenderCommandKind},
};
use novadraw_example_scenes::animation::{
    BOUNDS_DURATION, MECHANISM_DURATION, REPEAT_CYCLES, TEMPORARY_DURATION,
    build_choreography_example, build_example, build_mechanisms_example,
    build_runtime_control_example,
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
const MOTION_FAMILY_SAMPLE_MILLIS: u64 = 640;
const COMPOSITION_FIRST_SAMPLE_MILLIS: u64 = 500;
const COMPOSITION_SECOND_SAMPLE_MILLIS: u64 = 700;
const LOOP_POLICY_SAMPLE_MILLIS: u64 = 1_300;
const CONTROL_INTERRUPTION_SAMPLE_MILLIS: u64 = 450;
const CONTROL_INTERRUPTION_AFTER_MILLIS: u64 = 650;
const CONTROL_SUSPEND_SAMPLE_MILLIS: u64 = 400;
const CONTROL_SUSPEND_HOLD_MILLIS: u64 = 1_200;
const CONTROL_SUSPEND_RESUME_MILLIS: u64 = 1_500;
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

fn verification_cases() -> [VerificationCase; 14] {
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
        VerificationCase {
            name: "dashed-arrow-flow",
            run: verify_dashed_arrow_flow,
        },
        VerificationCase {
            name: "complete-motion-family",
            run: verify_complete_motion_family,
        },
        VerificationCase {
            name: "composition-grammar",
            run: verify_composition_grammar,
        },
        VerificationCase {
            name: "channels-and-loop-policies",
            run: verify_channels_and_loop_policies,
        },
        VerificationCase {
            name: "behavior-interruption-policies",
            run: verify_behavior_interruption_policies,
        },
        VerificationCase {
            name: "interaction-geometry-policy",
            run: verify_interaction_geometry_policy,
        },
        VerificationCase {
            name: "suspension-policies",
            run: verify_suspension_policies,
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

fn verify_dashed_arrow_flow() -> Result<VerificationMetrics, String> {
    let mut example = build_mechanisms_example();
    let arrow_bounds = example
        .runtime
        .tree()
        .figure_bounds(example.arrow_decoration)
        .ok_or_else(|| "arrow decoration is detached".to_string())?;
    let connection_bounds = example
        .runtime
        .tree()
        .figure_bounds(example.arrow_connection_figure)
        .ok_or_else(|| "connection Figure is detached".to_string())?;
    let target_bounds = example
        .runtime
        .tree()
        .figure_bounds(example.arrow_target)
        .ok_or_else(|| "arrow target is detached".to_string())?;
    if arrow_bounds.is_empty()
        || example.runtime.tree().parent_id(example.arrow_decoration)
            != Some(example.arrow_connection_figure)
    {
        return Err(format!(
            "arrow decoration was not placed on the animated connection: {arrow_bounds:?}"
        ));
    }
    let arrow_recording = example
        .runtime
        .record_full_frame()
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::StrokePath {
                path,
                paint: Paint::Solid(color),
                stroke,
            } if approx_eq(stroke.width(), 4.0) => {
                path.bounding_box().map(|bounds| (bounds, *color))
            }
            _ => None,
        });
    let Some((arrow_path_bounds, arrow_color)) = arrow_recording else {
        return Err("arrow decoration did not enter Render IR".to_string());
    };
    if arrow_color.alpha() < 0.99 {
        return Err(format!(
            "arrow decoration inherited an unexpected opacity: {}",
            arrow_color.alpha()
        ));
    }
    let arrow_tip_x =
        connection_bounds.x + arrow_bounds.x + arrow_path_bounds.x + arrow_path_bounds.width;
    let target_gap = target_bounds.x - arrow_tip_x;
    if !(6.0..=10.0).contains(&target_gap) {
        return Err(format!(
            "arrow tip does not preserve its target gap: {target_gap}"
        ));
    }
    let initial_dash = example
        .runtime
        .animations()
        .value(example.dash_channel)
        .map_err(|error| error.to_string())?;
    example
        .runtime
        .advance_time(time(500))
        .map_err(|error| error.to_string())?;
    let sampled_dash = example
        .runtime
        .animations()
        .value(example.dash_channel)
        .map_err(|error| error.to_string())?;
    if sampled_dash >= initial_dash
        || example
            .runtime
            .animations()
            .state(example.dash_animation)
            .map_err(|error| error.to_string())?
            != AnimationState::Running
    {
        return Err(format!(
            "dashed arrow flow did not remain active: {initial_dash}->{sampled_dash}"
        ));
    }

    Ok(metrics([
        (
            "arrow_bounds",
            format!(
                "{:.1},{:.1} {:.1}x{:.1} parent={:.1},{:.1}",
                arrow_bounds.x,
                arrow_bounds.y,
                arrow_bounds.width,
                arrow_bounds.height,
                connection_bounds.x,
                connection_bounds.y
            ),
        ),
        (
            "arrow_path",
            format!(
                "{:.1},{:.1} {:.1}x{:.1}",
                arrow_path_bounds.x,
                arrow_path_bounds.y,
                arrow_path_bounds.width,
                arrow_path_bounds.height
            ),
        ),
        (
            "arrow_rgba",
            format!(
                "{:.2},{:.2},{:.2},{:.2}",
                arrow_color.red(),
                arrow_color.green(),
                arrow_color.blue(),
                arrow_color.alpha()
            ),
        ),
        ("target_gap", format!("{target_gap:.2}")),
        (
            "dash_offset",
            format!("{initial_dash:.2}->{sampled_dash:.2}"),
        ),
    ]))
}

fn verify_complete_motion_family() -> Result<VerificationMetrics, String> {
    let mut example = build_choreography_example();
    let committed = example
        .sampler_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure));
    example
        .runtime
        .advance_time(time(MOTION_FAMILY_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    let values = example.sampler_channels.map(|channel| {
        example
            .runtime
            .animations()
            .value(channel)
            .map(|transform| transform.coeffs()[4])
            .map_err(|error| error.to_string())
    });
    let values: [f64; 5] = values
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "motion family did not expose five samples".to_string())?;
    for left in 0..values.len() {
        for right in (left + 1)..values.len() {
            if approx_eq(values[left], values[right]) {
                return Err(format!("motion family samples did not diverge: {values:?}"));
            }
        }
    }
    if example
        .sampler_figures
        .map(|figure| example.runtime.tree().figure_bounds(figure))
        != committed
    {
        return Err("motion family mutated committed Figure bounds".to_string());
    }
    if example
        .runtime
        .animations()
        .state(example.sampler_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("motion family composition is not running".to_string());
    }

    Ok(metrics([(
        "sampler_translation_x",
        values
            .into_iter()
            .map(|value| format!("{value:.2}"))
            .collect::<Vec<_>>()
            .join(","),
    )]))
}

fn verify_composition_grammar() -> Result<VerificationMetrics, String> {
    let mut example = build_choreography_example();
    example
        .runtime
        .advance_time(time(COMPOSITION_FIRST_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;

    let sequence = example.sequence_channels.map(|channel| {
        example
            .runtime
            .animations()
            .value(channel)
            .map(|transform| transform.coeffs()[5])
            .map_err(|error| error.to_string())
    });
    let [first, second, third] = sequence;
    let [first, second, third] = [first?, second?, third?];
    if !(approx_eq(first, 0.0) && 0.0 < second && second < third && approx_eq(third, 28.0)) {
        return Err(format!(
            "sequence did not expose completed/active/pending phases: [{first}, {second}, {third}]"
        ));
    }

    let stagger = example.stagger_channels.map(|channel| {
        example
            .runtime
            .animations()
            .value(channel)
            .map(|transform| transform.coeffs()[5])
            .map_err(|error| error.to_string())
    });
    let [first_stagger, second_stagger, third_stagger] = stagger;
    let [first_stagger, second_stagger, third_stagger] =
        [first_stagger?, second_stagger?, third_stagger?];
    if !(first_stagger < second_stagger && second_stagger < third_stagger) {
        return Err(format!(
            "stagger offsets were not ordered: [{first_stagger}, {second_stagger}, {third_stagger}]"
        ));
    }

    let delayed_before = example
        .runtime
        .animations()
        .value(example.delayed_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[5];
    if !approx_eq(delayed_before, 28.0) {
        return Err(format!(
            "delayed track did not hold its initial presentation: {delayed_before}"
        ));
    }
    example
        .runtime
        .advance_time(time(COMPOSITION_SECOND_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    let delayed_after = example
        .runtime
        .animations()
        .value(example.delayed_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[5];
    if !(0.0 < delayed_after && delayed_after < delayed_before) {
        return Err(format!(
            "delayed track did not advance after its offset: {delayed_before}->{delayed_after}"
        ));
    }
    if example
        .runtime
        .animations()
        .state(example.composition_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("composition grammar is not running".to_string());
    }

    Ok(metrics([
        ("sequence_y", format!("{first:.2},{second:.2},{third:.2}")),
        (
            "stagger_y",
            format!("{first_stagger:.2},{second_stagger:.2},{third_stagger:.2}"),
        ),
        (
            "delay_y",
            format!("{delayed_before:.2}->{delayed_after:.2}"),
        ),
    ]))
}

fn verify_channels_and_loop_policies() -> Result<VerificationMetrics, String> {
    let mut example = build_choreography_example();
    let committed = example
        .runtime
        .tree()
        .figure_bounds(example.choreography_figure);
    example
        .runtime
        .advance_time(time(LOOP_POLICY_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;

    let choreography_transform = example
        .runtime
        .animations()
        .value(example.choreography_channels.transform())
        .map_err(|error| error.to_string())?;
    let choreography_opacity = example
        .runtime
        .animations()
        .value(example.choreography_channels.opacity())
        .map_err(|error| error.to_string())?
        .get();
    if choreography_transform == Affine2D::IDENTITY || !(0.28..1.0).contains(&choreography_opacity)
    {
        return Err(format!(
            "parallel transform/opacity channels were not both sampled: opacity={choreography_opacity}"
        ));
    }
    if example
        .runtime
        .tree()
        .figure_bounds(example.choreography_figure)
        != committed
    {
        return Err("channel choreography mutated committed bounds".to_string());
    }

    let loop_values = example.loop_channels.map(|channel| {
        example
            .runtime
            .animations()
            .value(channel)
            .map(|transform| transform.coeffs()[4])
            .map_err(|error| error.to_string())
    });
    let [restart, reverse, continuous] = loop_values;
    let [restart, reverse, continuous] = [restart?, reverse?, continuous?];
    if approx_eq(restart, reverse)
        || approx_eq(restart, continuous)
        || approx_eq(reverse, continuous)
    {
        return Err(format!(
            "loop policies did not diverge: [{restart}, {reverse}, {continuous}]"
        ));
    }
    for animation in [
        example.choreography_animation,
        example.loop_animations[0],
        example.loop_animations[1],
        example.loop_animations[2],
    ] {
        if example
            .runtime
            .animations()
            .state(animation)
            .map_err(|error| error.to_string())?
            != AnimationState::Running
        {
            return Err(format!("loop animation is not running: {animation:?}"));
        }
    }

    Ok(metrics([
        ("choreography_opacity", format!("{choreography_opacity:.2}")),
        (
            "loop_translation_x",
            format!("{restart:.2},{reverse:.2},{continuous:.2}"),
        ),
    ]))
}

fn verify_behavior_interruption_policies() -> Result<VerificationMetrics, String> {
    let mut example = build_runtime_control_example();
    if example.runtime.animations().behavior_count() != 2
        || example.replace_behavior == example.ignore_behavior
    {
        return Err("control scene did not install two independent behaviors".to_string());
    }
    example
        .runtime
        .advance_time(time(CONTROL_INTERRUPTION_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    let replace_before = example
        .runtime
        .animations()
        .value(example.replace_channels.transform())
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    let ignore_before = example
        .runtime
        .animations()
        .value(example.ignore_channels.transform())
        .map_err(|error| error.to_string())?
        .coeffs()[4];

    let ignore_attempt = AnimationPlan::track(
        example.ignore_channels.transform(),
        Motion::Tween(
            Tween::to(Affine2D::IDENTITY, std::time::Duration::from_millis(600))
                .map_err(|error| error.to_string())?,
        ),
    )
    .map_err(|error| error.to_string())?
    .with_interruption(InterruptionPolicy::Ignore);
    let AnimationStart::Existing(ignore_owner) = example
        .runtime
        .animations()
        .start(ignore_attempt)
        .map_err(|error| error.to_string())?
    else {
        return Err("Ignore did not retain the active behavior owner".to_string());
    };

    let replacement = AnimationPlan::track(
        example.replace_channels.transform(),
        Motion::Tween(
            Tween::to(Affine2D::IDENTITY, std::time::Duration::from_millis(600))
                .map_err(|error| error.to_string())?,
        ),
    )
    .map_err(|error| error.to_string())?;
    let AnimationStart::Running(replacement_owner) = example
        .runtime
        .animations()
        .start(replacement)
        .map_err(|error| error.to_string())?
    else {
        return Err("Replace did not install a new channel owner".to_string());
    };
    let replace_handoff = example
        .runtime
        .animations()
        .value(example.replace_channels.transform())
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    if !approx_eq(replace_handoff, replace_before) {
        return Err(format!(
            "Replace did not preserve the current presentation sample: {replace_before}->{replace_handoff}"
        ));
    }

    example
        .runtime
        .advance_time(time(CONTROL_INTERRUPTION_AFTER_MILLIS))
        .map_err(|error| error.to_string())?;
    let replace_after = example
        .runtime
        .animations()
        .value(example.replace_channels.transform())
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    let ignore_after = example
        .runtime
        .animations()
        .value(example.ignore_channels.transform())
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    if !(replace_after.abs() < replace_before.abs() && ignore_after.abs() > ignore_before.abs()) {
        return Err(format!(
            "Replace and Ignore did not diverge after interruption: replace={replace_before}->{replace_after}, ignore={ignore_before}->{ignore_after}"
        ));
    }

    Ok(metrics([
        (
            "replace_translation_x",
            format!("{replace_before:.2}->{replace_after:.2}"),
        ),
        (
            "ignore_translation_x",
            format!("{ignore_before:.2}->{ignore_after:.2}"),
        ),
        (
            "owners",
            format!("existing={ignore_owner:?},replacement={replacement_owner:?}"),
        ),
    ]))
}

fn verify_interaction_geometry_policy() -> Result<VerificationMetrics, String> {
    let mut example = build_runtime_control_example();
    let committed_bounds = example
        .runtime
        .tree()
        .figure_bounds(example.moving_hit_toggle)
        .ok_or_else(|| "moving hit toggle is detached".to_string())?;
    let presentation_offset = example
        .runtime
        .animations()
        .value(example.moving_hit_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[4];

    let visual_x = committed_bounds.x + presentation_offset + committed_bounds.width / 2.0;
    let visual_y = committed_bounds.y + committed_bounds.height / 2.0;
    let visual_outcome =
        example
            .runtime
            .dispatch_mouse_pressed(visual_x, visual_y, MouseButton::Left);
    if visual_outcome.target() == Some(example.moving_hit_toggle) {
        return Err("Committed hit geometry followed its presentation transform".to_string());
    }
    example.runtime.pointer_exited();

    let committed_x = committed_bounds.x + committed_bounds.width / 2.0;
    let committed_y = committed_bounds.y + committed_bounds.height / 2.0;
    let committed_outcome =
        example
            .runtime
            .dispatch_mouse_pressed(committed_x, committed_y, MouseButton::Left);
    if committed_outcome.target() != Some(example.moving_hit_toggle)
        || !committed_outcome.is_handled()
    {
        return Err(format!(
            "Committed hit guide did not retain the moving Figure target: {committed_outcome:?}"
        ));
    }
    example
        .runtime
        .dispatch_mouse_released(committed_x, committed_y, MouseButton::Left);
    if example
        .runtime
        .animations()
        .state(example.hit_animation)
        .map_err(|error| error.to_string())?
        != AnimationState::Running
    {
        return Err("hit-policy animation is not running".to_string());
    }

    Ok(metrics([
        (
            "visual_sample_target",
            format!("{:?}", visual_outcome.target()),
        ),
        (
            "committed_guide_target",
            format!("{:?}", committed_outcome.target()),
        ),
        ("presentation_offset_x", format!("{presentation_offset:.1}")),
    ]))
}

fn verify_suspension_policies() -> Result<VerificationMetrics, String> {
    let mut example = build_runtime_control_example();
    example
        .runtime
        .advance_time(time(CONTROL_SUSPEND_SAMPLE_MILLIS))
        .map_err(|error| error.to_string())?;
    let pause_before = example
        .runtime
        .animations()
        .value(example.pause_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    let finish_before = example
        .runtime
        .animations()
        .value(example.finish_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[4];

    example
        .runtime
        .figure(example.pause_figure)
        .map_err(|error| error.to_string())?
        .set_visible(false)
        .map_err(|error| error.to_string())?;
    example
        .runtime
        .figure(example.finish_figure)
        .map_err(|error| error.to_string())?
        .set_visible(false)
        .map_err(|error| error.to_string())?;
    let pause_state = example
        .runtime
        .animations()
        .state(example.suspension_animations[0])
        .map_err(|error| error.to_string())?;
    let finish_state = example
        .runtime
        .animations()
        .state(example.suspension_animations[1])
        .map_err(|error| error.to_string())?;
    if pause_state != AnimationState::Paused || finish_state != AnimationState::Completed {
        return Err(format!(
            "hidden targets did not apply Pause/Finish: {pause_state:?}/{finish_state:?}"
        ));
    }
    if example
        .runtime
        .animations()
        .value(example.finish_channel)
        .map_err(|error| error.to_string())?
        != Affine2D::IDENTITY
    {
        return Err("Finish did not settle to committed presentation".to_string());
    }

    example
        .runtime
        .advance_time(time(CONTROL_SUSPEND_HOLD_MILLIS))
        .map_err(|error| error.to_string())?;
    let pause_held = example
        .runtime
        .animations()
        .value(example.pause_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    if !approx_eq(pause_before, pause_held) {
        return Err(format!(
            "Pause advanced while hidden: {pause_before}->{pause_held}"
        ));
    }

    example
        .runtime
        .figure(example.pause_figure)
        .map_err(|error| error.to_string())?
        .set_visible(true)
        .map_err(|error| error.to_string())?;
    example
        .runtime
        .figure(example.finish_figure)
        .map_err(|error| error.to_string())?
        .set_visible(true)
        .map_err(|error| error.to_string())?;
    example
        .runtime
        .advance_time(time(CONTROL_SUSPEND_RESUME_MILLIS))
        .map_err(|error| error.to_string())?;
    let pause_after = example
        .runtime
        .animations()
        .value(example.pause_channel)
        .map_err(|error| error.to_string())?
        .coeffs()[4];
    if !(pause_after > pause_held
        && example
            .runtime
            .animations()
            .state(example.suspension_animations[0])
            .map_err(|error| error.to_string())?
            == AnimationState::Running)
    {
        return Err(format!(
            "Pause did not resume from frozen local time: {pause_held}->{pause_after}"
        ));
    }

    Ok(metrics([
        (
            "pause_translation_x",
            format!("{pause_before:.2}->{pause_held:.2}->{pause_after:.2}"),
        ),
        ("finish_translation_x", format!("{finish_before:.2}->0.00")),
        ("hidden_states", format!("{pause_state:?},{finish_state:?}")),
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
