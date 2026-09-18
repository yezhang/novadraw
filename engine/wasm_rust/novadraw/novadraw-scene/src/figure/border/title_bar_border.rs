use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::{
    FontDescriptor, NdCanvas, TextConstraints, TextError, TextLayout, TextLayoutEngine,
};

use crate::Alignment;

use super::{Border, BorderSnapshot, inset_rectangle};

const DEFAULT_HORIZONTAL_PADDING: f64 = 8.0;
const DEFAULT_VERTICAL_PADDING: f64 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TitleBarMetrics {
    pub(crate) layout: TextLayout,
    pub(crate) insets: (f64, f64, f64, f64),
    pub(crate) preferred: (f64, f64),
}

/// Immutable TitleBarBorder configuration.
#[derive(Debug)]
pub struct TitleBarBorder {
    title: String,
    background: Color,
    alignment: Alignment,
    horizontal_padding: f64,
    vertical_padding: f64,
}

impl TitleBarBorder {
    pub fn new(title: impl Into<String>, background: Color) -> Self {
        Self {
            title: title.into(),
            background,
            alignment: Alignment::Center,
            horizontal_padding: DEFAULT_HORIZONTAL_PADDING,
            vertical_padding: DEFAULT_VERTICAL_PADDING,
        }
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_padding(mut self, horizontal: f64, vertical: f64) -> Self {
        assert!(
            horizontal.is_finite() && horizontal >= 0.0 && vertical.is_finite() && vertical >= 0.0,
            "title bar padding must be finite and non-negative"
        );
        self.horizontal_padding = horizontal;
        self.vertical_padding = vertical;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn measure(&self, layout: TextLayout) -> TitleBarMetrics {
        let preferred = (
            layout.width() as f64 + self.horizontal_padding * 2.0,
            layout.height() as f64 + self.vertical_padding * 2.0,
        );
        TitleBarMetrics {
            layout,
            insets: (preferred.1, 0.0, 0.0, 0.0),
            preferred,
        }
    }

    pub(crate) fn paint_metrics(
        &self,
        figure_bounds: Rectangle,
        metrics: &TitleBarMetrics,
        gc: &mut NdCanvas,
    ) {
        let layout = &metrics.layout;
        let height = metrics.insets.0.min(figure_bounds.height);
        gc.push_state();
        gc.set_background_color(self.background);
        gc.fill_rectangle(
            figure_bounds.x,
            figure_bounds.y,
            figure_bounds.width,
            height,
        );
        gc.pop_state();

        let available = (figure_bounds.width - self.horizontal_padding * 2.0).max(0.0);
        let x = match self.alignment {
            Alignment::Start => figure_bounds.x + self.horizontal_padding,
            Alignment::Center => {
                figure_bounds.x
                    + self.horizontal_padding
                    + (available - layout.width() as f64).max(0.0) / 2.0
            }
            Alignment::End => {
                figure_bounds.x + figure_bounds.width
                    - self.horizontal_padding
                    - layout.width() as f64
            }
        };
        gc.draw_text_layout(layout, x, figure_bounds.y + self.vertical_padding);
    }
}

impl Border for TitleBarBorder {
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0)
    }

    fn paint(&self, _figure_bounds: Rectangle, _gc: &mut NdCanvas) {}

    fn paint_snapshot_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: (f64, f64, f64, f64),
        snapshot: &BorderSnapshot,
        gc: &mut NdCanvas,
    ) {
        if let Some(metrics) = snapshot.title_bar_metrics() {
            self.paint_metrics(inset_rectangle(figure_bounds, incoming), metrics, gc);
        }
    }

    fn resolve_owner_snapshot(
        &self,
        previous: Option<&BorderSnapshot>,
        font: &FontDescriptor,
        text: &mut dyn TextLayoutEngine,
    ) -> Result<Option<BorderSnapshot>, TextError> {
        let cached = previous
            .and_then(BorderSnapshot::title_bar_metrics)
            .map(|metrics| &metrics.layout)
            .filter(|layout| {
                layout.key().text() == self.title
                    && layout.key().font() == font
                    && layout.key().constraints() == TextConstraints::UNBOUNDED
                    && layout.key().engine_revision() == text.revision()
            });
        let layout = match cached {
            Some(layout) => layout.clone(),
            None => text.layout(&self.title, font, TextConstraints::UNBOUNDED)?,
        };
        Ok(Some(BorderSnapshot::title_bar(self.measure(layout))))
    }

    fn has_owner_snapshot(&self) -> bool {
        true
    }

    fn preferred_size(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn is_opaque(&self) -> bool {
        self.background.is_opaque()
    }

    fn get_color(&self) -> Color {
        self.background
    }

    fn title_bar(&self) -> Option<&TitleBarBorder> {
        Some(self)
    }
}
