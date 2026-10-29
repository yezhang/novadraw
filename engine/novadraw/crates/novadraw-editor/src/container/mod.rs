//! Checked containment editing, parent-domain geometry, and open application plans.

mod creation;
mod geometry;
mod ordered;
mod service;
mod tool;
mod types;

pub use creation::{
    CheckedCreationPlan, ContainerCreationTemplate, ContainerCreationTool, CreationCandidate,
    CreationPlan, CreationPlanHeader, CreationStart,
};
pub use geometry::{ConstraintGeometry, GeometryTolerance};
pub use ordered::{InsertionPosition, ordered_insertion};
pub use tool::ContainerMoveTool;
pub use types::*;

use std::{error::Error, fmt};

use crate::{EditPartId, ViewerError};
use novadraw::geometry::InverseFailure;

pub(crate) const MAX_CONTAINMENT_DEPTH: usize = 10_000;

/// Structured rejection of a containment edit before a model command is submitted.
#[derive(Debug)]
pub enum ContainerEditError {
    /// The supplied live identity is not an eligible containment Part.
    Identity(EditPartId),
    /// An operation batch is empty, duplicated, or structurally inconsistent.
    Batch(&'static str),
    /// A candidate or plan does not match its checked provenance.
    Mismatch(&'static str),
    /// A model revision or policy generation changed during the gesture.
    Invalidated,
    /// The required policy explicitly refused permission.
    Rejected(String),
    /// Finite geometry could not be computed or represented.
    Geometry(&'static str),
    /// The source or target map is mathematically singular.
    Singular,
    /// A checked inverse failed numerically.
    Inverse(InverseFailure),
    /// Existing Viewer, policy, coordinate, or cleanup failure.
    Viewer(ViewerError),
}

impl fmt::Display for ContainerEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(part) => write!(f, "invalid container identity: {part:?}"),
            Self::Batch(reason) | Self::Mismatch(reason) | Self::Geometry(reason) => reason.fmt(f),
            Self::Invalidated => f.write_str("container gesture invalidated"),
            Self::Rejected(reason) => reason.fmt(f),
            Self::Singular => f.write_str("container coordinate map is singular"),
            Self::Inverse(reason) => write!(f, "container inverse failed: {reason:?}"),
            Self::Viewer(error) => error.fmt(f),
        }
    }
}

impl Error for ContainerEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Viewer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ViewerError> for ContainerEditError {
    fn from(error: ViewerError) -> Self {
        Self::Viewer(error)
    }
}

/// Keeps the model revision check at the actual execution boundary after session synchronization.
pub(crate) fn guard_command<A: crate::ModelAdapter + 'static>(
    command: Box<dyn crate::Command<A>>,
    baseline: crate::ModelRevision,
) -> Box<dyn crate::Command<A>> {
    Box::new(RevisionGuard { command, baseline })
}

struct RevisionGuard<A> {
    command: Box<dyn crate::Command<A>>,
    baseline: crate::ModelRevision,
}

impl<A: crate::ModelAdapter> crate::Command<A> for RevisionGuard<A> {
    fn label(&self) -> &str {
        self.command.label()
    }
    fn can_execute(&self, model: &A) -> bool {
        self.command.can_execute(model)
    }
    fn execute(&mut self, model: &mut A) -> Result<(), crate::CommandError> {
        if model.revision() != self.baseline {
            return Err(crate::CommandError::operation(
                "container model revision changed before execute",
            ));
        }
        self.command.execute(model)
    }
    fn undo(&mut self, model: &mut A) -> Result<(), crate::CommandError> {
        self.command.undo(model)
    }
    fn redo(&mut self, model: &mut A) -> Result<(), crate::CommandError> {
        self.command.redo(model)
    }
    fn can_undo(&self) -> bool {
        self.command.can_undo()
    }
    fn can_redo(&self) -> bool {
        self.command.can_redo()
    }
}
