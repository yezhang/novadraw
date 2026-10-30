use novadraw::geometry::Rectangle;
use novadraw_editor::{
    Command, CommandError, ModelAdapter, ModelConnection, ModelEvent, ModelRevision,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::convert::Infallible;

pub const NODE_WIDTH: f64 = 144.0;
pub const NODE_HEIGHT: f64 = 112.0;
const MAX_DOCUMENT_ITEMS: usize = 10_000;
const MAX_POSITION: f64 = 1_000_000.0;
pub const LED_MASK: u8 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Id {
    Root,
    Node(u64),
    Port(Port),
    Wire(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Direction {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Port {
    pub node: u64,
    pub direction: Direction,
    pub index: u8,
}
impl Port {
    pub const fn input(node: u64, index: u8) -> Self {
        Self {
            node,
            direction: Direction::Input,
            index,
        }
    }
    pub const fn output(node: u64, index: u8) -> Self {
        Self {
            node,
            direction: Direction::Output,
            index,
        }
    }
    pub fn label(self) -> &'static str {
        match (self.direction, self.index) {
            (Direction::Input, 0) => "A",
            (Direction::Input, 1) => "B",
            (Direction::Input, 2) => "C",
            (Direction::Input, _) => "D",
            (Direction::Output, 0) => "1",
            (Direction::Output, 1) => "2",
            (Direction::Output, 2) => "3",
            (Direction::Output, _) => "4",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Led,
    And,
    Or,
    Xor,
    Live,
    Ground,
}
impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Led,
        Self::And,
        Self::Or,
        Self::Xor,
        Self::Live,
        Self::Ground,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Led => "LED",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Xor => "XOR",
            Self::Live => "Live",
            Self::Ground => "Ground",
        }
    }
    pub const fn inputs(self) -> u8 {
        match self {
            Self::Led => 4,
            Self::Live | Self::Ground => 0,
            _ => 2,
        }
    }
    pub const fn outputs(self) -> u8 {
        if matches!(self, Self::Led) { 4 } else { 1 }
    }
    pub fn ports(self, node: u64) -> Vec<Id> {
        (0..self.inputs())
            .map(|i| Id::Port(Port::input(node, i)))
            .chain((0..self.outputs()).map(|i| Id::Port(Port::output(node, i))))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: Kind,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub value: u8,
}
impl Node {
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.x, self.y, NODE_WIDTH, NODE_HEIGHT)
    }
    fn valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.x.abs() <= MAX_POSITION
            && self.y.abs() <= MAX_POSITION
            && self.value <= LED_MASK
            && !self.name.trim().is_empty()
            && self.name.len() <= 128
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wire {
    pub source: Port,
    pub target: Port,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Routing {
    Manual,
    #[default]
    Manhattan,
    Shortest,
}
impl Routing {
    pub const ALL: [Self; 3] = [Self::Manual, Self::Manhattan, Self::Shortest];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Manhattan => "Manhattan",
            Self::Shortest => "Shortest path",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelativePoint {
    pub source: [f64; 2],
    pub target: [f64; 2],
}
impl RelativePoint {
    pub fn valid(&self) -> bool {
        self.source
            .into_iter()
            .chain(self.target)
            .all(|v| v.is_finite() && v.abs() <= MAX_POSITION)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    pub nodes: BTreeMap<u64, Node>,
    pub order: Vec<u64>,
    pub wires: BTreeMap<u64, Wire>,
    #[serde(default)]
    pub routing: Routing,
    #[serde(default)]
    pub bendpoints: BTreeMap<u64, Vec<RelativePoint>>,
}

pub struct Circuit {
    pub document: Document,
    pub signals: BTreeMap<u64, u8>,
    revision: ModelRevision,
    events: Vec<ModelEvent<Id, ()>>,
    pub next_id: u64,
}
impl Circuit {
    pub fn half_adder() -> Self {
        let nodes = [
            (1, Kind::Led, "Input A", 64.0, 64.0),
            (2, Kind::Led, "Input B", 64.0, 304.0),
            (3, Kind::Xor, "Sum · XOR", 352.0, 64.0),
            (4, Kind::And, "Carry · AND", 352.0, 304.0),
            (5, Kind::Led, "Sum", 640.0, 64.0),
            (6, Kind::Led, "Carry", 640.0, 304.0),
        ]
        .into_iter()
        .map(|(id, kind, name, x, y)| {
            (
                id,
                Node {
                    kind,
                    name: name.into(),
                    x,
                    y,
                    value: 0,
                },
            )
        })
        .collect();
        let pairs = [
            (1, 3, 0),
            (2, 3, 1),
            (1, 4, 0),
            (2, 4, 1),
            (3, 5, 0),
            (4, 6, 0),
        ];
        let wires = pairs
            .into_iter()
            .enumerate()
            .map(|(i, (source, target, pin))| {
                (
                    i as u64 + 7,
                    Wire {
                        source: Port::output(source, 0),
                        target: Port::input(target, pin),
                    },
                )
            })
            .collect();
        Self::from_document(Document {
            version: 1,
            nodes,
            order: (1..=6).collect(),
            wires,
            routing: Routing::default(),
            bendpoints: BTreeMap::new(),
        })
        .expect("valid half-adder")
    }
    pub fn from_document(document: Document) -> Result<Self, String> {
        if document.version != 1 {
            return Err("Unsupported document version".into());
        }
        if document.nodes.len() + document.wires.len() > MAX_DOCUMENT_ITEMS {
            return Err("Circuit is too large".into());
        }
        if document
            .nodes
            .iter()
            .any(|(id, node)| *id == 0 || *id == u64::MAX || !node.valid())
            || document
                .wires
                .keys()
                .any(|id| *id == 0 || *id == u64::MAX || document.nodes.contains_key(id))
            || document.order.len() != document.nodes.len()
            || document.order.iter().copied().collect::<BTreeSet<_>>()
                != document.nodes.keys().copied().collect()
        {
            return Err("Invalid node identity, order or geometry".into());
        }
        let next_id = document
            .nodes
            .keys()
            .chain(document.wires.keys())
            .max()
            .copied()
            .unwrap_or(0)
            + 1;
        let mut circuit = Self {
            document,
            signals: BTreeMap::new(),
            revision: ModelRevision::initial(),
            events: Vec::new(),
            next_id,
        };
        let mut occupied = BTreeSet::new();
        if circuit.document.bendpoints.iter().any(|(id, points)| {
            !circuit.document.wires.contains_key(id)
                || points.len() > MAX_DOCUMENT_ITEMS
                || points.iter().any(|point| !point.valid())
        }) {
            return Err("Invalid wire bendpoints".into());
        }
        for wire in circuit.document.wires.values() {
            if wire.source.direction != Direction::Output
                || wire.target.direction != Direction::Input
                || wire.source.node == wire.target.node
                || !circuit.valid_port(wire.source)
                || !circuit.valid_port(wire.target)
                || !occupied.insert(wire.target)
            {
                return Err("Invalid terminal or occupied input".into());
            }
        }
        circuit.evaluate()?;
        Ok(circuit)
    }
    pub fn from_json(json: &str) -> Result<Self, String> {
        Self::from_document(serde_json::from_str(json).map_err(|e| e.to_string())?)
    }
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.document).map_err(|e| e.to_string())
    }
    pub fn valid_port(&self, port: Port) -> bool {
        self.document.nodes.get(&port.node).is_some_and(|node| {
            port.index
                < match port.direction {
                    Direction::Input => node.kind.inputs(),
                    Direction::Output => node.kind.outputs(),
                }
        })
    }
    pub fn driven(&self, id: u64) -> bool {
        self.document
            .wires
            .values()
            .any(|wire| wire.target.node == id)
    }
    pub fn signal(&self, port: Port) -> bool {
        match port.direction {
            Direction::Output => self
                .signals
                .get(&port.node)
                .is_some_and(|value| value & (1 << port.index) != 0),
            Direction::Input => self
                .document
                .wires
                .values()
                .find(|wire| wire.target == port)
                .is_some_and(|wire| self.signal(wire.source)),
        }
    }
    pub fn accepts(&self, wire: Wire, excluding: Option<u64>) -> bool {
        if wire.source.direction != Direction::Output
            || wire.target.direction != Direction::Input
            || wire.source.node == wire.target.node
            || !self.valid_port(wire.source)
            || !self.valid_port(wire.target)
            || self
                .document
                .wires
                .iter()
                .any(|(id, other)| Some(*id) != excluding && other.target == wire.target)
        {
            return false;
        }
        let mut outgoing: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for (id, other) in &self.document.wires {
            if Some(*id) != excluding {
                outgoing
                    .entry(other.source.node)
                    .or_default()
                    .push(other.target.node);
            }
        }
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::from([wire.target.node]);
        while let Some(node) = queue.pop_front() {
            if node == wire.source.node {
                return false;
            }
            if visited.insert(node)
                && let Some(next) = outgoing.get(&node)
            {
                queue.extend(next);
            }
        }
        true
    }
    /// One deterministic visit per node. Feedback cycles are rejected before mutation.
    pub fn evaluate(&mut self) -> Result<usize, String> {
        let mut degree: BTreeMap<u64, usize> =
            self.document.nodes.keys().map(|id| (*id, 0)).collect();
        let mut outgoing: BTreeMap<u64, Vec<Wire>> = BTreeMap::new();
        for wire in self.document.wires.values() {
            *degree
                .get_mut(&wire.target.node)
                .ok_or("Missing target node")? += 1;
            outgoing.entry(wire.source.node).or_default().push(*wire);
        }
        let mut queue: VecDeque<_> = degree
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect();
        let mut inputs: BTreeMap<u64, u8> = BTreeMap::new();
        let mut driven = BTreeSet::new();
        let mut signals = BTreeMap::new();
        while let Some(id) = queue.pop_front() {
            let node = &self.document.nodes[&id];
            let bits = inputs.get(&id).copied().unwrap_or(0);
            let a = bits & 1 != 0;
            let b = bits & 2 != 0;
            let value = match node.kind {
                Kind::Led => {
                    if driven.contains(&id) {
                        bits
                    } else {
                        node.value
                    }
                }
                Kind::And => u8::from(a && b),
                Kind::Or => u8::from(a || b),
                Kind::Xor => u8::from(a ^ b),
                Kind::Live => 1,
                Kind::Ground => 0,
            };
            signals.insert(id, value);
            if let Some(wires) = outgoing.get(&id) {
                for wire in wires {
                    driven.insert(wire.target.node);
                    if value & (1 << wire.source.index) != 0 {
                        *inputs.entry(wire.target.node).or_default() |= 1 << wire.target.index;
                    }
                    let remaining = degree.get_mut(&wire.target.node).ok_or("Missing target")?;
                    *remaining -= 1;
                    if *remaining == 0 {
                        queue.push_back(wire.target.node);
                    }
                }
            }
        }
        if signals.len() != self.document.nodes.len() {
            return Err("Feedback circuits are not supported".into());
        }
        let visits = signals.len();
        self.signals = signals;
        Ok(visits)
    }
    pub(crate) fn publish(&mut self, id: Id) {
        self.revision = self.revision.next().expect("demo revision exhausted");
        self.events.push(ModelEvent::new(self.revision, id, ()));
    }
    fn changed(&mut self, subject: Id) -> Result<(), CommandError> {
        let before = self.signals.clone();
        self.evaluate().map_err(CommandError::operation)?;
        self.publish(subject);
        let changed: BTreeSet<_> = self
            .signals
            .iter()
            .filter_map(|(id, value)| (before.get(id) != Some(value)).then_some(*id))
            .collect();
        for id in &changed {
            self.publish(Id::Node(*id));
            for port in self.document.nodes[id].kind.ports(*id) {
                self.publish(port);
            }
        }
        let wires: Vec<_> = self
            .document
            .wires
            .iter()
            .filter_map(|(id, wire)| {
                changed
                    .contains(&wire.source.node)
                    .then_some((*id, wire.target))
            })
            .collect();
        for (id, target) in wires {
            self.publish(Id::Wire(id));
            self.publish(Id::Port(target));
        }
        Ok(())
    }
}
impl ModelAdapter for Circuit {
    type ModelId = Id;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> Id {
        Id::Root
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: Id) -> Result<Vec<Id>, Infallible> {
        Ok(match id {
            Id::Root => self.document.order.iter().copied().map(Id::Node).collect(),
            Id::Node(node) => self.document.nodes[&node].kind.ports(node),
            _ => Vec::new(),
        })
    }
    fn connections(&self) -> Result<Vec<ModelConnection<Id>>, Infallible> {
        Ok(self
            .document
            .wires
            .iter()
            .map(|(id, wire)| {
                ModelConnection::new(Id::Wire(*id), Id::Port(wire.source), Id::Port(wire.target))
            })
            .collect())
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<Id, ()>> {
        std::mem::take(&mut self.events)
    }
}

pub struct ChangeNode {
    pub id: u64,
    pub before: Option<Node>,
    pub after: Node,
}
impl Command<Circuit> for ChangeNode {
    fn label(&self) -> &str {
        "Change logic node"
    }
    fn can_execute(&self, model: &Circuit) -> bool {
        model.document.nodes.get(&self.id) == self.before.as_ref()
            && self.before.as_ref() != Some(&self.after)
            && self.after.valid()
            && self.id != 0
            && self.id != u64::MAX
            && !model.document.wires.contains_key(&self.id)
            && (self.before.is_some()
                || model.document.nodes.len() + model.document.wires.len() < MAX_DOCUMENT_ITEMS)
            && self
                .before
                .as_ref()
                .is_none_or(|node| node.kind == self.after.kind)
    }
    fn execute(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        if !self.can_execute(model) {
            return Err(CommandError::operation("Node change was rejected"));
        }
        model.document.nodes.insert(self.id, self.after.clone());
        let subject = if self.before.is_none() {
            model.document.order.push(self.id);
            model.next_id = model.next_id.max(self.id + 1);
            Id::Root
        } else {
            Id::Node(self.id)
        };
        model.changed(subject)
    }
    fn undo(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        match &self.before {
            Some(node) => {
                model.document.nodes.insert(self.id, node.clone());
            }
            None => {
                model.document.nodes.remove(&self.id);
                model.document.order.retain(|id| *id != self.id);
            }
        }
        model.changed(if self.before.is_none() {
            Id::Root
        } else {
            Id::Node(self.id)
        })
    }
}

pub struct ChangeWire {
    pub id: u64,
    pub before: Option<Wire>,
    pub after: Wire,
}
impl Command<Circuit> for ChangeWire {
    fn label(&self) -> &str {
        "Connect terminals"
    }
    fn can_execute(&self, model: &Circuit) -> bool {
        model.document.wires.get(&self.id).copied() == self.before
            && self.before != Some(self.after)
            && model.accepts(self.after, Some(self.id))
            && self.id != 0
            && self.id != u64::MAX
            && !model.document.nodes.contains_key(&self.id)
            && (self.before.is_some()
                || model.document.nodes.len() + model.document.wires.len() < MAX_DOCUMENT_ITEMS)
    }
    fn execute(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        if !self.can_execute(model) {
            return Err(CommandError::operation("Terminal connection was rejected"));
        }
        model.document.wires.insert(self.id, self.after);
        model.next_id = model.next_id.max(self.id + 1);
        model.changed(Id::Root)
    }
    fn undo(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        if let Some(wire) = self.before {
            model.document.wires.insert(self.id, wire);
        } else {
            model.document.wires.remove(&self.id);
        }
        model.changed(Id::Root)
    }
}

pub struct DeleteItem {
    pub id: Id,
    node: Option<(u64, Node, usize)>,
    wires: BTreeMap<u64, Wire>,
    bendpoints: BTreeMap<u64, Vec<RelativePoint>>,
}
impl DeleteItem {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            node: None,
            wires: BTreeMap::new(),
            bendpoints: BTreeMap::new(),
        }
    }
}
impl Command<Circuit> for DeleteItem {
    fn label(&self) -> &str {
        "Delete logic item"
    }
    fn execute(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        self.wires.clear();
        self.node = None;
        self.bendpoints.clear();
        match self.id {
            Id::Node(id) => {
                if let Some(node) = model.document.nodes.remove(&id) {
                    let index = model
                        .document
                        .order
                        .iter()
                        .position(|item| *item == id)
                        .expect("node order");
                    model.document.order.remove(index);
                    self.node = Some((id, node, index));
                    self.wires = model
                        .document
                        .wires
                        .iter()
                        .filter(|(_, wire)| wire.source.node == id || wire.target.node == id)
                        .map(|(id, wire)| (*id, *wire))
                        .collect();
                    model
                        .document
                        .wires
                        .retain(|id, _| !self.wires.contains_key(id));
                }
            }
            Id::Wire(id) => {
                if let Some(wire) = model.document.wires.remove(&id) {
                    self.wires.insert(id, wire);
                }
            }
            _ => return Err(CommandError::operation("Select a node or wire to delete")),
        }
        for id in self.wires.keys() {
            if let Some(points) = model.document.bendpoints.remove(id) {
                self.bendpoints.insert(*id, points);
            }
        }
        model.changed(Id::Root)
    }
    fn undo(&mut self, model: &mut Circuit) -> Result<(), CommandError> {
        if let Some((id, node, index)) = &self.node {
            model.document.nodes.insert(*id, node.clone());
            model.document.order.insert(*index, *id);
        }
        model.document.wires.extend(self.wires.clone());
        model.document.bendpoints.extend(self.bendpoints.clone());
        model.changed(Id::Root)
    }
}

pub struct ChangeRouting {
    pub before: Routing,
    pub after: Routing,
}
impl Command<Circuit> for ChangeRouting {
    fn label(&self) -> &str {
        "Change connection router"
    }
    fn can_execute(&self, m: &Circuit) -> bool {
        m.document.routing == self.before && self.before != self.after
    }
    fn execute(&mut self, m: &mut Circuit) -> Result<(), CommandError> {
        if !self.can_execute(m) {
            return Err(CommandError::operation("Router change rejected"));
        }
        m.document.routing = self.after;
        m.publish(Id::Root);
        Ok(())
    }
    fn undo(&mut self, m: &mut Circuit) -> Result<(), CommandError> {
        m.document.routing = self.before;
        m.publish(Id::Root);
        Ok(())
    }
}

pub struct ChangeBendpoints {
    pub id: u64,
    pub before: Vec<RelativePoint>,
    pub after: Vec<RelativePoint>,
}
impl ChangeBendpoints {
    fn put(m: &mut Circuit, id: u64, points: &[RelativePoint]) {
        if points.is_empty() {
            m.document.bendpoints.remove(&id);
        } else {
            m.document.bendpoints.insert(id, points.to_vec());
        }
        m.publish(Id::Wire(id));
    }
}
impl Command<Circuit> for ChangeBendpoints {
    fn label(&self) -> &str {
        "Edit relative bendpoint"
    }
    fn can_execute(&self, m: &Circuit) -> bool {
        m.document.routing == Routing::Manual
            && m.document.wires.contains_key(&self.id)
            && m.document
                .bendpoints
                .get(&self.id)
                .cloned()
                .unwrap_or_default()
                == self.before
            && self.before != self.after
            && self.after.len() <= MAX_DOCUMENT_ITEMS
            && self.after.iter().all(RelativePoint::valid)
    }
    fn execute(&mut self, m: &mut Circuit) -> Result<(), CommandError> {
        if !self.can_execute(m) {
            return Err(CommandError::operation("Bendpoint edit rejected"));
        }
        Self::put(m, self.id, &self.after);
        Ok(())
    }
    fn undo(&mut self, m: &mut Circuit) -> Result<(), CommandError> {
        Self::put(m, self.id, &self.before);
        Ok(())
    }
}
