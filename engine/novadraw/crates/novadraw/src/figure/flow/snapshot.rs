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
    sync::{Arc, Mutex},
    time::Instant,
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

#[derive(Default)]
struct FlowWork {
    statistics: FlowWorkStatistics,
    paragraphs: Vec<ParagraphAsset>,
}

struct ParagraphAsset {
    engine_identity: Option<crate::text::shaping::TextLayoutCacheIdentity>,
    engine_revision: u64,
    fingerprint: u64,
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
    fn layout(
        &mut self,
        context: &mut MeasureContext<'_>,
        request: &StyledTextRequest,
        call: FlowLayoutCall<'_, '_>,
    ) -> Result<(usize, StyledTextLayout), crate::text::TextError> {
        let diagnostics = call.scene.budget().text_diagnostics();
        let diagnostic_start = diagnostics.0.is_enabled().then(Instant::now);
        let engine_revision = context.revision();
        let engine_identity = context.cache_identity();
        let build = self.statistics.builds;
        let observe = |cache_hit, succeeded, elapsed_ns| {
            diagnostics
                .0
                .observe_layout(|| super::FlowLayoutObservation {
                    root: call.scene.root(),
                    owner: call.owner,
                    build,
                    scene_revision: call.scene_revision,
                    sources: call
                        .sources
                        .iter()
                        .map(|source| {
                            (
                                source.source,
                                call.scene.diagnostic_component_revision(source.source),
                            )
                        })
                        .collect(),
                    phase: match call.purpose {
                        FlowBuildPurpose::Measure => "measure",
                        FlowBuildPurpose::Prepare => "prepare",
                    },
                    inline_prefix: call.inline_prefix,
                    engine_identity,
                    engine_revision,
                    elapsed_ns,
                    request: Arc::new(request.clone()),
                    cache_hit,
                    succeeded,
                });
        };
        let lookup = diagnostics.time("flow-paragraph-lookup");
        let fingerprint = request.fingerprint();
        if let Some(index) = self.paragraphs.iter().position(|asset| {
            engine_identity.is_some()
                && asset.engine_identity == engine_identity
                && asset.engine_revision == engine_revision
                && asset.fingerprint == fingerprint
                && asset.layout.request() == request
        }) {
            drop(lookup);
            observe(
                true,
                true,
                diagnostic_start.map_or(0, |start| start.elapsed().as_nanos()),
            );
            self.statistics.paragraph_hits += 1;
            return Ok((index, self.paragraphs[index].layout.clone()));
        }
        drop(lookup);
        let engine_timing = diagnostics.time("flow-engine-layout");
        let start = diagnostics.0.is_enabled().then(Instant::now);
        let result = context.layout_styled_with_diagnostics(request, &diagnostics);
        drop(engine_timing);
        let engine_ns = start.map_or(0, |start| start.elapsed().as_nanos());
        observe(
            false,
            result.is_ok(),
            diagnostic_start.map_or(0, |start| start.elapsed().as_nanos()),
        );
        self.statistics.styled_layout_calls += 1;
        self.statistics.styled_layout_ns += engine_ns;
        let layout = result?;
        if self.paragraphs.len() == PARAGRAPH_CACHE_CAPACITY {
            self.paragraphs.remove(0);
        }
        self.paragraphs.push(ParagraphAsset {
            engine_identity,
            engine_revision,
            fingerprint,
            layout: layout.clone(),
            padding: Vec::new(),
            padded: None,
            source_geometry: HashMap::new(),
        });
        Ok((self.paragraphs.len() - 1, layout))
    }
}

/// Cumulative work for one instance-owned service. Timings require Runtime diagnostics enabled.
/// These counters describe CPU layout work, never GPU completion or presentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowWorkStatistics {
    pub builds: u64,
    pub styled_layout_calls: u64,
    pub paragraph_hits: u64,
    pub source_geometry_calls: u64,
    pub contribution_ns: u128,
    pub styled_layout_ns: u128,
    pub source_geometry_ns: u128,
    pub build_ns: u128,
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
}
impl FlowLayoutService {
    pub fn new(registry: FlowContributionRegistry) -> Self {
        Self {
            registry,
            cache: Arc::new(Mutex::new(None)),
            statistics: Arc::new(Mutex::new(FlowWork::default())),
        }
    }
    /// Measures a frozen contribution tree without publishing placements or drawing.
    /// Unbounded width is passed to the text engine as an unbounded constraint.
    pub fn measure(
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
            &mut *self
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
            FlowBuildOutput::Measurement(size) => Ok(PreparationMeasurement {
                preferred: FigureMeasurement::new(size.width, size.height, None),
                minimum: size,
            }),
            FlowBuildOutput::Snapshot(_, _) => unreachable!("measurement build purpose"),
        }
    }
    pub fn prepare(
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
            && revision == key
            && context.cache_identity().is_some()
            && scene.reuse_observations(&observations)?
        {
            return prepare_snapshot(snapshot, &members, bounds, scene);
        }
        let (snapshot, members) = build_snapshot(
            &self.registry,
            context,
            bounds,
            Some(bounds.width),
            scene,
            &mut *self
                .statistics
                .lock()
                .map_err(|_| FlowError::InvalidStructure)?,
        )?;
        let result = prepare_snapshot(snapshot.clone(), &members, bounds, scene)?;
        result.validate(scene)?;
        if flow_revision(context, bounds, scene)? != key {
            return Err(crate::text::TextError::InvalidPreparedScene(
                "flow preparation input changed during layout".into(),
            )
            .into());
        }
        *self.cache.lock().map_err(|_| FlowError::InvalidStructure)? =
            Some((key, snapshot, members, scene.observations()));
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
    pub sources: Vec<FlowSourceRange>,
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
enum FlowBuildOutput {
    Measurement(Dimension),
    Snapshot(Arc<FlowSnapshot>, Vec<FigureId>),
}

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
        work,
    )? {
        FlowBuildOutput::Snapshot(snapshot, members) => Ok((snapshot, members)),
        FlowBuildOutput::Measurement(_) => unreachable!("preparation build purpose"),
    }
}

fn build_flow(
    registry: &FlowContributionRegistry,
    context: &mut MeasureContext<'_>,
    bounds: Rectangle,
    available_width: Option<f64>,
    scene: &PreparationScene<'_>,
    purpose: FlowBuildPurpose,
    work: &mut FlowWork,
) -> Result<FlowBuildOutput, FlowError> {
    let _timing = scene.budget().time("flow-build");
    let timing_enabled = scene.budget().text_diagnostics().0.is_enabled();
    let build_start = timing_enabled.then(Instant::now);
    work.statistics.builds += 1;
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
    let contribution_timing = scene.budget().time("flow-contribution");
    let contribution_start = timing_enabled.then(Instant::now);
    let assembly = {
        let mut measure_border = |source, border: Arc<dyn super::FlowFragmentBorder>| {
            let resolved = scene.style(source)?;
            let style =
                crate::text::TextStyle::new(scene.parsed_font(source)?, resolved.foreground);
            context.set_font(style.font().clone());
            let renderer = border.prepare_renderer(context, &style)?.unwrap_or(border);
            let metrics = renderer.metrics(context, &style)?.validate()?;
            Ok((metrics, renderer))
        };
        let mut assembly = FlowAssembly::new(scene, registry, &mut measure_border);
        assembly.visit(scene.root())?;
        assembly.finish()
    };
    work.statistics.contribution_ns +=
        contribution_start.map_or(0, |start| start.elapsed().as_nanos());
    drop(contribution_timing);
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
    for (paragraph_index, mut p) in assembly.paragraphs.into_iter().enumerate() {
        scene
            .budget()
            .allocate(
                "flow paragraph shaping",
                (p.text
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
                let start = p.boxes.partition_point(|b| b.offset < leading.offset);
                p.boxes.insert(start, leading);
            }
            if paragraph_index == extent.1 {
                let end = p.boxes.partition_point(|b| b.offset <= trailing.offset);
                p.boxes.insert(end, trailing);
            }
        }
        // Equal byte offsets still have distinct opening/closing events in the tree.
        // In particular a parent's opening precedes its child's, and its closing follows it.
        p.boxes.sort_by_key(|b| {
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
        for (id, source) in p.boxes.iter().enumerate() {
            allocate(
                p.text
                    .len()
                    .saturating_add(p.spans.len())
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
            // Shape the prefix through the same engine to find the current line's remaining width.
            let prefix = &p.text[..source.offset];
            let prefix_spans = p
                .spans
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
                p.style.clone(),
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
        let request = StyledTextRequest::new(p.text, p.style, p.spans, boxes, constraints)?
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
                if p.boxes[placement.id as usize]
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
                        p.boxes
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
        let geometry_start = timing_enabled.then(Instant::now);
        for source in &p.sources {
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
        work.statistics.source_geometry_ns +=
            geometry_start.map_or(0, |start| start.elapsed().as_nanos());
        drop(geometry_timing);
        for placement in layout.inline_placements() {
            let source = &p.boxes[placement.id as usize];
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
                            && p.boxes[placement.id as usize].border_marker
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
            sources: p.sources,
        });
    }
    snapshot.size.height = (y - bounds.y + assembly.trailing_margin).max(0.0);
    if purpose == FlowBuildPurpose::Measure {
        work.statistics.build_ns += build_start.map_or(0, |start| start.elapsed().as_nanos());
        return Ok(FlowBuildOutput::Measurement(snapshot.size));
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
    work.statistics.build_ns += build_start.map_or(0, |start| start.elapsed().as_nanos());
    snapshot.statistics = work.statistics;
    Ok(FlowBuildOutput::Snapshot(
        Arc::new(snapshot),
        assembly.members,
    ))
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
    fn measurement_matches_preparation_and_rechecks_border_metrics() {
        for width in [None, Some(75.0), Some(240.0)] {
            let mut tree = FigureTree::new();
            let root = tree
                .builder()
                .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO)))
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
                            font: Some("16px Inter".into()),
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
                    Box::new(RectangleFigure::new(0.0, 0.0, 33.0, 21.0)),
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
