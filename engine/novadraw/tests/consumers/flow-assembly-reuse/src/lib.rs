use novadraw::{
    graphics::Color,
    text::{FontDescriptor, StyledTextRequest, TextConstraints, TextContentStore, TextStyle},
};

use novadraw::figure::{
    Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigurePreparation,
    FigurePresentation, PREPARATION, PreparationCapability, PreparationScene, PreparedScene,
    flow::{
        ContributionKey, DeclaredFlowHandler, FLOW, FlowAssembly, FlowCapability,
        FlowContributionContext, FlowContributionPayload, FlowContributionRegistry,
        FlowContributionSink, FlowContributor, FlowError, FlowLayoutService, FlowReuseDeclaration,
        FlowReuseIdentity,
    },
};
use novadraw::{
    geometry::Rectangle,
    text::{MeasureContext, TextError},
};

struct ExternalText;
impl FlowContributionPayload for ExternalText {}
const EXTERNAL_TEXT: ContributionKey<ExternalText> = ContributionKey::new("consumer.text");

/// A real consumer root compiled for Native and Wasm, including its handler.
pub struct ReusablePage {
    service: FlowLayoutService,
    identity: FlowReuseIdentity,
}
impl Default for ReusablePage {
    fn default() -> Self {
        let mut registry = FlowContributionRegistry::standard();
        let handler = FlowReuseIdentity::new();
        registry
            .register(
                EXTERNAL_TEXT,
                DeclaredFlowHandler::new(
                    |_: &ExternalText,
                     context: &FlowContributionContext<'_>,
                     out: &mut FlowAssembly<'_>| {
                        out.text(
                            context.source,
                            "consumer shared paragraph",
                            context.text_style()?,
                        )
                    },
                    move |_: &FlowContributionContext<'_>| Ok(Some(handler.declaration(0))),
                ),
            )
            .expect("one external payload");
        let service = FlowLayoutService::new(registry).with_assembly_reuse(true);
        service
            .set_assembly_limits(1024 * 1024, 4 * 1024 * 1024)
            .expect("empty bounded service");
        Self {
            service,
            identity: FlowReuseIdentity::new(),
        }
    }
}
impl Figure for ReusablePage {
    fn name(&self) -> &'static str {
        "ReusableConsumerPage"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 150.0, 100.0)
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(FLOW, FlowCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FlowContributor for ReusablePage {
    fn assembly_reuse(
        &self,
        context: &FlowContributionContext<'_>,
    ) -> Result<Option<FlowReuseDeclaration>, FlowError> {
        context
            .scene
            .budget()
            .allocate(
                "consumer declaration",
                std::mem::size_of::<std::any::TypeId>() as u64,
            )
            .map_err(TextError::from)?;
        Ok(Some(
            self.identity
                .declaration(0)
                .with_contribution(EXTERNAL_TEXT),
        ))
    }
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(EXTERNAL_TEXT, ExternalText);
        Ok(())
    }
}
impl FigurePreparation for ReusablePage {
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: novadraw::figure::MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<novadraw::figure::PreparationMeasurement, novadraw::figure::PreparationMeasureError>
    {
        self.service.measure(context, constraints, scene)
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Err(TextError::InvalidPreparedScene("scene required".into()))
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        self.service
            .prepare(context, bounds, scene)
            .map_err(FlowError::into_text)
    }
}

/// Independent consumer: checked shared content enters the ordinary request API.
#[unsafe(no_mangle)]
pub extern "C" fn shared_flow_content() -> bool {
    let store = TextContentStore::new(usize::MAX);
    let style = TextStyle::new(FontDescriptor::default(), Color::BLACK);
    let Ok(content) = store.create("shared".into(), style, vec![]) else {
        return false;
    };
    StyledTextRequest::from_content(content, vec![], TextConstraints::UNBOUNDED)
        .is_ok_and(|request| request.text() == "shared")
}

#[cfg(test)]
mod contracts {
    include!("../../../../crates/novadraw/tests/flow_assembly_reuse_contract.rs");
}
