use unicode_segmentation::UnicodeSegmentation;

use crate::geometry::Rectangle;
use crate::render::{
    FontDescriptor, NdCanvas, TextConstraints, TextError, TextLayout, TextLayoutEngine,
};
use crate::{Figure, FigureMeasurement, FigureStyle, MeasureConstraints};

const ELLIPSIS: &str = "...";

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
    width: f64,
    wrapping: FlowWrapping,
}

#[derive(Clone, Debug, PartialEq)]
struct FlowLayoutSnapshot {
    key: FlowLayoutKey,
    layout: TextLayout,
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
    layout: Option<FlowLayoutSnapshot>,
}

impl TextFlowFigure {
    pub fn new(bounds: Rectangle, page: FlowPage) -> Self {
        Self {
            bounds,
            page,
            revision: 1,
            wrapping: FlowWrapping::SoftWrap,
            layout: None,
        }
    }

    pub fn with_wrapping(mut self, wrapping: FlowWrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    pub fn text_layout(&self) -> Option<&TextLayout> {
        self.layout.as_ref().map(|snapshot| &snapshot.layout)
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
            width: bounds.width.max(0.0),
            wrapping: self.wrapping,
        };
        if self.layout.as_ref().is_some_and(|layout| layout.key == key) {
            return Ok(false);
        }
        let text = self.page.text();
        let constraints = match self.wrapping {
            FlowWrapping::NoWrap => TextConstraints::UNBOUNDED,
            FlowWrapping::SoftWrap | FlowWrapping::Truncate { .. } => {
                TextConstraints::new(Some(key.width as f32))?
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
        let next = FlowLayoutSnapshot { key, layout };
        let changed = self.layout.as_ref() != Some(&next);
        self.layout = Some(next);
        Ok(changed)
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

    fn initial_style(&self) -> FigureStyle {
        FigureStyle::default()
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
            gc.draw_text_layout(&snapshot.layout, 0.0, 0.0);
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
