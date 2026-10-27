//! Independent canvas regions for one winit window.
//!
//! The host owns input arbitration and coordinates. Renderers remain external.
//! No event loop, GPU device or application scene is owned by this crate.

pub mod geometry;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod host;
