//! Host-owned recovery example. The caller holds exclusive orchestration access
//! throughout recovery and resumes input/backend completion only after success.
use novadraw::{
    event::{ListenerScope, ObservationCheckpoint},
    runtime::{FramePreparationError, Runtime, RuntimeMutationError, StableQueryError},
};
use novadraw_inspector::{
    FigureInspector, FigureTreeSnapshot, InspectionSubscription, InspectorError,
};
use std::{
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub struct RecoveryBaseline {
    pub inspector: FigureInspector,
    pub subscriptions: Vec<InspectionSubscription>,
    pub checkpoint: ObservationCheckpoint,
    pub snapshot: FigureTreeSnapshot,
}

#[derive(Debug)]
pub enum RecoveryCause {
    Cleanup,
    Preparation(FramePreparationError),
    Checkpoint(StableQueryError),
    Registration(InspectorError),
    Snapshot(StableQueryError),
    Changed {
        before: ObservationCheckpoint,
        after: ObservationCheckpoint,
    },
    CallbackPanic,
}

#[derive(Debug)]
pub struct RecoveryFailure {
    pub cause: RecoveryCause,
    pub cleanup_errors: Vec<RuntimeMutationError>,
}

impl std::fmt::Display for RecoveryCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cleanup => f.write_str("old subscription cleanup failed"),
            Self::Preparation(error) => write!(f, "CPU stabilization: {error}"),
            Self::Checkpoint(error) => write!(f, "observation checkpoint: {error}"),
            Self::Registration(error) => write!(f, "subscription registration: {error}"),
            Self::Snapshot(error) => write!(f, "tree snapshot: {error}"),
            Self::Changed { before, after } => {
                write!(f, "checkpoint changed: {before:?} -> {after:?}")
            }
            Self::CallbackPanic => f.write_str("external callback panicked"),
        }
    }
}

impl std::error::Error for RecoveryCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preparation(error) => Some(error),
            Self::Checkpoint(error) | Self::Snapshot(error) => Some(error),
            Self::Registration(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Display for RecoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; cleanup errors: {}",
            self.cause,
            self.cleanup_errors.len()
        )
    }
}

impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

fn cleanup(runtime: &mut Runtime, tokens: &[InspectionSubscription]) -> Vec<RuntimeMutationError> {
    tokens
        .iter()
        .filter_map(|token| token.detach(runtime).err())
        .collect()
}

pub fn recover(
    runtime: &mut Runtime,
    old: &[InspectionSubscription],
    scopes: &[ListenerScope],
    capacity: NonZeroUsize,
) -> Result<RecoveryBaseline, RecoveryFailure> {
    recover_with_capture(runtime, old, scopes, capacity, |inspector, runtime| {
        inspector.capture_tree(runtime)
    })
}

// A consumer-side capture boundary also allows tests to model host violations
// and product snapshot errors without adding any engine fault injection API.
fn recover_with_capture(
    runtime: &mut Runtime,
    old: &[InspectionSubscription],
    scopes: &[ListenerScope],
    capacity: NonZeroUsize,
    capture: impl FnOnce(&FigureInspector, &mut Runtime) -> Result<FigureTreeSnapshot, StableQueryError>,
) -> Result<RecoveryBaseline, RecoveryFailure> {
    let errors = cleanup(runtime, old);
    if !errors.is_empty() {
        return Err(RecoveryFailure {
            cause: RecoveryCause::Cleanup,
            cleanup_errors: errors,
        });
    }
    let mut tokens = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| {
        runtime
            .frames()
            .stabilize_for_query()
            .map_err(RecoveryCause::Preparation)?;
        let before = runtime
            .observations()
            .checkpoint()
            .map_err(RecoveryCause::Checkpoint)?;
        let inspector = FigureInspector::new(capacity);
        for scope in scopes {
            tokens.push(
                inspector
                    .attach_scoped(runtime, *scope)
                    .map_err(RecoveryCause::Registration)?,
            );
        }
        let snapshot = capture(&inspector, runtime).map_err(RecoveryCause::Snapshot)?;
        let after = runtime
            .observations()
            .checkpoint()
            .map_err(RecoveryCause::Checkpoint)?;
        if before != after {
            return Err(RecoveryCause::Changed { before, after });
        }
        Ok((inspector, before, snapshot))
    }));
    match result {
        Ok(Ok((inspector, checkpoint, snapshot))) => Ok(RecoveryBaseline {
            inspector,
            subscriptions: tokens,
            checkpoint,
            snapshot,
        }),
        failure => Err(RecoveryFailure {
            cause: match failure {
                Ok(Err(cause)) => cause,
                Err(_) => RecoveryCause::CallbackPanic,
                Ok(Ok(_)) => unreachable!(),
            },
            cleanup_errors: cleanup(runtime, &tokens),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        event::{
            ListenerDirective, NotificationEffect, NotificationRecord, ObservationListener,
            UpdateEvent,
        },
        figure::RectangleFigure,
        geometry::Rectangle,
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, StableSceneQuery},
        tree::FigureId,
    };
    use std::{cell::RefCell, rc::Rc};

    fn scene() -> (Runtime, FigureId) {
        let mut runtime = Runtime::empty();
        let figure = runtime
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap(),
            ))
            .unwrap();
        (runtime, figure)
    }
    fn capacity() -> NonZeroUsize {
        NonZeroUsize::new(32).unwrap()
    }
    fn change(runtime: &mut Runtime, figure: FigureId) {
        runtime
            .figure(figure)
            .unwrap()
            .set_bounds(Rectangle::new(3.0, 4.0, 20.0, 20.0))
            .unwrap();
    }
    fn surface() -> SurfaceInfo {
        SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn checkpoint_query_is_pure_and_cpu_drive_does_not_require_a_frame() {
        let mut empty = Runtime::empty();
        let initial = empty.observations().checkpoint().unwrap();
        assert_eq!(initial.last_published_sequence, None);
        assert_eq!(empty.observations().checkpoint().unwrap(), initial);
        let (mut runtime, figure) = scene();
        let inspector = FigureInspector::new(capacity());
        let token = inspector.attach(&mut runtime).unwrap();
        assert!(matches!(
            runtime.observations().checkpoint(),
            Err(StableQueryError::NotStable { .. })
        ));
        assert!(inspector.events().unwrap().is_empty());
        runtime.frames().stabilize_for_query().unwrap();
        let checkpoint = runtime.observations().checkpoint().unwrap();
        assert!(checkpoint.last_published_sequence.is_some());
        assert!(inspector.events().unwrap().iter().all(|event| !matches!(
            event.effect,
            NotificationEffect::EmitUpdate(
                UpdateEvent::Painting { .. }
                    | UpdateEvent::Painted { .. }
                    | UpdateEvent::Prepared { .. }
                    | UpdateEvent::Submitted { .. }
            )
        )));
        assert_eq!(
            inspector
                .capture_tree(&mut runtime)
                .unwrap()
                .figure(figure)
                .unwrap()
                .bounds,
            Rectangle::new(0.0, 0.0, 20.0, 20.0)
        );
        inspector.clear_events().unwrap();
        token.detach(&mut runtime).unwrap();
        assert_eq!(runtime.observations().checkpoint().unwrap(), checkpoint);
        assert!(matches!(
            runtime
                .frames()
                .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Ready(_)
        ));
    }

    #[test]
    fn detach_retries_foreign_then_correct_owner_and_dispose_is_already_removed() {
        let (mut runtime, figure) = scene();
        let (mut foreign, foreign_figure) = scene();
        let inspector = FigureInspector::new(capacity());
        let token = inspector.attach(&mut runtime).unwrap();
        assert!(matches!(
            token.detach(&mut foreign),
            Err(RuntimeMutationError::ForeignListener(_))
        ));
        assert!(token.detach(&mut runtime).unwrap());
        assert!(!token.detach(&mut runtime).unwrap());
        assert!(matches!(
            inspector.attach_scoped(&mut runtime, ListenerScope::Figure(foreign_figure)),
            Err(InspectorError::Registration(
                RuntimeMutationError::ForeignRuntime(_)
            ))
        ));
        let scoped = inspector
            .attach_scoped(&mut runtime, ListenerScope::Figure(figure))
            .unwrap();
        runtime.dispose_subtree(figure).unwrap();
        assert!(!scoped.detach(&mut runtime).unwrap());
        assert!(matches!(
            inspector.attach_scoped(&mut runtime, ListenerScope::Figure(figure)),
            Err(InspectorError::Registration(
                RuntimeMutationError::UnknownOrDisposedFigure(_)
            ))
        ));
    }

    #[test]
    fn in_flight_frame_is_preserved_and_delayed_completion_follows_baseline() {
        let (mut runtime, _) = scene();
        let FramePreparation::Ready(submission) = runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        else {
            panic!("initial submission")
        };
        let baseline = recover(&mut runtime, &[], &[ListenerScope::Runtime], capacity()).unwrap();
        assert!(baseline.inspector.events().unwrap().is_empty());
        let _ack = runtime
            .frames()
            .complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        let events = baseline.inspector.events().unwrap();
        assert!(events.iter().any(|event| matches!(event.effect, NotificationEffect::EmitUpdate(UpdateEvent::Submitted { frame_id, .. }) if frame_id == submission.frame_id)));
        assert!(
            events
                .iter()
                .all(|event| Some(event.sequence) > baseline.checkpoint.last_published_sequence)
        );
        cleanup(&mut runtime, &baseline.subscriptions);
    }

    #[test]
    fn backend_recovery_damage_does_not_preempt_checkpoint_but_actual_fault_does() {
        use novadraw::render::{RenderFailure, RenderFailureDiagnostic};
        for terminal in [false, true] {
            let (mut runtime, figure) = scene();
            let submission = runtime
                .frames()
                .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
                .into_ready()
                .unwrap();
            let baseline =
                recover(&mut runtime, &[], &[ListenerScope::Runtime], capacity()).unwrap();
            assert!(runtime.observations().checkpoint().is_ok());
            let failure = if terminal {
                RenderFailure::Fatal(RenderFailureDiagnostic::new("terminal backend"))
            } else {
                RenderFailure::SurfaceLost(RenderFailureDiagnostic::new("recoverable surface"))
            };
            runtime
                .frames()
                .complete_submission(submission.session_id, submission.frame_id, Err(failure))
                .unwrap();
            if terminal {
                // A backend Terminated acknowledgement alone does not fault the CPU owner.
                assert!(runtime.observations().checkpoint().is_ok());
                struct Panic;
                impl ObservationListener for Panic {
                    fn observed(
                        &self,
                        _: &NotificationRecord,
                        _: StableSceneQuery<'_>,
                    ) -> ListenerDirective {
                        panic!("actual owner fault after backend acknowledgement");
                    }
                }
                runtime
                    .observations()
                    .add_observation_listener(Box::new(Panic))
                    .unwrap();
                runtime.figure(figure).unwrap().set_enabled(false).unwrap();
                assert!(
                    catch_unwind(AssertUnwindSafe(|| runtime.frames().stabilize_for_query()))
                        .is_err()
                );
                assert_eq!(
                    runtime.observations().checkpoint(),
                    Err(StableQueryError::Faulted)
                );
                let rejected = recover(
                    &mut runtime,
                    &baseline.subscriptions,
                    &[ListenerScope::Runtime],
                    capacity(),
                )
                .err()
                .unwrap();
                assert!(matches!(
                    rejected.cause,
                    RecoveryCause::Preparation(FramePreparationError::Faulted)
                ));
                assert!(rejected.cleanup_errors.is_empty());
                assert!(!baseline.subscriptions[0].detach(&mut runtime).unwrap());
            } else {
                assert!(runtime.observations().checkpoint().is_ok());
                let fresh = recover(
                    &mut runtime,
                    &baseline.subscriptions,
                    &[ListenerScope::Runtime],
                    capacity(),
                )
                .unwrap();
                assert!(fresh.inspector.events().unwrap().is_empty());
                // Recovery never consumed the still-required full repaint.
                assert!(
                    runtime
                        .frames()
                        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
                        .into_ready()
                        .is_ok()
                );
                cleanup(&mut runtime, &fresh.subscriptions);
            }
        }
    }

    #[test]
    fn idle_suspended_and_old_journal_recovery_do_not_replay_prior_records() {
        let (mut runtime, figure) = scene();
        let old = FigureInspector::new(capacity());
        let token = old.attach(&mut runtime).unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        token.detach(&mut runtime).unwrap();
        change(&mut runtime, figure);
        let fresh = recover(
            &mut runtime,
            &[token],
            &[ListenerScope::Runtime],
            capacity(),
        )
        .unwrap();
        assert!(fresh.inspector.events().unwrap().is_empty());
        assert_eq!(fresh.snapshot.figure(figure).unwrap().bounds.x, 3.0);
        let count = old.events().unwrap().len();
        runtime.figure(figure).unwrap().set_enabled(false).unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(old.events().unwrap().len(), count);
        assert!(!fresh.inspector.events().unwrap().is_empty());
        cleanup(&mut runtime, &fresh.subscriptions);
        let mut zero_surface = surface();
        zero_surface.pixel_width = 0;
        zero_surface.pixel_height = 0;
        assert!(matches!(
            runtime
                .frames()
                .prepare_submission(zero_surface, BackendCapabilities::RETAINED_PARTIAL),
            FramePreparation::Suspended
        ));
        let suspended = recover(&mut runtime, &[], &[ListenerScope::Runtime], capacity()).unwrap();
        assert!(suspended.inspector.events().unwrap().is_empty());
        cleanup(&mut runtime, &suspended.subscriptions);
    }

    #[test]
    fn recovery_cleans_partial_scopes_snapshot_failure_and_changed_checkpoint() {
        let (mut runtime, figure) = scene();
        let (_, foreign) = scene();
        let failure = recover(
            &mut runtime,
            &[],
            &[ListenerScope::Runtime, ListenerScope::Figure(foreign)],
            capacity(),
        )
        .err()
        .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::Registration(_)));
        assert!(failure.cleanup_errors.is_empty());
        let failure = recover_with_capture(
            &mut runtime,
            &[],
            &[ListenerScope::Runtime],
            capacity(),
            |_, _| {
                Err(StableQueryError::NotStable {
                    latest_stable_epoch: 0,
                })
            },
        )
        .err()
        .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::Snapshot(_)));
        assert!(failure.cleanup_errors.is_empty());
        let retained = Rc::new(RefCell::new(None));
        let copy = retained.clone();
        let failure = recover_with_capture(
            &mut runtime,
            &[],
            &[ListenerScope::Runtime],
            capacity(),
            move |inspector, runtime| {
                *copy.borrow_mut() = Some(inspector.clone());
                change(runtime, figure);
                runtime.frames().stabilize_for_query().unwrap();
                inspector.capture_tree(runtime)
            },
        )
        .err()
        .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::Changed { .. }));
        let rejected = retained.borrow().as_ref().unwrap().clone();
        let count = rejected.events().unwrap().len();
        runtime.figure(figure).unwrap().set_enabled(false).unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(rejected.events().unwrap().len(), count);
    }

    #[test]
    fn second_checkpoint_rejection_cleans_new_subscriptions() {
        let (mut runtime, figure) = scene();
        let retained = Rc::new(RefCell::new(None));
        let copy = retained.clone();
        let failure = recover_with_capture(
            &mut runtime,
            &[],
            &[ListenerScope::Runtime],
            capacity(),
            move |inspector, runtime| {
                let snapshot = inspector.capture_tree(runtime)?;
                *copy.borrow_mut() = Some(inspector.clone());
                change(runtime, figure);
                Ok(snapshot)
            },
        )
        .err()
        .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::Checkpoint(_)));
        assert!(failure.cleanup_errors.is_empty());
        runtime.frames().stabilize_for_query().unwrap();
        assert!(
            retained
                .borrow()
                .as_ref()
                .unwrap()
                .events()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn recovery_records_cleanup_errors_and_multi_runtime_partial_failure() {
        let (mut first, _) = scene();
        let (mut second, _) = scene();
        let inspector = FigureInspector::new(capacity());
        let token = inspector.attach(&mut first).unwrap();
        let failure = recover(
            &mut second,
            std::slice::from_ref(&token),
            &[ListenerScope::Runtime],
            capacity(),
        )
        .err()
        .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::Cleanup));
        assert_eq!(failure.cleanup_errors.len(), 1);
        assert!(token.detach(&mut first).unwrap());
        let success = recover(&mut first, &[], &[ListenerScope::Runtime], capacity()).unwrap();
        let foreign_scope = first.tree().root_id();
        assert!(
            recover(
                &mut second,
                &[],
                &[ListenerScope::Figure(foreign_scope)],
                capacity()
            )
            .is_err()
        );
        assert!(cleanup(&mut first, &success.subscriptions).is_empty());
        assert!(!success.subscriptions[0].detach(&mut first).unwrap());
    }

    #[test]
    fn callback_panic_stops_recovery_and_runtime_fault_does_not_block_detach() {
        struct Panic;
        impl ObservationListener for Panic {
            fn observed(
                &self,
                _: &NotificationRecord,
                _: StableSceneQuery<'_>,
            ) -> ListenerDirective {
                panic!("ordinary external observer");
            }
        }
        let (mut runtime, _) = scene();
        let token = FigureInspector::new(capacity())
            .attach(&mut runtime)
            .unwrap();
        runtime
            .observations()
            .add_observation_listener(Box::new(Panic))
            .unwrap();
        let failure = recover(&mut runtime, &[], &[ListenerScope::Runtime], capacity())
            .err()
            .unwrap();
        assert!(matches!(failure.cause, RecoveryCause::CallbackPanic));
        assert_eq!(
            runtime.observations().checkpoint(),
            Err(StableQueryError::Faulted)
        );
        assert!(token.detach(&mut runtime).unwrap());
    }
}
