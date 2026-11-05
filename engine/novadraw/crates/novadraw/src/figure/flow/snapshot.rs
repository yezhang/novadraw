use super::{FlowAssembly, FlowContributionRegistry, FlowError, FlowSourceRange};
use crate::{
    FigureId, FigureMeasurement, MeasureConstraints,
    figure::{
        FigureDrawing, FigurePresentation, PreparationMeasureError, PreparationMeasureErrorKind,
        PreparationMeasurement, PreparationScene, PreparedScene,
    },
    geometry::{Dimension, Point, Rectangle},
    graphics::{GraphicsError, PaintContext},
    layout::LayoutOutput,
    text::{
        CaretGeometry, MeasureContext, SelectionQuad, StyledTextLayout, StyledTextRequest,
        TextAffinity, TextConstraints, TextInlineBox, TextInteractionError, TextMovement,
        TextPosition, TextRange,
    },
};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

type CachedFlow = (
    u64,
    Arc<FlowSnapshot>,
    Vec<FigureId>,
    Vec<crate::connection::DependencyObservation>,
);

// A service retains recent exact-width assets, including measurement/preparation widths.
// Old published snapshots keep their own immutable leases after cache eviction.
const PARAGRAPH_CACHE_CAPACITY: usize = 256;
// Bounded/unbounded demands and scrollbar feedback widths can alternate.
const MEASUREMENT_CACHE_CAPACITY: usize = 4;

#[derive(PartialEq)]
struct FlowMeasurementKey {
    input: crate::figure::PreparationInputSnapshot,
    engine: crate::text::shaping::TextLayoutCacheIdentity,
    engine_revision: u64,
    bounds: Rectangle,
    available_width: Option<f64>,
    requested_constraints: Option<MeasureConstraints>,
    constraints: Option<(MeasureConstraints, MeasureConstraints)>,
}

struct FlowMeasurementRecord {
    key: FlowMeasurementKey,
    size: Dimension,
    observations: Vec<crate::connection::DependencyObservation>,
    retention: crate::text::TextContentLease,
}

#[derive(Default)]
struct FlowWork {
    statistics: FlowWorkStatistics,
    paragraphs: Vec<ParagraphAsset>,
    // Length only narrows candidates; engine/version and the full request prove reuse.
    // Reading it does not hash or materialize the text, spans, or inline boxes.
    paragraph_lookup: HashMap<usize, Vec<usize>>,
    // Stable slots preserve FIFO eviction without moving every retained asset.
    oldest_paragraph: usize,
    assembly: Option<(
        crate::figure::PreparationInputSnapshot,
        Arc<super::assembly::FrozenFlowPlan>,
        Vec<crate::connection::DependencyObservation>,
    )>,
    measurements: Vec<FlowMeasurementRecord>,
    assembly_workspace: super::workspace::AssemblyWorkspace,
}

struct ParagraphAsset {
    engine_identity: Option<crate::text::shaping::TextLayoutCacheIdentity>,
    engine_revision: u64,
    layout: StyledTextLayout,
    padding: Vec<(f64, f64)>,
    padded: Option<StyledTextLayout>,
    source_geometry: HashMap<(usize, usize), Arc<[SelectionQuad]>>,
}

struct FlowLayoutCall<'a, 'scene> {
    scene: &'a PreparationScene<'scene>,
    purpose: FlowBuildPurpose,
    owner: FigureId,
    scene_revision: u64,
    sources: &'a [FlowSourceRange],
    inline_prefix: Option<usize>,
}

impl FlowWork {
    fn cached_paragraph(
        &self,
        engine_identity: Option<crate::text::shaping::TextLayoutCacheIdentity>,
        engine_revision: u64,
        request: &StyledTextRequest,
    ) -> Option<usize> {
        let identity = engine_identity?;
        self.paragraph_lookup
            .get(&request.text().len())?
            .iter()
            .copied()
            .find(|&index| {
                let asset = &self.paragraphs[index];
                asset.engine_identity == Some(identity)
                    && asset.engine_revision == engine_revision
                    && asset.layout.request() == request
            })
    }

    fn cache_paragraph(&mut self, asset: ParagraphAsset) -> usize {
        let text_len = asset.layout.request().text().len();
        let index = if self.paragraphs.len() < PARAGRAPH_CACHE_CAPACITY {
            let index = self.paragraphs.len();
            self.paragraphs.push(asset);
            index
        } else {
            let index = self.oldest_paragraph;
            let retired = std::mem::replace(&mut self.paragraphs[index], asset);
            let retired_len = retired.layout.request().text().len();
            let bucket = self.paragraph_lookup.get_mut(&retired_len).unwrap();
            bucket.retain(|&candidate| candidate != index);
            if bucket.is_empty() {
                self.paragraph_lookup.remove(&retired_len);
            }
            self.oldest_paragraph = (index + 1) % PARAGRAPH_CACHE_CAPACITY;
            index
        };
        self.paragraph_lookup
            .entry(text_len)
            .or_default()
            .push(index);
        index
    }

    fn layout(
        &mut self,
        context: &mut MeasureContext<'_>,
        request: &StyledTextRequest,
        call: FlowLayoutCall<'_, '_>,
    ) -> Result<(usize, StyledTextLayout), crate::text::TextError> {
        let diagnostics = call.scene.budget().text_diagnostics();
        let engine_revision = context.revision();
        let engine_identity = context.cache_identity();
        let build = self.statistics.builds;
        let observe = |cache_hit: bool, succeeded: bool| {
            diagnostics.request(|| format!(
                "root={:?} owner={:?} build={} scene_revision={} sources={:?} phase={} inline_prefix={:?} engine_identity={:?} engine_revision={} cache_hit={} succeeded={} request={:?}",
                call.scene.root(), call.owner, build, call.scene_revision,
                call.sources.iter().map(|source| (source.source, call.scene.diagnostic_component_revision(source.source))).collect::<Vec<_>>(),
                match call.purpose { FlowBuildPurpose::Measure => "measure", FlowBuildPurpose::Prepare => "prepare" },
                call.inline_prefix, engine_identity, engine_revision, cache_hit, succeeded, request,
            ));
        };
        let lookup = diagnostics.time("flow-paragraph-lookup");
        if let Some(index) = self.cached_paragraph(engine_identity, engine_revision, request) {
            drop(lookup);
            observe(true, true);
            self.statistics.paragraph_hits += 1;
            return Ok((index, self.paragraphs[index].layout.clone()));
        }
        drop(lookup);
        let engine_timing = diagnostics.time("flow-engine-layout");
        let result = context.layout_styled_with_diagnostics(request, &diagnostics);
        drop(engine_timing);
        observe(false, result.is_ok());
        self.statistics.styled_layout_calls += 1;
        let layout = result?;
        let index = self.cache_paragraph(ParagraphAsset {
            engine_identity,
            engine_revision,
            layout: layout.clone(),
            padding: Vec::new(),
            padded: None,
            source_geometry: HashMap::new(),
        });
        Ok((index, layout))
    }
}

/// Deterministic cumulative work for one instance-owned service.
/// These counters describe CPU layout work, never GPU completion or presentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowWorkStatistics {
    pub builds: u64,
    pub styled_layout_calls: u64,
    pub paragraph_hits: u64,
    pub source_geometry_calls: u64,
    pub assembly_builds: u64,
    pub assembly_hits: u64,
    pub assembly_ineligible: u64,
}

fn flow_revision(
    context: &MeasureContext<'_>,
    bounds: Rectangle,
    scene: &PreparationScene<'_>,
) -> Result<u64, FlowError> {
    let _timing = scene.budget().time("flow-signature");
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    scene.revision()?.hash(&mut hash);
    context.revision().hash(&mut hash);
    context.cache_identity().hash(&mut hash);
    for value in [bounds.x, bounds.y, bounds.width, bounds.height] {
        value.to_bits().hash(&mut hash);
    }
    Ok(hash.finish())
}

/// Instance-owned flow preparation. Custom roots use the same service as `FlowPageFigure`.
/// Failed candidates never replace the last successfully checked cache entry.
#[derive(Clone)]
pub struct FlowLayoutService {
    registry: FlowContributionRegistry,
    cache: Arc<Mutex<Option<CachedFlow>>>,
    statistics: Arc<Mutex<FlowWork>>,
    inputs: crate::figure::PreparationInputRegistry,
    assembly_reuse: Arc<AtomicBool>,
    retention: crate::text::TextContentStore,
    cache_limit: Arc<Mutex<usize>>,
}
impl FlowLayoutService {
    pub fn new(registry: FlowContributionRegistry) -> Self {
        let retention = crate::text::TextContentStore::new(usize::MAX);
        let inputs = super::reuse::input_registry(registry.clone(), retention.clone());
        let work = FlowWork {
            assembly_workspace: super::workspace::AssemblyWorkspace::new(retention.clone()),
            ..FlowWork::default()
        };
        Self {
            registry,
            cache: Arc::new(Mutex::new(None)),
            statistics: Arc::new(Mutex::new(work)),
            inputs,
            assembly_reuse: Arc::new(AtomicBool::new(false)),
            retention,
            cache_limit: Arc::new(Mutex::new(usize::MAX)),
        }
    }
    /// Experimental assembly reuse. Defaults to disabled.
    /// Cloned enabled services share their cache and registered proof reader.
    pub fn with_assembly_reuse(self, enabled: bool) -> Self {
        self.assembly_reuse.store(enabled, Ordering::Relaxed);
        self
    }
    pub fn set_assembly_limits(
        &self,
        cache_units: usize,
        retained_units: usize,
    ) -> Result<(), FlowError> {
        *self
            .cache_limit
            .lock()
            .map_err(|_| FlowError::InvalidStructure)? = cache_units;
        self.retention
            .set_limit(retained_units)
            .map_err(crate::text::TextError::from)?;
        let mut work = self
            .statistics
            .lock()
            .map_err(|_| FlowError::InvalidStructure)?;
        work.assembly_workspace
            .set_limit(cache_units.min(retained_units));
        if work.assembly.as_ref().is_some_and(|(input, assembly, _)| {
            assembly
                .retained_units()
                .saturating_add(super::reuse::input_units(input))
                > cache_units
        }) {
            work.assembly = None;
        }
        work.measurements.retain(|record| {
            record
                .retention
                .units()
                .saturating_add(super::reuse::input_units(&record.key.input))
                <= cache_units
        });
        Ok(())
    }
    pub fn assembly_usage(&self) -> Result<crate::text::TextContentUsage, FlowError> {
        self.retention
            .usage()
            .map_err(crate::text::TextError::from)
            .map_err(FlowError::from)
    }
    pub fn work_statistics(&self) -> Result<FlowWorkStatistics, FlowError> {
        Ok(self
            .statistics
            .lock()
            .map_err(|_| FlowError::InvalidStructure)?
            .statistics)
    }
    /// Measures a frozen contribution tree without publishing placements or drawing.
    /// Unbounded width is passed to the text engine as an unbounded constraint.
    pub fn measure(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        self.release_failed_workspace(self.measure_inner(context, constraints, scene))
    }

    // Called after all temporary service locks and callback borrows have ended.
    // Preserve the original failure, including budget/input proof errors.
    fn release_failed_workspace<T, E>(&self, result: Result<T, E>) -> Result<T, E> {
        if result.is_err()
            && let Ok(mut work) = self.statistics.lock()
        {
            work.assembly_workspace.discard();
        }
        result
    }

    fn measure_inner(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let bounds = Rectangle::new(0.0, 0.0, constraints.max_width().unwrap_or(0.0), 0.0);
        let output = build_flow(
            &self.registry,
            context,
            bounds,
            constraints.max_width(),
            scene,
            FlowBuildPurpose::Measure,
            FlowBuildCall {
                requested_constraints: Some(constraints),
                work: &mut *self
                    .statistics
                    .lock()
                    .map_err(|_| PreparationMeasureError {
                        figure: scene.root(),
                        outer_constraints: constraints,
                        content_constraints: Some(constraints),
                        kind: PreparationMeasureErrorKind::Text(Box::new(
                            crate::text::TextError::InvalidPreparedScene(
                                "flow statistics poisoned".into(),
                            ),
                        )),
                    })?,
                reuse_inputs: self.assembly_reuse.load(Ordering::Relaxed).then_some(
                    AssemblyReuseCall {
                        inputs: &self.inputs,
                        retention: &self.retention,
                        cache_limit: *self.cache_limit.lock().map_err(|_| {
                            PreparationMeasureError {
                                figure: scene.root(),
                                outer_constraints: constraints,
                                content_constraints: Some(constraints),
                                kind: PreparationMeasureErrorKind::Text(Box::new(
                                    crate::text::TextError::InvalidPreparedScene(
                                        "flow cache limit poisoned".into(),
                                    ),
                                )),
                            }
                        })?,
                    },
                ),
            },
        )
        .map_err(|error| match error.into_text() {
            crate::text::TextError::PreparationMeasurement(error) => *error,
            crate::text::TextError::PreparationDependency(error) => PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: PreparationMeasureErrorKind::Dependency(error),
            },
            error => PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: PreparationMeasureErrorKind::Text(Box::new(error)),
            },
        })?;
        match output {
            FlowBuildOutput::Measurement(size, candidate) => {
                if let Some(candidate) = candidate {
                    self.statistics
                        .lock()
                        .map_err(|_| PreparationMeasureError {
                            figure: scene.root(),
                            outer_constraints: constraints,
                            content_constraints: Some(constraints),
                            kind: PreparationMeasureErrorKind::Text(Box::new(
                                crate::text::TextError::InvalidPreparedScene(
                                    "flow statistics poisoned".into(),
                                ),
                            )),
                        })?
                        .assembly = Some(candidate);
                }
                Ok(PreparationMeasurement {
                    preferred: FigureMeasurement::new(size.width, size.height, None),
                    minimum: size,
                })
            }
            FlowBuildOutput::Snapshot(_, _, _) => unreachable!("measurement build purpose"),
        }
    }
    pub fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, FlowError> {
        self.release_failed_workspace(self.prepare_inner(context, bounds, scene))
    }

    fn prepare_inner(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, FlowError> {
        let key = flow_revision(context, bounds, scene)?;
        let cached = self
            .cache
            .lock()
            .map_err(|_| FlowError::InvalidStructure)?
            .clone();
        if let Some((revision, snapshot, members, observations)) = cached
            && !self.assembly_reuse.load(Ordering::Relaxed)
            && revision == key
            && context.cache_identity().is_some()
            && scene.reuse_observations(&observations)?
        {
            return prepare_snapshot(snapshot, &members, bounds, scene);
        }
        let (snapshot, members, candidate) = match build_flow(
            &self.registry,
            context,
            bounds,
            Some(bounds.width),
            scene,
            FlowBuildPurpose::Prepare,
            FlowBuildCall {
                requested_constraints: None,
                work: &mut *self
                    .statistics
                    .lock()
                    .map_err(|_| FlowError::InvalidStructure)?,
                reuse_inputs: self.assembly_reuse.load(Ordering::Relaxed).then_some(
                    AssemblyReuseCall {
                        inputs: &self.inputs,
                        retention: &self.retention,
                        cache_limit: *self
                            .cache_limit
                            .lock()
                            .map_err(|_| FlowError::InvalidStructure)?,
                    },
                ),
            },
        )? {
            FlowBuildOutput::Snapshot(snapshot, members, candidate) => {
                (snapshot, members, candidate)
            }
            FlowBuildOutput::Measurement(_, _) => unreachable!("preparation build purpose"),
        };
        let result = prepare_snapshot(snapshot.clone(), &members, bounds, scene)?;
        result.validate(scene)?;
        if flow_revision(context, bounds, scene)? != key {
            return Err(crate::text::TextError::InvalidPreparedScene(
                "flow preparation input changed during layout".into(),
            )
            .into());
        }
        if let Some(candidate) = candidate {
            self.statistics
                .lock()
                .map_err(|_| FlowError::InvalidStructure)?
                .assembly = Some(candidate);
        }
        if !self.assembly_reuse.load(Ordering::Relaxed) {
            *self.cache.lock().map_err(|_| FlowError::InvalidStructure)? =
                Some((key, snapshot, members, scene.observations()));
        }
        Ok(result)
    }
}
impl Default for FlowLayoutService {
    fn default() -> Self {
        Self::new(FlowContributionRegistry::standard())
    }
}

#[derive(Clone, Debug)]
pub struct FlowFragment {
    pub source: FigureId,
    pub paragraph: usize,
    pub line: usize,
    pub bounds: Rectangle,
}
#[derive(Clone, Debug)]
pub struct FlowParagraphSnapshot {
    pub owner: FigureId,
    pub origin: Point,
    pub layout: StyledTextLayout,
    pub sources: super::FlowSourceRanges,
}
/// Positions always identify their source Figure and immutable scene revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceTextPosition {
    pub source: FigureId,
    pub byte_offset: usize,
    pub affinity: TextAffinity,
    pub revision: u64,
}
#[derive(Clone, Debug)]
pub struct FlowSnapshot {
    pub statistics: FlowWorkStatistics,
    pub revision: u64,
    pub paragraphs: Vec<FlowParagraphSnapshot>,
    pub fragments: Vec<FlowFragment>,
    pub placements: HashMap<FigureId, Rectangle>,
    pub size: Dimension,
    borders: Vec<super::border::BorderDrawing>,
    source_fragments: HashMap<FigureId, Vec<usize>>,
    source_ranges: HashMap<FigureId, Vec<(usize, usize)>>,
    paint_index: crate::text::outline::paint_index::EnvelopeIndex,
    outline_visual_bounds: Option<Rectangle>,
    // Preserve the assembly backing and its source maps for every published lease.
    _assembly: Arc<super::assembly::FrozenFlowPlan>,
}
fn translated(b: Rectangle, origin: Point) -> Rectangle {
    Rectangle::new(b.x + origin.x(), b.y + origin.y(), b.width, b.height)
}
impl FlowSnapshot {
    pub fn border_fragments(&self) -> impl Iterator<Item = &super::FlowBorderFragment> {
        self.borders.iter().map(|drawing| &drawing.fragment)
    }
    pub fn hit_text(&self, point: Point) -> Result<SourceTextPosition, TextInteractionError> {
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(TextInteractionError::NonFinitePoint);
        }
        // A source must occupy the actual fragment, not merely its envelope.
        let fragment = self
            .fragments
            .iter()
            .rev()
            .find(|f| {
                f.bounds.contains(point)
                    && self.paragraphs[f.paragraph]
                        .sources
                        .iter()
                        .any(|s| s.source == f.source)
            })
            .ok_or(TextInteractionError::InvisibleTextPosition)?;
        let paragraph = &self.paragraphs[fragment.paragraph];
        let position = paragraph.layout.layout().hit_test_text(Point::new(
            point.x() - paragraph.origin.x(),
            point.y() - paragraph.origin.y(),
        ))?;
        let source = paragraph
            .sources
            .iter()
            .find(|s| {
                s.source == fragment.source
                    && position.byte_offset() >= s.range.start
                    && position.byte_offset() <= s.range.end
            })
            .ok_or(TextInteractionError::InvalidTextPosition)?;
        Ok(SourceTextPosition {
            source: source.source,
            byte_offset: position.byte_offset() - source.range.start + source.source_offset,
            affinity: position.affinity(),
            revision: self.revision,
        })
    }
    fn resolve(
        &self,
        position: SourceTextPosition,
    ) -> Result<(usize, TextPosition), TextInteractionError> {
        if position.revision != self.revision {
            return Err(TextInteractionError::StaleTextLayoutRevision);
        }
        if let Some(ranges) = self.source_ranges.get(&position.source) {
            for &(index, source_index) in ranges {
                let p = &self.paragraphs[index];
                let source = &p.sources[source_index];
                if position.byte_offset >= source.source_offset
                    && position.byte_offset <= source.source_offset + source.range.len()
                {
                    let offset = source.range.start + position.byte_offset - source.source_offset;
                    if !p.layout.request().text().is_char_boundary(offset) {
                        return Err(TextInteractionError::InvalidTextPosition);
                    }
                    return Ok((index, TextPosition::new(offset, position.affinity)));
                }
            }
        }
        Err(TextInteractionError::InvalidTextPosition)
    }
    pub fn caret(
        &self,
        position: SourceTextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        let (index, position) = self.resolve(position)?;
        let p = &self.paragraphs[index];
        let geometry = p.layout.layout().caret_geometry(position)?;
        Ok(CaretGeometry::new(
            translated(geometry.bounds(), p.origin),
            geometry.line_index(),
        ))
    }
    /// Moves through the paragraph's visual/word map and preserves source identity.
    pub fn move_position(
        &self,
        position: SourceTextPosition,
        movement: TextMovement,
    ) -> Result<SourceTextPosition, TextInteractionError> {
        let (mut index, current) = self.resolve(position)?;
        let mut moved = self.paragraphs[index]
            .layout
            .layout()
            .move_text_position(current, movement)?;
        match movement {
            TextMovement::DocumentStart => {
                index = 0;
                moved = TextPosition::new(0, TextAffinity::Downstream);
            }
            TextMovement::DocumentEnd => {
                index = self.paragraphs.len() - 1;
                moved = TextPosition::new(
                    self.paragraphs[index].layout.request().text().len(),
                    TextAffinity::Upstream,
                );
            }
            TextMovement::PreviousVisual | TextMovement::PreviousWord
                if moved.byte_offset() == current.byte_offset()
                    && current.byte_offset() == 0
                    && index > 0 =>
            {
                index -= 1;
                moved = TextPosition::new(
                    self.paragraphs[index].layout.request().text().len(),
                    TextAffinity::Upstream,
                );
            }
            TextMovement::NextVisual | TextMovement::NextWord
                if moved.byte_offset() == current.byte_offset()
                    && current.byte_offset()
                        == self.paragraphs[index].layout.request().text().len()
                    && index + 1 < self.paragraphs.len() =>
            {
                index += 1;
                moved = TextPosition::new(0, TextAffinity::Downstream);
            }
            TextMovement::PreviousLine | TextMovement::NextLine => {
                let layout = self.paragraphs[index].layout.layout();
                let line = layout.caret_geometry(current)?.line_index();
                let previous = movement == TextMovement::PreviousLine;
                if (previous && line == 0 && index > 0)
                    || (!previous
                        && line + 1 == layout.line_metrics().len()
                        && index + 1 < self.paragraphs.len())
                {
                    let global_x = layout.caret_geometry(current)?.bounds().x
                        + self.paragraphs[index].origin.x();
                    index = if previous { index - 1 } else { index + 1 };
                    let paragraph = &self.paragraphs[index];
                    let target = if previous {
                        paragraph.layout.layout().line_metrics().last()
                    } else {
                        paragraph.layout.layout().line_metrics().first()
                    }
                    .ok_or(TextInteractionError::InvisibleTextPosition)?;
                    moved = paragraph.layout.layout().hit_test_text(Point::new(
                        global_x - paragraph.origin.x(),
                        f64::from(target.baseline),
                    ))?;
                }
            }
            _ => {}
        }
        let paragraph = &self.paragraphs[index];
        let mut sources = paragraph
            .sources
            .iter()
            .filter(|s| moved.byte_offset() >= s.range.start && moved.byte_offset() <= s.range.end);
        let source = if moved.affinity() == TextAffinity::Upstream {
            sources.next()
        } else {
            sources.next_back()
        }
        .ok_or(TextInteractionError::InvalidTextPosition)?;
        Ok(SourceTextPosition {
            source: source.source,
            byte_offset: moved.byte_offset() - source.range.start + source.source_offset,
            affinity: moved.affinity(),
            revision: self.revision,
        })
    }
    pub fn selection(
        &self,
        anchor: SourceTextPosition,
        focus: SourceTextPosition,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        let a = self.resolve(anchor)?;
        let b = self.resolve(focus)?;
        let (start, end) = if (a.0, a.1.byte_offset()) <= (b.0, b.1.byte_offset()) {
            (a, b)
        } else {
            (b, a)
        };
        let mut result = Vec::new();
        for index in start.0..=end.0 {
            let p = &self.paragraphs[index];
            let first = if index == start.0 {
                start.1
            } else {
                TextPosition::new(0, TextAffinity::Downstream)
            };
            let last = if index == end.0 {
                end.1
            } else {
                TextPosition::new(p.layout.request().text().len(), TextAffinity::Upstream)
            };
            result.extend(
                p.layout
                    .layout()
                    .selection_geometry(TextRange::new(first, last))?
                    .into_iter()
                    .map(|q| SelectionQuad::new(translated(q.bounds(), p.origin), q.line_index())),
            );
        }
        Ok(result)
    }
}
impl FigureDrawing for FlowSnapshot {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if let Some(clip) = context.canvas.covered_text_clip(Point::ZERO) {
            let mut result = Ok(());
            let work = self.paint_index.visit(clip, |index| {
                if result.is_ok() {
                    let paragraph = &self.paragraphs[index];
                    result = context.fill_styled_text(&paragraph.layout, paragraph.origin);
                }
            });
            context.canvas.count_paragraph_query(work);
            result?;
        } else {
            for paragraph in &self.paragraphs {
                context.fill_styled_text(&paragraph.layout, paragraph.origin)?;
            }
        }
        for border in &self.borders {
            context.push_state();
            context.set_fill_paint(border.fragment.style.color());
            let result = border.border.paint_fragment(&border.fragment, context);
            let restored = context.pop_state();
            result.and(restored)?;
        }
        Ok(())
    }
}
struct EmptyDrawing;
impl FigureDrawing for EmptyDrawing {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}

// These describe internal output materialization, never contributor identities.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowBuildPurpose {
    Measure,
    Prepare,
}
type AssemblyCandidate = (
    crate::figure::PreparationInputSnapshot,
    Arc<super::assembly::FrozenFlowPlan>,
    Vec<crate::connection::DependencyObservation>,
);
enum FlowBuildOutput {
    Measurement(Dimension, Option<AssemblyCandidate>),
    Snapshot(Arc<FlowSnapshot>, Vec<FigureId>, Option<AssemblyCandidate>),
}

#[cfg(test)]
fn build_snapshot(
    registry: &FlowContributionRegistry,
    context: &mut MeasureContext<'_>,
    bounds: Rectangle,
    available_width: Option<f64>,
    scene: &PreparationScene<'_>,
    work: &mut FlowWork,
) -> Result<(Arc<FlowSnapshot>, Vec<FigureId>), FlowError> {
    match build_flow(
        registry,
        context,
        bounds,
        available_width,
        scene,
        FlowBuildPurpose::Prepare,
        FlowBuildCall {
            requested_constraints: None,
            work,
            reuse_inputs: None,
        },
    )? {
        FlowBuildOutput::Snapshot(snapshot, members, _) => Ok((snapshot, members)),
        FlowBuildOutput::Measurement(_, _) => unreachable!("preparation build purpose"),
    }
}

#[derive(Clone, Copy)]
struct AssemblyReuseCall<'a> {
    inputs: &'a crate::figure::PreparationInputRegistry,
    retention: &'a crate::text::TextContentStore,
    cache_limit: usize,
}
struct FlowBuildCall<'a> {
    requested_constraints: Option<MeasureConstraints>,
    work: &'a mut FlowWork,
    reuse_inputs: Option<AssemblyReuseCall<'a>>,
}
fn build_flow(
    registry: &FlowContributionRegistry,
    context: &mut MeasureContext<'_>,
    bounds: Rectangle,
    available_width: Option<f64>,
    scene: &PreparationScene<'_>,
    purpose: FlowBuildPurpose,
    call: FlowBuildCall<'_>,
) -> Result<FlowBuildOutput, FlowError> {
    let FlowBuildCall {
        work,
        reuse_inputs,
        requested_constraints,
    } = call;
    let _timing = scene.budget().time("flow-build");
    let allocate = |entries: usize| {
        scene
            .budget()
            .allocate("flow snapshot", entries as u64)
            .map_err(|error| {
                FlowError::Text(crate::text::TextError::PreparationDependency(Box::new(
                    crate::figure::PreparationDependencyError::Budget(error),
                )))
            })
    };
    let first = reuse_inputs
        .map(|inputs| scene.input(inputs.inputs, super::reuse::ASSEMBLY_INPUT, scene.root()))
        .transpose()?;
    let eligible = first.as_ref().is_some_and(|value| {
        value
            .value(super::reuse::ASSEMBLY_INPUT)
            .is_some_and(Option::is_some)
    });
    let reused = if eligible {
        if let Some((input, assembly, observations)) = &work.assembly
            && Some(input) == first.as_ref()
            && scene.reuse_observations(observations)?
        {
            work.statistics.assembly_hits += 1;
            Some(assembly.clone())
        } else {
            None
        }
    } else {
        if reuse_inputs.is_some() {
            work.statistics.assembly_ineligible += 1;
        }
        None
    };
    let was_hit = reused.is_some();
    let assembly = if let Some(assembly) = reused {
        assembly
    } else {
        let _contribution_timing = scene.budget().time("flow-contribution");
        work.statistics.assembly_builds += 1;
        let mut measure_border = |source, border: Arc<dyn super::FlowFragmentBorder>| {
            let _border = scene.budget().time("flow-border");
            let resolved = scene.style(source)?;
            let style =
                crate::text::TextStyle::new(scene.parsed_font(source)?, resolved.foreground);
            context.set_font(style.font().clone());
            let renderer = {
                let _renderer = scene.budget().time("flow-border-renderer");
                border.prepare_renderer(context, &style)?.unwrap_or(border)
            };
            let metrics = {
                let _metrics = scene.budget().time("flow-border-metrics");
                renderer.metrics(context, &style)?.validate()?
            };
            Ok((metrics, renderer))
        };
        let mut assembly = FlowAssembly::new(
            scene,
            registry,
            &mut measure_border,
            &mut work.assembly_workspace,
        );
        assembly.check_declarations = eligible;
        assembly.visit(scene.root())?;
        assembly
            .finish()?
            .freeze_in(scene, reuse_inputs.map(|call| call.retention))?
    };
    if reuse_inputs.is_some() {
        scene.validate_input_proofs("flow assembly adoption")?;
    }
    // No unproved inline measurement or fragment renderer callback may be omitted.
    let measurement_key = if purpose == FlowBuildPurpose::Measure
        && eligible
        && assembly.decorations.is_empty()
        && assembly
            .paragraphs
            .iter()
            .all(|paragraph| paragraph.boxes.is_empty())
    {
        context.cache_identity().map(|engine| FlowMeasurementKey {
            input: first.as_ref().expect("eligible input").clone(),
            engine,
            engine_revision: context.revision(),
            bounds,
            available_width,
            requested_constraints,
            constraints: scene.measurement_constraints(),
        })
    } else {
        None
    };
    if let Some(key) = &measurement_key
        && let Some(record) = work.measurements.iter().find(|record| &record.key == key)
        && scene.reuse_observations(&record.observations)?
    {
        let _timing = scene.budget().time("flow-measurement-reuse");
        scene.validate_input_proofs("flow layout return")?;
        scene
            .budget()
            .check()
            .map_err(crate::text::TextError::from)?;
        return Ok(FlowBuildOutput::Measurement(record.size, None));
    }
    work.statistics.builds += 1;
    allocate(
        assembly
            .paragraphs
            .len()
            .saturating_mul(assembly.decorations.len().saturating_add(3))
            .saturating_add(assembly.members.len()),
    )?;
    let mut snapshot = FlowSnapshot {
        statistics: work.statistics,
        revision: flow_revision(context, bounds, scene)?,
        paragraphs: Vec::new(),
        fragments: Vec::new(),
        placements: HashMap::new(),
        size: Dimension::new(available_width.unwrap_or(0.0), 0.0),
        borders: Vec::new(),
        source_fragments: HashMap::new(),
        source_ranges: HashMap::new(),
        paint_index: crate::text::outline::paint_index::EnvelopeIndex::from_envelopes(&[]),
        outline_visual_bounds: None,
        _assembly: assembly.clone(),
    };
    let mut y = bounds.y;
    let mut paragraph_padding = Vec::new();
    let mut paragraph_right_edges = Vec::new();
    let mut decoration_extents = HashMap::new();
    for (index, paragraph) in assembly.paragraphs.iter().enumerate() {
        for decoration in &assembly.decorations {
            if paragraph
                .sources
                .iter()
                .any(|s| s.ancestors.contains(&decoration.source))
                || paragraph
                    .boxes
                    .iter()
                    .any(|b| b.ancestors.contains(&decoration.source))
            {
                decoration_extents
                    .entry(decoration.source)
                    .and_modify(|extent: &mut (usize, usize)| extent.1 = index)
                    .or_insert((index, index));
            }
        }
    }
    for (paragraph_index, p) in assembly.paragraphs.iter().enumerate() {
        // Only inline work is mutable and width-dependent; paragraph content and
        // source ancestry stay borrowed from the immutable assembly.
        allocate(p.boxes.iter().map(|b| b.ancestors.len() + 1).sum())?;
        let mut paragraph_boxes = p.boxes.clone();
        scene
            .budget()
            .allocate(
                "flow paragraph shaping",
                (p.content
                    .text()
                    .len()
                    .saturating_add(p.boxes.len())
                    .saturating_add(p.sources.len())) as u64,
            )
            .map_err(|error| {
                crate::text::TextError::PreparationDependency(Box::new(
                    crate::figure::PreparationDependencyError::Budget(error),
                ))
            })?;
        for decoration in &assembly.decorations {
            let sources = p
                .sources
                .iter()
                .filter(|s| s.ancestors.contains(&decoration.source))
                .collect::<Vec<_>>();
            let decorated_boxes = p
                .boxes
                .iter()
                .filter(|b| b.ancestors.contains(&decoration.source))
                .collect::<Vec<_>>();
            if sources.is_empty() && decorated_boxes.is_empty() {
                continue;
            }
            let metrics = decoration.metrics;
            let is_block = assembly
                .blocks
                .iter()
                .any(|b| b.source == decoration.source);
            let (left, right) = if is_block {
                (0.0, 0.0)
            } else {
                (
                    metrics.insets.left + metrics.margin.left,
                    metrics.insets.right + metrics.margin.right,
                )
            };
            let (leading_space, trailing_space) =
                if p.block.direction == crate::text::ParagraphDirection::RightToLeft {
                    (right, left)
                } else {
                    (left, right)
                };
            let first_offset = sources
                .iter()
                .map(|s| s.range.start)
                .chain(decorated_boxes.iter().map(|b| b.offset))
                .min()
                .expect("nonempty decoration");
            let last_offset = sources
                .iter()
                .map(|s| s.range.end)
                .chain(decorated_boxes.iter().map(|b| b.offset))
                .max()
                .expect("nonempty decoration");
            let source_ancestors = sources
                .first()
                .map(|s| &s.ancestors)
                .or_else(|| decorated_boxes.first().map(|b| &b.ancestors))
                .expect("nonempty decoration");
            let ancestors = source_ancestors
                .iter()
                .copied()
                .take_while(|s| *s != decoration.source)
                .chain(std::iter::once(decoration.source))
                .collect::<Vec<_>>();
            let leading = super::assembly::BoxSource {
                child: None,
                metrics: Some(Dimension::new(
                    metrics.leading_width + leading_space,
                    metrics.height,
                )),
                border_marker: Some((decoration.source, true)),
                offset: first_offset,
                ancestors: ancestors.clone(),
            };
            let trailing = super::assembly::BoxSource {
                child: None,
                metrics: Some(Dimension::new(
                    metrics.trailing_width + trailing_space,
                    metrics.height,
                )),
                border_marker: Some((decoration.source, false)),
                offset: last_offset,
                ancestors,
            };
            let extent = decoration_extents[&decoration.source];
            if paragraph_index == extent.0 {
                let start = paragraph_boxes.partition_point(|b| b.offset < leading.offset);
                paragraph_boxes.insert(start, leading);
            }
            if paragraph_index == extent.1 {
                let end = paragraph_boxes.partition_point(|b| b.offset <= trailing.offset);
                paragraph_boxes.insert(end, trailing);
            }
        }
        // Equal byte offsets still have distinct opening/closing events in the tree.
        // In particular a parent's opening precedes its child's, and its closing follows it.
        paragraph_boxes.sort_by_key(|b| {
            let order = if let Some((source, leading)) = b.border_marker {
                let (opening, closing) = assembly.traversal_order[&source];
                if leading { opening } else { closing }
            } else {
                assembly.traversal_order[b.ancestors.last().expect("box owner")].0
            };
            (b.offset, order)
        });
        let width = available_width
            .map(|width| (width - p.block.padding.width() - p.block.margin.width()).max(0.0));
        let constraints = TextConstraints::new(width.map(|width| width as f32))?;
        let mut boxes = Vec::new();
        for (id, source) in paragraph_boxes.iter().enumerate() {
            allocate(
                p.content
                    .text()
                    .len()
                    .saturating_add(p.content.spans().len())
                    .saturating_add(boxes.len())
                    .saturating_add(1),
            )?;
            if let Some(metrics) = source.metrics {
                boxes.push(TextInlineBox {
                    id: id as u64,
                    byte_offset: source.offset,
                    width: metrics.width as f32,
                    height: metrics.height as f32,
                });
                continue;
            }
            let child = source.child.ok_or(FlowError::InvalidStructure)?;
            let inline_prefix_timing = scene.budget().time("flow-inline-prefix");
            // Shape the prefix through the same engine to find the current line's remaining width.
            let prefix = &p.content.text()[..source.offset];
            let prefix_spans = p
                .content
                .spans()
                .iter()
                .filter_map(|s| {
                    let end = s.range.end.min(source.offset);
                    (s.range.start < end).then(|| crate::text::TextStyleSpan {
                        range: s.range.start..end,
                        style: s.style.clone(),
                    })
                })
                .collect();
            let request = StyledTextRequest::new(
                prefix,
                p.content.default_style().clone(),
                prefix_spans,
                boxes.clone(),
                constraints,
            )?
            .with_paragraph(
                p.block.direction,
                p.block
                    .alignment
                    .unwrap_or(crate::text::ParagraphAlignment::Leading),
                p.block.wrap,
            )
            .with_layout_direction(scene.style(p.owner)?.layout_direction);
            let (_, prefix_layout) = work.layout(
                context,
                &request,
                FlowLayoutCall {
                    scene,
                    purpose,
                    owner: p.owner,
                    scene_revision: snapshot.revision,
                    sources: &p.sources,
                    inline_prefix: Some(id),
                },
            )?;
            let occupied = prefix_layout
                .layout()
                .line_metrics()
                .last()
                .map_or(0.0, |line| f64::from(line.advance));
            let remaining = width.map(|width| (width - occupied).max(0.0));
            drop(inline_prefix_timing);
            let _inline_measure = scene.budget().time("flow-inline-measure");
            let mut measurement = scene
                .measure_figure(
                    context,
                    child,
                    MeasureConstraints::new(remaining, None)
                        .map_err(|_| FlowError::InvalidStructure)?,
                )
                .map_err(crate::text::TextError::from)?
                .preferred;
            if occupied > 0.0 && remaining.is_some_and(|remaining| measurement.width > remaining) {
                measurement = scene
                    .measure_figure(
                        context,
                        child,
                        MeasureConstraints::new(width, None)
                            .map_err(|_| FlowError::InvalidStructure)?,
                    )
                    .map_err(crate::text::TextError::from)?
                    .preferred;
            }
            boxes.push(TextInlineBox {
                id: id as u64,
                byte_offset: source.offset,
                width: measurement.width as f32,
                height: measurement.height as f32,
            });
        }
        let request = StyledTextRequest::from_content(p.content.clone(), boxes, constraints)?
            .with_paragraph(
                p.block.direction,
                p.block
                    .alignment
                    .unwrap_or(crate::text::ParagraphAlignment::Leading),
                p.block.wrap,
            )
            .with_layout_direction(scene.style(p.owner)?.layout_direction);
        let (asset_index, layout) = work.layout(
            context,
            &request,
            FlowLayoutCall {
                scene,
                purpose,
                owner: p.owner,
                scene_revision: snapshot.revision,
                sources: &p.sources,
                inline_prefix: None,
            },
        )?;
        allocate(
            layout
                .layout()
                .line_metrics()
                .len()
                .saturating_mul(assembly.decorations.len().saturating_add(1))
                .saturating_add(layout.runs().len()),
        )?;
        let mut line_padding = vec![(0.0_f64, 0.0_f64); layout.layout().line_metrics().len()];
        let mut decorated_lines = std::collections::HashSet::new();
        for decoration in &assembly.decorations {
            if assembly
                .blocks
                .iter()
                .any(|b| b.source == decoration.source)
            {
                continue;
            }
            let mut lines = std::collections::HashSet::new();
            for run in layout.runs() {
                if p.sources.iter().any(|source| {
                    source.ancestors.contains(&decoration.source)
                        && run.source_range.start < source.range.end
                        && source.range.start < run.source_range.end
                }) {
                    lines.insert(run.line_index);
                }
            }
            for placement in layout.inline_placements() {
                if paragraph_boxes[placement.id as usize]
                    .ancestors
                    .contains(&decoration.source)
                {
                    lines.insert(placement.line_index);
                }
            }
            for line in lines {
                // Nesting adds border space; siblings share the largest required extent.
                let chain = p
                    .sources
                    .iter()
                    .find(|source| source.ancestors.contains(&decoration.source))
                    .map(|source| &source.ancestors)
                    .or_else(|| {
                        paragraph_boxes
                            .iter()
                            .find(|source| source.ancestors.contains(&decoration.source))
                            .map(|source| &source.ancestors)
                    });
                if let Some(chain) = chain {
                    let (mut top, mut bottom) = (0.0, 0.0);
                    for ancestor in chain {
                        if let Some(border) =
                            assembly.decorations.iter().find(|d| d.source == *ancestor)
                            && !assembly.blocks.iter().any(|b| b.source == *ancestor)
                        {
                            top += border.metrics.insets.top + border.metrics.margin.top;
                            bottom += border.metrics.insets.bottom + border.metrics.margin.bottom;
                        }
                    }
                    line_padding[line].0 = line_padding[line].0.max(top);
                    line_padding[line].1 = line_padding[line].1.max(bottom);
                }
                decorated_lines.insert((decoration.source, line));
            }
        }
        let asset = &mut work.paragraphs[asset_index];
        let layout = if asset.padding == line_padding {
            if let Some(padded) = &asset.padded {
                padded.clone()
            } else {
                layout.with_line_padding(&line_padding)?
            }
        } else {
            let padded = layout.with_line_padding(&line_padding)?;
            asset.padding = line_padding;
            asset.padded = Some(padded.clone());
            asset.source_geometry.clear();
            padded
        };
        snapshot.size.width = snapshot.size.width.max(
            f64::from(layout.layout().width()) + p.block.padding.width() + p.block.margin.width(),
        );
        y += p.before;
        if purpose == FlowBuildPurpose::Measure {
            y += f64::from(layout.layout().height()) + p.after;
            continue;
        }
        let origin = Point::new(bounds.x + p.block.margin.left + p.block.padding.left, y);
        paragraph_right_edges.push(
            origin.x()
                + f64::from(layout.layout().width())
                + p.block.padding.right
                + p.block.margin.right,
        );
        let index = snapshot.paragraphs.len();
        let first_fragment = snapshot.fragments.len();
        let geometry_timing = scene.budget().time("flow-source-geometry");
        for source in p.sources.iter() {
            let range = TextRange::new(
                TextPosition::new(source.range.start, TextAffinity::Downstream),
                TextPosition::new(source.range.end, TextAffinity::Upstream),
            );
            let key = (source.range.start, source.range.end);
            let quads = if let Some(quads) = asset.source_geometry.get(&key) {
                quads.clone()
            } else {
                let quads: Arc<[SelectionQuad]> = layout
                    .layout()
                    .selection_geometry(range)
                    .map_err(|_| FlowError::InvalidStructure)?
                    .into();
                work.statistics.source_geometry_calls += 1;
                asset.source_geometry.insert(key, quads.clone());
                quads
            };
            allocate(
                quads
                    .len()
                    .saturating_mul(source.ancestors.len().saturating_add(1)),
            )?;
            for quad in quads.iter() {
                let b = translated(quad.bounds(), origin);
                for &ancestor in &source.ancestors {
                    snapshot.fragments.push(FlowFragment {
                        source: ancestor,
                        paragraph: index,
                        line: quad.line_index(),
                        bounds: b,
                    });
                }
            }
        }
        drop(geometry_timing);
        for placement in layout.inline_placements() {
            let source = &paragraph_boxes[placement.id as usize];
            allocate(source.ancestors.len().saturating_add(1))?;
            let b = translated(placement.bounds, origin);
            if let Some(child) = source.child {
                snapshot.placements.insert(child, b);
            }
            for &ancestor in &source.ancestors {
                snapshot.fragments.push(FlowFragment {
                    source: ancestor,
                    paragraph: index,
                    line: placement.line_index,
                    bounds: b,
                });
            }
        }
        for decoration in &assembly.decorations {
            let mut lines = std::collections::BTreeMap::<usize, Rectangle>::new();
            for fragment in snapshot.fragments[first_fragment..]
                .iter()
                .filter(|f| f.source == decoration.source)
            {
                lines
                    .entry(fragment.line)
                    .and_modify(|b| *b = b.union(fragment.bounds))
                    .or_insert(fragment.bounds);
            }
            if let Some(block) = assembly
                .blocks
                .iter()
                .find(|b| b.source == decoration.source)
            {
                for (&line, b) in &mut lines {
                    let first = block.paragraphs.start == index && line == 0;
                    let last = block.paragraphs.end == index + 1
                        && line + 1 == layout.layout().line_metrics().len();
                    let top = if first {
                        decoration.metrics.insets.top
                    } else {
                        0.0
                    };
                    let bottom = if last {
                        decoration.metrics.insets.bottom
                    } else {
                        0.0
                    };
                    *b = Rectangle::new(
                        bounds.x + block.left,
                        b.y - top,
                        (available_width.unwrap_or(snapshot.size.width) - block.left - block.right)
                            .max(0.0),
                        b.height + top + bottom,
                    );
                }
            } else {
                for (&line, b) in &mut lines {
                    if decorated_lines.contains(&(decoration.source, line)) {
                        *b = Rectangle::new(
                            b.x,
                            b.y - decoration.metrics.insets.top,
                            b.width,
                            b.height + decoration.metrics.insets.height(),
                        );
                    }
                }
            }
            let first = lines.keys().next().copied();
            let last = lines.keys().next_back().copied();
            for (line, b) in lines {
                let marker = |leading| {
                    layout.inline_placements().iter().find_map(|placement| {
                        (placement.line_index == line
                            && paragraph_boxes[placement.id as usize].border_marker
                                == Some((decoration.source, leading)))
                        .then(|| translated(placement.bounds, origin))
                    })
                };
                snapshot.borders.push(super::border::BorderDrawing {
                    fragment: super::FlowBorderFragment {
                        source: decoration.source,
                        paragraph: index,
                        line,
                        bounds: b,
                        direction: if p.block.direction == crate::text::ParagraphDirection::Auto {
                            if layout.runs().first().is_some_and(|r| r.right_to_left) {
                                crate::text::ParagraphDirection::RightToLeft
                            } else {
                                crate::text::ParagraphDirection::LeftToRight
                            }
                        } else {
                            p.block.direction
                        },
                        style: {
                            let style = scene.style(decoration.source)?;
                            crate::text::TextStyle::new(
                                scene.parsed_font(decoration.source)?,
                                style.foreground,
                            )
                        },
                        leading: Some(line) == first
                            && decoration_extents[&decoration.source].0 == index,
                        trailing: Some(line) == last
                            && decoration_extents[&decoration.source].1 == index,
                        leading_box: marker(true),
                        trailing_box: marker(false),
                    },
                    border: decoration.border.clone(),
                });
            }
        }
        paragraph_padding.push(p.top_padding);
        y += f64::from(layout.layout().height()) + p.after;
        snapshot.paragraphs.push(FlowParagraphSnapshot {
            owner: p.owner,
            origin,
            layout,
            sources: p.sources.clone(),
        });
    }
    snapshot.size.height = (y - bounds.y + assembly.trailing_margin).max(0.0);
    if purpose == FlowBuildPurpose::Measure {
        let candidate =
            assembly_candidate(first, eligible, was_hit, &assembly, scene, reuse_inputs)?;
        if let (Some(key), Some(call)) = (measurement_key, reuse_inputs)
            && context.cache_identity() == Some(key.engine)
            && context.revision() == key.engine_revision
        {
            let observations = scene.observations();
            let units = std::mem::size_of::<FlowMeasurementRecord>().saturating_add(
                observations.capacity().saturating_mul(std::mem::size_of::<
                    crate::connection::DependencyObservation,
                >()),
            );
            scene
                .budget()
                .check()
                .map_err(crate::text::TextError::from)?;
            if units.saturating_add(super::reuse::input_units(&key.input)) <= call.cache_limit {
                scene
                    .budget()
                    .allocate("flow measurement record", units as u64)
                    .map_err(crate::text::TextError::from)?;
                let retention = call
                    .retention
                    .reserve(units)
                    .map_err(crate::text::TextError::from)?;
                work.measurements.retain(|record| record.key != key);
                if work.measurements.len() == MEASUREMENT_CACHE_CAPACITY {
                    work.measurements.remove(0);
                }
                work.measurements.reserve_exact(1);
                work.measurements.push(FlowMeasurementRecord {
                    key,
                    size: snapshot.size,
                    observations,
                    retention,
                });
            }
        }
        return Ok(FlowBuildOutput::Measurement(snapshot.size, candidate));
    }
    for block in &assembly.blocks {
        let first = &snapshot.paragraphs[block.paragraphs.start];
        let last = &snapshot.paragraphs[block.paragraphs.end - 1];
        let top =
            first.origin.y() - (paragraph_padding[block.paragraphs.start] - block.padding_above);
        let bottom =
            last.origin.y() + f64::from(last.layout.layout().height()) + block.bottom_padding;
        snapshot.placements.insert(
            block.source,
            Rectangle::new(
                bounds.x + block.left,
                top,
                (paragraph_right_edges[block.paragraphs.clone()]
                    .iter()
                    .copied()
                    .fold(bounds.x + bounds.width, f64::max)
                    - block.right
                    - bounds.x
                    - block.left)
                    .max(0.0),
                (bottom - top).max(0.0),
            ),
        );
    }
    // All source envelopes are derived from the same fragment revision.
    for fragment in &snapshot.fragments {
        snapshot
            .placements
            .entry(fragment.source)
            .and_modify(|b| *b = b.union(fragment.bounds))
            .or_insert(fragment.bounds);
    }
    snapshot.placements.insert(
        scene.root(),
        Rectangle::new(0.0, 0.0, snapshot.size.width, snapshot.size.height),
    );
    for &member in &assembly.members {
        snapshot.placements.entry(member).or_insert(Rectangle::ZERO);
    }
    allocate(snapshot.fragments.len())?;
    for (index, fragment) in snapshot.fragments.iter().enumerate() {
        snapshot
            .source_fragments
            .entry(fragment.source)
            .or_default()
            .push(index);
    }
    for (paragraph, p) in snapshot.paragraphs.iter().enumerate() {
        allocate(p.sources.len())?;
        for (source_index, source) in p.sources.iter().enumerate() {
            snapshot
                .source_ranges
                .entry(source.source)
                .or_default()
                .push((paragraph, source_index));
        }
    }
    use crate::text::outline::paint_index::{Envelope, EnvelopeIndex};
    let envelopes = snapshot
        .paragraphs
        .iter()
        .map(|paragraph| {
            paragraph
                .layout
                .paint_runs()
                .fold(Envelope::Empty, |bounds, (layout, _)| {
                    let ink = match layout.outlines() {
                        None => Envelope::Unknown,
                        Some(outlines) => match outlines.ink_bounds() {
                            None => Envelope::Empty,
                            Some(ink) => crate::text::outline::paint_index::transformed_bounds(
                                ink,
                                crate::Affine2D::from_translation(
                                    paragraph.origin.x(),
                                    paragraph.origin.y(),
                                ),
                            )
                            .map_or(Envelope::Unknown, Envelope::Known),
                        },
                    };
                    bounds.union(ink)
                })
        })
        .collect::<Vec<_>>();
    allocate(envelopes.len().saturating_mul(2))?;
    snapshot.outline_visual_bounds = envelopes
        .iter()
        .filter_map(|envelope| match envelope {
            Envelope::Known(bounds) => Some(*bounds),
            _ => None,
        })
        .reduce(|a, b| a.union(b));
    snapshot.paint_index = EnvelopeIndex::from_envelopes(&envelopes);
    snapshot.statistics = work.statistics;
    let candidate = assembly_candidate(first, eligible, was_hit, &assembly, scene, reuse_inputs)?;
    Ok(FlowBuildOutput::Snapshot(
        Arc::new(snapshot),
        assembly.members.clone(),
        candidate,
    ))
}

fn assembly_candidate(
    first: Option<crate::figure::PreparationInputSnapshot>,
    eligible: bool,
    was_hit: bool,
    assembly: &Arc<super::assembly::FrozenFlowPlan>,
    scene: &PreparationScene<'_>,
    reuse: Option<AssemblyReuseCall<'_>>,
) -> Result<Option<AssemblyCandidate>, FlowError> {
    if first.is_some() {
        scene.validate_input_proofs("flow layout return")?;
        scene
            .budget()
            .check()
            .map_err(crate::text::TextError::from)?;
    }
    // No renderer is reused without its own complete declaration.
    if eligible
        && !was_hit
        && assembly.decorations.is_empty()
        && reuse.is_some_and(|call| {
            assembly
                .retained_units()
                .saturating_add(first.as_ref().map_or(0, super::reuse::input_units))
                <= call.cache_limit
        })
    {
        return Ok(Some((
            first.expect("eligible input"),
            assembly.clone(),
            scene.observations(),
        )));
    }
    Ok(None)
}

fn prepare_snapshot(
    snapshot: Arc<FlowSnapshot>,
    members: &[FigureId],
    bounds: Rectangle,
    scene: &PreparationScene<'_>,
) -> Result<PreparedScene, FlowError> {
    let _timing = scene.budget().time("flow-projection");
    let mut visual_bounds =
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height).union(Rectangle::new(
            bounds.x,
            bounds.y,
            snapshot.size.width,
            snapshot.size.height,
        ));
    if let Some(ink) = snapshot.outline_visual_bounds {
        visual_bounds = visual_bounds.union(ink);
    }
    let root = FigurePresentation::new(
        FigureMeasurement::new(snapshot.size.width, snapshot.size.height, None),
        snapshot.size,
        visual_bounds,
        snapshot.clone(),
    )?
    .with_data(snapshot.clone());
    let mut result = PreparedScene::new(root);
    let mut outputs = HashMap::<FigureId, LayoutOutput>::new();
    let members: std::collections::HashSet<_> = members.iter().copied().collect();
    for (&source, &global) in &snapshot.placements {
        if source == scene.root() {
            continue;
        }
        let parent = scene.parent(source)?.ok_or(FlowError::InvalidStructure)?;
        let parent_bounds = *snapshot
            .placements
            .get(&parent)
            .ok_or(FlowError::InvalidStructure)?;
        let client = scene.client_area(parent)?;
        let local = Rectangle::new(
            global.x - parent_bounds.x - client.x,
            global.y - parent_bounds.y - client.y,
            global.width,
            global.height,
        );
        outputs
            .entry(parent)
            .or_default()
            .set_child_bounds(source, local);
        if members.contains(&source) {
            let regions = snapshot
                .source_fragments
                .get(&source)
                .into_iter()
                .flatten()
                .map(|&index| &snapshot.fragments[index])
                .map(|f| {
                    Rectangle::new(
                        f.bounds.x - global.x,
                        f.bounds.y - global.y,
                        f.bounds.width,
                        f.bounds.height,
                    )
                })
                .collect();
            let insets = scene.insets(source)?;
            let presentation = FigurePresentation::new(
                FigureMeasurement::new(
                    (global.width - insets.width()).max(0.0),
                    (global.height - insets.height()).max(0.0),
                    None,
                ),
                Dimension::new(0.0, 0.0),
                Rectangle::new(0.0, 0.0, global.width, global.height),
                Arc::new(EmptyDrawing),
            )?
            .with_hit_regions(regions)?
            .with_data(snapshot.clone());
            result.present(source, presentation);
        }
    }
    for (parent, output) in outputs {
        result.layout(parent, output);
    }
    Ok(result)
}

#[cfg(test)]
mod measurement_tests {
    use super::*;
    use crate::{
        figure::{FigureStyle, RectangleFigure, flow::*},
        geometry::Insets,
        text::{BuiltinFont, FontDescriptor, ParagraphDirection, TextSystem},
        tree::FigureTree,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn paragraph_asset(
        context: &mut MeasureContext<'_>,
        request: &StyledTextRequest,
    ) -> ParagraphAsset {
        ParagraphAsset {
            engine_identity: context.cache_identity(),
            engine_revision: context.revision(),
            layout: context.layout_styled(request).unwrap(),
            padding: Vec::new(),
            padded: None,
            source_geometry: HashMap::new(),
        }
    }

    fn cache_request(text: String, width: f32) -> StyledTextRequest {
        StyledTextRequest::new(
            text,
            crate::text::TextStyle::new(FontDescriptor::default(), crate::graphics::Color::BLACK),
            Vec::new(),
            Vec::new(),
            TextConstraints::new(Some(width)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn paragraph_index_checks_complete_request_engine_and_revision_on_collisions() {
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let first = cache_request("same text".into(), 75.0);
        let second = cache_request("same text".into(), 240.0);
        let mut work = FlowWork::default();
        let first_index = work.cache_paragraph(paragraph_asset(&mut context, &first));
        let second_index = work.cache_paragraph(paragraph_asset(&mut context, &second));
        let identity = context.cache_identity();
        let revision = context.revision();
        assert_eq!(
            work.cached_paragraph(identity, revision, &first),
            Some(first_index)
        );
        assert_eq!(
            work.cached_paragraph(identity, revision, &second),
            Some(second_index)
        );
        assert_eq!(work.cached_paragraph(None, revision, &first), None);
        assert_eq!(
            work.cached_paragraph(Some(Default::default()), revision, &first),
            None
        );
        assert_eq!(work.cached_paragraph(identity, revision + 1, &first), None);
        let different = cache_request("fake text".into(), 75.0);
        assert_eq!(first.text().len(), different.text().len());
        assert_eq!(work.cached_paragraph(identity, revision, &different), None);
    }

    #[test]
    fn paragraph_index_preserves_fifo_capacity_and_retained_layouts() {
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let requests = (0..PARAGRAPH_CACHE_CAPACITY * 2 + 1)
            .map(|index| cache_request(format!("paragraph {index}"), 240.0))
            .collect::<Vec<_>>();
        let mut work = FlowWork::default();
        let first = paragraph_asset(&mut context, &requests[0]);
        let retained = first.layout.clone();
        work.cache_paragraph(first);
        for request in &requests[1..] {
            work.cache_paragraph(paragraph_asset(&mut context, request));
        }
        assert_eq!(work.paragraphs.len(), PARAGRAPH_CACHE_CAPACITY);
        assert!(work.paragraph_lookup.len() <= PARAGRAPH_CACHE_CAPACITY);
        assert_eq!(
            work.paragraph_lookup.values().map(Vec::len).sum::<usize>(),
            PARAGRAPH_CACHE_CAPACITY
        );
        for (index, request) in requests.iter().enumerate() {
            let found =
                work.cached_paragraph(context.cache_identity(), context.revision(), request);
            assert_eq!(
                found.is_some(),
                index >= requests.len() - PARAGRAPH_CACHE_CAPACITY
            );
            if let Some(slot) = found {
                assert_eq!(work.paragraphs[slot].layout.request(), request);
            }
        }
        assert_eq!(retained.request(), &requests[0]);
        assert!(!retained.layout().line_metrics().is_empty());
    }

    #[test]
    fn paragraph_index_eviction_removes_only_the_retired_collision_candidate() {
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let mut work = FlowWork::default();
        // Equal byte lengths share one bucket; full requests remain different.
        let requests = (0..=PARAGRAPH_CACHE_CAPACITY)
            .map(|index| cache_request(format!("collision {index:04}"), 240.0))
            .collect::<Vec<_>>();
        for request in &requests {
            work.cache_paragraph(paragraph_asset(&mut context, request));
        }
        assert_eq!(work.paragraph_lookup.len(), 1);
        assert_eq!(
            work.paragraph_lookup[&requests[0].text().len()].len(),
            PARAGRAPH_CACHE_CAPACITY
        );
        assert_eq!(
            work.cached_paragraph(context.cache_identity(), context.revision(), &requests[0]),
            None
        );
        for request in &requests[1..] {
            let index = work
                .cached_paragraph(context.cache_identity(), context.revision(), request)
                .unwrap();
            assert_eq!(work.paragraphs[index].layout.request(), request);
        }
    }

    struct MeasuredBorder(Arc<AtomicUsize>);
    impl FlowFragmentBorder for MeasuredBorder {
        fn metrics(
            &self,
            _: &mut MeasureContext<'_>,
            _: &crate::text::TextStyle,
        ) -> Result<FlowBorderMetrics, FlowError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(FlowBorderMetrics {
                insets: Insets::new(3.0, 5.0, 7.0, 9.0),
                margin: Insets::new(2.0, 4.0, 6.0, 8.0),
                ..FlowBorderMetrics::default()
            })
        }
        fn paint_fragment(
            &self,
            _: &FlowBorderFragment,
            _: &mut PaintContext<'_>,
        ) -> Result<(), GraphicsError> {
            Ok(())
        }
    }

    #[test]
    fn exported_sources_keep_only_their_own_lease_after_all_other_owners_drop() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
            .unwrap();
        let text = tree
            .builder()
            .add_child(root, Box::new(StructuredTextFigure::new("shared source")))
            .unwrap();
        let styles = [root, text]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        let store = service.retention.clone();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let prepared = service
            .prepare(
                &mut system.measure(FontDescriptor::default()),
                Rectangle::new(0.0, 0.0, 100.0, 300.0),
                &scene,
            )
            .unwrap();
        let ranges = prepared
            .root_presentation()
            .data::<FlowSnapshot>()
            .unwrap()
            .paragraphs[0]
            .sources
            .clone();
        drop(prepared);
        drop(scene);
        drop(service);
        drop(system);
        assert_eq!(ranges[0].source, text);
        assert!(store.usage().unwrap().retained_units > 0);
        assert_eq!(store.usage().unwrap().content_count, 0);
        let peer = ranges.clone();
        let usage = store.usage().unwrap();
        drop(ranges);
        assert_eq!(store.usage().unwrap(), usage);
        drop(peer);
        assert_eq!(store.usage().unwrap().retained_units, 0);
    }

    #[test]
    fn snapshot_borrows_immutable_content_and_source_backing_after_service_drop() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
            .unwrap();
        let text = tree
            .builder()
            .add_child(root, Box::new(StructuredTextFigure::new("alpha 中 beta")))
            .unwrap();
        let styles = [root, text]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let service = FlowLayoutService::default();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let (snapshot, _) = build_snapshot(
            &service.registry,
            &mut context,
            Rectangle::new(0.0, 0.0, 100.0, 300.0),
            Some(100.0),
            &scene,
            &mut service.statistics.lock().unwrap(),
        )
        .unwrap();
        let paragraph = &snapshot.paragraphs[0];
        let assembly = &snapshot._assembly.paragraphs[0];
        assert!(std::ptr::eq(
            paragraph.sources.as_ptr(),
            assembly.sources.as_ptr()
        ));
        assert!(std::ptr::eq(
            paragraph.layout.request().text().as_ptr(),
            assembly.content.text().as_ptr(),
        ));
        assert!(std::ptr::eq(
            paragraph.layout.request().spans().as_ptr(),
            assembly.content.spans().as_ptr(),
        ));
        assert!(std::ptr::eq(
            paragraph.sources[0].ancestors.as_ptr(),
            assembly.sources[0].ancestors.as_ptr(),
        ));
        let retained = snapshot.clone();
        drop(snapshot);
        drop(service);
        assert_eq!(
            retained.paragraphs[0].layout.request().text(),
            "alpha 中 beta"
        );
        assert_eq!(retained.paragraphs[0].sources[0].source, text);
        assert_eq!(retained.paragraphs[0].sources[0].ancestors, [root, text]);
    }

    #[test]
    fn declared_text_measurements_reuse_full_inputs_and_reject_budget_failure() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
            .unwrap();
        let text = tree
            .builder()
            .add_child(
                root,
                Box::new(StructuredTextFigure::new("alpha אבג beta wrap words")),
            )
            .unwrap();
        let styles = [root, text]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let narrow = MeasureConstraints::new(Some(75.0), None).unwrap();
        let wide = MeasureConstraints::new(Some(240.0), None).unwrap();
        let first = service.measure(&mut context, narrow, &scene).unwrap();
        let second = service.measure(&mut context, wide, &scene).unwrap();
        let builds = service.work_statistics().unwrap().builds;
        assert_eq!(builds, 2);
        let fresh = PreparationScene::new(&tree, root, &styles).unwrap();
        assert_eq!(
            service.measure(&mut context, narrow, &fresh).unwrap(),
            first
        );
        assert_eq!(service.measure(&mut context, wide, &fresh).unwrap(), second);
        assert_eq!(service.work_statistics().unwrap().builds, builds);
        assert_eq!(service.statistics.lock().unwrap().measurements.len(), 2);
        let height_limited = MeasureConstraints::new(Some(75.0), Some(10.0)).unwrap();
        assert_eq!(
            service
                .measure(&mut context, height_limited, &fresh)
                .unwrap(),
            first
        );
        assert_eq!(service.work_statistics().unwrap().builds, builds + 1);
        // Equal revisions from another engine instance do not grant authority.
        let mut other_system = TextSystem::new();
        other_system
            .register_builtin_font(BuiltinFont::Inter)
            .unwrap();
        service
            .measure(
                &mut other_system.measure(FontDescriptor::default()),
                narrow,
                &fresh,
            )
            .unwrap();
        assert_eq!(service.work_statistics().unwrap().builds, builds + 2);
        assert!(
            scene
                .budget()
                .consume("exhaust measurement budget", u64::MAX)
                .is_err()
        );
        assert!(service.measure(&mut context, narrow, &scene).is_err());
        service.set_assembly_limits(0, usize::MAX).unwrap();
        assert!(service.statistics.lock().unwrap().measurements.is_empty());
        let uncached = PreparationScene::new(&tree, root, &styles).unwrap();
        assert_eq!(
            service.measure(&mut context, narrow, &uncached).unwrap(),
            first
        );
        assert!(service.statistics.lock().unwrap().measurements.is_empty());
    }

    #[test]
    fn measurement_matches_preparation_and_rechecks_border_metrics() {
        for width in [None, Some(75.0), Some(240.0)] {
            let mut tree = FigureTree::new();
            let root = tree
                .builder()
                .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
                .unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            let block = tree
                .builder()
                .add_child(
                    root,
                    Box::new(
                        BlockFlowFigure::new(FlowBlockStyle {
                            direction: ParagraphDirection::RightToLeft,
                            padding: Insets::new(1.0, 2.0, 3.0, 4.0),
                            margin: Insets::new(2.0, 3.0, 4.0, 5.0),
                            ..FlowBlockStyle::default()
                        })
                        .unwrap()
                        .with_style(FigureStyle {
                            font: Some(crate::text::FontDescriptor::parse("16px Inter").unwrap()),
                            ..FigureStyle::default()
                        })
                        .with_fragment_border(MeasuredBorder(calls.clone())),
                    ),
                )
                .unwrap();
            let text = tree
                .builder()
                .add_child(
                    block,
                    Box::new(StructuredTextFigure::new("alpha אבג beta ")),
                )
                .unwrap();
            let adapter = tree
                .builder()
                .add_child(block, Box::new(FlowAdapterFigure::new()))
                .unwrap();
            let inline = tree
                .builder()
                .add_child(
                    adapter,
                    Box::new(RectangleFigure::new(0.0, 0.0, 33.0, 21.0).unwrap()),
                )
                .unwrap();
            let tail = tree
                .builder()
                .add_child(
                    block,
                    Box::new(StructuredTextFigure::new(" trailing words")),
                )
                .unwrap();
            let styles = [root, block, text, adapter, inline, tail]
                .into_iter()
                .map(|id| (id, tree.resolved_style(id).unwrap()))
                .collect();
            let mut system = TextSystem::new();
            system.register_builtin_font(BuiltinFont::Inter).unwrap();
            let mut context = system.measure(FontDescriptor::default());
            let service = FlowLayoutService::default();
            let constraints = MeasureConstraints::new(width, None).unwrap();
            let scene = PreparationScene::new(&tree, root, &styles).unwrap();
            let measured = service.measure(&mut context, constraints, &scene).unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), 1);
            assert_eq!(
                service
                    .statistics
                    .lock()
                    .unwrap()
                    .statistics
                    .source_geometry_calls,
                0
            );
            assert!(service.cache.lock().unwrap().is_none());
            let fresh = PreparationScene::new(&tree, root, &styles).unwrap();
            let (prepared, _) = build_snapshot(
                &service.registry,
                &mut context,
                Rectangle::ZERO,
                width,
                &fresh,
                &mut service.statistics.lock().unwrap(),
            )
            .unwrap();
            assert_eq!(measured.minimum, prepared.size);
            assert_eq!(measured.preferred.width, prepared.size.width);
            assert_eq!(measured.preferred.height, prepared.size.height);
            assert_eq!(calls.load(Ordering::Relaxed), 2);
            assert!(prepared.placements.contains_key(&inline));
            assert!(
                scene
                    .budget()
                    .consume("exhaust measurement budget", u64::MAX)
                    .is_err()
            );
            assert!(service.measure(&mut context, constraints, &scene).is_err());
        }
    }
}
