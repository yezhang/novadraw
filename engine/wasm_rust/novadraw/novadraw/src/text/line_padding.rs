//! Line geometry changes preserve the engine's shaping and interaction map.
use super::shaping::{TextInteractionMap, TextInteractionProvider, TextLayoutParts};
use super::{
    CaretGeometry, SelectionQuad, StyledTextLayout, TextError, TextInteractionError, TextLayout,
    TextLineMetrics, TextMovement, TextPosition, TextRange,
};
use crate::{Point, Rectangle};
use std::sync::Arc;

struct PaddedInteraction {
    original: TextInteractionMap,
    offsets: Vec<f64>,
    lines: Vec<TextLineMetrics>,
}
fn unbound(position: TextPosition) -> TextPosition {
    TextPosition::new(position.byte_offset(), position.affinity())
}
fn shifted(bounds: Rectangle, offset: f64) -> Rectangle {
    Rectangle::new(bounds.x, bounds.y + offset, bounds.width, bounds.height)
}
impl TextInteractionProvider for PaddedInteraction {
    fn text_len(&self) -> usize {
        self.original.source().len()
    }
    fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        let line = self
            .lines
            .iter()
            .position(|line| point.y() < f64::from(line.baseline + line.descent + line.leading))
            .unwrap_or(self.lines.len().saturating_sub(1));
        self.original.hit_test(Point::new(
            point.x(),
            point.y() - self.offsets.get(line).copied().unwrap_or(0.0),
        ))
    }
    fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        let caret = self.original.caret_geometry(unbound(position))?;
        Ok(CaretGeometry::new(
            shifted(caret.bounds(), self.offsets[caret.line_index()]),
            caret.line_index(),
        ))
    }
    fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        Ok(self
            .original
            .selection_geometry(TextRange::new(
                unbound(range.anchor()),
                unbound(range.focus()),
            ))?
            .into_iter()
            .map(|quad| {
                SelectionQuad::new(
                    shifted(quad.bounds(), self.offsets[quad.line_index()]),
                    quad.line_index(),
                )
            })
            .collect())
    }
    fn move_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        self.original.move_position(unbound(position), movement)
    }
}

impl StyledTextLayout {
    pub(crate) fn with_line_padding(self, padding: &[(f64, f64)]) -> Result<Self, TextError> {
        let original = self.layout();
        if padding.len() != original.line_metrics().len()
            || padding.iter().any(|(top, bottom)| {
                !top.is_finite() || !bottom.is_finite() || *top < 0.0 || *bottom < 0.0
            })
        {
            return Err(TextError::InvalidMetric);
        }
        if padding.iter().all(|pair| *pair == (0.0, 0.0)) {
            return Ok(self);
        }
        let mut lines = original.line_metrics().to_vec();
        let mut offsets = Vec::new();
        let mut extra = 0.0;
        for (line, (top, bottom)) in lines.iter_mut().zip(padding) {
            offsets.push(extra + top);
            line.baseline += (extra + top) as f32;
            line.ascent += *top as f32;
            line.descent += *bottom as f32;
            extra += top + bottom;
        }
        let mut glyph_runs = original.glyph_runs().to_vec();
        let mut glyph_offsets = Vec::new();
        for (run, styled) in glyph_runs.iter_mut().zip(self.runs()) {
            let offset = offsets[styled.line_index];
            for glyph in &mut run.glyphs {
                glyph.y += offset as f32;
                glyph_offsets.push(offset);
            }
        }
        let mut layout = TextLayout::from_parts(TextLayoutParts {
            text: original.key().text().to_owned(),
            font: original.key().font().clone(),
            constraints: original.key().constraints(),
            engine_revision: original.key().engine_revision(),
            width: original.width(),
            full_width: original.full_width(),
            height: (f64::from(original.height()) + extra) as f32,
            lines: lines.clone(),
            glyph_runs,
            visible_range: original.visible_range(),
            truncated: original.is_truncated(),
        })?;
        if let Some(map) = original.interaction_map() {
            layout = layout.with_interaction_map(
                TextInteractionMap::new(
                    original.key().text(),
                    Arc::new(PaddedInteraction {
                        original: map.clone(),
                        offsets: offsets.clone(),
                        lines,
                    }),
                )
                .map_err(|_| TextError::InvalidInteractionMap)?,
            )?;
        }
        if let Some(outlines) = original.outlines() {
            layout.outlines = Some(Arc::new(
                outlines
                    .with_vertical_offsets(&glyph_offsets)
                    .map_err(|_| TextError::InvalidGlyphRun)?,
            ));
        }
        let placements = self
            .inline_placements()
            .iter()
            .cloned()
            .map(|mut placement| {
                placement.bounds.y += offsets[placement.line_index];
                placement
            })
            .collect();
        let mut result = Self::new(
            self.request().clone(),
            layout,
            self.runs().to_vec(),
            placements,
        )?;
        // Paint runs have their own outline snapshots in an outline engine.
        for ((target, original), styled) in result
            .paint_layouts_mut()
            .iter_mut()
            .zip(self.paint_runs().map(|(layout, _)| layout))
            .zip(self.runs())
        {
            if let Some(outlines) = original.outlines() {
                target.outlines = Some(Arc::new(
                    outlines
                        .with_vertical_offsets(&vec![
                            offsets[styled.line_index];
                            outlines.instances().len()
                        ])
                        .map_err(|_| TextError::InvalidGlyphRun)?,
                ));
            }
        }
        Ok(result)
    }
}
