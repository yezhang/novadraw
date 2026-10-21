use super::{
    ContributionKey, FlowAssembly, FlowContributionContext, FlowContributionPayload,
    FlowContributionRegistry, FlowError,
};
use crate::{
    FigureId,
    geometry::Rectangle,
    graphics::{GraphicsError, PaintContext},
    text::{MeasureContext, ParagraphDirection, TextStyle},
};
use std::sync::Arc;

/// Reserved first/last inline boxes. Their baseline is the containing line's baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlowBorderMetrics {
    pub leading_width: f64,
    pub trailing_width: f64,
    pub height: f64,
}
impl FlowBorderMetrics {
    pub fn validate(self) -> Result<Self, FlowError> {
        if [self.leading_width, self.trailing_width, self.height]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(crate::text::TextError::InvalidMetric.into());
        }
        Ok(self)
    }
}
#[derive(Clone, Debug)]
pub struct FlowBorderFragment {
    pub source: FigureId,
    pub paragraph: usize,
    pub line: usize,
    pub bounds: Rectangle,
    pub direction: ParagraphDirection,
    pub style: TextStyle,
    pub leading: bool,
    pub trailing: bool,
    pub leading_box: Option<Rectangle>,
    pub trailing_box: Option<Rectangle>,
}
/// Open fragment-border payload, including owner-font measurement before shaping.
pub trait FlowFragmentBorder: Send + Sync {
    /// Produces an immutable renderer when drawing needs owner-dependent measured data.
    /// An already immutable border may use the default and paint itself.
    fn prepare_renderer(
        &self,
        _context: &mut MeasureContext<'_>,
        _style: &TextStyle,
    ) -> Result<Option<Arc<dyn FlowFragmentBorder>>, FlowError> {
        Ok(None)
    }

    fn metrics(
        &self,
        context: &mut MeasureContext<'_>,
        style: &TextStyle,
    ) -> Result<FlowBorderMetrics, FlowError>;
    fn paint_fragment(
        &self,
        fragment: &FlowBorderFragment,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError>;
}
#[derive(Clone)]
pub struct BorderContribution(pub Arc<dyn FlowFragmentBorder>);
impl FlowContributionPayload for BorderContribution {}
pub const BORDER_CONTRIBUTION: ContributionKey<BorderContribution> =
    ContributionKey::new("flow.fragment-border");

pub(crate) fn register(registry: &mut FlowContributionRegistry) {
    registry
        .register(
            BORDER_CONTRIBUTION,
            |p: &BorderContribution,
             c: &FlowContributionContext<'_>,
             out: &mut FlowAssembly<'_>| {
                out.decorate(c.source, p.0.clone())?;
                out.children(c.source)
            },
        )
        .expect("distinct standard identity");
}

#[derive(Clone)]
pub(crate) struct Decoration {
    pub source: FigureId,
    pub border: Arc<dyn FlowFragmentBorder>,
}
#[derive(Clone)]
pub(crate) struct BorderDrawing {
    pub fragment: FlowBorderFragment,
    pub border: Arc<dyn FlowFragmentBorder>,
}
impl std::fmt::Debug for BorderDrawing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fragment.fmt(f)
    }
}
