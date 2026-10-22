//! An external, non-layered consumer of the graph protocol. No Core-private API is used.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use novadraw::geometry::{Dimension, Insets, Point, Rectangle};
use novadraw::layout::graph::*;

struct Linear {
    gap: f64,
}
impl GraphAlgorithmPayload for Linear {}
const LINEAR: GraphAlgorithmKey<Linear> = GraphAlgorithmKey::new("consumer.linear");

struct Other;
impl GraphAlgorithmPayload for Other {}
// Deliberately equal diagnostic name, distinct type identity.
const OTHER: GraphAlgorithmKey<Other> = GraphAlgorithmKey::new("consumer.linear");

struct Proof(bool);
impl GraphCapabilityPayload for Proof {}
const PROOF: GraphCapabilityKey<Proof> = GraphCapabilityKey::new("consumer.proof");

struct Demand(bool);
impl GraphConstraintPayload for Demand {}
const DEMAND: GraphConstraintKey<Demand> = GraphConstraintKey::new("consumer.demand");

fn context() -> GraphSolveContext {
    GraphSolveContext::new(GraphBudget::new(1_000_000, 100_000))
}

fn linear(
    payload: &Linear,
    request: &GraphLayoutRequest,
    context: &mut GraphSolveContext,
) -> Result<GraphLayoutCandidate, GraphLayoutError> {
    let mut candidate = GraphLayoutCandidate::new(request);
    context.allocate(GraphStage::Placement, request.nodes().len() as u64)?;
    let mut x = 0.0;
    for node in request.nodes() {
        let size = node.size.preferred();
        candidate.placements.push(GraphPlacement {
            entity: node.id().into(),
            bounds: Rectangle::new(x, 0.0, size.width, size.height),
        });
        x += size.width + payload.gap;
    }
    context.allocate(GraphStage::Routing, request.edges().len() as u64)?;
    for edge in request.edges() {
        let source_bounds = candidate
            .placements
            .iter()
            .find(|placement| placement.entity == edge.source)
            .unwrap()
            .bounds;
        let target_bounds = candidate
            .placements
            .iter()
            .find(|placement| placement.entity == edge.target)
            .unwrap()
            .bounds;
        let source = PreparedGraphAttachment::on_bounds(
            edge.source,
            source_bounds,
            AttachmentSide::Right,
            GraphAttachment::default(),
            0.0,
        );
        let target = PreparedGraphAttachment::on_bounds(
            edge.target,
            target_bounds,
            AttachmentSide::Left,
            GraphAttachment::default(),
            0.0,
        );
        context.allocate(GraphStage::Routing, 2)?;
        candidate.routes.push(GraphRoute {
            edge: edge.id(),
            source,
            target,
            points: vec![source.position, target.position],
        });
    }
    candidate.extent = Rectangle::new(
        0.0,
        0.0,
        x.max(0.0),
        request
            .nodes()
            .iter()
            .map(|node| node.size.height.preferred)
            .fold(0.0, f64::max),
    );
    Ok(candidate)
}

fn service() -> GraphLayoutService {
    let mut service = GraphLayoutService::new();
    service
        .register(LINEAR, GraphAlgorithmCapabilities::new(), linear)
        .unwrap();
    service
}
fn permissive_service() -> GraphLayoutService {
    let mut service = GraphLayoutService::new();
    let mut capabilities = GraphAlgorithmCapabilities::new();
    capabilities.groups = true;
    capabilities.disconnected = true;
    capabilities.self_loops = true;
    service.register(LINEAR, capabilities, linear).unwrap();
    service
}

fn pair(direction: GraphDirection) -> (GraphLayoutRequest, GraphNodeId, GraphNodeId, GraphEdgeId) {
    let mut context = context();
    let mut builder =
        GraphRequestBuilder::new(direction, LINEAR, Linear { gap: 30.0 }, &mut context).unwrap();
    let a = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut context)
        .unwrap();
    let b = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut context)
        .unwrap();
    let edge = builder.add_edge(a, b, &mut context).unwrap();
    (builder.build(&mut context).unwrap(), a, b, edge)
}
fn candidate(request: &GraphLayoutRequest) -> GraphLayoutCandidate {
    linear(&Linear { gap: 30.0 }, request, &mut context()).unwrap()
}
fn validate(
    request: &GraphLayoutRequest,
    candidate: GraphLayoutCandidate,
) -> Result<GraphLayoutResult, GraphLayoutError> {
    GraphLayoutResult::validate(request, candidate, &permissive_service(), &mut context())
}
fn violation(error: GraphLayoutError, expected: GraphViolation) {
    match error {
        GraphLayoutError::InvalidInput { violation, .. }
        | GraphLayoutError::RejectedCandidate { violation, .. } => assert_eq!(violation, expected),
        other => panic!("unexpected rejection: {other}"),
    }
}

#[test]
fn external_nonlayered_solver_needs_no_rank_and_is_deterministic() {
    for direction in [GraphDirection::South, GraphDirection::East] {
        let (request, a, b, edge) = pair(direction);
        let service = service();
        let first = service.solve(&request, &mut context()).unwrap();
        let second = service.solve(&request, &mut context()).unwrap();
        assert_eq!(first.placements(), second.placements());
        assert_eq!(first.routes(), second.routes());
        assert_eq!(first.extent(), second.extent());
        assert_eq!(first.direction(), direction);
        assert!(first.capability(PROOF).unwrap().is_none());
        assert_eq!(
            first.bounds(a).unwrap(),
            Rectangle::new(0.0, 0.0, 20.0, 10.0)
        );
        assert_eq!(
            first.bounds(b).unwrap(),
            Rectangle::new(50.0, 0.0, 20.0, 10.0)
        );
        assert_eq!(
            first.route(edge).unwrap().points,
            [Point::new(20.0, 5.0), Point::new(50.0, 5.0)]
        );
        first.check_prefix(&request.clone(), &service).unwrap();
    }
}

#[test]
fn input_snapshot_and_instance_generation_are_both_required() {
    let (request, a, _, _) = pair(GraphDirection::South);
    let service = service();
    let result = service.solve(&request, &mut context()).unwrap();
    let (other, other_a, _, _) = pair(GraphDirection::South);
    assert_eq!(request.identity().revision(), other.identity().revision());
    assert_eq!(
        result.check_prefix(&other, &service),
        Err(GraphLayoutError::RequestMismatch)
    );
    assert_eq!(
        result.check_prefix(&request, &self::service()),
        Err(GraphLayoutError::RegistryMismatch)
    );
    assert!(matches!(
        request.node(other_a),
        Err(GraphLayoutError::UnknownEntity(_))
    ));
    assert_eq!(
        result.bounds(other_a),
        Err(GraphLayoutError::UnknownEntity(other_a.into()))
    );
    assert!(request.node(a).is_ok());
}

#[test]
fn new_measurement_prefix_retains_ids_without_reusing_the_old_certificate() {
    let (request, a, _, _) = pair(GraphDirection::South);
    let service = service();
    let result = service.solve(&request, &mut context()).unwrap();
    let mut ctx = context();
    let mut edit = GraphRequestBuilder::from_request(&request, &mut ctx).unwrap();
    edit.node_mut(a).unwrap().size.width = GraphAxisSize::fixed(30.0);
    edit.revision(1);
    edit.algorithm(LINEAR, Linear { gap: 40.0 }, &mut ctx)
        .unwrap();
    let next = edit.build(&mut ctx).unwrap();
    assert_eq!(request.node(a).unwrap().size.width.preferred, 20.0);
    assert_eq!(next.node(a).unwrap().size.width.preferred, 30.0);
    assert_eq!(next.identity().revision(), 1);
    assert_eq!(
        result.check_prefix(&next, &service),
        Err(GraphLayoutError::RequestMismatch)
    );
    let next_result = service.solve(&next, &mut ctx).unwrap();
    assert_eq!(next_result.bounds(a).unwrap().width, 30.0);
    result.check_prefix(&request, &service).unwrap();

    // An edit with unchanged numeric revision still creates a distinct frozen prefix.
    let next = GraphRequestBuilder::from_request(&request, &mut ctx)
        .unwrap()
        .build(&mut ctx)
        .unwrap();
    assert_eq!(next.identity().revision(), request.identity().revision());
    assert_eq!(
        result.check_prefix(&next, &service),
        Err(GraphLayoutError::RequestMismatch)
    );
}

#[test]
fn typed_identity_ignores_diagnostic_names_and_rejects_duplicate_types() {
    let mut service = service();
    service
        .register(
            OTHER,
            GraphAlgorithmCapabilities::new(),
            |_: &Other, request: &GraphLayoutRequest, _: &mut GraphSolveContext| {
                Ok(GraphLayoutCandidate::new(request))
            },
        )
        .unwrap();
    let request = GraphRequestBuilder::new(GraphDirection::South, OTHER, Other, &mut context())
        .unwrap()
        .build(&mut context())
        .unwrap();
    assert!(
        service
            .solve(&request, &mut context())
            .unwrap()
            .placements()
            .is_empty()
    );
    let duplicate = GraphAlgorithmKey::<Linear>::new("a.different.name");
    assert_eq!(
        service.register(duplicate, GraphAlgorithmCapabilities::new(), linear),
        Err(GraphLayoutError::DuplicateRegistration("a.different.name"))
    );
    let (normal, _, _, _) = pair(GraphDirection::South);
    assert_eq!(
        service
            .solve(&normal, &mut context())
            .unwrap()
            .placements()
            .len(),
        2
    );
}

#[test]
fn constraints_and_required_capabilities_are_checked_before_solve() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx)
        .unwrap();
    builder
        .node_mut(node)
        .unwrap()
        .constraints
        .insert(DEMAND, Demand(false), &mut ctx)
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    let mut service = GraphLayoutService::new();
    let observed = calls.clone();
    service
        .register(
            LINEAR,
            GraphAlgorithmCapabilities::new().constraint(DEMAND),
            move |payload: &Linear, request: &GraphLayoutRequest, ctx: &mut GraphSolveContext| {
                observed.fetch_add(1, Ordering::SeqCst);
                linear(payload, request, ctx)
            },
        )
        .unwrap();
    assert!(matches!(
        service.solve(&request, &mut ctx),
        Err(GraphLayoutError::UnsupportedConstraint(_))
    ));
    service
        .register_constraint(
            DEMAND,
            |value: &Demand,
             _: GraphConstraintOwner,
             _: &GraphLayoutRequest,
             _: &mut GraphSolveContext| {
                if value.0 {
                    Ok(())
                } else {
                    Err(GraphLayoutError::CapabilityRejected(DEMAND.name()))
                }
            },
        )
        .unwrap();
    assert!(matches!(
        service.solve(&request, &mut ctx),
        Err(GraphLayoutError::CapabilityRejected(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    builder.require(PROOF, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        service.solve(&request, &mut ctx),
        Err(GraphLayoutError::UnsupportedCapability(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

fn proof_service(include: bool, valid: bool) -> GraphLayoutService {
    let mut service = GraphLayoutService::new();
    service
        .register_capability(
            PROOF,
            |proof: &Proof,
             _: &GraphLayoutRequest,
             _: &GraphLayoutCandidate,
             ctx: &mut GraphSolveContext| {
                ctx.consume(GraphStage::Capability, 1)?;
                if proof.0 {
                    Ok(())
                } else {
                    Err(GraphLayoutError::CapabilityRejected(PROOF.name()))
                }
            },
        )
        .unwrap();
    service
        .register(
            LINEAR,
            GraphAlgorithmCapabilities::new().result(PROOF),
            move |_: &Linear, request: &GraphLayoutRequest, ctx: &mut GraphSolveContext| {
                let mut result = GraphLayoutCandidate::new(request);
                if include {
                    result.capabilities.insert(PROOF, Proof(valid), ctx)?;
                }
                Ok(result)
            },
        )
        .unwrap();
    service
}

#[test]
fn missing_or_invalid_result_capability_never_becomes_a_checked_result() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    builder.require(PROOF, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        proof_service(false, true).solve(&request, &mut ctx),
        Err(GraphLayoutError::MissingCapability(_))
    ));
    assert!(matches!(
        proof_service(true, false).solve(&request, &mut ctx),
        Err(GraphLayoutError::CapabilityRejected(_))
    ));
    assert!(
        proof_service(true, true)
            .solve(&request, &mut ctx)
            .unwrap()
            .capability(PROOF)
            .unwrap()
            .unwrap()
            .0
    );
}

#[test]
fn structural_admission_rejects_foreign_identity_cycle_and_bad_geometry() {
    let mut ctx = context();
    let (_, foreign, _, _) = pair(GraphDirection::South);
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let own = builder
        .add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx)
        .unwrap();
    builder.add_edge(own, foreign, &mut ctx).unwrap();
    assert!(matches!(
        builder.build(&mut ctx),
        Err(GraphLayoutError::UnknownEntity(_))
    ));

    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let a = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    let b = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    builder.group_mut(a).unwrap().parent = Some(b);
    builder.group_mut(b).unwrap().parent = Some(a);
    assert!(matches!(
        builder.build(&mut ctx),
        Err(GraphLayoutError::ContainmentCycle(_))
    ));

    for value in [-1.0, f64::NAN, f64::INFINITY] {
        let mut builder =
            GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
                .unwrap();
        builder
            .add_node(GraphSize::fixed(Dimension::new(value, 10.0)), &mut ctx)
            .unwrap();
        violation(builder.build(&mut ctx).err().unwrap(), GraphViolation::Size);
    }
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    builder.default_padding(Insets::uniform(f64::MAX));
    violation(
        builder.build(&mut ctx).err().unwrap(),
        GraphViolation::Padding,
    );
}

#[test]
fn candidate_has_exact_entity_and_edge_coverage() {
    let (request, a, _, _) = pair(GraphDirection::South);
    let mut proposed = candidate(&request);
    proposed.placements.pop();
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::IdentityCoverage,
    );
    let mut proposed = candidate(&request);
    proposed.placements[1].entity = a.into();
    assert!(matches!(
        validate(&request, proposed),
        Err(GraphLayoutError::DuplicateEntity(_))
    ));
    let mut proposed = candidate(&request);
    proposed.routes.clear();
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::IdentityCoverage,
    );
    let mut proposed = candidate(&request);
    proposed.request = pair(GraphDirection::South).0.identity();
    assert!(matches!(
        validate(&request, proposed),
        Err(GraphLayoutError::RequestMismatch)
    ));
}

#[test]
fn fixed_axes_and_nonshrinkable_preferred_demand_are_enforced() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::East, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let id = builder
        .add_node(
            GraphSize {
                width: GraphAxisSize::stretch(10.0, 20.0),
                height: GraphAxisSize::fixed(10.0),
            },
            &mut ctx,
        )
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = GraphLayoutCandidate::new(&request);
    proposed.extent = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    proposed.placements.push(GraphPlacement {
        entity: id.into(),
        bounds: Rectangle::new(0.0, 0.0, 40.0, 10.0),
    });
    validate(&request, proposed.clone()).unwrap();
    for (width, height) in [(15.0, 10.0), (9.0, 10.0), (40.0, 11.0)] {
        proposed.placements[0].bounds = Rectangle::new(0.0, 0.0, width, height);
        violation(
            validate(&request, proposed.clone()).err().unwrap(),
            GraphViolation::Size,
        );
    }
}

#[test]
fn nested_groups_allow_ancestors_but_reserve_their_inner_region() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let outer = builder
        .add_group(Dimension::new(100.0, 100.0), &mut ctx)
        .unwrap();
    let inner = builder
        .add_group(Dimension::new(40.0, 40.0), &mut ctx)
        .unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::new(10.0, 10.0)), &mut ctx)
        .unwrap();
    builder.group_mut(outer).unwrap().border_insets = Insets::uniform(5.0);
    builder.group_mut(outer).unwrap().inner_padding = Insets::uniform(5.0);
    builder.group_mut(inner).unwrap().parent = Some(outer);
    builder.group_mut(inner).unwrap().inner_padding = Insets::uniform(5.0);
    builder.node_mut(node).unwrap().parent = Some(inner);
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = GraphLayoutCandidate::new(&request);
    proposed.extent = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    proposed.placements = vec![
        GraphPlacement {
            entity: outer.into(),
            bounds: proposed.extent,
        },
        GraphPlacement {
            entity: inner.into(),
            bounds: Rectangle::new(10.0, 10.0, 40.0, 40.0),
        },
        GraphPlacement {
            entity: node.into(),
            bounds: Rectangle::new(15.0, 15.0, 10.0, 10.0),
        },
    ];
    validate(&request, proposed.clone()).unwrap();
    proposed.placements[2].bounds.x = 12.0;
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Containment,
    );

    // An empty group's border and inner padding still impose a minimum outer size.
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let group = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    builder.group_mut(group).unwrap().inner_padding = Insets::uniform(10.0);
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = GraphLayoutCandidate::new(&request);
    proposed.extent = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    proposed.placements.push(GraphPlacement {
        entity: group.into(),
        bounds: Rectangle::new(0.0, 0.0, 19.0, 20.0),
    });
    violation(
        validate(&request, proposed.clone()).err().unwrap(),
        GraphViolation::Size,
    );
    proposed.placements[0].bounds.width = 20.0;
    validate(&request, proposed).unwrap();
}

#[test]
fn nonmembers_and_external_padding_cannot_overlap_reserved_groups() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let group = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::new(10.0, 10.0)), &mut ctx)
        .unwrap();
    builder.node_mut(node).unwrap().padding = Some(Insets::uniform(5.0));
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = GraphLayoutCandidate::new(&request);
    proposed.extent = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    proposed.placements = vec![
        GraphPlacement {
            entity: group.into(),
            bounds: Rectangle::new(0.0, 0.0, 40.0, 40.0),
        },
        GraphPlacement {
            entity: node.into(),
            bounds: Rectangle::new(45.0, 10.0, 10.0, 10.0),
        },
    ];
    validate(&request, proposed.clone()).unwrap();
    proposed.placements[1].bounds.x = 44.0;
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Overlap,
    );
}

#[test]
fn attachment_geometry_original_direction_and_extent_are_hard_constraints() {
    let (request, _, _, _) = pair(GraphDirection::South);
    let mut proposed = candidate(&request);
    proposed.routes[0].source.position = Point::new(19.0, 5.0);
    proposed.routes[0].points[0] = proposed.routes[0].source.position;
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Attachment,
    );
    let mut proposed = candidate(&request);
    proposed.routes[0].points.reverse();
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Attachment,
    );
    let mut proposed = candidate(&request);
    proposed.routes[0]
        .points
        .insert(1, Point::new(10.0, f64::NAN));
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Geometry,
    );
    let mut proposed = candidate(&request);
    proposed.extent.width = 30.0;
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Extent,
    );
}

#[test]
fn unrelated_obstacle_clearance_and_endpoint_reentry_are_rejected() {
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::South,
        LINEAR,
        Linear { gap: 30.0 },
        &mut ctx,
    )
    .unwrap();
    let a = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
        .unwrap();
    let obstacle = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
        .unwrap();
    let b = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
        .unwrap();
    builder.add_edge(a, b, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = candidate(&request);
    violation(
        validate(&request, proposed.clone()).err().unwrap(),
        GraphViolation::Clearance,
    );
    proposed.routes[0].points = vec![
        proposed.routes[0].source.position,
        Point::new(25.0, 5.0),
        Point::new(25.0, 20.0),
        Point::new(95.0, 20.0),
        Point::new(95.0, 5.0),
        proposed.routes[0].target.position,
    ];
    proposed.extent.height = 20.0;
    validate(&request, proposed.clone()).unwrap();
    proposed.routes[0].points.insert(2, Point::new(10.0, 5.0));
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Clearance,
    );
    assert!(request.node(obstacle).is_ok());
}

#[test]
fn connectivity_groups_and_selfloop_admission_are_declared_per_algorithm() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let a = builder
        .add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx)
        .unwrap();
    builder
        .add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx)
        .unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        service().solve(&request, &mut ctx),
        Err(GraphLayoutError::Disconnected)
    ));

    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx)
        .unwrap();
    let edge = builder.add_edge(node, node, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(
        matches!(service().solve(&request, &mut ctx), Err(GraphLayoutError::UnsupportedSelfLoop(id)) if id == edge)
    );
    assert!(request.node(a).is_err());

    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    assert!(matches!(
        service().solve(&request, &mut ctx),
        Err(GraphLayoutError::UnsupportedGroups)
    ));
}

#[test]
fn single_budget_covers_allocations_admission_and_candidate_checking() {
    let mut ctx = GraphSolveContext::new(GraphBudget::new(100, 1));
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    assert!(matches!(
        builder.add_node(GraphSize::fixed(Dimension::ZERO), &mut ctx),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Request,
            allocation: true,
            ..
        })
    ));
    let (request, _, _, _) = pair(GraphDirection::South);
    let mut ctx = GraphSolveContext::new(GraphBudget::new(1, 100));
    assert!(matches!(
        service().solve(&request, &mut ctx),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Request,
            ..
        })
    ));
    let mut ctx = GraphSolveContext::new(GraphBudget::new(100_000, 0));
    assert!(matches!(
        service().solve(&request, &mut ctx),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Connectivity,
            allocation: true,
            ..
        })
    ));
    let mut ctx = GraphSolveContext::new(GraphBudget::new(u64::MAX, u64::MAX));
    ctx.consume(GraphStage::Algorithm, u64::MAX).unwrap();
    assert_eq!(
        ctx.consume(GraphStage::Result, 1),
        Err(GraphLayoutError::Overflow(GraphStage::Result))
    );
    let mut ctx = GraphSolveContext::new(GraphBudget::new(15, 100));
    assert!(matches!(
        GraphLayoutResult::validate(
            &request,
            candidate(&request),
            &permissive_service(),
            &mut ctx
        ),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Result,
            ..
        })
    ));
}

#[test]
fn node_attachment_must_exit_the_selected_side_and_selfloops_need_a_full_path() {
    let (request, _, _, _) = pair(GraphDirection::South);
    let mut proposed = candidate(&request);
    proposed.routes[0].points.insert(1, Point::new(10.0, 5.0));
    violation(
        validate(&request, proposed).err().unwrap(),
        GraphViolation::Attachment,
    );

    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut ctx)
        .unwrap();
    builder.add_edge(node, node, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    violation(
        validate(&request, candidate(&request)).err().unwrap(),
        GraphViolation::Clearance,
    );
}

#[test]
fn containment_depth_and_traversal_work_are_bounded_before_solving() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let mut groups = Vec::new();
    for _ in 0..10_002 {
        groups.push(builder.add_group(Dimension::ZERO, &mut ctx).unwrap());
    }
    for adjacent in groups.windows(2) {
        builder.group_mut(adjacent[0]).unwrap().parent = Some(adjacent[1]);
    }
    assert!(matches!(
        builder.build(&mut ctx),
        Err(GraphLayoutError::DepthExceeded)
    ));

    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let a = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    let b = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    let c = builder.add_group(Dimension::ZERO, &mut ctx).unwrap();
    builder.group_mut(a).unwrap().parent = Some(b);
    builder.group_mut(b).unwrap().parent = Some(c);
    let mut tiny = GraphSolveContext::new(GraphBudget::new(3, 100));
    assert!(matches!(
        builder.build(&mut tiny),
        Err(GraphLayoutError::BudgetExceeded {
            stage: GraphStage::Containment,
            ..
        })
    ));
}

#[test]
fn draw2d_attachment_offset_can_exceed_the_side_length_after_transposition() {
    let (request, a, b, edge) = pair(GraphDirection::East);
    let mut ctx = context();
    let mut builder = GraphRequestBuilder::from_request(&request, &mut ctx).unwrap();
    builder.node_mut(a).unwrap().outgoing.offset = AttachmentOffset::Distance(90.0);
    builder.node_mut(b).unwrap().incoming.offset = AttachmentOffset::Distance(90.0);
    let request = builder.build(&mut ctx).unwrap();
    let mut proposed = candidate(&request);
    let source = PreparedGraphAttachment::on_bounds(
        a.into(),
        proposed.placements[0].bounds,
        AttachmentSide::Right,
        request.node(a).unwrap().outgoing,
        0.0,
    );
    let target = PreparedGraphAttachment::on_bounds(
        b.into(),
        proposed.placements[1].bounds,
        AttachmentSide::Left,
        request.node(b).unwrap().incoming,
        0.0,
    );
    proposed.routes[0] = GraphRoute {
        edge,
        source,
        target,
        points: vec![source.position, target.position],
    };
    violation(
        validate(&request, proposed.clone()).err().unwrap(),
        GraphViolation::Extent,
    );
    proposed.extent.height = 90.0;
    validate(&request, proposed).unwrap();
}

struct BoundaryProof(Rectangle);
impl GraphCapabilityPayload for BoundaryProof {}
const BOUNDARY_PROOF: GraphCapabilityKey<BoundaryProof> =
    GraphCapabilityKey::new("consumer.selected-boundary");

#[test]
fn external_selected_boundary_uses_the_same_geometry_and_registered_proof_gate() {
    let mut ctx = context();
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, LINEAR, Linear { gap: 0.0 }, &mut ctx)
            .unwrap();
    let group = builder
        .add_group(Dimension::new(100.0, 100.0), &mut ctx)
        .unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::new(10.0, 10.0)), &mut ctx)
        .unwrap();
    builder.node_mut(node).unwrap().parent = Some(group);
    let edge = builder.add_edge(group, node, &mut ctx).unwrap();
    let request = builder.build(&mut ctx).unwrap();
    let boundary = Rectangle::new(20.0, 10.0, 5.0, 20.0);
    let mut service = GraphLayoutService::new();
    let mut capabilities = GraphAlgorithmCapabilities::new().result(BOUNDARY_PROOF);
    capabilities.groups = true;
    service.register(LINEAR, capabilities, linear).unwrap();
    service
        .register_capability(
            BOUNDARY_PROOF,
            |proof: &BoundaryProof,
             _: &GraphLayoutRequest,
             candidate: &GraphLayoutCandidate,
             ctx: &mut GraphSolveContext| {
                ctx.consume(GraphStage::Capability, 1)?;
                if candidate.routes[0].source.boundary.is_some_and(|boundary| {
                    boundary.is_certified_by(BOUNDARY_PROOF) && boundary.bounds() == proof.0
                }) {
                    Ok(())
                } else {
                    Err(GraphLayoutError::CapabilityRejected(BOUNDARY_PROOF.name()))
                }
            },
        )
        .unwrap();
    let mut candidate = GraphLayoutCandidate::new(&request);
    let outer = Rectangle::new(0.0, 0.0, 100.0, 100.0);
    let node_bounds = Rectangle::new(20.0, 50.0, 10.0, 10.0);
    candidate.placements = vec![
        GraphPlacement {
            entity: group.into(),
            bounds: outer,
        },
        GraphPlacement {
            entity: node.into(),
            bounds: node_bounds,
        },
    ];
    let source = PreparedGraphAttachment::on_boundary(
        group.into(),
        GraphAttachmentBoundary::new(boundary, BOUNDARY_PROOF),
        AttachmentSide::Bottom,
        GraphAttachment::default(),
        15.0,
    );
    let target = PreparedGraphAttachment::on_bounds(
        node.into(),
        node_bounds,
        AttachmentSide::Top,
        GraphAttachment::default(),
        0.0,
    );
    candidate.routes = vec![GraphRoute {
        edge,
        source,
        target,
        points: vec![source.position, target.position],
    }];
    candidate.extent = outer;
    assert!(matches!(
        GraphLayoutResult::validate(&request, candidate.clone(), &service, &mut ctx),
        Err(GraphLayoutError::MissingCapability(_))
    ));
    candidate
        .capabilities
        .insert(BOUNDARY_PROOF, BoundaryProof(boundary), &mut ctx)
        .unwrap();
    let result =
        GraphLayoutResult::validate(&request, candidate.clone(), &service, &mut ctx).unwrap();
    assert_eq!(
        result.route(edge).unwrap().source.position,
        Point::new(22.5, 15.0)
    );
    // Geometry is valid inside the owner, but its registered provenance does not match.
    let wrong = Rectangle::new(30.0, 10.0, 5.0, 20.0);
    candidate.routes[0].source = PreparedGraphAttachment::on_boundary(
        group.into(),
        GraphAttachmentBoundary::new(wrong, BOUNDARY_PROOF),
        AttachmentSide::Bottom,
        GraphAttachment::default(),
        15.0,
    );
    candidate.routes[0].points[0] = candidate.routes[0].source.position;
    assert!(matches!(
        GraphLayoutResult::validate(&request, candidate, &service, &mut ctx),
        Err(GraphLayoutError::CapabilityRejected(_))
    ));
}
