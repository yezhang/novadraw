//! Model-driven editing framework built on Novadraw's Figure runtime.
//!
//! G1 provides application model identity/notification boundaries and model-only command history.
//! G2 adds EditPart topology and model-to-Figure projection without moving application state into
//! the Figure runtime. G3 adds Viewer selection, targeting, root layers, and input arbitration.
//! G4 adds open typed requests, policy registries, active Tools, and model Command execution.
//! G5.1 adds ordered connection-model projection and Runtime routing bindings. G5.2 adds
//! source-locked, two-stage connection creation through model Commands.

#![deny(missing_docs)]

mod autoexpose;
pub mod command;
pub mod container;
mod direct_edit;
mod domain;
pub mod model;
#[path = "feedback/mod.rs"]
pub mod overlay;
mod part;
pub mod policy;
mod request;
mod selection;
pub mod text_input;
pub mod tool;
pub mod viewer;

pub use autoexpose::AutoexposeTick;
pub use command::{
    Command, CommandError, CommandOperation, CommandStack, CommandStackError, CommandStackEvent,
    CommandStackEventKind, CompoundCommand,
};
pub use direct_edit::{
    DirectTextComposition, DirectTextEdit, DirectTextEditDescriptor, DirectTextEditError,
    DirectTextEditRequest, DirectTextEditSessionId, DirectTextEditState, DirectTextFeature,
    DirectTextFeatureError, DirectTextFeedback, ExtendTextSelection, FocusLossPolicy, TextDelete,
    TextEditMode,
};
pub use domain::{DomainPointerRelease, EditorDomain, EditorDomainError};
pub use model::{ModelAdapter, ModelConnection, ModelEvent, ModelRevision, ModelRevisionError};
pub(crate) use overlay::{
    BendpointCreateAction, BendpointHandleSite, BendpointMoveAction, ConnectionEndpointAction,
    FeedbackId, HandleAction, HandleActionProtocol, HandleId, ResizeAction, SelectionAction,
    VisualOwner,
};
pub use part::{
    ConnectionAnchorContext, ConnectionAnchorDescriptor, ConnectionEndpoints,
    ConnectionPartFactoryContext, ConnectionPartId, ConnectionRouterKey,
    ConnectionRouterRegistration, ConnectionRouterSelection, ConnectionRoutingDescriptor,
    EditPartBehavior, EditPartError, EditPartFactory, EditPartId, EditorNamespace,
    PartFactoryContext, PartKind, PartNode, PartTree, PartTreeError, VisualBuildContext,
    VisualUpdateContext,
};
pub use policy::{
    ComponentSlot, ConnectionCreation, ConnectionCreationSlot, ConnectionFeedbackRoute,
    ConnectionReconnectSlot, ConnectionReconnection, DirectTextEditSlot, EditPolicy,
    EditingContribution, FeedbackVisual, LayoutSlot, PolicyError, PolicyHandler,
    PolicyHandlerBuilder, PolicyHost, PolicyInstallation, PolicyKey, PolicySlot, PolicyTarget,
    PrimaryDragSlot, SelectionFeedbackSlot,
};
pub use request::{
    BendpointOperation, BendpointRequest, ChangeBounds, ChangeBoundsKind, ChangeBoundsRequest,
    ConnectionEndpoint, ContributionLimit, CreateChild, CreateConnection, CreateConnectionRequest,
    CreateRequest, CreationType, CreationTypeError, DeleteParts, DeleteRequest, EditBendpoint,
    InteractionRevision, InteractionRevisionError, ReconnectConnection, ReconnectConnectionRequest,
    RequestKey, RequestMetadata, RequestModifiers, RequestPayload, RequestProtocol,
    RequestResolution, ResizeDirection, StartDirectTextEdit,
};
pub use selection::{SelectionDelta, SelectionModel};
pub use text_input::{
    SessionTextInputEvent, TextInputEffect, TextInputEvent, TextInputPurpose, TextInputSnapshot,
};
pub use tool::{
    ConnectionBendpointTool, ConnectionCreationTool, ConnectionEndpointTool, ErasedToolKey,
    SelectionTool, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent,
};
pub use viewer::{
    GraphicalViewer, ProjectionCleanup, ProjectionCleanupFailure, RootLayers, SelectionMode,
    ViewerDrive, ViewerError, ViewerInputOutcome, ViewerTarget,
};
