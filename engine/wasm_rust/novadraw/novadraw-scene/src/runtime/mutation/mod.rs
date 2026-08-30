use std::collections::VecDeque;

use crate::{BlockId, Figure};

pub(crate) struct PendingMutation {
    kind: PendingMutationKind,
}

pub(crate) enum PendingMutationKind {
    AddChildFigure {
        parent: BlockId,
        figure: Box<dyn Figure>,
    },
    RemoveChild {
        parent: BlockId,
        child: BlockId,
    },
    Reparent {
        child: BlockId,
        new_parent: BlockId,
    },
}

impl PendingMutation {
    pub(crate) fn add_child_figure(parent: BlockId, figure: Box<dyn Figure>) -> Self {
        Self {
            kind: PendingMutationKind::AddChildFigure { parent, figure },
        }
    }

    pub(crate) fn remove_child(parent: BlockId, child: BlockId) -> Self {
        Self {
            kind: PendingMutationKind::RemoveChild { parent, child },
        }
    }

    pub(crate) fn reparent(child: BlockId, new_parent: BlockId) -> Self {
        Self {
            kind: PendingMutationKind::Reparent { child, new_parent },
        }
    }

    pub(crate) fn into_kind(self) -> PendingMutationKind {
        self.kind
    }
}

pub struct PendingMutationBatch {
    mutations: Vec<PendingMutation>,
}

impl PendingMutationBatch {
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    pub(crate) fn into_vec(self) -> Vec<PendingMutation> {
        self.mutations
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

    pub fn drain(&mut self) -> PendingMutationBatch {
        PendingMutationBatch {
            mutations: self.queue.drain(..).collect(),
        }
    }
}

pub(crate) trait MutationContext {
    fn enqueue_mutation(&mut self, mutation: PendingMutation);

    fn add_child_later(&mut self, parent: BlockId, figure: Box<dyn Figure>) {
        self.enqueue_mutation(PendingMutation::add_child_figure(parent, figure));
    }

    fn remove_child_later(&mut self, parent: BlockId, child: BlockId) {
        self.enqueue_mutation(PendingMutation::remove_child(parent, child));
    }

    fn reparent_later(&mut self, child: BlockId, new_parent: BlockId) {
        self.enqueue_mutation(PendingMutation::reparent(child, new_parent));
    }
}
