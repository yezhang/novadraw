use std::collections::VecDeque;

use crate::{Figure, FigureId, LayerKey, LayerPlacement};

pub(crate) struct PendingMutation {
    kind: PendingMutationKind,
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

    pub(crate) fn from_kind(kind: PendingMutationKind) -> Self {
        Self { kind }
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
