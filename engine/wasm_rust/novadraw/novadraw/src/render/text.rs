use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use parley::{FontContext, FontFamily, Layout, LayoutContext, PositionedLayoutItem, StyleProperty};

use crate::ResourceId;

const DEFAULT_FONT_FAMILY: &str = "Inter Variable";
const DEFAULT_FONT_SIZE: f32 = 12.0;
const DEFAULT_FONT_WEIGHT: f32 = 400.0;
const INTER_FONT: &[u8] = include_bytes!("../../../assets/fonts/InterVariable.ttf");
const NOTO_SANS_SC_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-VF.ttf");
const JETBRAINS_MONO_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinFont {
    Inter,
    NotoSansSc,
    JetBrainsMono,
}

impl BuiltinFont {
    pub const ALL: [Self; 3] = [Self::Inter, Self::NotoSansSc, Self::JetBrainsMono];

    pub const fn family(self) -> &'static str {
        match self {
            Self::Inter => "Inter Variable",
            Self::NotoSansSc => "Noto Sans SC",
            Self::JetBrainsMono => "JetBrains Mono",
        }
    }

    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Inter => INTER_FONT,
            Self::NotoSansSc => NOTO_SANS_SC_FONT,
            Self::JetBrainsMono => JETBRAINS_MONO_FONT,
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
    pub family: String,
    pub size: f32,
    pub weight: f32,
    pub style: FontStyle,
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlyphPaint {
    Fill(crate::Color),
    Stroke { color: crate::Color, width: f64 },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    width: f32,
    full_width: f32,
    height: f32,
    lines: Vec<TextLineMetrics>,
    glyph_runs: Vec<GlyphRun>,
    visible_range: Range<usize>,
    truncated: bool,
    key: TextLayoutKey,
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
        self.key.text = source;
        self.key.constraints = constraints;
        self.visible_range = visible_range;
        self.truncated = truncated;
        self.full_width = full_width;
        Ok(self)
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
    NoUsableFont,
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
            Self::NoUsableFont => formatter.write_str("no usable font was found for the text"),
        }
    }
}

impl Error for TextError {}

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

pub trait TextLayoutEngine {
    fn revision(&self) -> u64;

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

        TextLayout::from_parts(TextLayoutParts {
            text: text.to_owned(),
            font: font.clone(),
            constraints,
            engine_revision: self.revision,
            width: layout.width(),
            full_width: layout.full_width(),
            height: layout.height(),
            lines,
            glyph_runs,
            visible_range: 0..text.len(),
            truncated: false,
        })
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
        assert_eq!(*paint, GlyphPaint::Fill(Color::rgba(0.2, 0.4, 0.6, 0.4)));
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
