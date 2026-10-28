use super::Runtime;
use crate::{
    graphics::Color,
    host::{
        BackgroundFrameInput, HostBackgroundError as Error, HostBackgroundId,
        HostBackgroundRequest, HostBackgroundSnapshot, InstalledBackground,
    },
    render::NdCanvas,
};

impl Runtime {
    pub(crate) fn set_background_input(
        &mut self,
        input: BackgroundFrameInput,
    ) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        input.validate(self.background.input)?;
        if self.background.input != Some(input) {
            self.background.input = Some(input);
            self.background_changed();
        }
        Ok(())
    }

    pub(crate) fn begin_background(
        &mut self,
        source: HostBackgroundId,
        input: BackgroundFrameInput,
        base: Color,
    ) -> Result<HostBackgroundRequest, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        input.validate(self.background.input)?;
        let next_request = self
            .background
            .next_request
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        let replacing = self.background.source != Some(source);
        let generation = if replacing {
            self.background
                .generation
                .checked_add(1)
                .ok_or(Error::Exhausted)?
        } else {
            self.background.generation
        };
        let request = HostBackgroundRequest {
            destination: self.tree.namespace(),
            source,
            generation,
            request: next_request,
            input,
        };
        self.background.generation = generation;
        self.background.next_request = next_request;
        self.background.source = Some(source);
        self.background.input = Some(input);
        self.background.base = Some(base);
        self.background.request = Some(request);
        self.background.error = None;
        if replacing {
            self.background.current = None;
        }
        self.background_changed();
        Ok(request)
    }

    pub(crate) fn complete_background(
        &mut self,
        request: HostBackgroundRequest,
        result: Result<HostBackgroundSnapshot, Error>,
    ) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if self.background.request != Some(request) {
            return Err(Error::StaleRequest);
        }
        let result = self.install_background(request, result);
        if let Err(error) = &result {
            self.background.error = Some(error.clone());
        }
        result
    }

    fn install_background(
        &mut self,
        request: HostBackgroundRequest,
        result: Result<HostBackgroundSnapshot, Error>,
    ) -> Result<(), Error> {
        let snapshot = result?;
        if snapshot.request != request {
            return Err(Error::StaleRequest);
        }
        if self.background.input != Some(request.input) {
            return Err(Error::InputMismatch);
        }
        if let Some(current) = &self.background.current {
            if snapshot.revision < current.snapshot.revision {
                return Err(Error::StaleRequest);
            }
            if snapshot.revision == current.snapshot.revision {
                // Input changes require a new content revision too: no ambiguous content identity.
                if snapshot != current.snapshot {
                    return Err(Error::SnapshotConflict);
                }
                self.background.error = None;
                return Ok(());
            }
        }
        let candidate = self
            .scenes
            .imports
            .prepare(&self.resources, request.source.0, &snapshot.content.drawing)
            .map_err(|error| Error::Import(Box::new(error)))?;
        self.resources = candidate.registry;
        self.scenes.imports = candidate.imports;
        self.background.current = Some(InstalledBackground {
            snapshot,
            drawing: candidate.drawing,
            _leases: candidate.leases,
        });
        self.background.error = None;
        self.background_changed();
        Ok(())
    }

    pub(crate) fn remove_background(&mut self, base: Color) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        let generation = self
            .background
            .generation
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        self.background.generation = generation;
        self.background.request = None;
        self.background.source = None;
        self.background.current = None;
        self.background.error = None;
        self.background.base = Some(base);
        self.background_changed();
        Ok(())
    }

    fn background_changed(&mut self) {
        self.request_full_redraw();
        if let Some(waker) = &self.background.waker {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn validate_background_frame(
        &self,
        surface: crate::render::SurfaceInfo,
    ) -> Result<(), Error> {
        if let Some(input) = self.background.input {
            if input.surface != surface {
                return Err(Error::InputMismatch);
            }
            if let Some(current) = &self.background.current
                && current.snapshot.request.input != input
            {
                return Err(Error::InputMismatch);
            }
        }
        Ok(())
    }

    pub(crate) fn compose_background(
        &self,
        canvas: &mut NdCanvas,
        surface: crate::render::SurfaceInfo,
    ) -> Result<(), crate::graphics::GraphicsError> {
        if self.background.base.is_none() {
            return Ok(());
        }
        let mut prefix = NdCanvas::new();
        if let Some(current) = &self.background.current {
            prefix.push_state();
            prefix.clip_rect(0.0, 0.0, surface.logical_width, surface.logical_height);
            let [a, b, c, d, e, f] = current.snapshot.request.input.host_to_canvas.coeffs();
            prefix.set_transform(a, b, c, d, e, f);
            let coverage = current.snapshot.content.coverage;
            prefix.clip_rect(coverage.x, coverage.y, coverage.width, coverage.height);
            prefix.replay(&current.drawing)?;
            prefix.pop_state();
        }
        canvas.prepend_recording(prefix);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{geometry::Affine2D, host::BackgroundInputVersions, render::SurfaceInfo};

    #[test]
    fn exhausted_identity_does_not_replace_request_or_clear_installed_state() {
        let mut runtime = Runtime::empty();
        let input = BackgroundFrameInput {
            surface: SurfaceInfo::default(),
            versions: BackgroundInputVersions {
                surface: 1,
                position: 1,
                dpi: 1,
            },
            host_to_canvas: Affine2D::IDENTITY,
        };
        let source = HostBackgroundId::new();
        let request = runtime
            .begin_background(source, input, Color::WHITE)
            .unwrap();
        runtime.background.next_request = u64::MAX;
        assert_eq!(
            runtime.begin_background(source, input, Color::BLACK),
            Err(Error::Exhausted)
        );
        assert_eq!(runtime.background.request, Some(request));
        assert_eq!(runtime.background.base, Some(Color::WHITE));
        runtime.background.generation = u64::MAX;
        assert_eq!(
            runtime.remove_background(Color::BLACK),
            Err(Error::Exhausted)
        );
        assert_eq!(runtime.background.request, Some(request));
    }
}
