use super::{CapabilityKey, Figure, FigurePresentation};
use crate::{Rectangle, text::TextError};

/// Immutable self-content prepared before animation admission.
///
/// Implementations copy their already prepared inputs; this entry does not measure,
/// shape text, run layout, or retain a reference to mutable Figure state.
pub trait FigureContentSnapshot {
    /// Changes when the prepared content or its externally owned inputs change.
    fn content_revision(&self) -> u64;

    /// Freezes self-content in node-local coordinates at the committed bounds.
    /// Sampled bounds change placement and clipping, not this content's metrics.
    fn snapshot_content(&self, bounds: Rectangle) -> Result<FigurePresentation, TextError>;
}

/// Typed adapter shared by built-in and consumer-defined Figures.
#[derive(Clone, Copy)]
pub struct ContentSnapshotCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureContentSnapshot>,
}

impl ContentSnapshotCapability {
    pub fn of<T: Figure + FigureContentSnapshot + 'static>() -> Self {
        Self {
            resolve: |figure| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(|figure| figure as &dyn FigureContentSnapshot)
            },
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureContentSnapshot> {
        (self.resolve)(figure)
    }
}

pub const CONTENT_SNAPSHOT: CapabilityKey<ContentSnapshotCapability> =
    CapabilityKey::new("novadraw.content-snapshot");
