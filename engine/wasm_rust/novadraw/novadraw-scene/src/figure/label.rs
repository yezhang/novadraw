use std::sync::Arc;

use novadraw_geometry::Rectangle;
use novadraw_render::{
    FontDescriptor, ImageResourceRef, NdCanvas, TextConstraints, TextError, TextLayout,
    TextLayoutEngine,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::ImageId;

use super::border::Border;
use super::{BorderedFigure, Bounded, Figure};

const DEFAULT_ICON_TEXT_GAP: f64 = 4.0;
const ELLIPSIS: &str = "...";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Alignment {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextPlacement {
    #[default]
    East,
    West,
    North,
    South,
}

#[derive(Clone, Debug, PartialEq)]
struct LabelLayout {
    key: LabelLayoutKey,
    text: TextLayout,
    text_origin: (f64, f64),
    icon_origin: Option<(f64, f64)>,
    preferred: (f64, f64),
    minimum: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
struct LabelLayoutKey {
    text_revision: u64,
    font: FontDescriptor,
    engine_revision: u64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    icon: Option<ImageResourceRef>,
    placement: TextPlacement,
    gap: f64,
}

/// A single-line text and optional image label.
///
/// The Runtime owns shaping and refreshes this Figure's immutable layout snapshot
/// before a frame is recorded. Paint only consumes that snapshot.
#[derive(Clone)]
pub struct LabelFigure {
    bounds: Rectangle,
    text: String,
    text_revision: u64,
    icon: Option<ImageId>,
    text_placement: TextPlacement,
    label_alignment: Alignment,
    text_alignment: Alignment,
    icon_alignment: Alignment,
    icon_text_gap: f64,
    border: Option<Arc<dyn Border>>,
    icon_ref: Option<ImageResourceRef>,
    layout: Option<LabelLayout>,
}

impl LabelFigure {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            text: text.into(),
            text_revision: 1,
            icon: None,
            text_placement: TextPlacement::East,
            label_alignment: Alignment::Center,
            text_alignment: Alignment::Center,
            icon_alignment: Alignment::Center,
            icon_text_gap: DEFAULT_ICON_TEXT_GAP,
            border: None,
            icon_ref: None,
            layout: None,
        }
    }

    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_icon(mut self, icon: ImageId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn icon(&self) -> Option<ImageId> {
        self.icon
    }

    pub fn text_placement(&self) -> TextPlacement {
        self.text_placement
    }

    pub fn label_alignment(&self) -> Alignment {
        self.label_alignment
    }

    pub fn text_alignment(&self) -> Alignment {
        self.text_alignment
    }

    pub fn icon_alignment(&self) -> Alignment {
        self.icon_alignment
    }

    pub fn icon_text_gap(&self) -> f64 {
        self.icon_text_gap
    }

    pub fn text_layout(&self) -> Option<&TextLayout> {
        self.layout.as_ref().map(|layout| &layout.text)
    }

    pub(crate) fn icon_bounds(&self) -> Option<Rectangle> {
        let origin = self.layout.as_ref()?.icon_origin?;
        let (width, height) = self.icon_ref?.logical_size();
        Some(Rectangle::new(origin.0, origin.1, width, height))
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.text_revision = self.text_revision.wrapping_add(1);
        self.layout = None;
    }

    pub fn set_icon(&mut self, icon: Option<ImageId>) {
        self.icon = icon;
        self.layout = None;
    }

    pub fn set_text_placement(&mut self, placement: TextPlacement) {
        self.text_placement = placement;
        self.layout = None;
    }

    pub fn set_label_alignment(&mut self, alignment: Alignment) {
        self.label_alignment = alignment;
    }

    pub fn set_text_alignment(&mut self, alignment: Alignment) {
        self.text_alignment = alignment;
    }

    pub fn set_icon_alignment(&mut self, alignment: Alignment) {
        self.icon_alignment = alignment;
    }

    pub fn set_icon_text_gap(&mut self, gap: f64) {
        self.icon_text_gap = gap;
        self.layout = None;
    }

    pub(crate) fn refresh_layout(
        &mut self,
        engine: &mut dyn TextLayoutEngine,
        font: &novadraw_render::FontDescriptor,
        bounds: Rectangle,
        icon: Option<ImageResourceRef>,
    ) -> Result<bool, TextError> {
        let key = LabelLayoutKey {
            text_revision: self.text_revision,
            font: font.clone(),
            engine_revision: engine.revision(),
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            icon,
            placement: self.text_placement,
            gap: self.icon_text_gap,
        };
        if let Some(layout) = self.layout.as_mut()
            && layout.key == key
        {
            let icon_size = icon.map(ImageResourceRef::logical_size);
            let (text_origin, icon_origin) = positions(
                bounds,
                (layout.text.width() as f64, layout.text.height() as f64),
                icon_size,
                self.text_placement,
                self.label_alignment,
                self.text_alignment,
                self.icon_alignment,
                self.icon_text_gap,
            );
            let changed = layout.text_origin != text_origin || layout.icon_origin != icon_origin;
            layout.text_origin = text_origin;
            layout.icon_origin = icon_origin;
            self.icon_ref = icon;
            return Ok(changed);
        }
        let full = engine.layout(&self.text, font, TextConstraints::UNBOUNDED)?;
        let icon_size = icon.map(ImageResourceRef::logical_size);
        let preferred = combined_size(
            (full.width() as f64, full.height() as f64),
            icon_size,
            self.text_placement,
            self.icon_text_gap,
        );
        let ellipsis = engine.layout(ELLIPSIS, font, TextConstraints::UNBOUNDED)?;
        let minimum_text = (
            full.width().min(ellipsis.width()) as f64,
            ellipsis.height() as f64,
        );
        let minimum = combined_size(
            minimum_text,
            icon_size,
            self.text_placement,
            self.icon_text_gap,
        );

        let available =
            available_text_width(bounds, icon_size, self.text_placement, self.icon_text_gap);
        let text = if available.is_some_and(|width| width < full.width() as f64) {
            truncate(engine, &self.text, font, available.unwrap(), full.width())?
        } else {
            full
        };
        let text_size = (text.width() as f64, text.height() as f64);
        let (text_origin, icon_origin) = positions(
            bounds,
            text_size,
            icon_size,
            self.text_placement,
            self.label_alignment,
            self.text_alignment,
            self.icon_alignment,
            self.icon_text_gap,
        );
        let next = LabelLayout {
            key,
            text,
            text_origin,
            icon_origin,
            preferred,
            minimum,
        };
        let changed = self.layout.as_ref() != Some(&next) || self.icon_ref != icon;
        self.layout = Some(next);
        self.icon_ref = icon;
        Ok(changed)
    }

    pub(crate) fn preferred_size(&self) -> Option<(f64, f64)> {
        self.layout.as_ref().map(|layout| layout.preferred)
    }

    pub(crate) fn minimum_size(&self) -> Option<(f64, f64)> {
        self.layout.as_ref().map(|layout| layout.minimum)
    }

    pub(crate) fn paint_with_icon(&self, gc: &mut NdCanvas, icon: Option<ImageResourceRef>) {
        let Some(layout) = &self.layout else {
            return;
        };
        if let (Some(origin), Some(image)) = (layout.icon_origin, icon) {
            gc.draw_image(image, origin.0, origin.1);
        }
        gc.draw_text_layout(&layout.text, layout.text_origin.0, layout.text_origin.1);
    }
}

impl Bounded for LabelFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }
    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }
    fn name(&self) -> &'static str {
        "LabelFigure"
    }
    fn preferred_size(&self) -> (f64, f64) {
        with_border_size(
            self.preferred_size().unwrap_or_default(),
            self.border.as_deref(),
        )
    }
    fn minimum_size(&self) -> (f64, f64) {
        with_border_size(
            self.minimum_size().unwrap_or_default(),
            self.border.as_deref(),
        )
    }
}

impl Figure for LabelFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "LabelFigure"
    }
    fn intrinsic_size(&self) -> (f64, f64) {
        with_border_size(
            self.preferred_size().unwrap_or_default(),
            self.border.as_deref(),
        )
    }
    fn intrinsic_minimum_size(&self) -> (f64, f64) {
        with_border_size(
            self.minimum_size().unwrap_or_default(),
            self.border.as_deref(),
        )
    }
    fn paint_figure(&self, gc: &mut NdCanvas) {
        self.paint_with_icon(gc, self.icon_ref);
    }
    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, _bounds: Rectangle) {
        self.paint_with_icon(gc, self.icon_ref);
    }

    fn initial_insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or_default()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        Some(self)
    }
}

impl BorderedFigure for LabelFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

fn with_border_size(size: (f64, f64), border: Option<&dyn Border>) -> (f64, f64) {
    let Some(border) = border else {
        return size;
    };
    let (top, left, bottom, right) = border.get_insets();
    let preferred = border.preferred_size();
    (
        (size.0 + left + right).max(preferred.0),
        (size.1 + top + bottom).max(preferred.1),
    )
}

fn combined_size(
    text: (f64, f64),
    icon: Option<(f64, f64)>,
    placement: TextPlacement,
    gap: f64,
) -> (f64, f64) {
    let Some(icon) = icon else { return text };
    if text.0 == 0.0 && text.1 == 0.0 {
        return icon;
    }
    match placement {
        TextPlacement::East | TextPlacement::West => (text.0 + icon.0 + gap, text.1.max(icon.1)),
        TextPlacement::North | TextPlacement::South => (text.0.max(icon.0), text.1 + icon.1 + gap),
    }
}

fn available_text_width(
    bounds: Rectangle,
    icon: Option<(f64, f64)>,
    placement: TextPlacement,
    gap: f64,
) -> Option<f64> {
    match (icon, placement) {
        (Some((width, _)), TextPlacement::East | TextPlacement::West) => {
            Some((bounds.width - width - gap).max(0.0))
        }
        _ => Some(bounds.width.max(0.0)),
    }
}

fn truncate(
    engine: &mut dyn TextLayoutEngine,
    source: &str,
    font: &novadraw_render::FontDescriptor,
    width: f64,
    full_width: f32,
) -> Result<TextLayout, TextError> {
    let constraints = TextConstraints::new(Some(width as f32))?;
    let ellipsis = engine.layout(ELLIPSIS, font, TextConstraints::UNBOUNDED)?;
    if width < ellipsis.width() as f64 {
        return Ok(ellipsis.with_visibility(source, 0..0, true, full_width, constraints));
    }
    let graphemes = source.graphemes(true).collect::<Vec<_>>();
    let mut low = 0;
    let mut high = graphemes.len();
    while low < high {
        let middle = (low + high).div_ceil(2);
        let candidate = format!("{}{}", graphemes[..middle].concat(), ELLIPSIS);
        if engine
            .layout(&candidate, font, TextConstraints::UNBOUNDED)?
            .width() as f64
            <= width
        {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let visible = graphemes[..low].concat();
    engine
        .layout(
            &format!("{visible}{ELLIPSIS}"),
            font,
            TextConstraints::UNBOUNDED,
        )
        .map(|layout| {
            layout.with_visibility(source, 0..visible.len(), true, full_width, constraints)
        })
}

fn aligned(origin: f64, available: f64, used: f64, alignment: Alignment) -> f64 {
    match alignment {
        Alignment::Start => origin,
        Alignment::Center => origin + (available - used).max(0.0) / 2.0,
        Alignment::End => origin + (available - used).max(0.0),
    }
}

#[allow(clippy::too_many_arguments)]
fn positions(
    bounds: Rectangle,
    text: (f64, f64),
    icon: Option<(f64, f64)>,
    placement: TextPlacement,
    label_alignment: Alignment,
    text_alignment: Alignment,
    icon_alignment: Alignment,
    gap: f64,
) -> ((f64, f64), Option<(f64, f64)>) {
    let Some(icon) = icon else {
        return (
            (
                aligned(bounds.x, bounds.width, text.0, label_alignment),
                aligned(bounds.y, bounds.height, text.1, label_alignment),
            ),
            None,
        );
    };
    let total = combined_size(text, Some(icon), placement, gap);
    match placement {
        TextPlacement::East | TextPlacement::West => {
            let x = aligned(bounds.x, bounds.width, total.0, label_alignment);
            let text_x = if placement == TextPlacement::East {
                x + icon.0 + gap
            } else {
                x
            };
            let icon_x = if placement == TextPlacement::East {
                x
            } else {
                x + text.0 + gap
            };
            (
                (
                    text_x,
                    aligned(bounds.y, bounds.height, text.1, text_alignment),
                ),
                Some((
                    icon_x,
                    aligned(bounds.y, bounds.height, icon.1, icon_alignment),
                )),
            )
        }
        TextPlacement::North | TextPlacement::South => {
            let y = aligned(bounds.y, bounds.height, total.1, label_alignment);
            let text_y = if placement == TextPlacement::North {
                y + icon.1 + gap
            } else {
                y
            };
            let icon_y = if placement == TextPlacement::North {
                y
            } else {
                y + text.1 + gap
            };
            (
                (
                    aligned(bounds.x, bounds.width, text.0, text_alignment),
                    text_y,
                ),
                Some((
                    aligned(bounds.x, bounds.width, icon.0, icon_alignment),
                    icon_y,
                )),
            )
        }
    }
}
