//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! G1 provides application model identity/notification boundaries and model-only command history.
//! G2 adds EditPart topology and model-to-Figure projection without moving application state into
//! the Figure runtime. G3 adds Viewer selection, targeting, root layers, and input arbitration.
//! G4 adds typed editing requests, role-keyed policies, active Tools, and model Command execution.

#![deny(missing_docs)]

mod command;
mod domain;
mod feedback;
mod model;
mod part;
mod policy;
mod request;
mod selection;
mod tool;
mod viewer;

pub use command::{
    Command, CommandError, CommandOperation, CommandStack, CommandStackError, CommandStackEvent,
    CommandStackEventKind, CompoundCommand,
};
pub use domain::{DomainPointerRelease, EditorDomain, EditorDomainError};
pub use feedback::{FeedbackId, HandleId, HandleRole, VisualOwner};
pub use model::{ModelAdapter, ModelEvent, ModelRevision, ModelRevisionError};
pub use part::{
    EditPartBehavior, EditPartError, EditPartFactory, EditPartId, EditorNamespace,
    PartFactoryContext, PartNode, PartTree, PartTreeError, VisualBuildContext, VisualUpdateContext,
};
pub use policy::{
    EditPolicy, FeedbackVisual, PolicyError, PolicyHost, PolicyInstallation, PolicyRole,
};
pub use request::{
    ChangeBoundsKind, ChangeBoundsRequest, CreateRequest, CreationType, CreationTypeError,
    DeleteRequest, EditorRequest, InteractionRevision, InteractionRevisionError, RequestModifiers,
    ResizeDirection,
};
pub use selection::{SelectionDelta, SelectionModel};
pub use tool::{SelectionTool, ToolError, ToolRelease};
pub use viewer::{
    GraphicalViewer, RootLayers, SelectionMode, ViewerError, ViewerInputOutcome, ViewerTarget,
};
