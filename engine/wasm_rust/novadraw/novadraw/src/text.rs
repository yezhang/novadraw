//! Text preparation, metrics and immutable layouts. No GPU is required.
#![allow(missing_docs)]

pub use crate::render::text::{
    BuiltinFont, CaretGeometry, FontDescriptor, FontFaceRef, FontStyle, SelectionQuad,
    TextAffinity, TextConstraints, TextError, TextInteractionError, TextLayout, TextLineMetrics,
    TextMovement, TextPosition, TextRange,
};

pub mod shaping {
    pub use crate::render::text::{
        GlyphRun, ParleyTextEngine, PositionedGlyph, TextInteractionMap, TextInteractionProvider,
        TextLayoutCacheIdentity, TextLayoutEngine, TextLayoutKey, TextLayoutParts,
        TextLayoutRevision,
    };
}

mod line_padding;
pub(crate) mod mirror;
pub mod outline;
mod styled;
pub use styled::{
    ParagraphAlignment, ParagraphDirection, StyledGlyphRun, StyledTextLayout, StyledTextRequest,
    TextInlineBox, TextInlinePlacement, TextStyle, TextStyleSpan,
};
mod system;
pub use system::{MeasureContext, TextSystem};
mod underline;
pub use underline::{PreparedTextUnderlines, TextUnderline, TextUnderlineFragment};

/// Font-provided decoration geometry in Canvas logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontDecorationMetrics {
    offset: f32,
    thickness: f32,
}

impl FontDecorationMetrics {
    /// Offset from the baseline in the downward-positive Canvas coordinate space.
    pub fn new(offset: f32, thickness: f32) -> Result<Self, TextError> {
        if !offset.is_finite() || !thickness.is_finite() || thickness <= 0.0 {
            return Err(TextError::InvalidMetric);
        }
        Ok(Self { offset, thickness })
    }
    pub fn offset(&self) -> f32 {
        self.offset
    }
    pub fn thickness(&self) -> f32 {
        self.thickness
    }
}

/// Metrics of one resolved font instance in logical units.
#[derive(Clone, Debug, PartialEq)]
pub struct FontMetrics {
    face: FontFaceRef,
    size: f32,
    normalized_coords: Vec<i16>,
    ascent: f32,
    descent: f32,
    leading: f32,
    underline: Option<FontDecorationMetrics>,
}

impl FontMetrics {
    pub fn new(
        face: FontFaceRef,
        size: f32,
        normalized_coords: Vec<i16>,
        ascent: f32,
        descent: f32,
        leading: f32,
    ) -> Result<Self, TextError> {
        if !size.is_finite()
            || size <= 0.0
            || !leading.is_finite()
            || [ascent, descent].iter().any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(TextError::InvalidMetric);
        }
        Ok(Self {
            face,
            size,
            normalized_coords,
            ascent,
            descent,
            leading,
            underline: None,
        })
    }

    pub fn face(&self) -> &FontFaceRef {
        &self.face
    }
    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }
    pub fn ascent(&self) -> f32 {
        self.ascent
    }
    pub fn descent(&self) -> f32 {
        self.descent
    }
    pub fn leading(&self) -> f32 {
        self.leading
    }
    /// Attaches font-provided metrics; external engines use the same value contract.
    pub fn with_underline(mut self, underline: FontDecorationMetrics) -> Self {
        self.underline = Some(underline);
        self
    }
    /// Missing font data or an external engine without decoration support fails explicitly.
    pub fn underline(&self) -> Result<FontDecorationMetrics, TextError> {
        self.underline.ok_or(TextError::FontMetricsUnsupported)
    }
}

/// Layout measurements, distinct from glyph ink and painted bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct TextMetrics {
    size: crate::Dimension,
    full_width: f32,
    baseline: f32,
    lines: Vec<TextLineMetrics>,
    truncated: bool,
}

impl TextMetrics {
    pub(crate) fn from_layout(layout: &TextLayout) -> Self {
        Self {
            size: layout.size(),
            full_width: layout.full_width(),
            baseline: layout.baseline(),
            lines: layout.line_metrics().to_vec(),
            truncated: layout.is_truncated(),
        }
    }
    pub fn size(&self) -> crate::Dimension {
        self.size
    }
    pub fn full_width(&self) -> f32 {
        self.full_width
    }
    pub fn baseline(&self) -> f32 {
        self.baseline
    }
    pub fn lines(&self) -> &[TextLineMetrics] {
        &self.lines
    }
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }
}
