use std::sync::Arc;

use crate::{
    FigureId,
    text::{StyledTextRequest, shaping::TextLayoutCacheIdentity},
};

/// One exact request attempt. Equal requests alone do not prove equal external dependencies.
#[derive(Clone, Debug)]
pub struct FlowLayoutObservation {
    pub root: FigureId,
    pub owner: FigureId,
    pub build: u64,
    /// Existing checked scene signature, retained without a new provider callback.
    pub scene_revision: u64,
    pub sources: Vec<(FigureId, u64)>,
    pub phase: &'static str,
    /// Prefix measurement for this inline box, or the complete paragraph.
    pub inline_prefix: Option<usize>,
    pub engine_identity: Option<TextLayoutCacheIdentity>,
    pub engine_revision: u64,
    pub request: Arc<StyledTextRequest>,
    pub cache_hit: bool,
    pub succeeded: bool,
    /// Lookup and engine work; excludes copying this diagnostic record.
    pub elapsed_ns: u128,
}

/// Drained diagnostics. Dropped records make the request denominator incomplete.
#[derive(Clone, Debug, Default)]
pub struct FlowLayoutObservations {
    pub records: Vec<FlowLayoutObservation>,
    pub dropped: u64,
}
