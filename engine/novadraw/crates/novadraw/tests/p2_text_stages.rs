use novadraw::{
    figure::{PreparationBudget, PreparationBudgetHandle},
    graphics::Color,
    render::ResourceId,
    text::{
        FontDescriptor, ParagraphAlignment, ParagraphDirection, StyledTextLayout,
        StyledTextRequest, TextConstraints, TextError, TextInlineBox, TextLayout,
        TextLayoutDiagnostics, TextStyle, TextStyleSpan, TextSystem, shaping::*,
    },
};
use std::{cell::RefCell, error::Error, fmt, rc::Rc};

const KEY: TextAnalysisKey<Payload> = TextAnalysisKey::new("external-analysis");
const OTHER_KEY: TextAnalysisKey<OtherPayload> = TextAnalysisKey::new("external-analysis");
const LIMITS: TextAnalysisCacheLimits = TextAnalysisCacheLimits {
    entries: 2,
    retained_bytes: 1_000_000,
};

struct Payload {
    bytes: Vec<u8>,
    shared: Option<(TextAnalysisBackingIdentity, Rc<Vec<u8>>)>,
}
impl TextAnalysisPayload for Payload {
    fn storage(&self, context: &TextStageContext) -> Result<TextAnalysisStorage, TextStageError> {
        let mut shared = Vec::new();
        if let Some((identity, data)) = &self.shared {
            context.charge(
                "external-storage-description",
                TextStageCostPlan {
                    work: 0,
                    allocation: 1,
                },
            )?;
            shared
                .try_reserve_exact(1)
                .map_err(|_| TextStageError::StorageReservation)?;
            shared.push(TextAnalysisBacking {
                identity: *identity,
                retained_bytes: std::mem::size_of::<Vec<u8>>()
                    + 2 * std::mem::size_of::<usize>()
                    + data.capacity(),
            });
        }
        Ok(TextAnalysisStorage {
            owned_bytes: self.bytes.capacity(),
            shared,
        })
    }
}
struct OtherPayload;
impl TextAnalysisPayload for OtherPayload {
    fn storage(&self, _: &TextStageContext) -> Result<TextAnalysisStorage, TextStageError> {
        Ok(TextAnalysisStorage {
            owned_bytes: 0,
            shared: Vec::new(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Lower,
    Analyze,
    Reflow,
}
#[derive(Clone, Copy, Debug)]
enum Change {
    Identity,
    Revision,
    Key,
    Registry,
    Disappear,
}
#[derive(Clone, Copy, Debug)]
enum Return {
    Value,
    None,
    Error,
}
#[derive(Clone, Copy, Debug)]
struct Fault {
    phase: Phase,
    change: Option<Change>,
    returned: Return,
    budget: bool,
}
#[derive(Default)]
struct State {
    calls: [usize; 4], // full, lower, analyze, reflow
    steps: usize,
    before_steps: Vec<PreparationBudget>,
    fault: Option<Fault>,
    lease: Option<TextAnalysisLease>,
    context: Option<TextStageContext>,
    registry: Option<Rc<TextAnalysisRegistry>>,
    lower_bytes: Option<usize>,
    missing_plan: Option<usize>,
    invalid_output: bool,
    shared: Option<(TextAnalysisBackingIdentity, Rc<Vec<u8>>)>,
    full_error: bool,
    foreign_lease: Option<TextAnalysisLease>,
}
struct Engine {
    identity: TextLayoutCacheIdentity,
    revision: u64,
    registry: Rc<TextAnalysisRegistry>,
    state: Rc<RefCell<State>>,
    available: bool,
    other_key: bool,
    secondary_identity: TextLayoutCacheIdentity,
    secondary_registry: Rc<TextAnalysisRegistry>,
}
impl Engine {
    fn new(limits: TextAnalysisCacheLimits) -> (Self, Rc<RefCell<State>>) {
        let identity = TextLayoutCacheIdentity::default();
        let registry = Rc::new(TextAnalysisRegistry::new(identity, limits).unwrap());
        let state = Rc::new(RefCell::new(State {
            registry: Some(registry.clone()),
            ..Default::default()
        }));
        (
            Self {
                identity,
                revision: 0,
                registry,
                state: state.clone(),
                available: true,
                other_key: false,
                secondary_identity: TextLayoutCacheIdentity::default(),
                secondary_registry: Rc::new(TextAnalysisRegistry::new(identity, limits).unwrap()),
            },
            state,
        )
    }
    fn lower_change(&self) -> Option<Change> {
        let state = self.state.borrow();
        state
            .fault
            .filter(|f| f.phase == Phase::Lower && state.calls[1] != 0)
            .and_then(|f| f.change)
    }
    fn after(&mut self, phase: Phase, context: &TextStageContext) -> Return {
        let fault = self.state.borrow().fault.filter(|f| f.phase == phase);
        let Some(fault) = fault else {
            return Return::Value;
        };
        if fault.budget {
            let _ = context
                .budget()
                .consume("ignored external exhaustion", u64::MAX);
        }
        if let Some(change) = fault.change {
            match change {
                Change::Identity => self.identity = TextLayoutCacheIdentity::default(),
                Change::Revision => self.revision += 1,
                Change::Key => self.other_key = true,
                Change::Registry => {
                    self.registry =
                        Rc::new(TextAnalysisRegistry::new(self.identity, LIMITS).unwrap());
                }
                Change::Disappear => self.available = false,
            }
        }
        fault.returned
    }
    fn output(&self, request: &StyledTextRequest) -> Result<StyledTextLayout, TextError> {
        // Deliberately simple external geometry: exercises the protocol without a private shaper.
        let width = request
            .constraints()
            .max_width
            .unwrap_or(request.text().len() as f32);
        let layout = TextLayout::from_parts(TextLayoutParts {
            text: request.text().to_owned(),
            font: request.default_style().font().clone(),
            constraints: request.constraints(),
            engine_revision: self.revision,
            width,
            full_width: request.text().len() as f32,
            height: 0.0,
            lines: Vec::new(),
            glyph_runs: Vec::new(),
            visible_range: 0..request.text().len(),
            truncated: false,
        })?;
        StyledTextLayout::new(request.clone(), layout, Vec::new(), Vec::new())
    }
}
#[derive(Debug)]
struct ExternalFailure(std::io::Error);
impl fmt::Display for ExternalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("external stage failed")
    }
}
impl Error for ExternalFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
fn external_error() -> TextStageError {
    TextStageError::extension(ExternalFailure(std::io::Error::other("original cause")))
}
impl TextLayoutEngine for Engine {
    fn revision(&self) -> u64 {
        self.revision + u64::from(matches!(self.lower_change(), Some(Change::Revision)))
    }
    fn cache_identity(&self) -> Option<TextLayoutCacheIdentity> {
        Some(if matches!(self.lower_change(), Some(Change::Identity)) {
            self.secondary_identity
        } else {
            self.identity
        })
    }
    fn staged_layout(&mut self) -> Option<&mut dyn StagedTextLayoutEngine> {
        if self.available && !matches!(self.lower_change(), Some(Change::Disappear)) {
            Some(self)
        } else {
            None
        }
    }
    fn layout_styled(
        &mut self,
        request: &StyledTextRequest,
    ) -> Result<StyledTextLayout, TextError> {
        self.state.borrow_mut().calls[0] += 1;
        if self.state.borrow().full_error {
            return Err(TextError::InvalidFontData);
        }
        self.output(request)
    }
    fn register_font(&mut self, _: ResourceId, _: u64, _: &[u8]) -> Result<(), TextError> {
        self.revision += 1;
        Ok(())
    }
    fn remove_font(&mut self, _: ResourceId) {}
    fn layout(
        &mut self,
        _: &str,
        _: &FontDescriptor,
        _: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        Err(TextError::StyledLayoutUnsupported)
    }
}
impl StagedTextLayoutEngine for Engine {
    fn analysis_key(&self) -> ErasedTextAnalysisKey {
        if self.other_key || matches!(self.lower_change(), Some(Change::Key)) {
            OTHER_KEY.erase()
        } else {
            KEY.erase()
        }
    }
    fn analysis_registry(&self) -> &TextAnalysisRegistry {
        if matches!(self.lower_change(), Some(Change::Registry)) {
            &self.secondary_registry
        } else {
            &self.registry
        }
    }
    fn analysis_admission_lower_bound(
        &self,
        _: &TextAnalysisRequest,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisAdmissionCost>, TextStageError> {
        self.state.borrow_mut().calls[1] += 1;
        context.charge(
            "external-lower",
            TextStageCostPlan {
                work: 1,
                allocation: 0,
            },
        )?;
        // Getter is immutable; tests use interior controls to exercise invalid callbacks too.
        let fault = self
            .state
            .borrow()
            .fault
            .filter(|f| f.phase == Phase::Lower);
        // Construct the checked value before injecting exhaustion. The matrix
        // intentionally returns Value/None/Error after a fault; a charged Core
        // query after exhaustion would turn the Value case into another Error.
        let lower = self
            .state
            .borrow()
            .lower_bytes
            .map(|bytes| self.registry.admission_lower_bound(context, bytes, 1))
            .transpose()?;
        if fault.is_some_and(|f| f.budget) {
            let _ = context
                .budget()
                .consume("ignored lower exhaustion", u64::MAX);
        }
        if fault.is_some_and(|f| matches!(f.returned, Return::Error)) {
            return Err(external_error());
        }
        if fault.is_some_and(|f| matches!(f.returned, Return::None)) {
            return Ok(None);
        }
        Ok(lower)
    }
    fn analyze(
        &mut self,
        request: &TextAnalysisRequest,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisLease>, TextStageError> {
        self.state.borrow_mut().calls[2] += 1;
        self.state.borrow_mut().context = Some(context.clone());
        context.charge(
            "external-analyze",
            TextStageCostPlan {
                work: request.request().text().len() as u64,
                allocation: request.request().text().len() as u64,
            },
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(request.request().text().len())
            .map_err(|_| TextStageError::StorageReservation)?;
        bytes.extend_from_slice(request.request().text().as_bytes());
        let shared = self.state.borrow().shared.clone();
        let lease = self
            .registry
            .insert(KEY, Payload { bytes, shared }, request, context)?;
        self.state.borrow_mut().lease = lease.clone();
        match self.after(Phase::Analyze, context) {
            Return::Value => Ok(self.state.borrow().foreign_lease.clone().or(lease)),
            Return::None => Ok(None),
            Return::Error => Err(external_error()),
        }
    }
    fn reflow(
        &mut self,
        lease: &TextAnalysisLease,
        request: &StyledTextRequest,
        context: &TextStageContext,
    ) -> Result<StyledTextLayout, TextStageError> {
        self.state.borrow_mut().calls[3] += 1;
        let payload = self.registry.read(KEY, lease, context)?;
        assert_eq!(&payload.bytes, request.text().as_bytes());
        for step in 0..3 {
            if self.state.borrow().missing_plan == Some(step) {
                return Err(TextStageError::ReflowAccountingUnavailable);
            }
            self.state
                .borrow_mut()
                .before_steps
                .push(context.budget().used());
            context.charge(
                ["external-clone", "external-break", "external-export"][step],
                TextStageCostPlan {
                    work: request.text().len() as u64,
                    allocation: request.text().len() as u64 + 1,
                },
            )?;
            self.state.borrow_mut().steps += 1;
        }
        let output = if self.state.borrow().invalid_output {
            self.output(&input("foreign input", Some(13.0)))
        } else {
            self.output(request)
        }
        .map_err(TextStageError::extension)?;
        match self.after(Phase::Reflow, context) {
            Return::Error => Err(external_error()),
            _ => Ok(output),
        }
    }
}
fn input(text: &str, width: Option<f32>) -> StyledTextRequest {
    StyledTextRequest::new(
        text,
        TextStyle::new(FontDescriptor::default(), Color::BLACK),
        Vec::new(),
        Vec::new(),
        TextConstraints::new(width).unwrap(),
    )
    .unwrap()
}
fn session() -> TextStageContext {
    TextStageContext::new(
        PreparationBudgetHandle::default(),
        TextLayoutDiagnostics::default(),
    )
}
fn run(
    system: &mut TextSystem,
    request: &StyledTextRequest,
    context: &TextStageContext,
) -> Result<StyledTextLayout, TextError> {
    system
        .measure(FontDescriptor::default())
        .layout_styled_in_session(request, context)
}
fn failure(error: &TextError) -> &TextStageFailure {
    let TextError::Stage(failure) = error else {
        panic!("expected stage failure: {error:?}");
    };
    failure
}

#[test]
fn external_engine_reuses_only_analysis_and_returns_current_width() {
    let (engine, state) = Engine::new(LIMITS);
    let mut system = TextSystem::with_engine(Box::new(engine));
    let first = run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    let second = run(&mut system, &input("abcdef", Some(4.0)), &session()).unwrap();
    assert_eq!(first.layout().width(), 2.0);
    assert_eq!(second.layout().width(), 4.0);
    assert_eq!(state.borrow().calls, [0, 1, 1, 2]);
    run(
        &mut system,
        &input("content changed", Some(4.0)),
        &session(),
    )
    .unwrap();
    assert_eq!(state.borrow().calls, [0, 2, 2, 3]);
}

#[test]
fn configured_zero_and_missing_capability_go_directly_to_complete_entry() {
    for (limits, available) in [
        (
            TextAnalysisCacheLimits {
                entries: 0,
                ..LIMITS
            },
            true,
        ),
        (
            TextAnalysisCacheLimits {
                retained_bytes: 0,
                ..LIMITS
            },
            true,
        ),
        (LIMITS, false),
    ] {
        let (mut engine, state) = Engine::new(limits);
        engine.available = available;
        let mut system = TextSystem::with_engine(Box::new(engine));
        run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
        assert_eq!(state.borrow().calls, [1, 0, 0, 0]);
        state.borrow_mut().full_error = true;
        assert_eq!(
            run(&mut system, &input("abcdef", Some(4.0)), &session()).unwrap_err(),
            TextError::InvalidFontData
        );
    }
}

#[test]
fn valid_hit_at_exact_byte_limit_does_not_probe_or_analyze() {
    let limits = TextAnalysisCacheLimits {
        entries: 1,
        ..LIMITS
    };
    let (engine, state) = Engine::new(limits);
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    let bytes = state
        .borrow()
        .lease
        .as_ref()
        .unwrap()
        .usage()
        .retained_bytes;
    let (engine, state) = Engine::new(TextAnalysisCacheLimits {
        retained_bytes: bytes,
        ..limits
    });
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    assert_eq!(
        state
            .borrow()
            .lease
            .as_ref()
            .unwrap()
            .usage()
            .retained_bytes,
        bytes
    );
    state.borrow_mut().lower_bytes = Some(usize::MAX / 2);
    run(&mut system, &input("abcdef", Some(4.0)), &session()).unwrap();
    assert_eq!(state.borrow().calls, [0, 1, 1, 2]);
    let exhausted = TextStageContext::new(
        PreparationBudgetHandle::new(PreparationBudget::new(1, 0)),
        TextLayoutDiagnostics::default(),
    );
    let error = run(&mut system, &input("abcdef", Some(5.0)), &exhausted).unwrap_err();
    assert_eq!(failure(&error).primary, TextStageFailureKind::Budget);
    assert!(exhausted.budget().check().is_err());
    assert_eq!(state.borrow().calls, [0, 1, 1, 2]);
}

#[test]
fn certain_miss_rejection_skips_analysis_and_falls_back_once() {
    let (engine, state) = Engine::new(LIMITS);
    state.borrow_mut().lower_bytes = Some(LIMITS.retained_bytes + 1);
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    assert_eq!(state.borrow().calls, [1, 1, 0, 0]);
}

#[test]
fn successful_absence_falls_back_once_with_the_same_session() {
    let (engine, state) = Engine::new(LIMITS);
    state.borrow_mut().fault = Some(Fault {
        phase: Phase::Analyze,
        change: None,
        returned: Return::None,
        budget: false,
    });
    let mut system = TextSystem::with_engine(Box::new(engine));
    let context = session();
    run(&mut system, &input("abcdef", Some(2.0)), &context).unwrap();
    assert_eq!(state.borrow().calls, [1, 1, 1, 0]);
    assert!(context.budget().used().work > 0);
}

#[test]
fn returned_value_none_and_error_all_receive_binding_and_budget_checks() {
    for phase in [Phase::Lower, Phase::Analyze, Phase::Reflow] {
        for change in [
            Change::Identity,
            Change::Revision,
            Change::Key,
            Change::Registry,
            Change::Disappear,
        ] {
            for returned in [Return::Value, Return::None, Return::Error] {
                for budget in [false, true] {
                    let (engine, state) = Engine::new(LIMITS);
                    if phase == Phase::Lower && matches!(returned, Return::Value) {
                        state.borrow_mut().lower_bytes = Some(0);
                    }
                    state.borrow_mut().fault = Some(Fault {
                        phase,
                        change: Some(change),
                        returned,
                        budget,
                    });
                    let mut system = TextSystem::with_engine(Box::new(engine));
                    let context = session();
                    let error =
                        run(&mut system, &input("abcdef", Some(2.0)), &context).unwrap_err();
                    let failure = failure(&error);
                    assert_eq!(
                        failure.primary,
                        if budget {
                            TextStageFailureKind::Budget
                        } else {
                            TextStageFailureKind::Binding
                        }
                    );
                    assert!(
                        failure.binding_failure.is_some(),
                        "{phase:?} {change:?} {returned:?}"
                    );
                    assert_eq!(failure.budget_failure.is_some(), budget);
                    assert_eq!(
                        failure.stage_error.is_some(),
                        matches!(returned, Return::Error)
                    );
                    assert_eq!(state.borrow().calls[0], 0);
                    if matches!(returned, Return::Error) {
                        let raw = failure.source().unwrap().source().unwrap();
                        assert!(raw.is::<ExternalFailure>());
                        assert!(raw.source().unwrap().is::<std::io::Error>());
                    }
                    if budget {
                        assert!(
                            context
                                .budget()
                                .consume("after failed callback", 0)
                                .is_err()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn lower_error_or_ignored_budget_failure_never_analyzes_or_falls_back() {
    for returned in [Return::Value, Return::None, Return::Error] {
        for budget in [false, true] {
            if !budget && !matches!(returned, Return::Error) {
                continue;
            }
            let (engine, state) = Engine::new(LIMITS);
            state.borrow_mut().fault = Some(Fault {
                phase: Phase::Lower,
                change: None,
                returned,
                budget,
            });
            let mut system = TextSystem::with_engine(Box::new(engine));
            let error = run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap_err();
            assert_eq!(
                failure(&error).primary,
                if budget {
                    TextStageFailureKind::Budget
                } else {
                    TextStageFailureKind::Stage
                }
            );
            assert_eq!(
                failure(&error).stage_error.is_some(),
                matches!(returned, Return::Error)
            );
            assert_eq!(state.borrow().calls, [0, 1, 0, 0]);
        }
    }
}

#[test]
fn missing_clone_break_or_export_plan_refuses_before_that_step() {
    for step in 0..3 {
        let (engine, state) = Engine::new(LIMITS);
        state.borrow_mut().missing_plan = Some(step);
        let mut system = TextSystem::with_engine(Box::new(engine));
        let error = run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap_err();
        assert_eq!(
            failure(&error).stage_error,
            Some(TextStageError::ReflowAccountingUnavailable)
        );
        assert_eq!(state.borrow().steps, step);
        assert_eq!(state.borrow().calls[0], 0);
    }
}

#[test]
fn clone_break_and_export_each_reject_an_insufficient_session_before_work() {
    let request = input("abcdef", Some(2.0));
    let (engine, state) = Engine::new(LIMITS);
    run(
        &mut TextSystem::with_engine(Box::new(engine)),
        &request,
        &session(),
    )
    .unwrap();
    let before = state.borrow().before_steps.clone();
    for (step, used) in before.iter().enumerate() {
        let (engine, state) = Engine::new(LIMITS);
        let mut system = TextSystem::with_engine(Box::new(engine));
        let context = TextStageContext::new(
            PreparationBudgetHandle::new(PreparationBudget::new(used.work + 5, u64::MAX)),
            TextLayoutDiagnostics::default(),
        );
        let error = run(&mut system, &request, &context).unwrap_err();
        assert_eq!(failure(&error).primary, TextStageFailureKind::Budget);
        assert_eq!(state.borrow().steps, step);
        assert_eq!(state.borrow().calls[0], 0);
        assert!(context.budget().check().is_err());
    }
}

#[test]
fn current_complete_output_is_checked_after_reflow() {
    let (engine, state) = Engine::new(LIMITS);
    state.borrow_mut().invalid_output = true;
    let mut system = TextSystem::with_engine(Box::new(engine));
    let error = run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap_err();
    assert_eq!(
        failure(&error).validation_error,
        Some(TextStageError::InvalidOutput)
    );
    assert_eq!(state.borrow().calls[0], 0);
}

#[test]
fn typed_read_checks_key_request_revision_and_registry_even_after_eviction() {
    let (engine, state) = Engine::new(LIMITS);
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    let (registry, context, lease) = {
        let s = state.borrow();
        (
            s.registry.clone().unwrap(),
            s.context.clone().unwrap(),
            s.lease.clone().unwrap(),
        )
    };
    assert_eq!(
        KEY.erase().diagnostic_name(),
        OTHER_KEY.erase().diagnostic_name()
    );
    assert!(matches!(
        registry.read(OTHER_KEY, &lease, &context),
        Err(TextStageError::KeyMismatch)
    ));
    registry.clear();
    assert!(registry.find(&context).unwrap().is_none());
    assert_eq!(
        registry.read(KEY, &lease, &context).unwrap().bytes,
        b"abcdef"
    );
    let foreign = TextAnalysisRegistry::new(TextLayoutCacheIdentity::default(), LIMITS).unwrap();
    assert!(matches!(
        foreign.read(KEY, &lease, &context),
        Err(TextStageError::EngineIdentityMismatch)
    ));
    run(&mut system, &input("different", Some(2.0)), &session()).unwrap();
    let new_context = state.borrow().context.clone().unwrap();
    assert!(matches!(
        registry.read(KEY, &lease, &new_context),
        Err(TextStageError::InputMismatch)
    ));
    assert!(matches!(
        registry.insert(
            KEY,
            Payload {
                bytes: Vec::new(),
                shared: None
            },
            lease.request(),
            &new_context
        ),
        Err(TextStageError::InputMismatch)
    ));
}

#[test]
fn evicted_lease_pins_entry_quota_and_registry_ledger_outlives_shell() {
    let (engine, state) = Engine::new(TextAnalysisCacheLimits {
        entries: 1,
        ..LIMITS
    });
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    let old = state.borrow_mut().lease.take().unwrap();
    let registry = state.borrow_mut().registry.take().unwrap();
    let before = registry.usage();
    registry.clear();
    assert_eq!(registry.usage().entries, 1);
    run(&mut system, &input("different", Some(2.0)), &session()).unwrap();
    assert_eq!(state.borrow().calls, [1, 2, 2, 1]);
    drop(old);
    assert_eq!(registry.usage().entries, 0);
    run(&mut system, &input("different", Some(2.0)), &session()).unwrap();
    let retained = state.borrow_mut().lease.take().unwrap();
    registry.clear();
    drop(system);
    drop(registry);
    assert_eq!(retained.usage().entries, 1);
    assert!(retained.usage().retained_bytes < before.retained_bytes);
}

#[test]
fn shared_backings_deduplicate_and_release_only_with_last_entry() {
    let (engine, state) = Engine::new(LIMITS);
    let data = Rc::new(vec![0u8; 1024]);
    state.borrow_mut().shared = Some((TextAnalysisBackingIdentity::default(), data));
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("aaaaaa", Some(2.0)), &session()).unwrap();
    let first = state.borrow_mut().lease.take().unwrap();
    let registry = state.borrow().registry.clone().unwrap();
    let first_bytes = registry.usage().retained_bytes;
    run(&mut system, &input("bbbbbb", Some(2.0)), &session()).unwrap();
    let second = state.borrow_mut().lease.take().unwrap();
    let second_bytes = registry.usage().retained_bytes;
    assert!(second_bytes - first_bytes < 1024);
    registry.clear();
    drop(first);
    assert_eq!(registry.usage().entries, 1);
    let pinned = registry.usage().retained_bytes;
    drop(second);
    assert_eq!(registry.usage().entries, 0);
    assert!(pinned - registry.usage().retained_bytes > 1024);
    // Backing table and cache capacities still exist and remain charged.
    assert!(registry.usage().retained_bytes > 0);
}

#[test]
fn conflicting_backing_description_rejects_without_changing_ledger() {
    let (engine, state) = Engine::new(LIMITS);
    let id = TextAnalysisBackingIdentity::default();
    state.borrow_mut().shared = Some((id, Rc::new(vec![0u8; 1024])));
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("aaaaaa", Some(2.0)), &session()).unwrap();
    let registry = state.borrow().registry.clone().unwrap();
    let before = registry.usage();
    state.borrow_mut().shared = Some((id, Rc::new(vec![0u8; 2048])));
    let error = run(&mut system, &input("bbbbbb", Some(2.0)), &session()).unwrap_err();
    assert_eq!(
        failure(&error).stage_error,
        Some(TextStageError::BackingDescriptionConflict)
    );
    assert_eq!(registry.usage(), before);
}

#[test]
fn analysis_projection_retains_every_dependency_except_width() {
    let base = input("abcdef", Some(0.0));
    assert_eq!(
        TextAnalysisRequest::new(&base).unwrap(),
        TextAnalysisRequest::new(&input("abcdef", None)).unwrap()
    );
    for changed in [
        input("abcdeg", Some(0.0)),
        base.clone().with_paragraph(
            ParagraphDirection::RightToLeft,
            ParagraphAlignment::Leading,
            true,
        ),
        base.clone()
            .with_paragraph(ParagraphDirection::Auto, ParagraphAlignment::Center, true),
        base.clone()
            .with_paragraph(ParagraphDirection::Auto, ParagraphAlignment::Leading, false),
        base.clone()
            .with_layout_direction(novadraw::figure::LayoutDirection::RightToLeft),
        StyledTextRequest::new(
            "abcdef",
            base.default_style().clone(),
            vec![TextStyleSpan {
                range: 0..1,
                style: TextStyle::new(FontDescriptor::default(), Color::WHITE),
            }],
            Vec::new(),
            base.constraints(),
        )
        .unwrap(),
        StyledTextRequest::new(
            "abcdef",
            base.default_style().clone(),
            Vec::new(),
            vec![TextInlineBox {
                id: 1,
                byte_offset: 0,
                width: 1.0,
                height: 1.0,
            }],
            base.constraints(),
        )
        .unwrap(),
    ] {
        assert_ne!(
            TextAnalysisRequest::new(&base).unwrap(),
            TextAnalysisRequest::new(&changed).unwrap()
        );
    }
}

#[test]
fn registry_metadata_and_arithmetic_are_bounded_before_allocation() {
    assert!(matches!(
        TextAnalysisRegistry::new(
            TextLayoutCacheIdentity::default(),
            TextAnalysisCacheLimits {
                entries: usize::MAX,
                retained_bytes: usize::MAX
            }
        ),
        Err(TextStageError::AccountingOverflow)
    ));
    assert!(matches!(
        TextAnalysisRegistry::new(
            TextLayoutCacheIdentity::default(),
            TextAnalysisCacheLimits {
                entries: 1,
                retained_bytes: 1
            }
        ),
        Err(TextStageError::CacheMetadataLimit)
    ));
}

#[test]
fn shared_content_is_charged_once_across_distinct_analysis_entries() {
    let request = input("abcdef", Some(2.0));
    let shared =
        request
            .clone()
            .with_paragraph(ParagraphDirection::Auto, ParagraphAlignment::Center, true);
    let separate = StyledTextRequest::new(
        request.text(),
        request.default_style().clone(),
        request.spans().to_vec(),
        request.inline_boxes().to_vec(),
        request.constraints(),
    )
    .unwrap()
    .with_paragraph(ParagraphDirection::Auto, ParagraphAlignment::Center, true);
    assert_eq!(shared, separate);
    let retained = |second: &StyledTextRequest| {
        let (engine, state) = Engine::new(LIMITS);
        let mut system = TextSystem::with_engine(Box::new(engine));
        run(&mut system, &request, &session()).unwrap();
        run(&mut system, second, &session()).unwrap();
        let usage = state.borrow().registry.as_ref().unwrap().usage();
        assert_eq!(usage.entries, 2);
        usage.retained_bytes
    };
    let shared_bytes = retained(&shared);
    let separate_bytes = retained(&separate);
    assert!(separate_bytes - shared_bytes >= separate.content().retained_units());
}

#[test]
fn stage_errors_preserve_the_existing_send_sync_text_error_boundary() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<TextError>();
    let error = external_error();
    assert_eq!(error, error.clone());
    assert_ne!(error, external_error());
}

#[test]
fn foreign_returned_lease_is_rejected_without_full_fallback() {
    let (engine, state) = Engine::new(LIMITS);
    let mut first = TextSystem::with_engine(Box::new(engine));
    run(&mut first, &input("abcdef", Some(2.0)), &session()).unwrap();
    let foreign = state.borrow().lease.clone().unwrap();
    let (engine, state) = Engine::new(LIMITS);
    state.borrow_mut().foreign_lease = Some(foreign);
    let mut second = TextSystem::with_engine(Box::new(engine));
    let error = run(&mut second, &input("abcdef", Some(2.0)), &session()).unwrap_err();
    assert_eq!(
        failure(&error).validation_error,
        Some(TextStageError::RegistryIdentityMismatch)
    );
    assert_eq!(state.borrow().calls, [0, 1, 1, 0]);
}

#[test]
fn font_revision_change_is_a_cache_miss_and_old_lease_read_fails() {
    let (engine, state) = Engine::new(LIMITS);
    let mut system = TextSystem::with_engine(Box::new(engine));
    run(&mut system, &input("abcdef", Some(2.0)), &session()).unwrap();
    let old = state.borrow().lease.clone().unwrap();
    system
        .register_builtin_font(novadraw::text::BuiltinFont::Inter)
        .unwrap();
    let output = run(&mut system, &input("abcdef", Some(4.0)), &session()).unwrap();
    assert_eq!(output.layout().key().engine_revision(), 1);
    assert_eq!(state.borrow().calls, [0, 2, 2, 2]);
    let state = state.borrow();
    assert!(matches!(
        state
            .registry
            .as_ref()
            .unwrap()
            .read(KEY, &old, state.context.as_ref().unwrap()),
        Err(TextStageError::RevisionMismatch)
    ));
}
