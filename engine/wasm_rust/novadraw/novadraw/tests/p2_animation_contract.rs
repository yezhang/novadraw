//! External contracts for the P2-M01 animation clock and timeline core.

use std::{sync::Arc, time::Duration};

use novadraw::{
    Affine2D, Color, Dimension, FigureMeasurement, FigureTree, MonotonicTime, Rectangle,
    RectangleFigure, Runtime,
    animation::{
        AnimationError, AnimationMode, AnimationPlan, AnimationStart, AnimationState,
        AnimationSuppression, AnimationValue, Decay, Easing, InteractionGeometryPolicy,
        InterruptionPolicy, Keyframe, Keyframes, Motion, Opacity, RepeatBehavior, Spring,
        SuspensionPolicy, Tween,
    },
    figure::{FigureDrawing, FigurePresentation},
    graphics::{GraphicsError, PaintContext},
    render::{BackendCapabilities, Paint, RenderCommandKind, RenderOutcome, SurfaceInfo},
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

fn surface(width: u32, height: u32) -> SurfaceInfo {
    SurfaceInfo {
        logical_width: f64::from(width),
        logical_height: f64::from(height),
        pixel_width: width,
        pixel_height: height,
        scale_factor: 1.0,
    }
}

fn complete_frame(runtime: &mut Runtime, width: u32, height: u32) {
    let submission = runtime
        .prepare_submission(
            surface(width, height),
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    assert!(runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Presented
    ));
}

fn first_solid_alpha(commands: &[novadraw::render::RenderCommand]) -> f64 {
    commands
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::FillRect {
                paint: Paint::Solid(color),
                ..
            } => Some(color.alpha()),
            _ => None,
        })
        .expect("expected a solid fill")
}

struct SolidDrawing;

impl FigureDrawing for SolidDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::rgba(0.8, 0.2, 0.1, 1.0));
        context.fill_rect(Rectangle::new(0.0, 0.0, 10.0, 10.0))
    }
}

struct PanickingDrawing;

impl FigureDrawing for PanickingDrawing {
    fn paint(&self, _context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        panic!("animation drawing panic")
    }
}

#[test]
fn disabled_mode_suppresses_without_work_or_override() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    assert!(runtime.animations().set_mode(AnimationMode::Disabled));

    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(0.0, 10.0, Duration::from_millis(100))
                .unwrap()
                .with_easing(Easing::EaseInOut),
        ),
    )
    .unwrap();

    assert_eq!(
        runtime.animations().start(plan).unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::Disabled)
    );
    assert_eq!(runtime.animations().value(channel).unwrap(), 10.0);
    assert!(!runtime.animations().has_override(channel).unwrap());
    assert_eq!(runtime.animations().active_animation_count(), 0);
    assert_eq!(runtime.animations().next_wake_deadline(), None);

    let before = runtime.animations().stats();
    assert!(!runtime.advance_time(time(50)).unwrap());
    let after = runtime.animations().stats();
    assert_eq!(after.tracks_sampled, before.tracks_sampled);
}

#[test]
fn reduced_motion_uses_static_fallback_and_mode_change_cancels_active_work() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    assert!(runtime.animations().set_mode(AnimationMode::ReducedMotion));
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    assert_eq!(
        runtime.animations().start(plan).unwrap(),
        AnimationStart::Suppressed(AnimationSuppression::ReducedMotionStaticFallback)
    );
    assert_eq!(runtime.animations().value(channel).unwrap(), 10.0);

    assert!(runtime.animations().set_mode(AnimationMode::Enabled));
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("enabled animation must run");
    };
    assert!(runtime.animations().has_override(channel).unwrap());
    assert!(runtime.animations().set_mode(AnimationMode::Disabled));
    assert!(!runtime.animations().has_override(channel).unwrap());
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );
}

#[test]
fn tween_uses_absolute_monotonic_time_and_clears_override_at_completion() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();

    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("enabled animation must run");
    };
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Scheduled
    );
    assert_eq!(runtime.animations().value(channel).unwrap(), 0.0);

    assert!(!runtime.advance_time(time(0)).unwrap());
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Running
    );
    assert!(runtime.advance_time(time(50)).unwrap());
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);
    assert!(!runtime.advance_time(time(50)).unwrap());
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);

    assert!(runtime.advance_time(time(100)).unwrap());
    assert_eq!(runtime.animations().value(channel).unwrap(), 10.0);
    assert!(!runtime.animations().has_override(channel).unwrap());
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Completed
    );
    assert_eq!(runtime.animations().next_wake_deadline(), None);
}

#[test]
fn non_monotonic_time_preserves_animation_state() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    runtime.animations().start(plan).unwrap();

    runtime.advance_time(time(60)).unwrap();
    let before = runtime.animations().value(channel).unwrap();
    assert!(runtime.advance_time(time(40)).is_err());
    assert_eq!(runtime.animations().value(channel).unwrap(), before);
}

#[test]
fn parallel_sequence_and_stagger_preserve_stable_track_order() {
    let tween = |channel, end| {
        AnimationPlan::track(
            channel,
            Motion::Tween(Tween::between(0.0, end, Duration::from_millis(100)).unwrap()),
        )
        .unwrap()
    };

    let mut parallel_runtime = Runtime::empty();
    let parallel_a = parallel_runtime.animations().create_channel(10.0).unwrap();
    let parallel_b = parallel_runtime.animations().create_channel(20.0).unwrap();
    let parallel =
        AnimationPlan::parallel(vec![tween(parallel_a, 10.0), tween(parallel_b, 20.0)]).unwrap();
    parallel_runtime.animations().start(parallel).unwrap();
    parallel_runtime.advance_time(time(0)).unwrap();
    parallel_runtime.advance_time(time(50)).unwrap();
    assert_eq!(
        parallel_runtime.animations().value(parallel_a).unwrap(),
        5.0
    );
    assert_eq!(
        parallel_runtime.animations().value(parallel_b).unwrap(),
        10.0
    );

    let mut sequence_runtime = Runtime::empty();
    let sequence_a = sequence_runtime.animations().create_channel(10.0).unwrap();
    let sequence_b = sequence_runtime.animations().create_channel(20.0).unwrap();
    let sequence =
        AnimationPlan::sequence(vec![tween(sequence_a, 10.0), tween(sequence_b, 20.0)]).unwrap();
    sequence_runtime.animations().start(sequence).unwrap();
    sequence_runtime.advance_time(time(0)).unwrap();
    sequence_runtime.advance_time(time(50)).unwrap();
    assert_eq!(
        sequence_runtime.animations().value(sequence_a).unwrap(),
        5.0
    );
    assert_eq!(
        sequence_runtime.animations().value(sequence_b).unwrap(),
        0.0
    );
    sequence_runtime.advance_time(time(150)).unwrap();
    assert_eq!(
        sequence_runtime.animations().value(sequence_a).unwrap(),
        10.0
    );
    assert_eq!(
        sequence_runtime.animations().value(sequence_b).unwrap(),
        10.0
    );

    let mut stagger_runtime = Runtime::empty();
    let stagger_a = stagger_runtime.animations().create_channel(10.0).unwrap();
    let stagger_b = stagger_runtime.animations().create_channel(20.0).unwrap();
    let stagger = AnimationPlan::stagger(
        Duration::from_millis(50),
        vec![tween(stagger_a, 10.0), tween(stagger_b, 20.0)],
    )
    .unwrap();
    stagger_runtime.animations().start(stagger).unwrap();
    stagger_runtime.advance_time(time(0)).unwrap();
    stagger_runtime.advance_time(time(75)).unwrap();
    assert_eq!(stagger_runtime.animations().value(stagger_a).unwrap(), 7.5);
    assert_eq!(stagger_runtime.animations().value(stagger_b).unwrap(), 5.0);
}

#[test]
fn keyframes_sample_piecewise_values() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(0.0).unwrap();
    let keyframes = Keyframes::new(
        Duration::from_millis(100),
        vec![
            Keyframe::new(0.0, 0.0),
            Keyframe::new(0.5, 10.0),
            Keyframe::new(1.0, 0.0),
        ],
    )
    .unwrap();
    runtime
        .animations()
        .start(AnimationPlan::track(channel, Motion::Keyframes(keyframes)).unwrap())
        .unwrap();

    runtime.advance_time(time(0)).unwrap();
    runtime.advance_time(time(25)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);
    runtime.advance_time(time(75)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);
    runtime.advance_time(time(100)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 0.0);
}

#[test]
fn spring_and_decay_are_bounded_and_finish_at_committed_values() {
    let mut spring_runtime = Runtime::empty();
    let spring_channel = spring_runtime.animations().create_channel(10.0).unwrap();
    let spring = Spring::between(
        0.0,
        10.0,
        1.0,
        120.0,
        14.0,
        0.001,
        Duration::from_millis(1_000),
    )
    .unwrap();
    spring_runtime
        .animations()
        .start(AnimationPlan::track(spring_channel, Motion::Spring(spring)).unwrap())
        .unwrap();
    spring_runtime.advance_time(time(0)).unwrap();
    spring_runtime.advance_time(time(100)).unwrap();
    let spring_sample = spring_runtime.animations().value(spring_channel).unwrap();
    assert!(spring_sample > 0.0);
    spring_runtime.advance_time(time(1_000)).unwrap();
    assert_eq!(
        spring_runtime.animations().value(spring_channel).unwrap(),
        10.0
    );

    let mut decay_runtime = Runtime::empty();
    let decay_channel = decay_runtime.animations().create_channel(10.0).unwrap();
    let decay = Decay::between(0.0, 10.0, 5.0, Duration::from_millis(1_000)).unwrap();
    decay_runtime
        .animations()
        .start(AnimationPlan::track(decay_channel, Motion::Decay(decay)).unwrap())
        .unwrap();
    decay_runtime.advance_time(time(0)).unwrap();
    decay_runtime.advance_time(time(100)).unwrap();
    let decay_sample = decay_runtime.animations().value(decay_channel).unwrap();
    assert!(decay_sample > 0.0 && decay_sample < 10.0);
    decay_runtime.advance_time(time(1_000)).unwrap();
    assert_eq!(
        decay_runtime.animations().value(decay_channel).unwrap(),
        10.0
    );

    assert!(matches!(
        Spring::between(
            0.0,
            1.0,
            0.0,
            120.0,
            14.0,
            0.001,
            Duration::from_millis(100)
        ),
        Err(AnimationError::InvalidSpring)
    ));
    assert!(matches!(
        Decay::between(0.0, 1.0, 0.0, Duration::from_millis(100)),
        Err(AnimationError::InvalidDecay)
    ));
}

#[test]
fn finite_repeat_and_reverse_preserve_channel_continuity_and_terminal_truth() {
    let mut restart_runtime = Runtime::empty();
    let restart_channel = restart_runtime.animations().create_channel(10.0).unwrap();
    let restart_cycle = AnimationPlan::track(
        restart_channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let restart = AnimationPlan::repeat(2, RepeatBehavior::Restart, restart_cycle).unwrap();
    restart_runtime.animations().start(restart).unwrap();
    restart_runtime.advance_time(time(0)).unwrap();
    assert_eq!(
        restart_runtime.animations().value(restart_channel).unwrap(),
        0.0
    );
    restart_runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        restart_runtime.animations().value(restart_channel).unwrap(),
        0.0
    );
    restart_runtime.advance_time(time(150)).unwrap();
    assert_eq!(
        restart_runtime.animations().value(restart_channel).unwrap(),
        5.0
    );
    restart_runtime.advance_time(time(200)).unwrap();
    assert_eq!(
        restart_runtime.animations().value(restart_channel).unwrap(),
        10.0
    );

    let mut reverse_runtime = Runtime::empty();
    let reverse_channel = reverse_runtime.animations().create_channel(0.0).unwrap();
    let reverse_cycle = AnimationPlan::track(
        reverse_channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let reverse = AnimationPlan::repeat(2, RepeatBehavior::Reverse, reverse_cycle).unwrap();
    reverse_runtime.animations().start(reverse).unwrap();
    reverse_runtime.advance_time(time(0)).unwrap();
    assert_eq!(
        reverse_runtime.animations().value(reverse_channel).unwrap(),
        0.0
    );
    reverse_runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        reverse_runtime.animations().value(reverse_channel).unwrap(),
        10.0
    );
    reverse_runtime.advance_time(time(150)).unwrap();
    assert_eq!(
        reverse_runtime.animations().value(reverse_channel).unwrap(),
        5.0
    );
    reverse_runtime.advance_time(time(200)).unwrap();
    assert_eq!(
        reverse_runtime.animations().value(reverse_channel).unwrap(),
        0.0
    );
}

#[test]
fn cancel_clears_override_and_retarget_starts_from_current_presentation() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let first = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(first).unwrap() else {
        panic!("animation must run");
    };
    runtime.advance_time(time(0)).unwrap();
    runtime.advance_time(time(50)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);

    runtime.animations().set_committed(channel, 20.0).unwrap();
    let replacement = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::to(20.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    runtime
        .animations()
        .retarget(animation, replacement)
        .unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);

    runtime.advance_time(time(100)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 12.5);
    assert!(runtime.animations().cancel(animation).unwrap());
    assert_eq!(runtime.animations().value(channel).unwrap(), 20.0);
    assert!(!runtime.animations().has_override(channel).unwrap());
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );
}

#[test]
fn failed_retarget_keeps_existing_animation_unchanged() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let first = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(first).unwrap() else {
        panic!("animation must run");
    };
    runtime.advance_time(time(0)).unwrap();
    runtime.advance_time(time(40)).unwrap();
    let before = runtime.animations().value(channel).unwrap();

    let invalid = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(before, 99.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    assert_eq!(
        runtime.animations().retarget(animation, invalid),
        Err(AnimationError::FinalValueMismatch)
    );
    assert_eq!(runtime.animations().value(channel).unwrap(), before);
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Running
    );
}

#[test]
fn replace_and_ignore_resolve_channel_ownership_without_partial_state() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let plan = || {
        AnimationPlan::track(
            channel,
            Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
        )
        .unwrap()
    };
    let AnimationStart::Running(first) = runtime.animations().start(plan()).unwrap() else {
        panic!("animation must run");
    };

    let ignored = plan().with_interruption(InterruptionPolicy::Ignore);
    assert_eq!(
        runtime.animations().start(ignored).unwrap(),
        AnimationStart::Existing(first)
    );
    assert_eq!(runtime.animations().active_animation_count(), 1);

    let AnimationStart::Running(second) = runtime.animations().start(plan()).unwrap() else {
        panic!("replacement must run");
    };
    assert_ne!(first, second);
    assert_eq!(
        runtime.animations().state(first).unwrap(),
        AnimationState::Cancelled
    );
    assert_eq!(runtime.animations().active_animation_count(), 1);
}

#[test]
fn foreign_handles_and_overlapping_tracks_are_rejected() {
    let mut first_runtime = Runtime::empty();
    let mut second_runtime = Runtime::empty();
    let foreign_channel = first_runtime.animations().create_channel(10.0).unwrap();
    let foreign_plan = AnimationPlan::track(
        foreign_channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    assert_eq!(
        second_runtime.animations().start(foreign_plan),
        Err(AnimationError::ForeignChannel)
    );

    let channel = second_runtime.animations().create_channel(10.0).unwrap();
    let track = || {
        AnimationPlan::track(
            channel,
            Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
        )
        .unwrap()
    };
    assert!(matches!(
        AnimationPlan::parallel(vec![track(), track()]),
        Err(AnimationError::OverlappingTracks)
    ));
    assert!(matches!(
        AnimationPlan::repeat(4_097, RepeatBehavior::Restart, track()),
        Err(AnimationError::BudgetExceeded(_))
    ));
}

#[test]
fn removing_a_channel_cancels_its_owner_and_invalidates_the_handle() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("animation must run");
    };

    assert_eq!(runtime.animations().remove_channel(channel).unwrap(), 10.0);
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );
    assert_eq!(
        runtime.animations().value(channel),
        Err(AnimationError::UnknownChannel)
    );
}

#[derive(Clone, Debug, PartialEq)]
struct Percent(f64);

impl AnimationValue for Percent {
    fn is_valid(&self) -> bool {
        self.0.is_finite() && (0.0..=1.0).contains(&self.0)
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Self(self.0 + (target.0 - self.0) * progress)
    }
}

#[test]
fn external_value_type_uses_the_same_typed_channel_contract() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(Percent(1.0)).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(Percent(0.0), Percent(1.0), Duration::from_millis(100)).unwrap(),
        ),
    )
    .unwrap();
    runtime.animations().start(plan).unwrap();
    runtime.advance_time(time(0)).unwrap();
    runtime.advance_time(time(25)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), Percent(0.25));
}

#[derive(Clone, Debug, PartialEq)]
struct InvalidAt(f64, f64);

impl AnimationValue for InvalidAt {
    fn is_valid(&self) -> bool {
        self.0.is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        if (progress - self.1).abs() < f64::EPSILON {
            Self(f64::NAN, self.1)
        } else {
            Self(self.0 + (target.0 - self.0) * progress, self.1)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PanicAtQuarter(f64);

impl AnimationValue for PanicAtQuarter {
    fn is_valid(&self) -> bool {
        self.0.is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        assert!(
            (progress - 0.25).abs() >= f64::EPSILON,
            "animation provider panic"
        );
        Self(self.0 + (target.0 - self.0) * progress)
    }
}

#[test]
fn provider_failure_is_rejected_or_terminates_without_stale_override() {
    let mut admission_runtime = Runtime::empty();
    let channel = admission_runtime
        .animations()
        .create_channel(InvalidAt(1.0, 0.5))
        .unwrap();
    let invalid = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                InvalidAt(0.0, 0.5),
                InvalidAt(1.0, 0.5),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        admission_runtime.animations().start(invalid),
        Err(AnimationError::InvalidValue)
    );
    assert_eq!(admission_runtime.animations().active_animation_count(), 0);
    assert!(
        !admission_runtime
            .animations()
            .has_override(channel)
            .unwrap()
    );

    let mut sample_runtime = Runtime::empty();
    sample_runtime.advance_time(time(0)).unwrap();
    let channel = sample_runtime
        .animations()
        .create_channel(InvalidAt(1.0, 0.25))
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                InvalidAt(0.0, 0.25),
                InvalidAt(1.0, 0.25),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(animation) = sample_runtime.animations().start(plan).unwrap()
    else {
        panic!("animation must run");
    };
    assert!(sample_runtime.advance_time(time(25)).unwrap());
    assert_eq!(
        sample_runtime.animations().state(animation).unwrap(),
        AnimationState::Failed
    );
    assert!(!sample_runtime.animations().has_override(channel).unwrap());
    assert_eq!(
        sample_runtime.animations().value(channel).unwrap(),
        InvalidAt(1.0, 0.25)
    );
}

#[test]
fn provider_panic_faults_runtime_and_clears_presentation() {
    let mut runtime = Runtime::empty();
    runtime.advance_time(time(0)).unwrap();
    let channel = runtime
        .animations()
        .create_channel(PanicAtQuarter(1.0))
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                PanicAtQuarter(0.0),
                PanicAtQuarter(1.0),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animations().start(plan).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.advance_time(time(25)).unwrap();
    }));
    assert!(panic.is_err());
    assert!(runtime.is_faulted());
    assert!(!runtime.animations().has_override(channel).unwrap());
}

#[test]
fn figure_opacity_and_transform_are_presentation_only() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            20.0,
            30.0,
            40.0,
            20.0,
            Color::rgba(0.2, 0.4, 0.8, 1.0),
        )))
        .unwrap();
    complete_frame(&mut runtime, 200, 120);
    runtime.advance_time(time(0)).unwrap();

    let channels = runtime
        .animations()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap();
    let plan = AnimationPlan::parallel(vec![
        AnimationPlan::track(
            channels.opacity(),
            Motion::Tween(
                Tween::between(
                    Opacity::TRANSPARENT,
                    Opacity::OPAQUE,
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
        AnimationPlan::track(
            channels.transform(),
            Motion::Tween(
                Tween::between(
                    Affine2D::from_translation(-20.0, 0.0),
                    Affine2D::IDENTITY,
                    Duration::from_millis(100),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    ])
    .unwrap();
    runtime.animations().start(plan).unwrap();

    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Some(Rectangle::new(20.0, 30.0, 40.0, 20.0))
    );
    assert_eq!(runtime.tree().resolved_style(figure).unwrap().alpha, 1.0);

    runtime.advance_time(time(50)).unwrap();
    let frame = runtime.record_full_frame();
    assert_eq!(first_solid_alpha(frame.commands()), 0.5);
    assert!(frame.commands().iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::ConcatTransform { matrix }
                if matrix == Affine2D::from_translation(-10.0, 0.0)
        )
    }));
    assert_eq!(
        runtime.tree().figure_bounds(figure),
        Some(Rectangle::new(20.0, 30.0, 40.0, 20.0))
    );
}

#[test]
fn presentation_damage_unions_old_and_new_surface_envelopes() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(20.0, 10.0, 10.0, 10.0)))
        .unwrap();
    complete_frame(&mut runtime, 100, 100);
    runtime.advance_time(time(0)).unwrap();
    let channel = runtime
        .animations()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                Affine2D::from_translation(-20.0, 0.0),
                Affine2D::IDENTITY,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animations().start(plan).unwrap();

    let submission = runtime
        .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(
        submission.damage.union(),
        Some(Rectangle::new(0.0, 10.0, 30.0, 10.0))
    );
}

#[test]
fn child_transform_override_bypasses_only_the_committed_child_clip() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = tree
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.advance_time(time(0)).unwrap();
    let transform = runtime
        .animations()
        .bind_figure(child, InteractionGeometryPolicy::Committed)
        .unwrap()
        .transform();
    let plan = AnimationPlan::track(
        transform,
        Motion::Tween(
            Tween::between(
                Affine2D::from_translation(30.0, 0.0),
                Affine2D::IDENTITY,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    runtime.animations().start(plan).unwrap();

    let frame = runtime.record_full_frame();
    assert!(!frame.commands().iter().any(|command| {
        matches!(
            command.kind,
            RenderCommandKind::Clip { rect }
                if rect == Rectangle::new(10.0, 10.0, 20.0, 20.0)
        )
    }));
    assert_eq!(
        runtime.tree().hit_test_simple((15.0, 15.0)),
        Some(child),
        "committed interaction geometry must not follow the presentation transform"
    );
}

#[test]
fn temporary_visual_is_non_source_and_removed_with_its_owner() {
    let mut runtime = Runtime::empty();
    complete_frame(&mut runtime, 100, 100);
    runtime.advance_time(time(0)).unwrap();
    let presentation = FigurePresentation::new(
        FigureMeasurement::new(10.0, 10.0, None),
        Dimension::new(10.0, 10.0),
        Rectangle::new(0.0, 0.0, 10.0, 10.0),
        Arc::new(SolidDrawing),
    )
    .unwrap();
    let visual = runtime
        .animations()
        .create_temporary_visual(presentation, Affine2D::from_translation(10.0, 20.0), 0.0)
        .unwrap();
    assert_eq!(runtime.animations().temporary_visual_count(), 1);
    assert!(!runtime.has_pending_update());

    let plan = AnimationPlan::track(
        visual.opacity(),
        Motion::Tween(
            Tween::between(
                Opacity::OPAQUE,
                Opacity::TRANSPARENT,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("temporary visual must activate with its plan");
    };
    let frame = runtime.record_full_frame();
    assert_eq!(first_solid_alpha(frame.commands()), 1.0);

    runtime.advance_time(time(50)).unwrap();
    let frame = runtime.record_full_frame();
    assert_eq!(first_solid_alpha(frame.commands()), 0.5);
    assert!(runtime.animations().cancel(animation).unwrap());
    assert_eq!(runtime.animations().temporary_visual_count(), 0);
    assert_eq!(
        runtime.animations().value(visual.opacity()),
        Err(AnimationError::UnknownChannel)
    );
    assert!(
        runtime
            .record_full_frame()
            .commands()
            .iter()
            .all(|command| !matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
}

#[test]
fn disposing_a_figure_cancels_bound_presentation_without_source_revival() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    runtime.advance_time(time(0)).unwrap();
    let channel = runtime
        .animations()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .opacity();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                Opacity::TRANSPARENT,
                Opacity::OPAQUE,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("animation must run");
    };

    runtime.dispose_subtree(figure).unwrap();
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );
    assert_eq!(
        runtime.animations().value(channel),
        Err(AnimationError::UnknownChannel)
    );
    assert!(!runtime.tree().is_attached(figure));
}

#[test]
fn pause_policy_excludes_surface_suspension_from_local_time() {
    let mut runtime = Runtime::empty();
    let channel = runtime.animations().create_channel(10.0).unwrap();
    runtime.advance_time(time(0)).unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 10.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap()
    .with_suspension(SuspensionPolicy::Pause);
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("animation must run");
    };

    assert!(matches!(
        runtime.prepare_submission(surface(0, 100), BackendCapabilities::RETAINED_PARTIAL),
        novadraw::FramePreparation::Suspended
    ));
    runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Paused
    );
    assert_eq!(runtime.animations().value(channel).unwrap(), 0.0);

    let submission = runtime
        .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Presented,
    );
    runtime.advance_time(time(150)).unwrap();
    assert_eq!(runtime.animations().value(channel).unwrap(), 5.0);
}

#[test]
fn hidden_figure_applies_pause_or_finish_policy_without_source_mutation() {
    let mut paused_runtime = Runtime::empty();
    let paused_figure = paused_runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    paused_runtime.advance_time(time(0)).unwrap();
    let paused_channel = paused_runtime
        .animations()
        .bind_figure(paused_figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .opacity();
    let paused_plan = AnimationPlan::track(
        paused_channel,
        Motion::Tween(
            Tween::between(
                Opacity::TRANSPARENT,
                Opacity::OPAQUE,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_suspension(SuspensionPolicy::Pause);
    let AnimationStart::Running(paused) = paused_runtime.animations().start(paused_plan).unwrap()
    else {
        panic!("animation must run");
    };
    paused_runtime
        .figure(paused_figure)
        .unwrap()
        .set_visible(false)
        .unwrap();
    assert_eq!(
        paused_runtime.animations().state(paused).unwrap(),
        AnimationState::Paused
    );
    assert_eq!(paused_runtime.animations().next_wake_deadline(), None);
    paused_runtime.advance_time(time(50)).unwrap();
    assert_eq!(
        paused_runtime.animations().value(paused_channel).unwrap(),
        Opacity::TRANSPARENT
    );
    paused_runtime
        .figure(paused_figure)
        .unwrap()
        .set_visible(true)
        .unwrap();
    paused_runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        paused_runtime.animations().value(paused_channel).unwrap(),
        Opacity::try_new(0.5).unwrap()
    );

    let mut finish_runtime = Runtime::empty();
    let finish_figure = finish_runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let finish_channel = finish_runtime
        .animations()
        .bind_figure(finish_figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .opacity();
    let finish_plan = AnimationPlan::track(
        finish_channel,
        Motion::Tween(
            Tween::between(
                Opacity::TRANSPARENT,
                Opacity::OPAQUE,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap()
    .with_suspension(SuspensionPolicy::Finish);
    let AnimationStart::Running(finished) = finish_runtime.animations().start(finish_plan).unwrap()
    else {
        panic!("animation must run");
    };
    finish_runtime
        .figure(finish_figure)
        .unwrap()
        .set_visible(false)
        .unwrap();
    assert_eq!(
        finish_runtime.animations().state(finished).unwrap(),
        AnimationState::Completed
    );
    assert!(
        !finish_runtime
            .animations()
            .has_override(finish_channel)
            .unwrap()
    );
}

#[test]
fn temporary_visual_is_removed_on_natural_completion() {
    let mut runtime = Runtime::empty();
    runtime.advance_time(time(0)).unwrap();
    let presentation = FigurePresentation::new(
        FigureMeasurement::new(10.0, 10.0, None),
        Dimension::new(10.0, 10.0),
        Rectangle::new(0.0, 0.0, 10.0, 10.0),
        Arc::new(SolidDrawing),
    )
    .unwrap();
    let visual = runtime
        .animations()
        .create_temporary_visual(presentation, Affine2D::IDENTITY, 0.0)
        .unwrap();
    let plan = AnimationPlan::track(
        visual.opacity(),
        Motion::Tween(
            Tween::between(
                Opacity::OPAQUE,
                Opacity::TRANSPARENT,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(animation) = runtime.animations().start(plan).unwrap() else {
        panic!("animation must run");
    };
    runtime.advance_time(time(100)).unwrap();
    assert_eq!(
        runtime.animations().state(animation).unwrap(),
        AnimationState::Completed
    );
    assert_eq!(runtime.animations().temporary_visual_count(), 0);
    assert_eq!(
        runtime.animations().value(visual.opacity()),
        Err(AnimationError::UnknownChannel)
    );
}

#[test]
fn retry_keeps_current_presentation_and_provider_panic_clears_it() {
    let mut retry_runtime = Runtime::empty();
    let figure = retry_runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    complete_frame(&mut retry_runtime, 100, 100);
    retry_runtime.advance_time(time(0)).unwrap();
    let opacity = retry_runtime
        .animations()
        .bind_figure(figure, InteractionGeometryPolicy::Committed)
        .unwrap()
        .opacity();
    let plan = AnimationPlan::track(
        opacity,
        Motion::Tween(
            Tween::between(
                Opacity::TRANSPARENT,
                Opacity::OPAQUE,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    retry_runtime.animations().start(plan).unwrap();
    retry_runtime.advance_time(time(50)).unwrap();
    let first = retry_runtime
        .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(first_solid_alpha(&first.commands), 0.5);
    assert!(retry_runtime.complete_submission(
        first.session_id,
        first.frame_id,
        RenderOutcome::Retry
    ));
    let retried = retry_runtime
        .prepare_submission(surface(100, 100), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(first_solid_alpha(&retried.commands), 0.5);

    let mut fault_runtime = Runtime::empty();
    fault_runtime.advance_time(time(0)).unwrap();
    let presentation = FigurePresentation::new(
        FigureMeasurement::new(10.0, 10.0, None),
        Dimension::new(10.0, 10.0),
        Rectangle::new(0.0, 0.0, 10.0, 10.0),
        Arc::new(PanickingDrawing),
    )
    .unwrap();
    let visual = fault_runtime
        .animations()
        .create_temporary_visual(presentation, Affine2D::IDENTITY, 0.0)
        .unwrap();
    let plan = AnimationPlan::track(
        visual.opacity(),
        Motion::Tween(
            Tween::between(
                Opacity::OPAQUE,
                Opacity::TRANSPARENT,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let AnimationStart::Running(animation) = fault_runtime.animations().start(plan).unwrap() else {
        panic!("animation must run");
    };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fault_runtime.record_full_frame();
    }));
    assert!(panic.is_err());
    assert!(fault_runtime.is_faulted());
    assert_eq!(fault_runtime.animations().temporary_visual_count(), 0);
    assert_eq!(
        fault_runtime.animations().state(animation).unwrap(),
        AnimationState::Cancelled
    );
}

#[test]
fn unsupported_presentation_hit_testing_is_rejected_before_channel_creation() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    assert_eq!(
        runtime
            .animations()
            .bind_figure(figure, InteractionGeometryPolicy::Presentation),
        Err(AnimationError::UnsupportedInteractionGeometry)
    );
    assert_eq!(
        runtime
            .animations()
            .bind_figure(figure, InteractionGeometryPolicy::NonInteractive),
        Err(AnimationError::UnsupportedInteractionGeometry)
    );
}
