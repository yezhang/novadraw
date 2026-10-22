use super::{FlowContributionRegistry, FlowError};
use crate::{
    FigureId,
    figure::PreparationScene,
    geometry::{Dimension, Insets},
    text::{ParagraphAlignment, ParagraphDirection, TextStyle, TextStyleSpan},
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::Arc,
};

/// Paragraph boundaries and logical alignment belong to blocks, never to inline spans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowBlockStyle {
    pub direction: ParagraphDirection,
    /// `None` inherits the nearest block's alignment; the root defaults to leading.
    pub alignment: Option<ParagraphAlignment>,
    pub wrap: bool,
    pub margin: Insets,
    pub padding: Insets,
}
impl Default for FlowBlockStyle {
    fn default() -> Self {
        Self {
            direction: ParagraphDirection::Auto,
            alignment: None,
            wrap: true,
            margin: Insets::ZERO,
            padding: Insets::ZERO,
        }
    }
}
impl FlowBlockStyle {
    pub fn validate(self) -> Result<Self, FlowError> {
        for value in [
            self.margin.top,
            self.margin.bottom,
            self.margin.left,
            self.margin.right,
            self.padding.top,
            self.padding.bottom,
            self.padding.left,
            self.padding.right,
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(FlowError::Text(crate::text::TextError::InvalidMetric));
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct FlowSourceRange {
    pub source: FigureId,
    pub range: Range<usize>,
    pub source_offset: usize,
    pub ancestors: Vec<FigureId>,
}

pub(crate) struct BoxSource {
    pub child: Option<FigureId>,
    pub metrics: Option<Dimension>,
    pub border_marker: Option<(FigureId, bool)>,
    pub offset: usize,
    pub ancestors: Vec<FigureId>,
}
pub(crate) struct Paragraph {
    pub owner: FigureId,
    pub text: String,
    pub style: TextStyle,
    pub spans: Vec<TextStyleSpan>,
    pub sources: Vec<FlowSourceRange>,
    pub boxes: Vec<BoxSource>,
    pub block: FlowBlockStyle,
    pub before: f64,
    pub top_padding: f64,
    pub after: f64,
}

pub(crate) struct BlockBox {
    pub source: FigureId,
    pub paragraphs: Range<usize>,
    pub left: f64,
    pub right: f64,
    pub padding_above: f64,
    pub bottom_padding: f64,
}

type BorderMeasurer<'a> = dyn FnMut(
        FigureId,
        Arc<dyn super::FlowFragmentBorder>,
    ) -> Result<(super::FlowBorderMetrics, Arc<dyn super::FlowFragmentBorder>), FlowError>
    + 'a;

pub(crate) struct FlowPlan {
    pub paragraphs: Vec<Paragraph>,
    pub members: Vec<FigureId>,
    pub decorations: Vec<super::border::Decoration>,
    pub blocks: Vec<BlockBox>,
    pub trailing_margin: f64,
    pub traversal_order: HashMap<FigureId, (usize, usize)>,
}

/// Normalized paragraph builder used by every registered handler.
/// It can only append contributions or traverse frozen direct children.
pub struct FlowAssembly<'a> {
    scene: &'a PreparationScene<'a>,
    registry: &'a FlowContributionRegistry,
    measure_border: &'a mut BorderMeasurer<'a>,
    pub(crate) paragraphs: Vec<Paragraph>,
    pub(crate) members: Vec<FigureId>,
    pub(crate) decorations: Vec<super::border::Decoration>,
    pub(crate) blocks: Vec<BlockBox>,
    pub(crate) traversal_order: HashMap<FigureId, (usize, usize)>,
    next_order: usize,
    visited: HashSet<FigureId>,
    stack: Vec<FigureId>,
    current: Option<Paragraph>,
    block: FlowBlockStyle,
    owner: FigureId,
    pending_margin: f64,
    pending_padding: f64,
    source_offsets: HashMap<FigureId, usize>,
}
impl<'a> FlowAssembly<'a> {
    fn allocate(&self, entries: usize) -> Result<(), FlowError> {
        self.scene
            .budget()
            .allocate("flow assembly", entries as u64)
            .map_err(|error| {
                crate::text::TextError::PreparationDependency(Box::new(
                    crate::figure::PreparationDependencyError::Budget(error),
                ))
                .into()
            })
    }
    pub(crate) fn new(
        scene: &'a PreparationScene<'a>,
        registry: &'a FlowContributionRegistry,
        measure_border: &'a mut BorderMeasurer<'a>,
    ) -> Self {
        Self {
            scene,
            registry,
            measure_border,
            paragraphs: Vec::new(),
            members: Vec::new(),
            decorations: Vec::new(),
            blocks: Vec::new(),
            traversal_order: HashMap::new(),
            next_order: 0,
            visited: HashSet::new(),
            stack: Vec::new(),
            current: None,
            block: FlowBlockStyle::default(),
            owner: scene.root(),
            pending_margin: 0.0,
            pending_padding: 0.0,
            source_offsets: HashMap::new(),
        }
    }
    pub fn visit(&mut self, source: FigureId) -> Result<(), FlowError> {
        self.allocate(4)?;
        if self.stack.last().copied() != self.scene.parent(source)? {
            return Err(FlowError::InvalidStructure);
        }
        if self.stack.len() >= 10_000 {
            return Err(FlowError::DepthLimit);
        }
        if !self.visited.insert(source) {
            return Err(FlowError::InvalidStructure);
        }
        self.members.push(source);
        let opening = self.next_order;
        self.next_order += 1;
        self.stack.push(source);
        let registry = self.registry;
        let result = registry.dispatch(source, self.scene, self);
        self.stack.pop();
        self.traversal_order
            .insert(source, (opening, self.next_order));
        self.next_order += 1;
        result
    }
    pub fn children(&mut self, source: FigureId) -> Result<(), FlowError> {
        if self.stack.last() != Some(&source) {
            return Err(FlowError::InvalidStructure);
        }
        for child in self.scene.children(source)? {
            self.visit(child)?;
        }
        Ok(())
    }
    fn paragraph(&mut self, style: TextStyle) -> &mut Paragraph {
        if self.current.is_none() {
            self.current = Some(Paragraph {
                owner: self.owner,
                text: String::new(),
                style,
                spans: Vec::new(),
                sources: Vec::new(),
                boxes: Vec::new(),
                block: self.block,
                before: self.pending_margin + self.pending_padding,
                top_padding: self.pending_padding,
                after: 0.0,
            });
            self.pending_margin = 0.0;
            self.pending_padding = 0.0;
        }
        self.current.as_mut().expect("paragraph initialized")
    }
    pub fn text(
        &mut self,
        source: FigureId,
        text: &str,
        style: TextStyle,
    ) -> Result<(), FlowError> {
        if self.stack.last() != Some(&source) {
            return Err(FlowError::InvalidStructure);
        }
        self.allocate(
            text.len()
                .saturating_add(self.stack.len())
                .saturating_add(4),
        )?;
        let ancestors = self.stack.clone();
        let source_offset = *self.source_offsets.entry(source).or_default();
        self.source_offsets
            .insert(source, source_offset + text.len());
        let paragraph = self.paragraph(style.clone());
        let start = paragraph.text.len();
        paragraph.text.push_str(text);
        if !text.is_empty() {
            paragraph.spans.push(TextStyleSpan {
                range: start..paragraph.text.len(),
                style,
            });
        }
        paragraph.sources.push(FlowSourceRange {
            source,
            range: start..paragraph.text.len(),
            source_offset,
            ancestors,
        });
        Ok(())
    }
    /// Ordinary subtrees retain identity; only the adapter's direct child is measured.
    pub fn figure_box(
        &mut self,
        source: FigureId,
        child: FigureId,
        style: TextStyle,
    ) -> Result<(), FlowError> {
        if self.stack.last() != Some(&source) || self.scene.parent(child)? != Some(source) {
            return Err(FlowError::InvalidStructure);
        }
        self.allocate(self.stack.len().saturating_add(2))?;
        let ancestors = self.stack.clone();
        let paragraph = self.paragraph(style);
        paragraph.boxes.push(BoxSource {
            child: Some(child),
            metrics: None,
            border_marker: None,
            offset: paragraph.text.len(),
            ancestors,
        });
        Ok(())
    }
    pub fn decorate(
        &mut self,
        source: FigureId,
        border: Arc<dyn super::FlowFragmentBorder>,
    ) -> Result<(), FlowError> {
        if self.stack.last() != Some(&source) {
            return Err(FlowError::InvalidStructure);
        }
        self.allocate(1)?;
        let (metrics, border) = (self.measure_border)(source, border)?;
        self.decorations.push(super::border::Decoration {
            source,
            border,
            metrics,
        });
        Ok(())
    }
    /// Block handlers use this same scope API, including third-party block contributors.
    pub fn block(&mut self, source: FigureId, mut style: FlowBlockStyle) -> Result<(), FlowError> {
        self.allocate(2)?;
        style.validate()?;
        if self.stack.last() != Some(&source) {
            return Err(FlowError::InvalidStructure);
        }
        self.flush();
        let previous = self.block;
        let owner = self.owner;
        if let Some(decoration) = self.decorations.iter().find(|d| d.source == source) {
            let metrics = decoration.metrics;
            style.margin.top += metrics.margin.top;
            style.margin.bottom += metrics.margin.bottom;
            style.margin.left += metrics.margin.left;
            style.margin.right += metrics.margin.right;
            style.padding.top += metrics.insets.top;
            style.padding.bottom += metrics.insets.bottom;
            style.padding.left += metrics.insets.left;
            style.padding.right += metrics.insets.right;
        }
        // The preparing root already receives its client area as the layout region.
        let insets = if source == self.scene.root() {
            Insets::ZERO
        } else {
            self.scene.insets(source)?
        };
        style.padding.top += insets.top;
        style.padding.bottom += insets.bottom;
        style.padding.left += insets.left;
        style.padding.right += insets.right;
        style.validate()?;
        let padding_above = self.pending_padding;
        let left = previous.padding.left + previous.margin.left + style.margin.left;
        let right = previous.padding.right + previous.margin.right + style.margin.right;
        if style.direction == ParagraphDirection::Auto {
            style.direction = previous.direction;
        }
        style.alignment = style.alignment.or(previous.alignment);
        self.pending_margin = self.pending_margin.max(style.margin.top);
        self.pending_padding += style.padding.top;
        style.padding.left += previous.padding.left;
        style.padding.right += previous.padding.right;
        style.margin.left += previous.margin.left;
        style.margin.right += previous.margin.right;
        self.block = style;
        self.owner = source;
        let start = self.paragraphs.len();
        self.children(source)?;
        // Preserve empty block identity and its vertical spacing.
        if self.current.is_none() && self.paragraphs.len() == start {
            let resolved = self.scene.style(source)?;
            let style = TextStyle::new(
                crate::text::FontDescriptor::parse(&resolved.font)?,
                resolved.foreground,
            );
            self.paragraph(style);
        }
        self.flush();
        self.pending_margin = self.pending_margin.max(style.margin.bottom);
        if let Some(last) = self.paragraphs.last_mut() {
            last.after += style.padding.bottom;
        }
        self.blocks.push(BlockBox {
            source,
            paragraphs: start..self.paragraphs.len(),
            left,
            right,
            padding_above,
            bottom_padding: self.paragraphs.last().map_or(0.0, |p| p.after),
        });
        self.block = previous;
        self.owner = owner;
        Ok(())
    }
    fn flush(&mut self) {
        if let Some(paragraph) = self.current.take() {
            self.paragraphs.push(paragraph);
        }
    }
    pub(crate) fn finish(mut self) -> FlowPlan {
        self.flush();
        FlowPlan {
            paragraphs: self.paragraphs,
            members: self.members,
            decorations: self.decorations,
            blocks: self.blocks,
            trailing_margin: self.pending_margin,
            traversal_order: self.traversal_order,
        }
    }
}
