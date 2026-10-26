//! Per-operation geometric coverage hints.

/// Independently recorded shape or text antialiasing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AntialiasMode {
    /// Uses the backend session's stable default.
    #[default]
    Default,
    /// Requests antialiased geometric coverage.
    On,
    /// Samples geometry at device pixel centers; paint and clip alpha remain independent.
    Off,
}
