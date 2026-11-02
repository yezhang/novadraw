use super::{CandidateSink, EvaluationContext, SnapError, SnapInput};
use crate::EditorNamespace;
use novadraw::geometry::Point;
use std::{any::Any, collections::HashSet, error::Error, fmt, marker::PhantomData, rc::Rc};
use uuid::Uuid;

/// Typed identity and diagnostic name shared by every snapping extension family.
pub struct ProtocolKey<T> {
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}
impl<T> ProtocolKey<T> {
    /// Creates an identity whose type, rather than diagnostic string, selects the protocol.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
    /// Returns the diagnostic name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Instance {
    owner: EditorNamespace,
    id: Uuid,
    generation: Uuid,
}
impl Instance {
    pub(crate) fn new(owner: EditorNamespace) -> Self {
        Self {
            owner,
            id: Uuid::new_v4(),
            generation: Uuid::new_v4(),
        }
    }
    pub(crate) fn replaced(self) -> Self {
        Self {
            generation: Uuid::new_v4(),
            ..self
        }
    }
}

/// Open, fallible geometric evaluator used for admission, layout and alignment facts.
pub trait EvaluationRule: 'static {
    /// Owned invocation parameters; no Clone, Send or Sync bound is required.
    type Payload: 'static;
    /// Original domain failure.
    type Error: Error + 'static;
    /// Returns the typed family key.
    fn key() -> ProtocolKey<Self>
    where
        Self: Sized;
    /// Returns the version of all independently mutable dependencies of this invocation.
    /// Immutable rules return a constant; undeclared captured mutations violate the contract.
    fn version(&self, payload: &Self::Payload) -> Result<u64, Self::Error>;
    /// Evaluates the declaration against frozen input and current geometry.
    fn evaluate(
        &self,
        payload: &Self::Payload,
        context: &EvaluationContext<'_>,
    ) -> Result<bool, Self::Error>;
}

/// Configured evaluator instance owned by one Viewer registry.
pub struct RuleBinding<R: EvaluationRule> {
    instance: Instance,
    rule: Rc<R>,
}
impl<R: EvaluationRule> Clone for RuleBinding<R> {
    fn clone(&self) -> Self {
        Self {
            instance: self.instance,
            rule: self.rule.clone(),
        }
    }
}
impl<R: EvaluationRule> RuleBinding<R> {
    /// Creates an unchecked declaration. Invocation still requires a live registry identity.
    pub fn bind(&self, payload: R::Payload) -> RuleCall {
        RuleCall {
            instance: self.instance,
            application: Rc::new(RuleApplication {
                rule: self.rule.clone(),
                payload,
            }),
        }
    }
}

trait ErasedRuleApplication {
    fn version(&self) -> Result<u64, SnapError>;
    fn evaluate(&self, context: &EvaluationContext<'_>) -> Result<bool, SnapError>;
}
struct RuleApplication<R: EvaluationRule> {
    rule: Rc<R>,
    payload: R::Payload,
}
impl<R: EvaluationRule> ErasedRuleApplication for RuleApplication<R> {
    fn version(&self) -> Result<u64, SnapError> {
        self.rule
            .version(&self.payload)
            .map_err(|e| SnapError::extension("snap.rule.version", e))
    }
    fn evaluate(&self, context: &EvaluationContext<'_>) -> Result<bool, SnapError> {
        self.rule
            .evaluate(&self.payload, context)
            .map_err(|e| SnapError::extension("snap.rule.evaluate", e))
    }
}

/// Owned evaluator declaration; neither an authorization nor a checked fact.
#[derive(Clone)]
pub struct RuleCall {
    pub(crate) instance: Instance,
    application: Rc<dyn ErasedRuleApplication>,
}
impl RuleCall {
    pub(crate) fn same_application(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.application, &other.application)
    }
    pub(crate) fn version(&self) -> Result<u64, SnapError> {
        self.application.version()
    }
    pub(crate) fn evaluate(&self, context: &EvaluationContext<'_>) -> Result<bool, SnapError> {
        self.application.evaluate(context)
    }
}
impl fmt::Debug for RuleCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RuleCall").field(&self.instance).finish()
    }
}

/// Public capture rule, registered and dispatched exactly like external evaluators.
pub struct ThresholdCapture;
impl EvaluationRule for ThresholdCapture {
    type Payload = ();
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("threshold-capture")
    }
    fn version(&self, _: &()) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(&self, _: &(), context: &EvaluationContext<'_>) -> Result<bool, SnapError> {
        Ok(context.capture_distance()? <= context.input.settings().radius())
    }
}

/// Public exact-projection admission using an ordinary external relation evaluator.
/// It imposes no radius limit; the supplied relation must verify the frozen original input.
pub struct ExactProjection<R: EvaluationRule> {
    relation: R,
}
impl<R: EvaluationRule> ExactProjection<R> {
    /// Installs a relation without evaluating it or granting any geometric exemption.
    pub const fn new(relation: R) -> Self {
        Self { relation }
    }
}
impl<R: EvaluationRule> EvaluationRule for ExactProjection<R> {
    type Payload = R::Payload;
    type Error = R::Error;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("exact-projection")
    }
    fn version(&self, payload: &Self::Payload) -> Result<u64, Self::Error> {
        self.relation.version(payload)
    }
    fn evaluate(
        &self,
        payload: &Self::Payload,
        context: &EvaluationContext<'_>,
    ) -> Result<bool, Self::Error> {
        self.relation.evaluate(payload, context)
    }
}

/// An open strategy protocol with owned configuration and private prepared state.
pub trait SnapStrategy: 'static {
    /// Owned configuration, shared by reference without cloning its values.
    type Payload: 'static;
    /// Owned prepared query data.
    type Prepared: 'static;
    /// Original domain failure.
    type Error: Error + 'static;
    /// Returns the typed protocol family.
    fn key() -> ProtocolKey<Self>
    where
        Self: Sized;
    /// Version of independently mutable captured dependencies.
    fn version(&self, payload: &Self::Payload) -> Result<u64, Self::Error>;
    /// Prepares from current complete input and checked provider features.
    fn prepare(
        &self,
        payload: &Self::Payload,
        input: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<Self::Prepared, Self::Error>;
    /// Declares whether the owned prepared snapshot covers every dependency of this input.
    /// The default conservatively rebuilds. Implementations must compare all input/features
    /// actually read by prepare; captured mutable dependencies also belong in `version`.
    /// This checked callback uses the same error and panic channel as prepare and query.
    fn reuse_prepared(
        &self,
        _payload: &Self::Payload,
        _prepared: &Self::Prepared,
        _input: &SnapInput,
        _features: &[SnapFeature],
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
    /// Contributes finite candidates through the shared bounded sink.
    fn query(
        &self,
        payload: &Self::Payload,
        prepared: &Self::Prepared,
        input: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), Self::Error>;
}

/// Typed instance identity of a configured strategy.
pub struct StrategyId<S: SnapStrategy> {
    pub(crate) instance: Instance,
    marker: PhantomData<fn() -> S>,
}
impl<S: SnapStrategy> Clone for StrategyId<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: SnapStrategy> Copy for StrategyId<S> {}

pub(crate) trait ErasedStrategy {
    fn version(&self) -> Result<u64, SnapError>;
    fn prepare(
        &self,
        input: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<Box<dyn Any>, SnapError>;
    fn reuse_prepared(
        &self,
        prepared: &dyn Any,
        input: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<bool, SnapError>;
    fn query(
        &self,
        prepared: &dyn Any,
        input: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError>;
}
struct StrategyApplication<S: SnapStrategy> {
    strategy: S,
    payload: S::Payload,
}
impl<S: SnapStrategy> ErasedStrategy for StrategyApplication<S> {
    fn version(&self) -> Result<u64, SnapError> {
        self.strategy
            .version(&self.payload)
            .map_err(|e| SnapError::extension("snap.strategy.version", e))
    }
    fn prepare(
        &self,
        input: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<Box<dyn Any>, SnapError> {
        self.strategy
            .prepare(&self.payload, input, features)
            .map(|p| Box::new(p) as Box<dyn Any>)
            .map_err(|e| SnapError::extension("snap.strategy.prepare", e))
    }
    fn reuse_prepared(
        &self,
        prepared: &dyn Any,
        input: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<bool, SnapError> {
        let prepared = prepared
            .downcast_ref::<S::Prepared>()
            .ok_or(SnapError::Identity)?;
        self.strategy
            .reuse_prepared(&self.payload, prepared, input, features)
            .map_err(|e| SnapError::extension("snap.strategy.reuse-prepared", e))
    }
    fn query(
        &self,
        prepared: &dyn Any,
        input: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError> {
        let prepared = prepared
            .downcast_ref::<S::Prepared>()
            .ok_or(SnapError::Identity)?;
        self.strategy
            .query(&self.payload, prepared, input, sink)
            .map_err(|e| SnapError::extension("snap.strategy.query", e))
    }
}
pub(crate) struct StrategyEntry {
    pub(crate) instance: Instance,
    pub(crate) admission: Instance,
    pub(crate) application: Box<dyn ErasedStrategy>,
}

/// Open provider of owned geometry with stable per-instance feature identities.
pub trait FeatureProvider: 'static {
    /// Owned provider configuration.
    type Payload: 'static;
    /// Original domain failure.
    type Error: Error + 'static;
    /// Returns the typed family key.
    fn key() -> ProtocolKey<Self>
    where
        Self: Sized;
    /// Version of independently mutable resource dependencies.
    fn version(&self, payload: &Self::Payload) -> Result<u64, Self::Error>;
    /// Contributes owned finite features. Empty success means legitimate nonparticipation.
    fn features(
        &self,
        payload: &Self::Payload,
        input: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), Self::Error>;
}

/// Typed identity of one configured geometric provider.
pub struct ProviderId<P: FeatureProvider> {
    instance: Instance,
    marker: PhantomData<fn() -> P>,
}
impl<P: FeatureProvider> Clone for ProviderId<P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<P: FeatureProvider> Copy for ProviderId<P> {}

pub(crate) trait ErasedProvider {
    fn version(&self) -> Result<u64, SnapError>;
    fn features(&self, input: &SnapInput, sink: &mut FeatureSink<'_>) -> Result<(), SnapError>;
}
struct ProviderApplication<P: FeatureProvider> {
    provider: P,
    payload: P::Payload,
}
impl<P: FeatureProvider> ErasedProvider for ProviderApplication<P> {
    fn version(&self) -> Result<u64, SnapError> {
        self.provider
            .version(&self.payload)
            .map_err(|e| SnapError::extension("snap.provider.version", e))
    }
    fn features(&self, input: &SnapInput, sink: &mut FeatureSink<'_>) -> Result<(), SnapError> {
        self.provider
            .features(&self.payload, input, sink)
            .map_err(|e| SnapError::extension("snap.provider.features", e))
    }
}
pub(crate) struct ProviderEntry {
    pub(crate) instance: Instance,
    pub(crate) application: Box<dyn ErasedProvider>,
}

/// Registry-issued identity of one provider feature.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FeatureId {
    pub(crate) provider: Instance,
    pub(crate) stable: u64,
}
/// Checked owned geometry contributed through an ordinary provider.
#[derive(Clone)]
pub struct SnapFeature {
    pub(crate) id: FeatureId,
    pub(crate) order: usize,
    pub(crate) points: Vec<Point>,
    payload: Option<Rc<dyn Any>>,
}
impl fmt::Debug for SnapFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnapFeature")
            .field("id", &self.id)
            .field("points", &self.points)
            .finish_non_exhaustive()
    }
}
impl SnapFeature {
    /// Returns the unforgeable provider and stable feature identity.
    pub const fn id(&self) -> FeatureId {
        self.id
    }
    /// Returns the provider's owned real geometry.
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    /// Reads an owned typed feature declaration without requiring Clone or thread bounds.
    pub fn payload<T: 'static>(&self, _key: ProtocolKey<T>) -> Option<&T> {
        self.payload.as_ref()?.downcast_ref()
    }
}
/// Bounded feature contribution sink; exceeding the bound poisons the complete operation.
pub struct FeatureSink<'a> {
    pub(crate) provider: Instance,
    pub(crate) order: usize,
    pub(crate) limit: usize,
    pub(crate) output: &'a mut Vec<SnapFeature>,
    pub(crate) identities: &'a mut HashSet<FeatureId>,
    pub(crate) failure: Option<SinkFailure>,
}
#[derive(Clone, Copy)]
pub(crate) enum SinkFailure {
    Budget(&'static str),
    Invalid(&'static str),
    Identity,
}
impl SinkFailure {
    pub(crate) fn capture(error: &SnapError) -> Self {
        match error {
            SnapError::Budget(s) => Self::Budget(s),
            SnapError::Identity => Self::Identity,
            SnapError::Invalid(s) => Self::Invalid(s),
            _ => Self::Invalid("rejected snapping contribution"),
        }
    }
    pub(crate) fn error(self) -> SnapError {
        match self {
            Self::Budget(s) => SnapError::Budget(s),
            Self::Invalid(s) => SnapError::Invalid(s),
            Self::Identity => SnapError::Identity,
        }
    }
}
impl FeatureSink<'_> {
    /// Validates and contributes one stable feature in this provider.
    pub fn push(&mut self, stable: u64, points: Vec<Point>) -> Result<(), SnapError> {
        self.contribute(stable, points, None)
    }
    /// Contributes typed feature data through the same finite geometry and budget checks.
    pub fn push_typed<T: 'static>(
        &mut self,
        stable: u64,
        points: Vec<Point>,
        _key: ProtocolKey<T>,
        payload: T,
    ) -> Result<(), SnapError> {
        self.contribute(stable, points, Some(Rc::new(payload)))
    }
    fn contribute(
        &mut self,
        stable: u64,
        points: Vec<Point>,
        payload: Option<Rc<dyn Any>>,
    ) -> Result<(), SnapError> {
        let result = self.push_inner(stable, points, payload);
        if let Err(error) = &result {
            self.failure.get_or_insert(SinkFailure::capture(error));
        }
        result
    }
    fn push_inner(
        &mut self,
        stable: u64,
        points: Vec<Point>,
        payload: Option<Rc<dyn Any>>,
    ) -> Result<(), SnapError> {
        if let Some(failure) = self.failure {
            return Err(failure.error());
        }
        if self.output.len() >= self.limit {
            return Err(SnapError::Budget("feature budget exceeded"));
        }
        if points.is_empty() {
            return Err(SnapError::Invalid("empty feature geometry"));
        }
        for &p in &points {
            super::geometry::finite_point(p)?;
        }
        let id = FeatureId {
            provider: self.provider,
            stable,
        };
        if !self.identities.insert(id) {
            return Err(SnapError::Invalid("duplicate provider feature identity"));
        }
        self.output.push(SnapFeature {
            id,
            order: self.order,
            points,
            payload,
        });
        Ok(())
    }
}

pub(crate) struct Registry {
    pub(crate) owner: EditorNamespace,
    pub(crate) rules: Vec<Instance>,
    pub(crate) strategies: Vec<StrategyEntry>,
    pub(crate) providers: Vec<ProviderEntry>,
    pub(crate) revision: Uuid,
    pub(crate) renderers: Vec<super::feedback::RendererEntry>,
}
impl Registry {
    pub(crate) fn new(owner: EditorNamespace) -> Self {
        Self {
            owner,
            rules: Vec::new(),
            strategies: Vec::new(),
            providers: Vec::new(),
            revision: Uuid::new_v4(),
            renderers: Vec::new(),
        }
    }
    pub(crate) fn check_rule(&self, call: &RuleCall) -> Result<(), SnapError> {
        if call.instance.owner != self.owner || !self.rules.contains(&call.instance) {
            Err(SnapError::Identity)
        } else {
            Ok(())
        }
    }
    pub(crate) fn changed(&mut self) {
        self.revision = Uuid::new_v4();
    }
}

/// Checked mutable registration access borrowed from a healthy Viewer.
/// Registration never calls prepare, query, rule evaluation or a business command.
pub struct SnapRegistration<'a> {
    pub(crate) registry: &'a mut Registry,
}
impl SnapRegistration<'_> {
    /// Registers an evaluator through the same open path used by built-in rules.
    pub fn register_rule<R: EvaluationRule>(
        &mut self,
        _key: ProtocolKey<R>,
        rule: R,
    ) -> Result<RuleBinding<R>, SnapError> {
        let instance = Instance::new(self.registry.owner);
        self.registry.rules.push(instance);
        self.registry.changed();
        Ok(RuleBinding {
            instance,
            rule: Rc::new(rule),
        })
    }
    /// Replaces a live rule, retiring all previous declarations.
    pub fn replace_rule<R: EvaluationRule>(
        &mut self,
        binding: &RuleBinding<R>,
        rule: R,
    ) -> Result<RuleBinding<R>, SnapError> {
        let index = self
            .registry
            .rules
            .iter()
            .position(|i| *i == binding.instance)
            .ok_or(SnapError::Identity)?;
        let instance = binding.instance.replaced();
        self.registry.rules[index] = instance;
        self.registry.changed();
        Ok(RuleBinding {
            instance,
            rule: Rc::new(rule),
        })
    }
    /// Removes a live evaluator. Strategies still bound to it will be rejected before callbacks.
    pub fn remove_rule<R: EvaluationRule>(
        &mut self,
        binding: &RuleBinding<R>,
    ) -> Result<(), SnapError> {
        let index = self
            .registry
            .rules
            .iter()
            .position(|i| *i == binding.instance)
            .ok_or(SnapError::Identity)?;
        self.registry.rules.remove(index);
        self.registry.changed();
        Ok(())
    }
    /// Registers one configured strategy and authorizes exactly one admission evaluator instance.
    pub fn register_strategy<S: SnapStrategy, R: EvaluationRule>(
        &mut self,
        _key: ProtocolKey<S>,
        strategy: S,
        payload: S::Payload,
        admission: &RuleBinding<R>,
    ) -> Result<StrategyId<S>, SnapError> {
        if !self.registry.rules.contains(&admission.instance) {
            return Err(SnapError::Identity);
        }
        let instance = Instance::new(self.registry.owner);
        self.registry.strategies.push(StrategyEntry {
            instance,
            admission: admission.instance,
            application: Box::new(StrategyApplication { strategy, payload }),
        });
        self.registry.changed();
        Ok(StrategyId {
            instance,
            marker: PhantomData,
        })
    }
    /// Replaces the strategy's configuration and implementation without calling either.
    pub fn replace_strategy<S: SnapStrategy, R: EvaluationRule>(
        &mut self,
        id: StrategyId<S>,
        strategy: S,
        payload: S::Payload,
        admission: &RuleBinding<R>,
    ) -> Result<StrategyId<S>, SnapError> {
        if !self.registry.rules.contains(&admission.instance) {
            return Err(SnapError::Identity);
        }
        let index = self
            .registry
            .strategies
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        let instance = id.instance.replaced();
        self.registry.strategies[index] = StrategyEntry {
            instance,
            admission: admission.instance,
            application: Box::new(StrategyApplication { strategy, payload }),
        };
        self.registry.changed();
        Ok(StrategyId {
            instance,
            marker: PhantomData,
        })
    }
    /// Removes one live strategy.
    pub fn remove_strategy<S: SnapStrategy>(&mut self, id: StrategyId<S>) -> Result<(), SnapError> {
        let index = self
            .registry
            .strategies
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        self.registry.strategies.remove(index);
        self.registry.changed();
        Ok(())
    }
    /// Moves a strategy to an explicit zero-based priority position.
    pub fn order_strategy<S: SnapStrategy>(
        &mut self,
        id: StrategyId<S>,
        position: usize,
    ) -> Result<(), SnapError> {
        if position >= self.registry.strategies.len() {
            return Err(SnapError::Invalid("invalid strategy order"));
        }
        let index = self
            .registry
            .strategies
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        let entry = self.registry.strategies.remove(index);
        self.registry.strategies.insert(position, entry);
        self.registry.changed();
        Ok(())
    }
    /// Registers an owned feature provider.
    pub fn register_provider<P: FeatureProvider>(
        &mut self,
        _key: ProtocolKey<P>,
        provider: P,
        payload: P::Payload,
    ) -> Result<ProviderId<P>, SnapError> {
        let instance = Instance::new(self.registry.owner);
        self.registry.providers.push(ProviderEntry {
            instance,
            application: Box::new(ProviderApplication { provider, payload }),
        });
        self.registry.changed();
        Ok(ProviderId {
            instance,
            marker: PhantomData,
        })
    }
    /// Replaces provider configuration and retires its old features.
    pub fn replace_provider<P: FeatureProvider>(
        &mut self,
        id: ProviderId<P>,
        provider: P,
        payload: P::Payload,
    ) -> Result<ProviderId<P>, SnapError> {
        let index = self
            .registry
            .providers
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        let instance = id.instance.replaced();
        self.registry.providers[index] = ProviderEntry {
            instance,
            application: Box::new(ProviderApplication { provider, payload }),
        };
        self.registry.changed();
        Ok(ProviderId {
            instance,
            marker: PhantomData,
        })
    }
    /// Removes one provider.
    pub fn remove_provider<P: FeatureProvider>(
        &mut self,
        id: ProviderId<P>,
    ) -> Result<(), SnapError> {
        let index = self
            .registry
            .providers
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        self.registry.providers.remove(index);
        self.registry.changed();
        Ok(())
    }
    /// Sets a provider's explicit contribution and stable tie-breaking order.
    pub fn order_provider<P: FeatureProvider>(
        &mut self,
        id: ProviderId<P>,
        position: usize,
    ) -> Result<(), SnapError> {
        if position >= self.registry.providers.len() {
            return Err(SnapError::Invalid("invalid provider order"));
        }
        let index = self
            .registry
            .providers
            .iter()
            .position(|e| e.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        let entry = self.registry.providers.remove(index);
        self.registry.providers.insert(position, entry);
        self.registry.changed();
        Ok(())
    }
}
