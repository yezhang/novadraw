use crate::{
    FigureId,
    connection::{ConnectionId, ConnectionRuntimeError, CoordinateSpace},
};

/// A failed publication barrier retains the preparing owner and required connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationRouteError {
    pub owner: FigureId,
    pub connection: Option<ConnectionId>,
    pub kind: PreparationRouteErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationRouteErrorKind {
    InvalidScope(CoordinateSpace),
    DuplicateMember,
    OutsideScope,
    IncompleteGroup,
    Unresolved,
    SnapshotMismatch,
    Connection(Box<ConnectionRuntimeError>),
}

impl std::fmt::Display for PreparationRouteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "preparation {:?} required route {:?} failed: {:?}",
            self.owner, self.connection, self.kind
        )
    }
}

impl std::error::Error for PreparationRouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            PreparationRouteErrorKind::Connection(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RequiredRouteGroup {
    pub scope: CoordinateSpace,
    pub connections: Vec<ConnectionId>,
}
