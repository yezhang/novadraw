use novadraw::geometry::{Dimension, Insets};
use novadraw::layout::graph::*;

#[path = "../../examples/scenes/src/compound_fixtures.rs"]
mod fixtures;

fn context() -> GraphSolveContext {
    GraphSolveContext::new(GraphBudget::new(200_000_000, 20_000_000))
}
fn service() -> GraphLayoutService {
    let mut service = GraphLayoutService::new();
    register_layered_protocol(&mut service).unwrap();
    register_compound_graph(&mut service).unwrap();
    service
}

#[test]
fn official_compound_fixtures_cover_both_directions() {
    let service = service();
    let counts = [
        (7, 2, 7),
        (3, 4, 3),
        (10, 4, 10),
        (37, 9, 38),
        (9, 2, 6),
        (4, 2, 3),
        (5, 1, 7),
        (8, 4, 7),
        (7, 2, 7),
        (8, 3, 10),
    ];
    for (name, counts) in fixtures::COMPOUND_FIXTURES.iter().zip(counts) {
        for direction in [GraphDirection::South, GraphDirection::East] {
            let fixture = fixtures::compound_fixture(name, direction, &mut context()).unwrap();
            assert_eq!(
                fixture.labels.len(),
                fixture.request.nodes().len() + fixture.request.groups().len()
            );
            assert_eq!(
                (
                    fixture.request.nodes().len(),
                    fixture.request.groups().len(),
                    fixture.request.edges().len()
                ),
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

#[test]
fn empty_group_respects_minimum_and_reserved_padding() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder =
            GraphRequestBuilder::new(direction, COMPOUND_GRAPH, CompoundGraphConfig, &mut ctx)
                .unwrap();
        let group = builder
            .add_group(Dimension::new(180.0, 120.0), &mut ctx)
            .unwrap();
        builder.group_mut(group).unwrap().border_insets = Insets::uniform(3.0);
        builder.group_mut(group).unwrap().inner_padding = Insets::uniform(5.0);
        let request = builder.build(&mut ctx).unwrap();
        let result = service().solve(&request, &mut ctx).unwrap();
        let bounds = result.bounds(group).unwrap();
        assert!(bounds.width >= 180.0 && bounds.height >= 120.0);
    }
}

fn proposal(request: &GraphLayoutRequest, result: &GraphLayoutResult) -> GraphLayoutCandidate {
    let mut candidate = GraphLayoutCandidate::new(request);
    candidate.placements = result.placements().to_vec();
    candidate.routes = result.routes().to_vec();
    candidate.extent = result.extent();
    candidate
        .capabilities
        .insert(
            LAYERED_RESULT,
            result.capability(LAYERED_RESULT).unwrap().unwrap().clone(),
            &mut context(),
        )
        .unwrap();
    candidate
}

#[test]
fn ancestor_edges_use_independent_boundaries_and_keep_original_direction() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder =
            GraphRequestBuilder::new(direction, COMPOUND_GRAPH, CompoundGraphConfig, &mut ctx)
                .unwrap();
        builder.default_padding(Insets::uniform(4.0));
        let group = builder
            .add_group(Dimension::new(120.0, 120.0), &mut ctx)
            .unwrap();
        builder.group_mut(group).unwrap().border_insets = Insets::uniform(3.0);
        let node = builder
            .add_node(
                GraphSize::directed(Dimension::new(40.0, 30.0), direction),
                &mut ctx,
            )
            .unwrap();
        builder.node_mut(node).unwrap().parent = Some(group);
        let out = builder.add_edge(group, node, &mut ctx).unwrap();
        let back = builder.add_edge(node, group, &mut ctx).unwrap();
        let request = builder.build(&mut ctx).unwrap();
        let service = service();
        let result = service.solve(&request, &mut ctx).unwrap();
        let evidence = result.capability(LAYERED_RESULT).unwrap().unwrap();
        assert!(evidence.edges.iter().all(|edge| !edge.feedback));
        let head = evidence.groups[0].head_bounds;
        let tail = evidence.groups[0].tail_bounds;
        assert_eq!(
            result.route(out).unwrap().source.boundary.unwrap().bounds(),
            head
        );
        assert_eq!(
            result
                .route(back)
                .unwrap()
                .target
                .boundary
                .unwrap()
                .bounds(),
            tail
        );
        let outer = result.bounds(group).unwrap();
        let head_side = match direction {
            GraphDirection::South => (
                result.route(out).unwrap().source.position.y(),
                outer.y + 3.0,
            ),
            GraphDirection::East => (
                result.route(out).unwrap().source.position.x(),
                outer.x + 3.0,
            ),
        };
        assert_eq!(head_side.0, head_side.1);
        let mut tampered = proposal(&request, &result);
        let route = tampered
            .routes
            .iter_mut()
            .find(|route| route.edge == out)
            .unwrap();
        let source = PreparedGraphAttachment::on_boundary(
            group.into(),
            GraphAttachmentBoundary::new(tail, LAYERED_RESULT),
            route.source.side,
            request.group(group).unwrap().outgoing,
            0.0,
        );
        route.source = source;
        route.points[0] = source.position;
        assert!(GraphLayoutResult::validate(&request, tampered, &service, &mut ctx).is_err());
        let mut missing = proposal(&request, &result);
        missing.capabilities = GraphResultCapabilities::new();
        assert!(matches!(
            GraphLayoutResult::validate(&request, missing, &service, &mut ctx),
            Err(GraphLayoutError::MissingCapability(_))
        ));
    }
}

#[test]
fn compound_cycles_reverse_crossing_edges_without_reversing_containment() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder =
            GraphRequestBuilder::new(direction, COMPOUND_GRAPH, CompoundGraphConfig, &mut ctx)
                .unwrap();
        builder.default_padding(Insets::uniform(8.0));
        let a = builder
            .add_group(Dimension::new(70.0, 60.0), &mut ctx)
            .unwrap();
        let b = builder
            .add_group(Dimension::new(70.0, 60.0), &mut ctx)
            .unwrap();
        let a1 = builder
            .add_node(
                GraphSize::directed(Dimension::new(30.0, 20.0), direction),
                &mut ctx,
            )
            .unwrap();
        let b1 = builder
            .add_node(
                GraphSize::directed(Dimension::new(30.0, 20.0), direction),
                &mut ctx,
            )
            .unwrap();
        builder.node_mut(a1).unwrap().parent = Some(a);
        builder.node_mut(b1).unwrap().parent = Some(b);
        builder.add_edge(a1, b1, &mut ctx).unwrap();
        builder.add_edge(b1, a1, &mut ctx).unwrap();
        builder.add_edge(a, b, &mut ctx).unwrap();
        builder.add_edge(b, a, &mut ctx).unwrap();
        let containment = builder.add_edge(a, a1, &mut ctx).unwrap();
        let request = builder.build(&mut ctx).unwrap();
        let result = service().solve(&request, &mut ctx).unwrap();
        let evidence = result.capability(LAYERED_RESULT).unwrap().unwrap();
        assert!(evidence.edges.iter().any(|edge| edge.feedback));
        assert!(
            !evidence
                .edges
                .iter()
                .find(|edge| edge.edge == containment)
                .unwrap()
                .feedback
        );
        for edge in request.edges() {
            let route = result.route(edge.id()).unwrap();
            assert_eq!(route.source.endpoint, edge.source);
            assert_eq!(route.target.endpoint, edge.target);
            assert_eq!(route.points.first(), Some(&route.source.position));
            assert_eq!(route.points.last(), Some(&route.target.position));
        }
    }
}

#[test]
fn group_row_constraints_order_whole_sibling_blocks() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let fixture =
            fixtures::compound_fixture("tangledSubgraphs", direction, &mut context()).unwrap();
        let mut ctx = context();
        let mut builder = GraphRequestBuilder::from_request(&fixture.request, &mut ctx).unwrap();
        for (i, group) in fixture.request.groups().iter().enumerate() {
            builder
                .group_mut(group.id())
                .unwrap()
                .constraints
                .insert(LAYERED_ROW, LayeredRowConstraint((3 - i) as u32), &mut ctx)
                .unwrap();
        }
        let request = builder.build(&mut ctx).unwrap();
        let result = service().solve(&request, &mut ctx).unwrap();
        let bounds: Vec<_> = request
            .groups()
            .iter()
            .map(|group| result.bounds(group.id()).unwrap())
            .collect();
        for pair in bounds.windows(2) {
            let cross = |bounds: novadraw::geometry::Rectangle| match direction {
                GraphDirection::South => bounds.x,
                GraphDirection::East => bounds.y,
            };
            assert!(cross(pair[0]) > cross(pair[1]));
        }
    }
}

#[test]
fn nested_fractional_geometry_keeps_exact_padded_boundaries() {
    for name in fixtures::COMPOUND_FIXTURES {
        for direction in [GraphDirection::South, GraphDirection::East] {
            let fixture = fixtures::compound_fixture(name, direction, &mut context()).unwrap();
            let mut ctx = context();
            let mut builder =
                GraphRequestBuilder::from_request(&fixture.request, &mut ctx).unwrap();
            builder.default_padding(Insets::new(1.7, 2.3, 3.1, 4.9));
            builder.margin(Insets::new(1.1, 2.9, 3.7, 4.3));
            for node in fixture.request.nodes() {
                builder.node_mut(node.id()).unwrap().size = GraphSize::directed(
                    Dimension::new(
                        node.size.width.preferred + 0.1,
                        node.size.height.preferred + 0.3,
                    ),
                    direction,
                );
            }
            for group in fixture.request.groups() {
                let value = builder.group_mut(group.id()).unwrap();
                value.border_insets = Insets::new(1.3, 2.1, 3.7, 4.3);
                value.inner_padding = Insets::new(2.9, 3.1, 4.7, 5.3);
            }
            let request = builder.build(&mut ctx).unwrap();
            service()
                .solve(&request, &mut ctx)
                .unwrap_or_else(|error| panic!("{name} {direction:?}: {error:?}"));
        }
    }
}

#[test]
fn budget_and_fixed_rank_height_conflicts_are_structured_errors() {
    let fixture =
        fixtures::compound_fixture("ShortestPassCase", GraphDirection::South, &mut context())
            .unwrap();
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::from_request(&fixture.request, &mut ctx).unwrap();
    let a = fixture.request.nodes()[0].id();
    builder.node_mut(a).unwrap().size = GraphSize::fixed(Dimension::new(50.0, 10.0));
    let request = builder.build(&mut ctx).unwrap();
    assert!(
        matches!(service().solve(&request,&mut ctx),Err(GraphLayoutError::UnsatisfiedSizeConstraint(id)) if id == a.into())
    );
    let mut tiny = GraphSolveContext::new(GraphBudget::new(300, 100_000));
    assert!(matches!(
        service().solve(&fixture.request, &mut tiny),
        Err(GraphLayoutError::BudgetExceeded { .. })
    ));
}

#[test]
fn nested_group_endpoints_convert_source_before_target() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let mut ctx = context();
        let mut builder =
            GraphRequestBuilder::new(direction, COMPOUND_GRAPH, CompoundGraphConfig, &mut ctx)
                .unwrap();
        builder.default_padding(Insets::uniform(8.0));
        let outer = builder
            .add_group(Dimension::new(100.0, 100.0), &mut ctx)
            .unwrap();
        let inner = builder
            .add_group(Dimension::new(60.0, 60.0), &mut ctx)
            .unwrap();
        builder.group_mut(inner).unwrap().parent = Some(outer);
        let node = builder
            .add_node(
                GraphSize::directed(Dimension::new(30.0, 20.0), direction),
                &mut ctx,
            )
            .unwrap();
        builder.node_mut(node).unwrap().parent = Some(inner);
        let down = builder.add_edge(outer, inner, &mut ctx).unwrap();
        let up = builder.add_edge(inner, outer, &mut ctx).unwrap();
        builder.add_edge(outer, node, &mut ctx).unwrap();
        builder.add_edge(node, outer, &mut ctx).unwrap();
        let request = builder.build(&mut ctx).unwrap();
        let result = service().solve(&request, &mut ctx).unwrap();
        let evidence = result.capability(LAYERED_RESULT).unwrap().unwrap();
        let outer_evidence = evidence
            .groups
            .iter()
            .find(|group| group.group == outer)
            .unwrap();
        let inner_evidence = evidence
            .groups
            .iter()
            .find(|group| group.group == inner)
            .unwrap();
        let route = result.route(down).unwrap();
        assert_eq!(
            route.source.boundary.unwrap().bounds(),
            outer_evidence.head_bounds
        );
        assert_eq!(
            route.target.boundary.unwrap().bounds(),
            inner_evidence.head_bounds
        );
        let route = result.route(up).unwrap();
        assert_eq!(
            route.source.boundary.unwrap().bounds(),
            inner_evidence.tail_bounds
        );
        assert_eq!(
            route.target.boundary.unwrap().bounds(),
            outer_evidence.tail_bounds
        );
        assert!(evidence.edges.iter().all(|edge| !edge.feedback));
    }
}

#[test]
fn zero_span_is_allowed_and_selfloop_admission_remains_explicit() {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::South,
        COMPOUND_GRAPH,
        CompoundGraphConfig,
        &mut ctx,
    )
    .unwrap();
    let group = builder
        .add_group(Dimension::new(50.0, 40.0), &mut ctx)
        .unwrap();
    let a = builder
        .add_node(
            GraphSize::directed(Dimension::new(20.0, 20.0), GraphDirection::South),
            &mut ctx,
        )
        .unwrap();
    let b = builder
        .add_node(
            GraphSize::directed(Dimension::new(20.0, 20.0), GraphDirection::South),
            &mut ctx,
        )
        .unwrap();
    builder.node_mut(a).unwrap().parent = Some(group);
    builder.node_mut(b).unwrap().parent = Some(group);
    let edge = builder.add_edge(a, b, &mut ctx).unwrap();
    builder
        .edge_mut(edge)
        .unwrap()
        .constraints
        .insert(
            LAYERED_EDGE,
            LayeredEdgeConstraint {
                minimum_span: 0,
                weight: 0,
            },
            &mut ctx,
        )
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    service().solve(&request, &mut ctx).unwrap();
    let mut builder = GraphRequestBuilder::from_request(&request, &mut ctx).unwrap();
    let loop_edge = builder.add_edge(group, group, &mut ctx).unwrap();
    builder
        .edge_mut(loop_edge)
        .unwrap()
        .constraints
        .insert(
            LAYERED_EDGE,
            LayeredEdgeConstraint {
                minimum_span: 0,
                weight: 0,
            },
            &mut ctx,
        )
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(
        matches!(service().solve(&request,&mut ctx),Err(GraphLayoutError::UnsupportedSelfLoop(id)) if id == loop_edge)
    );
}
