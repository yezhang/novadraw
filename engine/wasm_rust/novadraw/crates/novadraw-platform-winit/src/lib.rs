//! Winit window, input, and host adapters for Novadraw applications.

#[cfg(feature = "canvas")]
pub mod canvas;
mod host;
mod input;
mod pointer;
mod resource;
#[cfg(feature = "editor")]
mod text_input;

pub use host::WinitPlatformHost;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, adapt_key_event, adapt_key_input,
    adapt_modifiers, adapt_mouse_button, adapt_physical_key,
};
pub use pointer::{PointerInputError, WinitPointerAdapter};
pub use resource::FileResourceProvider;
#[cfg(feature = "editor")]
pub use text_input::WinitTextInputBridge;
