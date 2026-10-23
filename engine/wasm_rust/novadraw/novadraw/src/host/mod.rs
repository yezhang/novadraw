//! Platform host boundary.
//!
//! Host modules coordinate platform paint/update entry points without owning
//! graph state or runtime services.

mod background;
mod scene_host;

pub use background::{
    BackgroundFrameInput, BackgroundInputVersions, HostBackgroundContent, HostBackgroundError,
    HostBackgroundId, HostBackgroundProvider, HostBackgroundProviderKey, HostBackgroundRegistry,
    HostBackgroundRequest, HostBackgroundSnapshot, RECORDED_BACKGROUND,
};
pub(crate) use background::{BackgroundState, InstalledBackground};
pub use scene_host::{HeadlessHost, ImeState, PlatformHost};
