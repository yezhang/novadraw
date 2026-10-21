use std::hash::{Hash, Hasher};
use std::ops::Range;

use super::{FontDescriptor, TextConstraints, TextError, TextLayout};
use crate::{Rectangle, graphics::Color};

/// Paragraph base direction, independent of the direction of individual runs.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ParagraphDirection {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

/// Logical alignment follows the paragraph base direction.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ParagraphAlignment {
    #[default]
    Leading,
    Trailing,
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    font: FontDescriptor,
    color: Color,
}

impl TextStyle {
    pub fn new(font: FontDescriptor, color: Color) -> Self {
        Self { font, color }
    }
    pub fn font(&self) -> &FontDescriptor {
        &self.font
    }
    pub fn color(&self) -> Color {
        self.color
    }
    fn hash_into(&self, hash: &mut impl Hasher) {
        self.font.family().hash(hash);
        self.font.size().to_bits().hash(hash);
        self.font.weight().to_bits().hash(hash);
        std::mem::discriminant(&self.font.style()).hash(hash);
        for component in self.color.to_rgba_array() {
            component.to_bits().hash(hash);
        }
    }
}

/// Resolved, non-overlapping UTF-8 style span. Gaps use the default style.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleSpan {
    pub range: Range<usize>,
    pub style: TextStyle,
}

/// Identity and dimensions of a real Figure's measured inline box.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInlineBox {
    pub id: u64,
    pub byte_offset: usize,
    pub width: f32,
    pub height: f32,
}

/// Checked input shared by the default and external shaping engines.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledTextRequest {
    text: String,
    default_style: TextStyle,
    spans: Vec<TextStyleSpan>,
    boxes: Vec<TextInlineBox>,
    constraints: TextConstraints,
    direction: ParagraphDirection,
    alignment: ParagraphAlignment,
    wrap: bool,
}

impl StyledTextRequest {
    pub fn new(
        text: impl Into<String>,
        default_style: TextStyle,
        spans: Vec<TextStyleSpan>,
        boxes: Vec<TextInlineBox>,
        constraints: TextConstraints,
    ) -> Result<Self, TextError> {
        let text = text.into();
        TextConstraints::new(constraints.max_width)?;
        let mut end = 0;
        for span in &spans {
            if span.range.start < end
                || span.range.start >= span.range.end
                || span.range.end > text.len()
                || !text.is_char_boundary(span.range.start)
                || !text.is_char_boundary(span.range.end)
            {
                return Err(TextError::InvalidStyleRange);
            }
            end = span.range.end;
        }
        let mut ids = std::collections::HashSet::new();
        let mut offset = 0;
        for inline in &boxes {
            if inline.byte_offset < offset
                || inline.byte_offset > text.len()
                || !text.is_char_boundary(inline.byte_offset)
                || !ids.insert(inline.id)
            {
                return Err(TextError::InvalidInlineBox);
            }
            if [inline.width, inline.height]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
            {
                return Err(TextError::InvalidMetric);
            }
            offset = inline.byte_offset;
        }
        Ok(Self {
            text,
            default_style,
            spans,
            boxes,
            constraints,
            direction: ParagraphDirection::Auto,
            alignment: ParagraphAlignment::Leading,
            wrap: true,
        })
    }
    pub fn with_paragraph(
        mut self,
        direction: ParagraphDirection,
        alignment: ParagraphAlignment,
        wrap: bool,
    ) -> Self {
        self.direction = direction;
        self.alignment = alignment;
        self.wrap = wrap;
        self
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn default_style(&self) -> &TextStyle {
        &self.default_style
    }
    pub fn spans(&self) -> &[TextStyleSpan] {
        &self.spans
    }
    pub fn inline_boxes(&self) -> &[TextInlineBox] {
        &self.boxes
    }
    pub fn constraints(&self) -> TextConstraints {
        self.constraints
    }
    pub fn direction(&self) -> ParagraphDirection {
        self.direction
    }
    pub fn alignment(&self) -> ParagraphAlignment {
        self.alignment
    }
    pub fn wraps(&self) -> bool {
        self.wrap
    }
    pub fn style(&self, index: usize) -> Option<&TextStyle> {
        if index == 0 {
            Some(&self.default_style)
        } else {
            self.spans.get(index - 1).map(|span| &span.style)
        }
    }
    pub(crate) fn fingerprint(&self) -> u64 {
        // Private revision identity; never used as a portable serialization format.
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut hash);
        self.default_style.hash_into(&mut hash);
        self.spans.len().hash(&mut hash);
        for span in &self.spans {
            span.range.hash(&mut hash);
            span.style.hash_into(&mut hash);
        }
        self.boxes.len().hash(&mut hash);
        for inline in &self.boxes {
            inline.id.hash(&mut hash);
            inline.byte_offset.hash(&mut hash);
            inline.width.to_bits().hash(&mut hash);
            inline.height.to_bits().hash(&mut hash);
        }
        self.constraints.max_width.map(f32::to_bits).hash(&mut hash);
        self.direction.hash(&mut hash);
        self.alignment.hash(&mut hash);
        self.wrap.hash(&mut hash);
        hash.finish()
    }
}

/// One positioned glyph run's source and resolved paint style.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledGlyphRun {
    pub source_range: Range<usize>,
    pub style_index: usize,
    pub line_index: usize,
    pub right_to_left: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInlinePlacement {
    pub id: u64,
    pub bounds: Rectangle,
    pub line_index: usize,
}

/// Immutable checked result. Paint and interaction share the layout revision.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledTextLayout {
    request: StyledTextRequest,
    layout: TextLayout,
    runs: Vec<StyledGlyphRun>,
    placements: Vec<TextInlinePlacement>,
    paint_layouts: Vec<TextLayout>,
}

impl StyledTextLayout {
    pub fn new(
        request: StyledTextRequest,
        mut layout: TextLayout,
        runs: Vec<StyledGlyphRun>,
        placements: Vec<TextInlinePlacement>,
    ) -> Result<Self, TextError> {
        if layout.key().text() != request.text()
            || layout.key().font() != request.default_style.font()
            || layout.key().constraints() != request.constraints()
            || runs.len() != layout.glyph_runs().len()
        {
            return Err(TextError::InvalidStyledLayout);
        }
        for (run, glyphs) in runs.iter().zip(layout.glyph_runs()) {
            if run.source_range.start >= run.source_range.end
                || run.source_range.end > request.text.len()
                || !request.text.is_char_boundary(run.source_range.start)
                || !request.text.is_char_boundary(run.source_range.end)
                || request.style(run.style_index).is_none()
                || if run.style_index == 0 {
                    request.spans.iter().any(|span| {
                        run.source_range.start < span.range.end
                            && span.range.start < run.source_range.end
                    })
                } else {
                    request.spans.get(run.style_index - 1).is_none_or(|span| {
                        run.source_range.start < span.range.start
                            || run.source_range.end > span.range.end
                    })
                }
                || run.line_index >= layout.line_metrics().len()
                || request
                    .style(run.style_index)
                    .is_some_and(|style| style.font().size() != glyphs.font_size)
            {
                return Err(TextError::InvalidStyledLayout);
            }
        }
        let mut ids = std::collections::HashSet::new();
        if placements.len() != request.boxes.len() {
            return Err(TextError::InvalidStyledLayout);
        }
        for placement in &placements {
            let Some(input) = request.boxes.iter().find(|b| b.id == placement.id) else {
                return Err(TextError::InvalidStyledLayout);
            };
            let b = placement.bounds;
            if !ids.insert(placement.id)
                || placement.line_index >= layout.line_metrics().len()
                || [b.x, b.y, b.width, b.height].iter().any(|v| !v.is_finite())
                || b.width != f64::from(input.width)
                || b.height != f64::from(input.height)
                || b.y < 0.0
                || b.y + b.height > f64::from(layout.height()) + f64::from(f32::EPSILON)
                || (b.y + b.height
                    - f64::from(layout.line_metrics()[placement.line_index].baseline))
                .abs()
                    > f64::from(f32::EPSILON) * f64::from(layout.height()).max(1.0)
            {
                return Err(TextError::InvalidStyledLayout);
            }
        }
        layout.bind_style_revision(request.fingerprint());
        let paint_layouts = (0..runs.len())
            .map(|index| layout.glyph_run_layout(index))
            .collect();
        Ok(Self {
            request,
            layout,
            runs,
            placements,
            paint_layouts,
        })
    }
    pub fn request(&self) -> &StyledTextRequest {
        &self.request
    }
    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }
    pub fn runs(&self) -> &[StyledGlyphRun] {
        &self.runs
    }
    pub fn inline_placements(&self) -> &[TextInlinePlacement] {
        &self.placements
    }
    pub(crate) fn layout_mut(&mut self) -> &mut TextLayout {
        &mut self.layout
    }
    pub(crate) fn into_layout(self) -> TextLayout {
        self.layout
    }
    pub(crate) fn paint_layouts_mut(&mut self) -> &mut [TextLayout] {
        &mut self.paint_layouts
    }
    pub(crate) fn paint_runs(&self) -> impl Iterator<Item = (&TextLayout, Color)> {
        self.paint_layouts
            .iter()
            .zip(&self.runs)
            .map(|(layout, run)| {
                (
                    layout,
                    self.request
                        .style(run.style_index)
                        .expect("checked style")
                        .color(),
                )
            })
    }
}
