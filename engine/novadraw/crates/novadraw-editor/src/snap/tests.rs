use super::{
    registry::Registry,
    solver::{solve, validate_final},
    *,
};
use crate::{EditPartId, EditorNamespace, ModelRevision, container::CandidateId};
use novadraw::geometry::{Affine2D, Point, Rectangle, Vec2};
use std::{cell::Cell, convert::Infallible, error::Error, fmt, num::NonZeroUsize, rc::Rc};
use uuid::Uuid;

fn x() -> DofKey {
    DofKey::of::<SurfaceTranslationX>()
}

#[test]
fn proportional_grid_accepts_consistent_rank_one_projection_and_keeps_fixed_control() {
    struct Scale;
    let owner = EditorNamespace::new();
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::ZERO, Some(100.0), Some(200.0)).unwrap(),
        ExactProjection::new(NearestGrid),
    );
    let mut snapshot = input(owner);
    snapshot.dofs = vec![DofKey::of::<Scale>()];
    snapshot.controls = vec![
        ControlPoint {
            origin: Point::new(40.0, 80.0),
            influence: vec![Vec2::new(1.0, 2.0)],
        },
        ControlPoint {
            origin: Point::new(10.0, 10.0),
            influence: vec![Vec2::ZERO],
        },
    ];
    let solution = solve(&registry, &mut session(owner), snapshot, |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[-40.0]);
    assert_eq!(
        solution.geometry().points(),
        &[Point::ZERO, Point::new(10.0, 10.0)]
    );
    assert_eq!(solution.adopted_features().count(), 1);
}

#[test]
fn proportional_grid_skips_inconsistent_two_axis_projection_without_ratio_drift() {
    struct Scale;
    let owner = EditorNamespace::new();
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::ZERO, Some(100.0), Some(100.0)).unwrap(),
        ExactProjection::new(NearestGrid),
    );
    let mut snapshot = input(owner);
    snapshot.dofs = vec![DofKey::of::<Scale>()];
    snapshot.controls[0] = ControlPoint {
        origin: Point::new(40.0, 80.0),
        influence: vec![Vec2::new(1.0, 2.0)],
    };
    let solution = solve(&registry, &mut session(owner), snapshot, |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[0.0]);
    assert_eq!(solution.adopted_features().count(), 0);
}
fn y() -> DofKey {
    DofKey::of::<SurfaceTranslationY>()
}

#[test]
fn reference_line_renderer_has_checked_geometry_and_ordinary_registry_lifecycle() {
    let renderer = || {
        ReferenceLineRenderer::new(
            novadraw::graphics::Color::BLACK,
            novadraw::render::StrokeStyle::default(),
            8.0,
        )
        .unwrap()
    };
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let mut foreign = Registry::new(EditorNamespace::new());
    let mut registration = SnapRegistration {
        registry: &mut registry,
    };
    let id = registration
        .register_renderer(ReferenceLineRenderer::key(), renderer())
        .unwrap();
    assert!(
        registration
            .register_renderer(ReferenceLineRenderer::key(), renderer())
            .is_err()
    );
    assert!(
        SnapRegistration {
            registry: &mut foreign
        }
        .remove_renderer(id)
        .is_err()
    );
    let replaced = registration.replace_renderer(id, renderer()).unwrap();
    assert!(registration.remove_renderer(id).is_err());
    registration.remove_renderer(replaced).unwrap();
    assert!(registration.remove_renderer(replaced).is_err());
    assert!(ReferenceLine::new(Point::ZERO, Vec2::ZERO).is_err());
    let line = ReferenceLine::new(Point::ZERO, Vec2::new(1.0, 0.0)).unwrap();
    let geometry = SnapGeometry {
        values: vec![0.0, 0.0],
        points: vec![Point::new(0.0, 10.0), Point::new(0.0, 100.0)],
    };
    assert_eq!(renderer().render(&line, &geometry).unwrap().len(), 1);
}
fn input(owner: EditorNamespace) -> SnapInput {
    let part = EditPartId::for_test(owner);
    let limit = NonZeroUsize::new(64).unwrap();
    SnapInput {
        namespace: owner,
        candidate: CandidateId(Uuid::new_v4()),
        revision: ModelRevision::initial(),
        gesture: super::geometry::GestureIdentity::of::<SurfaceTranslationX>(1),
        target: part,
        sources: vec![part],
        anchor: part,
        generations: Vec::new(),
        source_maps: Vec::new(),
        target_map: Affine2D::IDENTITY,
        pointer: Point::new(40.0, 0.0),
        dofs: vec![x(), y()],
        controls: vec![ControlPoint {
            origin: Point::new(40.0, 0.0),
            influence: vec![Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        }],
        anchor_controls: 0..1,
        scene: Vec::new(),
        siblings: Vec::new(),
        viewport: (Point::ZERO, 1.0, Rectangle::new(0.0, 0.0, 800.0, 600.0)),
        settings: SnapSettings::new(
            8.0,
            0.0,
            SnapBudget {
                sources: limit,
                features: limit,
                candidates: limit,
                feedback: limit,
            },
        )
        .unwrap(),
        layouts: Vec::new(),
    }
}
fn session(owner: EditorNamespace) -> SnapSession {
    SnapSession {
        owner,
        id: Uuid::new_v4(),
        active: true,
        latest_candidate: None,
        latest_solution: None,
        baseline: None,
        metrics: Default::default(),
        clock: None,
        prepared: Vec::new(),
    }
}
struct Points(usize);
impl FeatureProvider for Points {
    type Payload = Rc<Cell<u64>>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-points")
    }
    fn version(&self, p: &Self::Payload) -> Result<u64, SnapError> {
        Ok(p.get())
    }
    fn features(
        &self,
        _: &Self::Payload,
        _: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), SnapError> {
        for i in 0..self.0 {
            sink.push(i as u64, vec![Point::ZERO])?;
        }
        Ok(())
    }
}
struct Linear;
impl EvaluationRule for Linear {
    type Payload = (f64, f64, f64);
    type Error = Infallible;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("linear")
    }
    fn version(&self, _: &Self::Payload) -> Result<u64, Infallible> {
        Ok(0)
    }
    fn evaluate(
        &self,
        &(a, b, c): &Self::Payload,
        context: &EvaluationContext<'_>,
    ) -> Result<bool, Infallible> {
        Ok(
            (a * context.value(x()).unwrap() + b * context.value(y()).unwrap() - c).abs()
                <= context.input.settings().tolerance(),
        )
    }
}
struct Projection;
impl EvaluationRule for Projection {
    type Payload = ();
    type Error = Infallible;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-nearest-projection")
    }
    fn version(&self, _: &()) -> Result<u64, Infallible> {
        Ok(0)
    }
    fn evaluate(&self, _: &(), ctx: &EvaluationContext<'_>) -> Result<bool, Infallible> {
        let original = ctx.input.controls()[0].origin().x();
        let expected = (original / 100.0).round_ties_even() * 100.0;
        Ok(ctx.geometry.points()[0].x() == expected)
    }
}
struct Spec {
    writes: Vec<(DofKey, f64)>,
    dependencies: Vec<DofKey>,
    equation: (f64, f64, f64),
    feedback: usize,
}
struct External<R: EvaluationRule<Payload = ()>> {
    admission: RuleBinding<R>,
    alignment: RuleBinding<Linear>,
    specs: Vec<Spec>,
    mutation: Option<Rc<Cell<u64>>>,
    ignore_limit: bool,
}
impl<R: EvaluationRule<Payload = ()>> SnapStrategy for External<R> {
    type Payload = Rc<Cell<u64>>;
    type Prepared = Rc<Cell<usize>>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-configured-strategy")
    }
    fn version(&self, payload: &Self::Payload) -> Result<u64, SnapError> {
        Ok(payload.get())
    }
    fn prepare(
        &self,
        _: &Self::Payload,
        _: &SnapInput,
        _: &[SnapFeature],
    ) -> Result<Self::Prepared, SnapError> {
        Ok(Rc::new(Cell::new(0)))
    }
    fn query(
        &self,
        _: &Self::Payload,
        prepared: &Self::Prepared,
        _: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError> {
        prepared.set(prepared.get() + 1);
        let feature = sink.features()[0].id();
        for (stable, spec) in self.specs.iter().enumerate() {
            let result = sink.push(SnapCandidate::new(
                feature,
                stable as u64,
                spec.writes.clone(),
                spec.dependencies.clone(),
                self.admission.bind(()),
                self.alignment.bind(spec.equation),
                (0..spec.feedback)
                    .map(|_| {
                        FeedbackDeclaration::new(
                            ProtocolKey::new("external-feedback"),
                            Rc::new(Cell::new(1)),
                        )
                    })
                    .collect(),
            ));
            if !self.ignore_limit {
                result?;
            }
        }
        if let Some(mutation) = &self.mutation {
            mutation.set(mutation.get() + 1);
        }
        Ok(())
    }
}
fn spec(dx: f64, dy: Option<f64>) -> Spec {
    let mut writes = vec![(x(), dx)];
    if let Some(dy) = dy {
        writes.push((y(), dy));
    }
    Spec {
        writes,
        dependencies: vec![x()],
        equation: (1.0, 0.0, dx),
        feedback: 1,
    }
}
fn register<R: EvaluationRule<Payload = ()>>(
    registry: &mut Registry,
    rule: R,
    specs: Vec<Spec>,
    mutation: Option<Rc<Cell<u64>>>,
    ignore_limit: bool,
) {
    let mut reg = SnapRegistration { registry };
    let admission = reg.register_rule(R::key(), rule).unwrap();
    let alignment = reg.register_rule(Linear::key(), Linear).unwrap();
    reg.register_provider(Points::key(), Points(1), Rc::new(Cell::new(0)))
        .unwrap();
    reg.register_strategy(
        External::<R>::key(),
        External {
            admission: admission.clone(),
            alignment,
            specs,
            mutation,
            ignore_limit,
        },
        Rc::new(Cell::new(0)),
        &admission,
    )
    .unwrap();
}

#[test]
fn external_projection_uses_same_registry_and_preserves_original_input() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ExactProjection::new(Projection),
        vec![spec(-40.0, None)],
        None,
        false,
    );
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().points(), &[Point::ZERO]);
    assert_eq!(
        solution.input().controls()[0].origin(),
        Point::new(40.0, 0.0)
    );
    assert_eq!(solution.feedback().count(), 1);
    assert_eq!(session.metrics().adopted, 1);
    assert_eq!(
        validate_final(
            &registry,
            &mut session,
            &solution,
            vec![-40.0, 0.0],
            |_| Ok(())
        )
        .unwrap(),
        *solution.geometry()
    );
}

#[test]
fn threshold_binding_rejects_the_same_over_radius_projection() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(-40.0, None)],
        None,
        false,
    );
    let error = solve(&registry, &mut session(owner), input(owner), |_| Ok(()))
        .err()
        .unwrap();
    assert!(matches!(
        error,
        SnapError::Invalid("candidate violates authorized admission")
    ));
}

#[test]
fn full_dependencies_protect_earlier_coupled_fact() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let early = Spec {
        writes: vec![(x(), 2.0)],
        dependencies: vec![x(), y()],
        equation: (1.0, 1.0, 2.0),
        feedback: 1,
    };
    let later = Spec {
        writes: vec![(y(), 3.0)],
        dependencies: vec![y()],
        equation: (0.0, 1.0, 3.0),
        feedback: 1,
    };
    register(
        &mut registry,
        ThresholdCapture,
        vec![early, later],
        None,
        false,
    );
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[2.0, 0.0]);
    assert_eq!(session.metrics().conflicts, 1);
    assert_eq!(solution.feedback().count(), 1);
}

#[test]
fn even_underdeclared_external_dependencies_do_not_publish_broken_earlier_facts() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let early = Spec {
        writes: vec![(x(), 2.0)],
        dependencies: vec![x()],
        equation: (1.0, 1.0, 2.0),
        feedback: 1,
    };
    let later = Spec {
        writes: vec![(y(), 3.0)],
        dependencies: vec![y()],
        equation: (0.0, 1.0, 3.0),
        feedback: 1,
    };
    register(
        &mut registry,
        ThresholdCapture,
        vec![early, later],
        None,
        false,
    );
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[2.0, 0.0]);
    assert_eq!(session.metrics().adopted, 1);
    assert!(session.metrics().evaluation_calls >= 7);
}

#[test]
fn independent_axes_compose_without_using_diagonal_as_capture_radius() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let dx = spec(7.0, None);
    let dy = Spec {
        writes: vec![(y(), 7.0)],
        dependencies: vec![y()],
        equation: (0.0, 1.0, 7.0),
        feedback: 1,
    };
    register(&mut registry, ThresholdCapture, vec![dx, dy], None, false);
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[7.0, 7.0]);
    assert_eq!(session.metrics().adopted, 2);
}

#[test]
fn malformed_own_fact_is_an_error_even_when_its_write_would_conflict() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let mut false_fact = spec(3.0, None);
    false_fact.equation.2 = 4.0;
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None), false_fact],
        None,
        false,
    );
    let error = solve(&registry, &mut session(owner), input(owner), |_| Ok(()))
        .err()
        .unwrap();
    assert!(matches!(
        error,
        SnapError::Invalid("candidate's own alignment fact is false")
    ));
}

#[test]
fn final_layout_cannot_destroy_adopted_fact_or_retry() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(3.0, None)],
        None,
        false,
    );
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    let queries = session.metrics().query_calls;
    assert!(matches!(
        validate_final(&registry, &mut session, &solution, vec![4.0, 0.0], |_| Ok(
            ()
        )),
        Err(SnapError::Rejected("final plan destroys adopted alignment"))
    ));
    assert_eq!(session.metrics().query_calls, queries);
}

#[test]
fn newer_position_retires_previous_solution_even_when_all_versions_match() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    let mut session = session(owner);
    let at_a = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    let mut input_b = input(owner);
    input_b.pointer = Point::new(41.0, 0.0);
    input_b.controls[0].origin = input_b.pointer;
    let at_b = solve(&registry, &mut session, input_b, |_| Ok(())).unwrap();
    assert_eq!(at_b.geometry().points(), &[Point::new(43.0, 0.0)]);
    assert!(matches!(
        validate_final(&registry, &mut session, &at_a, vec![2.0, 0.0], |_| Ok(())),
        Err(SnapError::Invalidated)
    ));
}

#[test]
fn optional_host_clock_records_callbacks_without_implicit_platform_time() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    let mut untimed = session(owner);
    solve(&registry, &mut untimed, input(owner), |_| Ok(())).unwrap();
    assert_eq!(untimed.metrics().callback_time, None);
    let ticks = Cell::new(0_u64);
    let mut timed = session(owner);
    timed.set_measurement_clock(move || {
        ticks.set(ticks.get() + 1);
        std::time::Duration::from_nanos(ticks.get())
    });
    solve(&registry, &mut timed, input(owner), |_| Ok(())).unwrap();
    let expected = timed.metrics().version_calls
        + timed.metrics().provider_calls
        + timed.metrics().prepare_calls
        + timed.metrics().query_calls
        + timed.metrics().evaluation_calls;
    assert_eq!(
        timed.metrics().callback_time,
        Some(std::time::Duration::from_nanos(expected as u64))
    );
}

#[test]
fn ignored_candidate_limit_still_rejects_complete_solve() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None), spec(3.0, None)],
        None,
        true,
    );
    let mut input = input(owner);
    let mut budget = input.settings.budget();
    budget.candidates = NonZeroUsize::new(1).unwrap();
    input.settings = SnapSettings::new(8.0, 0.0, budget).unwrap();
    assert!(solve(&registry, &mut session(owner), input, |_| Ok(())).is_err());
}

#[test]
fn nonfinite_candidate_and_feedback_overflow_are_whole_operation_errors() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(f64::NAN, None)],
        None,
        false,
    );
    assert!(solve(&registry, &mut session(owner), input(owner), |_| Ok(())).is_err());
    let mut registry = Registry::new(owner);
    let mut feedback = spec(2.0, None);
    feedback.feedback = 65;
    register(&mut registry, ThresholdCapture, vec![feedback], None, false);
    assert!(solve(&registry, &mut session(owner), input(owner), |_| Ok(())).is_err());
}

#[test]
fn query_mutating_prepared_provider_dependency_is_detected_without_retries() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let version = Rc::new(Cell::new(0));
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        Some(version.clone()),
        false,
    );
    // The mutable provider below is part of the complete dependency collection.
    SnapRegistration {
        registry: &mut registry,
    }
    .register_provider(Points::key(), Points(1), version)
    .unwrap();
    let mut session = session(owner);
    assert!(matches!(
        solve(&registry, &mut session, input(owner), |_| Ok(())),
        Err(SnapError::Invalidated)
    ));
    assert_eq!(session.metrics().query_calls, 1);
}

#[test]
fn owner_changes_after_callback_reject_and_session_cannot_take_new_model_baseline() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    let checks = Cell::new(0);
    let mut session = session(owner);
    assert!(matches!(
        solve(&registry, &mut session, input(owner), |_| {
            checks.set(checks.get() + 1);
            if checks.get() == 6 {
                Err(SnapError::Invalidated)
            } else {
                Ok(())
            }
        }),
        Err(SnapError::Invalidated)
    ));
    let mut next = input(owner);
    next.revision = next.revision.next().unwrap();
    assert!(matches!(
        solve(&registry, &mut session, next, |_| Ok(())),
        Err(SnapError::Invalidated)
    ));
    assert!(!session.active);
}

#[test]
fn foreign_retired_registration_and_authorization_are_checked_without_callbacks() {
    let owner = EditorNamespace::new();
    let mut a = Registry::new(owner);
    let mut b = Registry::new(EditorNamespace::new());
    let mut registration = SnapRegistration { registry: &mut a };
    let binding = registration.register_rule(Linear::key(), Linear).unwrap();
    let old_call = binding.bind((1.0, 0.0, 0.0));
    assert!(b.check_rule(&old_call).is_err());
    let replacement = registration.replace_rule(&binding, Linear).unwrap();
    assert!(registration.remove_rule(&binding).is_err());
    assert!(a.check_rule(&old_call).is_err());
    let mut other = SnapRegistration { registry: &mut b };
    assert!(other.replace_rule(&replacement, Linear).is_err());
    assert!(other.remove_rule(&replacement).is_err());
}

#[derive(Debug)]
struct LocalFailure(Rc<Cell<u32>>);
impl fmt::Display for LocalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("local extension failure")
    }
}
impl Error for LocalFailure {}
struct FailingRule;
impl EvaluationRule for FailingRule {
    type Payload = Rc<Cell<u32>>;
    type Error = LocalFailure;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("failure")
    }
    fn version(&self, _: &Self::Payload) -> Result<u64, LocalFailure> {
        Ok(0)
    }
    fn evaluate(
        &self,
        payload: &Self::Payload,
        _: &EvaluationContext<'_>,
    ) -> Result<bool, LocalFailure> {
        Err(LocalFailure(payload.clone()))
    }
}
#[test]
fn evaluator_source_keeps_nonclone_nonthread_payload_alive() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let cause = Rc::new(Cell::new(17));
    let binding = SnapRegistration {
        registry: &mut registry,
    }
    .register_rule(FailingRule::key(), FailingRule)
    .unwrap();
    let mut input = input(owner);
    input.layouts.push(binding.bind(cause));
    let error = solve(&registry, &mut session(owner), input, |_| Ok(()))
        .err()
        .unwrap();
    let leaf = error
        .source()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<LocalFailure>()
        .unwrap();
    assert_eq!(leaf.0.get(), 17);
}

struct InvalidatingRule;
impl EvaluationRule for InvalidatingRule {
    type Payload = (Rc<Cell<bool>>, Rc<Cell<u64>>);
    type Error = Infallible;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("adversarial-final-evaluator")
    }
    fn version(&self, payload: &Self::Payload) -> Result<u64, Infallible> {
        Ok(payload.1.get())
    }
    fn evaluate(
        &self,
        payload: &Self::Payload,
        _: &EvaluationContext<'_>,
    ) -> Result<bool, Infallible> {
        // Deliberately violate the read-only callback contract to exercise the post-call guard.
        if payload.0.get() {
            payload.1.set(payload.1.get() + 1);
        }
        Ok(true)
    }
}
#[test]
fn final_evaluator_dependency_change_prevents_geometry_publication_without_requery() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let trigger = Rc::new(Cell::new(false));
    let version = Rc::new(Cell::new(0));
    let binding = SnapRegistration {
        registry: &mut registry,
    }
    .register_rule(InvalidatingRule::key(), InvalidatingRule)
    .unwrap();
    let mut input = input(owner);
    input.layouts.push(binding.bind((trigger.clone(), version)));
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input, |_| Ok(())).unwrap();
    trigger.set(true);
    let queries = session.metrics().query_calls;
    assert!(matches!(
        validate_final(&registry, &mut session, &solution, vec![0.0, 0.0], |_| Ok(
            ()
        )),
        Err(SnapError::Invalidated)
    ));
    assert_eq!(session.metrics().query_calls, queries);
}

#[test]
fn failed_recomputation_of_same_candidate_retires_previous_publication() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    let mut next = input(owner);
    next.candidate = solution.input().candidate();
    next.settings = SnapSettings::new(1.0, 0.0, next.settings.budget()).unwrap();
    assert!(solve(&registry, &mut session, next, |_| Ok(())).is_err());
    assert!(matches!(
        validate_final(&registry, &mut session, &solution, vec![2.0, 0.0], |_| Ok(
            ()
        )),
        Err(SnapError::Invalidated)
    ));
}

#[test]
fn strategy_and_provider_lifecycle_reject_foreign_and_retired_without_changes() {
    let owner = EditorNamespace::new();
    let mut a = Registry::new(owner);
    let mut b = Registry::new(EditorNamespace::new());
    let mut reg = SnapRegistration { registry: &mut a };
    let admission = reg
        .register_rule(ThresholdCapture::key(), ThresholdCapture)
        .unwrap();
    let alignment = reg.register_rule(Linear::key(), Linear).unwrap();
    let strategy = || External {
        admission: admission.clone(),
        alignment: alignment.clone(),
        specs: vec![],
        mutation: None,
        ignore_limit: false,
    };
    let id = reg
        .register_strategy(
            External::<ThresholdCapture>::key(),
            strategy(),
            Rc::new(Cell::new(0)),
            &admission,
        )
        .unwrap();
    let provider = reg
        .register_provider(Points::key(), Points(1), Rc::new(Cell::new(0)))
        .unwrap();
    let mut other = SnapRegistration { registry: &mut b };
    let unchanged = other.registry.revision;
    assert!(
        other
            .replace_strategy(id, strategy(), Rc::new(Cell::new(0)), &admission)
            .is_err()
    );
    assert!(other.order_strategy(id, 0).is_err());
    assert!(other.remove_strategy(id).is_err());
    assert!(
        other
            .replace_provider(provider, Points(1), Rc::new(Cell::new(0)))
            .is_err()
    );
    assert!(other.order_provider(provider, 0).is_err());
    assert!(other.remove_provider(provider).is_err());
    assert_eq!(unchanged, other.registry.revision);
    let new_id = reg
        .replace_strategy(id, strategy(), Rc::new(Cell::new(0)), &admission)
        .unwrap();
    let new_provider = reg
        .replace_provider(provider, Points(1), Rc::new(Cell::new(0)))
        .unwrap();
    let unchanged = reg.registry.revision;
    assert!(
        reg.replace_strategy(id, strategy(), Rc::new(Cell::new(0)), &admission)
            .is_err()
    );
    assert!(reg.remove_strategy(id).is_err());
    assert!(reg.order_strategy(id, 0).is_err());
    assert!(
        reg.replace_provider(provider, Points(1), Rc::new(Cell::new(0)))
            .is_err()
    );
    assert!(reg.remove_provider(provider).is_err());
    assert!(reg.order_provider(provider, 0).is_err());
    assert_eq!(unchanged, reg.registry.revision);
    reg.order_strategy(new_id, 0).unwrap();
    reg.order_provider(new_provider, 0).unwrap();
    reg.remove_strategy(new_id).unwrap();
    reg.remove_provider(new_provider).unwrap();
}

#[test]
fn removed_admission_is_rejected_before_provider_or_strategy_callback() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    registry.rules.remove(0);
    let mut session = session(owner);
    assert!(matches!(
        solve(&registry, &mut session, input(owner), |_| Ok(())),
        Err(SnapError::Identity)
    ));
    assert_eq!(session.metrics().provider_calls, 0);
    assert_eq!(session.metrics().prepare_calls, 0);
}

#[test]
fn source_and_scene_feature_budgets_reject_before_extensions() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    register(
        &mut registry,
        ThresholdCapture,
        vec![spec(2.0, None)],
        None,
        false,
    );
    let mut limited = input(owner);
    let mut budget = limited.settings.budget();
    budget.sources = NonZeroUsize::new(1).unwrap();
    limited.settings = SnapSettings::new(8.0, 0.0, budget).unwrap();
    let mut session = session(owner);
    assert!(matches!(
        solve(&registry, &mut session, limited, |_| Ok(())),
        Err(SnapError::Budget(_))
    ));
    assert_eq!(session.metrics().version_calls, 0);
    let mut limited = input(owner);
    let mut budget = limited.settings.budget();
    budget.features = NonZeroUsize::new(1).unwrap();
    limited.settings = SnapSettings::new(8.0, 0.0, budget).unwrap();
    limited.scene = (0..2)
        .map(|_| SceneFeature {
            part: limited.anchor,
            corners: vec![Point::ZERO],
            mapping: Affine2D::IDENTITY,
        })
        .collect();
    assert!(matches!(
        solve(&registry, &mut session, limited, |_| Ok(())),
        Err(SnapError::Budget(_))
    ));
    assert_eq!(session.metrics().provider_calls, 0);
}

fn grid_registry<R: EvaluationRule<Payload = GridProjection>>(
    owner: EditorNamespace,
    spec: GridSpec,
    admission: R,
) -> Registry {
    let mut registry = Registry::new(owner);
    let mut reg = SnapRegistration {
        registry: &mut registry,
    };
    let admission = reg.register_rule(R::key(), admission).unwrap();
    let alignment = reg.register_rule(NearestGrid::key(), NearestGrid).unwrap();
    reg.register_provider(GridProvider::key(), GridProvider, spec)
        .unwrap();
    reg.register_strategy(
        Grid::<R>::key(),
        Grid::new(admission.clone(), alignment),
        (),
        &admission,
    )
    .unwrap();
    registry
}

#[test]
fn grid_capture_and_exact_projection_share_checked_admission_without_radius_exemption() {
    let owner = EditorNamespace::new();
    let spec = GridSpec::new(Point::ZERO, Some(100.0), None).unwrap();
    let capture = grid_registry(owner, spec, GridCapture);
    let mut session = session(owner);
    let solution = solve(&capture, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[0.0, 0.0]);
    assert_eq!(solution.adopted_features().count(), 0);
    assert_eq!(session.metrics().evaluation_calls, 1);
    let exact = grid_registry(owner, spec, ExactProjection::new(NearestGrid));
    let mut session = super::tests::session(owner);
    let solution = solve(&exact, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[-40.0, 0.0]);
    assert_eq!(solution.adopted_features().count(), 1);
}

#[test]
fn grid_negative_midpoints_use_ties_even_and_reject_derived_overflow() {
    let owner = EditorNamespace::new();
    let spec = GridSpec::new(Point::ZERO, Some(100.0), None).unwrap();
    let registry = grid_registry(owner, spec, ExactProjection::new(NearestGrid));
    for (original, expected) in [
        (-250.0, -200.0),
        (-150.0, -200.0),
        (-50.0, 0.0),
        (50.0, 0.0),
        (150.0, 200.0),
        (250.0, 200.0),
    ] {
        let mut input = input(owner);
        input.controls[0].origin = Point::new(original, 0.0);
        let solution = solve(&registry, &mut session(owner), input, |_| Ok(())).unwrap();
        assert_eq!(solution.geometry().points()[0].x(), expected);
    }
    assert!(GridSpec::new(Point::ZERO, Some(0.0), None).is_err());
    assert!(GridSpec::new(Point::ZERO, Some(f64::NAN), None).is_err());
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::new(-f64::MAX, 0.0), Some(1.0), None).unwrap(),
        ExactProjection::new(NearestGrid),
    );
    let mut input = input(owner);
    input.controls[0].origin = Point::new(f64::MAX, 0.0);
    assert!(solve(&registry, &mut session(owner), input, |_| Ok(())).is_err());
}

#[test]
fn grid_independent_axes_capture_separately_and_batch_uses_unique_anchor() {
    let owner = EditorNamespace::new();
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::ZERO, Some(100.0), Some(100.0)).unwrap(),
        GridCapture,
    );
    let mut input = input(owner);
    input.controls[0].origin = Point::new(93.0, 93.0);
    // A non-anchor in the batch must receive exactly the same correction.
    input.controls.push(ControlPoint {
        origin: Point::new(232.0, 154.0),
        influence: vec![Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
    });
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input, |_| Ok(())).unwrap();
    assert_eq!(
        solution.geometry().points(),
        &[Point::new(100.0, 100.0), Point::new(239.0, 161.0)]
    );
    assert_eq!(session.metrics().adopted, 2);
}

#[test]
fn rotated_nonuniform_grid_uses_complete_mapping_and_coupled_freedoms() {
    let owner = EditorNamespace::new();
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::ZERO, Some(100.0), Some(100.0)).unwrap(),
        ExactProjection::new(NearestGrid),
    );
    let mut input = input(owner);
    input.target_map = Affine2D::new(2.0, 1.0, -1.0, 3.0, 31.0, -14.0);
    input.settings = SnapSettings::new(8.0, 1e-9, input.settings.budget()).unwrap();
    input.controls[0].origin = input.target_map.transform_point(Point::new(40.0, 30.0));
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, input, |_| Ok(())).unwrap();
    assert!((solution.geometry().points()[0].x() - 31.0).abs() < 1e-9);
    assert!((solution.geometry().points()[0].y() + 14.0).abs() < 1e-9);
    assert_eq!(session.metrics().adopted, 1);
    assert!(
        validate_final(&registry, &mut session, &solution, vec![0.0, 0.0], |_| Ok(
            ()
        ))
        .is_err()
    );
}

fn install_lines<T: 'static>(registry: &mut Registry) -> StrategyId<LineSnap<T>> {
    let mut reg = SnapRegistration { registry };
    let admission = reg
        .register_rule(ThresholdCapture::key(), ThresholdCapture)
        .unwrap();
    let alignment = reg
        .register_rule(LineAlignment::key(), LineAlignment)
        .unwrap();
    reg.register_strategy(
        LineSnap::<T>::key(),
        LineSnap::new(admission.clone(), alignment),
        (),
        &admission,
    )
    .unwrap()
}

#[test]
fn geometry_indexes_committed_real_edges_and_rebuilds_changed_members() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    SnapRegistration {
        registry: &mut registry,
    }
    .register_provider(GeometryProvider::key(), GeometryProvider, ())
    .unwrap();
    install_lines::<GeometryFeature>(&mut registry);
    let mut first = input(owner);
    first.scene.push(SceneFeature {
        part: EditPartId::for_test(owner),
        corners: vec![
            Point::new(43.0, 20.0),
            Point::new(63.0, 20.0),
            Point::new(63.0, 30.0),
            Point::new(43.0, 30.0),
        ],
        mapping: Affine2D::IDENTITY,
    });
    let mut session = session(owner);
    let solution = solve(&registry, &mut session, first, |_| Ok(())).unwrap();
    assert_eq!(solution.geometry().values(), &[3.0, 0.0]);
    assert_eq!(session.metrics().features, 6);
    let mut second = input(owner);
    second.scene = solution.input.scene.clone();
    second.controls[0].origin = Point::new(41.0, 0.0);
    solve(&registry, &mut session, second, |_| Ok(())).unwrap();
    assert_eq!(session.metrics().prepare_calls, 0);
    assert_eq!(session.metrics().cache_hits, 1);
    let mut third = input(owner);
    third.scene = solution.input.scene.clone();
    // Non-model visibility/geometry changes can rebuild a valid gesture's asset.
    third.scene.clear();
    solve(&registry, &mut session, third, |_| Ok(())).unwrap();
    assert_eq!(session.metrics().prepare_calls, 1);
    assert_eq!(session.metrics().cache_hits, 0);
}

#[test]
fn guides_use_unique_anchor_and_stable_identity_on_exact_distance_tie() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let mut reg = SnapRegistration {
        registry: &mut registry,
    };
    reg.register_provider(
        GuideProvider::key(),
        GuideProvider,
        vec![
            GuideSpec::new(2, Point::new(38.0, 0.0), Point::new(38.0, 100.0)).unwrap(),
            GuideSpec::new(1, Point::new(42.0, 0.0), Point::new(42.0, 100.0)).unwrap(),
        ],
    )
    .unwrap();
    install_lines::<GuideFeature>(&mut registry);
    let mut input = input(owner);
    input.controls.insert(
        0,
        ControlPoint {
            origin: Point::new(300.0, 0.0),
            influence: vec![Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        },
    );
    input.anchor_controls = 1..2;
    let solution = solve(&registry, &mut session(owner), input, |_| Ok(())).unwrap();
    assert_eq!(
        solution.geometry().points(),
        &[Point::new(302.0, 0.0), Point::new(42.0, 0.0)]
    );
    assert_eq!(solution.adopted_features().next().unwrap().stable, 1);
}

#[test]
fn provider_reordering_only_rearbitrates_and_replacement_rebuilds_asset() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    let mut reg = SnapRegistration {
        registry: &mut registry,
    };
    let left = reg
        .register_provider(
            GuideProvider::key(),
            GuideProvider,
            vec![GuideSpec::new(0, Point::new(38.0, 0.0), Point::new(38.0, 100.0)).unwrap()],
        )
        .unwrap();
    let right = reg
        .register_provider(
            GuideProvider::key(),
            GuideProvider,
            vec![GuideSpec::new(0, Point::new(42.0, 0.0), Point::new(42.0, 100.0)).unwrap()],
        )
        .unwrap();
    install_lines::<GuideFeature>(&mut registry);
    let mut session = session(owner);
    let first = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(first.geometry().values(), &[-2.0, 0.0]);
    SnapRegistration {
        registry: &mut registry,
    }
    .order_provider(right, 0)
    .unwrap();
    let next = solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(next.geometry().values(), &[2.0, 0.0]);
    assert_eq!(session.metrics().cache_hits, 1);
    assert_eq!(session.metrics().prepare_calls, 0);
    SnapRegistration {
        registry: &mut registry,
    }
    .replace_provider(
        left,
        GuideProvider,
        vec![GuideSpec::new(0, Point::new(44.0, 0.0), Point::new(44.0, 100.0)).unwrap()],
    )
    .unwrap();
    solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    assert_eq!(session.metrics().cache_hits, 0);
    assert_eq!(session.metrics().prepare_calls, 1);
}

#[test]
fn grid_reuses_feature_asset_across_pointer_and_surface_mapping_changes() {
    let owner = EditorNamespace::new();
    let registry = grid_registry(
        owner,
        GridSpec::new(Point::ZERO, Some(100.0), None).unwrap(),
        ExactProjection::new(NearestGrid),
    );
    let mut session = session(owner);
    solve(&registry, &mut session, input(owner), |_| Ok(())).unwrap();
    let mut input = input(owner);
    input.target_map = Affine2D::from_scale(2.0, 2.0);
    input.controls[0].origin = Point::new(80.0, 0.0);
    solve(&registry, &mut session, input, |_| Ok(())).unwrap();
    assert_eq!(session.metrics().cache_hits, 1);
    assert_eq!(session.metrics().query_calls, 1);
}

struct ExternalLine;
struct ExternalLineProvider;
impl FeatureProvider for ExternalLineProvider {
    type Payload = ();
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-real-line")
    }
    fn version(&self, _: &()) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn features(&self, _: &(), _: &SnapInput, sink: &mut FeatureSink<'_>) -> Result<(), SnapError> {
        let start = Point::new(44.0, 0.0);
        let end = Point::new(45.0, 1.0);
        sink.push_typed(
            1,
            vec![start, end],
            AlignmentLine::<ExternalLine>::key(),
            AlignmentLine::<ExternalLine>::new(start, end)?,
        )
    }
}
#[test]
fn external_rotated_line_uses_same_provider_strategy_and_coupled_relation() {
    let owner = EditorNamespace::new();
    let mut registry = Registry::new(owner);
    SnapRegistration {
        registry: &mut registry,
    }
    .register_provider(ExternalLineProvider::key(), ExternalLineProvider, ())
    .unwrap();
    install_lines::<ExternalLine>(&mut registry);
    let mut input = input(owner);
    input.settings = SnapSettings::new(8.0, 1e-9, input.settings.budget()).unwrap();
    let solution = solve(&registry, &mut session(owner), input, |_| Ok(())).unwrap();
    assert!((solution.geometry().values()[0] - 2.0).abs() < 1e-9);
    assert!((solution.geometry().values()[1] + 2.0).abs() < 1e-9);
    assert_eq!(solution.adopted_features().count(), 1);
}
