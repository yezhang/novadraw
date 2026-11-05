use std::sync::Arc;

use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};
use crate::render::{
    FontDescriptor, ImageResourceRef, TextConstraints, TextError, TextLayout, TextLayoutEngine,
};

use crate::ImageId;

use super::border::Border;
use super::{AccessibleFigure, BorderedFigure, Bounded, Figure};

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
struct LabelIntrinsicLayout {
    key: LabelIntrinsicKey,
    full_text: TextLayout,
    preferred: (f64, f64),
    minimum: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
struct LabelPresentation {
    key: LabelPresentationKey,
    text: TextLayout,
    text_origin: (f64, f64),
    icon_origin: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
struct LabelIntrinsicKey {
    text_revision: u64,
    font: FontDescriptor,
    engine_revision: u64,
    icon: Option<ImageResourceRef>,
    placement: TextPlacement,
    gap: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct LabelPresentationKey {
    intrinsic: LabelIntrinsicKey,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    label_alignment: Alignment,
    text_alignment: Alignment,
    icon_alignment: Alignment,
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
    intrinsic: Option<LabelIntrinsicLayout>,
    presentation: Option<LabelPresentation>,
    content_revision: u64,
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
            intrinsic: None,
            presentation: None,
            content_revision: 0,
        }
    }

    pub fn with_bounds(mut self, bounds: Rectangle) -> Result<Self, super::ShapeGeometryError> {
        self.bounds = super::ShapeGeometryError::validate_bounds(bounds)?;
        Ok(self)
    }

    pub fn with_icon(mut self, icon: ImageId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_border(
        mut self,
        border: impl Border + 'static,
    ) -> Result<Self, super::border::BorderInputError> {
        border.prepare_metrics()?;
        self.border = Some(Arc::new(border));
        Ok(self)
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
        self.presentation
            .as_ref()
            .map(|presentation| &presentation.text)
    }

    pub(crate) fn icon_bounds(&self) -> Option<Rectangle> {
        let origin = self.presentation.as_ref()?.icon_origin?;
        let (width, height) = self.icon_ref?.logical_size();
        Some(Rectangle::new(origin.0, origin.1, width, height))
    }

    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.text_revision = self.text_revision.wrapping_add(1);
        self.intrinsic = None;
        self.presentation = None;
    }

    pub fn set_icon(&mut self, icon: Option<ImageId>) {
        if self.icon == icon {
            return;
        }
        self.icon = icon;
        self.intrinsic = None;
        self.presentation = None;
    }

    pub fn set_text_placement(&mut self, placement: TextPlacement) {
        if self.text_placement == placement {
            return;
        }
        self.text_placement = placement;
        self.intrinsic = None;
        self.presentation = None;
    }

    pub fn set_label_alignment(&mut self, alignment: Alignment) {
        if self.label_alignment == alignment {
            return;
        }
        self.label_alignment = alignment;
        self.presentation = None;
    }

    pub fn set_text_alignment(&mut self, alignment: Alignment) {
        if self.text_alignment == alignment {
            return;
        }
        self.text_alignment = alignment;
        self.presentation = None;
    }

    pub fn set_icon_alignment(&mut self, alignment: Alignment) {
        if self.icon_alignment == alignment {
            return;
        }
        self.icon_alignment = alignment;
        self.presentation = None;
    }

    pub fn set_icon_text_gap(&mut self, gap: f64) -> Result<(), super::ShapeGeometryError> {
        if !gap.is_finite() {
            return Err(super::ShapeGeometryError::NonFinite);
        }
        if gap < 0.0 {
            return Err(super::ShapeGeometryError::NegativeMetric);
        }
        if self.icon_text_gap == gap {
            return Ok(());
        }
        self.icon_text_gap = gap;
        self.intrinsic = None;
        self.presentation = None;
        Ok(())
    }

    pub(crate) fn refresh_intrinsic(
        &mut self,
        engine: &mut dyn TextLayoutEngine,
        font: &crate::render::FontDescriptor,
        icon: Option<ImageResourceRef>,
    ) -> Result<bool, TextError> {
        let key = LabelIntrinsicKey {
            text_revision: self.text_revision,
            font: font.clone(),
            engine_revision: engine.revision(),
            icon,
            placement: self.text_placement,
            gap: self.icon_text_gap,
        };
        if self
            .intrinsic
            .as_ref()
            .is_some_and(|intrinsic| intrinsic.key == key)
        {
            return Ok(false);
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
        let next = LabelIntrinsicLayout {
            key,
            full_text: full,
            preferred,
            minimum,
        };
        let changed = self.intrinsic.as_ref() != Some(&next);
        self.intrinsic = Some(next);
        self.presentation = None;
        self.icon_ref = icon;
        if changed {
            self.content_revision = self.content_revision.wrapping_add(1);
        }
        Ok(changed)
    }

    pub(crate) fn refresh_presentation(
        &mut self,
        engine: &mut dyn TextLayoutEngine,
        bounds: Rectangle,
    ) -> Result<bool, TextError> {
        let Some(intrinsic) = self.intrinsic.as_ref() else {
            return Ok(false);
        };
        let key = LabelPresentationKey {
            intrinsic: intrinsic.key.clone(),
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            label_alignment: self.label_alignment,
            text_alignment: self.text_alignment,
            icon_alignment: self.icon_alignment,
        };
        if self
            .presentation
            .as_ref()
            .is_some_and(|presentation| presentation.key == key)
        {
            return Ok(false);
        }
        let icon_size = self.icon_ref.map(ImageResourceRef::logical_size);
        let available =
            available_text_width(bounds, icon_size, self.text_placement, self.icon_text_gap);
        let text = if available.is_some_and(|width| width < intrinsic.full_text.width() as f64) {
            truncate(
                engine,
                &self.text,
                &intrinsic.key.font,
                available.unwrap(),
                intrinsic.full_text.width(),
            )?
        } else {
            intrinsic.full_text.clone()
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
        let next = LabelPresentation {
            key,
            text,
            text_origin,
            icon_origin,
        };
        let changed = self.presentation.as_ref() != Some(&next);
        self.presentation = Some(next);
        Ok(changed)
    }

    pub(crate) fn preferred_size(&self) -> Option<(f64, f64)> {
        self.intrinsic.as_ref().map(|intrinsic| intrinsic.preferred)
    }

    pub(crate) fn minimum_size(&self) -> Option<(f64, f64)> {
        self.intrinsic.as_ref().map(|intrinsic| intrinsic.minimum)
    }

    pub(crate) fn paint_with_icon(
        &self,
        context: &mut PaintContext<'_>,
        icon: Option<ImageResourceRef>,
    ) -> Result<(), GraphicsError> {
        paint_presentation(self.presentation.as_ref(), icon, context)
    }
}

fn paint_presentation(
    presentation: Option<&LabelPresentation>,
    icon: Option<ImageResourceRef>,
    context: &mut PaintContext<'_>,
) -> Result<(), GraphicsError> {
    let Some(presentation) = presentation else {
        return Ok(());
    };
    if let (Some(origin), Some(image)) = (presentation.icon_origin, icon) {
        let (width, height) = image.logical_size();
        context.draw_image(image, Rectangle::new(origin.0, origin.1, width, height))?;
    }
    if let Some(foreground) = context.stroke_paint().cloned() {
        context.push_state();
        context.set_fill_paint(foreground);
        let paint_result = context.fill_text(
            &presentation.text,
            crate::Point::new(presentation.text_origin.0, presentation.text_origin.1),
        );
        let pop_result = context.pop_state();
        paint_result.and(pop_result)?;
    }
    Ok(())
}

struct LabelContentDrawing {
    presentation: Option<LabelPresentation>,
    icon: Option<ImageResourceRef>,
}

impl super::FigureDrawing for LabelContentDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        paint_presentation(self.presentation.as_ref(), self.icon, context)
    }
}

impl super::FigureContentSnapshot for LabelFigure {
    fn content_revision(&self) -> u64 {
        self.content_revision
    }

    fn snapshot_content(&self, bounds: Rectangle) -> Result<super::FigurePresentation, TextError> {
        let measurement = self.presentation.as_ref().map_or(
            super::FigureMeasurement::new(0.0, 0.0, None),
            |presentation| {
                super::FigureMeasurement::new(
                    f64::from(presentation.text.width()),
                    f64::from(presentation.text.height()),
                    None,
                )
            },
        );
        super::FigurePresentation::new(
            measurement,
            self.intrinsic_content_minimum_size(),
            self.visual_bounds_in(bounds),
            Arc::new(LabelContentDrawing {
                presentation: self.presentation.clone(),
                icon: self.icon_ref,
            }),
        )
    }
}

impl Bounded for LabelFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }
    fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), super::ShapeGeometryError> {
        self.bounds =
            super::ShapeGeometryError::validate_bounds(Rectangle::new(x, y, width, height))?;
        Ok(())
    }
    fn name(&self) -> &'static str {
        "LabelFigure"
    }
    fn preferred_size(&self) -> Dimension {
        with_border_size(
            self.preferred_size().unwrap_or_default().into(),
            self.border.as_deref(),
        )
    }
    fn minimum_size(&self) -> Dimension {
        with_border_size(
            self.minimum_size().unwrap_or_default().into(),
            self.border.as_deref(),
        )
    }
}

impl super::FigureResourceDependencies for LabelFigure {
    fn resource_dependencies(&self) -> Vec<crate::render::ResourceId> {
        self.icon
            .into_iter()
            .map(|icon| icon.resource_id())
            .collect()
    }
}

impl Figure for LabelFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "LabelFigure"
    }
    fn intrinsic_size(&self) -> Dimension {
        with_border_size(
            self.preferred_size().unwrap_or_default().into(),
            self.border.as_deref(),
        )
    }
    fn intrinsic_content_size(&self) -> Dimension {
        self.preferred_size().unwrap_or_default().into()
    }
    fn intrinsic_minimum_size(&self) -> Dimension {
        with_border_size(
            self.minimum_size().unwrap_or_default().into(),
            self.border.as_deref(),
        )
    }
    fn intrinsic_content_minimum_size(&self) -> Dimension {
        self.minimum_size().unwrap_or_default().into()
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        _bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.paint_with_icon(context, self.icon_ref)
    }

    fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
        let base = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        self.presentation
            .as_ref()
            .and_then(|p| {
                p.text.ink_bounds().map(|ink| {
                    Rectangle::new(
                        ink.x + p.text_origin.0,
                        ink.y + p.text_origin.1,
                        ink.width,
                        ink.height,
                    )
                })
            })
            .map_or(base, |ink| base.union(ink))
    }

    fn initial_insets(&self) -> Insets {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or_default()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(
            crate::ACCESSIBILITY,
            crate::AccessibilityCapability::of::<Self>(),
        )?;
        out.register(crate::BORDER, crate::BorderCapability::of::<Self>())?;
        out.register(
            super::CONTENT_SNAPSHOT,
            super::ContentSnapshotCapability::of::<Self>(),
        )?;
        out.register(
            super::RESOURCE_DEPENDENCIES,
            super::ResourceDependenciesCapability::of::<Self>(),
        )
    }
}

impl AccessibleFigure for LabelFigure {
    fn accessible_name(&self) -> Option<&str> {
        Some(&self.text)
    }

    fn accessible_role(&self) -> crate::AccessibilityRole {
        crate::AccessibilityRole::Text
    }
}

impl BorderedFigure for LabelFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(
        &mut self,
        border: super::border::BorderCandidate,
    ) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border.into_border())
    }
}

fn with_border_size(size: Dimension, border: Option<&dyn Border>) -> Dimension {
    let Some(border) = border else {
        return size;
    };
    let insets = border.get_insets();
    let preferred = border.preferred_size();
    Dimension::new(
        (size.width + insets.width()).max(preferred.width),
        (size.height + insets.height()).max(preferred.height),
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
    font: &crate::render::FontDescriptor,
    width: f64,
    full_width: f32,
) -> Result<TextLayout, TextError> {
    let constraints = TextConstraints::new(Some(width as f32))?;
    let ellipsis = engine.layout(ELLIPSIS, font, TextConstraints::UNBOUNDED)?;
    if width < ellipsis.width() as f64 {
        return ellipsis.with_visibility(source, 0..0, true, full_width, constraints);
    }
    let (end, layout) = crate::text::prefix_search::fit_prefix(
        source,
        ELLIPSIS,
        Some(ellipsis),
        |candidate| engine.layout(candidate, font, TextConstraints::UNBOUNDED),
        |layout| f64::from(layout.width()) <= width,
    )?;
    layout
        .expect("the measured ellipsis is always retained for an empty prefix")
        .with_visibility(source, 0..end, true, full_width, constraints)
}

#[cfg(test)]
mod truncation_tests {
    use super::*;
    use crate::render::{BuiltinFont, ParleyTextEngine, ResourceId};
    use std::time::Instant;
    use unicode_segmentation::UnicodeSegmentation;

    struct CountingEngine {
        inner: ParleyTextEngine,
        calls: usize,
    }

    impl CountingEngine {
        fn new() -> Self {
            let mut inner = ParleyTextEngine::new();
            for (index, font) in BuiltinFont::ALL.into_iter().enumerate() {
                inner
                    .register_font(
                        ResourceId::new(uuid::Uuid::nil(), index as u64 + 1),
                        1,
                        font.bytes(),
                    )
                    .unwrap();
            }
            Self { inner, calls: 0 }
        }
    }

    impl TextLayoutEngine for CountingEngine {
        fn revision(&self) -> u64 {
            self.inner.revision()
        }

        fn register_font(
            &mut self,
            id: ResourceId,
            revision: u64,
            bytes: &[u8],
        ) -> Result<(), TextError> {
            self.inner.register_font(id, revision, bytes)
        }

        fn remove_font(&mut self, id: ResourceId) {
            self.inner.remove_font(id);
        }

        fn layout(
            &mut self,
            text: &str,
            font: &FontDescriptor,
            constraints: TextConstraints,
        ) -> Result<TextLayout, TextError> {
            self.calls += 1;
            self.inner.layout(text, font, constraints)
        }
    }

    #[test]
    fn truncation_matches_legacy_search_and_real_layouts() {
        let mut engine = CountingEngine::new();
        let font = FontDescriptor::default();
        for source in [
            "iiiiWWWW",
            "office fi ffi",
            "e\u{301}👩‍💻中ab",
            "中文 mixed",
            "مرحبا world",
        ] {
            let full_width = engine
                .layout(source, &font, TextConstraints::UNBOUNDED)
                .unwrap()
                .width();
            let candidates = source
                .grapheme_indices(true)
                .map(|(end, _)| end)
                .chain(std::iter::once(source.len()))
                .map(|end| {
                    let layout = engine
                        .layout(
                            &format!("{}...", &source[..end]),
                            &font,
                            TextConstraints::UNBOUNDED,
                        )
                        .unwrap();
                    (end, layout)
                })
                .collect::<Vec<_>>();
            let widths = candidates.iter().flat_map(|(_, layout)| {
                [
                    f64::from(layout.width()),
                    (f64::from(layout.width()) - 0.01).max(0.0),
                ]
            });
            for width in widths {
                let actual = truncate(&mut engine, source, &font, width, full_width).unwrap();
                let mut low = 0;
                let mut high = candidates.len() - 1;
                while low < high {
                    let probe = (low + high).div_ceil(2);
                    if f64::from(candidates[probe].1.width()) <= width {
                        low = probe;
                    } else {
                        high = probe - 1;
                    }
                }
                let (end, expected) = &candidates[low];
                if candidates
                    .windows(2)
                    .all(|pair| pair[0].1.width() <= pair[1].1.width())
                {
                    let enumerated = candidates
                        .iter()
                        .rev()
                        .find(|(_, layout)| f64::from(layout.width()) <= width)
                        .unwrap_or(&candidates[0]);
                    assert_eq!(*end, enumerated.0);
                }
                let expected = expected
                    .clone()
                    .with_visibility(
                        source,
                        0..*end,
                        true,
                        full_width,
                        TextConstraints::new(Some(width as f32)).unwrap(),
                    )
                    .unwrap();
                assert_eq!(actual, expected, "source={source:?}, width={width}");
            }
        }
    }

    #[test]
    fn equal_label_updates_keep_layouts_and_real_changes_invalidate_the_right_cache() {
        let mut engine = CountingEngine::new();
        let font = FontDescriptor::default();
        let bounds = Rectangle::new(0.0, 0.0, 40.0, 30.0);
        let mut label = LabelFigure::new("office 中文");
        label.refresh_intrinsic(&mut engine, &font, None).unwrap();
        label.refresh_presentation(&mut engine, bounds).unwrap();
        let intrinsic = label.intrinsic.clone();
        let presentation = label.presentation.clone();
        let revision = label.text_revision;
        let calls = engine.calls;
        label.set_text(label.text().to_owned());
        label.set_icon(None);
        label.set_text_placement(TextPlacement::East);
        label.set_label_alignment(Alignment::Center);
        label.set_text_alignment(Alignment::Center);
        label.set_icon_alignment(Alignment::Center);
        label.set_icon_text_gap(DEFAULT_ICON_TEXT_GAP).unwrap();
        assert!(label.set_icon_text_gap(f64::NAN).is_err());
        assert!(label.set_icon_text_gap(-1.0).is_err());
        assert_eq!(label.intrinsic, intrinsic);
        assert_eq!(label.presentation, presentation);
        assert_eq!(label.text_revision, revision);
        assert!(!label.refresh_intrinsic(&mut engine, &font, None).unwrap());
        assert!(!label.refresh_presentation(&mut engine, bounds).unwrap());
        assert_eq!(engine.calls, calls);
        label.set_text_alignment(Alignment::Start);
        assert_eq!(label.intrinsic, intrinsic);
        assert!(label.presentation.is_none());
        label.set_text("changed".to_owned());
        assert_eq!(label.text_revision, revision + 1);
        assert!(label.intrinsic.is_none());
        label.refresh_intrinsic(&mut engine, &font, None).unwrap();
        label
            .set_icon_text_gap(DEFAULT_ICON_TEXT_GAP + 1.0)
            .unwrap();
        assert!(label.intrinsic.is_none());
    }

    #[test]
    #[ignore = "release cost sample; run alone with --ignored --nocapture"]
    fn draw2d_equal_label_updates_cost() {
        const ITERATIONS: usize = 100;
        let source = "Wi 中文 office ".repeat(160);
        let bounds = Rectangle::new(0.0, 0.0, 300.0, 30.0);
        let font = FontDescriptor::default();
        let mut engine = CountingEngine::new();
        let mut label = LabelFigure::new(source.clone());
        label.refresh_intrinsic(&mut engine, &font, None).unwrap();
        label.refresh_presentation(&mut engine, bounds).unwrap();
        engine.calls = 0;
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            label.set_text(source.clone());
            label.set_icon(None);
            label.set_text_placement(TextPlacement::East);
            label.set_label_alignment(Alignment::Center);
            label.set_text_alignment(Alignment::Center);
            label.set_icon_alignment(Alignment::Center);
            label.set_icon_text_gap(DEFAULT_ICON_TEXT_GAP).unwrap();
            label.refresh_intrinsic(&mut engine, &font, None).unwrap();
            label.refresh_presentation(&mut engine, bounds).unwrap();
            std::hint::black_box(label.text_layout());
        }
        println!(
            "draw2d-equal-label iterations={ITERATIONS} calls={} elapsed_us={}",
            engine.calls,
            start.elapsed().as_micros()
        );
    }

    #[test]
    #[ignore = "release cost sample; run alone with --ignored --nocapture"]
    fn draw2d_truncation_cost() {
        const ITERATIONS: usize = 20;
        let font = FontDescriptor::default();
        let mut engine = CountingEngine::new();
        let sources = [
            ("uniform", "m".repeat(2048)),
            ("mixed", "Wi e\u{301} 中文 office 👩‍💻 ".repeat(80)),
            (
                "uneven",
                format!("{}{}", "i".repeat(1024), "W".repeat(1024)),
            ),
        ];
        for (name, source) in sources {
            let full_width = engine
                .layout(&source, &font, TextConstraints::UNBOUNDED)
                .unwrap()
                .width();
            for ratio in [0.1, 0.5, 0.9] {
                let width = f64::from(full_width) * ratio;
                // Warm the same font and shaping context on both versions.
                truncate(&mut engine, &source, &font, width, full_width).unwrap();
                engine.calls = 0;
                let start = Instant::now();
                for _ in 0..ITERATIONS {
                    std::hint::black_box(
                        truncate(&mut engine, &source, &font, width, full_width).unwrap(),
                    );
                }
                println!(
                    "draw2d-truncate {name} ratio={ratio} calls={} elapsed_us={}",
                    engine.calls,
                    start.elapsed().as_micros()
                );
            }
        }
    }
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
                y
            } else {
                y + icon.1 + gap
            };
            let icon_y = if placement == TextPlacement::North {
                y + text.1 + gap
            } else {
                y
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
