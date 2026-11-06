//! Locally maintained workbench controls.
//!
//! Applications own layouts, themes, assets and document commands. Docking renders an
//! application's panel identities and returns actions for a validated layout transaction.
//! This crate depends on neither Novadraw Core nor Editor.
//!
//! Derived from craft-libs; see `SOURCE.md` and `NOTICE` in this package.

pub mod buttons;
pub mod capture;
pub mod combo;
pub mod docking;
mod focus;
pub mod ime;
pub mod layout;
pub mod panes;
pub mod tabs;
pub mod text;
pub mod theme;

pub use combo::ComboBox;
