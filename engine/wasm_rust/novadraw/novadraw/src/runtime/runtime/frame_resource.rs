use crate::render::{
    BackendSessionId, BuiltinFont, FontData, ImageData, ImageDecodeError, ResourceId,
};
use crate::{FigureId, FontId, ImageId, Rectangle, ResourceError, ResourceStatus};

use super::{BackendSessionError, LogicalViewportResizeError, Runtime};

impl Runtime {
    pub fn logical_viewport(&self) -> Option<Rectangle> {
        self.logical_viewport
    }

    pub fn resize_logical_viewport(
        &mut self,
        width: f64,
        height: f64,
    ) -> Result<bool, LogicalViewportResizeError> {
        if self.faulted {
            return Err(LogicalViewportResizeError::Faulted);
        }
        if !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 {
            return Err(LogicalViewportResizeError::InvalidSize { width, height });
        }
        let bounds = Rectangle::new(0.0, 0.0, width, height);
        if self.logical_viewport == Some(bounds) {
            return Ok(false);
        }
        self.logical_viewport = Some(bounds);
        self.tree.resize_logical_viewport(&mut self.updates, bounds);
        self.full_redraw_pending = true;
        Ok(true)
    }

    pub fn backend_session_id(&self) -> BackendSessionId {
        self.backend_session_id
    }

    pub fn reset_backend_session(&mut self) -> Result<BackendSessionId, BackendSessionError> {
        if self.faulted {
            return Err(BackendSessionError::Faulted);
        }
        let next = self
            .backend_session_id
            .next()
            .ok_or(BackendSessionError::Exhausted)?;
        self.backend_session_id = next;
        self.in_flight = None;
        self.session_sync_pending = true;
        self.full_redraw_pending = true;
        Ok(next)
    }

    pub fn register_image(&mut self) -> ImageId {
        self.try_register_image()
            .expect("cannot register an image in a faulted Runtime")
    }

    pub fn try_register_image(&mut self) -> Result<ImageId, ResourceError> {
        self.guarded_resource_mutation(|runtime| Ok(runtime.resources.register_image()))
    }

    pub fn register_font(&mut self) -> FontId {
        self.try_register_font()
            .expect("cannot register a font in a faulted Runtime")
    }

    pub fn try_register_font(&mut self) -> Result<FontId, ResourceError> {
        self.guarded_resource_mutation(|runtime| Ok(runtime.resources.register_font()))
    }

    pub fn register_builtin_font(&mut self, builtin: BuiltinFont) -> Result<FontId, ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        if let Some(id) = self.builtin_fonts.get(&builtin) {
            return Ok(*id);
        }
        let id = self.try_register_font()?;
        self.complete_font(id, FontData::new(builtin.bytes().to_vec()))?;
        self.builtin_fonts.insert(builtin, id);
        Ok(id)
    }

    pub fn resource_status(
        &self,
        resource_id: ResourceId,
    ) -> Result<&ResourceStatus, ResourceError> {
        self.resources.status(resource_id)
    }

    pub fn add_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        if !self.tree.is_attached(figure) {
            return Err(ResourceError::UnknownFigure);
        }
        self.resources.add_dependency(resource_id, figure)
    }

    pub fn remove_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        self.guarded_resource_mutation(|runtime| {
            runtime.resources.remove_dependency(resource_id, figure)
        })
    }

    pub fn complete_image(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(move |runtime| runtime.complete_image_inner(id, image))
    }

    fn complete_image_inner(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        let dependents = self.resources.complete_image(id, image)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn complete_image_bytes(
        &mut self,
        id: ImageId,
        bytes: &[u8],
        scale: f64,
    ) -> Result<(), ImageDecodeError> {
        match ImageData::decode(bytes, scale) {
            Ok(image) => self
                .complete_image(id, image)
                .map_err(|_| ImageDecodeError::ResourceUpdateFailed),
            Err(error) => {
                let _ = self.fail_resource(id.resource_id(), error.to_string());
                Err(error)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn complete_image_file(
        &mut self,
        id: ImageId,
        path: impl AsRef<std::path::Path>,
        scale: f64,
    ) -> Result<(), ImageDecodeError> {
        match ImageData::decode_file(path, scale) {
            Ok(image) => self
                .complete_image(id, image)
                .map_err(|_| ImageDecodeError::ResourceUpdateFailed),
            Err(error) => {
                let _ = self.fail_resource(id.resource_id(), error.to_string());
                Err(error)
            }
        }
    }

    pub fn complete_font(&mut self, id: FontId, font: FontData) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(move |runtime| runtime.complete_font_inner(id, font))
    }

    fn complete_font_inner(&mut self, id: FontId, font: FontData) -> Result<(), ResourceError> {
        let revision = self
            .resources
            .next_revision(id.resource_id(), crate::ResourceKind::Font)?;
        self.text
            .register_font(id.resource_id(), revision, font.bytes())
            .map_err(|_| ResourceError::InvalidFontData)?;
        let dependents = self.resources.complete_font(id, font)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn fail_resource(
        &mut self,
        id: ResourceId,
        reason: impl Into<String>,
    ) -> Result<(), ResourceError> {
        let reason = reason.into();
        self.guarded_resource_mutation(move |runtime| runtime.fail_resource_inner(id, reason))
    }

    fn fail_resource_inner(&mut self, id: ResourceId, reason: String) -> Result<(), ResourceError> {
        if self.resources.kind(id)? == crate::ResourceKind::Font {
            self.text.remove_font(id);
        }
        let dependents = self.resources.fail(id, reason)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }

    pub fn remove_resource(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(|runtime| runtime.remove_resource_inner(id))
    }

    fn remove_resource_inner(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        if self.resources.kind(id)? == crate::ResourceKind::Font {
            self.text.remove_font(id);
            self.builtin_fonts
                .retain(|_, font_id| font_id.resource_id() != id);
        }
        let dependents = self.resources.remove(id)?;
        self.invalidate_resource_dependents(dependents);
        Ok(())
    }
}
