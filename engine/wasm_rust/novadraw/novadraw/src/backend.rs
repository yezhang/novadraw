//! Optional rendering backend implementations.

/// Vello renderer enabled by `native-vello` or `web-vello`.
pub mod vello {
    pub use novadraw_render::backend::vello::VelloRenderer;
}
