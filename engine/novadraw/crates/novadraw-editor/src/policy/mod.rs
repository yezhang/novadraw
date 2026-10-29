//! Typed editing policy registration and request dispatch.

use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    marker::PhantomData,
};

use novadraw::figure::{Figure, FigureStyle};
use novadraw::geometry::Point;

use crate::{
    BendpointRequest, ChangeBounds, ChangeBoundsRequest, Command, CreateChild, CreateConnection,
    CreateConnectionRequest, CreateRequest, DeleteParts, DeleteRequest, DirectTextEdit,
    DirectTextEditRequest, EditBendpoint, EditPartId, EditorNamespace, ModelAdapter,
    ReconnectConnection, ReconnectConnectionRequest, RequestKey, RequestMetadata, RequestProtocol,
    RequestResolution, StartDirectTextEdit, request::ErasedRequestKey,
};

/// Open marker implemented by every policy installation slot.
pub trait PolicySlot: 'static {
    /// Stable diagnostic name.
    const NAME: &'static str;

    /// Returns this slot's typed identity.
    fn key() -> PolicyKey<Self> {
        PolicyKey::new(Self::NAME)
    }
}

/// Typed identity of one policy installation slot.
pub struct PolicyKey<S: ?Sized> {
    name: &'static str,
    marker: PhantomData<fn() -> S>,
}

impl<S: ?Sized> PolicyKey<S> {
    /// Creates a key for one slot marker.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }

    /// Returns the diagnostic slot name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<S: ?Sized> Clone for PolicyKey<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized> Copy for PolicyKey<S> {}

impl<S: ?Sized> fmt::Debug for PolicyKey<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PolicyKey")
            .field(&self.name)
            .finish()
    }
}

macro_rules! policy_slots {
    ($(($slot:ident, $name:literal, $doc:literal)),+ $(,)?) => {
        $(
            #[doc = $doc]
            pub struct $slot;

            impl $slot {
                #[doc = "Returns this built-in slot's typed identity."]
                pub fn key() -> PolicyKey<Self> {
                    PolicyKey::new($name)
                }
            }

            impl PolicySlot for $slot {
                const NAME: &'static str = $name;
            }
        )+
    };
}

policy_slots!(
    (
        ComponentSlot,
        "component",
        "Fundamental model operations such as delete."
    ),
    (
        LayoutSlot,
        "layout",
        "Parent-owned child creation and bounds operations."
    ),
    (
        PrimaryDragSlot,
        "primary-drag",
        "Primary drag and selection-handle behavior."
    ),
    (
        SelectionFeedbackSlot,
        "selection-feedback",
        "Selection visual contribution."
    ),
    (
        ConnectionCreationSlot,
        "connection-creation",
        "Source-owned two-stage connection creation."
    ),
    (
        ConnectionReconnectSlot,
        "connection-reconnect",
        "Connection endpoint reconnection."
    ),
    (
        DirectTextEditSlot,
        "direct-text-edit",
        "Direct editing of an application text feature."
    ),
);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ErasedPolicyKey {
    name: &'static str,
    type_id: std::any::TypeId,
}

impl ErasedPolicyKey {
    fn of<S: PolicySlot>(key: PolicyKey<S>) -> Self {
        Self {
            name: key.name(),
            type_id: std::any::TypeId::of::<S>(),
        }
    }
}
/// Immutable host identity supplied to a policy.
#[derive(Clone, Copy, Debug)]
pub struct PolicyHost<I> {
    part: EditPartId,
    model: I,
    parent_model: Option<I>,
}

impl<I: Copy> PolicyHost<I> {
    pub(crate) fn new(part: EditPartId, model: I, parent_model: Option<I>) -> Self {
        Self {
            part,
            model,
            parent_model,
        }
    }

    /// Returns the host EditPart.
    pub const fn part(self) -> EditPartId {
        self.part
    }

    /// Returns the host model identity.
    pub const fn model(self) -> I {
        self.model
    }

    /// Returns the parent model identity.
    pub const fn parent_model(self) -> Option<I> {
        self.parent_model
    }
}

/// A policy-created transient Figure and its coordinate domain.
pub struct FeedbackVisual {
    figure: Box<dyn Figure>,
    scaled: bool,
    style: Option<FigureStyle>,
}

/// Source and target points resolved from endpoint Anchor providers for feedback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionFeedbackRoute {
    source: Point,
    target: Point,
}

impl ConnectionFeedbackRoute {
    pub(crate) const fn new(source: Point, target: Point) -> Self {
        Self { source, target }
    }

    /// Returns the feedback source endpoint in the Connection routing domain.
    pub const fn source(self) -> Point {
        self.source
    }

    /// Returns the feedback target endpoint in the Connection routing domain.
    pub const fn target(self) -> Point {
        self.target
    }
}

impl FeedbackVisual {
    /// Creates feedback in the model-scaled layer.
    pub fn scaled(figure: Box<dyn Figure>) -> Self {
        Self {
            figure,
            scaled: true,
            style: None,
        }
    }

    /// Creates feedback fixed in the surface domain.
    pub fn unscaled(figure: Box<dyn Figure>) -> Self {
        Self {
            figure,
            scaled: false,
            style: None,
        }
    }

    /// Supplies the local Figure style validated before feedback installation.
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub(crate) fn into_parts(self) -> (Box<dyn Figure>, bool, Option<FigureStyle>) {
        (self.figure, self.scaled, self.style)
    }
}

/// Failure returned by an EditPolicy extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyError {
    message: String,
}

impl PolicyError {
    /// Creates an explicit policy rejection or operation failure.
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the failure message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for PolicyError {}

/// Pluggable editing behavior installed in one typed policy slot.
pub trait EditPolicy<A: ModelAdapter>: 'static {
    /// Registers every typed request handler before the policy is installed.
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, A, Self>)
    where
        Self: Sized;

    /// Activates policy resources after its host is active.
    fn activate(&mut self, _host: PolicyHost<A::ModelId>, _model: &A) -> Result<(), PolicyError> {
        Ok(())
    }

    /// Releases policy resources before its host is retired.
    fn deactivate(&mut self, _host: PolicyHost<A::ModelId>, _model: &A) {}
}

/// Command and feedback emitted by one ordinary editing request handler.
pub struct EditingContribution<A: ModelAdapter> {
    command: Option<Box<dyn Command<A>>>,
    feedback: Vec<FeedbackVisual>,
}

impl<A: ModelAdapter> EditingContribution<A> {
    /// Creates a contribution with an optional command and transient feedback.
    pub fn new(command: Option<Box<dyn Command<A>>>, feedback: Vec<FeedbackVisual>) -> Self {
        Self { command, feedback }
    }

    /// Creates a command-only contribution.
    pub fn command(command: Box<dyn Command<A>>) -> Self {
        Self::new(Some(command), Vec::new())
    }

    /// Creates a feedback-only contribution.
    pub fn feedback(feedback: Vec<FeedbackVisual>) -> Self {
        Self::new(None, feedback)
    }

    pub(crate) fn into_parts(self) -> (Option<Box<dyn Command<A>>>, Vec<FeedbackVisual>) {
        (self.command, self.feedback)
    }
}

macro_rules! editing_request_protocol {
    ($marker:ty, $payload:ty, $name:literal) => {
        impl<A: ModelAdapter + 'static> RequestProtocol<A> for $marker {
            type Payload = $payload;
            type Contribution = EditingContribution<A>;

            fn key() -> RequestKey<Self> {
                RequestKey::new($name)
            }
        }
    };
}

editing_request_protocol!(ChangeBounds, ChangeBoundsRequest, "change-bounds");
editing_request_protocol!(CreateChild, CreateRequest, "create-child");
editing_request_protocol!(DeleteParts, DeleteRequest, "delete-parts");
editing_request_protocol!(EditBendpoint, BendpointRequest, "edit-bendpoint");

impl<A: ModelAdapter + 'static> RequestProtocol<A> for CreateConnection {
    type Payload = CreateConnectionRequest;
    type Contribution = Box<dyn ConnectionCreation<A>>;

    fn key() -> RequestKey<Self> {
        RequestKey::new("create-connection")
    }

    fn reduce(
        contributions: Vec<Self::Contribution>,
    ) -> Result<RequestResolution<Self::Contribution>, PolicyError> {
        RequestResolution::at_most_one(contributions, "create-connection")
    }
}

impl<A: ModelAdapter + 'static> RequestProtocol<A> for ReconnectConnection {
    type Payload = ReconnectConnectionRequest;
    type Contribution = Box<dyn ConnectionReconnection<A>>;

    fn key() -> RequestKey<Self> {
        RequestKey::new("reconnect-connection")
    }

    fn reduce(
        contributions: Vec<Self::Contribution>,
    ) -> Result<RequestResolution<Self::Contribution>, PolicyError> {
        RequestResolution::at_most_one(contributions, "reconnect-connection")
    }
}

impl<A: ModelAdapter + 'static> RequestProtocol<A> for StartDirectTextEdit {
    type Payload = DirectTextEditRequest;
    type Contribution = Box<dyn DirectTextEdit<A>>;

    fn key() -> RequestKey<Self> {
        RequestKey::new("start-direct-text-edit")
    }

    fn reduce(
        contributions: Vec<Self::Contribution>,
    ) -> Result<RequestResolution<Self::Contribution>, PolicyError> {
        RequestResolution::at_most_one(contributions, "start-direct-text-edit")
    }
}

/// Typed policy request callback.
pub type PolicyHandler<A, P, R> =
    fn(
        &mut P,
        PolicyHost<<A as ModelAdapter>::ModelId>,
        &RequestMetadata,
        &<R as RequestProtocol<A>>::Payload,
        &A,
    ) -> Result<Option<<R as RequestProtocol<A>>::Contribution>, PolicyError>;

/// Typed source-to-target routing callback.
pub type PolicyTarget<A, P, R> = fn(
    &mut P,
    PolicyHost<<A as ModelAdapter>::ModelId>,
    &RequestMetadata,
    &<R as RequestProtocol<A>>::Payload,
    &A,
) -> Result<Option<EditPartId>, PolicyError>;

trait ErasedPolicyHandler<A: ModelAdapter> {
    fn target(
        &self,
        policy: &mut dyn Any,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Option<EditPartId>, PolicyError>;

    fn has_target(&self) -> bool;

    fn contribute(
        &self,
        policy: &mut dyn Any,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Option<Box<dyn Any>>, PolicyError>;
}

struct TypedPolicyHandler<A: ModelAdapter, P, R: RequestProtocol<A>> {
    handler: PolicyHandler<A, P, R>,
    target: Option<PolicyTarget<A, P, R>>,
}

impl<A, P, R> ErasedPolicyHandler<A> for TypedPolicyHandler<A, P, R>
where
    A: ModelAdapter,
    P: 'static,
    R: RequestProtocol<A>,
{
    fn target(
        &self,
        policy: &mut dyn Any,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Option<EditPartId>, PolicyError> {
        let Some(target) = self.target else {
            return Ok(None);
        };
        let policy = policy
            .downcast_mut::<P>()
            .ok_or_else(|| PolicyError::operation("policy descriptor mismatch"))?;
        let payload = payload
            .downcast_ref::<R::Payload>()
            .ok_or_else(|| PolicyError::operation("request descriptor mismatch"))?;
        target(policy, host, metadata, payload, model)
    }

    fn has_target(&self) -> bool {
        self.target.is_some()
    }

    fn contribute(
        &self,
        policy: &mut dyn Any,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Option<Box<dyn Any>>, PolicyError> {
        let policy = policy
            .downcast_mut::<P>()
            .ok_or_else(|| PolicyError::operation("policy descriptor mismatch"))?;
        let payload = payload
            .downcast_ref::<R::Payload>()
            .ok_or_else(|| PolicyError::operation("request descriptor mismatch"))?;
        (self.handler)(policy, host, metadata, payload, model)
            .map(|contribution| contribution.map(|value| Box::new(value) as Box<dyn Any>))
    }
}

/// Install-time registry for one policy's typed request callbacks.
pub struct PolicyHandlerBuilder<'a, A: ModelAdapter, P> {
    handlers: BTreeMap<ErasedRequestKey, Box<dyn ErasedPolicyHandler<A>>>,
    duplicate: Option<&'static str>,
    marker: PhantomData<&'a mut P>,
}

impl<'a, A: ModelAdapter + 'static, P: 'static> PolicyHandlerBuilder<'a, A, P> {
    fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            duplicate: None,
            marker: PhantomData,
        }
    }

    /// Registers one typed request handler.
    pub fn handle<R>(&mut self, handler: PolicyHandler<A, P, R>)
    where
        R: RequestProtocol<A>,
    {
        self.insert::<R>(handler, None);
    }

    /// Registers one typed request handler with explicit source-to-target routing.
    pub fn handle_routed<R>(
        &mut self,
        target: PolicyTarget<A, P, R>,
        handler: PolicyHandler<A, P, R>,
    ) where
        R: RequestProtocol<A>,
    {
        self.insert::<R>(handler, Some(target));
    }

    fn insert<R>(&mut self, handler: PolicyHandler<A, P, R>, target: Option<PolicyTarget<A, P, R>>)
    where
        R: RequestProtocol<A>,
    {
        let key = ErasedRequestKey::of(R::key());
        if self
            .handlers
            .insert(
                key,
                Box::new(TypedPolicyHandler::<A, P, R> { handler, target }),
            )
            .is_some()
        {
            self.duplicate = Some(key.name());
        }
    }

    fn finish(
        self,
    ) -> Result<BTreeMap<ErasedRequestKey, Box<dyn ErasedPolicyHandler<A>>>, PolicyError> {
        if let Some(name) = self.duplicate {
            Err(PolicyError::operation(format!(
                "duplicate request handler: {name}"
            )))
        } else {
            Ok(self.handlers)
        }
    }
}

type PolicyActivate<A> =
    fn(&mut dyn Any, PolicyHost<<A as ModelAdapter>::ModelId>, &A) -> Result<(), PolicyError>;
type PolicyDeactivate<A> = fn(&mut dyn Any, PolicyHost<<A as ModelAdapter>::ModelId>, &A);

pub(crate) struct PolicyEntry<A: ModelAdapter> {
    policy: Box<dyn Any>,
    handlers: BTreeMap<ErasedRequestKey, Box<dyn ErasedPolicyHandler<A>>>,
    activate: PolicyActivate<A>,
    deactivate: PolicyDeactivate<A>,
    active: bool,
}

impl<A: ModelAdapter> PolicyEntry<A> {
    pub(crate) fn activate(
        &mut self,
        host: PolicyHost<A::ModelId>,
        model: &A,
    ) -> Result<(), PolicyError> {
        (self.activate)(self.policy.as_mut(), host, model)?;
        self.active = true;
        Ok(())
    }

    pub(crate) fn deactivate(&mut self, host: PolicyHost<A::ModelId>, model: &A) {
        if std::mem::replace(&mut self.active, false) {
            (self.deactivate)(self.policy.as_mut(), host, model);
        }
    }
}

/// One typed-slot policy installation declared by an EditPartBehavior.
pub struct PolicyInstallation<A: ModelAdapter> {
    key: ErasedPolicyKey,
    entry: PolicyEntry<A>,
}

impl<A: ModelAdapter + 'static> PolicyInstallation<A> {
    /// Builds and validates a policy's typed handler registry.
    pub fn new<S, P>(key: PolicyKey<S>, policy: P) -> Result<Self, PolicyError>
    where
        S: PolicySlot,
        P: EditPolicy<A>,
    {
        let mut builder = PolicyHandlerBuilder::<A, P>::new();
        P::register_handlers(&mut builder);
        Ok(Self {
            key: ErasedPolicyKey::of(key),
            entry: PolicyEntry {
                policy: Box::new(policy),
                handlers: builder.finish()?,
                active: false,
                activate: |policy, host, model| {
                    policy
                        .downcast_mut::<P>()
                        .ok_or_else(|| PolicyError::operation("policy descriptor mismatch"))?
                        .activate(host, model)
                },
                deactivate: |policy, host, model| {
                    if let Some(policy) = policy.downcast_mut::<P>() {
                        policy.deactivate(host, model);
                    }
                },
            },
        })
    }
}

/// Source-locked, gesture-scoped preparation for one connection Command.
///
/// A creation plan stores application model identity, never Viewer or Runtime identity. It is
/// discarded on cancel and enters the CommandStack only through the final Command it creates.
pub trait ConnectionCreation<A: ModelAdapter> {
    /// Returns whether the current target can complete this connection.
    fn can_complete(
        &self,
        source: PolicyHost<A::ModelId>,
        target: PolicyHost<A::ModelId>,
        request: &CreateConnectionRequest,
        model: &A,
    ) -> Result<bool, PolicyError>;

    /// Creates feedback for the current pointer and optional valid target.
    fn feedback(
        &mut self,
        _source: PolicyHost<A::ModelId>,
        _target: Option<PolicyHost<A::ModelId>>,
        _request: &CreateConnectionRequest,
        _model: &A,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        Ok(Vec::new())
    }

    /// Creates feedback using framework-resolved Anchor endpoints when available.
    fn feedback_with_route(
        &mut self,
        source: PolicyHost<A::ModelId>,
        target: Option<PolicyHost<A::ModelId>>,
        request: &CreateConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        model: &A,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let _ = route;
        self.feedback(source, target, request, model)
    }

    /// Builds the final model-only Command for a valid target.
    fn command(
        &mut self,
        source: PolicyHost<A::ModelId>,
        target: PolicyHost<A::ModelId>,
        request: &CreateConnectionRequest,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}

/// Connection-owned, gesture-scoped preparation for one endpoint reconnect.
pub trait ConnectionReconnection<A: ModelAdapter> {
    /// Returns whether the candidate can replace the moving endpoint.
    fn can_complete(
        &self,
        connection: PolicyHost<A::ModelId>,
        fixed: PolicyHost<A::ModelId>,
        candidate: PolicyHost<A::ModelId>,
        request: &ReconnectConnectionRequest,
        model: &A,
    ) -> Result<bool, PolicyError>;

    /// Creates feedback for the fixed endpoint and current candidate or pointer.
    fn feedback(
        &mut self,
        _connection: PolicyHost<A::ModelId>,
        _fixed: PolicyHost<A::ModelId>,
        _candidate: Option<PolicyHost<A::ModelId>>,
        _request: &ReconnectConnectionRequest,
        _model: &A,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        Ok(Vec::new())
    }

    /// Creates feedback using framework-resolved Anchor endpoints when available.
    fn feedback_with_route(
        &mut self,
        connection: PolicyHost<A::ModelId>,
        fixed: PolicyHost<A::ModelId>,
        candidate: Option<PolicyHost<A::ModelId>>,
        request: &ReconnectConnectionRequest,
        route: Option<ConnectionFeedbackRoute>,
        model: &A,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        let _ = route;
        self.feedback(connection, fixed, candidate, request, model)
    }

    /// Builds the final model-only reconnect Command.
    fn command(
        &mut self,
        connection: PolicyHost<A::ModelId>,
        fixed: PolicyHost<A::ModelId>,
        candidate: PolicyHost<A::ModelId>,
        request: &ReconnectConnectionRequest,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}

pub(crate) struct PolicyStore<A: ModelAdapter> {
    entries: HashMap<EditPartId, BTreeMap<ErasedPolicyKey, PolicyEntry<A>>>,
    namespace: EditorNamespace,
    generations: HashMap<EditPartId, crate::container::PolicyGeneration>,
}

impl<A: ModelAdapter> PolicyStore<A> {
    pub(crate) fn new(namespace: EditorNamespace) -> Self {
        Self {
            entries: HashMap::new(),
            namespace,
            generations: HashMap::new(),
        }
    }

    pub(crate) fn generation(&self, part: EditPartId) -> crate::container::PolicyGeneration {
        self.generations
            .get(&part)
            .copied()
            .unwrap_or(crate::container::PolicyGeneration(uuid::Uuid::nil()))
    }

    pub(crate) fn invalidate(&mut self, part: EditPartId) {
        self.generations.insert(
            part,
            crate::container::PolicyGeneration(uuid::Uuid::new_v4()),
        );
    }

    pub(crate) fn install_batch(
        &mut self,
        part: EditPartId,
        installations: Vec<PolicyInstallation<A>>,
    ) -> Result<(), PolicyError> {
        if part.namespace() != self.namespace {
            return Err(PolicyError::operation(
                "policy host belongs to another Viewer",
            ));
        }
        let existing = self.entries.get(&part);
        let mut incoming = std::collections::HashSet::with_capacity(installations.len());
        for installation in &installations {
            if existing.is_some_and(|entries| entries.contains_key(&installation.key))
                || !incoming.insert(installation.key)
            {
                return Err(PolicyError::operation(format!(
                    "duplicate policy slot: {}",
                    installation.key.name
                )));
            }
        }
        let entries = self.entries.entry(part).or_default();
        for installation in installations {
            entries.insert(installation.key, installation.entry);
        }
        self.invalidate(part);
        Ok(())
    }

    pub(crate) fn entries_mut(
        &mut self,
        part: EditPartId,
    ) -> Option<&mut BTreeMap<ErasedPolicyKey, PolicyEntry<A>>> {
        if part.namespace() != self.namespace {
            return None;
        }
        self.entries.get_mut(&part)
    }

    pub(crate) fn remove(
        &mut self,
        part: EditPartId,
    ) -> Option<BTreeMap<ErasedPolicyKey, PolicyEntry<A>>> {
        if part.namespace() != self.namespace {
            return None;
        }
        let removed = self.entries.remove(&part);
        self.generations.remove(&part);
        removed
    }

    pub(crate) fn resolve_target<R>(
        &mut self,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &R::Payload,
        model: &A,
    ) -> Result<Option<EditPartId>, PolicyError>
    where
        R: RequestProtocol<A>,
    {
        self.resolve_target_erased(
            host,
            ErasedRequestKey::of(R::key()),
            metadata,
            payload,
            model,
        )
    }

    pub(crate) fn resolve_target_erased(
        &mut self,
        host: PolicyHost<A::ModelId>,
        key: ErasedRequestKey,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Option<EditPartId>, PolicyError> {
        let Some(entries) = self.entries_mut(host.part()) else {
            return Ok(None);
        };
        let mut registered = false;
        for entry in entries.values_mut() {
            let Some(handler) = entry.handlers.get(&key) else {
                continue;
            };
            registered = true;
            if handler.has_target()
                && let Some(target) =
                    handler.target(entry.policy.as_mut(), host, metadata, payload, model)?
            {
                return Ok(Some(target));
            }
        }
        Ok(registered.then_some(host.part()))
    }

    pub(crate) fn contributions<R>(
        &mut self,
        host: PolicyHost<A::ModelId>,
        metadata: &RequestMetadata,
        payload: &R::Payload,
        model: &A,
    ) -> Result<Vec<R::Contribution>, PolicyError>
    where
        R: RequestProtocol<A>,
    {
        self.contributions_erased(
            host,
            ErasedRequestKey::of(R::key()),
            metadata,
            payload,
            model,
        )?
        .into_iter()
        .map(|contribution| {
            contribution
                .downcast::<R::Contribution>()
                .map(|contribution| *contribution)
                .map_err(|_| PolicyError::operation("request contribution descriptor mismatch"))
        })
        .collect()
    }

    pub(crate) fn contributions_erased(
        &mut self,
        host: PolicyHost<A::ModelId>,
        key: ErasedRequestKey,
        metadata: &RequestMetadata,
        payload: &dyn Any,
        model: &A,
    ) -> Result<Vec<Box<dyn Any>>, PolicyError> {
        let Some(entries) = self.entries_mut(host.part()) else {
            return Ok(Vec::new());
        };
        let mut contributions = Vec::new();
        for entry in entries.values_mut() {
            let Some(handler) = entry.handlers.get(&key) else {
                continue;
            };
            let Some(contribution) =
                handler.contribute(entry.policy.as_mut(), host, metadata, payload, model)?
            else {
                continue;
            };
            contributions.push(contribution);
        }
        Ok(contributions)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::{InteractionRevision, ModelEvent, ModelRevision};

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct ModelId;

    struct Model;

    impl ModelAdapter for Model {
        type ModelId = ModelId;
        type Event = ();
        type Error = Infallible;

        fn root(&self) -> Self::ModelId {
            ModelId
        }

        fn revision(&self) -> ModelRevision {
            ModelRevision::initial()
        }

        fn children(&self, _model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
            Ok(Vec::new())
        }

        fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
            Vec::new()
        }
    }

    struct TestRequest;

    impl RequestProtocol<Model> for TestRequest {
        type Payload = u32;
        type Contribution = u32;

        fn key() -> RequestKey<Self> {
            RequestKey::new("test-request")
        }
    }

    struct TestPolicy;

    impl EditPolicy<Model> for TestPolicy {
        fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
            builder.handle::<TestRequest>(Self::handle);
        }
    }

    impl TestPolicy {
        fn handle(
            &mut self,
            _host: PolicyHost<ModelId>,
            _metadata: &RequestMetadata,
            payload: &u32,
            _model: &Model,
        ) -> Result<Option<u32>, PolicyError> {
            Ok(Some(*payload))
        }
    }

    #[test]
    fn erased_handler_rejects_a_mismatched_payload() {
        let handler: Box<dyn ErasedPolicyHandler<Model>> =
            Box::new(TypedPolicyHandler::<Model, TestPolicy, TestRequest> {
                handler: TestPolicy::handle,
                target: None,
            });
        let namespace = EditorNamespace::new();
        let part = EditPartId::for_test(namespace);
        let metadata = RequestMetadata::new(InteractionRevision::initial(), vec![part]);
        let mut policy: Box<dyn Any> = Box::new(TestPolicy);

        let error = handler
            .contribute(
                policy.as_mut(),
                PolicyHost::new(part, ModelId, None),
                &metadata,
                &"wrong payload",
                &Model,
            )
            .expect_err("descriptor mismatch must be rejected");

        assert_eq!(error.message(), "request descriptor mismatch");
    }

    #[test]
    fn duplicate_request_handler_is_rejected_before_installation() {
        let mut builder = PolicyHandlerBuilder::<Model, TestPolicy>::new();
        builder.handle::<TestRequest>(TestPolicy::handle);
        builder.handle::<TestRequest>(TestPolicy::handle);

        let Err(error) = builder.finish() else {
            panic!("duplicate handler must be rejected");
        };
        assert_eq!(error.message(), "duplicate request handler: test-request");
    }
}
