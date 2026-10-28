use novadraw::{
    connection::*,
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
        FigureMeasurement, FigurePreparation, FigurePresentation, GraphProjectionFigure,
        MeasureConstraints, PREPARATION, PreparationCapability, PreparationMeasureError,
        PreparationMeasurement, PreparationScene, RectangleFigure, SetGraphProjection,
    },
    geometry::{Dimension, PointList, Rectangle},
    graphics::{GraphicsError, PaintContext},
    layout::graph::*,
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, FramePreparationError, Runtime},
    text::{MeasureContext, TextError},
    tree::FigureId,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

const BUDGET: GraphBudget = GraphBudget::new(1_000_000, 100_000);
#[path = "../../../examples/scenes/src/compound_fixtures.rs"]
mod compound_fixtures;
#[path = "../../../examples/scenes/src/graph_fixtures.rs"]
mod directed_fixtures;
struct Empty;
impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}
struct WidthSensitive(Arc<AtomicBool>);
impl Figure for WidthSensitive {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalWidthSensitive"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for WidthSensitive {
    fn revision(&self) -> u64 {
        u64::from(self.0.load(Ordering::Relaxed))
    }
    fn measure_scene(
        &self,
        _: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        if self.0.load(Ordering::Relaxed) {
            return Err(PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: novadraw::figure::PreparationMeasureErrorKind::Text(Box::new(
                    TextError::InvalidMetric,
                )),
            });
        }
        let height = if constraints.max_width().is_some_and(|width| width >= 200.0) {
            20.0
        } else {
            60.0
        };
        Ok(PreparationMeasurement {
            preferred: FigureMeasurement::new(80.0, height, None),
            minimum: Dimension::new(10.0, 10.0),
        })
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        FigurePresentation::new(
            FigureMeasurement::new(bounds.width, bounds.height, None),
            Dimension::new(10.0, 10.0),
            bounds,
            Arc::new(Empty),
        )
    }
}

struct Fixture {
    runtime: Runtime,
    owner: FigureId,
    request: GraphLayoutRequest,
    binding: Arc<GraphProjectionBinding>,
    connections: Vec<ConnectionId>,
    fail: Arc<AtomicBool>,
}
fn fixture(dynamic: bool) -> Fixture {
    fixture_in_viewport(dynamic, false)
}
fn fixture_in_viewport(dynamic: bool, viewport: bool) -> Fixture {
    let mut service = GraphLayoutService::new();
    register_layered_protocol(&mut service).unwrap();
    register_directed_graph(&mut service).unwrap();
    let mut solve = GraphSolveContext::new(BUDGET);
    let mut builder = GraphRequestBuilder::new(
        GraphDirection::East,
        DIRECTED_GRAPH,
        DirectedGraphConfig::default(),
        &mut solve,
    )
    .unwrap();
    let a = builder
        .add_node(
            GraphSize::directed(Dimension::new(80.0, 60.0), GraphDirection::East),
            &mut solve,
        )
        .unwrap();
    let b = builder
        .add_node(
            GraphSize::directed(Dimension::new(200.0, 40.0), GraphDirection::East),
            &mut solve,
        )
        .unwrap();
    let c = builder
        .add_node(
            GraphSize::directed(Dimension::new(30.0, 20.0), GraphDirection::East),
            &mut solve,
        )
        .unwrap();
    for source in [a, b] {
        let edge = builder.add_edge(source, c, &mut solve).unwrap();
        builder
            .edge_mut(edge)
            .unwrap()
            .constraints
            .insert(LAYERED_EDGE, LayeredEdgeConstraint::default(), &mut solve)
            .unwrap();
    }
    let request = builder.build(&mut solve).unwrap();
    let mut runtime = Runtime::empty();
    let graph = Box::new(GraphProjectionFigure::new(Rectangle::new(
        0.0, 0.0, 600.0, 400.0,
    )));
    let owner = if viewport {
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)))
            .unwrap();
        let viewport = runtime
            .viewports()
            .add(root, Rectangle::new(10.0, 20.0, 100.0, 80.0))
            .unwrap()
            .figure_id();
        runtime
            .viewports()
            .set_tracks_width(viewport, true)
            .unwrap();
        runtime
            .viewports()
            .set_tracks_height(viewport, true)
            .unwrap();
        runtime.viewports().set_contents(viewport, graph).unwrap()
    } else {
        runtime.set_contents(graph).unwrap()
    };
    let fail = Arc::new(AtomicBool::new(false));
    let mut entities = Vec::new();
    for node in request.nodes() {
        let figure: Box<dyn Figure> = if dynamic && node.id() == a {
            Box::new(WidthSensitive(fail.clone()))
        } else {
            Box::new(RectangleFigure::new(
                0.0,
                0.0,
                node.size.preferred().width,
                node.size.preferred().height,
            ))
        };
        let id = runtime.figure(owner).unwrap().add(figure).unwrap();
        entities.push((node.id().into(), id));
    }
    let mut edges = Vec::new();
    for edge in request.edges() {
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let dummy = runtime
            .connections()
            .register_router(Box::new(DirectRouter));
        let connection = runtime
            .connections()
            .register_state(
                figure,
                None,
                None,
                RouterBinding::Explicit { router: dummy },
                None,
            )
            .unwrap();
        edges.push((edge.id(), connection));
    }
    let connections = edges.iter().map(|entry| entry.1).collect();
    let binding = Arc::new(
        GraphProjectionBinding::new(owner, &request, entities, edges, &mut solve).unwrap(),
    );
    let router = runtime
        .connections()
        .register_router(Box::new(GraphProjectionRouter::new(binding.clone())));
    for &(edge, connection) in binding.edges() {
        let source = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Source).unwrap(),
        ));
        let target = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Target).unwrap(),
        ));
        runtime
            .connections()
            .set_source(connection, Some(source))
            .unwrap();
        runtime
            .connections()
            .set_target(connection, Some(target))
            .unwrap();
        runtime
            .connections()
            .set_router_binding(connection, RouterBinding::Explicit { router })
            .unwrap();
    }
    let measurements = if dynamic {
        vec![GraphProjectionMeasurement {
            entity: a.into(),
            constraints: MeasureConstraints::new(Some(80.0), None).unwrap(),
        }]
    } else {
        vec![]
    };
    let projection = GraphProjection::new(
        request.clone(),
        Arc::new(service),
        binding.clone(),
        measurements,
        BUDGET,
    )
    .unwrap();
    runtime
        .figure(owner)
        .unwrap()
        .update_component(SetGraphProjection(projection))
        .unwrap();
    Fixture {
        runtime,
        owner,
        request,
        binding,
        connections,
        fail,
    }
}
fn frame(runtime: &mut Runtime) -> Result<(), FramePreparationError> {
    runtime.frames().request_full_redraw();
    match runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 600.0,
            logical_height: 400.0,
            pixel_width: 600,
            pixel_height: 400,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    ) {
        FramePreparation::Ready(frame) => {
            let _ = runtime
                .frames()
                .complete_submission(
                    frame.session_id,
                    frame.frame_id,
                    Ok(RenderDisposition::Presented),
                )
                .unwrap();
            Ok(())
        }
        FramePreparation::Error(error) => Err(error),
        other => panic!("{other:?}"),
    }
}
#[test]
fn graph_measurement_in_a_tracked_viewport_preserves_extent_and_route_identity_when_scrolled() {
    let mut f = fixture_in_viewport(true, true);
    frame(&mut f.runtime).unwrap();
    let viewport = f.runtime.tree().parent_id(f.owner).unwrap().unwrap();
    let tree = f.runtime.tree();
    let snapshot = tree.prepared_presentation(f.owner).unwrap().unwrap();
    let graph = snapshot.data::<GraphProjectionSnapshot>().unwrap();
    let identity = graph.request().identity();
    let extent = snapshot.minimum_size();
    let bounds = tree.figure_bounds(f.owner).unwrap();
    assert!(bounds.width >= extent.width && bounds.height >= extent.height);
    assert!(bounds.width > 100.0);
    let calculations = f.runtime.connections().routing_stats().route_calculations;
    f.runtime
        .viewports()
        .set_view_location(viewport, 50.0, 30.0)
        .unwrap();
    frame(&mut f.runtime).unwrap();
    assert_eq!(
        f.runtime
            .viewports()
            .handle(viewport)
            .unwrap()
            .view_location()
            .x(),
        50.0
    );
    assert_eq!(
        f.runtime
            .tree()
            .prepared_presentation(f.owner)
            .unwrap()
            .unwrap()
            .data::<GraphProjectionSnapshot>()
            .unwrap()
            .request()
            .identity(),
        identity
    );
    assert_eq!(
        f.runtime.connections().routing_stats().route_calculations,
        calculations
    );
}

#[test]
fn directed_projection_publishes_candidate_bounds_and_complete_routes_together() {
    let mut f = fixture(false);
    frame(&mut f.runtime).unwrap();
    let tree = f.runtime.tree();
    let presentation = tree.prepared_presentation(f.owner).unwrap().unwrap();
    let snapshot = presentation.data::<GraphProjectionSnapshot>().unwrap();
    assert_eq!(snapshot.request().identity(), f.request.identity());
    for &(entity, figure) in f.binding.entities() {
        assert_eq!(
            tree.figure_bounds(figure).unwrap(),
            snapshot.result().bounds(entity).unwrap()
        );
    }
    for &(edge, connection) in f.binding.edges() {
        let transform = tree.local_to_parent_transform(connection.figure()).unwrap();
        let points = tree
            .connection_route_points(connection.figure())
            .unwrap()
            .unwrap()
            .iter()
            .map(|point| transform.transform_point(*point))
            .collect::<Vec<_>>();
        assert_eq!(
            PointList::from_points(points),
            PointList::from_points(snapshot.result().route(edge).unwrap().points.clone())
        );
    }
    let identity = snapshot.request().identity();
    let calculations = f.runtime.connections().routing_stats().route_calculations;
    f.runtime.figure(f.owner).unwrap().revalidate().unwrap();
    frame(&mut f.runtime).unwrap();
    assert_eq!(
        f.runtime
            .tree()
            .prepared_presentation(f.owner)
            .unwrap()
            .unwrap()
            .data::<GraphProjectionSnapshot>()
            .unwrap()
            .request()
            .identity(),
        identity
    );
    assert_eq!(
        f.runtime.connections().routing_stats().route_calculations,
        calculations
    );
}
#[test]
fn east_width_feedback_converges_on_first_submission_and_recovers_from_leaf_failure() {
    let mut f = fixture(true);
    frame(&mut f.runtime).unwrap();
    let a = f.request.nodes()[0].id();
    let figure = f.binding.figure(a.into()).unwrap();
    assert_eq!(f.runtime.tree().figure_bounds(figure).unwrap().width, 200.0);
    assert_eq!(f.runtime.tree().figure_bounds(figure).unwrap().height, 20.0);
    let identity = f
        .runtime
        .tree()
        .prepared_presentation(f.owner)
        .unwrap()
        .unwrap()
        .data::<GraphProjectionSnapshot>()
        .unwrap()
        .request()
        .identity();
    f.runtime.figure(f.owner).unwrap().revalidate().unwrap();
    frame(&mut f.runtime).unwrap();
    assert_eq!(
        f.runtime
            .tree()
            .prepared_presentation(f.owner)
            .unwrap()
            .unwrap()
            .data::<GraphProjectionSnapshot>()
            .unwrap()
            .request()
            .identity(),
        identity
    );
    f.fail.store(true, Ordering::Relaxed);
    f.runtime.figure(f.owner).unwrap().revalidate().unwrap();
    assert!(
        matches!(frame(&mut f.runtime), Err(FramePreparationError::Text(TextError::PreparationMeasurement(error))) if error.figure == figure)
    );
    f.fail.store(false, Ordering::Relaxed);
    frame(&mut f.runtime).unwrap();
    assert_eq!(f.runtime.tree().figure_bounds(figure).unwrap().height, 20.0);
}
#[test]
fn incompatible_anchor_binding_cannot_publish_a_partial_graph() {
    let mut f = fixture(false);
    frame(&mut f.runtime).unwrap();
    let source = f
        .runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            novadraw::geometry::Point::new(0.0, 0.0),
            f.binding.routing_space(),
        )));
    f.runtime
        .connections()
        .set_source(f.connections[0], Some(source))
        .unwrap();
    assert!(frame(&mut f.runtime).is_err());
    assert!(matches!(
        f.runtime
            .connections()
            .state(f.connections[0])
            .unwrap()
            .resolution,
        ConnectionResolution::Unresolved(_)
    ));
    let edge = f.binding.edges()[0].0;
    let source = f.runtime.connections().register_anchor(Box::new(
        f.binding
            .anchor(&f.request, edge, RouteEnd::Source)
            .unwrap(),
    ));
    f.runtime
        .connections()
        .set_source(f.connections[0], Some(source))
        .unwrap();
    frame(&mut f.runtime).unwrap();
}

#[test]
fn missing_duplicate_and_retired_bindings_are_rejected() {
    let mut f = fixture(false);
    let mut context = GraphSolveContext::new(BUDGET);
    assert!(
        GraphProjectionBinding::new(
            f.owner,
            &f.request,
            vec![],
            f.binding.edges().to_vec(),
            &mut context
        )
        .is_err()
    );
    let mut entities = f.binding.entities().to_vec();
    entities[1].1 = entities[0].1;
    assert!(
        GraphProjectionBinding::new(
            f.owner,
            &f.request,
            entities,
            f.binding.edges().to_vec(),
            &mut context
        )
        .is_err()
    );
    frame(&mut f.runtime).unwrap();
    let removed = f.binding.entities()[0].1;
    assert!(f.runtime.figure(f.owner).unwrap().remove(removed).unwrap());
    assert!(frame(&mut f.runtime).is_err());
    assert!(f.runtime.tree().figure_bounds(removed).is_err());
    assert!(
        f.runtime
            .connections()
            .resolve_route(f.connections[0], f.binding.routing_space())
            .is_err()
    );
}

struct External;
impl GraphAlgorithmPayload for External {}
const EXTERNAL: GraphAlgorithmKey<External> = GraphAlgorithmKey::new("external.projection");
struct ExternalProof(usize);
impl GraphCapabilityPayload for ExternalProof {}
const EXTERNAL_PROOF: GraphCapabilityKey<ExternalProof> =
    GraphCapabilityKey::new("external.projection-proof");

#[test]
fn oscillating_external_measurement_feedback_is_rejected_without_publication() {
    let mut service = GraphLayoutService::new();
    service
        .register(
            EXTERNAL,
            GraphAlgorithmCapabilities::new(),
            |_: &External, request: &GraphLayoutRequest, context: &mut GraphSolveContext| {
                context.allocate(GraphStage::Placement, 1)?;
                let node = &request.nodes()[0];
                let width = if node.size.height.preferred == 60.0 {
                    200.0
                } else {
                    80.0
                };
                let mut result = GraphLayoutCandidate::new(request);
                let bounds = Rectangle::new(0.0, 0.0, width, node.size.height.preferred);
                result.placements.push(GraphPlacement {
                    entity: node.id().into(),
                    bounds,
                });
                result.extent = bounds;
                Ok(result)
            },
        )
        .unwrap();
    let mut context = GraphSolveContext::new(BUDGET);
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::East, EXTERNAL, External, &mut context).unwrap();
    let node = builder
        .add_node(
            GraphSize::directed(Dimension::new(80.0, 60.0), GraphDirection::East),
            &mut context,
        )
        .unwrap();
    let request = builder.build(&mut context).unwrap();
    let mut runtime = Runtime::empty();
    let owner = runtime
        .set_contents(Box::new(GraphProjectionFigure::new(Rectangle::new(
            0.0, 0.0, 600.0, 400.0,
        ))))
        .unwrap();
    let figure = runtime
        .figure(owner)
        .unwrap()
        .add(Box::new(WidthSensitive(Arc::new(AtomicBool::new(false)))))
        .unwrap();
    let binding = Arc::new(
        GraphProjectionBinding::new(
            owner,
            &request,
            vec![(node.into(), figure)],
            vec![],
            &mut context,
        )
        .unwrap(),
    );
    let measurements = vec![GraphProjectionMeasurement {
        entity: node.into(),
        constraints: MeasureConstraints::new(Some(80.0), None).unwrap(),
    }];
    runtime
        .figure(owner)
        .unwrap()
        .update_component(SetGraphProjection(
            GraphProjection::new(request, Arc::new(service), binding, measurements, BUDGET)
                .unwrap(),
        ))
        .unwrap();
    assert!(
        matches!(frame(&mut runtime), Err(FramePreparationError::Text(TextError::GraphProjection(error))) if *error == GraphLayoutError::NonConvergentMeasurement)
    );
    assert!(
        runtime
            .tree()
            .prepared_presentation(owner)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        runtime.tree().figure_bounds(figure).unwrap(),
        Rectangle::ZERO
    );
}
#[test]
fn external_non_layered_algorithm_uses_the_same_projection_and_repeated_frames_do_not_solve() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut service = GraphLayoutService::new();
    service
        .register_capability(
            EXTERNAL_PROOF,
            |proof: &ExternalProof,
             request: &GraphLayoutRequest,
             _: &GraphLayoutCandidate,
             _: &mut GraphSolveContext| {
                if proof.0 != request.nodes().len() {
                    return Err(GraphLayoutError::CapabilityRejected(
                        "external.projection-proof",
                    ));
                }
                Ok(())
            },
        )
        .unwrap();
    service
        .register(
            EXTERNAL,
            GraphAlgorithmCapabilities::new().result(EXTERNAL_PROOF),
            move |_: &External, request: &GraphLayoutRequest, context: &mut GraphSolveContext| {
                count.fetch_add(1, Ordering::Relaxed);
                context.allocate(GraphStage::Placement, 2)?;
                let mut output = GraphLayoutCandidate::new(request);
                for (index, node) in request.nodes().iter().enumerate() {
                    output.placements.push(GraphPlacement {
                        entity: node.id().into(),
                        bounds: Rectangle::new(index as f64 * 60.0, 0.0, 20.0, 10.0),
                    });
                }
                let edge = &request.edges()[0];
                let source = PreparedGraphAttachment::on_bounds(
                    edge.source,
                    output.placements[0].bounds,
                    AttachmentSide::Right,
                    GraphAttachment::default(),
                    0.0,
                );
                let target = PreparedGraphAttachment::on_bounds(
                    edge.target,
                    output.placements[1].bounds,
                    AttachmentSide::Left,
                    GraphAttachment::default(),
                    0.0,
                );
                context.allocate(GraphStage::Routing, 3)?;
                output.routes.push(GraphRoute {
                    edge: edge.id(),
                    source,
                    target,
                    points: vec![source.position, target.position],
                });
                output.extent = Rectangle::new(0.0, 0.0, 80.0, 10.0);
                output
                    .capabilities
                    .insert(EXTERNAL_PROOF, ExternalProof(2), context)?;
                Ok(output)
            },
        )
        .unwrap();
    let mut context = GraphSolveContext::new(BUDGET);
    let mut builder =
        GraphRequestBuilder::new(GraphDirection::South, EXTERNAL, External, &mut context).unwrap();
    let node = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut context)
        .unwrap();
    let second = builder
        .add_node(GraphSize::fixed(Dimension::new(20.0, 10.0)), &mut context)
        .unwrap();
    let edge = builder.add_edge(node, second, &mut context).unwrap();
    builder.require(EXTERNAL_PROOF, &mut context).unwrap();
    let request = builder.build(&mut context).unwrap();
    let mut runtime = Runtime::empty();
    let owner = runtime
        .set_contents(Box::new(GraphProjectionFigure::new(Rectangle::new(
            0.0, 0.0, 600.0, 400.0,
        ))))
        .unwrap();
    let figure = runtime
        .figure(owner)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .unwrap();
    let second_figure = runtime
        .figure(owner)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)))
        .unwrap();
    let connection_figure = runtime
        .figure(owner)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let dummy = runtime
        .connections()
        .register_router(Box::new(DirectRouter));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            None,
            None,
            RouterBinding::Explicit { router: dummy },
            None,
        )
        .unwrap();
    let binding = Arc::new(
        GraphProjectionBinding::new(
            owner,
            &request,
            vec![(node.into(), figure), (second.into(), second_figure)],
            vec![(edge, connection)],
            &mut context,
        )
        .unwrap(),
    );
    let router = runtime
        .connections()
        .register_router(Box::new(GraphProjectionRouter::new(binding.clone())));
    let source = runtime.connections().register_anchor(Box::new(
        binding.anchor(&request, edge, RouteEnd::Source).unwrap(),
    ));
    let target = runtime.connections().register_anchor(Box::new(
        binding.anchor(&request, edge, RouteEnd::Target).unwrap(),
    ));
    runtime
        .connections()
        .set_source(connection, Some(source))
        .unwrap();
    runtime
        .connections()
        .set_target(connection, Some(target))
        .unwrap();
    runtime
        .connections()
        .set_router_binding(connection, RouterBinding::Explicit { router })
        .unwrap();
    runtime
        .figure(owner)
        .unwrap()
        .update_component(SetGraphProjection(
            GraphProjection::new(request, Arc::new(service), binding, vec![], BUDGET).unwrap(),
        ))
        .unwrap();
    frame(&mut runtime).unwrap();
    let snapshot = runtime
        .tree()
        .prepared_presentation(owner)
        .unwrap()
        .unwrap()
        .data::<GraphProjectionSnapshot>()
        .unwrap();
    assert_eq!(
        snapshot
            .result()
            .capability(EXTERNAL_PROOF)
            .unwrap()
            .unwrap()
            .0,
        2
    );
    assert!(
        snapshot
            .result()
            .capability(LAYERED_RESULT)
            .unwrap()
            .is_none()
    );
    let solved = calls.load(Ordering::Relaxed);
    runtime.figure(owner).unwrap().revalidate().unwrap();
    frame(&mut runtime).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), solved);
}

fn project_fixture(request: GraphLayoutRequest, service: Arc<GraphLayoutService>) {
    let budget = GraphBudget::new(200_000_000, 20_000_000);
    let mut context = GraphSolveContext::new(budget);
    let mut runtime = Runtime::empty();
    runtime
        .frames()
        .set_preparation_budget(novadraw::figure::PreparationBudget::new(
            budget.work,
            budget.allocation,
        ));
    let owner = runtime
        .set_contents(Box::new(GraphProjectionFigure::new(Rectangle::new(
            0.0, 0.0, 600.0, 400.0,
        ))))
        .unwrap();
    let mut entities = Vec::new();
    for entity in request
        .nodes()
        .iter()
        .map(|node| GraphEndpoint::Node(node.id()))
        .chain(
            request
                .groups()
                .iter()
                .map(|group| GraphEndpoint::Group(group.id())),
        )
    {
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0)))
            .unwrap();
        entities.push((entity, figure));
    }
    let mut edges = Vec::new();
    let dummy = runtime
        .connections()
        .register_router(Box::new(DirectRouter));
    for edge in request.edges() {
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(ConnectionFigure::new()))
            .unwrap();
        let connection = runtime
            .connections()
            .register_state(
                figure,
                None,
                None,
                RouterBinding::Explicit { router: dummy },
                None,
            )
            .unwrap();
        edges.push((edge.id(), connection));
    }
    let binding = Arc::new(
        GraphProjectionBinding::new(owner, &request, entities, edges, &mut context).unwrap(),
    );
    let router = runtime
        .connections()
        .register_router(Box::new(GraphProjectionRouter::new(binding.clone())));
    for &(edge, connection) in binding.edges() {
        let source = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Source).unwrap(),
        ));
        let target = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Target).unwrap(),
        ));
        runtime
            .connections()
            .set_source(connection, Some(source))
            .unwrap();
        runtime
            .connections()
            .set_target(connection, Some(target))
            .unwrap();
        runtime
            .connections()
            .set_router_binding(connection, RouterBinding::Explicit { router })
            .unwrap();
    }
    runtime
        .figure(owner)
        .unwrap()
        .update_component(SetGraphProjection(
            GraphProjection::new(request, service, binding.clone(), vec![], budget).unwrap(),
        ))
        .unwrap();
    frame(&mut runtime).unwrap();
    let tree = runtime.tree();
    let snapshot = tree
        .prepared_presentation(owner)
        .unwrap()
        .unwrap()
        .data::<GraphProjectionSnapshot>()
        .unwrap();
    for &(edge, connection) in binding.edges() {
        let transform = tree.local_to_parent_transform(connection.figure()).unwrap();
        let points = tree
            .connection_route_points(connection.figure())
            .unwrap()
            .unwrap()
            .iter()
            .map(|point| transform.transform_point(*point))
            .collect::<Vec<_>>();
        assert_eq!(points, snapshot.result().route(edge).unwrap().points);
    }
}

#[test]
fn all_official_fixture_directions_publish_through_the_same_flat_adapter() {
    let mut service = GraphLayoutService::new();
    register_layered_protocol(&mut service).unwrap();
    register_directed_graph(&mut service).unwrap();
    register_compound_graph(&mut service).unwrap();
    let service = Arc::new(service);
    for direction in [GraphDirection::South, GraphDirection::East] {
        for name in directed_fixtures::DIRECTED_FIXTURES {
            let mut context = GraphSolveContext::new(GraphBudget::new(200_000_000, 20_000_000));
            let fixture =
                directed_fixtures::directed_fixture(name, direction, &mut context).unwrap();
            assert_eq!(fixture.labels.len(), fixture.request.nodes().len());
            project_fixture(fixture.request, service.clone());
        }
        for name in compound_fixtures::COMPOUND_FIXTURES {
            let mut context = GraphSolveContext::new(GraphBudget::new(200_000_000, 20_000_000));
            let fixture =
                compound_fixtures::compound_fixture(name, direction, &mut context).unwrap();
            assert_eq!(
                fixture.labels.len(),
                fixture.request.nodes().len() + fixture.request.groups().len()
            );
            project_fixture(fixture.request, service.clone());
        }
    }
}

#[test]
fn host_budget_rejects_the_whole_attempt_and_a_later_policy_change_can_retry() {
    let mut f = fixture(false);
    let limit = novadraw::figure::PreparationBudget::new(1, 100);
    f.runtime.frames().set_preparation_budget(limit);
    assert_eq!(f.runtime.frames().preparation_budget(), limit);
    assert!(frame(&mut f.runtime).is_err());
    assert!(
        f.runtime
            .tree()
            .prepared_presentation(f.owner)
            .unwrap()
            .is_none()
    );
    f.runtime
        .frames()
        .set_preparation_budget(novadraw::figure::PreparationBudget::default());
    frame(&mut f.runtime).unwrap();
}
