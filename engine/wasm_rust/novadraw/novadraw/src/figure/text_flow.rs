use unicode_segmentation::UnicodeSegmentation;

use std::ops::Range;

use crate::geometry::{Point, Rectangle};
use crate::render::{
    CaretGeometry, FontDescriptor, NdCanvas, SelectionQuad, TextAffinity, TextConstraints,
    TextError, TextInteractionError, TextLayout, TextLayoutEngine, TextLayoutRevision,
    TextMovement, TextPosition, TextRange,
};
use crate::{Alignment, Figure, FigureMeasurement, MeasureConstraints};

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

pub trait TextFlowBehavior {
    fn page(&self) -> &FlowPage;
    fn wrapping(&self) -> FlowWrapping;
    fn replace_page(&mut self, page: FlowPage);
    fn replace_wrapping(&mut self, wrapping: FlowWrapping);
}

#[derive(Clone)]
pub struct TextFlowFigure {
    bounds: Rectangle,
    page: FlowPage,
    revision: u64,
    wrapping: FlowWrapping,
    horizontal_alignment: Alignment,
    vertical_alignment: Alignment,
    layout: Option<FlowLayoutSnapshot>,
}

impl TextFlowFigure {
    pub fn new(bounds: Rectangle, page: FlowPage) -> Self {
        Self {
            bounds,
            page,
            revision: 1,
            wrapping: FlowWrapping::SoftWrap,
            horizontal_alignment: Alignment::Start,
            vertical_alignment: Alignment::Start,
            layout: None,
        }
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

    pub fn text_layout(&self) -> Option<&TextLayout> {
        self.layout.as_ref().map(|snapshot| &snapshot.layout)
    }

    pub fn hit_test_text(&self, point: Point) -> Result<FlowTextPosition, TextInteractionError> {
        let snapshot = self
            .layout
            .as_ref()
            .ok_or(TextInteractionError::Unavailable)?;
        let local = Point::new(
            point.x() - snapshot.origin.x(),
            point.y() - snapshot.origin.y(),
        );
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
        Ok(CaretGeometry::new(
            Rectangle::new(
                bounds.x + snapshot.origin.x(),
                bounds.y + snapshot.origin.y(),
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
                Ok(SelectionQuad::new(
                    Rectangle::new(
                        bounds.x + snapshot.origin.x(),
                        bounds.y + snapshot.origin.y(),
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

    pub(crate) fn refresh_layout(
        &mut self,
        engine: &mut dyn TextLayoutEngine,
        font: &FontDescriptor,
        bounds: Rectangle,
    ) -> Result<bool, TextError> {
        let key = FlowLayoutKey {
            revision: self.revision,
            font: font.clone(),
            engine_revision: engine.revision(),
            bounds,
            wrapping: self.wrapping,
            horizontal_alignment: self.horizontal_alignment,
            vertical_alignment: self.vertical_alignment,
        };
        if self.layout.as_ref().is_some_and(|layout| layout.key == key) {
            return Ok(false);
        }
        let text = self.page.text();
        let constraints = match self.wrapping {
            FlowWrapping::NoWrap => TextConstraints::UNBOUNDED,
            FlowWrapping::SoftWrap | FlowWrapping::Truncate { .. } => {
                TextConstraints::new(Some(key.bounds.width.max(0.0) as f32))?
            }
        };
        let full = engine.layout(&text, font, constraints)?;
        let layout = match self.wrapping {
            FlowWrapping::Truncate { max_lines } if full.line_metrics().len() > max_lines => {
                truncate_to_lines(
                    engine,
                    &text,
                    font,
                    constraints,
                    max_lines,
                    full.full_width(),
                )?
            }
            _ => full,
        };
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

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
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

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, _bounds: Rectangle) {
        if let Some(snapshot) = &self.layout {
            gc.draw_text_layout(&snapshot.layout, snapshot.origin.x(), snapshot.origin.y());
        }
    }

    fn text_flow(&self) -> Option<&dyn TextFlowBehavior> {
        Some(self)
    }

    fn text_flow_mut(&mut self) -> Option<&mut dyn TextFlowBehavior> {
        Some(self)
    }
}

impl TextFlowBehavior for TextFlowFigure {
    fn page(&self) -> &FlowPage {
        &self.page
    }

    fn wrapping(&self) -> FlowWrapping {
        self.wrapping
    }

    fn replace_page(&mut self, page: FlowPage) {
        self.page = page;
        self.revision = self.revision.wrapping_add(1);
        self.layout = None;
    }

    fn replace_wrapping(&mut self, wrapping: FlowWrapping) {
        self.wrapping = wrapping;
        self.layout = None;
    }
}

fn truncate_to_lines(
    engine: &mut dyn TextLayoutEngine,
    source: &str,
    font: &FontDescriptor,
    constraints: TextConstraints,
    max_lines: usize,
    full_width: f32,
) -> Result<TextLayout, TextError> {
    if max_lines == 0 {
        return engine.layout("", font, constraints)?.with_visibility(
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
        if engine
            .layout(&candidate, font, constraints)?
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
    engine
        .layout(
            &format!("{}{}", &source[..visible_end], ELLIPSIS),
            font,
            constraints,
        )?
        .with_visibility(source, 0..visible_end, true, full_width, constraints)
}
