use novadraw::geometry::{Dimension, Insets};
use novadraw::layout::graph::*;

#[path = "../../../examples/scenes/src/graph_fixtures.rs"]
mod fixtures;

#[test]
fn official_directed_fixtures_cover_both_directions() {
    let service = service();
    let counts = [
        (9, 8),
        (21, 29),
        (11, 12),
        (9, 10),
        (18, 18),
        (15, 14),
        (62, 77),
        (16, 17),
        (21, 28),
        (10, 16),
        (7, 7),
        (6, 5),
        (4, 3),
        (11, 16),
        (6, 6),
        (9, 9),
    ];
    for (name, counts) in fixtures::DIRECTED_FIXTURES.iter().zip(counts) {
        for direction in [GraphDirection::South, GraphDirection::East] {
            let fixture = fixtures::directed_fixture(name, direction, &mut context()).unwrap();
            assert_eq!(fixture.labels.len(), fixture.request.nodes().len());
            assert_eq!(
                (fixture.request.nodes().len(), fixture.request.edges().len()),
                counts,
                "{name}"
            );
            let result = service
                .solve(&fixture.request, &mut context())
                .unwrap_or_else(|error| panic!("{name} {direction:?}: {error:?}"));
            let repeated = service.solve(&fixture.request, &mut context()).unwrap();
            assert_eq!(
                result.placements(),
                repeated.placements(),
                "{name} {direction:?}"
            );
            assert_eq!(result.routes(), repeated.routes(), "{name} {direction:?}");
        }
    }
}

fn context() -> GraphSolveContext {
    GraphSolveContext::new(GraphBudget::new(200_000_000, 20_000_000))
}
fn service() -> GraphLayoutService {
    let mut service = GraphLayoutService::new();
    register_layered_protocol(&mut service).unwrap();
    register_directed_graph(&mut service).unwrap();
    service
}
fn request(direction: GraphDirection, cycle: bool) -> GraphLayoutRequest {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        direction,
        DIRECTED_GRAPH,
        DirectedGraphConfig::default(),
        &mut ctx,
    )
    .unwrap();
    builder.default_padding(Insets::uniform(16.0));
    let mut nodes = Vec::new();
    for _ in 0..4 {
        nodes.push(
            builder
                .add_node(
                    GraphSize::directed(Dimension::new(50.0, 40.0), direction),
                    &mut ctx,
                )
                .unwrap(),
        );
    }
    for (source, target) in [(0, 1), (1, 2), (2, 3), (0, 3)] {
        let edge = builder
            .add_edge(nodes[source], nodes[target], &mut ctx)
            .unwrap();
        let edge = builder.edge_mut(edge).unwrap();
        edge.width = 1.0;
        edge.padding = 10.0;
    }
    if cycle {
        let edge = builder.add_edge(nodes[3], nodes[0], &mut ctx).unwrap();
        builder.edge_mut(edge).unwrap().padding = 10.0;
    }
    builder.require(LAYERED_RESULT, &mut ctx).unwrap();
    builder.build(&mut ctx).unwrap()
}

#[test]
fn directed_long_edges_and_feedback_preserve_original_endpoints() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        for cycle in [false, true] {
            let request = request(direction, cycle);
            let service = service();
            let result = service.solve(&request, &mut context()).unwrap();
            let evidence = result.capability(LAYERED_RESULT).unwrap().unwrap();
            assert_eq!(evidence.nodes.len(), 4);
            assert!(
                evidence
                    .edges
                    .iter()
                    .any(|edge| !edge.virtual_nodes.is_empty())
            );
            assert_eq!(evidence.edges.iter().any(|edge| edge.feedback), cycle);
            for edge in request.edges() {
                let route = result.route(edge.id()).unwrap();
                assert_eq!(route.source.endpoint, edge.source);
                assert_eq!(route.target.endpoint, edge.target);
                assert_eq!(route.points.first(), Some(&route.source.position));
                assert_eq!(route.points.last(), Some(&route.target.position));
            }
            let repeated = service.solve(&request, &mut context()).unwrap();
            assert_eq!(result.placements(), repeated.placements());
            assert_eq!(result.routes(), repeated.routes());
            assert_eq!(
                evidence,
                repeated.capability(LAYERED_RESULT).unwrap().unwrap()
            );
        }
    }
}

#[test]
fn fixed_axis_rejects_row_stretch_and_zero_span_is_admitted() {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::South,
        DIRECTED_GRAPH,
        DirectedGraphConfig::default(),
        &mut ctx,
    )
    .unwrap();
    let a = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
        .unwrap();
    let b = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 30.0)), &mut ctx)
        .unwrap();
    let edge = builder.add_edge(a, b, &mut ctx).unwrap();
    builder
        .edge_mut(edge)
        .unwrap()
        .constraints
        .insert(
            LAYERED_EDGE,
            LayeredEdgeConstraint {
                minimum_span: 0,
                weight: 1,
            },
            &mut ctx,
        )
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        service().solve(&request, &mut context()),
        Err(GraphLayoutError::UnsatisfiedSizeConstraint(_))
    ));
}

#[test]
fn zero_and_positive_span_self_loops_have_distinct_failures() {
    for minimum_span in [0, 1] {
        let mut ctx = context();
        let mut builder = GraphRequestBuilder::new(
            GraphDirection::South,
            DIRECTED_GRAPH,
            DirectedGraphConfig::default(),
            &mut ctx,
        )
        .unwrap();
        let node = builder
            .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
            .unwrap();
        let edge = builder.add_edge(node, node, &mut ctx).unwrap();
        builder
            .edge_mut(edge)
            .unwrap()
            .constraints
            .insert(
                LAYERED_EDGE,
                LayeredEdgeConstraint {
                    minimum_span,
                    weight: 1,
                },
                &mut ctx,
            )
            .unwrap();
        let request = builder.build(&mut ctx).unwrap();
        let error = match service().solve(&request, &mut context()) {
            Err(error) => error,
            Ok(_) => panic!("self loop accepted"),
        };
        if minimum_span == 0 {
            assert_eq!(error, GraphLayoutError::UnsupportedSelfLoop(edge));
        } else {
            assert_eq!(error, GraphLayoutError::UnsatisfiedRankConstraint(edge));
        }
    }
}

#[test]
fn empty_graph_still_provides_checked_layered_capability() {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::East,
        DIRECTED_GRAPH,
        DirectedGraphConfig::default(),
        &mut ctx,
    )
    .unwrap();
    builder.require(LAYERED_RESULT, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    let result = service().solve(&request, &mut context()).unwrap();
    assert!(
        result
            .capability(LAYERED_RESULT)
            .unwrap()
            .unwrap()
            .nodes
            .is_empty()
    );
}

#[test]
fn east_offsets_keep_source_distance_even_beyond_transposed_side_length() {
    let fixture =
        fixtures::directed_fixture("offsetTest", GraphDirection::East, &mut context()).unwrap();
    let result = service().solve(&fixture.request, &mut context()).unwrap();
    let edge = &fixture.request.edges()[1];
    let route = result.route(edge.id()).unwrap();
    let bounds = result.bounds(edge.source).unwrap();
    assert_eq!(route.source.offset, AttachmentOffset::Distance(90.0));
    assert_eq!(route.source.position.y(), bounds.y + 90.0);
    assert!(route.source.position.y() > bounds.y + bounds.height);
}

#[test]
fn explicit_row_order_survives_crossing_reduction_and_transposition() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder = GraphRequestBuilder::new(
            direction,
            DIRECTED_GRAPH,
            DirectedGraphConfig::default(),
            &mut ctx,
        )
        .unwrap();
        let mut nodes = Vec::new();
        for _ in 0..4 {
            nodes.push(
                builder
                    .add_node(
                        GraphSize::directed(Dimension::new(30.0, 20.0), direction),
                        &mut ctx,
                    )
                    .unwrap(),
            );
        }
        for (index, constraint) in [(0, 2), (1, 1), (2, 1), (3, 2)] {
            builder
                .node_mut(nodes[index])
                .unwrap()
                .constraints
                .insert(LAYERED_ROW, LayeredRowConstraint(constraint), &mut ctx)
                .unwrap();
        }
        for (source, target) in [(0, 2), (1, 3), (0, 3)] {
            builder
                .add_edge(nodes[source], nodes[target], &mut ctx)
                .unwrap();
        }
        let request = builder.build(&mut ctx).unwrap();
        let result = service().solve(&request, &mut context()).unwrap();
        let cross = |index| {
            let bounds = result.bounds(nodes[index]).unwrap();
            if direction == GraphDirection::South {
                bounds.x
            } else {
                bounds.y
            }
        };
        assert!(cross(1) < cross(0));
        assert!(cross(2) < cross(3));
    }
}

#[test]
fn layered_evidence_cannot_forge_ranks_or_virtual_geometry() {
    let request = request(GraphDirection::South, false);
    let service = service();
    let result = service.solve(&request, &mut context()).unwrap();
    for modification in 0..3 {
        let mut candidate = GraphLayoutCandidate::new(&request);
        candidate.placements = result.placements().to_vec();
        candidate.routes = result.routes().to_vec();
        candidate.extent = result.extent();
        let mut evidence = result.capability(LAYERED_RESULT).unwrap().unwrap().clone();
        match modification {
            0 => evidence.nodes[0].rank += 1,
            1 => {
                evidence
                    .edges
                    .iter_mut()
                    .find(|edge| !edge.virtual_nodes.is_empty())
                    .unwrap()
                    .virtual_nodes[0]
                    .bounds
                    .y += 1.0
            }
            _ => {
                for node in &mut evidence.nodes {
                    node.rank += 1;
                }
                for edge in &mut evidence.edges {
                    edge.source_rank += 1;
                    edge.target_rank += 1;
                    for node in &mut edge.virtual_nodes {
                        node.rank += 1;
                    }
                }
            }
        }
        candidate
            .capabilities
            .insert(LAYERED_RESULT, evidence, &mut context())
            .unwrap();
        assert!(
            GraphLayoutResult::validate(&request, candidate, &service, &mut context()).is_err()
        );
    }
}

#[test]
fn fractional_padding_keeps_padded_rows_disjoint() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder = GraphRequestBuilder::new(
            direction,
            DIRECTED_GRAPH,
            DirectedGraphConfig::default(),
            &mut ctx,
        )
        .unwrap();
        let height = 1.9148622692548622;
        let size = if direction == GraphDirection::South {
            Dimension::new(10.0, height)
        } else {
            Dimension::new(height, 10.0)
        };
        let a = builder
            .add_node(GraphSize::directed(size, direction), &mut ctx)
            .unwrap();
        let b = builder
            .add_node(GraphSize::directed(size, direction), &mut ctx)
            .unwrap();
        let pads = [
            (1.7017505035892777, 1.6404399741374223),
            (0.7517165713528977, 0.0),
        ];
        for (node, (top, bottom)) in [a, b].into_iter().zip(pads) {
            builder.node_mut(node).unwrap().padding = Some(if direction == GraphDirection::South {
                Insets::new(top, 0.0, bottom, 0.0)
            } else {
                Insets::new(0.0, top, 0.0, bottom)
            });
        }
        builder.add_edge(a, b, &mut ctx).unwrap();
        let request = builder.build(&mut ctx).unwrap();
        service().solve(&request, &mut context()).unwrap();
    }
}

#[test]
fn maximum_rank_zero_span_rejects_auxiliary_budget_without_integer_panic() {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::South,
        DIRECTED_GRAPH,
        DirectedGraphConfig::default(),
        &mut ctx,
    )
    .unwrap();
    let mut nodes = Vec::new();
    for _ in 0..3 {
        nodes.push(
            builder
                .add_node(
                    GraphSize::directed(Dimension::new(1.0, 1.0), GraphDirection::South),
                    &mut ctx,
                )
                .unwrap(),
        );
    }
    for (source, target, span) in [(0, 1, 0), (2, 0, u32::MAX)] {
        let edge = builder
            .add_edge(nodes[source], nodes[target], &mut ctx)
            .unwrap();
        builder
            .edge_mut(edge)
            .unwrap()
            .constraints
            .insert(
                LAYERED_EDGE,
                LayeredEdgeConstraint {
                    minimum_span: span,
                    weight: 1,
                },
                &mut ctx,
            )
            .unwrap();
    }
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        service().solve(&request, &mut context()),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Ordering,
            allocation: true,
            ..
        })
    ));
}

#[test]
fn fractional_long_edge_stubs_reach_actual_padded_obstacle_boundary() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let original = request(direction, false);
        let mut ctx = context();
        let mut builder = GraphRequestBuilder::from_request(&original, &mut ctx).unwrap();
        for node in original.nodes() {
            let value = builder.node_mut(node.id()).unwrap();
            value.size =
                GraphSize::directed(Dimension::new(10.721433, 1.9148622692548622), direction);
            value.padding = Some(Insets::new(
                1.7017505035892777,
                0.303342,
                1.6404399741374223,
                0.701355,
            ));
        }
        let request = builder.build(&mut ctx).unwrap();
        service().solve(&request, &mut context()).unwrap();
    }
}
