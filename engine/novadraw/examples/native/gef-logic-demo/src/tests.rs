use crate::{model::*, parts::port_bounds, session::Session};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::Point;
use novadraw_editor::Command;

#[test]
fn half_adder_truth_table_and_undo_redo() {
    let mut session = Session::new(Circuit::half_adder()).unwrap();
    for a in 0..=1 {
        for b in 0..=1 {
            session.set_input(1, a).unwrap();
            session.set_input(2, b).unwrap();
            assert_eq!(session.viewer.model().signals[&5], a ^ b);
            assert_eq!(session.viewer.model().signals[&6], a & b);
        }
    }
    session.domain.undo(&mut session.viewer).unwrap();
    assert_eq!(session.viewer.model().signals[&5], 1);
    assert_eq!(session.viewer.model().signals[&6], 0);
    session.domain.redo(&mut session.viewer).unwrap();
    assert_eq!(session.viewer.model().signals[&6], 1);
}
#[test]
fn or_gate_truth_table_and_led_bit_order() {
    let mut circuit = Circuit::half_adder();
    circuit.document.nodes.get_mut(&3).unwrap().kind = Kind::Or;
    for a in 0..=1 {
        for b in 0..=1 {
            circuit.document.nodes.get_mut(&1).unwrap().value = a;
            circuit.document.nodes.get_mut(&2).unwrap().value = b;
            assert_eq!(circuit.evaluate().unwrap(), 6);
            assert_eq!(circuit.signals[&3], a | b);
        }
    }
    circuit.document.wires.clear();
    circuit.document.nodes.get_mut(&1).unwrap().value = 8;
    circuit.document.nodes.get_mut(&5).unwrap().value = 15;
    circuit.document.wires.insert(
        13,
        Wire {
            source: Port::output(1, 3),
            target: Port::input(5, 2),
        },
    );
    circuit.evaluate().unwrap();
    assert_eq!(circuit.signals[&5], 4);
    assert!(circuit.signal(Port::output(1, 3)));
    assert!(!circuit.signal(Port::output(1, 0)));
}
#[test]
fn connection_validation_covers_direction_occupancy_and_cycles() {
    let c = Circuit::half_adder();
    assert!(!c.accepts(
        Wire {
            source: Port::input(1, 0),
            target: Port::input(3, 0)
        },
        None
    ));
    assert!(!c.accepts(
        Wire {
            source: Port::output(2, 0),
            target: Port::input(3, 0)
        },
        None
    ));
    assert!(!c.accepts(
        Wire {
            source: Port::output(5, 0),
            target: Port::input(1, 0)
        },
        None
    ));
    assert!(!c.accepts(
        Wire {
            source: Port::output(3, 1),
            target: Port::input(1, 0)
        },
        None
    ));
    assert!(c.accepts(
        Wire {
            source: Port::output(1, 0),
            target: Port::input(6, 1)
        },
        None
    ));
    assert!(c.accepts(c.document.wires[&7], Some(7)));
}
#[test]
fn file_roundtrip_recomputes_values_and_rejects_invalid_documents() {
    let mut c = Circuit::half_adder();
    c.document.nodes.get_mut(&1).unwrap().value = 1;
    c.evaluate().unwrap();
    let json = c.to_json().unwrap();
    assert!(!json.contains("signals"));
    let reopened = Circuit::from_json(&json).unwrap();
    assert_eq!(c.document, reopened.document);
    assert_eq!(c.signals, reopened.signals);
    let mut bad = c.document.clone();
    bad.order.push(1);
    assert!(Circuit::from_document(bad).is_err());
    let mut bad = c.document.clone();
    bad.wires.insert(
        13,
        Wire {
            source: Port::output(6, 0),
            target: Port::input(2, 0),
        },
    );
    assert!(Circuit::from_document(bad).is_err());
    let mut bad = c.document;
    bad.nodes.get_mut(&1).unwrap().x = f64::NAN;
    assert!(Circuit::from_document(bad).is_err());
}
#[test]
fn deleting_multiple_nodes_and_selected_wire_restores_exact_document() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let before = s.viewer.model().document.clone();
    let one = s.viewer.part_for_model(Id::Node(1)).unwrap();
    let three = s.viewer.part_for_model(Id::Node(3)).unwrap();
    let wire = s
        .viewer
        .connection_part_for_model(Id::Wire(7))
        .unwrap()
        .edit_part();
    s.viewer.set_selection(&[one, three, wire]).unwrap();
    s.delete().unwrap();
    assert!(!s.viewer.model().document.nodes.contains_key(&1));
    assert!(!s.viewer.model().document.nodes.contains_key(&3));
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document, before);
    s.domain.redo(&mut s.viewer).unwrap();
    assert!(!s.viewer.model().document.nodes.contains_key(&1));
}
#[test]
fn port_projection_anchor_follows_node_move() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let wire = s.viewer.connection_part_for_model(Id::Wire(7)).unwrap();
    let initial = s
        .viewer
        .connection_route_endpoints_in_surface(wire)
        .unwrap();
    let before = s.viewer.model().document.nodes[&1].clone();
    let after = Node {
        x: before.x + 32.0,
        y: before.y + 24.0,
        ..before.clone()
    };
    s.domain
        .execute_command(
            &mut s.viewer,
            Box::new(ChangeNode {
                id: 1,
                before: Some(before),
                after,
            }),
        )
        .unwrap();
    s.sync().unwrap();
    let moved = s
        .viewer
        .connection_route_endpoints_in_surface(wire)
        .unwrap();
    assert_eq!(
        moved.0,
        Point::new(initial.0.x() + 32.0, initial.0.y() + 24.0)
    );
    assert_eq!(moved.1, initial.1);
    assert!(
        s.viewer
            .part_for_model(Id::Port(Port::input(3, 0)))
            .is_some()
    );
    s.domain.undo(&mut s.viewer).unwrap();
    s.sync().unwrap();
    assert_eq!(
        s.viewer
            .connection_route_endpoints_in_surface(wire)
            .unwrap(),
        initial
    );
}
fn pin(s: &Session, port: Port) -> Point {
    let b = port_bounds(s.viewer.model(), port);
    let node = &s.viewer.model().document.nodes[&port.node];
    Point::new(node.x + b.x + b.width / 2.0, node.y + b.y + b.height / 2.0)
}
fn click(s: &mut Session, point: Point) {
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            point,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain
        .pointer_released(&mut s.viewer, point, MouseButton::Left)
        .unwrap();
}
#[test]
fn connection_tool_binds_terminal_and_cancel_keeps_history() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let before = s.viewer.model().document.clone();
    s.set_mode(crate::session::Mode::Connect).unwrap();
    let source = pin(&s, Port::output(1, 2));
    let target = pin(&s, Port::input(6, 2));
    click(&mut s, source);
    s.domain.pointer_moved(&mut s.viewer, target).unwrap();
    click(&mut s, target);
    assert_eq!(
        s.viewer.model().document.wires[&13],
        Wire {
            source: Port::output(1, 2),
            target: Port::input(6, 2)
        }
    );
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document, before);
    s.set_mode(crate::session::Mode::Connect).unwrap();
    click(&mut s, source);
    s.cancel().unwrap();
    assert_eq!(s.viewer.model().document, before);
    assert!(!s.domain.has_active_gesture());
}
#[test]
fn reconnect_preserves_identity_and_undo_restores_terminal() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let before = s.viewer.model().document.wires[&7];
    let after = Wire {
        source: Port::output(2, 2),
        ..before
    };
    s.domain
        .execute_command(
            &mut s.viewer,
            Box::new(ChangeWire {
                id: 7,
                before: Some(before),
                after,
            }),
        )
        .unwrap();
    assert_eq!(s.viewer.model().document.wires[&7], after);
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document.wires[&7], before);
}
#[test]
fn rejected_command_does_not_mutate_model() {
    let mut c = Circuit::half_adder();
    let before = c.document.clone();
    let mut cmd = ChangeWire {
        id: 13,
        before: None,
        after: Wire {
            source: Port::output(2, 0),
            target: Port::input(3, 0),
        },
    };
    assert!(!cmd.can_execute(&c));
    assert!(cmd.execute(&mut c).is_err());
    assert_eq!(c.document, before);
}

#[test]
fn selection_drag_with_overlay_commits_once_and_cancel_restores() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let part = s.viewer.part_for_model(Id::Node(1)).unwrap();
    s.viewer.replace_selection(part).unwrap();
    s.sync().unwrap();
    let start = Point::new(136.0, 120.0);
    let end = Point::new(168.0, 144.0);
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain.pointer_moved(&mut s.viewer, end).unwrap();
    s.sync().unwrap();
    assert_eq!(s.viewer.model().document.nodes[&1].x, 64.0);
    s.domain
        .pointer_released(&mut s.viewer, end, MouseButton::Left)
        .unwrap();
    assert_eq!(s.viewer.model().document.nodes[&1].x, 96.0);
    assert_eq!(s.viewer.model().document.nodes[&1].y, 88.0);
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document.nodes[&1].x, 64.0);
    s.sync().unwrap();
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain.pointer_moved(&mut s.viewer, end).unwrap();
    s.cancel().unwrap();
    assert_eq!(s.viewer.model().document.nodes[&1].x, 64.0);
}

#[test]
fn creation_tool_projects_new_ports_and_undo_removes_them() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    s.set_mode(crate::session::Mode::Create(Kind::Or)).unwrap();
    click(&mut s, Point::new(500.0, 440.0));
    assert_eq!(s.viewer.model().document.nodes[&13].kind, Kind::Or);
    assert!(
        s.viewer
            .part_for_model(Id::Port(Port::input(13, 1)))
            .is_some()
    );
    s.domain.undo(&mut s.viewer).unwrap();
    assert!(!s.viewer.model().document.nodes.contains_key(&13));
    s.domain.redo(&mut s.viewer).unwrap();
    assert!(
        s.viewer
            .part_for_model(Id::Port(Port::output(13, 0)))
            .is_some()
    );
}

#[test]
fn endpoint_handle_reconnects_through_policy() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let connection = s.viewer.connection_part_for_model(Id::Wire(7)).unwrap();
    s.viewer.replace_selection(connection.edit_part()).unwrap();
    s.sync().unwrap();
    let (_, start) = s
        .viewer
        .connection_route_endpoints_in_surface(connection)
        .unwrap();
    let end = pin(&s, Port::input(4, 2));
    // AND has only two inputs, so an invalid terminal never becomes a target.
    assert!(!s.viewer.model().valid_port(Port::input(4, 2)));
    let end = Point::new(end.x(), end.y() + 20.0);
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain.pointer_moved(&mut s.viewer, end).unwrap();
    s.cancel().unwrap();
    let before = s.viewer.model().document.wires[&7];
    s.sync().unwrap();
    let end = pin(&s, Port::input(6, 1));
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            start,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain.pointer_moved(&mut s.viewer, end).unwrap();
    s.domain
        .pointer_released(&mut s.viewer, end, MouseButton::Left)
        .unwrap();
    assert_eq!(
        s.viewer.model().document.wires[&7].target,
        Port::input(6, 1)
    );
    assert_eq!(s.viewer.model().document.wires.len(), 6);
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document.wires[&7], before);
}

#[test]
fn input_port_notifications_do_not_depend_on_gate_output_changing() {
    use novadraw_editor::ModelAdapter;
    let mut c = Circuit::half_adder();
    let before = c.document.nodes[&1].clone();
    let after = Node {
        value: 1,
        ..before.clone()
    };
    ChangeNode {
        id: 1,
        before: Some(before),
        after,
    }
    .execute(&mut c)
    .unwrap();
    assert_eq!(c.signals[&4], 0);
    assert!(
        c.drain_events()
            .iter()
            .any(|event| event.subject() == Id::Port(Port::input(4, 0)))
    );
}

#[test]
fn wire_component_stroke_tracks_signal_and_undo() {
    use novadraw::connection::{ConnectionFigure, ConnectionFigureBehavior};
    use novadraw::graphics::Color;
    use novadraw::runtime::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate};
    struct Probe(Color);
    impl FigureComponentUpdate for Probe {
        type Figure = ConnectionFigure;
        type Prepared = ();
        type Error = String;
        fn prepare(
            self,
            current: &ConnectionFigure,
            _: FigureComponentContext,
        ) -> Result<PreparedFigureUpdate<()>, String> {
            if current.connection_stroke_color() != self.0 {
                return Err("Wire stroke did not follow signal".into());
            }
            Ok(PreparedFigureUpdate::unchanged(()))
        }
        fn commit(_: (), _: &mut ConnectionFigure) {}
    }
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    let part = s
        .viewer
        .connection_part_for_model(Id::Wire(7))
        .unwrap()
        .edit_part();
    let figure = s.viewer.parts().get(part).unwrap().primary_figure();
    s.viewer
        .update_visual_component(figure, Probe(crate::parts::low_color()))
        .unwrap();
    s.set_input(1, 1).unwrap();
    s.viewer
        .update_visual_component(figure, Probe(crate::parts::high_color()))
        .unwrap();
    s.domain.undo(&mut s.viewer).unwrap();
    s.viewer
        .update_visual_component(figure, Probe(crate::parts::low_color()))
        .unwrap();
}

#[test]
#[ignore = "manual propagation cost measurement"]
fn propagation_costs() {
    use std::{collections::BTreeMap, time::Instant};
    for count in [6_u64, 512, 2048] {
        let nodes = (1..=count)
            .map(|id| {
                (
                    id,
                    Node {
                        kind: Kind::Led,
                        name: format!("LED {id}"),
                        x: 0.0,
                        y: 0.0,
                        value: u8::from(id == 1),
                    },
                )
            })
            .collect();
        let wires: BTreeMap<_, _> = (1..count)
            .map(|id| {
                (
                    count + id,
                    Wire {
                        source: Port::output(id, 0),
                        target: Port::input(id + 1, 0),
                    },
                )
            })
            .collect();
        let mut circuit = Circuit::from_document(Document {
            version: 1,
            nodes,
            order: (1..=count).collect(),
            wires,
        })
        .unwrap();
        let mut samples = Vec::new();
        const SAMPLE_COUNT: usize = 201;
        for _ in 0..SAMPLE_COUNT {
            let start = Instant::now();
            assert_eq!(
                std::hint::black_box(circuit.evaluate().unwrap()),
                count as usize
            );
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "nodes={count} wires={} samples={SAMPLE_COUNT} p50_ms={:.4} p95_ms={:.4}",
            count - 1,
            samples[SAMPLE_COUNT / 2],
            samples[SAMPLE_COUNT * 95 / 100]
        );
    }
}
