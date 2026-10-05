use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use parley::{
    FontContext, FontFamily, Layout, LayoutContext, PositionedLayoutItem, StyleProperty,
    editing::{Cursor, Selection},
    layout::Affinity as ParleyAffinity,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Point, Rectangle, ResourceId};

const DEFAULT_FONT_FAMILY: &str = "Inter Variable";
const DEFAULT_FONT_SIZE: f32 = 12.0;
const DEFAULT_FONT_WEIGHT: f32 = 400.0;
const FNV_1A_64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_1A_64_PRIME: u64 = 0x0000_0100_0000_01b3;
const INTER_FONT: &[u8] = include_bytes!("../../../assets/fonts/InterVariable.ttf");
const NOTO_SANS_SC_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-VF.ttf");
const JETBRAINS_MONO_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const NOTO_SANS_ARABIC_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansArabic-VF.ttf");

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFont {
    Inter,
    NotoSansSc,
    JetBrainsMono,
    NotoSansArabic,
}

impl BuiltinFont {
    pub const ALL: [Self; 4] = [
        Self::Inter,
        Self::NotoSansSc,
        Self::JetBrainsMono,
        Self::NotoSansArabic,
    ];

    pub const fn family(self) -> &'static str {
        match self {
            Self::Inter => "Inter Variable",
            Self::NotoSansSc => "Noto Sans SC",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::NotoSansArabic => "Noto Sans Arabic",
        }
    }

    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Inter => INTER_FONT,
            Self::NotoSansSc => NOTO_SANS_SC_FONT,
            Self::JetBrainsMono => JETBRAINS_MONO_FONT,
            Self::NotoSansArabic => NOTO_SANS_ARABIC_FONT,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontDescriptor {
    pub(crate) family: String,
    pub(crate) size: f32,
    pub(crate) weight: f32,
    pub(crate) style: FontStyle,
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            family: DEFAULT_FONT_FAMILY.to_string(),
            size: DEFAULT_FONT_SIZE,
            weight: DEFAULT_FONT_WEIGHT,
            style: FontStyle::Normal,
        }
    }
}

impl FontDescriptor {
    pub fn family(&self) -> &str {
        &self.family
    }
    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn weight(&self) -> f32 {
        self.weight
    }
    pub fn style(&self) -> FontStyle {
        self.style
    }

    pub fn new(family: impl Into<String>, size: f32) -> Result<Self, TextError> {
        let descriptor = Self {
            family: family.into(),
            size,
            ..Self::default()
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn with_weight(mut self, weight: f32) -> Result<Self, TextError> {
        self.weight = weight;
        self.validate()?;
        Ok(self)
    }

    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn parse(value: &str) -> Result<Self, TextError> {
        let value = value.trim();
        let Some(size_start) = value.find(|character: char| character.is_ascii_digit()) else {
            return Err(TextError::InvalidFontDescriptor(value.to_string()));
        };
        let before_size = value[..size_start].trim();
        let size_and_family = &value[size_start..];
        let Some(px_end) = size_and_family.find("px") else {
            return Err(TextError::InvalidFontDescriptor(value.to_string()));
        };
        let size = size_and_family[..px_end]
            .trim()
            .parse::<f32>()
            .map_err(|_| TextError::InvalidFontDescriptor(value.to_string()))?;
        let family = size_and_family[px_end + 2..].trim();
        if family.is_empty() {
            return Err(TextError::InvalidFontDescriptor(value.to_string()));
        }

        let mut descriptor = Self::new(family, size)?;
        for token in before_size.split_whitespace() {
            match token {
                "normal" => {}
                "italic" => descriptor.style = FontStyle::Italic,
                "oblique" => descriptor.style = FontStyle::Oblique,
                "bold" => descriptor.weight = 700.0,
                token => {
                    descriptor.weight = token
                        .parse::<f32>()
                        .map_err(|_| TextError::InvalidFontDescriptor(value.to_string()))?;
                }
            }
        }
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn validate(&self) -> Result<(), TextError> {
        if self.family.trim().is_empty()
            || !self.size.is_finite()
            || self.size <= 0.0
            || !self.weight.is_finite()
            || !(1.0..=1000.0).contains(&self.weight)
        {
            return Err(TextError::InvalidFontDescriptor(format!(
                "{}px {}",
                self.size, self.family
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextConstraints {
    pub max_width: Option<f32>,
}

impl TextConstraints {
    pub const UNBOUNDED: Self = Self { max_width: None };

    pub fn new(max_width: Option<f32>) -> Result<Self, TextError> {
        if max_width.is_some_and(|width| !width.is_finite() || width < 0.0) {
            return Err(TextError::InvalidMetric);
        }
        Ok(Self { max_width })
    }
}

impl Default for TextConstraints {
    fn default() -> Self {
        Self::UNBOUNDED
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextLineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub baseline: f32,
    pub advance: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TextAffinity {
    Upstream,
    #[default]
    Downstream,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextLayoutRevision(u64);

impl TextLayoutRevision {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextPosition {
    byte_offset: usize,
    affinity: TextAffinity,
    layout_revision: Option<TextLayoutRevision>,
}

impl TextPosition {
    pub const fn new(byte_offset: usize, affinity: TextAffinity) -> Self {
        Self {
            byte_offset,
            affinity,
            layout_revision: None,
        }
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

    pub const fn with_layout_revision(self, layout_revision: TextLayoutRevision) -> Self {
        Self {
            layout_revision: Some(layout_revision),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRange {
    anchor: TextPosition,
    focus: TextPosition,
}

impl TextRange {
    pub const fn new(anchor: TextPosition, focus: TextPosition) -> Self {
        Self { anchor, focus }
    }

    pub const fn anchor(self) -> TextPosition {
        self.anchor
    }

    pub const fn focus(self) -> TextPosition {
        self.focus
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextMovement {
    PreviousVisual,
    NextVisual,
    PreviousWord,
    NextWord,
    PreviousLine,
    NextLine,
    LineStart,
    LineEnd,
    ParagraphStart,
    ParagraphEnd,
    DocumentStart,
    DocumentEnd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaretGeometry {
    bounds: Rectangle,
    line_index: usize,
}

impl CaretGeometry {
    pub const fn new(bounds: Rectangle, line_index: usize) -> Self {
        Self { bounds, line_index }
    }

    pub const fn bounds(self) -> Rectangle {
        self.bounds
    }

    pub const fn line_index(self) -> usize {
        self.line_index
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectionQuad {
    bounds: Rectangle,
    line_index: usize,
}

impl SelectionQuad {
    pub const fn new(bounds: Rectangle, line_index: usize) -> Self {
        Self { bounds, line_index }
    }

    pub const fn bounds(self) -> Rectangle {
        self.bounds
    }

    pub const fn line_index(self) -> usize {
        self.line_index
    }
}

pub trait TextInteractionProvider: Send + Sync {
    fn text_len(&self) -> usize;

    fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError>;

    fn caret_geometry(&self, position: TextPosition)
    -> Result<CaretGeometry, TextInteractionError>;

    fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError>;

    fn move_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError>;
}

#[derive(Clone)]
pub struct TextInteractionMap {
    source: Arc<str>,
    visible_range: Range<usize>,
    revision: TextLayoutRevision,
    provider: Arc<dyn TextInteractionProvider>,
}

impl fmt::Debug for TextInteractionMap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextInteractionMap")
            .field("source", &self.source)
            .field("visible_range", &self.visible_range)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TextInteractionMap {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.visible_range == other.visible_range
            && self.revision == other.revision
    }
}

impl TextInteractionMap {
    pub fn new(
        source: impl Into<Arc<str>>,
        provider: Arc<dyn TextInteractionProvider>,
    ) -> Result<Self, TextInteractionError> {
        let source = source.into();
        if provider.text_len() != source.len() {
            return Err(TextInteractionError::ProviderTextMismatch);
        }
        let visible_range = 0..source.len();
        Ok(Self {
            revision: source_revision(&source),
            source,
            visible_range,
            provider,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }

    pub const fn revision(&self) -> TextLayoutRevision {
        self.revision
    }

    pub fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(TextInteractionError::NonFinitePoint);
        }
        let position = self.normalize_provider_position(self.provider.hit_test(point)?, None);
        let position = if position.byte_offset > self.visible_range.end {
            TextPosition::new(self.visible_range.end, TextAffinity::Upstream)
        } else {
            position
        };
        self.validate_position(position)?;
        Ok(position.with_layout_revision(self.revision))
    }

    pub fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        self.validate_position(position)?;
        let geometry = self.provider.caret_geometry(position)?;
        validate_interaction_rectangle(geometry.bounds)?;
        Ok(geometry)
    }

    pub fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        self.validate_position(range.anchor)?;
        self.validate_position(range.focus)?;
        let quads = self.provider.selection_geometry(range)?;
        for quad in &quads {
            validate_interaction_rectangle(quad.bounds)?;
        }
        Ok(quads)
    }

    pub fn move_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        self.validate_position(position)?;
        let moved = match movement {
            TextMovement::ParagraphStart => {
                let start = self.source[..position.byte_offset]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                TextPosition::new(start, TextAffinity::Downstream)
            }
            TextMovement::ParagraphEnd => {
                let end = self.source[position.byte_offset..]
                    .find('\n')
                    .map_or(self.source.len(), |index| position.byte_offset + index);
                TextPosition::new(end, TextAffinity::Upstream)
            }
            TextMovement::DocumentStart => TextPosition::new(0, TextAffinity::Downstream),
            TextMovement::DocumentEnd => {
                TextPosition::new(self.visible_range.end, TextAffinity::Upstream)
            }
            _ => self.normalize_provider_position(
                self.provider.move_position(position, movement)?,
                Some(position),
            ),
        };
        let moved = if moved.byte_offset > self.visible_range.end {
            TextPosition::new(self.visible_range.end, TextAffinity::Upstream)
        } else {
            moved
        };
        self.validate_position(moved)?;
        Ok(moved.with_layout_revision(self.revision))
    }

    fn validate_position(&self, position: TextPosition) -> Result<(), TextInteractionError> {
        if position
            .layout_revision
            .is_some_and(|revision| revision != self.revision)
        {
            return Err(TextInteractionError::StaleTextLayoutRevision);
        }
        if position.byte_offset > self.source.len()
            || !is_grapheme_boundary(&self.source, position.byte_offset)
        {
            return Err(TextInteractionError::InvalidTextPosition);
        }
        if position.byte_offset < self.visible_range.start
            || position.byte_offset > self.visible_range.end
        {
            return Err(TextInteractionError::InvisibleTextPosition);
        }
        Ok(())
    }

    fn normalize_provider_position(
        &self,
        position: TextPosition,
        origin: Option<TextPosition>,
    ) -> TextPosition {
        if is_grapheme_boundary(&self.source, position.byte_offset) {
            return position;
        }
        let move_forward = origin.map_or(position.affinity == TextAffinity::Upstream, |origin| {
            position.byte_offset >= origin.byte_offset
        });
        let boundary = if move_forward {
            grapheme_boundaries(&self.source)
                .find(|boundary| *boundary >= position.byte_offset)
                .unwrap_or(self.source.len())
        } else {
            grapheme_boundaries(&self.source)
                .take_while(|boundary| *boundary <= position.byte_offset)
                .last()
                .unwrap_or(0)
        };
        TextPosition::new(boundary, position.affinity)
    }

    fn with_visibility(
        mut self,
        source: Arc<str>,
        visible_range: Range<usize>,
    ) -> Result<Self, TextInteractionError> {
        validate_interaction_visible_range(&source, &visible_range)?;
        if visible_range.end > self.provider.text_len() {
            return Err(TextInteractionError::ProviderTextMismatch);
        }
        self.source = source;
        self.visible_range = visible_range;
        Ok(self)
    }

    fn bind_revision(&mut self, revision: TextLayoutRevision) {
        self.revision = revision;
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FontFaceRef {
    resource_id: ResourceId,
    revision: u64,
    collection_index: u32,
}

impl FontFaceRef {
    pub const fn new(resource_id: ResourceId, revision: u64, collection_index: u32) -> Self {
        Self {
            resource_id,
            revision,
            collection_index,
        }
    }

    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn collection_index(&self) -> u32 {
        self.collection_index
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositionedGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontFaceRef,
    pub font_size: f32,
    pub normalized_coords: Vec<i16>,
    pub skew_degrees: Option<f32>,
    pub glyphs: Vec<PositionedGlyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlyphPaint {
    Fill(super::Paint),
    Stroke {
        paint: super::Paint,
        stroke: super::StrokeStyle,
    },
}

impl GlyphPaint {
    pub fn paint(&self) -> &super::Paint {
        match self {
            Self::Fill(paint) | Self::Stroke { paint, .. } => paint,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    width: f32,
    full_width: f32,
    height: f32,
    lines: Vec<TextLineMetrics>,
    glyph_runs: Vec<GlyphRun>,
    interaction: Option<Arc<TextInteractionMap>>,
    visible_range: Range<usize>,
    truncated: bool,
    key: TextLayoutKey,
    pub(crate) outlines: Option<Arc<crate::text::outline::OutlinedText>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutParts {
    pub text: String,
    pub font: FontDescriptor,
    pub constraints: TextConstraints,
    pub engine_revision: u64,
    pub width: f32,
    pub full_width: f32,
    pub height: f32,
    pub lines: Vec<TextLineMetrics>,
    pub glyph_runs: Vec<GlyphRun>,
    pub visible_range: Range<usize>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutKey {
    text: Arc<str>,
    font: FontDescriptor,
    constraints: TextConstraints,
    engine_revision: u64,
}

impl Default for TextLayoutKey {
    fn default() -> Self {
        Self {
            text: Arc::from(""),
            font: FontDescriptor::default(),
            constraints: TextConstraints::UNBOUNDED,
            engine_revision: 0,
        }
    }
}

impl TextLayoutKey {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn font(&self) -> &FontDescriptor {
        &self.font
    }

    pub fn constraints(&self) -> TextConstraints {
        self.constraints
    }

    pub fn engine_revision(&self) -> u64 {
        self.engine_revision
    }
}

impl TextLayout {
    pub(crate) fn set_engine_revision(&mut self, revision: u64) {
        self.key.engine_revision = revision;
        let interaction_revision = self.interaction_revision();
        if let Some(interaction) = self.interaction.as_mut() {
            Arc::make_mut(interaction).bind_revision(interaction_revision);
        }
    }

    /// Prepared neutral geometry, present only when an outline consumer was selected.
    pub fn outlines(&self) -> Option<&crate::text::outline::OutlinedText> {
        self.outlines.as_deref()
    }

    /// Exact unhinted ink for prepared outlines; independent of layout advance.
    pub fn ink_bounds(&self) -> Option<Rectangle> {
        self.outlines().and_then(|outlines| outlines.ink_bounds())
    }

    /// Logical layout size; this is not the visible ink envelope.
    pub fn size(&self) -> crate::Dimension {
        crate::Dimension::new(f64::from(self.width), f64::from(self.height))
    }

    /// Metrics from this exact immutable shaping snapshot.
    pub fn metrics(&self) -> crate::text::TextMetrics {
        crate::text::TextMetrics::from_layout(self)
    }

    pub fn from_parts(parts: TextLayoutParts) -> Result<Self, TextError> {
        parts.font.validate()?;
        TextConstraints::new(parts.constraints.max_width)?;
        validate_metric(parts.width)?;
        validate_metric(parts.full_width)?;
        validate_metric(parts.height)?;
        validate_visible_range(&parts.text, &parts.visible_range, parts.truncated)?;
        for line in &parts.lines {
            validate_metric(line.ascent)?;
            validate_metric(line.descent)?;
            validate_finite(line.leading)?;
            validate_metric(line.baseline)?;
            validate_metric(line.advance)?;
        }
        for run in &parts.glyph_runs {
            if !run.font_size.is_finite() || run.font_size <= 0.0 {
                return Err(TextError::InvalidGlyphRun);
            }
            if run.skew_degrees.is_some_and(|skew| !skew.is_finite())
                || run
                    .glyphs
                    .iter()
                    .any(|glyph| !glyph.x.is_finite() || !glyph.y.is_finite())
            {
                return Err(TextError::InvalidGlyphRun);
            }
        }

        Ok(Self {
            width: parts.width,
            full_width: parts.full_width,
            height: parts.height,
            lines: parts.lines,
            glyph_runs: parts.glyph_runs,
            interaction: None,
            outlines: None,
            visible_range: parts.visible_range,
            truncated: parts.truncated,
            key: TextLayoutKey {
                text: Arc::from(parts.text),
                font: parts.font,
                constraints: parts.constraints,
                engine_revision: parts.engine_revision,
            },
        })
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn full_width(&self) -> f32 {
        self.full_width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn line_metrics(&self) -> &[TextLineMetrics] {
        &self.lines
    }

    pub fn glyph_runs(&self) -> &[GlyphRun] {
        &self.glyph_runs
    }

    pub fn with_interaction_map(
        mut self,
        mut interaction: TextInteractionMap,
    ) -> Result<Self, TextError> {
        if interaction.source() != self.key.text() {
            return Err(TextError::InvalidInteractionMap);
        }
        interaction.bind_revision(self.interaction_revision());
        self.interaction = Some(Arc::new(interaction));
        Ok(self)
    }

    pub fn interaction_map(&self) -> Option<&TextInteractionMap> {
        self.interaction.as_deref()
    }

    pub fn hit_test_text(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        self.interaction_map()
            .ok_or(TextInteractionError::Unavailable)?
            .hit_test(point)
    }

    pub fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        self.interaction_map()
            .ok_or(TextInteractionError::Unavailable)?
            .caret_geometry(position)
    }

    pub fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        self.interaction_map()
            .ok_or(TextInteractionError::Unavailable)?
            .selection_geometry(range)
    }

    pub fn move_text_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        self.interaction_map()
            .ok_or(TextInteractionError::Unavailable)?
            .move_position(position, movement)
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub fn key(&self) -> &TextLayoutKey {
        &self.key
    }

    pub fn ascent(&self) -> f32 {
        self.lines.first().map_or(0.0, |line| line.ascent)
    }

    pub fn descent(&self) -> f32 {
        self.lines.first().map_or(0.0, |line| line.descent)
    }

    pub fn baseline(&self) -> f32 {
        self.lines.first().map_or(0.0, |line| line.baseline)
    }

    pub fn is_empty(&self) -> bool {
        self.glyph_runs.iter().all(|run| run.glyphs.is_empty())
    }

    pub fn with_visibility(
        mut self,
        source: impl Into<Arc<str>>,
        visible_range: Range<usize>,
        truncated: bool,
        full_width: f32,
        constraints: TextConstraints,
    ) -> Result<Self, TextError> {
        let source = source.into();
        TextConstraints::new(constraints.max_width)?;
        validate_metric(full_width)?;
        validate_visible_range(&source, &visible_range, truncated)?;
        self.key.text = Arc::clone(&source);
        self.key.constraints = constraints;
        self.visible_range = visible_range;
        self.truncated = truncated;
        self.full_width = full_width;
        if let Some(interaction) = self.interaction.take() {
            let mut interaction = (*interaction)
                .clone()
                .with_visibility(source, self.visible_range.clone())
                .map_err(|_| TextError::InvalidInteractionMap)?;
            interaction.bind_revision(self.interaction_revision());
            self.interaction = Some(Arc::new(interaction));
        }
        Ok(self)
    }

    fn interaction_revision(&self) -> TextLayoutRevision {
        let mut revision = source_revision(self.key.text());
        mix_revision(&mut revision, self.key.engine_revision().to_le_bytes());
        mix_revision(
            &mut revision,
            self.key.font().family.as_bytes().iter().copied(),
        );
        mix_revision(&mut revision, self.key.font().size.to_bits().to_le_bytes());
        mix_revision(
            &mut revision,
            self.key.font().weight.to_bits().to_le_bytes(),
        );
        mix_revision(
            &mut revision,
            [match self.key.font().style {
                FontStyle::Normal => 0,
                FontStyle::Italic => 1,
                FontStyle::Oblique => 2,
            }],
        );
        mix_revision(&mut revision, self.width.to_bits().to_le_bytes());
        mix_revision(&mut revision, self.full_width.to_bits().to_le_bytes());
        mix_revision(&mut revision, self.height.to_bits().to_le_bytes());
        mix_revision(
            &mut revision,
            self.key
                .constraints()
                .max_width
                .map(f32::to_bits)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        mix_revision(&mut revision, self.visible_range.start.to_le_bytes());
        mix_revision(&mut revision, self.visible_range.end.to_le_bytes());
        mix_revision(&mut revision, [u8::from(self.truncated)]);
        for line in &self.lines {
            mix_revision(&mut revision, line.ascent.to_bits().to_le_bytes());
            mix_revision(&mut revision, line.descent.to_bits().to_le_bytes());
            mix_revision(&mut revision, line.leading.to_bits().to_le_bytes());
            mix_revision(&mut revision, line.baseline.to_bits().to_le_bytes());
            mix_revision(&mut revision, line.advance.to_bits().to_le_bytes());
        }
        revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextError {
    RuntimeFaulted,
    InvalidFontDescriptor(String),
    InvalidFontData,
    InvalidMetric,
    InvalidVisibleRange,
    InvalidGlyphRun,
    InvalidInteractionMap,
    NoUsableFont,
    FontMetricsUnsupported,
    Outline(crate::text::outline::FontError),
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeFaulted => formatter.write_str("Runtime is faulted"),
            Self::InvalidFontDescriptor(descriptor) => {
                write!(formatter, "invalid font descriptor: {descriptor}")
            }
            Self::InvalidFontData => formatter.write_str("font data is empty or invalid"),
            Self::InvalidMetric => {
                formatter.write_str("text metric must be finite and non-negative")
            }
            Self::InvalidVisibleRange => {
                formatter.write_str("visible text range must be ordered and on UTF-8 boundaries")
            }
            Self::InvalidGlyphRun => {
                formatter.write_str("glyph run metrics must be finite and font size positive")
            }
            Self::InvalidInteractionMap => {
                formatter.write_str("text interaction map does not match the text layout")
            }
            Self::NoUsableFont => formatter.write_str("no usable font was found for the text"),
            Self::FontMetricsUnsupported => {
                formatter.write_str("layout engine does not provide font metrics")
            }
            Self::Outline(error) => error.fmt(formatter),
        }
    }
}

impl Error for TextError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextInteractionError {
    Unavailable,
    InvalidParagraph,
    InvalidTextPosition,
    InvisibleTextPosition,
    StaleTextLayoutRevision,
    NonFinitePoint,
    InvalidGeometry,
    ProviderTextMismatch,
}

impl fmt::Display for TextInteractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("text interaction data is unavailable"),
            Self::InvalidParagraph => formatter.write_str("text paragraph is invalid"),
            Self::InvalidTextPosition => formatter.write_str("text position is invalid"),
            Self::InvisibleTextPosition => formatter.write_str("text position is not visible"),
            Self::StaleTextLayoutRevision => {
                formatter.write_str("text position belongs to a stale layout revision")
            }
            Self::NonFinitePoint => formatter.write_str("text hit-test point must be finite"),
            Self::InvalidGeometry => {
                formatter.write_str("text interaction geometry must be finite and non-negative")
            }
            Self::ProviderTextMismatch => {
                formatter.write_str("text interaction provider does not match source text")
            }
        }
    }
}

impl Error for TextInteractionError {}

fn validate_metric(value: f32) -> Result<(), TextError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(TextError::InvalidMetric)
    }
}

fn validate_finite(value: f32) -> Result<(), TextError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(TextError::InvalidMetric)
    }
}

fn validate_visible_range(
    text: &str,
    visible_range: &Range<usize>,
    truncated: bool,
) -> Result<(), TextError> {
    let valid = visible_range.start <= visible_range.end
        && visible_range.end <= text.len()
        && text.is_char_boundary(visible_range.start)
        && text.is_char_boundary(visible_range.end)
        && (truncated || *visible_range == (0..text.len()));
    if valid {
        Ok(())
    } else {
        Err(TextError::InvalidVisibleRange)
    }
}

fn validate_interaction_visible_range(
    text: &str,
    visible_range: &Range<usize>,
) -> Result<(), TextInteractionError> {
    if visible_range.start <= visible_range.end
        && visible_range.end <= text.len()
        && text.is_char_boundary(visible_range.start)
        && text.is_char_boundary(visible_range.end)
    {
        Ok(())
    } else {
        Err(TextInteractionError::InvalidTextPosition)
    }
}

fn grapheme_boundaries(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
}

fn source_revision(text: &str) -> TextLayoutRevision {
    let mut revision = TextLayoutRevision(FNV_1A_64_OFFSET_BASIS);
    mix_revision(&mut revision, text.as_bytes().iter().copied());
    revision
}

fn mix_revision(revision: &mut TextLayoutRevision, bytes: impl IntoIterator<Item = u8>) {
    for byte in bytes {
        revision.0 ^= u64::from(byte);
        revision.0 = revision.0.wrapping_mul(FNV_1A_64_PRIME);
    }
}

fn is_grapheme_boundary(text: &str, offset: usize) -> bool {
    grapheme_boundaries(text).any(|boundary| boundary == offset)
}

fn validate_interaction_rectangle(rectangle: Rectangle) -> Result<(), TextInteractionError> {
    if rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
        && rectangle.width >= 0.0
        && rectangle.height >= 0.0
    {
        Ok(())
    } else {
        Err(TextInteractionError::InvalidGeometry)
    }
}

#[derive(Clone)]
struct ParleyTextInteraction {
    layout: Layout<()>,
    text_len: usize,
}

impl TextInteractionProvider for ParleyTextInteraction {
    fn text_len(&self) -> usize {
        self.text_len
    }

    fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        let cursor = Cursor::from_point(&self.layout, point.x() as f32, point.y() as f32);
        Ok(text_position_from_parley(cursor))
    }

    fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        let cursor = parley_cursor(&self.layout, position)?;
        let bounds = cursor.geometry(&self.layout, 0.0);
        let midpoint = ((bounds.y0 + bounds.y1) * 0.5) as f32;
        let line_index = self
            .layout
            .lines()
            .position(|line| {
                let metrics = line.metrics();
                midpoint >= metrics.min_coord && midpoint <= metrics.max_coord
            })
            .unwrap_or_else(|| self.layout.len().saturating_sub(1));
        Ok(CaretGeometry::new(
            Rectangle::new(bounds.x0, bounds.y0, bounds.width(), bounds.height()),
            line_index,
        ))
    }

    fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        let anchor = parley_cursor(&self.layout, range.anchor)?;
        let focus = parley_cursor(&self.layout, range.focus)?;
        Ok(Selection::new(anchor, focus)
            .geometry(&self.layout)
            .into_iter()
            .map(|(bounds, line_index)| {
                SelectionQuad::new(
                    Rectangle::new(bounds.x0, bounds.y0, bounds.width(), bounds.height()),
                    line_index,
                )
            })
            .collect())
    }

    fn move_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        let cursor = parley_cursor(&self.layout, position)?;
        let selection = Selection::new(cursor, cursor);
        let moved = match movement {
            TextMovement::PreviousVisual => selection.previous_visual(&self.layout, false),
            TextMovement::NextVisual => selection.next_visual(&self.layout, false),
            TextMovement::PreviousWord => selection.previous_visual_word(&self.layout, false),
            TextMovement::NextWord => selection.next_visual_word(&self.layout, false),
            TextMovement::PreviousLine => selection.previous_line(&self.layout, false),
            TextMovement::NextLine => selection.next_line(&self.layout, false),
            TextMovement::LineStart => selection.line_start(&self.layout, false),
            TextMovement::LineEnd => selection.line_end(&self.layout, false),
            TextMovement::ParagraphStart
            | TextMovement::ParagraphEnd
            | TextMovement::DocumentStart
            | TextMovement::DocumentEnd => {
                return Err(TextInteractionError::InvalidTextPosition);
            }
        };
        Ok(text_position_from_parley(moved.focus()))
    }
}

fn parley_cursor(
    layout: &Layout<()>,
    position: TextPosition,
) -> Result<Cursor, TextInteractionError> {
    let cursor = Cursor::from_byte_index(
        layout,
        position.byte_offset,
        match position.affinity {
            TextAffinity::Upstream => ParleyAffinity::Upstream,
            TextAffinity::Downstream => ParleyAffinity::Downstream,
        },
    );
    if cursor.index() == position.byte_offset {
        Ok(cursor)
    } else {
        Err(TextInteractionError::InvalidTextPosition)
    }
}

fn text_position_from_parley(cursor: Cursor) -> TextPosition {
    TextPosition::new(
        cursor.index(),
        match cursor.affinity() {
            ParleyAffinity::Upstream => TextAffinity::Upstream,
            ParleyAffinity::Downstream => TextAffinity::Downstream,
        },
    )
}

pub trait TextLayoutEngine {
    fn revision(&self) -> u64;

    /// Metrics of the resolved font instance, independent of text advance.
    fn font_metrics(
        &mut self,
        _font: &FontDescriptor,
    ) -> Result<crate::text::FontMetrics, TextError> {
        Err(TextError::FontMetricsUnsupported)
    }

    fn register_font(
        &mut self,
        resource_id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError>;

    fn remove_font(&mut self, resource_id: ResourceId);

    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError>;
}

pub struct ParleyTextEngine {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    registered_fonts: BTreeMap<ResourceId, RegisteredFont>,
    registered_faces: Vec<RegisteredFace>,
    resolved_faces: HashMap<(u64, u32), FontFaceRef>,
    revision: u64,
}

#[derive(Clone)]
struct RegisteredFont {
    revision: u64,
    bytes: Arc<Vec<u8>>,
}

struct RegisteredFace {
    face: FontFaceRef,
    bytes: Arc<Vec<u8>>,
}

impl Default for ParleyTextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ParleyTextEngine {
    pub fn new() -> Self {
        Self {
            font_context: empty_font_context(),
            layout_context: LayoutContext::new(),
            registered_fonts: BTreeMap::new(),
            registered_faces: Vec::new(),
            resolved_faces: HashMap::new(),
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        TextLayoutEngine::revision(self)
    }

    pub fn register_font(
        &mut self,
        resource_id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        TextLayoutEngine::register_font(self, resource_id, revision, bytes)
    }

    pub fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        TextLayoutEngine::layout(self, text, font, constraints)
    }
}

impl TextLayoutEngine for ParleyTextEngine {
    fn revision(&self) -> u64 {
        self.revision
    }

    fn font_metrics(
        &mut self,
        font: &FontDescriptor,
    ) -> Result<crate::text::FontMetrics, TextError> {
        use skrifa::{
            FontRef, MetadataProvider,
            instance::{NormalizedCoord, Size},
        };
        // Resolve through the same explicit collection and synthesis policy as layout.
        // A space selects the primary face without using its advance as a metric.
        let layout = self.layout(" ", font, TextConstraints::UNBOUNDED)?;
        let run = layout.glyph_runs().first().ok_or(TextError::NoUsableFont)?;
        let registered = self
            .registered_fonts
            .get(&run.font.resource_id())
            .ok_or(TextError::NoUsableFont)?;
        let face = FontRef::from_index(&registered.bytes, run.font.collection_index())
            .map_err(|_| TextError::InvalidFontData)?;
        let coords: Vec<_> = run
            .normalized_coords
            .iter()
            .map(|value| NormalizedCoord::from_bits(*value))
            .collect();
        let metrics = face.metrics(Size::new(run.font_size), coords.as_slice());
        crate::text::FontMetrics::new(
            run.font.clone(),
            run.font_size,
            run.normalized_coords.clone(),
            metrics.ascent,
            -metrics.descent,
            metrics.leading,
        )
    }

    fn register_font(
        &mut self,
        resource_id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        if !font_data_is_valid(bytes) {
            return Err(TextError::InvalidFontData);
        }
        self.registered_fonts.insert(
            resource_id,
            RegisteredFont {
                revision,
                bytes: Arc::new(bytes.to_vec()),
            },
        );
        self.rebuild_font_context();
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    fn remove_font(&mut self, resource_id: ResourceId) {
        if self.registered_fonts.remove(&resource_id).is_some() {
            self.rebuild_font_context();
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        font.validate()?;
        TextConstraints::new(constraints.max_width)?;

        FontFamily::parse(&font.family)
            .ok_or_else(|| TextError::InvalidFontDescriptor(font.family.clone()))?;
        let fallback_stack = self.fallback_stack(&font.family);
        let fallback_families = FontFamily::parse_list(&fallback_stack).collect::<Vec<_>>();
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(fallback_families.as_slice());
        builder.push_default(StyleProperty::FontSize(font.size));
        builder.push_default(StyleProperty::FontWeight(parley::FontWeight::new(
            font.weight,
        )));
        builder.push_default(StyleProperty::FontStyle(match font.style {
            FontStyle::Normal => parley::FontStyle::Normal,
            FontStyle::Italic => parley::FontStyle::Italic,
            FontStyle::Oblique => parley::FontStyle::Oblique(None),
        }));

        let mut layout: Layout<()> = builder.build(text);
        layout.break_all_lines(constraints.max_width);

        let lines = layout
            .lines()
            .map(|line| {
                let metrics = line.metrics();
                TextLineMetrics {
                    ascent: metrics.ascent,
                    descent: metrics.descent,
                    leading: metrics.leading,
                    baseline: metrics.baseline,
                    advance: metrics.advance,
                }
            })
            .collect();
        let mut glyph_runs = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let font_key = (font.data.id(), font.index);
                let font_face = if let Some(face) = self.resolved_faces.get(&font_key) {
                    face.clone()
                } else {
                    let face = self
                        .registered_faces
                        .iter()
                        .find(|registered| {
                            registered.face.collection_index() == font.index
                                && registered.bytes.as_slice() == font.data.data()
                        })
                        .map(|registered| registered.face.clone())
                        .ok_or(TextError::NoUsableFont)?;
                    self.resolved_faces.insert(font_key, face.clone());
                    face
                };
                let mut x = glyph_run.offset();
                let baseline = glyph_run.baseline();
                let glyphs = glyph_run
                    .glyphs()
                    .map(|glyph| {
                        let positioned = PositionedGlyph {
                            id: glyph.id,
                            x: x + glyph.x,
                            y: baseline - glyph.y,
                        };
                        x += glyph.advance;
                        positioned
                    })
                    .collect();
                glyph_runs.push(GlyphRun {
                    font: font_face,
                    font_size: run.font_size(),
                    normalized_coords: run.normalized_coords().to_vec(),
                    skew_degrees: run.synthesis().skew(),
                    glyphs,
                });
            }
        }
        if !text.is_empty() && glyph_runs.is_empty() {
            return Err(TextError::NoUsableFont);
        }

        let width = layout.width();
        let full_width = layout.full_width();
        let height = layout.height();
        let interaction = TextInteractionMap::new(
            text,
            Arc::new(ParleyTextInteraction {
                layout,
                text_len: text.len(),
            }),
        )
        .map_err(|_| TextError::InvalidInteractionMap)?;

        TextLayout::from_parts(TextLayoutParts {
            text: text.to_owned(),
            font: font.clone(),
            constraints,
            engine_revision: self.revision,
            width,
            full_width,
            height,
            lines,
            glyph_runs,
            visible_range: 0..text.len(),
            truncated: false,
        })
        .and_then(|layout| layout.with_interaction_map(interaction))
    }
}

pub type TextEngine = ParleyTextEngine;

impl ParleyTextEngine {
    fn fallback_stack(&mut self, preferred_family: &str) -> String {
        let mut families = vec![preferred_family.to_string()];
        for family in self.font_context.collection.family_names() {
            if !families.iter().any(|candidate| candidate == family) {
                families.push(family.to_string());
            }
        }
        families.join(", ")
    }

    fn rebuild_font_context(&mut self) {
        let mut font_context = empty_font_context();
        let mut registered_faces = Vec::new();
        for (resource_id, registered_font) in &self.registered_fonts {
            let font_bytes: Arc<dyn AsRef<[u8]> + Send + Sync> =
                Arc::clone(&registered_font.bytes) as Arc<dyn AsRef<[u8]> + Send + Sync>;
            let registered = font_context
                .collection
                .register_fonts(parley::fontique::Blob::new(font_bytes), None);
            for (_, fonts) in registered {
                registered_faces.extend(fonts.into_iter().map(|font| RegisteredFace {
                    face: FontFaceRef::new(*resource_id, registered_font.revision, font.index()),
                    bytes: Arc::clone(&registered_font.bytes),
                }));
            }
        }
        self.font_context = font_context;
        self.registered_faces = registered_faces;
        self.resolved_faces.clear();
    }
}

fn empty_font_context() -> FontContext {
    FontContext {
        collection: parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: parley::fontique::SourceCache::default(),
    }
}

fn font_data_is_valid(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let mut collection = parley::fontique::Collection::new(parley::fontique::CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    !collection
        .register_fonts(parley::fontique::Blob::new(Arc::new(bytes.to_vec())), None)
        .is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::geometry::Point;
    use uuid::Uuid;

    fn engine_with_builtins() -> TextEngine {
        let mut engine = TextEngine::new();
        for (index, font) in BuiltinFont::ALL.into_iter().enumerate() {
            engine
                .register_font(
                    ResourceId::new(Uuid::nil(), index as u64 + 1),
                    1,
                    font.bytes(),
                )
                .unwrap();
        }
        engine
    }

    #[test]
    fn parses_existing_figure_style_font_descriptor() {
        assert_eq!(
            FontDescriptor::parse("italic bold 18px serif").unwrap(),
            FontDescriptor {
                family: "serif".to_string(),
                size: 18.0,
                weight: 700.0,
                style: FontStyle::Italic,
            }
        );
        assert!(FontDescriptor::parse("sans-serif").is_err());
        assert!(FontDescriptor::new("sans-serif", 0.0).is_err());
    }

    #[test]
    fn invalid_font_data_does_not_advance_revision() {
        let mut engine = TextEngine::new();
        let id = ResourceId::new(Uuid::nil(), 1);

        assert_eq!(
            engine.register_font(id, 1, &[]),
            Err(TextError::InvalidFontData)
        );
        assert_eq!(
            engine.register_font(id, 1, &[1, 2, 3]),
            Err(TextError::InvalidFontData)
        );
        assert_eq!(engine.revision(), 0);
    }

    #[test]
    fn invalid_replacement_preserves_the_registered_font() {
        let mut engine = TextEngine::new();
        let id = ResourceId::new(Uuid::nil(), 1);
        engine
            .register_font(id, 1, BuiltinFont::Inter.bytes())
            .unwrap();

        assert_eq!(
            engine.register_font(id, 2, &[1, 2, 3]),
            Err(TextError::InvalidFontData)
        );
        let layout = engine
            .layout(
                "text",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();

        assert_eq!(engine.revision(), 1);
        assert!(
            layout
                .glyph_runs()
                .iter()
                .all(|run| run.font.resource_id() == id && run.font.revision() == 1)
        );
    }

    #[test]
    fn replacing_and_removing_a_font_rebuilds_the_font_context() {
        let mut engine = TextEngine::new();
        let id = ResourceId::new(Uuid::nil(), 1);
        engine
            .register_font(id, 1, BuiltinFont::Inter.bytes())
            .unwrap();
        engine
            .register_font(id, 2, BuiltinFont::JetBrainsMono.bytes())
            .unwrap();

        let layout = engine
            .layout(
                "text",
                &FontDescriptor::new(BuiltinFont::JetBrainsMono.family(), 12.0).unwrap(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        assert!(
            layout
                .glyph_runs()
                .iter()
                .all(|run| run.font.resource_id() == id && run.font.revision() == 2)
        );

        engine.remove_font(id);
        assert_eq!(
            engine.layout(
                "text",
                &FontDescriptor::new(BuiltinFont::JetBrainsMono.family(), 12.0).unwrap(),
                TextConstraints::UNBOUNDED,
            ),
            Err(TextError::NoUsableFont)
        );
    }

    #[test]
    fn parley_layout_produces_real_metrics_and_glyphs() {
        let mut engine = engine_with_builtins();
        let font = FontDescriptor::default();
        let layout = engine
            .layout("Wide ii", &font, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(layout.width() > 0.0);
        assert!(layout.height() > 0.0);
        assert_eq!(layout.line_metrics().len(), 1);
        assert!(!layout.glyph_runs().is_empty());
        assert!(!layout.is_empty());
    }

    #[test]
    fn bundled_fonts_cover_ui_cjk_and_monospace_roles() {
        let mut engine = engine_with_builtins();
        let cases = [
            ("Interface", BuiltinFont::Inter),
            ("绘图引擎", BuiltinFont::NotoSansSc),
            ("fn main()", BuiltinFont::JetBrainsMono),
        ];

        for (text, family) in cases {
            let layout = engine
                .layout(
                    text,
                    &FontDescriptor::new(family.family(), 14.0).unwrap(),
                    TextConstraints::UNBOUNDED,
                )
                .unwrap();
            assert!(
                !layout.is_empty(),
                "{} should produce glyphs",
                family.family()
            );
        }
    }

    #[test]
    fn registered_cjk_font_falls_back_when_the_requested_font_lacks_glyphs() {
        let mut engine = engine_with_builtins();
        let layout = engine
            .layout(
                "中文",
                &FontDescriptor::new(BuiltinFont::Inter.family(), 14.0).unwrap(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();

        assert!(
            layout
                .glyph_runs()
                .iter()
                .any(|run| { run.font.resource_id() == ResourceId::new(Uuid::nil(), 2) })
        );
    }

    #[test]
    fn proportional_font_measurement_is_not_character_count_estimation() {
        let mut engine = engine_with_builtins();
        let font = FontDescriptor::default();
        let wide = engine
            .layout("WWWW", &font, TextConstraints::UNBOUNDED)
            .unwrap();
        let narrow = engine
            .layout("iiii", &font, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(wide.width() > narrow.width());
    }

    #[test]
    fn width_constraint_breaks_text_into_multiple_lines() {
        let mut engine = engine_with_builtins();
        let font = FontDescriptor::default();
        let unbounded = engine
            .layout("alpha beta gamma", &font, TextConstraints::UNBOUNDED)
            .unwrap();
        let constrained = engine
            .layout(
                "alpha beta gamma",
                &font,
                TextConstraints::new(Some(unbounded.width() / 2.0)).unwrap(),
            )
            .unwrap();

        assert!(constrained.line_metrics().len() > 1);
        assert!(constrained.height() > unbounded.height());
    }

    #[test]
    fn canvas_records_positioned_glyph_runs_with_scoped_alpha() {
        let mut engine = engine_with_builtins();
        let layout = engine
            .layout(
                "glyphs",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let mut canvas = crate::NdCanvas::new();
        canvas.fill_style(Color::rgba(0.2, 0.4, 0.6, 0.8));
        canvas.global_alpha(0.5);
        canvas.fill_text_layout(&layout, 10.0, 20.0);

        let commands = canvas.commands();
        assert!(commands.len() > 1);
        let crate::render::RenderCommandKind::DrawGlyphRun { paint, run, origin } =
            &commands[1].kind
        else {
            panic!("expected glyph run");
        };
        assert_eq!(*origin, Point::new(10.0, 20.0));
        assert_eq!(
            *paint,
            GlyphPaint::Fill(Color::rgba(0.2, 0.4, 0.6, 0.4).into())
        );
        assert!(!run.glyphs.is_empty());
    }

    #[test]
    fn external_layout_parts_reject_invalid_metrics_and_utf8_ranges() {
        let mut invalid_metric = valid_layout_parts("text");
        invalid_metric.width = f32::NAN;
        assert_eq!(
            TextLayout::from_parts(invalid_metric),
            Err(TextError::InvalidMetric)
        );

        let mut invalid_range = valid_layout_parts("é");
        invalid_range.visible_range = 0..1;
        invalid_range.truncated = true;
        assert_eq!(
            TextLayout::from_parts(invalid_range),
            Err(TextError::InvalidVisibleRange)
        );

        let mut invalid_run = valid_layout_parts("text");
        invalid_run.glyph_runs[0].font_size = 0.0;
        assert_eq!(
            TextLayout::from_parts(invalid_run),
            Err(TextError::InvalidGlyphRun)
        );
    }

    fn valid_layout_parts(text: &str) -> TextLayoutParts {
        TextLayoutParts {
            text: text.to_owned(),
            font: FontDescriptor::default(),
            constraints: TextConstraints::UNBOUNDED,
            engine_revision: 1,
            width: 10.0,
            full_width: 10.0,
            height: 12.0,
            lines: vec![TextLineMetrics {
                ascent: 8.0,
                descent: 2.0,
                leading: 2.0,
                baseline: 8.0,
                advance: 10.0,
            }],
            glyph_runs: vec![GlyphRun {
                font: FontFaceRef::new(ResourceId::new(Uuid::nil(), 1), 1, 0),
                font_size: 12.0,
                normalized_coords: Vec::new(),
                skew_degrees: None,
                glyphs: vec![PositionedGlyph {
                    id: 1,
                    x: 0.0,
                    y: 8.0,
                }],
            }],
            visible_range: 0..text.len(),
            truncated: false,
        }
    }
}
