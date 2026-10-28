use crate::runtime::timing::{TimingRecorder, TimingScope};

/// Optional, instance-owned CPU scopes. Names describe diagnostics, never behavior identity.
/// External engines can report their own stages through the same interface.
#[derive(Clone, Default)]
pub struct TextLayoutDiagnostics(pub(crate) TimingRecorder);

impl TextLayoutDiagnostics {
    /// A disabled scope does not read a clock or allocate a record.
    pub fn time(&self, stage: &'static str) -> TextLayoutTimingScope {
        TextLayoutTimingScope(self.0.start(stage))
    }
}

/// Finishes a synchronous CPU stage on return, including error returns.
#[must_use]
pub struct TextLayoutTimingScope(#[allow(dead_code)] TimingScope);
