//! Runtime services around the Figure tree.
//!
//! [`Runtime`] is the preferred composition root. It owns one tree together
//! with interaction, deferred mutation, and update state.

pub mod context;
pub mod event;
pub mod focus;
pub mod interaction;
pub mod mutation;
pub mod resource;
// `runtime::Runtime` is the deliberate public domain name.
#[allow(clippy::module_inception)]
pub mod runtime;
pub mod update;

pub use focus::{
    FocusChange, FocusError, FocusTraversalDirection, FocusTraversalOutcome, FocusTraversalPolicy,
    TreeOrderFocusTraversal,
};
pub use interaction::{InteractionState, PointerId};
pub use mutation::RuntimeMutationError;
pub use resource::{
    FontId, ImageId, ResourceError, ResourceKind, ResourceRegistry, ResourceStatus,
};
pub use runtime::{BackendSessionError, Runtime};
