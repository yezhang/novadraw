//! Role-keyed editing policies.

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
};

use novadraw_scene::Figure;

use crate::{
    Command, CreateConnectionRequest, EditPartId, EditorNamespace, EditorRequest, ModelAdapter,
};

/// Stable role used to install an EditPolicy on one EditPart.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PolicyRole {
    /// Fundamental model operations such as delete.
    Component,
    /// Parent-owned child creation and bounds operations.
    Layout,
    /// Primary drag and selection-handle behavior.
    PrimaryDrag,
    /// Selection visual contribution.
    SelectionFeedback,
    /// Source-owned two-stage connection creation.
    ConnectionCreation,
    /// Application-defined role.
    Custom(Arc<str>),
}

impl PolicyRole {
    /// Creates a non-empty application role.
    pub fn custom(value: impl AsRef<str>) -> Result<Self, PolicyError> {
        let value = value.as_ref();
        if value.is_empty() {
            Err(PolicyError::operation("policy role must not be empty"))
        } else {
            Ok(Self::Custom(Arc::from(value)))
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
}

impl FeedbackVisual {
    /// Creates feedback in the model-scaled layer.
    pub fn scaled(figure: Box<dyn Figure>) -> Self {
        Self {
            figure,
            scaled: true,
        }
    }

    /// Creates feedback fixed in the surface domain.
    pub fn unscaled(figure: Box<dyn Figure>) -> Self {
        Self {
            figure,
            scaled: false,
        }
    }

    pub(crate) fn into_parts(self) -> (Box<dyn Figure>, bool) {
        (self.figure, self.scaled)
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

/// Pluggable editing behavior installed on one EditPart role.
pub trait EditPolicy<A: ModelAdapter> {
    /// Returns whether this policy contributes to the request.
    fn understands(&self, request: &EditorRequest) -> bool;

    /// Returns the target part or `None` when this policy is not a target.
    fn target(&self, host: PolicyHost<A::ModelId>, request: &EditorRequest) -> Option<EditPartId> {
        self.understands(request).then_some(host.part())
    }

    /// Returns no contribution, an explicit rejection, or a model Command.
    fn command(
        &mut self,
        host: PolicyHost<A::ModelId>,
        request: &EditorRequest,
        model: &A,
    ) -> Result<Option<Box<dyn Command<A>>>, PolicyError>;

    /// Creates source or target feedback for the latest request state.
    fn feedback(
        &mut self,
        _host: PolicyHost<A::ModelId>,
        _request: &EditorRequest,
        _model: &A,
    ) -> Result<Vec<FeedbackVisual>, PolicyError> {
        Ok(Vec::new())
    }

    /// Creates a source-locked connection plan when this policy accepts the first stage.
    fn start_connection(
        &mut self,
        _host: PolicyHost<A::ModelId>,
        _request: &CreateConnectionRequest,
        _model: &A,
    ) -> Result<Option<Box<dyn ConnectionCreation<A>>>, PolicyError> {
        Ok(None)
    }

    /// Activates policy resources after its host is active.
    fn activate(&mut self, _host: PolicyHost<A::ModelId>, _model: &A) -> Result<(), PolicyError> {
        Ok(())
    }

    /// Releases policy resources before its host is retired.
    fn deactivate(&mut self, _host: PolicyHost<A::ModelId>, _model: &A) {}
}

/// One role-keyed policy declared by an EditPartBehavior.
pub type PolicyInstallation<A> = (PolicyRole, Box<dyn EditPolicy<A>>);

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

    /// Builds the final model-only Command for a valid target.
    fn command(
        &mut self,
        source: PolicyHost<A::ModelId>,
        target: PolicyHost<A::ModelId>,
        request: &CreateConnectionRequest,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}

pub(crate) struct PolicyStore<A: ModelAdapter> {
    entries: HashMap<EditPartId, BTreeMap<PolicyRole, Box<dyn EditPolicy<A>>>>,
    namespace: EditorNamespace,
}

impl<A: ModelAdapter> PolicyStore<A> {
    pub(crate) fn new(namespace: EditorNamespace) -> Self {
        Self {
            entries: HashMap::new(),
            namespace,
        }
    }

    pub(crate) fn install(
        &mut self,
        part: EditPartId,
        role: PolicyRole,
        policy: Box<dyn EditPolicy<A>>,
    ) -> Result<(), PolicyError> {
        if part.namespace() != self.namespace {
            return Err(PolicyError::operation(
                "policy host belongs to another Viewer",
            ));
        }
        let roles = self.entries.entry(part).or_default();
        if roles.contains_key(&role) {
            return Err(PolicyError::operation("duplicate policy role"));
        }
        roles.insert(role, policy);
        Ok(())
    }

    pub(crate) fn roles_mut(
        &mut self,
        part: EditPartId,
    ) -> Option<&mut BTreeMap<PolicyRole, Box<dyn EditPolicy<A>>>> {
        if part.namespace() != self.namespace {
            return None;
        }
        self.entries.get_mut(&part)
    }

    pub(crate) fn remove(
        &mut self,
        part: EditPartId,
    ) -> Option<BTreeMap<PolicyRole, Box<dyn EditPolicy<A>>>> {
        if part.namespace() != self.namespace {
            return None;
        }
        self.entries.remove(&part)
    }
}
