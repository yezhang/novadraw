use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use novadraw::{
    animation::{
        AnimationBudgetKind, AnimationChannel, AnimationError, AnimationId, AnimationLimits,
        AnimationPlan, AnimationReservations, AnimationStart, AnimationState, AnimationValue,
        Motion, Tween,
    },
    event::MonotonicTime,
    runtime::Runtime,
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_micros(milliseconds * 1_000)
}

fn track(channel: AnimationChannel<f64>) -> AnimationPlan {
    AnimationPlan::track(
        channel,
        Motion::Tween(Tween::between(0.0, 1.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap()
}

fn owner(start: AnimationStart) -> AnimationId {
    match start {
        AnimationStart::Running(id) => id,
        other => panic!("expected active owner, got {other:?}"),
    }
}

fn limits(tracks: usize) -> AnimationLimits {
    AnimationLimits {
        active_animations: 2,
        active_tracks: tracks,
        staging_tracks: 4,
    }
}

#[test]
fn full_capacity_rejects_new_work_and_downsize_without_evicting_either_order() {
    for reverse in [false, true] {
        let mut runtime = Runtime::empty();
        runtime.animation().advance_time(time(0)).unwrap();
        runtime
            .animation()
            .timeline()
            .set_limits(limits(2))
            .unwrap();
        let a = runtime.animation().timeline().create_channel(1.0).unwrap();
        let b = runtime.animation().timeline().create_channel(1.0).unwrap();
        let c = runtime.animation().timeline().create_channel(1.0).unwrap();
        let ordered = if reverse { [b, a] } else { [a, b] };
        let owners = ordered.map(|channel| {
            owner(
                runtime
                    .animation()
                    .timeline()
                    .start(track(channel))
                    .unwrap(),
            )
        });
        runtime.animation().advance_time(time(50)).unwrap();
        let wake = runtime.animation().next_wake_deadline();
        let version = runtime.animation().timeline().sample_version();
        let reserved = AnimationReservations {
            active_animations: 2,
            active_tracks: 2,
        };
        assert_eq!(runtime.animation().timeline().reservations(), reserved);
        assert!(matches!(
            runtime.animation().timeline().start(track(c)),
            Err(AnimationError::BudgetExceeded(_))
        ));
        assert_eq!(
            runtime.animation().timeline().set_limits(limits(1)),
            Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks
            ))
        );
        assert_eq!(runtime.animation().timeline().limits(), limits(2));
        assert_eq!(runtime.animation().timeline().reservations(), reserved);
        assert_eq!(runtime.animation().timeline().sample_version(), version);
        assert_eq!(runtime.animation().next_wake_deadline(), wake);
        for channel in [a, b] {
            assert_eq!(runtime.animation().timeline().value(channel).unwrap(), 0.5);
            assert!(
                runtime
                    .animation()
                    .timeline()
                    .has_override(channel)
                    .unwrap()
            );
        }
        for id in owners {
            assert_eq!(
                runtime.animation().timeline().state(id).unwrap(),
                AnimationState::Running
            );
        }
        runtime.animation().advance_time(time(100)).unwrap();
        assert_eq!(
            runtime.animation().timeline().reservations(),
            AnimationReservations::default()
        );
        runtime
            .animation()
            .timeline()
            .set_limits(limits(1))
            .unwrap();
        assert!(matches!(
            runtime.animation().timeline().start(track(c)),
            Ok(AnimationStart::Running(_))
        ));
    }
}

#[test]
fn net_retarget_at_capacity_and_independent_staging_rejection_keep_current_time() {
    let mut runtime = Runtime::empty();
    runtime.animation().advance_time(time(0)).unwrap();
    runtime
        .animation()
        .timeline()
        .set_limits(limits(2))
        .unwrap();
    let a = runtime.animation().timeline().create_channel(1.0).unwrap();
    let b = runtime.animation().timeline().create_channel(1.0).unwrap();
    let id = owner(runtime.animation().timeline().start(track(a)).unwrap());
    let other = owner(runtime.animation().timeline().start(track(b)).unwrap());
    runtime.animation().advance_time(time(50)).unwrap();
    let before = runtime.animation().timeline().sample_version();
    let replacement = AnimationPlan::track(
        a,
        Motion::Tween(Tween::to(1.0, Duration::from_millis(100)).unwrap()),
    )
    .unwrap();
    runtime
        .animation()
        .timeline()
        .retarget(id, replacement.clone())
        .unwrap();
    let replaced = runtime.animation().timeline().sample_version();
    assert_eq!(replaced.time, before.time);
    assert!(replaced.active_revision > before.active_revision);
    assert_eq!(runtime.animation().timeline().value(a).unwrap(), 0.5);
    let reservations = runtime.animation().timeline().reservations();
    let wake = runtime.animation().next_wake_deadline();
    let mut no_staging = limits(2);
    no_staging.staging_tracks = 0;
    runtime
        .animation()
        .timeline()
        .set_limits(no_staging)
        .unwrap();
    assert_eq!(
        runtime.animation().timeline().retarget(id, replacement),
        Err(AnimationError::BudgetExceeded(
            AnimationBudgetKind::StagingTracks
        ))
    );
    assert_eq!(runtime.animation().timeline().sample_version(), replaced);
    assert_eq!(runtime.animation().timeline().reservations(), reservations);
    assert_eq!(runtime.animation().next_wake_deadline(), wake);
    assert_eq!(runtime.animation().timeline().value(a).unwrap(), 0.5);
    assert_eq!(runtime.animation().timeline().value(b).unwrap(), 0.5);
    assert_eq!(
        runtime.animation().timeline().state(other).unwrap(),
        AnimationState::Running
    );
    runtime.animation().advance_time(time(75)).unwrap();
    assert_eq!(runtime.animation().timeline().value(a).unwrap(), 0.625);
    assert_eq!(runtime.animation().timeline().value(b).unwrap(), 0.75);
}

#[derive(Clone, Debug)]
struct ObservedValue {
    value: f64,
    samples: Arc<AtomicUsize>,
    reject_call: usize,
}

impl PartialEq for ObservedValue {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl AnimationValue for ObservedValue {
    fn is_valid(&self) -> bool {
        self.value.is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        let call = self.samples.fetch_add(1, Ordering::SeqCst) + 1;
        Self {
            value: if call == self.reject_call {
                f64::NAN
            } else {
                self.value + (target.value - self.value) * progress
            },
            ..self.clone()
        }
    }
}

#[test]
fn repeated_time_and_pull_queries_do_not_sample_and_scheduled_stays_unanchored() {
    let mut runtime = Runtime::empty();
    let samples = Arc::new(AtomicUsize::new(0));
    let end = ObservedValue {
        value: 1.0,
        samples: samples.clone(),
        reject_call: usize::MAX,
    };
    let channel = runtime
        .animation()
        .timeline()
        .create_channel(end.clone())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                ObservedValue {
                    value: 0.0,
                    ..end.clone()
                },
                end,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let id = owner(runtime.animation().timeline().start(plan).unwrap());
    assert_eq!(runtime.animation().timeline().sample_version().time, None);
    assert_eq!(
        runtime.animation().timeline().state(id).unwrap(),
        AnimationState::Scheduled
    );
    runtime.animation().advance_time(time(30)).unwrap();
    runtime.animation().advance_time(time(80)).unwrap();
    let calls = samples.load(Ordering::SeqCst);
    let counters = runtime.animation().timeline().stats();
    let version = runtime.animation().timeline().sample_version();
    for _ in 0..3 {
        assert!(!runtime.animation().advance_time(time(80)).unwrap());
        assert_eq!(
            runtime.animation().timeline().value(channel).unwrap().value,
            0.5
        );
        assert_eq!(runtime.animation().timeline().sample_version(), version);
    }
    assert_eq!(samples.load(Ordering::SeqCst), calls);
    assert_eq!(runtime.animation().timeline().stats(), counters);
    assert!(runtime.animation().advance_time(time(70)).is_err());
    assert_eq!(runtime.animation().timeline().sample_version(), version);
    assert_eq!(samples.load(Ordering::SeqCst), calls);
}

#[test]
fn initial_candidate_failure_preserves_old_owner_values_and_reservation() {
    let mut runtime = Runtime::empty();
    runtime.animation().advance_time(time(0)).unwrap();
    let samples = Arc::new(AtomicUsize::new(0));
    let end = ObservedValue {
        value: 1.0,
        samples: samples.clone(),
        reject_call: usize::MAX,
    };
    let channel = runtime
        .animation()
        .timeline()
        .create_channel(end.clone())
        .unwrap();
    let plan = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                ObservedValue {
                    value: 0.0,
                    ..end.clone()
                },
                end.clone(),
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let id = owner(runtime.animation().timeline().start(plan).unwrap());
    runtime.animation().advance_time(time(50)).unwrap();
    let version = runtime.animation().timeline().sample_version();
    let reservations = runtime.animation().timeline().reservations();
    let wake = runtime.animation().next_wake_deadline();
    let candidate = AnimationPlan::track(
        channel,
        Motion::Tween(
            Tween::between(
                ObservedValue {
                    value: 0.5,
                    // Reject the initial publication after endpoint validation, exposing a failure
                    // that must still happen before owner transfer.
                    reject_call: 4,
                    ..end.clone()
                },
                end,
                Duration::from_millis(100),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    samples.store(0, Ordering::SeqCst);
    assert_eq!(
        runtime
            .animation()
            .timeline()
            .retarget(id, candidate.clone()),
        Err(AnimationError::InvalidValue)
    );
    samples.store(0, Ordering::SeqCst);
    assert_eq!(
        runtime.animation().timeline().start(candidate),
        Err(AnimationError::InvalidValue)
    );
    assert_eq!(runtime.animation().timeline().sample_version(), version);
    assert_eq!(runtime.animation().timeline().reservations(), reservations);
    assert_eq!(runtime.animation().next_wake_deadline(), wake);
    assert_eq!(
        runtime.animation().timeline().value(channel).unwrap().value,
        0.5
    );
    assert_eq!(
        runtime.animation().timeline().state(id).unwrap(),
        AnimationState::Running
    );
    runtime.animation().advance_time(time(75)).unwrap();
    assert_eq!(
        runtime.animation().timeline().value(channel).unwrap().value,
        0.75
    );
}
