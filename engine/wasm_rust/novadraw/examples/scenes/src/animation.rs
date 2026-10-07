//! P2-M01 animation orthogonality example.

use std::{sync::Arc, time::Duration};

use novadraw::{
    Affine2D, Bendpoint, BendpointConnectionRouter, BendpointConstraint, ChopboxAnchor, Color,
    ConnectionFigure, CoordinateSpace, Dimension, EllipseFigure, FigureId, FigureMeasurement,
    FigureStyle, LabelFigure, MonotonicTime, Point, Rectangle, RectangleFigure,
    RoundedRectangleFigure, RouterBinding, Runtime, StackLayout, ViewportHandle,
    animation::{
        AnimationBehavior, AnimationBehaviorContext, AnimationBehaviorId, AnimationChannel,
        AnimationId, AnimationPlan, AnimationStart, AnimationTrigger, BoundsTransition,
        ConnectionPulse, ConnectionRouteTransition, Easing, FigurePresentationChannels,
        InteractionGeometryPolicy, Keyframe, Keyframes, Motion, Opacity, RepeatBehavior, Spring,
        TemporaryVisual, Tween, ViewportTransition,
    },
    figure::{FigureDrawing, FigurePresentation},
    graphics::{DashPattern, GraphicsError, PaintContext, StrokeStyle},
    render::BuiltinFont,
};

use crate::{DemoSuite, SceneSpec};

pub const WINDOW_WIDTH: f64 = 800.0;
pub const WINDOW_HEIGHT: f64 = 600.0;
pub const ARRIVAL_DURATION: Duration = Duration::from_millis(1_200);
pub const ARRIVAL_STAGGER: Duration = Duration::from_millis(180);
pub const MOTION_DURATION: Duration = Duration::from_millis(1_400);
pub const BOUNDS_DURATION: Duration = Duration::from_millis(1_600);
pub const TEMPORARY_DURATION: Duration = Duration::from_millis(1_800);
pub const MECHANISM_DURATION: Duration = Duration::from_millis(1_400);
pub const PULSE_DURATION: Duration = Duration::from_millis(1_100);
pub const HANDOFF_DURATION: Duration = Duration::from_millis(300);
pub const REPEAT_CYCLES: u32 = 41;

const BACKGROUND: Color = Color::rgba(0.945, 0.95, 0.955, 1.0);
const PANEL: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const PANEL_STROKE: Color = Color::rgba(0.79, 0.82, 0.84, 1.0);
const TEXT: Color = Color::rgba(0.09, 0.12, 0.15, 1.0);
const MUTED_TEXT: Color = Color::rgba(0.36, 0.4, 0.44, 1.0);
const BLUE: Color = Color::rgba(0.05, 0.46, 0.71, 1.0);
const GREEN: Color = Color::rgba(0.07, 0.57, 0.38, 1.0);
const CORAL: Color = Color::rgba(0.88, 0.25, 0.2, 1.0);
const GOLD: Color = Color::rgba(0.96, 0.68, 0.08, 1.0);
const VIOLET: Color = Color::rgba(0.47, 0.3, 0.73, 1.0);
const GHOST: Color = Color::rgba(0.62, 0.66, 0.69, 1.0);

const PANEL_X: f64 = 28.0;
const PANEL_WIDTH: f64 = 744.0;
const PANEL_HEIGHT: f64 = 142.0;
const FIRST_ROW_Y: f64 = 76.0;
const SECOND_ROW_Y: f64 = 230.0;
const THIRD_ROW_Y: f64 = 384.0;
const SHAPE_WIDTH: f64 = 72.0;
const SHAPE_HEIGHT: f64 = 48.0;
const ARRIVAL_OFFSET_X: f64 = -92.0;
const MOTION_OFFSET_Y: f64 = 34.0;

/// Handles retained by the headless verifier so each orthogonal dimension can be inspected.
pub struct AnimationExample {
    pub runtime: Runtime,
    pub target_figures: [FigureId; 3],
    pub target_channels: [FigurePresentationChannels; 3],
    pub motion_figures: [FigureId; 3],
    pub motion_channels: [AnimationChannel<Affine2D>; 3],
    pub bounds_figure: FigureId,
    pub bounds_channel: AnimationChannel<Affine2D>,
    pub temporary: TemporaryVisual,
    pub target_animation: AnimationId,
    pub motion_animation: AnimationId,
    pub bounds_animation: AnimationId,
    pub temporary_animation: AnimationId,
}

/// Handles retained by the complete-mechanism verifier.
pub struct AnimationMechanismsExample {
    pub runtime: Runtime,
    pub trigger_figure: FigureId,
    pub trigger_channel: AnimationChannel<Affine2D>,
    pub trigger_behavior: AnimationBehaviorId,
    pub layout_child: FigureId,
    pub layout_channel: AnimationChannel<Affine2D>,
    pub layout_animation: AnimationId,
    pub viewport: ViewportHandle,
    pub viewport_contents: FigureId,
    pub viewport_channel: AnimationChannel<Affine2D>,
    pub viewport_animation: AnimationId,
    pub connection_figure: FigureId,
    pub connection_target: FigureId,
    pub route_channel: AnimationChannel<novadraw::PointList>,
    pub dash_channel: AnimationChannel<f64>,
    pub route_animation: AnimationId,
    pub dash_animation: AnimationId,
    pub pulse_animation: AnimationId,
}

struct TemporaryMarkerDrawing;

impl FigureDrawing for TemporaryMarkerDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_fill_paint(GOLD);
        context.fill_rect(Rectangle::new(0.0, 0.0, 22.0, 22.0))?;
        context.set_fill_paint(Color::WHITE);
        context.fill_rect(Rectangle::new(6.0, 6.0, 10.0, 10.0))
    }
}

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "animation",
        "Animation",
        vec![
            SceneSpec::runtime_visual(
                "orthogonal-model",
                "Orthogonal_Model",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                || build_example().runtime,
            ),
            SceneSpec::runtime_visual(
                "runtime-mechanisms",
                "Runtime_Mechanisms",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                || build_mechanisms_example().runtime,
            ),
        ],
    )
}

pub fn build_example() -> AnimationExample {
    let mut runtime = Runtime::empty();
    runtime
        .register_builtin_font(BuiltinFont::Inter)
        .expect("built-in font");
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            BACKGROUND,
        )))
        .expect("valid animation root");
    runtime
        .figure(root)
        .expect("animation root")
        .set_style(FigureStyle {
            foreground: Some(TEXT),
            font: Some("14px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid root style");

    add_label(
        &mut runtime,
        root,
        "Animation = Target x Motion x Composition",
        Rectangle::new(32.0, 18.0, 736.0, 30.0),
        "22px Inter Variable",
        TEXT,
    );
    add_label(
        &mut runtime,
        root,
        "Committed state stays final; Runtime samples a disposable presentation plane.",
        Rectangle::new(32.0, 47.0, 736.0, 20.0),
        "13px Inter Variable",
        MUTED_TEXT,
    );
    for row_y in [FIRST_ROW_Y, SECOND_ROW_Y, THIRD_ROW_Y] {
        add_panel(&mut runtime, root, row_y);
    }

    add_label(
        &mut runtime,
        root,
        "TARGET REUSE  /  one arrival motion, three Figure types, staggered",
        Rectangle::new(48.0, FIRST_ROW_Y + 12.0, 690.0, 22.0),
        "14px Inter Variable",
        TEXT,
    );
    let target_figures = add_target_row(&mut runtime, root);

    add_label(
        &mut runtime,
        root,
        "MOTION REUSE  /  same transform target, interchangeable samplers in parallel",
        Rectangle::new(48.0, SECOND_ROW_Y + 12.0, 690.0, 22.0),
        "14px Inter Variable",
        TEXT,
    );
    let motion_figures = add_motion_row(&mut runtime, root);

    add_label(
        &mut runtime,
        root,
        "PRESENTATION  /  committed bounds transition + non-source temporary visual",
        Rectangle::new(48.0, THIRD_ROW_Y + 12.0, 690.0, 22.0),
        "14px Inter Variable",
        TEXT,
    );
    let (bounds_figure, old_bounds, final_bounds) = add_presentation_row(&mut runtime, root);

    runtime
        .advance_time(MonotonicTime::ZERO)
        .expect("animation clock starts at zero");

    let target_channels = target_figures.map(|figure| {
        runtime
            .animations()
            .bind_figure(figure, InteractionGeometryPolicy::Committed)
            .expect("target Figure channels")
    });
    let target_animation = start_running(
        &mut runtime,
        repeated_target_plan(target_channels).expect("valid target animation"),
    );

    let motion_channels = motion_figures.map(|figure| {
        runtime
            .animations()
            .bind_figure(figure, InteractionGeometryPolicy::Committed)
            .expect("motion Figure channels")
            .transform()
    });
    let motion_animation = start_running(
        &mut runtime,
        repeated_motion_plan(motion_channels).expect("valid motion animation"),
    );

    let bounds_channel = runtime
        .animations()
        .bind_figure(bounds_figure, InteractionGeometryPolicy::Committed)
        .expect("bounds Figure channels")
        .transform();
    let capture = runtime
        .animations()
        .capture_figures([bounds_figure])
        .expect("bounds capture");
    runtime
        .figure(bounds_figure)
        .expect("bounds Figure")
        .set_bounds(final_bounds)
        .expect("commit final bounds");
    let bounds_animation = match runtime
        .animations()
        .transition_bounds(
            capture,
            BoundsTransition::new(BOUNDS_DURATION).with_easing(Easing::EaseInOut),
        )
        .expect("valid bounds transition")
    {
        AnimationStart::Running(animation) => animation,
        outcome => panic!("bounds transition must run, got {outcome:?}"),
    };
    debug_assert_ne!(old_bounds, final_bounds);

    let temporary = runtime
        .animations()
        .create_temporary_visual(
            temporary_presentation(),
            temporary_end_transform(),
            Opacity::TRANSPARENT.get(),
        )
        .expect("temporary visual");
    let temporary_animation = start_running(
        &mut runtime,
        repeated_temporary_plan(temporary).expect("valid temporary animation"),
    );

    AnimationExample {
        runtime,
        target_figures,
        target_channels,
        motion_figures,
        motion_channels,
        bounds_figure,
        bounds_channel,
        temporary,
        target_animation,
        motion_animation,
        bounds_animation,
        temporary_animation,
    }
}

/// Builds a scene that exercises Runtime orchestration and domain-specific consumers.
pub fn build_mechanisms_example() -> AnimationMechanismsExample {
    let mut runtime = Runtime::empty();
    runtime
        .register_builtin_font(BuiltinFont::Inter)
        .expect("built-in font");
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            BACKGROUND,
        )))
        .expect("valid animation root");
    runtime
        .figure(root)
        .expect("animation root")
        .set_style(FigureStyle {
            foreground: Some(TEXT),
            font: Some("14px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid root style");

    add_label(
        &mut runtime,
        root,
        "Animation Runtime Mechanisms",
        Rectangle::new(32.0, 18.0, 736.0, 30.0),
        "22px Inter Variable",
        TEXT,
    );
    add_label(
        &mut runtime,
        root,
        "Stable transactions, optional behaviors, domain channels, continuous effects.",
        Rectangle::new(32.0, 47.0, 736.0, 20.0),
        "13px Inter Variable",
        MUTED_TEXT,
    );
    runtime
        .advance_time(MonotonicTime::ZERO)
        .expect("animation clock starts at zero");

    let trigger_panel = add_mechanism_panel(
        &mut runtime,
        root,
        Rectangle::new(28.0, 84.0, 176.0, 188.0),
        "TRIGGER",
        "bounds fact -> plan",
    );
    let trigger_figure = runtime
        .container(trigger_panel)
        .expect("trigger panel")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(28.0, 82.0, 94.0, 54.0, 10.0, BLUE)
                .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("trigger Figure");
    let trigger_channel = runtime
        .animations()
        .bind_figure(trigger_figure, InteractionGeometryPolicy::Committed)
        .expect("trigger channel")
        .transform();
    let behavior_channel = trigger_channel;
    let trigger_behavior = runtime
        .animations()
        .install_behavior(
            AnimationBehavior::new(
                AnimationTrigger::FigureBoundsChanged,
                move |_context: AnimationBehaviorContext<'_>| {
                    Ok(Some(AnimationPlan::track(
                        behavior_channel,
                        Motion::Tween(
                            Tween::between(
                                Affine2D::from_uniform_scale(0.72),
                                Affine2D::IDENTITY,
                                MECHANISM_DURATION,
                            )?
                            .with_easing(Easing::EaseOut),
                        ),
                    )?))
                },
            )
            .scoped_to(trigger_figure),
        )
        .expect("trigger behavior");
    runtime
        .figure(trigger_figure)
        .expect("trigger Figure")
        .set_bounds(Rectangle::new(38.0, 76.0, 114.0, 66.0))
        .expect("trigger source mutation");
    runtime
        .stabilize_for_query()
        .expect("trigger stable transaction");

    let layout_panel = add_mechanism_panel(
        &mut runtime,
        root,
        Rectangle::new(218.0, 84.0, 176.0, 188.0),
        "LAYOUT",
        "stable before -> final",
    );
    let layout_container = runtime
        .container(layout_panel)
        .expect("layout panel")
        .add(Box::new(
            RectangleFigure::new_with_color(18.0, 72.0, 112.0, 72.0, Color::TRANSPARENT)
                .with_stroke(GHOST, 2.0),
        ))
        .expect("layout container");
    let layout_child = runtime
        .container(layout_container)
        .expect("layout container")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(0.0, 0.0, 20.0, 20.0, 8.0, GREEN)
                .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("layout child");
    runtime
        .container(layout_container)
        .expect("layout container")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("stack layout");
    let layout_channel = runtime
        .animations()
        .bind_figure(layout_child, InteractionGeometryPolicy::Committed)
        .expect("layout channel")
        .transform();
    let (_, layout_start) = runtime
        .transition_bounds_transaction(
            [layout_child],
            BoundsTransition::new(MECHANISM_DURATION).with_easing(Easing::EaseInOut),
            |runtime| {
                runtime
                    .figure(layout_container)?
                    .set_bounds(Rectangle::new(12.0, 62.0, 146.0, 92.0))
            },
        )
        .expect("layout transition");
    let layout_animation = expect_running(layout_start, "layout transition");

    let viewport_panel = add_mechanism_panel(
        &mut runtime,
        root,
        Rectangle::new(408.0, 84.0, 176.0, 188.0),
        "VIEWPORT",
        "pan committed first",
    );
    let viewport = runtime
        .add_viewport(viewport_panel, Rectangle::new(14.0, 72.0, 148.0, 84.0))
        .expect("viewport");
    let viewport_contents = runtime
        .viewport(viewport.figure_id())
        .expect("viewport")
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 320.0, 84.0, VIOLET)
                .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("viewport contents");
    for (x, color) in [(18.0, BLUE), (124.0, GOLD), (230.0, GREEN)] {
        runtime
            .container(viewport_contents)
            .expect("viewport contents")
            .add(Box::new(
                RoundedRectangleFigure::new_with_color(x, 18.0, 72.0, 48.0, 8.0, color)
                    .with_stroke(Color::WHITE, 2.0),
            ))
            .expect("viewport marker");
    }
    let viewport_channel = runtime
        .animations()
        .bind_figure(viewport_contents, InteractionGeometryPolicy::Committed)
        .expect("viewport channel")
        .transform();
    let (_, viewport_start) = runtime
        .transition_viewport_transaction(
            &viewport,
            ViewportTransition::new(MECHANISM_DURATION).with_easing(Easing::EaseInOut),
            |runtime| {
                runtime
                    .viewport(viewport.figure_id())?
                    .set_view_location(118.0, 0.0)
            },
        )
        .expect("viewport transition");
    let viewport_animation = expect_running(viewport_start, "viewport transition");

    let connection_panel = add_mechanism_panel(
        &mut runtime,
        root,
        Rectangle::new(28.0, 292.0, 556.0, 270.0),
        "CONNECTION",
        "route crossfade + dash flow + pulse",
    );
    let source = runtime
        .container(connection_panel)
        .expect("connection panel")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(34.0, 104.0, 88.0, 54.0, 8.0, BLUE)
                .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("source");
    let target = runtime
        .container(connection_panel)
        .expect("connection panel")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(420.0, 104.0, 88.0, 54.0, 8.0, CORAL)
                .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("target");
    let connection_figure = runtime
        .container(connection_panel)
        .expect("connection panel")
        .add(Box::new(
            ConnectionFigure::new().with_stroke_style(
                StrokeStyle::default()
                    .with_width(3.0)
                    .expect("stroke width")
                    .with_dash_pattern(DashPattern::Dash),
            ),
        ))
        .expect("connection Figure");
    let source_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(source)));
    let target_anchor = runtime.register_connection_anchor(Box::new(ChopboxAnchor::new(target)));
    let connection = runtime
        .register_connection_state(
            connection_figure,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit {
                router: runtime.direct_connection_router(),
            },
            None,
        )
        .expect("connection state");
    runtime
        .resolve_connection_route(connection, CoordinateSpace::ChildContent(connection_panel))
        .expect("initial direct route");
    let route_channel = runtime
        .animations()
        .bind_connection_route(connection_figure)
        .expect("route channel");
    let router = runtime.register_connection_router(Box::new(BendpointConnectionRouter));
    let (_, route_start) = runtime
        .transition_connection_routes_transaction(
            [connection_figure],
            ConnectionRouteTransition::new(MECHANISM_DURATION).with_easing(Easing::EaseInOut),
            |runtime| {
                runtime.set_connection_route_configuration(
                    connection,
                    RouterBinding::Explicit { router },
                    Some(Box::new(BendpointConstraint::new(vec![
                        Bendpoint::Absolute(Point::new(270.0, 48.0)),
                        Bendpoint::Absolute(Point::new(270.0, 204.0)),
                    ]))),
                )
            },
        )
        .expect("route transition");
    let route_animation = expect_running(route_start, "route transition");
    let dash_animation = expect_running(
        runtime
            .animations()
            .start_connection_dash_flow(connection_figure, 28.0)
            .expect("dash flow"),
        "dash flow",
    );
    let dash_channel = runtime
        .animations()
        .bind_connection_dash_offset(connection_figure)
        .expect("dash channel");
    let pulse_animation = expect_running(
        runtime
            .animations()
            .start_connection_pulse(
                connection_figure,
                ConnectionPulse::new(PULSE_DURATION, 6.0, GOLD)
                    .with_endpoint_handoff(target, HANDOFF_DURATION),
            )
            .expect("connection pulse"),
        "connection pulse",
    );

    AnimationMechanismsExample {
        runtime,
        trigger_figure,
        trigger_channel,
        trigger_behavior,
        layout_child,
        layout_channel,
        layout_animation,
        viewport,
        viewport_contents,
        viewport_channel,
        viewport_animation,
        connection_figure,
        connection_target: target,
        route_channel,
        dash_channel,
        route_animation,
        dash_animation,
        pulse_animation,
    }
}

fn add_mechanism_panel(
    runtime: &mut Runtime,
    root: FigureId,
    bounds: Rectangle,
    title: &str,
    subtitle: &str,
) -> FigureId {
    let panel = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RectangleFigure::new_with_color(bounds.x, bounds.y, bounds.width, bounds.height, PANEL)
                .with_stroke(PANEL_STROKE, 1.0),
        ))
        .expect("mechanism panel");
    add_label(
        runtime,
        panel,
        title,
        Rectangle::new(14.0, 12.0, bounds.width - 28.0, 20.0),
        "13px Inter Variable",
        TEXT,
    );
    add_label(
        runtime,
        panel,
        subtitle,
        Rectangle::new(14.0, 34.0, bounds.width - 28.0, 18.0),
        "11px Inter Variable",
        MUTED_TEXT,
    );
    panel
}

fn expect_running(start: AnimationStart, label: &str) -> AnimationId {
    match start {
        AnimationStart::Running(animation) => animation,
        outcome => panic!("{label} must run, got {outcome:?}"),
    }
}

fn add_panel(runtime: &mut Runtime, root: FigureId, y: f64) {
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RectangleFigure::new_with_color(PANEL_X, y, PANEL_WIDTH, PANEL_HEIGHT, PANEL)
                .with_stroke(PANEL_STROKE, 1.0),
        ))
        .expect("panel");
}

fn add_label(
    runtime: &mut Runtime,
    root: FigureId,
    text: &str,
    bounds: Rectangle,
    font: &str,
    color: Color,
) -> FigureId {
    let label = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(LabelFigure::new(text).with_bounds(bounds)))
        .expect("label");
    runtime
        .figure(label)
        .expect("label Figure")
        .set_style(FigureStyle {
            foreground: Some(color),
            font: Some(font.to_string()),
            ..FigureStyle::default()
        })
        .expect("label style");
    label
}

fn add_target_row(runtime: &mut Runtime, root: FigureId) -> [FigureId; 3] {
    let y = FIRST_ROW_Y + 66.0;
    let positions = [190.0, 390.0, 590.0];
    let labels = ["Rectangle", "Ellipse", "Rounded"];
    for (x, label) in positions.into_iter().zip(labels) {
        add_label(
            runtime,
            root,
            label,
            Rectangle::new(x - 12.0, FIRST_ROW_Y + 40.0, 96.0, 18.0),
            "12px Inter Variable",
            MUTED_TEXT,
        );
    }
    [
        runtime
            .container(root)
            .expect("root container")
            .add(Box::new(
                RectangleFigure::new_with_color(positions[0], y, SHAPE_WIDTH, SHAPE_HEIGHT, BLUE)
                    .with_stroke(Color::WHITE, 2.0),
            ))
            .expect("target rectangle"),
        runtime
            .container(root)
            .expect("root container")
            .add(Box::new(
                EllipseFigure::new_with_color(positions[1], y, SHAPE_WIDTH, SHAPE_HEIGHT, GREEN)
                    .with_stroke(Color::WHITE, 2.0),
            ))
            .expect("target ellipse"),
        runtime
            .container(root)
            .expect("root container")
            .add(Box::new(RoundedRectangleFigure::new_with_color(
                positions[2],
                y,
                SHAPE_WIDTH,
                SHAPE_HEIGHT,
                12.0,
                CORAL,
            )))
            .expect("target rounded rectangle"),
    ]
}

fn add_motion_row(runtime: &mut Runtime, root: FigureId) -> [FigureId; 3] {
    let y = SECOND_ROW_Y + 50.0;
    let positions = [190.0, 390.0, 590.0];
    let labels = ["Tween", "Keyframes", "Spring"];
    let colors = [BLUE, VIOLET, GREEN];
    let mut figures = Vec::with_capacity(positions.len());
    for ((x, label), color) in positions.into_iter().zip(labels).zip(colors) {
        add_label(
            runtime,
            root,
            label,
            Rectangle::new(x - 4.0, SECOND_ROW_Y + 40.0, 90.0, 18.0),
            "12px Inter Variable",
            MUTED_TEXT,
        );
        figures.push(
            runtime
                .container(root)
                .expect("root container")
                .add(Box::new(
                    EllipseFigure::new_with_color(x, y, SHAPE_WIDTH, SHAPE_HEIGHT, color)
                        .with_stroke(Color::WHITE, 2.0),
                ))
                .expect("motion Figure"),
        );
    }
    figures.try_into().expect("three motion Figures")
}

fn add_presentation_row(runtime: &mut Runtime, root: FigureId) -> (FigureId, Rectangle, Rectangle) {
    let old_bounds = Rectangle::new(124.0, THIRD_ROW_Y + 69.0, 64.0, 44.0);
    let final_bounds = Rectangle::new(302.0, THIRD_ROW_Y + 59.0, 92.0, 64.0);
    runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RectangleFigure::new_with_color(
                old_bounds.x,
                old_bounds.y,
                old_bounds.width,
                old_bounds.height,
                Color::TRANSPARENT,
            )
            .with_stroke(GHOST, 2.0),
        ))
        .expect("old bounds guide");
    let figure = runtime
        .container(root)
        .expect("root container")
        .add(Box::new(
            RoundedRectangleFigure::new_with_color(
                old_bounds.x,
                old_bounds.y,
                old_bounds.width,
                old_bounds.height,
                10.0,
                CORAL,
            )
            .with_stroke(Color::WHITE, 2.0),
        ))
        .expect("bounds transition Figure");
    add_label(
        runtime,
        root,
        "source final",
        Rectangle::new(304.0, THIRD_ROW_Y + 116.0, 90.0, 18.0),
        "11px Inter Variable",
        MUTED_TEXT,
    );
    add_label(
        runtime,
        root,
        "temporary",
        Rectangle::new(610.0, THIRD_ROW_Y + 116.0, 90.0, 18.0),
        "11px Inter Variable",
        MUTED_TEXT,
    );
    (figure, old_bounds, final_bounds)
}

fn repeated_target_plan(
    channels: [FigurePresentationChannels; 3],
) -> Result<AnimationPlan, novadraw::animation::AnimationError> {
    let plans = channels
        .into_iter()
        .map(|channels| {
            AnimationPlan::parallel(vec![
                AnimationPlan::track(
                    channels.transform(),
                    Motion::Tween(
                        Tween::between(
                            Affine2D::from_translation(ARRIVAL_OFFSET_X, 0.0),
                            Affine2D::IDENTITY,
                            ARRIVAL_DURATION,
                        )?
                        .with_easing(Easing::EaseOut),
                    ),
                )?,
                AnimationPlan::track(
                    channels.opacity(),
                    Motion::Tween(
                        Tween::between(Opacity::try_new(0.25)?, Opacity::OPAQUE, ARRIVAL_DURATION)?
                            .with_easing(Easing::EaseOut),
                    ),
                )?,
            ])
        })
        .collect::<Result<Vec<_>, _>>()?;
    AnimationPlan::repeat(
        REPEAT_CYCLES,
        RepeatBehavior::Reverse,
        AnimationPlan::stagger(ARRIVAL_STAGGER, plans)?,
    )
}

fn repeated_motion_plan(
    channels: [AnimationChannel<Affine2D>; 3],
) -> Result<AnimationPlan, novadraw::animation::AnimationError> {
    let start = Affine2D::from_translation(0.0, MOTION_OFFSET_Y);
    let tween = AnimationPlan::track(
        channels[0],
        Motion::Tween(
            Tween::between(start, Affine2D::IDENTITY, MOTION_DURATION)?
                .with_easing(Easing::EaseInOut),
        ),
    )?;
    let keyframes = AnimationPlan::track(
        channels[1],
        Motion::Keyframes(Keyframes::new(
            MOTION_DURATION,
            vec![
                Keyframe::new(0.0, start),
                Keyframe::new(0.58, Affine2D::from_translation(0.0, -20.0)),
                Keyframe::new(1.0, Affine2D::IDENTITY),
            ],
        )?),
    )?;
    let spring = AnimationPlan::track(
        channels[2],
        Motion::Spring(Spring::between(
            start,
            Affine2D::IDENTITY,
            1.0,
            120.0,
            12.0,
            0.001,
            MOTION_DURATION,
        )?),
    )?;
    AnimationPlan::repeat(
        REPEAT_CYCLES,
        RepeatBehavior::Reverse,
        AnimationPlan::parallel(vec![tween, keyframes, spring])?,
    )
}

fn temporary_presentation() -> FigurePresentation {
    let size = Dimension::new(22.0, 22.0);
    FigurePresentation::new(
        FigureMeasurement::new(size.width, size.height, None),
        size,
        Rectangle::new(0.0, 0.0, size.width, size.height),
        Arc::new(TemporaryMarkerDrawing),
    )
    .expect("valid temporary visual")
}

fn temporary_end_transform() -> Affine2D {
    Affine2D::from_translation(704.0, THIRD_ROW_Y + 72.0)
}

fn repeated_temporary_plan(
    visual: TemporaryVisual,
) -> Result<AnimationPlan, novadraw::animation::AnimationError> {
    let transform = AnimationPlan::track(
        visual.transform(),
        Motion::Keyframes(Keyframes::new(
            TEMPORARY_DURATION,
            vec![
                Keyframe::new(0.0, Affine2D::from_translation(500.0, THIRD_ROW_Y + 82.0)),
                Keyframe::new(0.36, Affine2D::from_translation(570.0, THIRD_ROW_Y + 54.0)),
                Keyframe::new(0.72, Affine2D::from_translation(646.0, THIRD_ROW_Y + 92.0)),
                Keyframe::new(1.0, temporary_end_transform()),
            ],
        )?),
    )?;
    let opacity = AnimationPlan::track(
        visual.opacity(),
        Motion::Keyframes(Keyframes::new(
            TEMPORARY_DURATION,
            vec![
                Keyframe::new(0.0, Opacity::try_new(0.2)?),
                Keyframe::new(0.12, Opacity::OPAQUE),
                Keyframe::new(0.78, Opacity::OPAQUE),
                Keyframe::new(1.0, Opacity::TRANSPARENT),
            ],
        )?),
    )?;
    AnimationPlan::repeat(
        REPEAT_CYCLES,
        RepeatBehavior::Reverse,
        AnimationPlan::parallel(vec![transform, opacity])?,
    )
}

fn start_running(runtime: &mut Runtime, plan: AnimationPlan) -> AnimationId {
    match runtime
        .animations()
        .start(plan)
        .expect("animation admission")
    {
        AnimationStart::Running(animation) => animation,
        outcome => panic!("example animation must run, got {outcome:?}"),
    }
}
