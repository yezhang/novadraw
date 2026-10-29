use crate::{Color, graphics::Opacity, text::FontDescriptor};

/// Container layout context, independent of paragraph direction and affine transforms.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LayoutDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl LayoutDirection {
    pub const fn is_mirrored(self) -> bool {
        matches!(self, Self::RightToLeft)
    }
}

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
    pub layout_direction: Option<LayoutDirection>,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub alpha: Option<Opacity>,
    pub font: Option<FontDescriptor>,
    pub cursor: Option<CursorIcon>,
    /// None inherits; Some(None) suppresses; Some(Some(text)) sets local content.
    pub tooltip: Option<Option<String>>,
}

/// Fully resolved style after applying ancestor inheritance and root defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub layout_direction: LayoutDirection,
    pub foreground: Color,
    pub background: Color,
    pub alpha: Opacity,
    pub font: FontDescriptor,
    pub cursor: CursorIcon,
    pub tooltip: Option<String>,
}

impl Default for ResolvedStyle {
    fn default() -> Self {
        Self {
            layout_direction: LayoutDirection::default(),
            foreground: Color::BLACK,
            background: Color::TRANSPARENT,
            alpha: Opacity::OPAQUE,
            font: FontDescriptor::default(),
            cursor: CursorIcon::Default,
            tooltip: None,
        }
    }
}

impl ResolvedStyle {
    pub(crate) fn apply_override(&mut self, style: &FigureStyle) {
        if let Some(direction) = style.layout_direction {
            self.layout_direction = direction;
        }
        if let Some(foreground) = style.foreground {
            self.foreground = foreground;
        }
        if let Some(background) = style.background {
            self.background = background;
        }
        if let Some(alpha) = style.alpha {
            self.alpha = alpha;
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
