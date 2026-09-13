//! Application-owned model identity, read access, and notification adapters.
//!
//! The framework does not own a second copy of application data or expose an untyped object store
//! as its primary model protocol.

use std::{error::Error, fmt, hash::Hash};

/// Monotonic revision attached to application model notifications.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModelRevision(u64);

impl ModelRevision {
    /// Returns the first valid model revision.
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Creates a non-zero model revision.
    pub const fn new(value: u64) -> Result<Self, ModelRevisionError> {
        if value == 0 {
            return Err(ModelRevisionError::Zero);
        }
        Ok(Self(value))
    }

    /// Returns the raw revision value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Advances the revision without wrapping.
    pub const fn next(self) -> Result<Self, ModelRevisionError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(ModelRevisionError::Exhausted),
        }
    }
}

/// Failure to construct or advance a model revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelRevisionError {
    /// Revision zero is reserved as an invalid sentinel.
    Zero,
    /// The revision reached `u64::MAX` and cannot advance.
    Exhausted,
}

impl fmt::Display for ModelRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(formatter, "model revision must be non-zero"),
            Self::Exhausted => write!(formatter, "model revision space is exhausted"),
        }
    }
}

impl Error for ModelRevisionError {}

/// Ordered application model notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelEvent<I, E> {
    revision: ModelRevision,
    subject: I,
    payload: E,
}

impl<I, E> ModelEvent<I, E> {
    /// Creates a model event.
    pub fn new(revision: ModelRevision, subject: I, payload: E) -> Self {
        Self {
            revision,
            subject,
            payload,
        }
    }

    /// Returns the source model revision.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }

    /// Returns the model object affected by the event.
    pub const fn subject(&self) -> I
    where
        I: Copy,
    {
        self.subject
    }

    /// Returns the application-defined event payload.
    pub const fn payload(&self) -> &E {
        &self.payload
    }

    /// Consumes the event and returns its payload.
    pub fn into_payload(self) -> E {
        self.payload
    }
}

/// Application-owned model access used by the editor projection layer.
///
/// The adapter exposes stable identity and ordered notifications without transferring model
/// ownership into the editor framework.
pub trait ModelAdapter {
    /// Stable application model identity.
    type ModelId: Copy + Eq + Hash + fmt::Debug + 'static;
    /// Application-defined notification payload.
    type Event;
    /// Failure returned by model queries.
    type Error: Error + 'static;

    /// Returns the root model object projected by a Viewer.
    fn root(&self) -> Self::ModelId;

    /// Returns the latest stable model revision represented by model queries.
    fn revision(&self) -> ModelRevision;

    /// Returns direct model children in stable display order.
    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error>;

    /// Drains the next ordered notification batch.
    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>>;
}
