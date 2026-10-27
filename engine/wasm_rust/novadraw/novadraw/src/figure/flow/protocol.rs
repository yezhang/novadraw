use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    sync::Arc,
};

use crate::figure::{CapabilityKey, PreparationScene, ResolvedStyle};
use crate::text::TextStyle;
use crate::{Figure, FigureId};

use super::{FlowAssembly, FlowError};

/// Typed behavior identity. The payload type owns identity; the name is diagnostic.
pub struct ContributionKey<P> {
    name: &'static str,
    marker: PhantomData<fn() -> P>,
}
impl<P> ContributionKey<P> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
    pub const fn name(self) -> &'static str {
        self.name
    }
}
impl<P> Copy for ContributionKey<P> {}
impl<P> Clone for ContributionKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Owned, immutable contribution admitted through the same registry for all nodes.
pub trait FlowContributionPayload: Any + Send + Sync {}

pub struct FlowContributionSink {
    entries: Vec<(TypeId, &'static str, Box<dyn Any + Send + Sync>)>,
}
impl FlowContributionSink {
    pub fn append<P: FlowContributionPayload>(&mut self, key: ContributionKey<P>, payload: P) {
        self.entries
            .push((TypeId::of::<P>(), key.name, Box::new(payload)));
    }
}

/// A contributor observes only its frozen source identity, inherited style and measurements.
pub struct FlowContributionContext<'a> {
    pub source: FigureId,
    pub scene: &'a PreparationScene<'a>,
}
impl FlowContributionContext<'_> {
    pub fn style(&self) -> Result<&ResolvedStyle, FlowError> {
        Ok(self.scene.style(self.source)?)
    }
    pub fn text_style(&self) -> Result<TextStyle, FlowError> {
        let style = self.style()?;
        Ok(TextStyle::new(
            self.scene.parsed_font(self.source)?,
            style.foreground,
        ))
    }
}

pub trait FlowContributor {
    fn contribute(
        &self,
        context: &FlowContributionContext<'_>,
        output: &mut FlowContributionSink,
    ) -> Result<(), FlowError>;
}

#[derive(Clone, Copy)]
pub struct FlowCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FlowContributor>,
}
impl FlowCapability {
    pub fn of<T: Figure + FlowContributor + 'static>() -> Self {
        Self {
            resolve: |figure| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(|figure| figure as &dyn FlowContributor)
            },
        }
    }
    pub fn resolve(self, figure: &dyn Figure) -> Option<&dyn FlowContributor> {
        (self.resolve)(figure)
    }
}
pub const FLOW: CapabilityKey<FlowCapability> = CapabilityKey::new("novadraw.flow");

pub trait FlowContributionHandler<P: FlowContributionPayload>: Send + Sync {
    fn process(
        &self,
        payload: &P,
        context: &FlowContributionContext<'_>,
        output: &mut FlowAssembly<'_>,
    ) -> Result<(), FlowError>;
}
impl<P, F> FlowContributionHandler<P> for F
where
    P: FlowContributionPayload,
    F: Fn(&P, &FlowContributionContext<'_>, &mut FlowAssembly<'_>) -> Result<(), FlowError>
        + Send
        + Sync,
{
    fn process(
        &self,
        payload: &P,
        context: &FlowContributionContext<'_>,
        output: &mut FlowAssembly<'_>,
    ) -> Result<(), FlowError> {
        self(payload, context, output)
    }
}

type Handler = dyn Fn(
        &(dyn Any + Send + Sync),
        &FlowContributionContext<'_>,
        &mut FlowAssembly<'_>,
    ) -> Result<(), FlowError>
    + Send
    + Sync;

/// Instance-owned registry. Standard handlers are registered through this public API.
#[derive(Clone, Default)]
pub struct FlowContributionRegistry {
    handlers: HashMap<TypeId, Arc<Handler>>,
}
impl FlowContributionRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<P: FlowContributionPayload>(
        &mut self,
        key: ContributionKey<P>,
        handler: impl FlowContributionHandler<P> + 'static,
    ) -> Result<(), FlowError> {
        if self.handlers.contains_key(&TypeId::of::<P>()) {
            return Err(FlowError::DuplicateContribution(key.name));
        }
        self.handlers.insert(
            TypeId::of::<P>(),
            Arc::new(move |payload, context, output| {
                handler.process(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(FlowError::InvalidContribution(key.name))?,
                    context,
                    output,
                )
            }),
        );
        Ok(())
    }

    pub(crate) fn dispatch(
        &self,
        source: FigureId,
        scene: &PreparationScene<'_>,
        output: &mut FlowAssembly<'_>,
    ) -> Result<(), FlowError> {
        let capability = scene
            .capability(source, FLOW)?
            .ok_or(FlowError::MissingContributor(source))?;
        let contributor = capability
            .resolve(scene.figure(source)?)
            .ok_or(FlowError::MissingContributor(source))?;
        let context = FlowContributionContext { source, scene };
        let mut sink = FlowContributionSink {
            entries: Vec::new(),
        };
        contributor.contribute(&context, &mut sink)?;
        for (id, name, payload) in sink.entries {
            let handler = self
                .handlers
                .get(&id)
                .ok_or(FlowError::UnknownContribution(name))?;
            handler(payload.as_ref(), &context, output)?;
        }
        Ok(())
    }
}
