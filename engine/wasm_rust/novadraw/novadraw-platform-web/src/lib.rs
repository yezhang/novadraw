//! Browser input and host adapters for Novadraw applications.

mod host;
mod input;

pub use host::WebPlatformHost;
pub use input::{
    AdaptedGesture, AdaptedKeyInput, WebInputAdapter, WebPointerInput, WebWheelDeltaMode,
    adapt_key_input,
};
