//! Full-frame rendering into an application-owned logical-coordinate Vello Scene.
//!
//! Use one renderer per Runtime. The host owns GPU rendering, composition, and
//! presentation. `Presented` acknowledges publication to the Scene, not the window.

use crate::SceneEncoder;
use novadraw::geometry::Rectangle;
use novadraw::render::{
    BackendCapabilities, BackendSessionDecision, BackendSessionGate, DamageMode, RenderBackend,
    RenderDisposition, RenderFailure, RenderFailureDiagnostic, RenderResult, RenderSubmission,
};
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};

/// Scene type of the Vello version used by this backend.
pub use vello::Scene;

/// Resource-aware encoder without a window, GPU device, or event loop.
pub struct VelloSceneRenderer {
    encoder: SceneEncoder,
    session_gate: BackendSessionGate,
}

impl Default for VelloSceneRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VelloSceneRenderer {
    /// Creates an encoder with independent resource and session state.
    pub fn new() -> Self {
        Self {
            encoder: SceneEncoder::new(1.0),
            session_gate: BackendSessionGate::default(),
        }
    }

    /// Sets per-operation preparation budgets and discards coverage caches.
    pub fn set_antialias_config(&mut self, config: crate::AntialiasConfig) {
        self.encoder.antialias_config = config;
        self.encoder.antialias_cache = crate::antialias_frame::Cache::default();
    }

    /// Binds a logical-coordinate target, replaced only after successful encoding.
    ///
    /// The host applies the submission's DPI once when rendering the target.
    /// Arbitrary additional host scaling is not certified for device-pixel OFF AA.
    pub fn bind<'a>(&'a mut self, target: &'a mut Scene) -> SceneBackend<'a> {
        SceneBackend {
            renderer: self,
            target,
        }
    }
}

/// Scoped backend publishing into a caller-owned Scene.
pub struct SceneBackend<'a> {
    renderer: &'a mut VelloSceneRenderer,
    target: &'a mut Scene,
}

impl RenderBackend for SceneBackend<'_> {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
            .with_glyph_runs()
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
            .with_host_background()
            .with_per_operation_antialias()
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        let capabilities = self.capabilities();
        submission.validate_composition(capabilities)?;
        capabilities
            .validate_capabilities(&submission.commands)
            .map_err(RenderFailure::Unsupported)?;
        novadraw::render::validate_graphics_input(
            &submission.commands,
            submission.surface.scale_factor,
        )
        .map_err(RenderFailure::InvalidInput)?;
        let surface = submission.surface;
        if surface.pixel_width == 0 || surface.pixel_height == 0 {
            return Ok(RenderDisposition::Skipped);
        }
        if !surface.is_renderable() || !matches!(submission.damage.mode(), DamageMode::Full) {
            return Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                "Scene backend requires a renderable surface and full-frame damage",
            )));
        }
        let inverse_scale = surface.scale_factor.recip();
        let clip = Rectangle::new(
            0.0,
            0.0,
            f64::from(surface.pixel_width) * inverse_scale,
            f64::from(surface.pixel_height) * inverse_scale,
        );
        if !(inverse_scale as f32).is_finite()
            || inverse_scale as f32 == 0.0
            || !(clip.width as f32).is_finite()
            || !(clip.height as f32).is_finite()
        {
            return Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                "Scene target extent or inverse DPI exceeds portable coordinate limits",
            )));
        }
        let renderer = &mut self.renderer;
        let mut gate = renderer.session_gate;
        let decision = gate.accept(submission.session_id, &submission.resources);
        if matches!(
            decision,
            BackendSessionDecision::RejectStale | BackendSessionDecision::RejectMissingSnapshot
        ) {
            return Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                "invalid backend submission session",
            )));
        }
        let Some(prepared) = crate::antialias_frame::prepare(
            &renderer.encoder,
            submission,
            decision == BackendSessionDecision::Continue,
        )?
        else {
            return Ok(RenderDisposition::Retry);
        };
        renderer
            .encoder
            .sync_resources(&mut renderer.session_gate, submission)?;
        if !renderer
            .encoder
            .has_required_resources(&submission.commands)
        {
            return Ok(RenderDisposition::Retry);
        }
        renderer
            .encoder
            .encode(&submission.commands, clip, surface.scale_factor, prepared);
        let mut candidate = Scene::new();
        if let Some(color) = submission.clear_color {
            candidate.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::new([
                    color.red() as f32,
                    color.green() as f32,
                    color.blue() as f32,
                    color.alpha() as f32,
                ]),
                None,
                &Rect::new(0.0, 0.0, clip.width, clip.height),
            );
        }
        candidate.append(&renderer.encoder.scene, Some(Affine::scale(inverse_scale)));
        *self.target = candidate;
        Ok(RenderDisposition::Presented)
    }

    fn resize(&mut self, _: u32, _: u32, _: f64) {
        // The next full submission defines the target extent and DPI.
    }
}
