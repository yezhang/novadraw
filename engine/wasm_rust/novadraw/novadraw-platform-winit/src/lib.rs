//! Winit window, input, and host adapters for Novadraw applications.

mod host;
mod input;

pub use host::WinitPlatformHost;
pub use input::{AdaptedGesture, AdaptedKeyInput, WinitGestureAdapter, adapt_key_input};
