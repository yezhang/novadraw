//! Structured, identity-preserving text flow through an open contribution registry.
mod assembly;
mod border;
mod nodes;
mod protocol;
mod snapshot;

pub use assembly::{FlowAssembly, FlowBlockStyle, FlowSourceRange};
pub use border::{
    BORDER_CONTRIBUTION, BorderContribution, FlowBorderFragment, FlowBorderMetrics,
    FlowFragmentBorder,
};
pub use nodes::{
    BLOCK_CONTRIBUTION, BlockContribution, BlockFlowFigure, FIGURE_BOX_CONTRIBUTION,
    FigureBoxContribution, FlowAdapterFigure, FlowPageFigure, GROUP_CONTRIBUTION,
    GroupContribution, InlineFlowFigure, SetFlowBlockStyle, SetStructuredFlowText,
    StructuredTextFigure, TEXT_CONTRIBUTION, TextContribution,
};
pub use protocol::{
    ContributionKey, FLOW, FlowCapability, FlowContributionContext, FlowContributionHandler,
    FlowContributionPayload, FlowContributionRegistry, FlowContributionSink, FlowContributor,
};
pub use snapshot::{
    FlowFragment, FlowLayoutService, FlowParagraphSnapshot, FlowSnapshot, SourceTextPosition,
};

use crate::{FigureId, text::TextError};

#[derive(Debug)]
pub enum FlowError {
    Text(TextError),
    DuplicateContribution(&'static str),
    InvalidContribution(&'static str),
    UnknownContribution(&'static str),
    MissingContributor(FigureId),
    InvalidStructure,
    DepthLimit,
}
impl From<TextError> for FlowError {
    fn from(error: TextError) -> Self {
        Self::Text(error)
    }
}
impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(e) => e.fmt(f),
            Self::DuplicateContribution(n) => write!(f, "duplicate flow contribution: {n}"),
            Self::InvalidContribution(n) => write!(f, "invalid flow contribution: {n}"),
            Self::UnknownContribution(n) => write!(f, "unregistered flow contribution: {n}"),
            Self::MissingContributor(id) => write!(f, "missing flow contributor: {id:?}"),
            Self::InvalidStructure => f.write_str("invalid flow structure"),
            Self::DepthLimit => f.write_str("flow nesting exceeds the tree depth limit"),
        }
    }
}
impl std::error::Error for FlowError {}
impl FlowError {
    pub(crate) fn into_text(self) -> TextError {
        match self {
            Self::Text(e) => e,
            e => TextError::InvalidPreparedScene(e.to_string()),
        }
    }
}
