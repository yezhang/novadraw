use super::{FlowAssembly, FlowContributionRegistry, FlowError, FlowSourceRange};
use crate::{
    FigureId, FigureMeasurement, MeasureConstraints,
    figure::{FigureDrawing, FigurePresentation, PreparationScene, PreparedScene},
    geometry::{Dimension, Point, Rectangle},
    graphics::{GraphicsError, PaintContext},
    layout::LayoutOutput,
    text::{
        CaretGeometry, MeasureContext, SelectionQuad, StyledTextLayout, StyledTextRequest,
        TextAffinity, TextConstraints, TextInlineBox, TextInteractionError, TextPosition,
        TextRange,
    },
};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

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
    pub revision: u64,
    pub paragraphs: Vec<FlowParagraphSnapshot>,
    pub fragments: Vec<FlowFragment>,
    pub placements: HashMap<FigureId, Rectangle>,
    pub size: Dimension,
    borders: Vec<super::border::BorderDrawing>,
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
        for (index, p) in self.paragraphs.iter().enumerate() {
            for source in &p.sources {
                if source.source == position.source
                    && position.byte_offset >= source.source_offset
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
        for paragraph in &self.paragraphs {
            context.fill_styled_text(&paragraph.layout, paragraph.origin)?;
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

pub(crate) fn prepare(
    registry: &FlowContributionRegistry,
    context: &mut MeasureContext<'_>,
    bounds: Rectangle,
    scene: &PreparationScene<'_>,
) -> Result<PreparedScene, FlowError> {
    let mut assembly = FlowAssembly::new(scene, registry);
    assembly.visit(scene.root())?;
    assembly.finish();
    let mut revision = std::collections::hash_map::DefaultHasher::new();
    scene.revision()?.hash(&mut revision);
    context.revision().hash(&mut revision);
    let mut snapshot = FlowSnapshot {
        revision: revision.finish(),
        paragraphs: Vec::new(),
        fragments: Vec::new(),
        placements: HashMap::new(),
        size: Dimension::new(bounds.width, 0.0),
        borders: Vec::new(),
    };
    let mut y = bounds.y;
    for mut p in assembly.paragraphs {
        let mut renderers = HashMap::new();
        for decoration in &assembly.decorations {
            let sources = p
                .sources
                .iter()
                .filter(|s| s.ancestors.contains(&decoration.source))
                .collect::<Vec<_>>();
            if sources.is_empty() {
                continue;
            }
            let resolved = scene.style(decoration.source)?;
            let style = crate::text::TextStyle::new(
                crate::text::FontDescriptor::parse(&resolved.font)?,
                resolved.foreground,
            );
            context.set_font(style.font().clone());
            let renderer = decoration
                .border
                .prepare_renderer(context, &style)?
                .unwrap_or_else(|| decoration.border.clone());
            let metrics = renderer.metrics(context, &style)?.validate()?;
            renderers.insert(decoration.source, renderer);
            let first = sources.first().expect("nonempty sources");
            let last = sources.last().expect("nonempty sources");
            let ancestors = first
                .ancestors
                .iter()
                .copied()
                .take_while(|s| *s != decoration.source)
                .chain(std::iter::once(decoration.source))
                .collect::<Vec<_>>();
            let leading = super::assembly::BoxSource {
                child: None,
                metrics: Some(Dimension::new(metrics.leading_width, metrics.height)),
                border_marker: Some((decoration.source, true)),
                offset: first.range.start,
                ancestors: ancestors.clone(),
            };
            let trailing = super::assembly::BoxSource {
                child: None,
                metrics: Some(Dimension::new(metrics.trailing_width, metrics.height)),
                border_marker: Some((decoration.source, false)),
                offset: last.range.end,
                ancestors,
            };
            let start = p.boxes.partition_point(|b| b.offset < leading.offset);
            p.boxes.insert(start, leading);
            let end = p.boxes.partition_point(|b| b.offset <= trailing.offset);
            p.boxes.insert(end, trailing);
        }
        let width = (bounds.width - p.block.padding.width() - p.block.margin.width()).max(0.0);
        let constraints = TextConstraints::new(Some(width as f32))?;
        let mut boxes = Vec::new();
        for (id, source) in p.boxes.iter().enumerate() {
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
            .with_paragraph(p.block.direction, p.block.alignment, p.block.wrap);
            let prefix_layout = context.layout_styled(&request)?;
            let occupied = prefix_layout
                .layout()
                .line_metrics()
                .last()
                .map_or(0.0, |line| f64::from(line.advance));
            let remaining = (width - occupied).max(0.0);
            let mut measurement = scene.preferred_measurement(
                child,
                MeasureConstraints::width(remaining).map_err(|_| FlowError::InvalidStructure)?,
            )?;
            if occupied > 0.0 && measurement.width > remaining {
                measurement = scene.preferred_measurement(
                    child,
                    MeasureConstraints::width(width).map_err(|_| FlowError::InvalidStructure)?,
                )?;
            }
            boxes.push(TextInlineBox {
                id: id as u64,
                byte_offset: source.offset,
                width: measurement.width as f32,
                height: measurement.height as f32,
            });
        }
        let request = StyledTextRequest::new(p.text, p.style, p.spans, boxes, constraints)?
            .with_paragraph(p.block.direction, p.block.alignment, p.block.wrap);
        let layout = context.layout_styled(&request)?;
        snapshot.size.width = snapshot.size.width.max(
            f64::from(layout.layout().width()) + p.block.padding.width() + p.block.margin.width(),
        );
        y += p.before;
        let origin = Point::new(bounds.x + p.block.margin.left + p.block.padding.left, y);
        let index = snapshot.paragraphs.len();
        for source in &p.sources {
            let range = TextRange::new(
                TextPosition::new(source.range.start, TextAffinity::Downstream),
                TextPosition::new(source.range.end, TextAffinity::Upstream),
            );
            for quad in layout
                .layout()
                .selection_geometry(range)
                .map_err(|_| FlowError::InvalidStructure)?
            {
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
        for placement in layout.inline_placements() {
            let source = &p.boxes[placement.id as usize];
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
            for fragment in snapshot
                .fragments
                .iter()
                .filter(|f| f.paragraph == index && f.source == decoration.source)
            {
                lines
                    .entry(fragment.line)
                    .and_modify(|b| *b = b.union(fragment.bounds))
                    .or_insert(fragment.bounds);
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
                                crate::text::FontDescriptor::parse(&style.font)?,
                                style.foreground,
                            )
                        },
                        leading: Some(line) == first,
                        trailing: Some(line) == last,
                        leading_box: marker(true),
                        trailing_box: marker(false),
                    },
                    border: renderers
                        .get(&decoration.source)
                        .cloned()
                        .unwrap_or_else(|| decoration.border.clone()),
                });
            }
        }
        let owner_bounds = Rectangle::new(
            bounds.x + p.block.margin.left,
            y - p.block.padding.top,
            width.max(f64::from(layout.layout().width())) + p.block.padding.width(),
            f64::from(layout.layout().height()) + p.block.padding.height(),
        );
        snapshot
            .placements
            .entry(p.owner)
            .and_modify(|b| *b = b.union(owner_bounds))
            .or_insert(owner_bounds);
        y += f64::from(layout.layout().height()) + p.after;
        snapshot.paragraphs.push(FlowParagraphSnapshot {
            owner: p.owner,
            origin,
            layout,
            sources: p.sources,
        });
    }
    snapshot.size.height = (y - bounds.y).max(0.0);
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
        Rectangle::new(0.0, 0.0, bounds.width, snapshot.size.height),
    );
    for &member in &assembly.members {
        snapshot.placements.entry(member).or_insert(Rectangle::ZERO);
    }
    let snapshot = Arc::new(snapshot);
    let root = FigurePresentation::new(
        FigureMeasurement::new(snapshot.size.width, snapshot.size.height, None),
        Dimension::new(0.0, 0.0),
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height).union(Rectangle::new(
            bounds.x,
            bounds.y,
            snapshot.size.width,
            snapshot.size.height,
        )),
        snapshot.clone(),
    )?
    .with_data(snapshot.clone());
    let mut result = PreparedScene::new(root);
    let mut outputs = HashMap::<FigureId, LayoutOutput>::new();
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
        if assembly.members.contains(&source) {
            let regions = snapshot
                .fragments
                .iter()
                .filter(|f| f.source == source)
                .map(|f| {
                    Rectangle::new(
                        f.bounds.x - global.x,
                        f.bounds.y - global.y,
                        f.bounds.width,
                        f.bounds.height,
                    )
                })
                .collect();
            let presentation = FigurePresentation::new(
                FigureMeasurement::new(global.width, global.height, None),
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
