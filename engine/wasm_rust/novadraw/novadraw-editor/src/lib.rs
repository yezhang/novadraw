//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! G1 provides application model identity/notification boundaries and model-only command history.
//! Later milestones add EditPart, Viewer, Tool, Request, Policy, and feedback behavior without
//! moving application state into the Figure runtime.

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
