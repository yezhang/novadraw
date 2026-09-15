//! Selection handles and transient source or target feedback.
//!
//! Feedback is represented by Figures in explicit root layers so it shares canonical coordinate,
//! clipping, hit-testing, and disposal behavior with the Draw2D runtime.

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

/// Interaction role carried by a handle visual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleRole {
    /// Selection-only handle with no bounds operation.
    Selection,
    /// Resize handle for one edge or corner.
    Resize(ResizeDirection),
    /// Draggable source or target endpoint of a connection.
    ConnectionEndpoint(ConnectionEndpoint),
}

/// Editor ownership associated with a registered Figure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualOwner {
    /// Stable visual belonging to an EditPart.
    Part(EditPartId),
    /// Interactive handle associated with a host EditPart.
    Handle {
        /// Handle identity.
        id: HandleId,
        /// EditPart manipulated by the handle.
        owner: EditPartId,
        /// Interaction represented by the handle.
        role: HandleRole,
    },
    /// Non-interactive transient feedback.
    Feedback {
        /// Feedback identity.
        id: FeedbackId,
        /// Optional EditPart whose operation produced the feedback.
        owner: Option<EditPartId>,
    },
}

impl VisualOwner {
    /// Returns the associated EditPart when one exists.
    pub const fn owner_part(self) -> Option<EditPartId> {
        match self {
            Self::Part(part) => Some(part),
            Self::Handle { owner, .. } => Some(owner),
            Self::Feedback { owner, .. } => owner,
        }
    }
}
