use crate::render::{BackendSessionId, BuiltinFont, FontData, ImageData, ResourceId};
use crate::{
    FigureId, FontId, ImageCompletionError, ImageId, Rectangle, ResourceError, ResourceKind,
    ResourceRequestToken, ResourceStatus,
};

use super::{BackendSessionError, LogicalViewportResizeError, Runtime};

impl Runtime {
    pub(crate) fn logical_viewport(&self) -> Option<Rectangle> {
        self.logical_viewport
    }

    pub(crate) fn resize_logical_viewport(
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

    pub(crate) fn backend_session_id(&self) -> BackendSessionId {
        self.backend_session_id
    }

    pub(crate) fn reset_backend_session(
        &mut self,
    ) -> Result<BackendSessionId, BackendSessionError> {
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

    pub(crate) fn register_image(&mut self) -> Result<ImageId, ResourceError> {
        self.guarded_resource_mutation(|runtime| Ok(runtime.resources.register_image()))
    }

    pub(crate) fn register_font(&mut self) -> Result<FontId, ResourceError> {
        self.guarded_resource_mutation(|runtime| Ok(runtime.resources.register_font()))
    }

    pub(crate) fn register_builtin_font(
        &mut self,
        builtin: BuiltinFont,
    ) -> Result<FontId, ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        if let Some(id) = self.builtin_fonts.get(&builtin) {
            return Ok(*id);
        }
        let id = self.register_font()?;
        self.complete_font(id, FontData::new(builtin.bytes().to_vec()))?;
        self.builtin_fonts.insert(builtin, id);
        Ok(id)
    }

    pub(crate) fn resource_status(
        &self,
        resource_id: ResourceId,
    ) -> Result<&ResourceStatus, ResourceError> {
        self.resources.status(resource_id)
    }

    pub(crate) fn add_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        if self.faulted {
            return Err(ResourceError::Faulted);
        }
        if self.tree.is_attached(figure) != Ok(true) {
            return Err(ResourceError::UnknownFigure);
        }
        self.resources.add_dependency(resource_id, figure)?;
        self.record_resource_input(resource_id, figure);
        Ok(())
    }

    pub(crate) fn remove_resource_dependency(
        &mut self,
        resource_id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        self.guarded_resource_mutation(|runtime| {
            let removed = runtime.resources.remove_dependency(resource_id, figure)?;
            if removed && let Some(node) = runtime.tree.node_mut(figure) {
                node.resource_inputs.retain(|(id, _)| *id != resource_id);
            }
            Ok(removed)
        })
    }

    pub(crate) fn complete_image(
        &mut self,
        id: ImageId,
        image: ImageData,
    ) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(move |runtime| runtime.complete_image_inner(id, image))
    }

    fn complete_image_inner(&mut self, id: ImageId, image: ImageData) -> Result<(), ResourceError> {
        let dependents = self.resources.complete_image(id, image)?;
        self.invalidate_resource_dependents(id.resource_id(), dependents);
        Ok(())
    }

    pub(crate) fn begin_resource_request(
        &mut self,
        id: ResourceId,
    ) -> Result<ResourceRequestToken, ResourceError> {
        self.guarded_resource_mutation(|runtime| runtime.resources.begin_request(id))
    }

    pub(crate) fn complete_image_request(
        &mut self,
        token: ResourceRequestToken,
        image: ImageData,
    ) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(move |runtime| {
            let dependents = runtime.resources.complete_image_request(token, image)?;
            runtime.invalidate_resource_dependents(token.resource_id(), dependents);
            Ok(())
        })
    }

    pub(crate) fn complete_image_request_bytes(
        &mut self,
        token: ResourceRequestToken,
        bytes: &[u8],
        scale: f64,
    ) -> Result<(), ImageCompletionError> {
        if self.faulted {
            return Err(ResourceError::Faulted.into());
        }
        self.resources
            .validate_request(token, ResourceKind::Image)?;
        match ImageData::decode(bytes, scale) {
            Ok(image) => self
                .complete_image_request(token, image)
                .map_err(Into::into),
            Err(error) => {
                self.fail_resource_request(token, error.to_string())?;
                Err(ImageCompletionError::Decode(error))
            }
        }
    }

    pub(crate) fn complete_font(
        &mut self,
        id: FontId,
        font: FontData,
    ) -> Result<(), ResourceError> {
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
        self.invalidate_resource_dependents(id.resource_id(), dependents);
        Ok(())
    }

    pub(crate) fn complete_font_request(
        &mut self,
        token: ResourceRequestToken,
        font: FontData,
    ) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(move |runtime| {
            runtime
                .resources
                .validate_request(token, ResourceKind::Font)?;
            let revision = runtime
                .resources
                .next_revision(token.resource_id(), ResourceKind::Font)?;
            runtime
                .text
                .register_font(token.resource_id(), revision, font.bytes())
                .map_err(|_| ResourceError::InvalidFontData)?;
            let dependents = runtime.resources.complete_font_request(token, font)?;
            runtime.invalidate_resource_dependents(token.resource_id(), dependents);
            Ok(())
        })
    }

    pub(crate) fn fail_resource(
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
        self.invalidate_resource_dependents(id, dependents);
        Ok(())
    }

    pub(crate) fn fail_resource_request(
        &mut self,
        token: ResourceRequestToken,
        reason: impl Into<String>,
    ) -> Result<(), ResourceError> {
        let reason = reason.into();
        self.guarded_resource_mutation(move |runtime| {
            let kind = runtime.resources.kind(token.resource_id())?;
            runtime.resources.validate_request(token, kind)?;
            if kind == ResourceKind::Font {
                runtime.text.remove_font(token.resource_id());
            }
            let dependents = runtime.resources.fail_request(token, reason)?;
            runtime.invalidate_resource_dependents(token.resource_id(), dependents);
            Ok(())
        })
    }

    pub(crate) fn remove_resource(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        self.guarded_resource_mutation(|runtime| runtime.remove_resource_inner(id))
    }

    fn remove_resource_inner(&mut self, id: ResourceId) -> Result<(), ResourceError> {
        if self.resources.kind(id)? == crate::ResourceKind::Font {
            self.text.remove_font(id);
            self.builtin_fonts
                .retain(|_, font_id| font_id.resource_id() != id);
        }
        let dependents = self.resources.remove(id)?;
        self.invalidate_resource_dependents(id, dependents);
        Ok(())
    }
}
