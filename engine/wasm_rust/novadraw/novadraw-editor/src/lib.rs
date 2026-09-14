//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! G1 provides application model identity/notification boundaries and model-only command history.
//! G2 adds EditPart topology and model-to-Figure projection without moving application state into
//! the Figure runtime. G3 adds Viewer selection, targeting, root layers, and input arbitration.
//! G4 adds typed editing requests, role-keyed policies, active Tools, and model Command execution.
//! G5.1 adds ordered connection-model projection and Runtime routing bindings. G5.2 adds
//! source-locked, two-stage connection creation through model Commands.

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
pub use model::{ModelAdapter, ModelConnection, ModelEvent, ModelRevision, ModelRevisionError};
pub use part::{
    ConnectionEndpoints, ConnectionPartFactoryContext, ConnectionPartId, EditPartBehavior,
    EditPartError, EditPartFactory, EditPartId, EditorNamespace, PartFactoryContext, PartKind,
    PartNode, PartTree, PartTreeError, VisualBuildContext, VisualUpdateContext,
};
pub use policy::{
    ConnectionCreation, EditPolicy, FeedbackVisual, PolicyError, PolicyHost, PolicyInstallation,
    PolicyRole,
};
pub use request::{
    ChangeBoundsKind, ChangeBoundsRequest, CreateConnectionRequest, CreateRequest, CreationType,
    CreationTypeError, DeleteRequest, EditorRequest, InteractionRevision, InteractionRevisionError,
    RequestModifiers, ResizeDirection,
};
pub use selection::{SelectionDelta, SelectionModel};
pub use tool::{
    ConnectionCreationTool, ConnectionToolPress, SelectionTool, ToolError, ToolRelease,
};
pub use viewer::{
    GraphicalViewer, RootLayers, SelectionMode, ViewerError, ViewerInputOutcome, ViewerTarget,
};
