use crate::{
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigurePreparation, FigurePresentation,
        FigureStyle, PREPARATION, PreparationCapability, PreparationScene, PreparedScene,
    },
    geometry::Rectangle,
    layout::graph::{GraphLayoutError, GraphProjection},
    runtime::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate},
    text::{MeasureContext, TextError},
};

/// Flat container whose graph producer is installed after its child identities are allocated.
pub struct GraphProjectionFigure {
    bounds: Rectangle,
    style: FigureStyle,
    projection: Option<GraphProjection>,
}
impl GraphProjectionFigure {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            style: FigureStyle::default(),
            projection: None,
        }
    }
    pub fn with_style(mut self, style: FigureStyle) -> Self {
        self.style = style;
        self
    }
}
impl Figure for GraphProjectionFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn initial_style(&self) -> FigureStyle {
        self.style.clone()
    }
    fn name(&self) -> &'static str {
        "GraphProjectionFigure"
    }
    fn register_capabilities(
        &self,
        output: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        output.register(CONTAINER, ContainerCapability::of::<Self>())?;
        output.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigureContainer for GraphProjectionFigure {
    fn child_clipping_strategy(&self) -> super::ChildClippingStrategy {
        super::ChildClippingStrategy::OverflowVisible
    }
}
impl FigurePreparation for GraphProjectionFigure {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Err(TextError::GraphProjection(Box::new(
            GraphLayoutError::InvalidProjectionBinding,
        )))
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        _: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        self.projection
            .as_ref()
            .ok_or_else(|| {
                TextError::GraphProjection(Box::new(GraphLayoutError::InvalidProjectionBinding))
            })?
            .prepare(context, scene)
    }
}

/// Source replacement uses the ordinary typed component mutation path.
pub struct SetGraphProjection(pub GraphProjection);
impl FigureComponentUpdate for SetGraphProjection {
    type Figure = GraphProjectionFigure;
    type Prepared = GraphProjection;
    type Error = GraphLayoutError;
    fn prepare(
        self,
        _: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<GraphProjection>, GraphLayoutError> {
        if self.0.binding().owner() != context.figure_id {
            return Err(GraphLayoutError::InvalidProjectionBinding);
        }
        Ok(PreparedFigureUpdate::layout_geometry_and_paint(self.0))
    }
    fn commit(projection: GraphProjection, figure: &mut Self::Figure) {
        figure.projection = Some(projection);
    }
}
