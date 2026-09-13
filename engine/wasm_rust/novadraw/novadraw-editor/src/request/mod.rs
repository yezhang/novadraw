//! Typed, model-independent editing intent.
//!
//! Requests describe selection, bounds changes, creation, deletion, and connection editing. They
//! do not mutate application models and do not use an unbounded dynamic map as their main API.
