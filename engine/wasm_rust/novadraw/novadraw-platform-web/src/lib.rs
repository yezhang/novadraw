//! Browser input and host adapters for Novadraw applications.

#[cfg(all(feature = "editor", target_arch = "wasm32"))]
mod dom_text_input;
#[cfg(all(feature = "editor", target_arch = "wasm32"))]
mod edit_context;
mod host;
mod input;
#[cfg(all(feature = "resource", target_arch = "wasm32"))]
mod resource;
#[cfg(feature = "editor")]
mod text_input;

#[cfg(all(feature = "editor", target_arch = "wasm32"))]
pub use dom_text_input::WebTextInputHost;
#[cfg(all(feature = "editor", target_arch = "wasm32"))]
pub use edit_context::WebEditContextHost;
pub use host::WebPlatformHost;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WebInputAdapter, WebPointerInput, WebWheelDeltaMode,
    adapt_key_input, adapt_pointer_button,
};
#[cfg(all(feature = "resource", target_arch = "wasm32"))]
pub use resource::{FetchResourceProvider, WebResourceError};
#[cfg(feature = "editor")]
pub use text_input::{
    WebEditContextBridge, WebTextInputAction, WebTextInputBridge, WebTextOffsetError,
};
