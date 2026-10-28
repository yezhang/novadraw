use std::collections::HashMap;

use crate::runtime::update::{
    DiscretePropertyValue, ErasedPropertyKey, PropertyChangeEvent, PropertyKey,
    property::standard as property,
};
use crate::{
    AncestorEventKind, FigureEvent, FigureId, NotificationEffect, PropertyValue, Rectangle,
    StableSceneQuery,
};

use super::{AnimationError, AnimationPlan};

/// Stable committed fact that can activate an installed animation behavior.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AnimationFact {
    /// A Figure's committed bounds changed.
    FigureBoundsChanged {
        /// Changed Figure.
        figure: FigureId,
        /// Bounds before the first change in this stable transaction.
        old_bounds: Rectangle,
        /// Bounds after the last change in this stable transaction.
        new_bounds: Rectangle,
    },
    /// A typed committed property changed.
    PropertyChanged(PropertyChangeEvent),
    /// A Figure was attached to or detached from a parent.
    Lifecycle {
        /// Changed Figure.
        figure: FigureId,
        /// Parent at the lifecycle boundary.
        parent: FigureId,
        /// Lifecycle operation.
        kind: AnimationLifecycle,
    },
}

impl AnimationFact {
    pub(crate) fn figure(&self) -> FigureId {
        match self {
            Self::FigureBoundsChanged { figure, .. } | Self::Lifecycle { figure, .. } => *figure,
            Self::PropertyChanged(event) => event.figure_id(),
        }
    }
}

/// Figure lifecycle operation visible to an animation behavior.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum AnimationLifecycle {
    /// A Figure was attached to a parent.
    Attached,
    /// A Figure was detached from a parent.
    Detached,
    /// A Figure became visible.
    Shown,
    /// A Figure became hidden.
    Hidden,
}

/// Committed-fact selector for one animation behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationTrigger {
    /// Match committed Figure bounds changes.
    FigureBoundsChanged,
    /// Match one typed property regardless of value kind.
    PropertyChanged(PropertySelector),
    /// Match one typed discrete-state property.
    StateChanged(StatePropertySelector),
    /// Match every committed fact in one stable transaction.
    Transaction,
    /// Match one lifecycle operation.
    Lifecycle(AnimationLifecycle),
}

/// Type-erased selector created from a typed property key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertySelector(ErasedPropertyKey);

impl PropertySelector {
    /// Returns the selected property identity.
    pub const fn property(self) -> ErasedPropertyKey {
        self.0
    }
}

/// Type-erased selector restricted to discrete property value types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatePropertySelector(ErasedPropertyKey);

impl StatePropertySelector {
    /// Returns the selected property identity.
    pub const fn property(self) -> ErasedPropertyKey {
        self.0
    }
}

impl AnimationTrigger {
    /// Matches changes emitted with the supplied typed property key.
    pub fn property_changed<V: 'static>(property: PropertyKey<V>) -> Self {
        Self::PropertyChanged(PropertySelector(property.erase()))
    }

    /// Matches changes to a typed discrete-state property.
    pub fn state_changed<V>(property: PropertyKey<V>) -> Self
    where
        V: DiscretePropertyValue,
    {
        Self::StateChanged(StatePropertySelector(property.erase()))
    }

    pub(crate) fn matches(self, fact: &AnimationFact) -> bool {
        match (self, fact) {
            (Self::FigureBoundsChanged, AnimationFact::FigureBoundsChanged { .. }) => true,
            (Self::PropertyChanged(expected), AnimationFact::PropertyChanged(event)) => {
                expected.property() == event.property()
            }
            (Self::StateChanged(expected), AnimationFact::PropertyChanged(event)) => {
                expected.property() == event.property()
            }
            (Self::Transaction, _) => true,
            (
                Self::Lifecycle(AnimationLifecycle::Attached),
                AnimationFact::Lifecycle {
                    kind: AnimationLifecycle::Attached,
                    ..
                },
            )
            | (
                Self::Lifecycle(AnimationLifecycle::Detached),
                AnimationFact::Lifecycle {
                    kind: AnimationLifecycle::Detached,
                    ..
                },
            ) => true,
            (
                Self::Lifecycle(
                    expected @ (AnimationLifecycle::Shown | AnimationLifecycle::Hidden),
                ),
                AnimationFact::PropertyChanged(event),
            ) if event.property().is(property::VISIBLE)
                && matches!(event.new_value(), PropertyValue::Bool(_)) =>
            {
                let PropertyValue::Bool(visible) = event.new_value() else {
                    unreachable!("guard checked bool property value");
                };
                (*visible && expected == AnimationLifecycle::Shown)
                    || (!*visible && expected == AnimationLifecycle::Hidden)
            }
            _ => false,
        }
    }
}

/// Scope used to filter committed facts before invoking a behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationBehaviorScope {
    /// Match facts from every Figure in the Runtime.
    Runtime,
    /// Match facts from one Figure only.
    Figure(FigureId),
}

impl AnimationBehaviorScope {
    pub(crate) fn matches(self, fact: &AnimationFact) -> bool {
        match self {
            Self::Runtime => true,
            Self::Figure(figure) => fact.figure() == figure,
        }
    }
}

/// Fact retention policy within one stable Runtime transaction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum AnimationFactCoalescing {
    /// Keep the first old value and last new value for each Figure/property key.
    #[default]
    LatestPerSubject,
    /// Preserve every matching fact in source order.
    PreserveAll,
}

/// Read-only input passed to an animation plan factory.
#[derive(Clone, Copy)]
pub struct AnimationBehaviorContext<'a> {
    epoch: u64,
    facts: &'a [AnimationFact],
    scene: StableSceneQuery<'a>,
}

impl<'a> AnimationBehaviorContext<'a> {
    pub(crate) fn new(epoch: u64, facts: &'a [AnimationFact], scene: StableSceneQuery<'a>) -> Self {
        Self {
            epoch,
            facts,
            scene,
        }
    }

    /// Returns the stable source epoch that produced these facts.
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Returns matching, coalesced committed facts.
    pub const fn facts(self) -> &'a [AnimationFact] {
        self.facts
    }

    /// Returns a read-only query over the same stable committed scene.
    pub const fn scene(self) -> StableSceneQuery<'a> {
        self.scene
    }
}

/// Produces an optional animation plan from stable committed facts.
pub trait AnimationPlanFactory: Send {
    /// Builds one plan without mutating Runtime source state.
    fn create_plan(
        &mut self,
        context: AnimationBehaviorContext<'_>,
    ) -> Result<Option<AnimationPlan>, AnimationError>;
}

impl<F> AnimationPlanFactory for F
where
    F: for<'a> FnMut(AnimationBehaviorContext<'a>) -> Result<Option<AnimationPlan>, AnimationError>
        + Send,
{
    fn create_plan(
        &mut self,
        context: AnimationBehaviorContext<'_>,
    ) -> Result<Option<AnimationPlan>, AnimationError> {
        self(context)
    }
}

/// Optional policy that turns stable committed facts into animation plans.
pub struct AnimationBehavior {
    pub(crate) trigger: AnimationTrigger,
    pub(crate) scope: AnimationBehaviorScope,
    pub(crate) coalescing: AnimationFactCoalescing,
    pub(crate) factory: Box<dyn AnimationPlanFactory>,
    pub(crate) reduced_motion_factory: Option<Box<dyn AnimationPlanFactory>>,
}

impl AnimationBehavior {
    /// Creates a Runtime-scoped behavior with latest-per-subject coalescing.
    pub fn new(trigger: AnimationTrigger, factory: impl AnimationPlanFactory + 'static) -> Self {
        Self {
            trigger,
            scope: AnimationBehaviorScope::Runtime,
            coalescing: AnimationFactCoalescing::LatestPerSubject,
            factory: Box::new(factory),
            reduced_motion_factory: None,
        }
    }

    /// Restricts the behavior to one Figure.
    pub fn scoped_to(mut self, figure: FigureId) -> Self {
        self.scope = AnimationBehaviorScope::Figure(figure);
        self
    }

    /// Replaces the stable-transaction fact coalescing policy.
    pub fn with_coalescing(mut self, coalescing: AnimationFactCoalescing) -> Self {
        self.coalescing = coalescing;
        self
    }

    /// Installs a capability-specific plan factory for reduced-motion mode.
    pub fn with_reduced_motion(mut self, factory: impl AnimationPlanFactory + 'static) -> Self {
        self.reduced_motion_factory = Some(Box::new(factory));
        self
    }

    pub(crate) fn matching_facts(&self, facts: &[AnimationFact]) -> Vec<AnimationFact> {
        let matching: Vec<_> = facts
            .iter()
            .filter(|fact| self.scope.matches(fact) && self.trigger.matches(fact))
            .cloned()
            .collect();
        match self.coalescing {
            AnimationFactCoalescing::LatestPerSubject => coalesce_facts(matching),
            AnimationFactCoalescing::PreserveAll => matching,
        }
    }
}

/// One bounded, recoverable behavior factory or plan-admission failure.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationBehaviorFailure {
    behavior: crate::identity::AnimationBehaviorId,
    epoch: u64,
    error: AnimationError,
}

impl AnimationBehaviorFailure {
    pub(crate) fn new(
        behavior: crate::identity::AnimationBehaviorId,
        epoch: u64,
        error: AnimationError,
    ) -> Self {
        Self {
            behavior,
            epoch,
            error,
        }
    }

    /// Returns the behavior that failed.
    pub const fn behavior(&self) -> crate::identity::AnimationBehaviorId {
        self.behavior
    }

    /// Returns the stable source epoch that was being processed.
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns the structured factory or admission error.
    pub const fn error(&self) -> &AnimationError {
        &self.error
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FactKey {
    Bounds(FigureId),
    Property(FigureId, ErasedPropertyKey),
}

fn coalesce_facts(facts: Vec<AnimationFact>) -> Vec<AnimationFact> {
    let mut result = Vec::with_capacity(facts.len());
    let mut positions = HashMap::new();
    for fact in facts {
        let key = match &fact {
            AnimationFact::FigureBoundsChanged { figure, .. } => Some(FactKey::Bounds(*figure)),
            AnimationFact::PropertyChanged(event) => {
                Some(FactKey::Property(event.figure_id(), event.property()))
            }
            AnimationFact::Lifecycle { .. } => None,
        };
        let Some(key) = key else {
            result.push(fact);
            continue;
        };
        if let Some(index) = positions.get(&key).copied() {
            merge_fact(&mut result[index], fact);
        } else {
            positions.insert(key, result.len());
            result.push(fact);
        }
    }
    result.retain(|fact| match fact {
        AnimationFact::FigureBoundsChanged {
            old_bounds,
            new_bounds,
            ..
        } => old_bounds != new_bounds,
        AnimationFact::PropertyChanged(event) => !event.is_noop(),
        AnimationFact::Lifecycle { .. } => true,
    });
    result
}

fn merge_fact(existing: &mut AnimationFact, latest: AnimationFact) {
    match (existing, latest) {
        (
            AnimationFact::FigureBoundsChanged { new_bounds, .. },
            AnimationFact::FigureBoundsChanged {
                new_bounds: latest, ..
            },
        ) => *new_bounds = latest,
        (AnimationFact::PropertyChanged(existing), AnimationFact::PropertyChanged(latest)) => {
            existing.merge_latest(latest);
        }
        _ => unreachable!("facts with different keys are never merged"),
    }
}

pub(crate) fn facts_from_effects(effects: &[NotificationEffect]) -> Vec<AnimationFact> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            NotificationEffect::EmitFigure(FigureEvent::FigureMoved {
                figure_id,
                old_bounds,
                new_bounds,
            }) => Some(AnimationFact::FigureBoundsChanged {
                figure: *figure_id,
                old_bounds: *old_bounds,
                new_bounds: *new_bounds,
            }),
            NotificationEffect::EmitProperty(event) => {
                Some(AnimationFact::PropertyChanged(event.clone()))
            }
            NotificationEffect::EmitAncestor(event) => match event.kind {
                AncestorEventKind::Added => Some(AnimationFact::Lifecycle {
                    figure: event.figure_id,
                    parent: event.parent_id,
                    kind: AnimationLifecycle::Attached,
                }),
                AncestorEventKind::Removed => Some(AnimationFact::Lifecycle {
                    figure: event.figure_id,
                    parent: event.parent_id,
                    kind: AnimationLifecycle::Detached,
                }),
                AncestorEventKind::Moved => None,
            },
            _ => None,
        })
        .collect()
}
