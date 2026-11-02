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
            routing: Routing::default(),
            bendpoints: BTreeMap::new(),
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

#[test]
fn official_expanded_full_adder_and_four_bit_truth_tables() {
    use crate::presets::Preset;
    let mut full = Preset::FullAdder.circuit();
    let carry_in = *full
        .document
        .nodes
        .iter()
        .find(|(_, n)| n.name == "Carry in")
        .unwrap()
        .0;
    for a in 0..=1 {
        for b in 0..=1 {
            for cin in 0..=1 {
                full.document.nodes.get_mut(&1).unwrap().value = a;
                full.document.nodes.get_mut(&2).unwrap().value = b;
                full.document.nodes.get_mut(&carry_in).unwrap().value = cin;
                full.evaluate().unwrap();
                assert_eq!(full.signals[&3], a + b + cin);
            }
        }
    }
    let mut large = Preset::FourBitAdder.circuit();
    assert_eq!(
        large
            .document
            .nodes
            .values()
            .filter(|n| n.kind == Kind::Xor)
            .count(),
        7
    );
    assert_eq!(
        large
            .document
            .nodes
            .values()
            .filter(|n| n.kind == Kind::And)
            .count(),
        7
    );
    assert_eq!(
        large
            .document
            .nodes
            .values()
            .filter(|n| n.kind == Kind::Or)
            .count(),
        3
    );
    assert_eq!(large.signals[&3], 10); // Official createLargeModel starts with 3 + 7.
    for a in 0..=15 {
        for b in 0..=15 {
            large.document.nodes.get_mut(&1).unwrap().value = a;
            large.document.nodes.get_mut(&2).unwrap().value = b;
            large.evaluate().unwrap();
            assert_eq!(large.signals[&3], (a + b) & LED_MASK);
        }
    }
    let restored = Circuit::from_json(&large.to_json().unwrap()).unwrap();
    assert_eq!(restored.document, large.document);
}

#[test]
fn all_presets_have_committed_routes_in_all_router_modes() {
    for preset in crate::presets::Preset::ALL {
        let mut s = Session::new(preset.circuit()).unwrap();
        for routing in Routing::ALL {
            s.set_routing(routing).unwrap();
            let wires = s
                .viewer
                .model()
                .document
                .wires
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for id in wires {
                let wire = s.viewer.connection_part_for_model(Id::Wire(id)).unwrap();
                assert!(
                    s.viewer.connection_route_points_in_surface(wire).is_some(),
                    "{preset:?} {routing:?} wire {id}"
                );
            }
        }
    }
}

#[test]
fn relative_bendpoint_tool_create_move_delete_zoom_and_undo() {
    use novadraw_editor::overlay::{BendpointCreateAction, BendpointMoveAction};
    for scale in [0.5, 1.5] {
        let mut s = Session::new(Circuit::half_adder()).unwrap();
        s.set_routing(Routing::Manual).unwrap();
        s.domain
            .set_viewport_scale_at(&mut s.viewer, scale, Some(Point::ZERO))
            .unwrap();
        let wire = s.viewer.connection_part_for_model(Id::Wire(7)).unwrap();
        s.viewer.replace_selection(wire.edit_part()).unwrap();
        s.sync().unwrap();
        let site = s
            .viewer
            .connection_bendpoint_handle_sites(wire)
            .unwrap()
            .into_iter()
            .find(|s| s.action().get::<BendpointCreateAction>().is_some())
            .unwrap();
        let start = site.location();
        let end = Point::new(start.x(), start.y() + 64.0 * scale);
        s.domain
            .pointer_pressed(
                &mut s.viewer,
                start,
                MouseButton::Left,
                KeyModifiers::default(),
            )
            .unwrap();
        s.domain.pointer_moved(&mut s.viewer, end).unwrap();
        assert!(!s.viewer.model().document.bendpoints.contains_key(&7));
        s.domain
            .pointer_released(&mut s.viewer, end, MouseButton::Left)
            .unwrap();
        s.sync().unwrap();
        assert_eq!(s.viewer.model().document.bendpoints[&7].len(), 1);
        let created = crate::parts::resolved_bendpoints(s.viewer.model(), 7)[0];
        let site = s
            .viewer
            .connection_bendpoint_handle_sites(wire)
            .unwrap()
            .into_iter()
            .find(|s| s.action().get::<BendpointMoveAction>().is_some())
            .unwrap();
        let start = site.location();
        let end = Point::new(start.x() + 32.0 * scale, start.y() + 24.0 * scale);
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
        s.sync().unwrap();
        assert_eq!(
            crate::parts::resolved_bendpoints(s.viewer.model(), 7)[0],
            Point::new(created.x() + 32.0, created.y() + 24.0)
        );
        s.delete_bendpoint(0).unwrap();
        assert!(!s.viewer.model().document.bendpoints.contains_key(&7));
        s.domain.undo(&mut s.viewer).unwrap();
        s.domain.undo(&mut s.viewer).unwrap();
        assert_eq!(
            crate::parts::resolved_bendpoints(s.viewer.model(), 7)[0],
            created
        );
        s.domain.undo(&mut s.viewer).unwrap();
        assert!(!s.viewer.model().document.bendpoints.contains_key(&7));
    }
}

#[test]
fn relative_offsets_follow_endpoint_motion_and_survive_router_switch() {
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    s.set_routing(Routing::Manual).unwrap();
    let wire = s.viewer.model().document.wires[&7];
    let source = crate::parts::port_point(s.viewer.model(), wire.source);
    let target = crate::parts::port_point(s.viewer.model(), wire.target);
    let p = Point::new(280.0, 200.0);
    let points = vec![RelativePoint {
        source: [p.x() - source.x(), p.y() - source.y()],
        target: [p.x() - target.x(), p.y() - target.y()],
    }];
    s.domain
        .execute_command(
            &mut s.viewer,
            Box::new(ChangeBendpoints {
                id: 7,
                before: vec![],
                after: points.clone(),
            }),
        )
        .unwrap();
    let node = s.viewer.model().document.nodes[&1].clone();
    s.domain
        .execute_command(
            &mut s.viewer,
            Box::new(ChangeNode {
                id: 1,
                before: Some(node.clone()),
                after: Node {
                    x: node.x + 32.0,
                    y: node.y + 24.0,
                    ..node
                },
            }),
        )
        .unwrap();
    s.sync().unwrap();
    assert_eq!(
        crate::parts::resolved_bendpoints(s.viewer.model(), 7)[0],
        Point::new(296.0, 212.0)
    );
    s.set_routing(Routing::Shortest).unwrap();
    assert_eq!(s.viewer.model().document.bendpoints[&7], points);
    let mut invalid = ChangeBendpoints {
        id: 7,
        before: points.clone(),
        after: vec![],
    };
    assert!(!invalid.can_execute(s.viewer.model()));
    assert!(
        invalid
            .execute(&mut Circuit::from_json(&s.viewer.model().to_json().unwrap()).unwrap())
            .is_err()
    );
    s.domain.undo(&mut s.viewer).unwrap();
    s.sync().unwrap();
    assert_eq!(s.viewer.model().document.routing, Routing::Manual);
    let restored = Circuit::from_json(&s.viewer.model().to_json().unwrap()).unwrap();
    assert_eq!(restored.document.bendpoints[&7], points);
    let part = s
        .viewer
        .connection_part_for_model(Id::Wire(7))
        .unwrap()
        .edit_part();
    s.viewer.replace_selection(part).unwrap();
    s.delete().unwrap();
    assert!(!s.viewer.model().document.bendpoints.contains_key(&7));
    s.domain.undo(&mut s.viewer).unwrap();
    assert_eq!(s.viewer.model().document.bendpoints[&7], points);
}

#[test]
fn switching_router_cancels_inflight_bendpoint_and_removes_handles() {
    use novadraw_editor::overlay::BendpointCreateAction;
    let mut s = Session::new(Circuit::half_adder()).unwrap();
    s.set_routing(Routing::Manual).unwrap();
    let wire = s.viewer.connection_part_for_model(Id::Wire(7)).unwrap();
    s.viewer.replace_selection(wire.edit_part()).unwrap();
    s.sync().unwrap();
    let site = s.viewer.connection_bendpoint_handle_sites(wire).unwrap()[0].clone();
    assert!(site.action().get::<BendpointCreateAction>().is_some());
    s.domain
        .pointer_pressed(
            &mut s.viewer,
            site.location(),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    s.domain
        .pointer_moved(&mut s.viewer, Point::new(280.0, 200.0))
        .unwrap();
    s.set_routing(Routing::Manhattan).unwrap();
    assert!(!s.domain.has_active_gesture());
    assert!(s.viewer.model().document.bendpoints.is_empty());
    s.domain
        .pointer_released(&mut s.viewer, Point::new(280.0, 200.0), MouseButton::Left)
        .unwrap();
    assert!(s.viewer.model().document.bendpoints.is_empty());
}

#[test]
fn visibility_route_avoids_intermediate_node_and_tracks_obstacle_motion() {
    use novadraw::geometry::Rectangle;
    let mut c = Circuit::half_adder();
    c.document.wires.clear();
    c.document.wires.insert(
        7,
        Wire {
            source: Port::output(1, 0),
            target: Port::input(5, 0),
        },
    );
    let obstacle = c.document.nodes.get_mut(&3).unwrap();
    obstacle.x = 352.0;
    obstacle.y = 40.0;
    let mut s = Session::new(c).unwrap();
    s.set_routing(Routing::Shortest).unwrap();
    let wire = s.viewer.connection_part_for_model(Id::Wire(7)).unwrap();
    let path = s.viewer.connection_route_points_in_surface(wire).unwrap();
    let bounds = s.viewer.model().document.nodes[&3].bounds();
    let outside = |p: Point, b: Rectangle| {
        !(p.x() > b.x && p.x() < b.x + b.width && p.y() > b.y && p.y() < b.y + b.height)
    };
    for segment in path.windows(2) {
        for step in 1..100 {
            let p = segment[0] + (segment[1] - segment[0]) * (f64::from(step) / 100.0);
            assert!(
                outside(p, bounds),
                "route crossed intermediate node at {p:?}"
            );
        }
    }
    let node = s.viewer.model().document.nodes[&3].clone();
    s.domain
        .execute_command(
            &mut s.viewer,
            Box::new(ChangeNode {
                id: 3,
                before: Some(node.clone()),
                after: Node { y: 180.0, ..node },
            }),
        )
        .unwrap();
    s.sync().unwrap();
    assert_ne!(
        s.viewer.connection_route_points_in_surface(wire).unwrap(),
        path
    );
}

#[test]
fn constant_sources_and_invalid_bendpoint_documents() {
    let mut c = Circuit::half_adder();
    c.document.nodes.get_mut(&1).unwrap().kind = Kind::Live;
    c.document.nodes.get_mut(&2).unwrap().kind = Kind::Ground;
    c.evaluate().unwrap();
    assert_eq!(c.signals[&5], 1);
    assert_eq!(c.signals[&6], 0);
    assert!(!c.valid_port(Port::input(1, 0)));
    let mut bad = c.document.clone();
    bad.bendpoints.insert(999, vec![]);
    assert!(Circuit::from_document(bad).is_err());
    let mut bad = c.document;
    bad.bendpoints.insert(
        7,
        vec![RelativePoint {
            source: [f64::NAN, 0.0],
            target: [0.0, 0.0],
        }],
    );
    assert!(Circuit::from_document(bad).is_err());
}

#[test]
#[ignore = "manual preset routing cost measurement"]
fn preset_routing_costs() {
    use std::time::Instant;
    const SAMPLES: usize = 51;
    for preset in crate::presets::Preset::ALL {
        let mut s = Session::new(preset.circuit()).unwrap();
        for routing in Routing::ALL {
            s.set_routing(routing).unwrap();
            let mut samples = Vec::new();
            for iteration in 0..SAMPLES {
                let value = (iteration % 2) as u8;
                let start = Instant::now();
                s.set_input(1, value).unwrap();
                let node = s.viewer.model().document.nodes[&1].clone();
                s.domain
                    .execute_command(
                        &mut s.viewer,
                        Box::new(ChangeNode {
                            id: 1,
                            before: Some(node.clone()),
                            after: Node {
                                x: node.x + if iteration % 2 == 0 { 1.0 } else { -1.0 },
                                ..node
                            },
                        }),
                    )
                    .unwrap();
                s.sync().unwrap();
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "preset={preset:?} router={routing:?} nodes={} wires={} samples={SAMPLES} p50_ms={:.4} p95_ms={:.4}",
                s.viewer.model().document.nodes.len(),
                s.viewer.model().document.wires.len(),
                samples[SAMPLES / 2],
                samples[SAMPLES * 95 / 100]
            );
        }
    }
}
