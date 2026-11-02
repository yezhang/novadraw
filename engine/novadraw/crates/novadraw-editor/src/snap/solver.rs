use super::{
    DofKey, EvaluationContext, FeatureId, FeatureSink, ProtocolKey, RuleCall, SnapError,
    SnapFeature, SnapGeometry, SnapInput,
    registry::{ErasedProvider, ErasedStrategy, Instance, Registry, SinkFailure},
};
use crate::{EditPartId, EditorNamespace, ModelRevision, container::CandidateId};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};
use uuid::Uuid;

/// Owned typed feedback declaration consumed through an ordinary renderer protocol.
#[derive(Clone)]
pub struct FeedbackDeclaration {
    payload: Rc<dyn Any>,
}
impl FeedbackDeclaration {
    pub(crate) fn erased_payload(&self) -> &dyn Any {
        self.payload.as_ref()
    }
    /// Stores an owned payload without Clone or thread bounds.
    pub fn new<T: 'static>(_key: ProtocolKey<T>, payload: T) -> Self {
        Self {
            payload: Rc::new(payload),
        }
    }
    /// Reads a declaration by the same typed protocol identity.
    pub fn payload<T: 'static>(&self, _key: ProtocolKey<T>) -> Option<&T> {
        self.payload.downcast_ref()
    }
}

/// Unchecked candidate declaration contributed by built-in or external strategies.
pub struct SnapCandidate {
    feature: FeatureId,
    stable: u64,
    writes: Vec<(DofKey, f64)>,
    dependencies: Vec<DofKey>,
    admission: RuleCall,
    alignment: RuleCall,
    feedback: Vec<FeedbackDeclaration>,
    filter_admission: bool,
}
impl SnapCandidate {
    /// Declares one indivisible correction with its full alignment dependencies.
    pub fn new(
        feature: FeatureId,
        stable: u64,
        writes: Vec<(DofKey, f64)>,
        dependencies: Vec<DofKey>,
        admission: RuleCall,
        alignment: RuleCall,
        feedback: Vec<FeedbackDeclaration>,
    ) -> Self {
        Self {
            feature,
            stable,
            writes,
            dependencies,
            admission,
            alignment,
            feedback,
            filter_admission: false,
        }
    }
}

pub(crate) struct CollectedCandidate {
    strategy: usize,
    provider: usize,
    candidate: SnapCandidate,
    distance: f64,
}
/// Bounded candidate contribution sink. Ignoring a push error cannot recover partial success.
pub struct CandidateSink<'a> {
    pub(crate) strategy: usize,
    pub(crate) admission: Instance,
    pub(crate) input: &'a SnapInput,
    pub(crate) features: &'a [SnapFeature],
    pub(crate) feature_orders: &'a HashMap<FeatureId, usize>,
    pub(crate) identities: &'a mut HashSet<(usize, FeatureId, u64)>,
    pub(crate) feedback_contributions: &'a mut usize,
    pub(crate) output: &'a mut Vec<CollectedCandidate>,
    pub(crate) failure: Option<SinkFailure>,
}
impl CandidateSink<'_> {
    /// Returns all checked provider features available to this strategy.
    pub fn features(&self) -> &[SnapFeature] {
        self.features
    }
    /// Contributes a candidate. Declaration checks are shared by every strategy.
    pub fn push(&mut self, candidate: SnapCandidate) -> Result<(), SnapError> {
        let result = self.push_inner(candidate);
        if let Err(error) = &result {
            self.failure.get_or_insert(SinkFailure::capture(error));
        }
        result
    }
    /// Offers a geometric proposal for checked admission filtering before arbitration.
    /// A false authorized rule means legitimate nonparticipation; malformed declarations,
    /// wrong authorization, evaluator errors and panic still reject the whole solve.
    /// Unlike `push`, this does not assert that the proposal already satisfies admission.
    pub fn propose(&mut self, mut proposal: SnapCandidate) -> Result<(), SnapError> {
        proposal.filter_admission = true;
        self.push(proposal)
    }
    fn push_inner(&mut self, candidate: SnapCandidate) -> Result<(), SnapError> {
        if let Some(failure) = self.failure {
            return Err(failure.error());
        }
        if self.output.len() >= self.input.settings.budget().candidates.get() {
            return Err(SnapError::Budget("candidate budget exceeded"));
        }
        let feedback_count = self
            .feedback_contributions
            .checked_add(candidate.feedback.len())
            .ok_or(SnapError::Budget("feedback count overflow"))?;
        if feedback_count > self.input.settings.budget().feedback.get() {
            return Err(SnapError::Budget("feedback contribution budget exceeded"));
        }
        if candidate.admission.instance != self.admission {
            return Err(SnapError::Identity);
        }
        let provider = *self
            .feature_orders
            .get(&candidate.feature)
            .ok_or(SnapError::Identity)?;
        if candidate.writes.is_empty() || candidate.dependencies.is_empty() {
            return Err(SnapError::Invalid(
                "empty candidate freedoms or dependencies",
            ));
        }
        let mut values = vec![0.0; self.input.dofs.len()];
        let mut writes = Vec::new();
        for &(key, value) in &candidate.writes {
            let index = self
                .input
                .dofs
                .iter()
                .position(|k| *k == key)
                .ok_or(SnapError::Invalid("candidate writes a fixed freedom"))?;
            if !value.is_finite() || writes.contains(&key) {
                return Err(SnapError::Invalid("invalid candidate correction"));
            }
            writes.push(key);
            values[index] = value;
        }
        let mut dependencies = Vec::new();
        for &key in &candidate.dependencies {
            if !self.input.dofs.contains(&key) || dependencies.contains(&key) {
                return Err(SnapError::Invalid("invalid alignment dependency"));
            }
            dependencies.push(key);
        }
        if !self
            .identities
            .insert((self.strategy, candidate.feature, candidate.stable))
        {
            return Err(SnapError::Invalid("duplicate candidate identity"));
        }
        let geometry = self.input.geometry(values)?;
        let distance = EvaluationContext {
            input: self.input,
            geometry: &geometry,
        }
        .capture_distance()?;
        self.output.push(CollectedCandidate {
            strategy: self.strategy,
            provider,
            candidate,
            distance,
        });
        *self.feedback_contributions = feedback_count;
        Ok(())
    }
}

/// Separate counts and callback time for one complete solve or failed prefix.
#[derive(Clone, Debug, Default)]
pub struct SnapMetrics {
    /// Most recently entered extension boundary, also retained when that callback panics.
    pub callback_boundary: Option<&'static str>,
    /// Complete owner snapshot validations, including full sibling membership scans.
    pub owner_checks: usize,
    /// Provider geometry callbacks.
    pub provider_calls: usize,
    /// Strategy preparation callbacks.
    pub prepare_calls: usize,
    /// Checked prepared-dependency comparison callbacks.
    pub reuse_calls: usize,
    /// Prepared assets reused after their complete declared dependencies passed.
    pub cache_hits: usize,
    /// Strategy query callbacks.
    pub query_calls: usize,
    /// Admission, layout and alignment evaluation callbacks.
    pub evaluation_calls: usize,
    /// Read-only version callback invocations.
    pub version_calls: usize,
    /// Candidate declarations attempted by the arbiter.
    pub attempted: usize,
    /// Candidates adopted after complete tentative validation.
    pub adopted: usize,
    /// Valid candidates skipped due to an earlier protected fact or layout conflict.
    pub conflicts: usize,
    /// Accepted owned provider features.
    pub features: usize,
    /// Accepted feedback declarations.
    pub feedback: usize,
    /// Total synchronous extension callback time, including versions.
    /// `None` means no measurement clock was supplied; it does not mean zero cost.
    pub callback_time: Option<Duration>,
}

struct RuleDependency {
    call: RuleCall,
    version: u64,
}
enum Dependency<'a> {
    Rule(RuleDependency),
    Provider(&'a dyn ErasedProvider, u64),
    Strategy(&'a dyn ErasedStrategy, u64),
}
impl Dependency<'_> {
    fn boundary(&self) -> &'static str {
        match self {
            Self::Rule(_) => "snap.rule.version",
            Self::Provider(_, _) => "snap.provider.version",
            Self::Strategy(_, _) => "snap.strategy.version",
        }
    }
    fn current(&self) -> Result<u64, SnapError> {
        match self {
            Self::Rule(d) => d.call.version(),
            Self::Provider(p, _) => p.version(),
            Self::Strategy(s, _) => s.version(),
        }
    }
    fn expected(&self) -> u64 {
        match self {
            Self::Rule(d) => d.version,
            Self::Provider(_, v) | Self::Strategy(_, v) => *v,
        }
    }
}

struct Runner<'a, C> {
    registry: &'a Registry,
    input: &'a SnapInput,
    check: C,
    dependencies: Vec<Dependency<'a>>,
    metrics: &'a mut SnapMetrics,
    clock: Option<Rc<dyn Fn() -> Duration>>,
}

fn measured<T>(
    metrics: &mut SnapMetrics,
    clock: &Option<Rc<dyn Fn() -> Duration>>,
    boundary: &'static str,
    callback: impl FnOnce() -> Result<T, SnapError>,
) -> Result<T, SnapError> {
    let start = clock.as_ref().map(|clock| clock());
    metrics.callback_boundary = Some(boundary);
    let result = callback();
    let timing = if let (Some(start), Some(clock)) = (start, clock) {
        clock()
            .checked_sub(start)
            .and_then(|elapsed| {
                metrics
                    .callback_time
                    .unwrap_or_default()
                    .checked_add(elapsed)
            })
            .map(|total| {
                metrics.callback_time = Some(total);
            })
            .ok_or(SnapError::Invalid("invalid snapping measurement clock"))
    } else {
        Ok(())
    };
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            timing?;
            Ok(value)
        }
    }
}
impl<'a, C: FnMut(&SnapInput) -> Result<(), SnapError>> Runner<'a, C> {
    fn owner_check(&mut self) -> Result<(), SnapError> {
        self.metrics.owner_checks += 1;
        (self.check)(self.input)
    }
    fn validate(&mut self) -> Result<(), SnapError> {
        self.owner_check()?;
        for dependency in &self.dependencies {
            self.metrics.version_calls += 1;
            let current = measured(self.metrics, &self.clock, dependency.boundary(), || {
                dependency.current()
            });
            if current? != dependency.expected() {
                return Err(SnapError::Invalidated);
            }
        }
        self.owner_check()
    }
    fn version(
        &mut self,
        boundary: &'static str,
        call: impl FnOnce() -> Result<u64, SnapError>,
    ) -> Result<u64, SnapError> {
        self.validate()?;
        self.metrics.version_calls += 1;
        let result = measured(self.metrics, &self.clock, boundary, call);
        let validity = self.validate();
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                validity?;
                Ok(value)
            }
        }
    }
    fn callback<T>(
        &mut self,
        boundary: &'static str,
        call: impl FnOnce() -> Result<T, SnapError>,
    ) -> Result<T, SnapError> {
        self.validate()?;
        let result = measured(self.metrics, &self.clock, boundary, call);
        let validity = self.validate();
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                validity?;
                Ok(value)
            }
        }
    }
    fn evaluate(&mut self, call: &RuleCall, geometry: &SnapGeometry) -> Result<bool, SnapError> {
        self.registry.check_rule(call)?;
        if !self
            .dependencies
            .iter()
            .any(|d| matches!(d, Dependency::Rule(r) if r.call.same_application(call)))
        {
            let version = self.version("snap.rule.version", || call.version())?;
            self.dependencies.push(Dependency::Rule(RuleDependency {
                call: call.clone(),
                version,
            }));
        }
        self.metrics.evaluation_calls += 1;
        self.callback("snap.rule.evaluate", || {
            call.evaluate(&EvaluationContext {
                input: self.input,
                geometry,
            })
        })
    }
    fn layouts(&mut self, geometry: &SnapGeometry) -> Result<bool, SnapError> {
        for rule in &self.input.layouts {
            if !self.evaluate(rule, geometry)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Gesture-owned lifecycle, checked prepared assets and failed-prefix metrics.
pub struct SnapSession {
    pub(crate) owner: EditorNamespace,
    pub(crate) id: Uuid,
    pub(crate) active: bool,
    pub(crate) latest_candidate: Option<CandidateId>,
    pub(crate) latest_solution: Option<Uuid>,
    pub(crate) baseline: Option<(
        ModelRevision,
        super::geometry::GestureIdentity,
        EditPartId,
        Vec<EditPartId>,
    )>,
    pub(crate) metrics: SnapMetrics,
    pub(crate) clock: Option<Rc<dyn Fn() -> Duration>>,
    pub(crate) prepared: Vec<PreparedEntry>,
}
pub(crate) struct PreparedEntry {
    instance: Instance,
    version: u64,
    prepared: Box<dyn Any>,
}
impl SnapSession {
    /// Supplies a host-owned monotonic measurement clock, without global state or platform types.
    /// Native callers can use elapsed Instant; Web callers can use their host's performance clock.
    pub fn set_measurement_clock(&mut self, clock: impl Fn() -> Duration + 'static) {
        self.clock = Some(Rc::new(clock));
    }
    /// Cancels the session. A cancelled identity cannot be revived by equal geometry.
    pub fn cancel(&mut self) {
        self.active = false;
        self.latest_solution = None;
        self.prepared.clear();
    }
    /// Returns costs of the latest attempted solve, including failures.
    pub const fn metrics(&self) -> &SnapMetrics {
        &self.metrics
    }
}

struct AdoptedFact {
    feature: FeatureId,
    alignment: RuleCall,
    dependencies: Vec<DofKey>,
    feedback: Vec<FeedbackDeclaration>,
}
/// Framework-issued checked arbitration result; not a model command authorization.
///
/// Actual container-plan geometry must still pass final validation before feedback or commit.
///
/// ```compile_fail
/// use novadraw_editor::snap::SnapSolution;
/// fn forge(solution: &mut SnapSolution) { solution.facts.clear(); }
/// ```
pub struct SnapSolution {
    pub(crate) session: Uuid,
    publication: Uuid,
    pub(crate) registry_revision: Uuid,
    pub(crate) input: SnapInput,
    geometry: SnapGeometry,
    facts: Vec<AdoptedFact>,
    rule_dependencies: Vec<RuleDependency>,
    provider_versions: Vec<(Instance, u64)>,
    strategy_versions: Vec<(Instance, u64)>,
}
impl SnapSolution {
    /// Returns the exact frozen input.
    pub const fn input(&self) -> &SnapInput {
        &self.input
    }
    /// Returns the checked tentative geometry.
    pub const fn geometry(&self) -> &SnapGeometry {
        &self.geometry
    }
    /// Returns accepted facts' source identities in arbitration order.
    pub fn adopted_features(&self) -> impl Iterator<Item = FeatureId> + '_ {
        self.facts.iter().map(|f| f.feature)
    }
    /// Returns accepted feedback declarations, pending actual-plan validation.
    pub fn feedback(&self) -> impl Iterator<Item = &FeedbackDeclaration> {
        self.facts.iter().flat_map(|f| &f.feedback)
    }
}

pub(crate) fn solve<C: FnMut(&SnapInput) -> Result<(), SnapError>>(
    registry: &Registry,
    session: &mut SnapSession,
    input: SnapInput,
    check: C,
) -> Result<SnapSolution, SnapError> {
    // Even a failed recomputation retires the previous publication. Candidate identity alone
    // cannot distinguish two attempts using the same E03 candidate and different settings.
    session.latest_solution = None;
    let mut previous_prepared = std::mem::take(&mut session.prepared);
    session.metrics = SnapMetrics::default();
    if session.clock.is_some() {
        session.metrics.callback_time = Some(Duration::ZERO);
    }
    if !session.active || session.owner != input.namespace {
        return Err(SnapError::Identity);
    }
    session.latest_candidate = Some(input.candidate);
    let baseline = (
        input.revision,
        input.gesture,
        input.anchor,
        input.sources.clone(),
    );
    if session
        .baseline
        .as_ref()
        .is_some_and(|frozen| *frozen != baseline)
    {
        session.cancel();
        return Err(SnapError::Invalidated);
    }
    session.baseline = Some(baseline);
    let source_count = registry
        .providers
        .len()
        .checked_add(registry.strategies.len())
        .and_then(|count| count.checked_add(input.siblings.len()))
        .ok_or(SnapError::Budget("source count overflow"))?;
    if source_count > input.settings.budget().sources.get() {
        return Err(SnapError::Budget("source budget exceeded"));
    }
    // All identities, including bindings, are rejected before any extension callback.
    for s in &registry.strategies {
        if !registry.rules.contains(&s.admission) {
            return Err(SnapError::Identity);
        }
    }
    for rule in &input.layouts {
        registry.check_rule(rule)?;
    }
    let feature_limit = input
        .settings
        .budget()
        .features
        .get()
        .checked_sub(input.scene.len())
        .ok_or(SnapError::Budget("scene feature budget exceeded"))?;
    let mut runner = Runner {
        registry,
        input: &input,
        check,
        dependencies: Vec::new(),
        metrics: &mut session.metrics,
        clock: session.clock.clone(),
    };
    runner.validate()?;
    let mut features = Vec::new();
    let mut feature_identities = HashSet::new();
    let mut provider_versions = Vec::new();
    for (order, provider) in registry.providers.iter().enumerate() {
        let version = runner.version("snap.provider.version", || provider.application.version())?;
        runner
            .dependencies
            .push(Dependency::Provider(provider.application.as_ref(), version));
        provider_versions.push((provider.instance, version));
        let mut sink = FeatureSink {
            provider: provider.instance,
            order,
            limit: feature_limit,
            output: &mut features,
            identities: &mut feature_identities,
            failure: None,
        };
        runner.metrics.provider_calls += 1;
        runner.callback("snap.provider.features", || {
            provider.application.features(&input, &mut sink)
        })?;
        if let Some(failure) = sink.failure {
            return Err(failure.error());
        }
    }
    runner.metrics.features = features.len();
    let feature_orders = features.iter().map(|f| (f.id, f.order)).collect();
    let mut candidates = Vec::new();
    let mut candidate_identities = HashSet::new();
    let mut feedback_contributions = 0;
    let mut strategy_versions = Vec::new();
    let mut next_prepared = Vec::new();
    for (order, strategy) in registry.strategies.iter().enumerate() {
        let version = runner.version("snap.strategy.version", || strategy.application.version())?;
        runner
            .dependencies
            .push(Dependency::Strategy(strategy.application.as_ref(), version));
        strategy_versions.push((strategy.instance, version));
        let previous = previous_prepared
            .iter()
            .position(|entry| entry.instance == strategy.instance && entry.version == version)
            .map(|index| previous_prepared.swap_remove(index));
        let reusable = if let Some(previous) = &previous {
            runner.metrics.reuse_calls += 1;
            runner.callback("snap.strategy.reuse-prepared", || {
                strategy
                    .application
                    .reuse_prepared(previous.prepared.as_ref(), &input, &features)
            })?
        } else {
            false
        };
        let prepared = if reusable {
            runner.metrics.cache_hits += 1;
            previous.ok_or(SnapError::Identity)?.prepared
        } else {
            runner.metrics.prepare_calls += 1;
            runner.callback("snap.strategy.prepare", || {
                strategy.application.prepare(&input, &features)
            })?
        };
        let mut sink = CandidateSink {
            strategy: order,
            admission: strategy.admission,
            input: &input,
            features: &features,
            feature_orders: &feature_orders,
            identities: &mut candidate_identities,
            feedback_contributions: &mut feedback_contributions,
            output: &mut candidates,
            failure: None,
        };
        runner.metrics.query_calls += 1;
        runner.callback("snap.strategy.query", || {
            strategy
                .application
                .query(prepared.as_ref(), &input, &mut sink)
        })?;
        if let Some(failure) = sink.failure {
            return Err(failure.error());
        }
        next_prepared.push(PreparedEntry {
            instance: strategy.instance,
            version,
            prepared,
        });
    }
    candidates.sort_by(|a, b| {
        a.strategy
            .cmp(&b.strategy)
            .then_with(|| a.distance.total_cmp(&b.distance))
            .then_with(|| a.provider.cmp(&b.provider))
            .then_with(|| a.candidate.feature.stable.cmp(&b.candidate.feature.stable))
            .then_with(|| a.candidate.stable.cmp(&b.candidate.stable))
    });
    for c in &candidates {
        registry.check_rule(&c.candidate.admission)?;
        registry.check_rule(&c.candidate.alignment)?;
    }
    let mut values = vec![0.0; input.dofs.len()];
    let mut geometry = input.geometry(values.clone())?;
    let mut occupied = Vec::new();
    let mut protected = Vec::new();
    let mut facts: Vec<AdoptedFact> = Vec::new();
    for collected in candidates {
        runner.metrics.attempted += 1;
        let candidate = collected.candidate;
        let mut individual = vec![0.0; input.dofs.len()];
        for &(key, value) in &candidate.writes {
            let index = input
                .dofs
                .iter()
                .position(|k| *k == key)
                .ok_or(SnapError::Invalid("fixed freedom"))?;
            individual[index] = value;
        }
        let individual = input.geometry(individual)?;
        if !runner.evaluate(&candidate.admission, &individual)? {
            if candidate.filter_admission {
                continue;
            }
            return Err(SnapError::Invalid(
                "candidate violates authorized admission",
            ));
        }
        if !runner.evaluate(&candidate.alignment, &individual)? {
            return Err(SnapError::Invalid(
                "candidate's own alignment fact is false",
            ));
        }
        if candidate
            .writes
            .iter()
            .any(|(key, _)| occupied.contains(key) || protected.contains(key))
        {
            runner.metrics.conflicts += 1;
            continue;
        }
        let mut tentative = values.clone();
        for &(key, value) in &candidate.writes {
            let index = input
                .dofs
                .iter()
                .position(|k| *k == key)
                .ok_or(SnapError::Invalid("fixed freedom"))?;
            tentative[index] = value;
        }
        let tentative_geometry = input.geometry(tentative.clone())?;
        let mut compatible = runner.layouts(&tentative_geometry)?;
        // Reevaluate all earlier facts, even when declared dependency sets appear disjoint.
        for fact in &facts {
            if !runner.evaluate(&fact.alignment, &tentative_geometry)? {
                compatible = false;
            }
        }
        if !runner.evaluate(&candidate.alignment, &tentative_geometry)? {
            compatible = false;
        }
        if !compatible {
            runner.metrics.conflicts += 1;
            continue;
        }
        let feedback_count = runner
            .metrics
            .feedback
            .checked_add(candidate.feedback.len())
            .ok_or(SnapError::Budget("feedback count overflow"))?;
        if feedback_count > input.settings.budget().feedback.get() {
            return Err(SnapError::Budget("feedback budget exceeded"));
        }
        runner.metrics.feedback = feedback_count;
        for &(key, _) in &candidate.writes {
            occupied.push(key);
        }
        protected.extend(candidate.dependencies.iter().copied());
        facts.push(AdoptedFact {
            feature: candidate.feature,
            alignment: candidate.alignment,
            dependencies: candidate.dependencies,
            feedback: candidate.feedback,
        });
        values = tentative;
        geometry = tentative_geometry;
        runner.metrics.adopted += 1;
    }
    if !runner.layouts(&geometry)? {
        return Err(SnapError::Rejected(
            "solved geometry violates layout constraint",
        ));
    }
    runner.validate()?;
    let rule_dependencies = runner
        .dependencies
        .into_iter()
        .filter_map(|d| {
            if let Dependency::Rule(r) = d {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    let publication = Uuid::new_v4();
    session.latest_solution = Some(publication);
    session.prepared = next_prepared;
    Ok(SnapSolution {
        session: session.id,
        publication,
        registry_revision: registry.revision,
        input,
        geometry,
        facts,
        rule_dependencies,
        provider_versions,
        strategy_versions,
    })
}

pub(crate) fn validate_final<C: FnMut(&SnapInput) -> Result<(), SnapError>>(
    registry: &Registry,
    session: &mut SnapSession,
    solution: &SnapSolution,
    values: Vec<f64>,
    check: C,
) -> Result<SnapGeometry, SnapError> {
    let geometry = solution.input.geometry(values)?;
    validate_final_geometry(registry, session, solution, geometry, check)
}

pub(crate) fn validate_final_geometry<C: FnMut(&SnapInput) -> Result<(), SnapError>>(
    registry: &Registry,
    session: &mut SnapSession,
    solution: &SnapSolution,
    geometry: SnapGeometry,
    check: C,
) -> Result<SnapGeometry, SnapError> {
    if !session.active
        || session.id != solution.session
        || session.latest_solution != Some(solution.publication)
        || session.latest_candidate != Some(solution.input.candidate)
        || registry.revision != solution.registry_revision
    {
        return Err(SnapError::Invalidated);
    }
    let mut runner = Runner {
        registry,
        input: &solution.input,
        check,
        dependencies: Vec::new(),
        metrics: &mut session.metrics,
        clock: session.clock.clone(),
    };
    for r in &solution.rule_dependencies {
        registry.check_rule(&r.call)?;
        runner.dependencies.push(Dependency::Rule(RuleDependency {
            call: r.call.clone(),
            version: r.version,
        }));
    }
    for &(id, v) in &solution.provider_versions {
        let p = registry
            .providers
            .iter()
            .find(|p| p.instance == id)
            .ok_or(SnapError::Identity)?;
        runner
            .dependencies
            .push(Dependency::Provider(p.application.as_ref(), v));
    }
    for &(id, v) in &solution.strategy_versions {
        let s = registry
            .strategies
            .iter()
            .find(|s| s.instance == id)
            .ok_or(SnapError::Identity)?;
        runner
            .dependencies
            .push(Dependency::Strategy(s.application.as_ref(), v));
    }
    runner.validate()?;
    if !runner.layouts(&geometry)? {
        return Err(SnapError::Rejected("final plan violates layout"));
    }
    for fact in &solution.facts {
        // Keep the full declaration alive for future operation forms and diagnostics.
        if fact
            .dependencies
            .iter()
            .any(|d| !solution.input.dofs.contains(d))
        {
            return Err(SnapError::Invalid("invalid retained alignment dependency"));
        }
        if !runner.evaluate(&fact.alignment, &geometry)? {
            return Err(SnapError::Rejected("final plan destroys adopted alignment"));
        }
    }
    runner.validate()?;
    Ok(geometry)
}
