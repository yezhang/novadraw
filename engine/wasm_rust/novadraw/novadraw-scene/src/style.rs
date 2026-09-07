use novadraw_core::Color;

pub const DEFAULT_FONT_DESCRIPTOR: &str = "12px Inter Variable";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Crosshair,
    Text,
    Move,
    NotAllowed,
    EastWestResize,
    NorthSouthResize,
    NorthEastSouthWestResize,
    NorthWestSouthEastResize,
}

/// Optional style values explicitly set on one Figure node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FigureStyle {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub alpha: Option<f64>,
    pub font: Option<String>,
    pub cursor: Option<CursorIcon>,
    /// None inherits; Some(None) suppresses; Some(Some(text)) sets local content.
    pub tooltip: Option<Option<String>>,
}

/// Fully resolved style after applying ancestor inheritance and root defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub foreground: Color,
    pub background: Color,
    pub alpha: f64,
    pub font: String,
    pub cursor: CursorIcon,
    pub tooltip: Option<String>,
}

impl Default for ResolvedStyle {
    fn default() -> Self {
        Self {
            foreground: Color::BLACK,
            background: Color::TRANSPARENT,
            alpha: 1.0,
            font: DEFAULT_FONT_DESCRIPTOR.to_string(),
            cursor: CursorIcon::Default,
            tooltip: None,
        }
    }
}

impl ResolvedStyle {
    pub(crate) fn apply_override(&mut self, style: &FigureStyle) {
        if let Some(foreground) = style.foreground {
            self.foreground = foreground;
        }
        if let Some(background) = style.background {
            self.background = background;
        }
        if let Some(alpha) = style.alpha {
            self.alpha = alpha.clamp(0.0, 1.0);
        }
        if let Some(font) = &style.font {
            self.font.clone_from(font);
        }
        if let Some(cursor) = style.cursor {
            self.cursor = cursor;
        }
        if let Some(tooltip) = &style.tooltip {
            self.tooltip.clone_from(tooltip);
        }
    }
}
