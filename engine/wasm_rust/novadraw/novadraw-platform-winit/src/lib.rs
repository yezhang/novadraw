//! Winit window, input, and host adapters for Novadraw applications.

mod host;
mod input;
#[cfg(feature = "editor")]
mod text_input;

pub use host::WinitPlatformHost;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, adapt_key_input, adapt_modifiers,
    adapt_mouse_button, adapt_physical_key,
};
#[cfg(feature = "editor")]
pub use text_input::WinitTextInputBridge;
