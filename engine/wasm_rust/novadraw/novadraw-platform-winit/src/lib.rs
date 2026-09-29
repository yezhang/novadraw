//! Winit window, input, and host adapters for Novadraw applications.

mod host;
mod input;
#[cfg(feature = "editor")]
mod text_input;

pub use host::WinitPlatformHost;
pub use input::{AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, adapt_key_input};
#[cfg(feature = "editor")]
pub use text_input::WinitTextInputBridge;
