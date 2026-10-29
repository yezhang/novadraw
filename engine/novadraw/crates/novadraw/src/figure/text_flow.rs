use unicode_segmentation::UnicodeSegmentation;

use std::ops::Range;

use super::{
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing, FigurePreparation,
    FigurePresentation, PREPARATION, PreparationCapability, PreparationMeasureError,
    PreparationMeasureErrorKind, PreparationMeasurement, PreparationScene, PreparedScene,
};
use crate::geometry::{Point, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};
use crate::render::{
    CaretGeometry, FontDescriptor, SelectionQuad, TextAffinity, TextConstraints, TextError,
    TextInteractionError, TextLayout, TextLayoutRevision, TextMovement, TextPosition, TextRange,
};
use crate::{Alignment, Figure, FigureMeasurement, MeasureConstraints};
use crate::{geometry::Dimension, text::MeasureContext};
use std::sync::Arc;

const ELLIPSIS: &str = "...";

fn aligned(origin: f64, available: f64, used: f64, alignment: Alignment) -> f64 {
    match alignment {
        Alignment::Start => origin,
        Alignment::Center => origin + (available - used).max(0.0) / 2.0,
        Alignment::End => origin + (available - used).max(0.0),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InlineTextFragment {
    text: String,
}

impl InlineTextFragment {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlowParagraph {
    fragments: Vec<InlineTextFragment>,
}

impl FlowParagraph {
    pub fn new(fragments: impl Into<Vec<InlineTextFragment>>) -> Self {
        Self {
            fragments: fragments.into(),
        }
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self::new(vec![InlineTextFragment::new(text)])
    }

    pub fn fragments(&self) -> &[InlineTextFragment] {
        &self.fragments
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlowPage {
    paragraphs: Vec<FlowParagraph>,
}

impl FlowPage {
    pub fn new(paragraphs: impl Into<Vec<FlowParagraph>>) -> Self {
        Self {
            paragraphs: paragraphs.into(),
        }
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self::new(vec![FlowParagraph::from_text(text)])
    }

    pub fn paragraphs(&self) -> &[FlowParagraph] {
        &self.paragraphs
    }

    fn text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|paragraph| {
                paragraph
                    .fragments
                    .iter()
                    .map(InlineTextFragment::text)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn paragraph_ranges(&self) -> Vec<Range<usize>> {
        let mut offset = 0;
        self.paragraphs
            .iter()
            .enumerate()
            .map(|(index, paragraph)| {
                let length = paragraph
                    .fragments
                    .iter()
                    .map(|fragment| fragment.text.len())
                    .sum::<usize>();
                let range = offset..offset + length;
                offset = range.end + usize::from(index + 1 < self.paragraphs.len());
                range
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct FlowTextPosition {
    paragraph: usize,
    byte_offset: usize,
    affinity: TextAffinity,
    layout_revision: Option<TextLayoutRevision>,
}

impl FlowTextPosition {
    pub const fn new(paragraph: usize, byte_offset: usize, affinity: TextAffinity) -> Self {
        Self {
            paragraph,
            byte_offset,
            affinity,
            layout_revision: None,
        }
    }

    pub const fn paragraph(self) -> usize {
        self.paragraph
    }

    pub const fn byte_offset(self) -> usize {
        self.byte_offset
    }

    pub const fn affinity(self) -> TextAffinity {
        self.affinity
    }

    pub const fn layout_revision(self) -> Option<TextLayoutRevision> {
        self.layout_revision
    }

    fn with_layout_revision(self, layout_revision: Option<TextLayoutRevision>) -> Self {
        Self {
            layout_revision,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct FlowTextRange {
    anchor: FlowTextPosition,
    focus: FlowTextPosition,
}

impl FlowTextRange {
    pub const fn new(anchor: FlowTextPosition, focus: FlowTextPosition) -> Self {
        Self { anchor, focus }
    }

    pub const fn anchor(self) -> FlowTextPosition {
        self.anchor
    }

    pub const fn focus(self) -> FlowTextPosition {
        self.focus
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FlowWrapping {
    NoWrap,
    #[default]
    SoftWrap,
    Truncate {
        max_lines: usize,
    },
}

/// Paint-only viewport applied to a shaped TextFlow layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextFlowViewport {
    content_offset: Point,
    clip_to_bounds: bool,
}

impl TextFlowViewport {
    /// Leaves the shaped content unshifted and unclipped.
    pub const UNCLIPPED: Self = Self {
        content_offset: Point::ZERO,
        clip_to_bounds: false,
    };

    /// Clips shaped content to the Figure bounds after applying an offset.
    pub const fn clipped(content_offset: Point) -> Self {
        Self {
            content_offset,
            clip_to_bounds: true,
        }
    }

    /// Returns the node-local offset applied after layout alignment.
    pub const fn content_offset(self) -> Point {
        self.content_offset
    }

    /// Returns whether paint is clipped to the current Figure bounds.
    pub const fn clips_to_bounds(self) -> bool {
        self.clip_to_bounds
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlowLayoutKey {
    revision: u64,
    font: FontDescriptor,
    engine_revision: u64,
    bounds: Rectangle,
    wrapping: FlowWrapping,
    horizontal_alignment: Alignment,
    vertical_alignment: Alignment,
}

#[derive(Clone, Debug, PartialEq)]
struct FlowLayoutSnapshot {
    key: FlowLayoutKey,
    layout: TextLayout,
    paragraph_ranges: Vec<Range<usize>>,
    origin: Point,
}

#[derive(Clone)]
pub struct TextFlowFigure {
    bounds: Rectangle,
    page: FlowPage,
    revision: u64,
    wrapping: FlowWrapping,
    horizontal_alignment: Alignment,
    vertical_alignment: Alignment,
    viewport: TextFlowViewport,
    layout: Option<FlowLayoutSnapshot>,
}

impl TextFlowFigure {
    pub fn new(bounds: Rectangle, page: FlowPage) -> Result<Self, super::ShapeGeometryError> {
        Ok(Self {
            bounds: super::ShapeGeometryError::validate_bounds(bounds)?,
            page,
            revision: 1,
            wrapping: FlowWrapping::SoftWrap,
            horizontal_alignment: Alignment::Start,
            vertical_alignment: Alignment::Start,
            viewport: TextFlowViewport::UNCLIPPED,
            layout: None,
        })
    }

    pub fn with_wrapping(mut self, wrapping: FlowWrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    pub fn with_alignment(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.horizontal_alignment = horizontal;
        self.vertical_alignment = vertical;
        self
    }

    pub fn with_viewport(mut self, viewport: TextFlowViewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn viewport(&self) -> TextFlowViewport {
        self.viewport
    }

    /// Available on the immutable prepared value stored in `FigurePresentation`.
    /// Attached source components keep no independent layout; use Runtime text queries.
    pub fn text_layout(&self) -> Option<&TextLayout> {
        self.layout.as_ref().map(|snapshot| &snapshot.layout)
    }

    pub fn hit_test_text(&self, point: Point) -> Result<FlowTextPosition, TextInteractionError> {
        let snapshot = self
            .layout
            .as_ref()
            .ok_or(TextInteractionError::Unavailable)?;
        let origin = self.visual_origin(snapshot);
        let local = Point::new(point.x() - origin.x(), point.y() - origin.y());
        snapshot.flow_position_for_text(snapshot.layout.hit_test_text(local)?)
    }

    pub fn caret_geometry(
        &self,
        position: FlowTextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        let snapshot = self
            .layout
            .as_ref()
            .ok_or(TextInteractionError::Unavailable)?;
        let geometry = snapshot
            .layout
            .caret_geometry(snapshot.to_text_position(position)?)?;
        let bounds = geometry.bounds();
        let origin = self.visual_origin(snapshot);
        Ok(CaretGeometry::new(
            Rectangle::new(
                bounds.x + origin.x(),
                bounds.y + origin.y(),
                bounds.width,
                bounds.height,
            ),
            geometry.line_index(),
        ))
    }

    pub fn selection_geometry(
        &self,
        range: FlowTextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        let snapshot = self
            .layout
            .as_ref()
            .ok_or(TextInteractionError::Unavailable)?;
        snapshot
            .layout
            .selection_geometry(TextRange::new(
                snapshot.to_text_position(range.anchor)?,
                snapshot.to_text_position(range.focus)?,
            ))?
            .into_iter()
            .map(|quad| {
                let bounds = quad.bounds();
                let origin = self.visual_origin(snapshot);
                Ok(SelectionQuad::new(
                    Rectangle::new(
                        bounds.x + origin.x(),
                        bounds.y + origin.y(),
                        bounds.width,
                        bounds.height,
                    ),
                    quad.line_index(),
                ))
            })
            .collect()
    }

    pub fn move_text_position(
        &self,
        position: FlowTextPosition,
        movement: TextMovement,
    ) -> Result<FlowTextPosition, TextInteractionError> {
        let snapshot = self
            .layout
            .as_ref()
            .ok_or(TextInteractionError::Unavailable)?;
        let moved = match movement {
            TextMovement::ParagraphStart => {
                FlowTextPosition::new(position.paragraph, 0, TextAffinity::Downstream)
            }
            TextMovement::ParagraphEnd => {
                let paragraph = snapshot
                    .paragraph_ranges
                    .get(position.paragraph)
                    .ok_or(TextInteractionError::InvalidParagraph)?;
                FlowTextPosition::new(position.paragraph, paragraph.len(), TextAffinity::Upstream)
            }
            _ => snapshot.flow_position_for_text(
                snapshot
                    .layout
                    .move_text_position(snapshot.to_text_position(position)?, movement)?,
            )?,
        };
        let revision = snapshot
            .layout
            .interaction_map()
            .ok_or(TextInteractionError::Unavailable)?
            .revision();
        let moved = moved.with_layout_revision(Some(revision));
        snapshot.to_text_position(moved)?;
        Ok(moved)
    }

    fn prepare_layout(
        &mut self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<bool, TextError> {
        let key = FlowLayoutKey {
            revision: self.revision,
            font: context.font().clone(),
            engine_revision: context.revision(),
            bounds,
            wrapping: self.wrapping,
            horizontal_alignment: self.horizontal_alignment,
            vertical_alignment: self.vertical_alignment,
        };
        if self.layout.as_ref().is_some_and(|layout| layout.key == key) {
            return Ok(false);
        }
        let layout = self.shape(context, Some(bounds.width.max(0.0)))?;
        let origin = Point::new(
            aligned(
                key.bounds.x,
                key.bounds.width,
                f64::from(layout.width()),
                key.horizontal_alignment,
            ),
            aligned(
                key.bounds.y,
                key.bounds.height,
                f64::from(layout.height()),
                key.vertical_alignment,
            ),
        );
        let next = FlowLayoutSnapshot {
            key,
            layout,
            paragraph_ranges: self.page.paragraph_ranges(),
            origin,
        };
        let changed = self.layout.as_ref() != Some(&next);
        self.layout = Some(next);
        Ok(changed)
    }

    fn visual_origin(&self, snapshot: &FlowLayoutSnapshot) -> Point {
        Point::new(
            snapshot.origin.x() + self.viewport.content_offset.x(),
            snapshot.origin.y() + self.viewport.content_offset.y(),
        )
    }

    fn shape(
        &self,
        context: &mut MeasureContext<'_>,
        width: Option<f64>,
    ) -> Result<TextLayout, TextError> {
        let text = self.page.text();
        let constraints = match self.wrapping {
            FlowWrapping::NoWrap => TextConstraints::UNBOUNDED,
            FlowWrapping::SoftWrap | FlowWrapping::Truncate { .. } => {
                TextConstraints::new(width.map(|width| width as f32))?
            }
        };
        let full = context.layout_text(&text, constraints)?;
        match self.wrapping {
            FlowWrapping::Truncate { max_lines } if full.line_metrics().len() > max_lines => {
                truncate_to_lines(context, &text, constraints, max_lines, full.full_width())
            }
            _ => Ok(full),
        }
    }

    fn charge_shaping(&self, scene: &PreparationScene<'_>) -> Result<(), TextError> {
        let entries = self
            .page
            .paragraphs
            .iter()
            .fold(0usize, |total, paragraph| {
                paragraph
                    .fragments
                    .iter()
                    .fold(total.saturating_add(1), |total, fragment| {
                        total.saturating_add(fragment.text.len())
                    })
            });
        // Truncation shapes logarithmically many prefixes and collects grapheme offsets.
        let passes = if matches!(self.wrapping, FlowWrapping::Truncate { .. }) {
            entries.max(1).ilog2() as usize + 3
        } else {
            3
        };
        scene
            .budget()
            .allocate("text flow shaping", entries.saturating_mul(passes) as u64)
            .map_err(|error| {
                TextError::PreparationDependency(Box::new(
                    super::PreparationDependencyError::Budget(error),
                ))
            })
    }

    pub fn set_viewport(&mut self, viewport: TextFlowViewport) {
        self.viewport = viewport;
    }

    pub(crate) fn page(&self) -> &FlowPage {
        &self.page
    }

    pub(crate) fn wrapping(&self) -> FlowWrapping {
        self.wrapping
    }

    pub(crate) fn replace_page(&mut self, page: FlowPage) {
        self.page = page;
        self.revision = self.revision.wrapping_add(1);
        self.layout = None;
    }

    pub(crate) fn replace_wrapping(&mut self, wrapping: FlowWrapping) {
        self.wrapping = wrapping;
        self.layout = None;
    }
}

impl FlowLayoutSnapshot {
    fn to_text_position(
        &self,
        position: FlowTextPosition,
    ) -> Result<TextPosition, TextInteractionError> {
        let range = self
            .paragraph_ranges
            .get(position.paragraph)
            .ok_or(TextInteractionError::InvalidParagraph)?;
        if position.byte_offset > range.len()
            || !self
                .layout
                .key()
                .text()
                .is_char_boundary(range.start + position.byte_offset)
        {
            return Err(TextInteractionError::InvalidTextPosition);
        }
        let position_in_layout =
            TextPosition::new(range.start + position.byte_offset, position.affinity);
        Ok(position
            .layout_revision
            .map_or(position_in_layout, |revision| {
                position_in_layout.with_layout_revision(revision)
            }))
    }

    fn flow_position_for_text(
        &self,
        position: TextPosition,
    ) -> Result<FlowTextPosition, TextInteractionError> {
        let offset = position.byte_offset();
        let Some((paragraph, range)) =
            self.paragraph_ranges
                .iter()
                .enumerate()
                .find(|(index, range)| {
                    offset >= range.start
                        && (offset < range.end
                            || (offset == range.end
                                && (*index + 1 == self.paragraph_ranges.len()
                                    || self
                                        .paragraph_ranges
                                        .get(*index + 1)
                                        .is_none_or(|next| offset < next.start))))
                })
        else {
            return Err(TextInteractionError::InvalidTextPosition);
        };
        Ok(
            FlowTextPosition::new(paragraph, offset - range.start, position.affinity())
                .with_layout_revision(position.layout_revision()),
        )
    }
}

impl super::Bounded for TextFlowFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), super::ShapeGeometryError> {
        self.bounds =
            super::ShapeGeometryError::validate_bounds(Rectangle::new(x, y, width, height))?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "TextFlowFigure"
    }
}

impl Figure for TextFlowFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "TextFlowFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }

    fn intrinsic_measurement(&self, _constraints: MeasureConstraints) -> FigureMeasurement {
        self.layout
            .as_ref()
            .map_or_else(FigureMeasurement::default, |snapshot| {
                FigureMeasurement::new(
                    f64::from(snapshot.layout.width()),
                    f64::from(snapshot.layout.height()),
                    Some(f64::from(snapshot.layout.baseline())),
                )
            })
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        if let Some(snapshot) = &self.layout {
            if self.viewport.clips_to_bounds() {
                context.clip_rect(bounds)?;
            }
            let origin = self.visual_origin(snapshot);
            if let Some(foreground) = context.stroke_paint().cloned() {
                context.push_state();
                context.set_fill_paint(foreground);
                let paint_result = context.fill_text(&snapshot.layout, origin);
                let pop_result = context.pop_state();
                paint_result.and(pop_result)?;
            }
        }
        Ok(())
    }

    fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
        let base = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        if self.viewport.clips_to_bounds() {
            return base;
        }
        self.layout
            .as_ref()
            .and_then(|snapshot| {
                snapshot.layout.ink_bounds().map(|ink| {
                    let origin = self.visual_origin(snapshot);
                    Rectangle::new(
                        ink.x + origin.x(),
                        ink.y + origin.y(),
                        ink.width,
                        ink.height,
                    )
                })
            })
            .map_or(base, |ink| base.union(ink))
    }
}

impl FigureDrawing for TextFlowFigure {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let bounds = self
            .layout
            .as_ref()
            .map_or(Rectangle::ZERO, |snapshot| snapshot.key.bounds);
        Figure::paint(self, context, bounds)
    }
}
impl FigurePreparation for TextFlowFigure {
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let failure = |kind| PreparationMeasureError {
            figure: scene.root(),
            outer_constraints: constraints,
            content_constraints: Some(constraints),
            kind,
        };
        self.charge_shaping(scene).map_err(|error| {
            failure(match error {
                TextError::PreparationDependency(error) => {
                    PreparationMeasureErrorKind::Dependency(error)
                }
                error => PreparationMeasureErrorKind::Text(Box::new(error)),
            })
        })?;
        let layout = self
            .shape(context, constraints.max_width())
            .map_err(|error| failure(PreparationMeasureErrorKind::Text(Box::new(error))))?;
        Ok(PreparationMeasurement {
            preferred: FigureMeasurement::new(
                f64::from(layout.width()),
                f64::from(layout.height()),
                Some(f64::from(layout.baseline())),
            ),
            minimum: Dimension::new(f64::from(layout.width()), f64::from(layout.height())),
        })
    }

    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        let mut prepared = self.clone();
        prepared.prepare_layout(context, bounds)?;
        let measurement = prepared.intrinsic_measurement(MeasureConstraints::UNBOUNDED);
        let visual = prepared.visual_bounds_in(bounds);
        let prepared = Arc::new(prepared);
        FigurePresentation::new(measurement, measurement.size(), visual, prepared.clone())
            .map(|presentation| presentation.with_data(prepared))
    }

    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        self.charge_shaping(scene)?;
        self.prepare(context, bounds).map(PreparedScene::new)
    }
}

fn truncate_to_lines(
    context: &mut MeasureContext<'_>,
    source: &str,
    constraints: TextConstraints,
    max_lines: usize,
    full_width: f32,
) -> Result<TextLayout, TextError> {
    if max_lines == 0 {
        return context.layout_text("", constraints)?.with_visibility(
            source,
            0..0,
            true,
            full_width,
            constraints,
        );
    }
    let graphemes = source.grapheme_indices(true).collect::<Vec<_>>();
    let mut low = 0;
    let mut high = graphemes.len();
    while low < high {
        let middle = (low + high).div_ceil(2);
        let byte_end = graphemes
            .get(middle)
            .map_or(source.len(), |(index, _)| *index);
        let candidate = format!("{}{}", &source[..byte_end], ELLIPSIS);
        if context
            .layout_text(&candidate, constraints)?
            .line_metrics()
            .len()
            <= max_lines
        {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let visible_end = graphemes.get(low).map_or(source.len(), |(index, _)| *index);
    context
        .layout_text(
            &format!("{}{}", &source[..visible_end], ELLIPSIS),
            constraints,
        )?
        .with_visibility(source, 0..visible_end, true, full_width, constraints)
}
