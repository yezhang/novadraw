use crate::{DocumentVersion, ParagraphHeightIndex, ParagraphId, RichTextDocumentSnapshot};
use novadraw::{
    figure::{FigureDrawing, PreparationBudgetHandle},
    geometry::{Dimension, Point, Rectangle},
    graphics::{GraphicsError, PaintContext},
    text::{
        MeasureContext, StyledTextLayout, StyledTextRequest, TextConstraints, TextError,
        TextLayoutDiagnostics, TextPosition, shaping::TextStageContext,
    },
};
use std::sync::Arc;
use uuid::Uuid;

/// Positions are tied to an immutable document AND layout, including width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextPosition {
    pub document: DocumentVersion,
    pub layout: Uuid,
    pub paragraph: ParagraphId,
    pub text: TextPosition,
}

pub struct RichTextLayoutSnapshot {
    document: RichTextDocumentSnapshot,
    identity: Uuid,
    layouts: Vec<StyledTextLayout>,
    heights: ParagraphHeightIndex,
    size: Dimension,
    visual: Rectangle,
}

impl RichTextLayoutSnapshot {
    pub fn document_version(&self) -> DocumentVersion {
        self.document.version()
    }
    pub fn document(&self) -> &RichTextDocumentSnapshot {
        &self.document
    }
    pub fn height_index(&self) -> &ParagraphHeightIndex {
        &self.heights
    }
    pub fn size(&self) -> Dimension {
        self.size
    }
    pub fn paragraph_count(&self) -> usize {
        self.layouts.len()
    }
    pub fn hit_test(&self, point: Point) -> Result<Option<RichTextPosition>, TextError> {
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(TextError::InvalidMetric);
        }
        let index = self
            .heights
            .intersecting(point.y()..point.y() + f64::EPSILON.max(point.y().abs() * f64::EPSILON))
            .start;
        let Some(layout) = self.layouts.get(index) else {
            return Ok(None);
        };
        let y = self.heights.offset(index).ok_or(TextError::InvalidMetric)?;
        let text = layout
            .layout()
            .hit_test_text(Point::new(point.x(), point.y() - y))
            .map_err(|e| TextError::InvalidPreparedScene(e.to_string()))?;
        Ok(Some(RichTextPosition {
            document: self.document.version(),
            layout: self.identity,
            paragraph: self.document.paragraphs()[index].id(),
            text,
        }))
    }
    pub fn contains_position(&self, position: &RichTextPosition) -> bool {
        position.document == self.document.version()
            && position.layout == self.identity
            && self
                .document
                .paragraphs()
                .iter()
                .enumerate()
                .any(|(index, p)| {
                    p.id() == position.paragraph
                        && self.layouts[index]
                            .layout()
                            .caret_geometry(position.text)
                            .is_ok()
                })
    }
    pub(crate) fn prepare(
        document: &RichTextDocumentSnapshot,
        context: &mut MeasureContext<'_>,
        width: Option<f64>,
        budget: PreparationBudgetHandle,
        direction: novadraw::figure::LayoutDirection,
    ) -> Result<Arc<Self>, TextError> {
        let diagnostics = TextLayoutDiagnostics::default();
        let _timing = diagnostics.time("richtext.snapshot");
        let width = width.map(|w| w as f32);
        let constraints = TextConstraints::new(width)?;
        budget.consume("richtext paragraphs", document.paragraphs().len() as u64)?;
        budget.allocate("richtext snapshot", (document.paragraphs().len() * (
            std::mem::size_of::<StyledTextLayout>() + std::mem::size_of::<f64>() +
            // Conservative metadata charge for height-summary tree nodes.
            2 * (std::mem::size_of::<Arc<()>>() * 2 + std::mem::size_of::<f64>() + std::mem::size_of::<usize>())
        )) as u64)?;
        let session = TextStageContext::new(budget.clone(), TextLayoutDiagnostics::default());
        let mut layouts = Vec::with_capacity(document.paragraphs().len());
        let mut heights = Vec::with_capacity(document.paragraphs().len());
        let mut size = Dimension::ZERO;
        let mut visual = Rectangle::ZERO;
        for paragraph in document.paragraphs() {
            let _paragraph = diagnostics.time("richtext.paragraph");
            budget.allocate(
                "richtext input",
                (paragraph.len_bytes() + std::mem::size_of_val(paragraph.spans())) as u64,
            )?;
            let request = StyledTextRequest::new(
                paragraph.text(),
                paragraph.style().clone(),
                paragraph.spans().to_vec(),
                Vec::new(),
                constraints,
            )?
            .with_paragraph(paragraph.direction, paragraph.alignment, true)
            .with_layout_direction(direction);
            let layout = context.layout_styled_in_session(&request, &session)?;
            let measured = layout.layout().size();
            if let Some(ink) = layout.layout().ink_bounds() {
                let ink = Rectangle::new(ink.x, ink.y + size.height, ink.width, ink.height);
                visual = visual.union(ink);
            }
            size.width = size.width.max(measured.width);
            let height = measured.height + paragraph.space_after;
            size.height += height;
            heights.push(height);
            layouts.push(layout);
        }
        let heights = ParagraphHeightIndex::new(&heights)?;
        if !size.width.is_finite() || !size.height.is_finite() {
            return Err(TextError::InvalidMetric);
        }
        visual = visual.union(Rectangle::new(0.0, 0.0, size.width, size.height));
        budget.check()?;
        Ok(Arc::new(Self {
            document: document.clone(),
            identity: Uuid::new_v4(),
            layouts,
            heights,
            size,
            visual,
        }))
    }
    pub(crate) fn visual_bounds(&self) -> Rectangle {
        self.visual
    }
}

impl FigureDrawing for RichTextLayoutSnapshot {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        // Core clips the ordered command stream. Logical heights alone do not
        // prove native glyph/antialias coverage, so this initial slice submits
        // all paragraphs. Coverage-aware output is deliberately deferred.
        let mut y = 0.0;
        for (layout, paragraph) in self.layouts.iter().zip(self.document.paragraphs()) {
            context.fill_styled_text(layout, Point::new(0.0, y))?;
            y += layout.layout().size().height + paragraph.space_after;
        }
        Ok(())
    }
}
