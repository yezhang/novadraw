//! Runtime services around the Figure tree.
//!
//! [`Runtime`] is the preferred composition root. It owns one tree together
//! with interaction, deferred mutation, and update state.

pub mod context;
pub mod event;
pub mod mutation;
// `runtime::Runtime` is the deliberate public domain name.
#[allow(clippy::module_inception)]
pub mod runtime;
pub mod system;
pub mod update;

pub use runtime::Runtime;
