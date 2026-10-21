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
    pub alignment: ParagraphAlignment,
    pub wrap: bool,
    pub margin: Insets,
    pub padding: Insets,
}
impl Default for FlowBlockStyle {
    fn default() -> Self {
        Self {
            direction: ParagraphDirection::Auto,
            alignment: ParagraphAlignment::Leading,
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
    pub after: f64,
}

/// Normalized paragraph builder used by every registered handler.
/// It can only append contributions or traverse frozen direct children.
pub struct FlowAssembly<'a> {
    scene: &'a PreparationScene<'a>,
    registry: &'a FlowContributionRegistry,
    pub(crate) paragraphs: Vec<Paragraph>,
    pub(crate) members: Vec<FigureId>,
    pub(crate) decorations: Vec<super::border::Decoration>,
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
    pub(crate) fn new(
        scene: &'a PreparationScene<'a>,
        registry: &'a FlowContributionRegistry,
    ) -> Self {
        Self {
            scene,
            registry,
            paragraphs: Vec::new(),
            members: Vec::new(),
            decorations: Vec::new(),
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
        self.stack.push(source);
        let registry = self.registry;
        let result = registry.dispatch(source, self.scene, self);
        self.stack.pop();
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
        self.decorations
            .push(super::border::Decoration { source, border });
        Ok(())
    }
    /// Block handlers use this same scope API, including third-party block contributors.
    pub fn block(&mut self, source: FigureId, mut style: FlowBlockStyle) -> Result<(), FlowError> {
        style.validate()?;
        if self.stack.last() != Some(&source) {
            return Err(FlowError::InvalidStructure);
        }
        self.flush();
        let previous = self.block;
        let owner = self.owner;
        if style.direction == ParagraphDirection::Auto {
            style.direction = previous.direction;
        }
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
        self.block = previous;
        self.owner = owner;
        Ok(())
    }
    fn flush(&mut self) {
        if let Some(paragraph) = self.current.take() {
            self.paragraphs.push(paragraph);
        }
    }
    pub(crate) fn finish(&mut self) {
        self.flush();
    }
}
