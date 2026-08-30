//! Platform host boundary.
//!
//! Host modules coordinate platform paint/update entry points without owning
//! graph state or runtime services.

mod scene_host;

pub use scene_host::{PlatformHost, SceneHost};
