//! Typed, model-independent editing intent.

use std::{error::Error, fmt, sync::Arc};

use novadraw_geometry::{Dimension, Point, Rectangle, Vec2};

use crate::{ConnectionPartId, EditPartId};

/// Monotonic identity of one interaction update.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteractionRevision(u64);

impl InteractionRevision {
    /// Returns the first valid interaction revision.
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Creates a non-zero revision.
    pub const fn new(value: u64) -> Result<Self, InteractionRevisionError> {
        if value == 0 {
            Err(InteractionRevisionError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the raw revision value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Advances without wrapping.
    pub const fn next(self) -> Result<Self, InteractionRevisionError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(InteractionRevisionError::Exhausted),
        }
    }
}

/// Failure to construct or advance an interaction revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionRevisionError {
    /// Revision zero is reserved.
    Zero,
    /// Revision identity space is exhausted.
    Exhausted,
}

impl fmt::Display for InteractionRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("interaction revision must be non-zero"),
            Self::Exhausted => formatter.write_str("interaction revision space is exhausted"),
        }
    }
}

impl Error for InteractionRevisionError {}

/// Platform-independent modifier snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestModifiers {
    /// Shift is pressed.
    pub shift: bool,
    /// Control is pressed.
    pub control: bool,
    /// Alt/Option is pressed.
    pub alt: bool,
    /// Meta/Command is pressed.
    pub meta: bool,
}

/// Edge or corner used by a resize gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeDirection {
    /// Top edge.
    North,
    /// Top-right corner.
    NorthEast,
    /// Right edge.
    East,
    /// Bottom-right corner.
    SouthEast,
    /// Bottom edge.
    South,
    /// Bottom-left corner.
    SouthWest,
    /// Left edge.
    West,
    /// Top-left corner.
    NorthWest,
}

/// Endpoint of a connection manipulated by a reconnect gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionEndpoint {
    /// Move the source endpoint while preserving the target.
    Source,
    /// Move the target endpoint while preserving the source.
    Target,
}

/// Mutation applied to an ordered connection bendpoint list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BendpointOperation {
    /// Insert a bendpoint at the segment's insertion index.
    Create {
        /// Insertion index in the ordered bendpoint list.
        index: usize,
    },
    /// Move an existing bendpoint.
    Move {
        /// Index in the ordered bendpoint list.
        index: usize,
    },
    /// Remove an existing bendpoint.
    Delete {
        /// Index in the ordered bendpoint list.
        index: usize,
    },
}

/// Typed intent to mutate one connection bendpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct BendpointRequest {
    connection: ConnectionPartId,
    operation: BendpointOperation,
    location: Point,
    modifiers: RequestModifiers,
    revision: InteractionRevision,
}

impl BendpointRequest {
    /// Creates a bendpoint request.
    pub fn new(
        connection: ConnectionPartId,
        operation: BendpointOperation,
        location: Point,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            connection,
            operation,
            location,
            modifiers,
            revision,
        }
    }

    /// Returns the owning connection.
    pub const fn connection(&self) -> ConnectionPartId {
        self.connection
    }

    /// Returns the bendpoint mutation.
    pub const fn operation(&self) -> BendpointOperation {
        self.operation
    }

    /// Returns the latest pointer location in the Connection routing domain.
    pub const fn location(&self) -> Point {
        self.location
    }

    /// Returns the modifier snapshot.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }
}

/// Bounds operation requested for selected parts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeBoundsKind {
    /// Move without changing size.
    Move,
    /// Resize from one edge or corner.
    Resize(ResizeDirection),
}

/// Move or resize intent for an ordered set of EditParts.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeBoundsRequest {
    kind: ChangeBoundsKind,
    parts: Vec<EditPartId>,
    location: Point,
    move_delta: Vec2,
    size_delta: Dimension,
    target_candidate: Option<EditPartId>,
    modifiers: RequestModifiers,
    revision: InteractionRevision,
}

impl ChangeBoundsRequest {
    /// Creates a move request.
    pub fn moving(
        parts: Vec<EditPartId>,
        location: Point,
        move_delta: Vec2,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            kind: ChangeBoundsKind::Move,
            parts,
            location,
            move_delta,
            size_delta: Dimension::ZERO,
            target_candidate: None,
            modifiers,
            revision,
        }
    }

    /// Creates a resize request.
    pub fn resizing(
        parts: Vec<EditPartId>,
        location: Point,
        move_delta: Vec2,
        size_delta: Dimension,
        direction: ResizeDirection,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            kind: ChangeBoundsKind::Resize(direction),
            parts,
            location,
            move_delta,
            size_delta,
            target_candidate: None,
            modifiers,
            revision,
        }
    }

    /// Returns the bounds operation.
    pub const fn kind(&self) -> ChangeBoundsKind {
        self.kind
    }

    /// Returns source parts in deterministic command-contribution order.
    pub fn parts(&self) -> &[EditPartId] {
        &self.parts
    }

    /// Returns the latest pointer location in model-content coordinates.
    pub const fn location(&self) -> Point {
        self.location
    }

    /// Returns the model-content translation applied before resize.
    pub const fn move_delta(&self) -> Vec2 {
        self.move_delta
    }

    /// Returns the width and height delta in model-content units.
    pub const fn size_delta(&self) -> Dimension {
        self.size_delta
    }

    /// Returns the latest target candidate resolved during the gesture.
    pub const fn target_candidate(&self) -> Option<EditPartId> {
        self.target_candidate
    }

    /// Sets the latest target candidate.
    pub fn with_target_candidate(mut self, target: Option<EditPartId>) -> Self {
        self.target_candidate = target;
        self
    }

    /// Returns the modifier snapshot.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }

    /// Applies this request to source bounds.
    pub fn transformed_bounds(&self, source: Rectangle) -> Rectangle {
        Rectangle::new(
            source.x + self.move_delta.x(),
            source.y + self.move_delta.y(),
            source.width + self.size_delta.width,
            source.height + self.size_delta.height,
        )
    }
}

/// Stable application-defined creation type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CreationType(Arc<str>);

impl CreationType {
    /// Creates a non-empty creation type.
    pub fn new(value: impl AsRef<str>) -> Result<Self, CreationTypeError> {
        let value = value.as_ref();
        if value.is_empty() {
            Err(CreationTypeError)
        } else {
            Ok(Self(Arc::from(value)))
        }
    }

    /// Returns the application-defined type string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Empty creation types are invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreationTypeError;

impl fmt::Display for CreationTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("creation type must not be empty")
    }
}

impl Error for CreationTypeError {}

/// Intent to create one model child.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateRequest {
    parent: EditPartId,
    creation_type: CreationType,
    bounds: Rectangle,
    modifiers: RequestModifiers,
    revision: InteractionRevision,
}

impl CreateRequest {
    /// Creates a typed creation request.
    pub fn new(
        parent: EditPartId,
        creation_type: CreationType,
        bounds: Rectangle,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            parent,
            creation_type,
            bounds,
            modifiers,
            revision,
        }
    }

    /// Returns the target parent part.
    pub const fn parent(&self) -> EditPartId {
        self.parent
    }

    /// Returns the application-defined creation type.
    pub const fn creation_type(&self) -> &CreationType {
        &self.creation_type
    }

    /// Returns requested bounds in the target parent model domain.
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the modifier snapshot.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }
}

/// Intent to delete an ordered set of model-backed parts.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteRequest {
    parts: Vec<EditPartId>,
    revision: InteractionRevision,
}

impl DeleteRequest {
    /// Creates a delete request.
    pub fn new(parts: Vec<EditPartId>, revision: InteractionRevision) -> Self {
        Self { parts, revision }
    }

    /// Returns parts in deterministic contribution order.
    pub fn parts(&self) -> &[EditPartId] {
        &self.parts
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }
}

/// Two-stage intent to create one model connection.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateConnectionRequest {
    connection_type: CreationType,
    source: EditPartId,
    target_candidate: Option<EditPartId>,
    location: Point,
    modifiers: RequestModifiers,
    revision: InteractionRevision,
}

/// Typed intent to reconnect one endpoint of an existing connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ReconnectConnectionRequest {
    connection: ConnectionPartId,
    endpoint: ConnectionEndpoint,
    target_candidate: Option<EditPartId>,
    location: Point,
    modifiers: RequestModifiers,
    revision: InteractionRevision,
}

impl ReconnectConnectionRequest {
    /// Creates a source-locked reconnect request.
    pub fn new(
        connection: ConnectionPartId,
        endpoint: ConnectionEndpoint,
        location: Point,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            connection,
            endpoint,
            target_candidate: None,
            location,
            modifiers,
            revision,
        }
    }

    /// Returns the connection whose endpoint is moving.
    pub const fn connection(&self) -> ConnectionPartId {
        self.connection
    }

    /// Returns the moving endpoint.
    pub const fn endpoint(&self) -> ConnectionEndpoint {
        self.endpoint
    }

    /// Returns the latest endpoint candidate.
    pub const fn target_candidate(&self) -> Option<EditPartId> {
        self.target_candidate
    }

    /// Replaces the latest endpoint candidate.
    pub fn with_target_candidate(mut self, target: Option<EditPartId>) -> Self {
        self.target_candidate = target;
        self
    }

    /// Returns the latest pointer location in the Connection routing domain.
    pub const fn location(&self) -> Point {
        self.location
    }

    /// Returns the modifiers captured when dragging started.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }
}

impl CreateConnectionRequest {
    /// Creates a source-locked connection request.
    pub fn new(
        connection_type: CreationType,
        source: EditPartId,
        location: Point,
        modifiers: RequestModifiers,
        revision: InteractionRevision,
    ) -> Self {
        Self {
            connection_type,
            source,
            target_candidate: None,
            location,
            modifiers,
            revision,
        }
    }

    /// Returns the application-defined connection type.
    pub const fn connection_type(&self) -> &CreationType {
        &self.connection_type
    }

    /// Returns the source Part locked by the first stage.
    pub const fn source(&self) -> EditPartId {
        self.source
    }

    /// Returns the latest target candidate, if any.
    pub const fn target_candidate(&self) -> Option<EditPartId> {
        self.target_candidate
    }

    /// Sets the latest target candidate without changing the locked source.
    pub fn with_target_candidate(mut self, target: Option<EditPartId>) -> Self {
        self.target_candidate = target;
        self
    }

    /// Returns the latest pointer location in the Connection routing domain.
    pub const fn location(&self) -> Point {
        self.location
    }

    /// Returns the modifier snapshot captured when the gesture started.
    pub const fn modifiers(&self) -> RequestModifiers {
        self.modifiers
    }

    /// Returns the interaction revision.
    pub const fn revision(&self) -> InteractionRevision {
        self.revision
    }
}

/// Closed set of editing requests implemented by the first editor vertical slice.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorRequest {
    /// Move or resize existing parts.
    ChangeBounds(ChangeBoundsRequest),
    /// Create one child.
    Create(CreateRequest),
    /// Delete existing parts.
    Delete(DeleteRequest),
    /// Create one model connection through a source-locked two-stage gesture.
    CreateConnection(CreateConnectionRequest),
    /// Move one endpoint of an existing connection.
    ReconnectConnection(ReconnectConnectionRequest),
    /// Create, move, or delete one connection bendpoint.
    Bendpoint(BendpointRequest),
}

impl EditorRequest {
    /// Returns a stable command label for this request kind.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ChangeBounds(request) => match request.kind() {
                ChangeBoundsKind::Move => "Move",
                ChangeBoundsKind::Resize(_) => "Resize",
            },
            Self::Create(_) => "Create",
            Self::Delete(_) => "Delete",
            Self::CreateConnection(_) => "Create connection",
            Self::ReconnectConnection(_) => "Reconnect connection",
            Self::Bendpoint(_) => "Edit bendpoint",
        }
    }

    /// Returns the interaction revision shared by every request kind.
    pub const fn revision(&self) -> InteractionRevision {
        match self {
            Self::ChangeBounds(request) => request.revision(),
            Self::Create(request) => request.revision(),
            Self::Delete(request) => request.revision(),
            Self::CreateConnection(request) => request.revision(),
            Self::ReconnectConnection(request) => request.revision(),
            Self::Bendpoint(request) => request.revision(),
        }
    }

    /// Returns source parts that contribute commands.
    pub fn source_parts(&self) -> &[EditPartId] {
        match self {
            Self::ChangeBounds(request) => request.parts(),
            Self::Create(request) => std::slice::from_ref(&request.parent),
            Self::Delete(request) => request.parts(),
            Self::CreateConnection(request) => std::slice::from_ref(&request.source),
            Self::ReconnectConnection(request) => {
                std::slice::from_ref(request.connection.edit_part_ref())
            }
            Self::Bendpoint(request) => std::slice::from_ref(request.connection.edit_part_ref()),
        }
    }
}
