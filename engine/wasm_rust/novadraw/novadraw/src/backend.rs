//! Optional rendering backend implementations.

/// Vello renderer enabled by `native-vello` or `web-vello`.
pub mod vello {
    #[cfg(all(feature = "native-vello", not(target_arch = "wasm32")))]
    pub use novadraw_render::backend::vello::NativeWindow;
    pub use novadraw_render::backend::vello::{VelloInitializationError, VelloRenderer};
}
