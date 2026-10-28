use std::convert::Infallible;

use novadraw_editor::{ModelAdapter, ModelEvent, ModelRevision, ModelRevisionError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Debug, Eq, PartialEq)]
enum DiagramEvent {
    Renamed,
}

struct DiagramAdapter {
    revision: ModelRevision,
    events: Vec<ModelEvent<NodeId, DiagramEvent>>,
}

impl ModelAdapter for DiagramAdapter {
    type Error = Infallible;
    type Event = DiagramEvent;
    type ModelId = NodeId;

    fn root(&self) -> Self::ModelId {
        NodeId(1)
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(if model == NodeId(1) {
            vec![NodeId(2), NodeId(3)]
        } else {
            Vec::new()
        })
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

#[test]
fn model_revision_rejects_zero_and_advances_without_wrapping() {
    assert_eq!(ModelRevision::new(0), Err(ModelRevisionError::Zero));

    let initial = ModelRevision::initial();
    assert_eq!(initial.value(), 1);
    assert_eq!(initial.next().unwrap().value(), 2);
    assert_eq!(
        ModelRevision::new(u64::MAX).unwrap().next(),
        Err(ModelRevisionError::Exhausted)
    );
}

#[test]
fn model_adapter_exposes_stable_identity_children_and_ordered_events() {
    let first = ModelEvent::new(ModelRevision::initial(), NodeId(2), DiagramEvent::Renamed);
    let mut adapter = DiagramAdapter {
        revision: ModelRevision::initial(),
        events: vec![first],
    };

    assert_eq!(adapter.root(), NodeId(1));
    assert_eq!(adapter.revision(), ModelRevision::initial());
    assert_eq!(
        adapter.children(NodeId(1)).unwrap(),
        vec![NodeId(2), NodeId(3)]
    );

    let events = adapter.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].revision(), ModelRevision::initial());
    assert_eq!(events[0].subject(), NodeId(2));
    assert_eq!(events[0].payload(), &DiagramEvent::Renamed);
    assert!(adapter.drain_events().is_empty());
}
