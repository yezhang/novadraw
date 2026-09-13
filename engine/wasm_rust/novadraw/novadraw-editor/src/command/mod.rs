//! Model-only commands and undo/redo history.
//!
//! Commands may retain stable application model identity and owned model data. They must not
//! retain live EditPart or Figure handles because views are rebuilt after model restoration.
