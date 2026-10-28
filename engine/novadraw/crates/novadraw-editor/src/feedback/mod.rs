//! Selection handles and transient source or target feedback.
//!
//! Feedback is represented by Figures in explicit root layers so it shares canonical coordinate,
//! clipping, hit-testing, and disposal behavior with the Draw2D runtime.

use std::{any::Any, marker::PhantomData, sync::Arc};

use novadraw::geometry::Point;
use uuid::Uuid;

use crate::{ConnectionEndpoint, EditPartId, EditorNamespace, ResizeDirection};

macro_rules! overlay_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            namespace: EditorNamespace,
            local: Uuid,
        }

        impl $name {
            pub(crate) fn new(namespace: EditorNamespace) -> Self {
                Self {
                    namespace,
                    local: Uuid::new_v4(),
                }
            }

            /// Returns the Viewer namespace that owns this identity.
            pub const fn namespace(self) -> EditorNamespace {
                self.namespace
            }
        }
    };
}

overlay_id!(HandleId, "Identity of an interactive handle visual.");
overlay_id!(FeedbackId, "Identity of a transient feedback visual.");

/// Open typed behavior implemented by every handle action.
pub trait HandleActionProtocol: 'static {
    /// Action-specific immutable payload.
    type Payload: 'static;

    /// Returns the typed action identity.
    fn key() -> HandleActionKey<Self>;
}

/// Typed identity of one handle action.
pub struct HandleActionKey<R: ?Sized> {
    name: &'static str,
    marker: PhantomData<fn() -> R>,
}

impl<R: ?Sized> HandleActionKey<R> {
    /// Creates a key for one action protocol marker.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }

    /// Returns the diagnostic action name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<R: ?Sized> Clone for HandleActionKey<R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: ?Sized> Copy for HandleActionKey<R> {}

/// Opaque typed action attached to one handle instance.
#[derive(Clone)]
pub struct HandleAction {
    type_id: std::any::TypeId,
    name: &'static str,
    payload: Arc<dyn Any>,
}

impl HandleAction {
    /// Creates an action from a protocol's typed payload.
    pub fn new<R: HandleActionProtocol>(payload: R::Payload) -> Self {
        Self {
            type_id: std::any::TypeId::of::<R>(),
            name: R::key().name(),
            payload: Arc::new(payload),
        }
    }

    /// Returns this action's diagnostic name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Reads the payload only when the requested protocol identity matches.
    pub fn get<R: HandleActionProtocol>(&self) -> Option<&R::Payload> {
        (self.type_id == std::any::TypeId::of::<R>())
            .then(|| self.payload.downcast_ref::<R::Payload>())
            .flatten()
    }

    pub(crate) const fn type_id(&self) -> std::any::TypeId {
        self.type_id
    }
}

impl std::fmt::Debug for HandleAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HandleAction")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

macro_rules! handle_actions {
    ($(($action:ident, $payload:ty, $name:literal, $doc:literal)),+ $(,)?) => {
        $(
            #[doc = $doc]
            pub struct $action;

            impl HandleActionProtocol for $action {
                type Payload = $payload;

                fn key() -> HandleActionKey<Self> {
                    HandleActionKey::new($name)
                }
            }
        )+
    };
}

handle_actions!(
    (
        SelectionAction,
        (),
        "selection",
        "Selection-only handle action."
    ),
    (
        ResizeAction,
        ResizeDirection,
        "resize",
        "Bounds resize action."
    ),
    (
        ConnectionEndpointAction,
        ConnectionEndpoint,
        "connection-endpoint",
        "Connection endpoint reconnect action."
    ),
    (
        BendpointMoveAction,
        usize,
        "bendpoint-move",
        "Bendpoint move action."
    ),
    (
        BendpointCreateAction,
        usize,
        "bendpoint-create",
        "Bendpoint creation action."
    ),
);

/// One framework-derived bendpoint handle placement in logical surface coordinates.
#[derive(Clone, Debug)]
pub struct BendpointHandleSite {
    action: HandleAction,
    location: Point,
}

impl BendpointHandleSite {
    /// Creates a handle placement.
    pub fn new(action: HandleAction, location: Point) -> Self {
        Self { action, location }
    }

    /// Returns the interaction represented by the handle.
    pub const fn action(&self) -> &HandleAction {
        &self.action
    }

    /// Returns the committed route location in logical surface coordinates.
    pub const fn location(&self) -> Point {
        self.location
    }
}

/// Editor ownership associated with a registered Figure.
#[derive(Clone, Debug)]
pub struct VisualOwner {
    kind: VisualOwnerKind,
}

#[derive(Clone, Debug)]
enum VisualOwnerKind {
    Part(EditPartId),
    Handle {
        id: HandleId,
        owner: EditPartId,
        action: HandleAction,
    },
    Feedback {
        id: FeedbackId,
        owner: Option<EditPartId>,
    },
}

impl VisualOwner {
    pub(crate) const fn part(part: EditPartId) -> Self {
        Self {
            kind: VisualOwnerKind::Part(part),
        }
    }

    pub(crate) fn handle(id: HandleId, owner: EditPartId, action: HandleAction) -> Self {
        Self {
            kind: VisualOwnerKind::Handle { id, owner, action },
        }
    }

    pub(crate) const fn feedback(id: FeedbackId, owner: Option<EditPartId>) -> Self {
        Self {
            kind: VisualOwnerKind::Feedback { id, owner },
        }
    }

    /// Returns the associated EditPart when one exists.
    pub const fn owner_part(&self) -> Option<EditPartId> {
        match &self.kind {
            VisualOwnerKind::Part(part) => Some(*part),
            VisualOwnerKind::Handle { owner, .. } => Some(*owner),
            VisualOwnerKind::Feedback { owner, .. } => *owner,
        }
    }

    pub(crate) fn handle_parts(&self) -> Option<(HandleId, EditPartId, &HandleAction)> {
        match &self.kind {
            VisualOwnerKind::Handle { id, owner, action } => Some((*id, *owner, action)),
            VisualOwnerKind::Part(_) | VisualOwnerKind::Feedback { .. } => None,
        }
    }

    pub(crate) const fn feedback_id(&self) -> Option<FeedbackId> {
        match self.kind {
            VisualOwnerKind::Feedback { id, .. } => Some(id),
            VisualOwnerKind::Part(_) | VisualOwnerKind::Handle { .. } => None,
        }
    }

    pub(crate) const fn is_part(&self) -> bool {
        matches!(self.kind, VisualOwnerKind::Part(_))
    }

    pub(crate) fn is_overlay(&self) -> bool {
        self.handle_parts().is_some() || self.feedback_id().is_some()
    }
}
