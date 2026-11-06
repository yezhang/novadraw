//! Locally maintained workbench controls.
//!
//! Applications own layouts, themes, assets and document commands. Docking renders an
//! application's panel identities and returns actions for a validated layout transaction.
//! Optional `canvas` and `editor-canvas` features add one-way engine integration.
//!
//! Derived from craft-libs; see `SOURCE.md` and `NOTICE` in this package.

pub mod buttons;
#[cfg(feature = "canvas")]
pub mod canvas;
pub mod capture;
pub mod combo;
pub mod docking;
#[cfg(feature = "editor-canvas")]
pub mod editor_canvas;
mod focus;
pub mod ime;
pub mod layout;
pub mod panes;
pub mod tabs;
pub mod text;
pub mod theme;

pub use combo::ComboBox;
