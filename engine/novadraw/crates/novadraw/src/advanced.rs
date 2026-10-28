//! Low-level engine state for diagnostics and specialized integrations.
//!
//! Most applications should use [`crate::Runtime`] and scoped mutable facades instead.

pub use crate::figure::RootFigure;
pub use crate::graph::{FigureNode, LayoutState, NodeState};
pub use crate::identity::RuntimeNamespace;
pub use crate::runtime::event::EventDispatcher;
pub use crate::runtime::interaction::InteractionState;
pub use crate::runtime::mutation::PendingMutations;
pub use crate::runtime::update::{NotificationQueue, UpdateManager};
