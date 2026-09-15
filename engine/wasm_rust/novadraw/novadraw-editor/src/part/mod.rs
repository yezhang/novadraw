//! EditPart identity, topology, lifecycle, factories, and model-to-visual projection.
//!
//! Part topology is separate from both application model containment and the Novadraw FigureTree.

use std::{collections::HashMap, error::Error, fmt};

use novadraw_geometry::Rectangle;
use novadraw_scene::{
    Figure, FigureId, FigureStyle, Runtime, RuntimeMutationError, RuntimeNamespace,
};
use slotmap::{DefaultKey, Key, KeyData, SlotMap};
use uuid::Uuid;

use crate::{ModelAdapter, PolicyInstallation};

/// Namespace shared by every EditPart handle owned by one Viewer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EditorNamespace(Uuid);

impl EditorNamespace {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the namespace UUID.
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Namespaced generational identity of an EditPart.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EditPartId {
    namespace: EditorNamespace,
    local: KeyData,
}

impl EditPartId {
    fn from_local(namespace: EditorNamespace, local: KeyData) -> Self {
        Self { namespace, local }
    }

    /// Returns the owning Viewer namespace.
    pub const fn namespace(self) -> EditorNamespace {
        self.namespace
    }
}

/// The structural role of an EditPart inside a Viewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartKind {
    /// Synthetic Viewer root with no application model.
    Root,
    /// Model-backed containment Part.
    Containment,
    /// Model-backed connection Part outside containment.
    Connection,
}

/// Checked connection role for an [`EditPartId`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConnectionPartId(EditPartId);

impl ConnectionPartId {
    /// Returns the canonical EditPart identity.
    pub const fn edit_part(self) -> EditPartId {
        self.0
    }

    pub(crate) const fn edit_part_ref(&self) -> &EditPartId {
        &self.0
    }
}

/// Source and target EditParts bound to one connection Part.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionEndpoints {
    source: EditPartId,
    target: EditPartId,
}

impl ConnectionEndpoints {
    pub(crate) fn new(source: EditPartId, target: EditPartId) -> Self {
        Self { source, target }
    }

    /// Returns the source endpoint Part.
    pub const fn source(self) -> EditPartId {
        self.source
    }

    /// Returns the target endpoint Part.
    pub const fn target(self) -> EditPartId {
        self.target
    }
}

/// Read-only EditPart state stored by [`PartTree`].
pub struct PartNode<I> {
    id: EditPartId,
    kind: PartKind,
    model_id: Option<I>,
    parent: Option<EditPartId>,
    children: Vec<EditPartId>,
    primary_figure: FigureId,
    content_pane: FigureId,
    visuals: Vec<FigureId>,
    active: bool,
}

impl<I: Copy> PartNode<I> {
    /// Returns this part's identity.
    pub const fn id(&self) -> EditPartId {
        self.id
    }

    /// Returns this Part's structural role.
    pub const fn kind(&self) -> PartKind {
        self.kind
    }

    /// Returns the represented model identity, or `None` for the synthetic root part.
    pub const fn model_id(&self) -> Option<I> {
        self.model_id
    }

    /// Returns the primary Figure controlled by this part.
    pub const fn primary_figure(&self) -> FigureId {
        self.primary_figure
    }

    /// Returns the Figure that receives child part visuals.
    pub const fn content_pane(&self) -> FigureId {
        self.content_pane
    }

    /// Returns all Figures registered as visuals of this part.
    pub fn visuals(&self) -> &[FigureId] {
        &self.visuals
    }

    /// Returns whether the part is active.
    pub const fn is_active(&self) -> bool {
        self.active
    }
}

/// Controller topology for one Viewer.
pub struct PartTree<I> {
    namespace: EditorNamespace,
    nodes: SlotMap<DefaultKey, PartNode<I>>,
    root: EditPartId,
    contents: Option<EditPartId>,
    connections: Vec<ConnectionPartId>,
    connection_endpoints: HashMap<ConnectionPartId, ConnectionEndpoints>,
    outgoing: HashMap<EditPartId, Vec<ConnectionPartId>>,
    incoming: HashMap<EditPartId, Vec<ConnectionPartId>>,
}

impl<I: Copy> PartTree<I> {
    pub(crate) fn new(root_figure: FigureId, content_pane: FigureId) -> Self {
        let namespace = EditorNamespace::new();
        let mut nodes: SlotMap<DefaultKey, PartNode<I>> = SlotMap::with_key();
        let root = nodes.insert_with_key(|key| {
            let id = EditPartId::from_local(namespace, key.data());
            PartNode {
                id,
                kind: PartKind::Root,
                model_id: None,
                parent: None,
                children: Vec::new(),
                primary_figure: root_figure,
                content_pane,
                visuals: vec![root_figure],
                active: true,
            }
        });
        Self {
            namespace,
            nodes,
            root: EditPartId::from_local(namespace, root.data()),
            contents: None,
            connections: Vec::new(),
            connection_endpoints: HashMap::new(),
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
        }
    }

    /// Returns the Viewer namespace.
    pub const fn namespace(&self) -> EditorNamespace {
        self.namespace
    }

    /// Returns the synthetic root part.
    pub const fn root(&self) -> EditPartId {
        self.root
    }

    /// Returns the model-backed contents part.
    pub const fn contents(&self) -> Option<EditPartId> {
        self.contents
    }

    /// Resolves a part owned by this Viewer.
    pub fn get(&self, id: EditPartId) -> Option<&PartNode<I>> {
        self.local(id).and_then(|key| self.nodes.get(key))
    }

    /// Returns a part's parent.
    pub fn parent(&self, id: EditPartId) -> Option<EditPartId> {
        self.get(id)
            .filter(|node| node.kind != PartKind::Connection)
            .and_then(|node| node.parent)
    }

    /// Returns direct children in stable display order.
    pub fn children(&self, id: EditPartId) -> Option<&[EditPartId]> {
        self.get(id)
            .filter(|node| node.kind != PartKind::Connection)
            .map(|node| node.children.as_slice())
    }

    /// Returns all connection Parts in connection-layer order.
    pub fn connection_parts(&self) -> &[ConnectionPartId] {
        &self.connections
    }

    /// Converts a general EditPart identity into a checked connection identity.
    pub fn as_connection(&self, id: EditPartId) -> Result<ConnectionPartId, PartTreeError> {
        let node = self.get(id).ok_or_else(|| self.invalid_part_error(id))?;
        (node.kind == PartKind::Connection)
            .then_some(ConnectionPartId(id))
            .ok_or(PartTreeError::InvalidPartKind)
    }

    /// Returns the source and target of a connection Part.
    pub fn connection_endpoints(
        &self,
        connection: ConnectionPartId,
    ) -> Result<ConnectionEndpoints, PartTreeError> {
        self.resolve_connection(connection)?;
        self.connection_endpoints
            .get(&connection)
            .copied()
            .ok_or(PartTreeError::ConnectionNotBound)
    }

    /// Returns connections whose source is the supplied containment Part.
    pub fn source_connections(
        &self,
        endpoint: EditPartId,
    ) -> Result<&[ConnectionPartId], PartTreeError> {
        self.resolve_endpoint(endpoint)?;
        Ok(self
            .outgoing
            .get(&endpoint)
            .map(Vec::as_slice)
            .unwrap_or_default())
    }

    /// Returns connections whose target is the supplied containment Part.
    pub fn target_connections(
        &self,
        endpoint: EditPartId,
    ) -> Result<&[ConnectionPartId], PartTreeError> {
        self.resolve_endpoint(endpoint)?;
        Ok(self
            .incoming
            .get(&endpoint)
            .map(Vec::as_slice)
            .unwrap_or_default())
    }

    pub(crate) fn insert(
        &mut self,
        parent: EditPartId,
        model_id: I,
        primary_figure: FigureId,
        content_pane: FigureId,
        visuals: Vec<FigureId>,
    ) -> Result<EditPartId, PartTreeError> {
        let parent_key = self.local(parent).ok_or(PartTreeError::ForeignPart)?;
        if !self.nodes.contains_key(parent_key) {
            return Err(PartTreeError::UnknownParent);
        }
        let namespace = self.namespace;
        let key = self.nodes.insert_with_key(|key| {
            let id = EditPartId::from_local(namespace, key.data());
            PartNode {
                id,
                kind: PartKind::Containment,
                model_id: Some(model_id),
                parent: Some(parent),
                children: Vec::new(),
                primary_figure,
                content_pane,
                visuals,
                active: false,
            }
        });
        let id = EditPartId::from_local(namespace, key.data());
        self.nodes[parent_key].children.push(id);
        Ok(id)
    }

    pub(crate) fn insert_connection(
        &mut self,
        model_id: I,
        primary_figure: FigureId,
        content_pane: FigureId,
        visuals: Vec<FigureId>,
        source: EditPartId,
        target: EditPartId,
    ) -> Result<ConnectionPartId, PartTreeError> {
        self.resolve_endpoint(source)?;
        self.resolve_endpoint(target)?;
        let namespace = self.namespace;
        let key = self.nodes.insert_with_key(|key| {
            let id = EditPartId::from_local(namespace, key.data());
            PartNode {
                id,
                kind: PartKind::Connection,
                model_id: Some(model_id),
                parent: None,
                children: Vec::new(),
                primary_figure,
                content_pane,
                visuals,
                active: false,
            }
        });
        let connection = ConnectionPartId(EditPartId::from_local(namespace, key.data()));
        self.connections.push(connection);
        self.connection_endpoints
            .insert(connection, ConnectionEndpoints::new(source, target));
        self.rebuild_connection_indexes();
        Ok(connection)
    }

    pub(crate) fn bind_connection(
        &mut self,
        connection: ConnectionPartId,
        source: EditPartId,
        target: EditPartId,
    ) -> Result<bool, PartTreeError> {
        self.resolve_connection(connection)?;
        self.resolve_endpoint(source)?;
        self.resolve_endpoint(target)?;
        let endpoints = ConnectionEndpoints::new(source, target);
        if self.connection_endpoints.get(&connection) == Some(&endpoints) {
            return Ok(false);
        }
        self.connection_endpoints.insert(connection, endpoints);
        self.rebuild_connection_indexes();
        Ok(true)
    }

    pub(crate) fn unbind_connection(
        &mut self,
        connection: ConnectionPartId,
    ) -> Result<bool, PartTreeError> {
        self.resolve_connection(connection)?;
        let removed = self.connection_endpoints.remove(&connection).is_some();
        if removed {
            self.rebuild_connection_indexes();
        }
        Ok(removed)
    }

    pub(crate) fn set_connection_order(
        &mut self,
        order: Vec<ConnectionPartId>,
    ) -> Result<bool, PartTreeError> {
        if order.len() != self.connections.len() {
            return Err(PartTreeError::InvalidConnectionOrder);
        }
        let mut seen = std::collections::HashSet::with_capacity(order.len());
        for connection in &order {
            self.resolve_connection(*connection)?;
            if !seen.insert(*connection) {
                return Err(PartTreeError::InvalidConnectionOrder);
            }
        }
        if order == self.connections {
            return Ok(false);
        }
        self.connections = order;
        self.rebuild_connection_indexes();
        Ok(true)
    }

    pub(crate) fn retire_connection(
        &mut self,
        connection: ConnectionPartId,
    ) -> Result<(), PartTreeError> {
        let key = self.resolve_connection(connection)?;
        if self.connection_endpoints.contains_key(&connection) {
            return Err(PartTreeError::ConnectionStillBound);
        }
        self.connections
            .retain(|candidate| *candidate != connection);
        self.nodes.remove(key);
        self.rebuild_connection_indexes();
        Ok(())
    }

    pub(crate) fn designate_contents(&mut self, id: EditPartId) -> Result<(), PartTreeError> {
        if self.parent(id) != Some(self.root) {
            return Err(PartTreeError::InvalidContents);
        }
        self.contents = Some(id);
        Ok(())
    }

    pub(crate) fn set_active(&mut self, id: EditPartId, active: bool) -> Result<(), PartTreeError> {
        let key = self.resolve(id)?;
        self.nodes[key].active = active;
        Ok(())
    }

    pub(crate) fn reorder_child(
        &mut self,
        parent: EditPartId,
        child: EditPartId,
        index: usize,
    ) -> Result<bool, PartTreeError> {
        let parent_key = self.resolve(parent)?;
        self.resolve(child)?;
        let children = &mut self.nodes[parent_key].children;
        let current = children
            .iter()
            .position(|candidate| *candidate == child)
            .ok_or(PartTreeError::InvalidParent)?;
        if index >= children.len() {
            return Err(PartTreeError::InvalidIndex);
        }
        if current == index {
            return Ok(false);
        }
        children.remove(current);
        children.insert(index, child);
        Ok(true)
    }

    pub(crate) fn subtree_ids(&self, root: EditPartId) -> Result<Vec<EditPartId>, PartTreeError> {
        let key = self.resolve(root)?;
        if self.nodes[key].kind == PartKind::Connection {
            return Err(PartTreeError::InvalidPartKind);
        }
        let mut result = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            result.push(id);
            let node = self.get(id).ok_or(PartTreeError::UnknownPart)?;
            stack.extend(node.children.iter().rev().copied());
        }
        Ok(result)
    }

    pub(crate) fn retire_subtree(&mut self, root: EditPartId) -> Result<(), PartTreeError> {
        if root == self.root {
            return Err(PartTreeError::CannotRemoveRoot);
        }
        let ids = self.subtree_ids(root)?;
        if ids.iter().any(|id| {
            self.outgoing.get(id).is_some_and(|items| !items.is_empty())
                || self.incoming.get(id).is_some_and(|items| !items.is_empty())
        }) {
            return Err(PartTreeError::EndpointInUse);
        }
        let parent = self.parent(root).ok_or(PartTreeError::InvalidParent)?;
        let parent_key = self.resolve(parent)?;
        self.nodes[parent_key]
            .children
            .retain(|candidate| *candidate != root);
        for id in ids.into_iter().rev() {
            let key = self.resolve(id)?;
            self.nodes.remove(key);
        }
        if self.contents == Some(root) {
            self.contents = None;
        }
        Ok(())
    }

    fn local(&self, id: EditPartId) -> Option<DefaultKey> {
        (id.namespace == self.namespace).then(|| DefaultKey::from(id.local))
    }

    fn resolve(&self, id: EditPartId) -> Result<DefaultKey, PartTreeError> {
        let key = self.local(id).ok_or(PartTreeError::ForeignPart)?;
        self.nodes
            .contains_key(key)
            .then_some(key)
            .ok_or(PartTreeError::UnknownPart)
    }

    fn resolve_connection(
        &self,
        connection: ConnectionPartId,
    ) -> Result<DefaultKey, PartTreeError> {
        let key = self.resolve(connection.0)?;
        (self.nodes[key].kind == PartKind::Connection)
            .then_some(key)
            .ok_or(PartTreeError::InvalidPartKind)
    }

    fn resolve_endpoint(&self, endpoint: EditPartId) -> Result<DefaultKey, PartTreeError> {
        let key = self.resolve(endpoint)?;
        (self.nodes[key].kind == PartKind::Containment)
            .then_some(key)
            .ok_or(PartTreeError::InvalidPartKind)
    }

    fn invalid_part_error(&self, id: EditPartId) -> PartTreeError {
        if id.namespace == self.namespace {
            PartTreeError::UnknownPart
        } else {
            PartTreeError::ForeignPart
        }
    }

    fn rebuild_connection_indexes(&mut self) {
        self.outgoing.clear();
        self.incoming.clear();
        for connection in self.connections.iter().copied() {
            let Some(endpoints) = self.connection_endpoints.get(&connection).copied() else {
                continue;
            };
            self.outgoing
                .entry(endpoints.source)
                .or_default()
                .push(connection);
            self.incoming
                .entry(endpoints.target)
                .or_default()
                .push(connection);
        }
    }
}

/// Part topology mutation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartTreeError {
    /// The handle belongs to another Viewer.
    ForeignPart,
    /// The handle has been retired or never existed.
    UnknownPart,
    /// The requested parent does not exist.
    UnknownParent,
    /// The part is not a child of the supplied parent.
    InvalidParent,
    /// The child index is outside the parent child list.
    InvalidIndex,
    /// Contents must be the direct child of the synthetic root.
    InvalidContents,
    /// The Part does not have the role required by this operation.
    InvalidPartKind,
    /// The connection Part does not currently have both endpoint relations.
    ConnectionNotBound,
    /// The connection Part must be unbound before it is retired.
    ConnectionStillBound,
    /// Connection ordering must contain every live connection exactly once.
    InvalidConnectionOrder,
    /// A containment subtree is still referenced by a connection.
    EndpointInUse,
    /// The synthetic root cannot be removed.
    CannotRemoveRoot,
}

impl fmt::Display for PartTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for PartTreeError {}

/// Error returned by an application EditPart extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditPartError {
    message: String,
}

impl EditPartError {
    /// Creates an extension error.
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the extension error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for EditPartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for EditPartError {}

impl From<RuntimeMutationError> for EditPartError {
    fn from(value: RuntimeMutationError) -> Self {
        Self::operation(value.to_string())
    }
}

/// Immutable creation context passed to an [`EditPartFactory`].
#[derive(Clone, Copy, Debug)]
pub struct PartFactoryContext<I> {
    parent_part: EditPartId,
    parent_model: Option<I>,
    model_id: I,
}

/// Immutable creation context for a connection Part.
#[derive(Clone, Copy, Debug)]
pub struct ConnectionPartFactoryContext<I> {
    root_part: EditPartId,
    model_id: I,
    source_part: EditPartId,
    source_model: I,
    target_part: EditPartId,
    target_model: I,
}

impl<I: Copy> ConnectionPartFactoryContext<I> {
    pub(crate) fn new(
        root_part: EditPartId,
        model_id: I,
        source_part: EditPartId,
        source_model: I,
        target_part: EditPartId,
        target_model: I,
    ) -> Self {
        Self {
            root_part,
            model_id,
            source_part,
            source_model,
            target_part,
            target_model,
        }
    }

    /// Returns the synthetic root that owns the connection Part lifecycle.
    pub const fn root_part(self) -> EditPartId {
        self.root_part
    }

    /// Returns the represented connection model identity.
    pub const fn model_id(self) -> I {
        self.model_id
    }

    /// Returns the source endpoint Part.
    pub const fn source_part(self) -> EditPartId {
        self.source_part
    }

    /// Returns the source endpoint model identity.
    pub const fn source_model(self) -> I {
        self.source_model
    }

    /// Returns the target endpoint Part.
    pub const fn target_part(self) -> EditPartId {
        self.target_part
    }

    /// Returns the target endpoint model identity.
    pub const fn target_model(self) -> I {
        self.target_model
    }
}

impl<I: Copy> PartFactoryContext<I> {
    pub(crate) fn new(parent_part: EditPartId, parent_model: Option<I>, model_id: I) -> Self {
        Self {
            parent_part,
            parent_model,
            model_id,
        }
    }

    /// Returns the parent controller.
    pub const fn parent_part(self) -> EditPartId {
        self.parent_part
    }

    /// Returns the parent model identity, or `None` below the synthetic root.
    pub const fn parent_model(self) -> Option<I> {
        self.parent_model
    }

    /// Returns the model represented by the part being created.
    pub const fn model_id(self) -> I {
        self.model_id
    }
}

/// Restricted context for constructing a part's compound visual.
pub struct VisualBuildContext<'a> {
    runtime: &'a mut Runtime,
    primary: FigureId,
    content_pane: FigureId,
    visuals: Vec<FigureId>,
}

impl<'a> VisualBuildContext<'a> {
    pub(crate) fn new(runtime: &'a mut Runtime, primary: FigureId) -> Self {
        Self {
            runtime,
            primary,
            content_pane: primary,
            visuals: vec![primary],
        }
    }

    /// Returns the primary Figure.
    pub const fn primary(&self) -> FigureId {
        self.primary
    }

    /// Adds an owned decorative or content-pane Figure below this part's visual subtree.
    pub fn add_child(
        &mut self,
        parent: FigureId,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, EditPartError> {
        if !self.visuals.contains(&parent) {
            return Err(EditPartError::operation(
                "visual parent is not owned by this EditPart",
            ));
        }
        let id = self.runtime.try_add_figure(parent, figure)?;
        self.visuals.push(id);
        Ok(id)
    }

    /// Selects one owned Figure as the child EditPart content pane.
    pub fn set_content_pane(&mut self, content_pane: FigureId) -> Result<(), EditPartError> {
        if !self.visuals.contains(&content_pane) {
            return Err(EditPartError::operation(
                "content pane is not owned by this EditPart",
            ));
        }
        self.content_pane = content_pane;
        Ok(())
    }

    pub(crate) fn finish(self) -> (FigureId, Vec<FigureId>) {
        (self.content_pane, self.visuals)
    }
}

/// Restricted context for refreshing one part's visual state.
pub struct VisualUpdateContext<'a> {
    runtime: &'a mut Runtime,
    primary: FigureId,
    content_pane: FigureId,
}

impl<'a> VisualUpdateContext<'a> {
    pub(crate) fn new(runtime: &'a mut Runtime, primary: FigureId, content_pane: FigureId) -> Self {
        Self {
            runtime,
            primary,
            content_pane,
        }
    }

    /// Returns the primary Figure.
    pub const fn primary(&self) -> FigureId {
        self.primary
    }

    /// Returns the child EditPart content pane.
    pub const fn content_pane(&self) -> FigureId {
        self.content_pane
    }

    /// Updates primary Figure bounds through the Runtime transaction boundary.
    pub fn set_primary_bounds(&mut self, bounds: Rectangle) -> Result<bool, EditPartError> {
        Ok(self.runtime.set_bounds(self.primary, bounds))
    }

    /// Updates primary Figure style through the Runtime transaction boundary.
    pub fn set_primary_style(&mut self, style: FigureStyle) -> Result<bool, EditPartError> {
        Ok(self.runtime.set_figure_style(self.primary, style))
    }
}

/// Application controller behavior for one model-backed EditPart.
pub trait EditPartBehavior<A: ModelAdapter> {
    /// Creates the primary Figure before it is attached to the Runtime.
    fn create_figure(
        &mut self,
        model: &A,
        model_id: A::ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError>;

    /// Optionally constructs internal visuals and selects a content pane.
    fn configure_visual(
        &mut self,
        _model: &A,
        _model_id: A::ModelId,
        _context: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        Ok(())
    }

    /// Creates role-keyed policies before the initial visual refresh.
    fn create_policies(
        &mut self,
        _model: &A,
        _model_id: A::ModelId,
    ) -> Result<Vec<PolicyInstallation<A>>, EditPartError> {
        Ok(Vec::new())
    }

    /// Refreshes visual properties from the current application model.
    fn refresh_visuals(
        &mut self,
        _model: &A,
        _model_id: A::ModelId,
        _context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        Ok(())
    }

    /// Activates model observation after the complete initial projection exists.
    fn activate(&mut self, _model: &A, _model_id: A::ModelId) -> Result<(), EditPartError> {
        Ok(())
    }

    /// Deactivates model observation before the part and visual subtree are retired.
    fn deactivate(&mut self, _model: &A, _model_id: A::ModelId) {}
}

/// Factory for application-specific EditPart behavior.
pub trait EditPartFactory<A: ModelAdapter> {
    /// Creates behavior for one model object in a parent context.
    fn create(
        &mut self,
        context: PartFactoryContext<A::ModelId>,
        model: &A,
    ) -> Result<Box<dyn EditPartBehavior<A>>, EditPartError>;

    /// Creates behavior for one connection model outside containment.
    fn create_connection(
        &mut self,
        context: ConnectionPartFactoryContext<A::ModelId>,
        model: &A,
    ) -> Result<Box<dyn EditPartBehavior<A>>, EditPartError> {
        self.create(
            PartFactoryContext::new(context.root_part, None, context.model_id),
            model,
        )
    }
}

pub(crate) struct BehaviorStore<A: ModelAdapter> {
    entries: HashMap<EditPartId, Box<dyn EditPartBehavior<A>>>,
    namespace: EditorNamespace,
}

impl<A: ModelAdapter> BehaviorStore<A> {
    pub(crate) fn new(namespace: EditorNamespace) -> Self {
        Self {
            entries: HashMap::new(),
            namespace,
        }
    }

    pub(crate) fn insert(
        &mut self,
        expected: EditPartId,
        behavior: Box<dyn EditPartBehavior<A>>,
    ) -> Result<(), PartTreeError> {
        if expected.namespace != self.namespace {
            return Err(PartTreeError::ForeignPart);
        }
        self.entries.insert(expected, behavior);
        Ok(())
    }

    pub(crate) fn get_mut(
        &mut self,
        id: EditPartId,
    ) -> Option<&mut (dyn EditPartBehavior<A> + 'static)> {
        if id.namespace != self.namespace {
            return None;
        }
        self.entries.get_mut(&id).map(Box::as_mut)
    }

    pub(crate) fn remove(&mut self, id: EditPartId) -> Option<Box<dyn EditPartBehavior<A>>> {
        if id.namespace != self.namespace {
            return None;
        }
        self.entries.remove(&id)
    }
}

pub(crate) fn validate_runtime_namespace(
    expected: RuntimeNamespace,
    figure: FigureId,
) -> Result<(), EditPartError> {
    if figure.namespace() != expected {
        return Err(EditPartError::operation(
            "Figure belongs to another Runtime",
        ));
    }
    Ok(())
}
