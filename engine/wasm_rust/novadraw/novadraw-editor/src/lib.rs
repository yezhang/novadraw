//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! This crate is intentionally at the G0 architecture stage. Its module boundaries are present,
//! but no editing behavior is exported until the corresponding contracts have executable tests.

#![deny(missing_docs)]

mod command;
mod domain;
mod feedback;
mod model;
mod part;
mod policy;
mod request;
mod tool;
mod viewer;
