//! Editor session coordination.
//!
//! The future domain owns Command history, the active Tool, and one or more Viewers. It remains
//! platform independent and does not own winit, DOM, AppKit, or a concrete render backend.
