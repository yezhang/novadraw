//! Figure tree construction, immutable queries, and search.

mod subtree;
pub(crate) use subtree::SubtreeNode;
pub use subtree::{FigureSubtree, SubtreeNodeError, SubtreeNodeId, SubtreeReceipt};

pub use crate::graph::{
    ChildInsertionError, CoordinateFailure, CoordinateQueryError, DEFAULT_VALIDATION_BUDGET,
    ExclusionSearch, FREEFORM_EXTENT_PROPERTY, FigureId, FigureTree, FigureTreeBuilder,
    FreeformError, FreeformState, GraphMutationError, IdentitySearch, MAX_TREE_DEPTH,
    TreeQueryError, TreeSearch, TreeSearchContext, ValidationError,
};
