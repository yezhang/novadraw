//! Viewer contents, root layers, registries, targeting, selection, and EditPart focus.
//!
//! Viewer-owned selection remains independent from Figure state. Visual targeting will use
//! FigureTree hit-testing followed by ancestor lookup in the visual registry.
