use std::{collections::HashMap, error::Error, fmt, sync::Arc};

use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use crate::{
    Bounded, ChildPolicy, Figure, FigureContainer, FigureId, GraphMutationError, HitParticipation,
    Layer, Runtime,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LayerKey(Arc<str>);

impl LayerKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, LayerKeyError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(LayerKeyError);
        }
        Ok(Self(Arc::from(value)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayerKeyError;

impl fmt::Display for LayerKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "layer key must not be empty")
    }
}

impl Error for LayerKeyError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerPlacement {
    Last,
    Before(LayerKey),
    After(LayerKey),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerError {
    Graph(GraphMutationError),
    UnknownPane,
    NotLayer,
    DuplicateKey,
    UnknownKey,
    AlreadyLayerMember,
    InconsistentState,
}

impl fmt::Display for LayerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(formatter),
            Self::UnknownPane => write!(formatter, "layered pane does not exist"),
            Self::NotLayer => write!(formatter, "figure does not provide the Layer capability"),
            Self::DuplicateKey => write!(formatter, "layer key already exists in this pane"),
            Self::UnknownKey => write!(formatter, "layer key does not exist in this pane"),
            Self::AlreadyLayerMember => write!(formatter, "figure is already a layer in this pane"),
            Self::InconsistentState => write!(formatter, "layer membership is inconsistent"),
        }
    }
}

impl Error for LayerError {}

impl From<GraphMutationError> for LayerError {
    fn from(value: GraphMutationError) -> Self {
        Self::Graph(value)
    }
}

#[derive(Default)]
pub(crate) struct LayeredPaneState {
    by_key: HashMap<LayerKey, FigureId>,
    by_child: HashMap<FigureId, LayerKey>,
}

impl LayeredPaneState {
    pub(crate) fn layer(&self, key: &LayerKey) -> Option<FigureId> {
        self.by_key.get(key).copied()
    }

    pub(crate) fn key(&self, child: FigureId) -> Option<&LayerKey> {
        self.by_child.get(&child)
    }

    pub(crate) fn contains_key(&self, key: &LayerKey) -> bool {
        self.by_key.contains_key(key)
    }

    pub(crate) fn contains_child(&self, child: FigureId) -> bool {
        self.by_child.contains_key(&child)
    }

    pub(crate) fn len(&self) -> usize {
        self.by_child.len()
    }

    pub(crate) fn insert(&mut self, key: LayerKey, child: FigureId) {
        self.by_child.insert(child, key.clone());
        self.by_key.insert(key, child);
    }

    pub(crate) fn remove_key(&mut self, key: &LayerKey) -> Option<FigureId> {
        let child = self.by_key.remove(key)?;
        self.by_child.remove(&child);
        Some(child)
    }

    pub(crate) fn remove_child(&mut self, child: FigureId) -> Option<LayerKey> {
        let key = self.by_child.remove(&child)?;
        self.by_key.remove(&key);
        Some(key)
    }

    pub(crate) fn retain_children(&mut self, mut retain: impl FnMut(FigureId) -> bool) {
        self.by_child.retain(|child, _| retain(*child));
        self.by_key
            .retain(|_, child| self.by_child.contains_key(child));
    }
}

#[derive(Clone)]
pub struct LayerFigure {
    bounds: Rectangle,
}

impl LayerFigure {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
        }
    }
}

impl Bounded for LayerFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "LayerFigure"
    }
}

impl Figure for LayerFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "LayerFigure"
    }

    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn layer(&self) -> Option<&dyn Layer> {
        Some(self)
    }
}

impl FigureContainer for LayerFigure {}
impl Layer for LayerFigure {}

#[derive(Clone)]
pub struct LayeredPane {
    bounds: Rectangle,
}

impl LayeredPane {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
        }
    }
}

impl Bounded for LayeredPane {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "LayeredPane"
    }
}

impl Figure for LayeredPane {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "LayeredPane"
    }

    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn layer(&self) -> Option<&dyn Layer> {
        Some(self)
    }
}

impl FigureContainer for LayeredPane {
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Layered
    }
}

impl Layer for LayeredPane {}

pub struct LayeredPaneHandle<'a> {
    pane_id: FigureId,
    runtime: &'a mut Runtime,
}

impl<'a> LayeredPaneHandle<'a> {
    pub(crate) fn new(pane_id: FigureId, runtime: &'a mut Runtime) -> Self {
        Self { pane_id, runtime }
    }

    pub fn pane_id(&self) -> FigureId {
        self.pane_id
    }

    pub fn add_layer(
        &mut self,
        figure: Box<dyn Figure>,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<FigureId, LayerError> {
        self.runtime.add_layer(self.pane_id, figure, key, placement)
    }

    pub fn remove_layer(&mut self, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.runtime.remove_layer(self.pane_id, key)
    }

    pub fn move_layer(
        &mut self,
        key: &LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.runtime.move_layer(self.pane_id, key, placement)
    }

    pub fn reparent_layer(
        &mut self,
        child: FigureId,
        key: LayerKey,
        placement: LayerPlacement,
    ) -> Result<bool, LayerError> {
        self.runtime
            .reparent_layer(child, self.pane_id, key, placement)
    }

    pub fn layer(&self, key: &LayerKey) -> Result<FigureId, LayerError> {
        self.runtime.layer(self.pane_id, key)
    }

    pub fn layer_key(&self, child: FigureId) -> Result<LayerKey, LayerError> {
        self.runtime.layer_key(self.pane_id, child)
    }

    pub fn layer_ids(&self) -> Result<Vec<FigureId>, LayerError> {
        self.runtime.layer_ids(self.pane_id)
    }
}
