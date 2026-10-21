use super::{
    ContributionKey, FLOW, FlowBlockStyle, FlowCapability, FlowContributionContext,
    FlowContributionPayload, FlowContributionRegistry, FlowContributionSink, FlowContributor,
    FlowError,
};
use crate::{
    Figure, FigureId,
    figure::{
        CONTAINER, ChildClippingStrategy, ChildPolicy, ContainerCapability,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
        FigurePreparation, FigurePresentation, FigureStyle, PREPARATION, PreparationCapability,
        PreparationScene, PreparedScene,
    },
    geometry::Rectangle,
    runtime::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate},
    text::{MeasureContext, TextError},
};

#[derive(Clone)]
pub struct TextContribution(pub String);
impl FlowContributionPayload for TextContribution {}
pub const TEXT_CONTRIBUTION: ContributionKey<TextContribution> = ContributionKey::new("flow.text");
#[derive(Clone, Copy)]
pub struct GroupContribution;
impl FlowContributionPayload for GroupContribution {}
pub const GROUP_CONTRIBUTION: ContributionKey<GroupContribution> =
    ContributionKey::new("flow.group");
#[derive(Clone, Copy)]
pub struct BlockContribution(pub FlowBlockStyle);
impl FlowContributionPayload for BlockContribution {}
pub const BLOCK_CONTRIBUTION: ContributionKey<BlockContribution> =
    ContributionKey::new("flow.block");
#[derive(Clone, Copy)]
pub struct FigureBoxContribution(pub FigureId);
impl FlowContributionPayload for FigureBoxContribution {}
pub const FIGURE_BOX_CONTRIBUTION: ContributionKey<FigureBoxContribution> =
    ContributionKey::new("flow.figure-box");

impl FlowContributionRegistry {
    /// The standard library uses exactly the same registration API as external handlers.
    pub fn standard() -> Self {
        let mut registry = Self::new();
        registry
            .register(
                TEXT_CONTRIBUTION,
                |p: &TextContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::FlowAssembly<'_>| {
                    out.text(c.source, &p.0, c.text_style()?)
                },
            )
            .expect("distinct standard identity");
        registry
            .register(
                GROUP_CONTRIBUTION,
                |_: &GroupContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::FlowAssembly<'_>| { out.children(c.source) },
            )
            .expect("distinct standard identity");
        registry
            .register(
                BLOCK_CONTRIBUTION,
                |p: &BlockContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::FlowAssembly<'_>| { out.block(c.source, p.0) },
            )
            .expect("distinct standard identity");
        registry
            .register(
                FIGURE_BOX_CONTRIBUTION,
                |p: &FigureBoxContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::FlowAssembly<'_>| {
                    out.figure_box(c.source, p.0, c.text_style()?)
                },
            )
            .expect("distinct standard identity");
        super::border::register(&mut registry);
        registry
    }
}

#[derive(Clone)]
pub struct FlowPageFigure {
    bounds: Rectangle,
    style: FigureStyle,
    registry: FlowContributionRegistry,
}
impl FlowPageFigure {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            style: FigureStyle::default(),
            registry: FlowContributionRegistry::standard(),
        }
    }
    pub fn with_registry(mut self, registry: FlowContributionRegistry) -> Self {
        self.registry = registry;
        self
    }
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = style;
        self
    }
}
impl FlowContributor for FlowPageFigure {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(GROUP_CONTRIBUTION, GroupContribution);
        Ok(())
    }
}
impl FigurePreparation for FlowPageFigure {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Err(TextError::InvalidPreparedScene(
            "structured flow requires a frozen subtree".into(),
        ))
    }
    fn prepare_scene(
        &self,
        c: &mut MeasureContext<'_>,
        b: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        super::snapshot::prepare(&self.registry, c, b, scene).map_err(FlowError::into_text)
    }
}
impl Figure for FlowPageFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn initial_style(&self) -> FigureStyle {
        self.style.clone()
    }
    fn name(&self) -> &'static str {
        "FlowPageFigure"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(FLOW, FlowCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigureContainer for FlowPageFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}

#[derive(Clone)]
pub struct BlockFlowFigure {
    bounds: Rectangle,
    style: FigureStyle,
    block: FlowBlockStyle,
}
impl BlockFlowFigure {
    pub fn new(block: FlowBlockStyle) -> Result<Self, FlowError> {
        Ok(Self {
            bounds: Rectangle::ZERO,
            style: FigureStyle::default(),
            block: block.validate()?,
        })
    }
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = style;
        self
    }
    pub fn block_style(&self) -> FlowBlockStyle {
        self.block
    }
}
impl Default for BlockFlowFigure {
    fn default() -> Self {
        Self::new(FlowBlockStyle::default()).expect("valid default block")
    }
}
impl FlowContributor for BlockFlowFigure {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(BLOCK_CONTRIBUTION, BlockContribution(self.block));
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct InlineFlowFigure {
    bounds: Rectangle,
    style: FigureStyle,
    border: Option<std::sync::Arc<dyn super::FlowFragmentBorder>>,
}
impl InlineFlowFigure {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = style;
        self
    }
    pub fn with_fragment_border(
        mut self,
        border: impl super::FlowFragmentBorder + 'static,
    ) -> Self {
        self.border = Some(std::sync::Arc::new(border));
        self
    }
}
impl FlowContributor for InlineFlowFigure {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        if let Some(border) = &self.border {
            out.append(
                super::BORDER_CONTRIBUTION,
                super::BorderContribution(border.clone()),
            );
        } else {
            out.append(GROUP_CONTRIBUTION, GroupContribution);
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct StructuredTextFigure {
    bounds: Rectangle,
    style: FigureStyle,
    text: String,
}
impl StructuredTextFigure {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            style: FigureStyle::default(),
            text: text.into(),
        }
    }
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = style;
        self
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl FlowContributor for StructuredTextFigure {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(TEXT_CONTRIBUTION, TextContribution(self.text.clone()));
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct FlowAdapterFigure {
    bounds: Rectangle,
    style: FigureStyle,
}
impl FlowAdapterFigure {
    pub fn new() -> Self {
        Self::default()
    }
}
impl FlowContributor for FlowAdapterFigure {
    fn contribute(
        &self,
        c: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        let children = c.scene.children(c.source)?;
        if children.len() != 1 {
            return Err(FlowError::InvalidStructure);
        }
        out.append(FIGURE_BOX_CONTRIBUTION, FigureBoxContribution(children[0]));
        Ok(())
    }
}
macro_rules! node_figure {
    ($ty:ty) => {
        impl Figure for $ty {
            fn initial_bounds(&self) -> Rectangle {
                self.bounds
            }
            fn initial_style(&self) -> FigureStyle {
                self.style.clone()
            }
            fn name(&self) -> &'static str {
                stringify!($ty)
            }
            fn register_capabilities(
                &self,
                out: &mut FigureCapabilityBuilder,
            ) -> Result<(), FigureCapabilityRegistrationError> {
                out.register(FLOW, FlowCapability::of::<Self>())?;
                out.register(CONTAINER, ContainerCapability::of::<Self>())
            }
        }
    };
}
node_figure!(BlockFlowFigure);
node_figure!(InlineFlowFigure);
node_figure!(StructuredTextFigure);
node_figure!(FlowAdapterFigure);
impl FigureContainer for BlockFlowFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}
impl FigureContainer for InlineFlowFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}
impl FigureContainer for StructuredTextFigure {}
impl FigureContainer for FlowAdapterFigure {
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }
}

pub struct SetStructuredFlowText(pub String);
impl SetStructuredFlowText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}
impl FigureComponentUpdate for SetStructuredFlowText {
    type Figure = StructuredTextFigure;
    type Prepared = String;
    type Error = FlowError;
    fn prepare(
        self,
        current: &Self::Figure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<String>, FlowError> {
        if current.text == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0.clone()).with_property_change(
                crate::event::property::TEXT,
                current.text.clone(),
                self.0,
            ),
        )
    }
    fn commit(text: String, target: &mut Self::Figure) {
        target.text = text;
    }
}
