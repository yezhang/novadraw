//! Browser input and host adapters for Novadraw applications.

#[cfg(all(feature = "editor", target_arch = "wasm32"))]
mod dom_text_input;
mod host;
mod input;
#[cfg(feature = "editor")]
mod text_input;

#[cfg(all(feature = "editor", target_arch = "wasm32"))]
pub use dom_text_input::WebTextInputHost;
pub use host::WebPlatformHost;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WebInputAdapter, WebPointerInput, WebWheelDeltaMode,
    adapt_key_input,
};
#[cfg(feature = "editor")]
pub use text_input::{WebTextInputAction, WebTextInputBridge};
