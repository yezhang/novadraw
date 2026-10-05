use std::{collections::VecDeque, error::Error, fmt};

use crate::connection::ConnectionRuntimeError;
use crate::geometry::{Dimension, Rectangle};
use crate::{
    ChildClippingStrategy, Figure, FigureId, GraphMutationError, LayerKey, LayerPlacement,
    LayoutConstraint, LayoutError, LayoutManager,
};

use super::runtime::Runtime;

pub trait FigureComponentUpdate {
    type Figure: Figure + 'static;
    type Prepared;
    type Error;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error>;

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigureComponentContext {
    pub figure_id: FigureId,
    pub component_revision: u64,
    pub bounds: crate::geometry::Rectangle,
}

#[derive(Debug)]
pub struct PreparedFigureUpdate<T> {
    pub(crate) value: T,
    pub(crate) invalidation: ComponentInvalidation,
}

impl<T> PreparedFigureUpdate<T> {
    pub fn new(value: T) -> Self {
        Self::layout_geometry_and_paint(value)
    }

    pub fn paint(value: T) -> Self {
        Self {
            value,
            invalidation: ComponentInvalidation::Paint,
        }
    }

    pub fn geometry_and_paint(value: T) -> Self {
        Self {
            value,
            invalidation: ComponentInvalidation::GeometryAndPaint,
        }
    }

    pub fn layout_geometry_and_paint(value: T) -> Self {
        Self {
            value,
            invalidation: ComponentInvalidation::LayoutGeometryAndPaint,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentInvalidation {
    Paint,
    GeometryAndPaint,
    LayoutGeometryAndPaint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentUpdateReceipt {
    pub figure: FigureId,
    pub previous_revision: u64,
    pub revision: u64,
    pub invalidation: ComponentInvalidation,
}

#[derive(Debug, PartialEq)]
pub enum ComponentUpdateError<E> {
    Runtime(RuntimeMutationError),
    WrongFigureType {
        figure: FigureId,
        expected: &'static str,
        actual: &'static str,
    },
    RevisionExhausted(FigureId),
    Rejected(E),
}

impl<E> From<RuntimeMutationError> for ComponentUpdateError<E> {
    fn from(value: RuntimeMutationError) -> Self {
        Self::Runtime(value)
    }
}

impl<E: fmt::Display> fmt::Display for ComponentUpdateError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => error.fmt(formatter),
            Self::WrongFigureType {
                figure,
                expected,
                actual,
            } => write!(
                formatter,
                "Figure {figure:?} has type {actual}, expected {expected}"
            ),
            Self::RevisionExhausted(figure) => {
                write!(
                    formatter,
                    "component revision is exhausted for Figure {figure:?}"
                )
            }
            Self::Rejected(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for ComponentUpdateError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Rejected(error) => Some(error),
            Self::WrongFigureType { .. } | Self::RevisionExhausted(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeMutationError {
    ForeignRuntime(FigureId),
    Faulted,
    UnknownOrDisposedFigure(FigureId),
    DetachedFigure(FigureId),
    SyntheticRootOperation(FigureId),
    WrongCapability {
        figure: FigureId,
        capability: &'static str,
    },
    WrongComponentType {
        figure: FigureId,
        expected: &'static str,
        actual: &'static str,
    },
    ComponentRevisionExhausted(FigureId),
    ComponentUpdateRejected(FigureId),
    InvalidParentRelation {
        parent: FigureId,
        child: FigureId,
    },
    LayeredParent(FigureId),
    InvalidChildIndex {
        parent: FigureId,
        index: usize,
        child_count: usize,
    },
    /// A complete child order was not an exact permutation of the current direct children.
    InvalidChildOrder {
        /// Parent whose child order was rejected.
        parent: FigureId,
    },
    InvalidSize {
        figure: FigureId,
        size: Dimension,
    },
    InvalidBounds {
        figure: FigureId,
        bounds: Rectangle,
    },
    Layout(LayoutError),
    Graph(GraphMutationError),
    Connection(ConnectionRuntimeError),
    Rejected,
}

impl fmt::Display for RuntimeMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignRuntime(figure) => write!(formatter, "foreign Runtime Figure: {figure:?}"),
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::UnknownOrDisposedFigure(figure) => {
                write!(formatter, "unknown or disposed Figure ID: {figure:?}")
            }
            Self::DetachedFigure(figure) => write!(formatter, "detached Figure: {figure:?}"),
            Self::SyntheticRootOperation(figure) => {
                write!(
                    formatter,
                    "operation is not allowed on synthetic root {figure:?}"
                )
            }
            Self::WrongCapability { figure, capability } => {
                write!(formatter, "Figure {figure:?} does not support {capability}")
            }
            Self::WrongComponentType {
                figure,
                expected,
                actual,
            } => write!(
                formatter,
                "Figure {figure:?} has component type {actual}, expected {expected}"
            ),
            Self::ComponentRevisionExhausted(figure) => {
                write!(
                    formatter,
                    "component revision is exhausted for Figure {figure:?}"
                )
            }
            Self::ComponentUpdateRejected(figure) => {
                write!(
                    formatter,
                    "deferred component update was rejected for Figure {figure:?}"
                )
            }
            Self::InvalidParentRelation { parent, child } => {
                write!(formatter, "{child:?} is not a direct child of {parent:?}")
            }
            Self::LayeredParent(parent) => {
                write!(
                    formatter,
                    "{parent:?} requires the keyed Layer mutation API"
                )
            }
            Self::InvalidChildIndex {
                parent,
                index,
                child_count,
            } => write!(
                formatter,
                "child index {index} is outside parent {parent:?} child count {child_count}"
            ),
            Self::InvalidChildOrder { parent } => {
                write!(
                    formatter,
                    "child order is not an exact permutation of parent {parent:?}"
                )
            }
            Self::InvalidSize { figure, size } => write!(
                formatter,
                "Figure {figure:?} size must be finite and non-negative, got {size:?}"
            ),
            Self::InvalidBounds { figure, bounds } => write!(
                formatter,
                "Figure {figure:?} bounds must be finite with non-negative size, got {bounds:?}"
            ),
            Self::Layout(error) => error.fmt(formatter),
            Self::Graph(error) => error.fmt(formatter),
            Self::Connection(error) => error.fmt(formatter),
            Self::Rejected => write!(formatter, "runtime mutation was rejected"),
        }
    }
}

impl Error for RuntimeMutationError {}

impl From<LayoutError> for RuntimeMutationError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<GraphMutationError> for RuntimeMutationError {
    fn from(value: GraphMutationError) -> Self {
        Self::Graph(value)
    }
}

impl From<ConnectionRuntimeError> for RuntimeMutationError {
    fn from(value: ConnectionRuntimeError) -> Self {
        Self::Connection(value)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SizeOverrideKind {
    Preferred,
    Minimum,
    Maximum,
}

pub(crate) struct PendingMutation {
    kind: PendingMutationKind,
}

pub(crate) trait DeferredComponentUpdate {
    fn apply(self: Box<Self>, runtime: &mut Runtime) -> Result<bool, RuntimeMutationError>;
}

struct TypedDeferredComponentUpdate<U> {
    figure: FigureId,
    update: U,
}

impl<U> DeferredComponentUpdate for TypedDeferredComponentUpdate<U>
where
    U: FigureComponentUpdate + 'static,
{
    fn apply(self: Box<Self>, runtime: &mut Runtime) -> Result<bool, RuntimeMutationError> {
        let Self { figure, update } = *self;
        match runtime.update_component(figure, update) {
            Ok(_) => Ok(true),
            Err(ComponentUpdateError::Runtime(error)) => Err(error),
            Err(ComponentUpdateError::WrongFigureType {
                figure,
                expected,
                actual,
            }) => Err(RuntimeMutationError::WrongComponentType {
                figure,
                expected,
                actual,
            }),
            Err(ComponentUpdateError::RevisionExhausted(figure)) => {
                Err(RuntimeMutationError::ComponentRevisionExhausted(figure))
            }
            Err(ComponentUpdateError::Rejected(_)) => {
                Err(RuntimeMutationError::ComponentUpdateRejected(figure))
            }
        }
    }
}

pub(crate) enum PendingMutationKind {
    AddChildFigure {
        parent: FigureId,
        figure: Box<dyn Figure>,
    },
    RemoveChild {
        parent: FigureId,
        child: FigureId,
    },
    Reparent {
        child: FigureId,
        new_parent: FigureId,
    },
    SetLayoutManager {
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    },
    SetLayoutConstraint {
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    },
    RemoveLayoutConstraint {
        child: FigureId,
    },
    SetSizeOverride {
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<Dimension>,
    },
    MoveChildToIndex {
        parent: FigureId,
        child: FigureId,
        index: usize,
    },
    BringChildToFront {
        parent: FigureId,
        child: FigureId,
    },
    SendChildToBack {
        parent: FigureId,
        child: FigureId,
    },
    SetChildClippingStrategy {
        figure: FigureId,
        strategy: ChildClippingStrategy,
    },
    AddLayerFigure {
        pane: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    },
    RemoveLayer {
        pane: FigureId,
        key: LayerKey,
    },
    MoveLayer {
        pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    },
    ReparentLayer {
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    },
    UpdateComponent(Box<dyn DeferredComponentUpdate>),
}

impl PendingMutation {
    pub(crate) fn add_child_figure(parent: FigureId, figure: Box<dyn Figure>) -> Self {
        Self {
            kind: PendingMutationKind::AddChildFigure { parent, figure },
        }
    }

    pub(crate) fn remove_child(parent: FigureId, child: FigureId) -> Self {
        Self {
            kind: PendingMutationKind::RemoveChild { parent, child },
        }
    }

    pub(crate) fn reparent(child: FigureId, new_parent: FigureId) -> Self {
        Self {
            kind: PendingMutationKind::Reparent { child, new_parent },
        }
    }

    pub(crate) fn set_layout_manager(
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) -> Self {
        Self {
            kind: PendingMutationKind::SetLayoutManager { container, manager },
        }
    }

    pub(crate) fn set_layout_constraint(
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> Self {
        Self {
            kind: PendingMutationKind::SetLayoutConstraint { child, constraint },
        }
    }

    pub(crate) fn remove_layout_constraint(child: FigureId) -> Self {
        Self {
            kind: PendingMutationKind::RemoveLayoutConstraint { child },
        }
    }

    pub(crate) fn set_size_override(
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<Dimension>,
    ) -> Self {
        Self {
            kind: PendingMutationKind::SetSizeOverride { figure, kind, size },
        }
    }

    pub(crate) fn move_child_to_index(parent: FigureId, child: FigureId, index: usize) -> Self {
        Self {
            kind: PendingMutationKind::MoveChildToIndex {
                parent,
                child,
                index,
            },
        }
    }

    pub(crate) fn bring_child_to_front(parent: FigureId, child: FigureId) -> Self {
        Self {
            kind: PendingMutationKind::BringChildToFront { parent, child },
        }
    }

    pub(crate) fn send_child_to_back(parent: FigureId, child: FigureId) -> Self {
        Self {
            kind: PendingMutationKind::SendChildToBack { parent, child },
        }
    }

    pub(crate) fn set_child_clipping_strategy(
        figure: FigureId,
        strategy: ChildClippingStrategy,
    ) -> Self {
        Self {
            kind: PendingMutationKind::SetChildClippingStrategy { figure, strategy },
        }
    }

    pub(crate) fn add_layer_figure(
        pane: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Self {
        Self {
            kind: PendingMutationKind::AddLayerFigure {
                pane,
                figure,
                key,
                placement,
            },
        }
    }

    pub(crate) fn remove_layer(pane: FigureId, key: LayerKey) -> Self {
        Self {
            kind: PendingMutationKind::RemoveLayer { pane, key },
        }
    }

    pub(crate) fn move_layer(pane: FigureId, key: LayerKey, placement: LayerPlacement) -> Self {
        Self {
            kind: PendingMutationKind::MoveLayer {
                pane,
                key,
                placement,
            },
        }
    }

    pub(crate) fn reparent_layer(
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Self {
        Self {
            kind: PendingMutationKind::ReparentLayer {
                child,
                new_pane,
                key,
                placement,
            },
        }
    }

    pub(crate) fn update_component<U>(figure: FigureId, update: U) -> Self
    where
        U: FigureComponentUpdate + 'static,
    {
        Self {
            kind: PendingMutationKind::UpdateComponent(Box::new(TypedDeferredComponentUpdate {
                figure,
                update,
            })),
        }
    }

    pub(crate) fn into_kind(self) -> PendingMutationKind {
        self.kind
    }
}

#[derive(Default)]
pub struct PendingMutations {
    queue: VecDeque<PendingMutation>,
}

impl PendingMutations {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn enqueue(&mut self, mutation: PendingMutation) {
        self.queue.push_back(mutation);
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(crate) fn drain(&mut self) -> Vec<PendingMutation> {
        self.queue.drain(..).collect()
    }
}

pub(crate) trait MutationContext {
    fn enqueue_mutation(&mut self, mutation: PendingMutation);

    fn add_child_later(&mut self, parent: FigureId, figure: Box<dyn Figure>) {
        self.enqueue_mutation(PendingMutation::add_child_figure(parent, figure));
    }

    fn remove_child_later(&mut self, parent: FigureId, child: FigureId) {
        self.enqueue_mutation(PendingMutation::remove_child(parent, child));
    }

    fn reparent_later(&mut self, child: FigureId, new_parent: FigureId) {
        self.enqueue_mutation(PendingMutation::reparent(child, new_parent));
    }

    fn set_layout_manager_later(
        &mut self,
        container: FigureId,
        manager: Option<Box<dyn LayoutManager>>,
    ) {
        self.enqueue_mutation(PendingMutation::set_layout_manager(container, manager));
    }

    fn set_layout_constraint_later(
        &mut self,
        child: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) {
        self.enqueue_mutation(PendingMutation::set_layout_constraint(child, constraint));
    }

    fn remove_layout_constraint_later(&mut self, child: FigureId) {
        self.enqueue_mutation(PendingMutation::remove_layout_constraint(child));
    }

    fn set_size_override_later(
        &mut self,
        figure: FigureId,
        kind: SizeOverrideKind,
        size: Option<Dimension>,
    ) {
        self.enqueue_mutation(PendingMutation::set_size_override(figure, kind, size));
    }

    fn move_child_to_index_later(&mut self, parent: FigureId, child: FigureId, index: usize) {
        self.enqueue_mutation(PendingMutation::move_child_to_index(parent, child, index));
    }

    fn bring_child_to_front_later(&mut self, parent: FigureId, child: FigureId) {
        self.enqueue_mutation(PendingMutation::bring_child_to_front(parent, child));
    }

    fn send_child_to_back_later(&mut self, parent: FigureId, child: FigureId) {
        self.enqueue_mutation(PendingMutation::send_child_to_back(parent, child));
    }

    fn set_child_clipping_strategy_later(
        &mut self,
        figure: FigureId,
        strategy: ChildClippingStrategy,
    ) {
        self.enqueue_mutation(PendingMutation::set_child_clipping_strategy(
            figure, strategy,
        ));
    }

    fn add_layer_later(
        &mut self,
        pane: FigureId,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) {
        self.enqueue_mutation(PendingMutation::add_layer_figure(
            pane, figure, key, placement,
        ));
    }

    fn remove_layer_later(&mut self, pane: FigureId, key: LayerKey) {
        self.enqueue_mutation(PendingMutation::remove_layer(pane, key));
    }

    fn move_layer_later(&mut self, pane: FigureId, key: LayerKey, placement: LayerPlacement) {
        self.enqueue_mutation(PendingMutation::move_layer(pane, key, placement));
    }

    fn reparent_layer_later(
        &mut self,
        child: FigureId,
        new_pane: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) {
        self.enqueue_mutation(PendingMutation::reparent_layer(
            child, new_pane, key, placement,
        ));
    }
}
