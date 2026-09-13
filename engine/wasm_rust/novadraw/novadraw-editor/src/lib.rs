//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! G1 provides application model identity/notification boundaries and model-only command history.
//! G2 adds EditPart topology and model-to-Figure projection without moving application state into
//! the Figure runtime.

#![deny(missing_docs)]

mod command;
mod domain;
mod feedback;
mod model;
mod part;
mod policy;
mod request;
mod tool;
mod viewer;

pub use command::{
    Command, CommandError, CommandOperation, CommandStack, CommandStackError, CommandStackEvent,
    CommandStackEventKind, CompoundCommand,
};
pub use model::{ModelAdapter, ModelEvent, ModelRevision, ModelRevisionError};
pub use part::{
    EditPartBehavior, EditPartError, EditPartFactory, EditPartId, EditorNamespace,
    PartFactoryContext, PartNode, PartTree, PartTreeError, VisualBuildContext, VisualUpdateContext,
};
pub use viewer::{GraphicalViewer, ViewerError};
