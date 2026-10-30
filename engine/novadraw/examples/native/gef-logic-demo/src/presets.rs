//! Expanded gate topology of GEF LogicDiagramFactory presets.
//! Circuit boundary passthroughs are resolved to wires; gate composition stays visible.
use crate::model::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    #[default]
    HalfAdder,
    FullAdder,
    FourBitAdder,
}
impl Preset {
    pub const ALL: [Self; 3] = [Self::HalfAdder, Self::FullAdder, Self::FourBitAdder];
    pub const fn name(self) -> &'static str {
        match self {
            Self::HalfAdder => "Half-adder",
            Self::FullAdder => "Full-adder",
            Self::FourBitAdder => "4-bit adder",
        }
    }
    pub fn circuit(self) -> Circuit {
        if self == Self::HalfAdder {
            return Circuit::half_adder();
        }
        let mut b = Builder::default();
        let a = b.node(Kind::Led, "Input A", 64.0, 64.0, 3);
        let second = b.node(Kind::Led, "Input B", 64.0, 304.0, 7);
        let result = b.node(
            Kind::Led,
            "Sum",
            if self == Self::FullAdder {
                1120.0
            } else {
                3680.0
            },
            64.0,
            0,
        );
        if self == Self::FullAdder {
            let cin = b.node(Kind::Led, "Carry in", 64.0, 544.0, 0);
            let (sum, carry) = b.full(
                Port::output(a, 0),
                Port::output(second, 0),
                Port::output(cin, 0),
                352.0,
                "FA",
            );
            b.wire(sum, Port::input(result, 0));
            b.wire(carry, Port::input(result, 1));
        } else {
            // GEF createLargeModel: low bit HA, then three FA stages with carry chaining.
            let (sum, mut carry) =
                b.half(Port::output(a, 0), Port::output(second, 0), 352.0, "Bit 0");
            b.wire(sum, Port::input(result, 0));
            for bit in 1..4 {
                let (sum, next) = b.full(
                    Port::output(a, bit),
                    Port::output(second, bit),
                    carry,
                    352.0 + f64::from(bit) * 832.0,
                    &format!("Bit {bit}"),
                );
                b.wire(sum, Port::input(result, bit));
                carry = next;
            }
            // The official large model exposes four sum bits and discards overflow.
        }
        Circuit::from_document(b.document()).expect("valid GEF preset topology")
    }
}
#[derive(Default)]
struct Builder {
    next: u64,
    nodes: BTreeMap<u64, Node>,
    wires: BTreeMap<u64, Wire>,
}
impl Builder {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
    fn node(&mut self, kind: Kind, name: &str, x: f64, y: f64, value: u8) -> u64 {
        let id = self.id();
        self.nodes.insert(
            id,
            Node {
                kind,
                name: name.into(),
                x,
                y,
                value,
            },
        );
        id
    }
    fn wire(&mut self, source: Port, target: Port) {
        let id = self.id();
        self.wires.insert(id, Wire { source, target });
    }
    fn half(&mut self, a: Port, b: Port, x: f64, name: &str) -> (Port, Port) {
        let xor = self.node(Kind::Xor, &format!("{name} · XOR"), x, 64.0, 0);
        let and = self.node(Kind::And, &format!("{name} · AND"), x, 304.0, 0);
        for id in [xor, and] {
            self.wire(a, Port::input(id, 0));
            self.wire(b, Port::input(id, 1));
        }
        (Port::output(xor, 0), Port::output(and, 0))
    }
    fn full(&mut self, a: Port, b: Port, cin: Port, x: f64, name: &str) -> (Port, Port) {
        let (first, carry1) = self.half(a, b, x, &format!("{name} H1"));
        let (sum, carry2) = self.half(first, cin, x + 256.0, &format!("{name} H2"));
        let or = self.node(Kind::Or, &format!("{name} · OR"), x + 512.0, 304.0, 0);
        self.wire(carry1, Port::input(or, 0));
        self.wire(carry2, Port::input(or, 1));
        (sum, Port::output(or, 0))
    }
    fn document(self) -> Document {
        Document {
            version: 1,
            order: self.nodes.keys().copied().collect(),
            nodes: self.nodes,
            wires: self.wires,
            routing: Routing::Manhattan,
            bendpoints: BTreeMap::new(),
        }
    }
}
