//! Active editing tools and gesture-scoped trackers.
//!
//! Tools interpret normalized input as Requests. They do not mutate Figures or application models
//! directly and must clear transient feedback before executing a Command.
