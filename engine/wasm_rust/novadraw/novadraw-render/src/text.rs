use std::error::Error;
use std::fmt;

use novadraw_core::Color;
use parley::{FontContext, FontFamily, Layout, LayoutContext, PositionedLayoutItem, StyleProperty};

const DEFAULT_FONT_FAMILY: &str = "sans-serif";
const DEFAULT_FONT_SIZE: f32 = 12.0;
const DEFAULT_FONT_WEIGHT: f32 = 400.0;

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextGlyphRun {
    pub font: parley::FontData,
    pub font_size: f32,
    pub normalized_coords: Vec<i16>,
    pub skew_degrees: Option<f32>,
    pub color: Color,
    pub glyphs: Vec<TextGlyph>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    width: f32,
    full_width: f32,
    height: f32,
    lines: Vec<TextLineMetrics>,
    glyph_runs: Vec<TextGlyphRun>,
}

impl TextLayout {
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

    pub fn glyph_runs(&self) -> &[TextGlyphRun] {
        &self.glyph_runs
    }

    pub fn is_empty(&self) -> bool {
        self.glyph_runs.iter().all(|run| run.glyphs.is_empty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextError {
    InvalidFontDescriptor(String),
    InvalidFontData,
    InvalidMetric,
    NoUsableFont,
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontDescriptor(descriptor) => {
                write!(formatter, "invalid font descriptor: {descriptor}")
            }
            Self::InvalidFontData => formatter.write_str("font data is empty or invalid"),
            Self::InvalidMetric => {
                formatter.write_str("text metric must be finite and non-negative")
            }
            Self::NoUsableFont => formatter.write_str("no usable font was found for the text"),
        }
    }
}

impl Error for TextError {}

pub struct TextEngine {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    revision: u64,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    pub fn new() -> Self {
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn register_font(&mut self, bytes: Vec<u8>) -> Result<(), TextError> {
        if bytes.is_empty() {
            return Err(TextError::InvalidFontData);
        }
        let registered = self
            .font_context
            .collection
            .register_fonts(bytes.into(), None);
        if registered.is_empty() {
            return Err(TextError::InvalidFontData);
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        color: Color,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        font.validate()?;
        TextConstraints::new(constraints.max_width)?;

        let family = FontFamily::parse(&font.family)
            .ok_or_else(|| TextError::InvalidFontDescriptor(font.family.clone()))?;
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(family);
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
                let mut x = glyph_run.offset();
                let baseline = glyph_run.baseline();
                let glyphs = glyph_run
                    .glyphs()
                    .map(|glyph| {
                        let positioned = TextGlyph {
                            id: glyph.id,
                            x: x + glyph.x,
                            y: baseline - glyph.y,
                        };
                        x += glyph.advance;
                        positioned
                    })
                    .collect();
                glyph_runs.push(TextGlyphRun {
                    font: run.font().clone(),
                    font_size: run.font_size(),
                    normalized_coords: run.normalized_coords().to_vec(),
                    skew_degrees: run.synthesis().skew(),
                    color,
                    glyphs,
                });
            }
        }
        if !text.is_empty() && glyph_runs.is_empty() {
            return Err(TextError::NoUsableFont);
        }

        Ok(TextLayout {
            width: layout.width(),
            full_width: layout.full_width(),
            height: layout.height(),
            lines,
            glyph_runs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(
            engine.register_font(Vec::new()),
            Err(TextError::InvalidFontData)
        );
        assert_eq!(
            engine.register_font(vec![1, 2, 3]),
            Err(TextError::InvalidFontData)
        );
        assert_eq!(engine.revision(), 0);
    }

    #[test]
    fn parley_layout_produces_real_metrics_and_glyphs() {
        let mut engine = TextEngine::new();
        let font = FontDescriptor::default();
        let layout = engine
            .layout("Wide ii", &font, Color::BLACK, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(layout.width() > 0.0);
        assert!(layout.height() > 0.0);
        assert_eq!(layout.line_metrics().len(), 1);
        assert!(!layout.glyph_runs().is_empty());
        assert!(!layout.is_empty());
    }

    #[test]
    fn proportional_font_measurement_is_not_character_count_estimation() {
        let mut engine = TextEngine::new();
        let font = FontDescriptor::default();
        let wide = engine
            .layout("WWWW", &font, Color::BLACK, TextConstraints::UNBOUNDED)
            .unwrap();
        let narrow = engine
            .layout("iiii", &font, Color::BLACK, TextConstraints::UNBOUNDED)
            .unwrap();

        assert!(wide.width() > narrow.width());
    }

    #[test]
    fn width_constraint_breaks_text_into_multiple_lines() {
        let mut engine = TextEngine::new();
        let font = FontDescriptor::default();
        let unbounded = engine
            .layout(
                "alpha beta gamma",
                &font,
                Color::BLACK,
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let constrained = engine
            .layout(
                "alpha beta gamma",
                &font,
                Color::BLACK,
                TextConstraints::new(Some(unbounded.width() / 2.0)).unwrap(),
            )
            .unwrap();

        assert!(constrained.line_metrics().len() > 1);
        assert!(constrained.height() > unbounded.height());
    }

    #[test]
    fn canvas_records_positioned_glyph_runs_with_scoped_alpha() {
        let mut engine = TextEngine::new();
        let layout = engine
            .layout(
                "glyphs",
                &FontDescriptor::default(),
                Color::rgba(0.2, 0.4, 0.6, 0.8),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let mut canvas = crate::NdCanvas::new();
        canvas.global_alpha(0.5);
        canvas.draw_text_layout(&layout, 10.0, 20.0);

        let commands = canvas.commands();
        assert!(commands.len() > 1);
        let crate::RenderCommandKind::GlyphRun { run, origin } = &commands[1].kind else {
            panic!("expected glyph run");
        };
        assert_eq!(*origin, glam::DVec2::new(10.0, 20.0));
        assert_eq!(run.color.a, 0.4);
        assert!(!run.glyphs.is_empty());
    }
}
