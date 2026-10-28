use std::ops::Range;

use super::{
    FontMetrics, StyledTextLayout, TextAffinity, TextError, TextPosition, TextRange,
    shaping::{GlyphRun, TextLayoutEngine, TextLayoutRevision},
};
use crate::{geometry::Rectangle, graphics::Color};

/// Logical UTF-8 range and paint for one underline.
#[derive(Clone, Debug, PartialEq)]
pub struct TextUnderline {
    pub range: Range<usize>,
    pub color: Color,
}

/// Immutable geometry from one actual font run on one line.
#[derive(Clone, Debug, PartialEq)]
pub struct TextUnderlineFragment {
    source_range: Range<usize>,
    line_index: usize,
    bounds: Rectangle,
    color: Color,
    metrics: FontMetrics,
}

impl TextUnderlineFragment {
    pub fn source_range(&self) -> Range<usize> {
        self.source_range.clone()
    }
    pub fn line_index(&self) -> usize {
        self.line_index
    }
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }
    pub fn color(&self) -> Color {
        self.color
    }
    pub fn font_metrics(&self) -> &FontMetrics {
        &self.metrics
    }
}

/// Owned snapshot; geometry and font metrics are never recomputed while painting.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextUnderlines {
    revision: TextLayoutRevision,
    fragments: Vec<TextUnderlineFragment>,
}

impl PreparedTextUnderlines {
    pub fn layout_revision(&self) -> TextLayoutRevision {
        self.revision
    }
    pub fn fragments(&self) -> &[TextUnderlineFragment] {
        &self.fragments
    }
    pub fn ink_bounds(&self) -> Option<Rectangle> {
        self.fragments
            .iter()
            .map(|fragment| fragment.bounds)
            .reduce(|a, b| a.union(b))
    }

    pub(super) fn prepare(
        engine: &mut dyn TextLayoutEngine,
        text: &StyledTextLayout,
        inputs: &[TextUnderline],
    ) -> Result<Self, TextError> {
        let layout = text.layout();
        let mut last_end = 0;
        // Validate every range before querying fonts or producing fragments.
        for input in inputs {
            if input.range.start < last_end || input.range.start >= input.range.end {
                return Err(TextError::InvalidStyleRange);
            }
            layout
                .caret_geometry(TextPosition::new(
                    input.range.start,
                    TextAffinity::Downstream,
                ))
                .and_then(|_| {
                    layout
                        .caret_geometry(TextPosition::new(input.range.end, TextAffinity::Upstream))
                })
                .map_err(|_| TextError::InvalidStyleRange)?;
            last_end = input.range.end;
        }
        let mut metrics_cache: Vec<(GlyphRun, FontMetrics)> = Vec::new();
        let mut fragments = Vec::new();
        for input in inputs {
            // Query once per logical range. Normalize overlapping visual intervals before
            // splitting by actual font runs, including mirrored bidi fragment boundaries.
            let quads = layout
                .selection_geometry(TextRange::new(
                    TextPosition::new(input.range.start, TextAffinity::Downstream),
                    TextPosition::new(input.range.end, TextAffinity::Upstream),
                ))
                .map_err(|_| TextError::InvalidInteractionMap)?;
            let mut intervals = vec![Vec::<(f64, f64)>::new(); layout.line_metrics().len()];
            for quad in quads {
                let bounds = quad.bounds();
                let Some(line) = intervals.get_mut(quad.line_index()) else {
                    return Err(TextError::InvalidStyledLayout);
                };
                let right = bounds.x + bounds.width;
                if !right.is_finite() {
                    return Err(TextError::InvalidMetric);
                }
                if bounds.width > 0.0 {
                    line.push((bounds.x, right));
                }
            }
            for line in &mut intervals {
                line.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut merged: Vec<(f64, f64)> = Vec::new();
                for (start, end) in line.drain(..) {
                    if let Some(previous) = merged.last_mut().filter(|previous| previous.1 >= start)
                    {
                        previous.1 = previous.1.max(end);
                    } else {
                        merged.push((start, end));
                    }
                }
                *line = merged;
            }
            for (run, glyphs) in text.runs().iter().zip(layout.glyph_runs()) {
                let range = input.range.start.max(run.source_range.start)
                    ..input.range.end.min(run.source_range.end);
                if range.is_empty() {
                    continue;
                }
                let metrics = if let Some((_, metrics)) =
                    metrics_cache.iter().find(|(cached, _)| {
                        cached.font == glyphs.font
                            && cached.font_size == glyphs.font_size
                            && cached.normalized_coords == glyphs.normalized_coords
                    }) {
                    metrics.clone()
                } else {
                    let metrics = engine.resolved_font_metrics(glyphs)?;
                    if metrics.face() != &glyphs.font
                        || metrics.size() != glyphs.font_size
                        || metrics.normalized_coords() != glyphs.normalized_coords.as_ref()
                    {
                        return Err(TextError::InvalidStyledLayout);
                    }
                    // Cache the font instance only, without duplicating all positioned glyphs.
                    let instance = GlyphRun {
                        font: glyphs.font.clone(),
                        font_size: glyphs.font_size,
                        normalized_coords: glyphs.normalized_coords.clone(),
                        skew_degrees: glyphs.skew_degrees,
                        glyphs: super::shaping::GlyphRange::new(Vec::new())?,
                    };
                    metrics_cache.push((instance, metrics.clone()));
                    metrics
                };
                let decoration = metrics.underline()?;
                for &(start, end) in &intervals[run.line_index] {
                    let left = start.max(run.bounds.x);
                    let right = end.min(run.bounds.x + run.bounds.width);
                    if right <= left {
                        continue;
                    }
                    let bounds = Rectangle::new(
                        left,
                        f64::from(run.baseline) + f64::from(decoration.offset()),
                        right - left,
                        f64::from(decoration.thickness()),
                    );
                    if !bounds.y.is_finite()
                        || !(bounds.x + bounds.width).is_finite()
                        || !(bounds.y + bounds.height).is_finite()
                    {
                        return Err(TextError::InvalidMetric);
                    }
                    fragments.push(TextUnderlineFragment {
                        source_range: range.clone(),
                        line_index: run.line_index,
                        bounds,
                        color: input.color,
                        metrics: metrics.clone(),
                    });
                }
            }
        }
        Ok(Self {
            revision: layout.revision(),
            fragments,
        })
    }
}
