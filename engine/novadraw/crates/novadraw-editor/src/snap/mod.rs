//! Open, Viewer-owned snapping protocols and checked geometric arbitration.
//!
//! Extensions contribute declarations; only the Editor can issue checked input and receipts.
//! Solving does not authorize a model command or replace the container's release validation.

mod creation;
mod feedback;
mod geometry;
mod grid;
mod lines;
mod movement;
pub(crate) mod registry;
mod resize;
mod resize_tool;
mod service;
mod solver;
#[cfg(test)]
mod tests;

pub use creation::*;
pub use feedback::*;
pub use geometry::*;
pub use grid::*;
pub use lines::*;
pub use movement::*;
pub use registry::*;
pub use resize::*;
pub use resize_tool::*;
pub use solver::*;

use crate::{ViewerError, diagnostics::OwnedError};
use std::{error::Error, fmt};

/// Failure of one complete snapping operation; no partial receipt is issued.
#[derive(Debug)]
pub enum SnapError {
    /// The Viewer cannot accept new work.
    Faulted,
    /// A handle belongs to another registry or has been replaced or removed.
    Identity,
    /// Input or declared dependencies changed during a callback.
    Invalidated,
    /// An extension supplied an invalid finite-geometry declaration.
    Invalid(&'static str),
    /// Legal geometry cannot satisfy the retained operation or layout constraints.
    Rejected(&'static str),
    /// An explicit resource limit was exceeded.
    Budget(&'static str),
    /// An extension error, retaining its original source without thread or Clone bounds.
    Extension(ExtensionError),
    /// An existing Viewer or coordinate error.
    Viewer(ViewerError),
}

/// Owned error from an extension callback.
#[derive(Debug)]
pub struct ExtensionError {
    boundary: &'static str,
    source: OwnedError,
}

impl ExtensionError {
    /// Returns the callback boundary.
    pub const fn boundary(&self) -> &'static str {
        self.boundary
    }
}

impl SnapError {
    pub(crate) fn extension(boundary: &'static str, error: impl Error + 'static) -> Self {
        Self::Extension(ExtensionError {
            boundary,
            source: OwnedError::new(error),
        })
    }
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.boundary, self.source.get())
    }
}
impl Error for ExtensionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.get())
    }
}
impl fmt::Display for SnapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Faulted => f.write_str("snapping Viewer is faulted"),
            Self::Identity => f.write_str("foreign or retired snapping identity"),
            Self::Invalidated => f.write_str("snapping input invalidated"),
            Self::Invalid(s) | Self::Rejected(s) | Self::Budget(s) => f.write_str(s),
            Self::Extension(e) => e.fmt(f),
            Self::Viewer(e) => e.fmt(f),
        }
    }
}
impl Error for SnapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Extension(e) => Some(e),
            Self::Viewer(e) => Some(e),
            _ => None,
        }
    }
}
impl From<ViewerError> for SnapError {
    fn from(error: ViewerError) -> Self {
        if matches!(error, ViewerError::Faulted) {
            Self::Faulted
        } else {
            Self::Viewer(error)
        }
    }
}
