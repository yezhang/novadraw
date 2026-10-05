use super::shaping::{ParleyTextEngine, TextLayoutEngine};
use super::{
    BuiltinFont, FontDescriptor, FontMetrics, TextConstraints, TextError, TextLayout, TextMetrics,
};
use crate::render::FontData;
use crate::{FontId, ResourceError, ResourceRegistry};

/// Explicit composition root for standalone text preparation.
/// Attached figures borrow their Runtime's service instead.
pub struct TextSystem {
    engine: Box<dyn TextLayoutEngine>,
    resources: ResourceRegistry,
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    /// Creates an empty explicit font collection.
    pub fn new() -> Self {
        Self::with_engine(Box::new(ParleyTextEngine::new()))
    }

    pub fn with_engine(engine: Box<dyn TextLayoutEngine>) -> Self {
        Self {
            engine,
            resources: ResourceRegistry::new(),
        }
    }

    pub fn register_font(&mut self, data: FontData) -> Result<FontId, ResourceError> {
        let id = self.resources.register_font();
        if let Err(error) = self.replace_font(id, data) {
            self.resources.remove(id.resource_id())?;
            return Err(error);
        }
        Ok(id)
    }

    pub fn register_builtin_font(&mut self, font: BuiltinFont) -> Result<FontId, ResourceError> {
        self.register_font(FontData::new(font.bytes().to_vec()))
    }

    pub fn replace_font(&mut self, id: FontId, data: FontData) -> Result<(), ResourceError> {
        let revision = self
            .resources
            .next_revision(id.resource_id(), crate::ResourceKind::Font)?;
        self.engine
            .register_font(id.resource_id(), revision, data.bytes())
            .map_err(|_| ResourceError::InvalidFontData)?;
        self.resources.complete_font(id, data)?;
        Ok(())
    }

    pub fn remove_font(&mut self, id: FontId) -> Result<(), ResourceError> {
        self.resources.remove(id.resource_id())?;
        self.engine.remove_font(id.resource_id());
        Ok(())
    }

    pub fn resources(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub fn measure(&mut self, font: FontDescriptor) -> MeasureContext<'_> {
        MeasureContext::new(self.engine.as_mut(), font)
    }

    pub(crate) fn parts(&mut self) -> (&mut dyn TextLayoutEngine, &ResourceRegistry) {
        (self.engine.as_mut(), &self.resources)
    }
}

/// Layout-only borrow. Font overrides are local to this measurement.
pub struct MeasureContext<'a> {
    engine: &'a mut dyn TextLayoutEngine,
    font: FontDescriptor,
}

impl<'a> MeasureContext<'a> {
    pub(crate) fn new(engine: &'a mut dyn TextLayoutEngine, font: FontDescriptor) -> Self {
        Self { engine, font }
    }
    pub fn font(&self) -> &FontDescriptor {
        &self.font
    }
    pub fn set_font(&mut self, font: FontDescriptor) {
        self.font = font;
    }
    pub fn font_metrics(&mut self) -> Result<FontMetrics, TextError> {
        self.engine.font_metrics(&self.font)
    }
    pub fn layout_text(
        &mut self,
        text: &str,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.engine.layout(text, &self.font, constraints)
    }
    pub fn measure_text(
        &mut self,
        text: &str,
        constraints: TextConstraints,
    ) -> Result<TextMetrics, TextError> {
        self.layout_text(text, constraints)
            .map(|layout| layout.metrics())
    }
}
