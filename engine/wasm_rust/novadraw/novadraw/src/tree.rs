//! Figure tree construction, immutable queries, and search.

pub use crate::graph::{
    ChildInsertionError, DEFAULT_VALIDATION_BUDGET, ExclusionSearch, FREEFORM_EXTENT_PROPERTY,
    FigureId, FigureTree, FigureTreeBuilder, FreeformError, FreeformState, GraphMutationError,
    IdentitySearch, MAX_TREE_DEPTH, TreeQueryError, TreeSearch, TreeSearchContext, ValidationError,
};
