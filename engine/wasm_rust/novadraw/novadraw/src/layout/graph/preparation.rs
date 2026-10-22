//! Disposable pure-result caching; only Runtime presentation owns publication authority.
use std::sync::{Arc, Mutex};

use crate::{
    figure::{
        FigureDrawing, FigureMeasurement, FigurePresentation, MeasureConstraints,
        PreparationMeasurement, PreparationScene, PreparedScene,
    },
    geometry::Dimension,
    graphics::{GraphicsError, PaintContext},
    layout::LayoutOutput,
    text::{MeasureContext, TextError},
};

use super::{
    GraphBudget, GraphEndpoint, GraphLayoutError, GraphLayoutRequest, GraphLayoutService,
    GraphProjectionBinding, GraphProjectionSnapshot, GraphRequestBuilder, GraphSize,
    GraphSolveContext, GraphStage,
};

/// One dynamically measured entity. Unlisted entities retain explicit fixture requirements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphProjectionMeasurement {
    pub entity: GraphEndpoint,
    pub constraints: MeasureConstraints,
}
/// Final authoritative measurement retained with the checked graph result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphProjectionMeasurementResult {
    pub entity: GraphEndpoint,
    pub constraints: MeasureConstraints,
    pub measurement: PreparationMeasurement,
}
type Demand = GraphProjectionMeasurementResult;
struct Cached {
    source: crate::figure::preparation::PreparationSource,
    text: u64,
    snapshot: Arc<GraphProjectionSnapshot>,
    demands: Vec<Demand>,
    observations: Vec<crate::connection::DependencyObservation>,
    version: u64,
}

/// Composable preparation producer for any container Figure, using the same public protocols.
/// The cache contains checked pure results, never attached bounds or current routing authority.
pub struct GraphProjection {
    request: GraphLayoutRequest,
    service: Arc<GraphLayoutService>,
    binding: Arc<GraphProjectionBinding>,
    measurements: Vec<GraphProjectionMeasurement>,
    budget: GraphBudget,
    cache: Mutex<Option<Cached>>,
}
impl GraphProjection {
    pub fn binding(&self) -> &GraphProjectionBinding {
        &self.binding
    }
    pub fn new(
        request: GraphLayoutRequest,
        service: Arc<GraphLayoutService>,
        binding: Arc<GraphProjectionBinding>,
        measurements: Vec<GraphProjectionMeasurement>,
        budget: GraphBudget,
    ) -> Result<Self, GraphLayoutError> {
        let mut context = GraphSolveContext::new(budget);
        context.allocate(
            GraphStage::Request,
            (binding.entities().len() + binding.edges().len()) as u64,
        )?;
        GraphProjectionBinding::new(
            binding.owner(),
            &request,
            binding.entities().to_vec(),
            binding.edges().to_vec(),
            &mut context,
        )?;
        let mut entities = std::collections::HashSet::new();
        for measurement in &measurements {
            context.allocate(GraphStage::Measurement, 1)?;
            binding.figure(measurement.entity)?;
            if !entities.insert(measurement.entity) {
                return Err(GraphLayoutError::InvalidProjectionBinding);
            }
        }
        Ok(Self {
            request,
            service,
            binding,
            measurements,
            budget,
            cache: Mutex::new(None),
        })
    }

    /// Freeze, measure, solve and buffer placement. No tree mutation occurs here.
    pub fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        if self.binding.owner() != scene.root() {
            return Err(TextError::GraphProjection(Box::new(
                GraphLayoutError::InvalidProjectionBinding,
            )));
        }
        for figure in self
            .binding
            .entities()
            .iter()
            .map(|entry| entry.1)
            .chain(self.binding.edges().iter().map(|entry| entry.1.figure()))
        {
            if scene.parent(figure)? != Some(scene.root()) {
                return Err(TextError::GraphProjection(Box::new(
                    GraphLayoutError::InvalidProjectionBinding,
                )));
            }
        }
        let mut solve = GraphSolveContext::with_preparation_budget(self.budget, scene.budget());
        let source = scene.source_prefix()?;
        let text = context.revision();
        // Reuse only after exact source equality and re-observation of the final constraints.
        // A retained pure result is never read through the self-snapshot query.
        let mut cache = self.cache.lock().map_err(|_| TextError::RuntimeFaulted)?;
        if let Some(cached) = cache.as_ref()
            && cached.source == source
            && cached.text == text
            && cached
                .snapshot
                .result
                .check_prefix(&cached.snapshot.request, &self.service)
                .is_ok()
            && scene.reuse_observations(&cached.observations)?
        {
            let mut unchanged = true;
            for demand in &cached.demands {
                let current = scene.measure_figure(
                    context,
                    self.binding.figure(demand.entity).map_err(graph_error)?,
                    demand.constraints,
                )?;
                if current != demand.measurement {
                    unchanged = false;
                }
            }
            let observations = scene.observations();
            let comparison_work = observations
                .len()
                .checked_mul(cached.observations.len())
                .ok_or_else(|| graph_error(GraphLayoutError::Overflow(GraphStage::Measurement)))?;
            solve
                .consume(GraphStage::Measurement, comparison_work as u64)
                .map_err(graph_error)?;
            if unchanged
                && observations.len() == cached.observations.len()
                && observations
                    .iter()
                    .all(|observation| cached.observations.contains(observation))
            {
                return self.output(cached.snapshot.clone(), cached.version, scene);
            }
        }
        let mut demands = Vec::new();
        for measurement in &self.measurements {
            solve
                .allocate(GraphStage::Measurement, 1)
                .map_err(graph_error)?;
            demands.push(Demand {
                entity: measurement.entity,
                constraints: measurement.constraints,
                measurement: scene.measure_figure(
                    context,
                    self.binding
                        .figure(measurement.entity)
                        .map_err(graph_error)?,
                    measurement.constraints,
                )?,
            });
        }
        solve
            .allocate(GraphStage::Measurement, demands.len() as u64)
            .map_err(graph_error)?;
        let initial = demands.clone();
        let mut previous = Vec::new();
        let mut request = if demands.is_empty() {
            self.request.clone()
        } else {
            self.request_for(&demands, &initial, scene, &mut solve)?
        };
        loop {
            solve
                .consume(GraphStage::Measurement, 1)
                .map_err(graph_error)?;
            let result = self
                .service
                .solve(&request, &mut solve)
                .map_err(graph_error)?;
            let mut final_demands = Vec::new();
            for demand in &demands {
                solve
                    .allocate(GraphStage::Measurement, 1)
                    .map_err(graph_error)?;
                let width = result.bounds(demand.entity).map_err(graph_error)?.width;
                let constraints = MeasureConstraints::new(Some(width), None).map_err(|_| {
                    graph_error(GraphLayoutError::Overflow(GraphStage::Measurement))
                })?;
                final_demands.push(Demand {
                    entity: demand.entity,
                    constraints,
                    measurement: scene.measure_figure(
                        context,
                        self.binding.figure(demand.entity).map_err(graph_error)?,
                        constraints,
                    )?,
                });
            }
            let sizes_match = self.demands_match(&request, &final_demands, &initial);
            if sizes_match {
                solve
                    .allocate(GraphStage::Result, (final_demands.len() + 1) as u64)
                    .map_err(graph_error)?;
                let snapshot = Arc::new(GraphProjectionSnapshot {
                    request,
                    result,
                    binding: self.binding.clone(),
                    measurements: final_demands.clone(),
                });
                let version = cache
                    .as_ref()
                    .map_or(Ok(1), |cached| {
                        cached
                            .version
                            .checked_add(1)
                            .ok_or(GraphLayoutError::Overflow(GraphStage::Result))
                    })
                    .map_err(graph_error)?;
                let output = self.output(snapshot.clone(), version, scene)?;
                *cache = Some(Cached {
                    source,
                    text,
                    snapshot,
                    demands: final_demands,
                    observations: scene.observations(),
                    version,
                });
                return Ok(output);
            }
            solve
                .consume(GraphStage::Measurement, previous.len() as u64)
                .map_err(graph_error)?;
            if previous.contains(&final_demands) {
                return Err(graph_error(GraphLayoutError::NonConvergentMeasurement));
            }
            solve
                .allocate(GraphStage::Measurement, (final_demands.len() + 1) as u64)
                .map_err(graph_error)?;
            previous.push(final_demands.clone());
            demands = final_demands;
            request = self.request_for(&demands, &initial, scene, &mut solve)?;
        }
    }
    fn node_size(&self, demand: &Demand, initial: &Demand) -> GraphSize {
        let GraphEndpoint::Node(id) = demand.entity else {
            unreachable!("node demand")
        };
        let mut size = self.request.node(id).expect("validated binding").size;
        size.width.minimum = initial
            .measurement
            .minimum
            .width
            .max(demand.measurement.minimum.width);
        size.width.preferred = initial
            .measurement
            .preferred
            .width
            .max(demand.measurement.preferred.width)
            .max(size.width.minimum);
        size.height.minimum = demand.measurement.minimum.height;
        size.height.preferred = demand.measurement.preferred.height.max(size.height.minimum);
        size
    }
    fn group_minimum(&self, demand: &Demand) -> Dimension {
        let GraphEndpoint::Group(id) = demand.entity else {
            unreachable!("group demand")
        };
        let minimum = self.request.group(id).expect("validated binding").minimum;
        Dimension::new(
            minimum
                .width
                .max(demand.measurement.minimum.width)
                .max(demand.measurement.preferred.width),
            minimum
                .height
                .max(demand.measurement.minimum.height)
                .max(demand.measurement.preferred.height),
        )
    }
    fn demands_match(
        &self,
        request: &GraphLayoutRequest,
        demands: &[Demand],
        initial: &[Demand],
    ) -> bool {
        demands
            .iter()
            .zip(initial)
            .all(|(demand, initial)| match demand.entity {
                GraphEndpoint::Node(id) => request
                    .node(id)
                    .is_ok_and(|node| node.size == self.node_size(demand, initial)),
                GraphEndpoint::Group(id) => request
                    .group(id)
                    .is_ok_and(|group| group.minimum == self.group_minimum(demand)),
            })
    }
    fn request_for(
        &self,
        demands: &[Demand],
        initial: &[Demand],
        scene: &PreparationScene<'_>,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutRequest, TextError> {
        let mut builder =
            GraphRequestBuilder::from_request(&self.request, context).map_err(graph_error)?;
        for (demand, initial) in demands.iter().zip(initial) {
            context
                .consume(GraphStage::Measurement, 1)
                .map_err(graph_error)?;
            match demand.entity {
                GraphEndpoint::Node(id) => {
                    builder.node_mut(id).map_err(graph_error)?.size =
                        self.node_size(demand, initial)
                }
                GraphEndpoint::Group(id) => {
                    let group = builder.group_mut(id).map_err(graph_error)?;
                    group.minimum = self.group_minimum(demand);
                    group.border_insets =
                        scene.insets(self.binding.figure(demand.entity).map_err(graph_error)?)?;
                }
            }
        }
        builder.build(context).map_err(graph_error)
    }
    fn output(
        &self,
        snapshot: Arc<GraphProjectionSnapshot>,
        version: u64,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        scene.budget().allocate(
            "graph projection output",
            (snapshot.result.placements().len() + self.binding.edges().len()) as u64,
        )?;
        let extent = snapshot.result.extent();
        let size = Dimension::new(
            extent.x.max(0.0) + extent.width,
            extent.y.max(0.0) + extent.height,
        );
        let presentation = FigurePresentation::new(
            FigureMeasurement::new(size.width, size.height, None),
            size,
            extent,
            Arc::new(Empty),
        )?;
        let mut output = PreparedScene::new(super::projection::with_snapshot(
            presentation,
            snapshot.clone(),
        ))
        .with_result_version(version);
        let mut layout = LayoutOutput::new();
        for placement in snapshot.result.placements() {
            layout.set_child_bounds(
                self.binding.figure(placement.entity).map_err(graph_error)?,
                placement.bounds,
            );
        }
        output.layout(scene.root(), layout);
        if !self.binding.edges().is_empty() {
            output.require_route_group(
                self.binding.routing_space(),
                self.binding.edges().iter().map(|entry| entry.1),
            );
        }
        Ok(output)
    }
}
struct Empty;
impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}
fn graph_error(error: GraphLayoutError) -> TextError {
    TextError::GraphProjection(Box::new(error))
}
